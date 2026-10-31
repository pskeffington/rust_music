//! Bounded, local stdio protocol for an independently packaged Rust Music unit.
//!
//! Frames are length-prefixed binary messages. No socket, runtime network,
//! serde, or allocation proportional to an unchecked frame length is used.
//! Reads are synchronous and may block; the parent process supervisor must
//! enforce wall-clock deadlines and terminate/reap a child that exceeds them.

use rust_music_core::AudioFormat;
pub use rust_music_library::AudioContainer;
use std::io::{self, Read, Write};

pub const PROTOCOL_VERSION: u16 = 1;
pub const FRAME_MAGIC: [u8; 4] = *b"RBPK";
pub const MAX_FRAME_BYTES: usize = 16 * 1024;
pub const MAX_PACKAGE_ID_BYTES: usize = 96;
pub const MAX_PATH_BYTES: usize = 4096;
pub const MAX_TRACKS_PER_VIEW: usize = 32;
/// Absolute protocol bound. Host `serve` mode still defaults to 2,048 tracks;
/// the standalone desktop mode may opt into its independently bounded 20k cap.
pub const MAX_LIBRARY_TRACKS: u16 = 20_000;
pub const MAX_TITLE_BYTES: usize = 128;
pub const MAX_ERROR_BYTES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    Io,
    Invalid(&'static str),
    Oversized,
    Truncated,
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io => write!(f, "package process I/O failed"),
            Self::Invalid(message) => write!(f, "invalid package process frame: {message}"),
            Self::Oversized => write!(f, "package process frame exceeds 16 KiB"),
            Self::Truncated => write!(f, "truncated package process frame"),
        }
    }
}

impl std::error::Error for ProtocolError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Playback {
    Empty = 0,
    Ready = 1,
    Playing = 2,
    Paused = 6,
    Stopped = 3,
    Finished = 4,
    Failed = 5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ErrorCode {
    InvalidRequest = 1,
    Denied = 2,
    Unsupported = 3,
    Unavailable = 4,
    Decoder = 5,
    Output = 6,
    Internal = 7,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackView {
    pub index: u16,
    pub title: String,
    pub container: AudioContainer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct View {
    pub playback: Playback,
    pub selected: Option<u16>,
    pub track_count: u16,
    pub page_offset: u16,
    pub tracks: Vec<TrackView>,
    pub format: Option<AudioFormat>,
    pub frames_delivered: u64,
    pub total_frames: Option<u64>,
    /// True only while the child slot's proxy lease is held. The host owns the
    /// actual cross-module audio-focus lease.
    pub audio_claimed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    Hello { min_version: u16, max_version: u16 },
    Activate { width: u16, height: u16 },
    Suspend,
    Resume { width: u16, height: u16 },
    Resize { width: u16, height: u16 },
    ScanLibrary { root: String },
    Select { index: u16 },
    Previous,
    Next,
    Play,
    Stop,
    Pause,
    ResumePlayback,
    SetVolumePercent { percent: u8 },
    Snapshot { page: u16 },
    Shutdown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reply {
    Hello { selected_version: u16 },
    Ack,
    View(View),
    Error { code: ErrorCode, message: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Envelope<T> {
    pub package_id: String,
    pub request_id: u64,
    pub body: T,
}

fn valid_package_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_PACKAGE_ID_BYTES
        && value.split('.').count() >= 3
        && value.split('.').all(|part| {
            !part.is_empty()
                && part.len() <= 32
                && part.bytes().next().is_some_and(|b| b.is_ascii_lowercase())
                && part
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
}

fn valid_text(value: &str, max: usize) -> bool {
    value.len() <= max && !value.chars().any(char::is_control)
}

struct Writer(Vec<u8>);
impl Writer {
    fn new() -> Self {
        Self(Vec::with_capacity(256))
    }
    fn byte(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_be_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_be_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_be_bytes());
    }
    fn string(&mut self, value: &str, max: usize) -> Result<(), ProtocolError> {
        if !valid_text(value, max) || value.len() > u16::MAX as usize {
            return Err(ProtocolError::Invalid("text field outside bound"));
        }
        self.u16(value.len() as u16);
        self.0.extend_from_slice(value.as_bytes());
        Ok(())
    }
    fn finish(self) -> Result<Vec<u8>, ProtocolError> {
        if self.0.len() > MAX_FRAME_BYTES {
            Err(ProtocolError::Oversized)
        } else {
            Ok(self.0)
        }
    }
}

struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}
impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, at: 0 }
    }
    fn take(&mut self, len: usize) -> Result<&'a [u8], ProtocolError> {
        let end = self.at.checked_add(len).ok_or(ProtocolError::Truncated)?;
        let slice = self
            .data
            .get(self.at..end)
            .ok_or(ProtocolError::Truncated)?;
        self.at = end;
        Ok(slice)
    }
    fn byte(&mut self) -> Result<u8, ProtocolError> {
        Ok(self.take(1)?[0])
    }
    fn u16(&mut self) -> Result<u16, ProtocolError> {
        Ok(u16::from_be_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> Result<u32, ProtocolError> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64, ProtocolError> {
        Ok(u64::from_be_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn string(&mut self, max: usize) -> Result<String, ProtocolError> {
        let len = usize::from(self.u16()?);
        if len > max {
            return Err(ProtocolError::Invalid("text field outside bound"));
        }
        let value = std::str::from_utf8(self.take(len)?)
            .map_err(|_| ProtocolError::Invalid("text is not UTF-8"))?;
        if !valid_text(value, max) {
            return Err(ProtocolError::Invalid("control character in text"));
        }
        Ok(value.to_owned())
    }
    fn done(&self) -> Result<(), ProtocolError> {
        if self.at == self.data.len() {
            Ok(())
        } else {
            Err(ProtocolError::Invalid("trailing payload bytes"))
        }
    }
}

fn encode_header(
    kind: u8,
    package_id: &str,
    request_id: u64,
    body: Writer,
) -> Result<Vec<u8>, ProtocolError> {
    if !valid_package_id(package_id) || request_id == 0 {
        return Err(ProtocolError::Invalid("invalid package ID or request ID"));
    }
    let body = body.finish()?;
    let mut out = Writer::new();
    out.byte(PROTOCOL_VERSION as u8);
    out.byte(kind);
    out.u64(request_id);
    out.string(package_id, MAX_PACKAGE_ID_BYTES)?;
    out.0.extend_from_slice(&body);
    out.finish()
}

impl Envelope<Request> {
    pub fn encode(&self) -> Result<Vec<u8>, ProtocolError> {
        let mut body = Writer::new();
        match &self.body {
            Request::Hello {
                min_version,
                max_version,
            } => {
                body.byte(0);
                body.u16(*min_version);
                body.u16(*max_version);
            }
            Request::Activate { width, height } => {
                body.byte(1);
                body.u16(*width);
                body.u16(*height);
            }
            Request::Suspend => body.byte(2),
            Request::Resume { width, height } => {
                body.byte(3);
                body.u16(*width);
                body.u16(*height);
            }
            Request::Resize { width, height } => {
                body.byte(4);
                body.u16(*width);
                body.u16(*height);
            }
            Request::ScanLibrary { root } => {
                if !valid_text(root, MAX_PATH_BYTES) || root.is_empty() {
                    return Err(ProtocolError::Invalid("invalid library root"));
                }
                body.byte(5);
                body.string(root, MAX_PATH_BYTES)?;
            }
            Request::Select { index } => {
                body.byte(6);
                body.u16(*index);
            }
            Request::Previous => body.byte(7),
            Request::Next => body.byte(8),
            Request::Play => body.byte(9),
            Request::Stop => body.byte(10),
            Request::Snapshot { page } => {
                body.byte(11);
                body.u16(*page);
            }
            Request::Shutdown => body.byte(12),
            Request::Pause => body.byte(13),
            Request::ResumePlayback => body.byte(14),
            Request::SetVolumePercent { percent } => {
                if *percent > 100 {
                    return Err(ProtocolError::Invalid("volume must be 0..=100"));
                }
                body.byte(15);
                body.byte(*percent);
            }
        }
        encode_header(1, &self.package_id, self.request_id, body)
    }

    pub fn decode(payload: &[u8]) -> Result<Self, ProtocolError> {
        let (mut reader, package_id, request_id) = decode_header(payload, 1)?;
        let body = match reader.byte()? {
            0 => Request::Hello {
                min_version: reader.u16()?,
                max_version: reader.u16()?,
            },
            1 => Request::Activate {
                width: reader.u16()?,
                height: reader.u16()?,
            },
            2 => Request::Suspend,
            3 => Request::Resume {
                width: reader.u16()?,
                height: reader.u16()?,
            },
            4 => Request::Resize {
                width: reader.u16()?,
                height: reader.u16()?,
            },
            5 => Request::ScanLibrary {
                root: reader.string(MAX_PATH_BYTES)?,
            },
            6 => Request::Select {
                index: reader.u16()?,
            },
            7 => Request::Previous,
            8 => Request::Next,
            9 => Request::Play,
            10 => Request::Stop,
            11 => Request::Snapshot {
                page: reader.u16()?,
            },
            12 => Request::Shutdown,
            13 => Request::Pause,
            14 => Request::ResumePlayback,
            15 => Request::SetVolumePercent {
                percent: reader.byte()?,
            },
            _ => return Err(ProtocolError::Invalid("unknown request tag")),
        };
        reader.done()?;
        let envelope = Self {
            package_id,
            request_id,
            body,
        };
        // Re-encoding applies field-specific validation to decoded input.
        envelope.encode()?;
        Ok(envelope)
    }
}

impl Envelope<Reply> {
    pub fn encode(&self) -> Result<Vec<u8>, ProtocolError> {
        let mut body = Writer::new();
        match &self.body {
            Reply::Hello { selected_version } => {
                body.byte(0);
                body.u16(*selected_version);
            }
            Reply::Ack => body.byte(1),
            Reply::View(view) => {
                validate_view(view)?;
                body.byte(2);
                body.byte(view.playback as u8);
                encode_optional_u16(&mut body, view.selected);
                body.u16(view.track_count);
                body.u16(view.page_offset);
                body.byte(view.tracks.len() as u8);
                for track in &view.tracks {
                    body.u16(track.index);
                    body.string(&track.title, MAX_TITLE_BYTES)?;
                    body.byte(container_tag(track.container));
                }
                match view.format {
                    Some(format) => {
                        body.byte(1);
                        body.u32(format.sample_rate);
                        body.byte(format.channels);
                        body.byte(format.bits_per_sample);
                    }
                    None => body.byte(0),
                }
                body.u64(view.frames_delivered);
                encode_optional_u64(&mut body, view.total_frames);
                body.byte(u8::from(view.audio_claimed));
            }
            Reply::Error { code, message } => {
                if !valid_text(message, MAX_ERROR_BYTES) {
                    return Err(ProtocolError::Invalid("error text outside bound"));
                }
                body.byte(3);
                body.byte(*code as u8);
                body.string(message, MAX_ERROR_BYTES)?;
            }
        }
        encode_header(2, &self.package_id, self.request_id, body)
    }

    pub fn decode(payload: &[u8]) -> Result<Self, ProtocolError> {
        let (mut reader, package_id, request_id) = decode_header(payload, 2)?;
        let body = match reader.byte()? {
            0 => Reply::Hello {
                selected_version: reader.u16()?,
            },
            1 => Reply::Ack,
            2 => {
                let playback = playback_from(reader.byte()?)?;
                let selected = decode_optional_u16(&mut reader)?;
                let track_count = reader.u16()?;
                let page_offset = reader.u16()?;
                let count = usize::from(reader.byte()?);
                if count > MAX_TRACKS_PER_VIEW {
                    return Err(ProtocolError::Invalid("too many tracks in view"));
                }
                let mut tracks = Vec::with_capacity(count);
                for _ in 0..count {
                    tracks.push(TrackView {
                        index: reader.u16()?,
                        title: reader.string(MAX_TITLE_BYTES)?,
                        container: container_from(reader.byte()?)?,
                    });
                }
                let format = match reader.byte()? {
                    0 => None,
                    1 => Some(AudioFormat {
                        sample_rate: reader.u32()?,
                        channels: reader.byte()?,
                        bits_per_sample: reader.byte()?,
                    }),
                    _ => return Err(ProtocolError::Invalid("invalid format presence flag")),
                };
                let frames_delivered = reader.u64()?;
                let total_frames = decode_optional_u64(&mut reader)?;
                let audio_claimed = match reader.byte()? {
                    0 => false,
                    1 => true,
                    _ => return Err(ProtocolError::Invalid("invalid boolean")),
                };
                Reply::View(View {
                    playback,
                    selected,
                    track_count,
                    page_offset,
                    tracks,
                    format,
                    frames_delivered,
                    total_frames,
                    audio_claimed,
                })
            }
            3 => Reply::Error {
                code: error_from(reader.byte()?)?,
                message: reader.string(MAX_ERROR_BYTES)?,
            },
            _ => return Err(ProtocolError::Invalid("unknown reply tag")),
        };
        reader.done()?;
        if let Reply::View(view) = &body {
            validate_view(view)?;
        }
        let envelope = Self {
            package_id,
            request_id,
            body,
        };
        envelope.encode()?;
        Ok(envelope)
    }
}

fn decode_header<'a>(
    payload: &'a [u8],
    kind: u8,
) -> Result<(Reader<'a>, String, u64), ProtocolError> {
    if payload.len() > MAX_FRAME_BYTES {
        return Err(ProtocolError::Oversized);
    }
    let mut reader = Reader::new(payload);
    if reader.byte()? != PROTOCOL_VERSION as u8 || reader.byte()? != kind {
        return Err(ProtocolError::Invalid(
            "protocol version or message kind mismatch",
        ));
    }
    let request_id = reader.u64()?;
    let package_id = reader.string(MAX_PACKAGE_ID_BYTES)?;
    if !valid_package_id(&package_id) || request_id == 0 {
        return Err(ProtocolError::Invalid("invalid package ID or request ID"));
    }
    Ok((reader, package_id, request_id))
}

fn validate_view(view: &View) -> Result<(), ProtocolError> {
    if view.tracks.len() > MAX_TRACKS_PER_VIEW
        || view.track_count > MAX_LIBRARY_TRACKS
        || view.tracks.iter().enumerate().any(|(offset, track)| {
            track.index >= view.track_count
                || usize::from(track.index) != usize::from(view.page_offset) + offset
                || !valid_text(&track.title, MAX_TITLE_BYTES)
        })
        || (view.selected.is_some_and(|index| index >= view.track_count))
        || view.page_offset > view.track_count
    {
        return Err(ProtocolError::Invalid("view fields outside bounds"));
    }
    if let Some(format) = view.format {
        format
            .validate()
            .map_err(|_| ProtocolError::Invalid("invalid PCM format in view"))?;
    }
    Ok(())
}

fn encode_optional_u16(out: &mut Writer, value: Option<u16>) {
    match value {
        Some(value) => {
            out.byte(1);
            out.u16(value);
        }
        None => out.byte(0),
    }
}
fn decode_optional_u16(input: &mut Reader<'_>) -> Result<Option<u16>, ProtocolError> {
    match input.byte()? {
        0 => Ok(None),
        1 => Ok(Some(input.u16()?)),
        _ => Err(ProtocolError::Invalid("invalid option flag")),
    }
}
fn encode_optional_u64(out: &mut Writer, value: Option<u64>) {
    match value {
        Some(value) => {
            out.byte(1);
            out.u64(value);
        }
        None => out.byte(0),
    }
}
fn decode_optional_u64(input: &mut Reader<'_>) -> Result<Option<u64>, ProtocolError> {
    match input.byte()? {
        0 => Ok(None),
        1 => Ok(Some(input.u64()?)),
        _ => Err(ProtocolError::Invalid("invalid option flag")),
    }
}
fn container_tag(value: AudioContainer) -> u8 {
    match value {
        AudioContainer::Flac => 1,
        AudioContainer::Wave => 2,
        AudioContainer::Aiff => 3,
        AudioContainer::Caf => 4,
        AudioContainer::Mp4 => 5,
    }
}
fn container_from(value: u8) -> Result<AudioContainer, ProtocolError> {
    match value {
        1 => Ok(AudioContainer::Flac),
        2 => Ok(AudioContainer::Wave),
        3 => Ok(AudioContainer::Aiff),
        4 => Ok(AudioContainer::Caf),
        5 => Ok(AudioContainer::Mp4),
        _ => Err(ProtocolError::Invalid("unknown audio container")),
    }
}
fn playback_from(value: u8) -> Result<Playback, ProtocolError> {
    match value {
        0 => Ok(Playback::Empty),
        1 => Ok(Playback::Ready),
        2 => Ok(Playback::Playing),
        3 => Ok(Playback::Stopped),
        4 => Ok(Playback::Finished),
        5 => Ok(Playback::Failed),
        6 => Ok(Playback::Paused),
        _ => Err(ProtocolError::Invalid("unknown playback state")),
    }
}
fn error_from(value: u8) -> Result<ErrorCode, ProtocolError> {
    match value {
        1 => Ok(ErrorCode::InvalidRequest),
        2 => Ok(ErrorCode::Denied),
        3 => Ok(ErrorCode::Unsupported),
        4 => Ok(ErrorCode::Unavailable),
        5 => Ok(ErrorCode::Decoder),
        6 => Ok(ErrorCode::Output),
        7 => Ok(ErrorCode::Internal),
        _ => Err(ProtocolError::Invalid("unknown error code")),
    }
}

/// Writes one bounded `RBPK` frame. Payload length is checked before output.
pub fn write_frame<W: Write>(writer: &mut W, payload: &[u8]) -> Result<(), ProtocolError> {
    if payload.is_empty() || payload.len() > MAX_FRAME_BYTES {
        return Err(ProtocolError::Oversized);
    }
    writer
        .write_all(&FRAME_MAGIC)
        .map_err(|_| ProtocolError::Io)?;
    writer
        .write_all(&(payload.len() as u32).to_be_bytes())
        .map_err(|_| ProtocolError::Io)?;
    writer.write_all(payload).map_err(|_| ProtocolError::Io)?;
    writer.flush().map_err(|_| ProtocolError::Io)
}

/// Reads one bounded frame, allocating only after validating its length against
/// `MAX_FRAME_BYTES`. `None` means clean EOF before any part of the next frame;
/// partial headers/bodies fail closed. This blocking read has no deadline; the
/// parent process supervisor must bound the transaction and child lifetime.
pub fn read_frame<R: Read>(reader: &mut R) -> Result<Option<Vec<u8>>, ProtocolError> {
    let mut header = [0u8; 8];
    loop {
        match reader.read(&mut header[..1]) {
            Ok(0) => return Ok(None),
            Ok(1) => break,
            Ok(_) => unreachable!(),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(ProtocolError::Io),
        }
    }
    reader
        .read_exact(&mut header[1..])
        .map_err(|_| ProtocolError::Truncated)?;
    if header[..4] != FRAME_MAGIC {
        return Err(ProtocolError::Invalid("frame magic mismatch"));
    }
    let len = u32::from_be_bytes(header[4..8].try_into().unwrap()) as usize;
    if len == 0 {
        return Err(ProtocolError::Invalid("empty frame"));
    }
    if len > MAX_FRAME_BYTES {
        return Err(ProtocolError::Oversized);
    }
    let mut payload = vec![0; len];
    reader
        .read_exact(&mut payload)
        .map_err(|_| ProtocolError::Truncated)?;
    Ok(Some(payload))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "org.rustboy.music";

    #[test]
    fn request_roundtrip_preserves_unicode_path_and_identity() {
        let request = Envelope {
            package_id: ID.into(),
            request_id: 7,
            body: Request::ScanLibrary {
                root: "/Music/álbum".into(),
            },
        };
        let payload = request.encode().unwrap();
        assert_eq!(Envelope::<Request>::decode(&payload).unwrap(), request);
        let mut stream = Vec::new();
        write_frame(&mut stream, &payload).unwrap();
        assert_eq!(read_frame(&mut stream.as_slice()).unwrap(), Some(payload));
    }

    #[test]
    fn pause_resume_and_bounded_volume_requests_roundtrip() {
        for (request_id, body) in [
            (1, Request::Pause),
            (2, Request::ResumePlayback),
            (3, Request::SetVolumePercent { percent: 0 }),
            (4, Request::SetVolumePercent { percent: 100 }),
        ] {
            let request = Envelope {
                package_id: ID.into(),
                request_id,
                body,
            };
            let bytes = request.encode().unwrap();
            assert_eq!(Envelope::<Request>::decode(&bytes).unwrap(), request);
        }
        assert!(Envelope {
            package_id: ID.into(),
            request_id: 5,
            body: Request::SetVolumePercent { percent: 101 },
        }
        .encode()
        .is_err());
    }

    #[test]
    fn view_roundtrip_is_small_and_contains_no_absolute_path() {
        let reply = Envelope {
            package_id: ID.into(),
            request_id: 7,
            body: Reply::View(View {
                playback: Playback::Ready,
                selected: Some(0),
                track_count: 1,
                page_offset: 0,
                tracks: vec![TrackView {
                    index: 0,
                    title: "Track title".into(),
                    container: AudioContainer::Flac,
                }],
                format: Some(AudioFormat::new(44_100, 2, 16).unwrap()),
                frames_delivered: 0,
                total_frames: Some(441_000),
                audio_claimed: false,
            }),
        };
        let payload = reply.encode().unwrap();
        assert!(payload.len() < 256);
        assert_eq!(Envelope::<Reply>::decode(&payload).unwrap(), reply);
        assert!(!payload.windows(6).any(|bytes| bytes == b"/Users"));
    }

    #[test]
    fn fixed_hello_vector_locks_cross_repository_wire_layout() {
        let hello = Envelope {
            package_id: ID.into(),
            request_id: 1,
            body: Request::Hello {
                min_version: 1,
                max_version: 1,
            },
        };
        assert_eq!(
            hello.encode().unwrap(),
            [
                1, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 17, b'o', b'r', b'g', b'.', b'r', b'u', b's',
                b't', b'b', b'o', b'y', b'.', b'm', b'u', b's', b'i', b'c', 0, 0, 1, 0, 1,
            ]
        );
    }

    #[test]
    fn oversized_and_partial_frames_fail_before_large_allocation() {
        let header = [*b"RBPK", ((MAX_FRAME_BYTES + 1) as u32).to_be_bytes()].concat();
        assert_eq!(
            read_frame(&mut header.as_slice()),
            Err(ProtocolError::Oversized)
        );
        let mut short_header = &b"RBPK\0"[..];
        assert_eq!(read_frame(&mut short_header), Err(ProtocolError::Truncated));
        let mut short_payload = &b"RBPK\0\0\0\x04xy"[..];
        assert_eq!(
            read_frame(&mut short_payload),
            Err(ProtocolError::Truncated)
        );
    }

    #[test]
    fn completed_process_frames_flush_the_output_stream() {
        #[derive(Default)]
        struct FlushProbe {
            bytes: Vec<u8>,
            flushed: bool,
        }
        impl Write for FlushProbe {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                self.bytes.extend_from_slice(bytes);
                Ok(bytes.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                self.flushed = true;
                Ok(())
            }
        }
        let mut output = FlushProbe::default();
        write_frame(&mut output, &[1]).unwrap();
        assert!(output.flushed);
    }

    #[test]
    fn hostile_identity_paths_counts_and_trailing_data_are_rejected() {
        for package_id in ["", "org", "org.RustBoy.music", "org.rustboy.music/"] {
            let request = Envelope {
                package_id: package_id.into(),
                request_id: 1,
                body: Request::Stop,
            };
            assert!(request.encode().is_err(), "accepted {package_id:?}");
        }
        let request = Envelope {
            package_id: ID.into(),
            request_id: 1,
            body: Request::ScanLibrary {
                root: "bad\0path".into(),
            },
        };
        assert!(request.encode().is_err());
        let mut payload = Envelope {
            package_id: ID.into(),
            request_id: 2,
            body: Request::Stop,
        }
        .encode()
        .unwrap();
        payload.push(0);
        assert!(Envelope::<Request>::decode(&payload).is_err());
    }

    #[test]
    fn view_track_count_and_title_limits_are_checked() {
        let mut tracks = Vec::new();
        for index in 0..=MAX_TRACKS_PER_VIEW {
            tracks.push(TrackView {
                index: index as u16,
                title: "x".into(),
                container: AudioContainer::Flac,
            });
        }
        let view = View {
            playback: Playback::Empty,
            selected: None,
            track_count: 100,
            page_offset: 0,
            tracks,
            format: None,
            frames_delivered: 0,
            total_frames: None,
            audio_claimed: false,
        };
        assert!(Envelope {
            package_id: ID.into(),
            request_id: 1,
            body: Reply::View(view)
        }
        .encode()
        .is_err());
        let view = View {
            playback: Playback::Empty,
            selected: None,
            track_count: 1,
            page_offset: 0,
            tracks: vec![TrackView {
                index: 0,
                title: "x".repeat(MAX_TITLE_BYTES + 1),
                container: AudioContainer::Flac,
            }],
            format: None,
            frames_delivered: 0,
            total_frames: None,
            audio_claimed: false,
        };
        assert!(Envelope {
            package_id: ID.into(),
            request_id: 1,
            body: Reply::View(view)
        }
        .encode()
        .is_err());
    }

    #[test]
    fn standalone_track_limit_accepts_twenty_thousand_and_rejects_more() {
        let view = View {
            playback: Playback::Empty,
            selected: Some(19_999),
            track_count: 20_000,
            page_offset: 19_999,
            tracks: vec![TrackView {
                index: 19_999,
                title: "last track".into(),
                container: AudioContainer::Wave,
            }],
            format: None,
            frames_delivered: 0,
            total_frames: None,
            audio_claimed: false,
        };
        assert!(Envelope {
            package_id: ID.into(),
            request_id: 1,
            body: Reply::View(view.clone()),
        }
        .encode()
        .is_ok());

        let mut oversized = view;
        oversized.track_count = 20_001;
        assert!(Envelope {
            package_id: ID.into(),
            request_id: 2,
            body: Reply::View(oversized),
        }
        .encode()
        .is_err());
    }
}
