//! Offline decoder acceptance harness. Native device output is a separate gate.
#[cfg(any(
    all(target_os = "macos", feature = "macos-output"),
    all(target_os = "linux", feature = "alsa-output")
))]
use rust_music::PcmSink;
use rust_music::{open_file, AudioFormat, Decoder, MusicError, NullSink, Result};
#[cfg(any(
    all(target_os = "macos", feature = "macos-output"),
    all(target_os = "linux", feature = "alsa-output")
))]
use rust_music_host::{AudioFocus, AudioLease, Command, MusicSlot, MUSIC_PACKAGE_ID};
#[cfg(any(
    all(target_os = "macos", feature = "macos-output"),
    all(target_os = "linux", feature = "alsa-output")
))]
use rust_music_ipc::{
    read_frame, write_frame, AudioContainer, Envelope, ErrorCode, Playback, Reply, Request,
    TrackView, View, MAX_ERROR_BYTES, MAX_TITLE_BYTES, MAX_TRACKS_PER_VIEW, PROTOCOL_VERSION,
};
#[cfg(any(
    all(target_os = "macos", feature = "macos-output"),
    all(target_os = "linux", feature = "alsa-output")
))]
use rust_music_library::{scan, AudioContainer as LibraryContainer, Library, ScanLimits};
use std::{
    env,
    fs::OpenOptions,
    io::{Seek, SeekFrom, Write},
};
#[cfg(any(
    all(target_os = "macos", feature = "macos-output"),
    all(target_os = "linux", feature = "alsa-output")
))]
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, RecvTimeoutError, SyncSender},
        Arc,
    },
    thread,
    time::Duration,
};

const PCM_GUID: [u8; 16] = [
    1, 0, 0, 0, 0, 0, 0x10, 0, 0x80, 0, 0, 0xaa, 0, 0x38, 0x9b, 0x71,
];

fn riff_overhead(format: AudioFormat) -> u32 {
    if format.channels > 2 {
        60
    } else {
        36
    }
}

fn write_header<W: Write>(output: &mut W, format: AudioFormat, data_size: u32) -> Result<()> {
    if !matches!(format.bits_per_sample, 8 | 16 | 24 | 32) {
        return Err(MusicError::Unsupported(
            "WAV export supports 8/16/24/32-bit source",
        ));
    }
    let bytes_per_frame = u32::from(format.channels) * u32::from(format.bits_per_sample / 8);
    // RIFF size includes the one-byte pad after odd-length PCM data. The data
    // chunk's length field itself remains the original unpadded byte count.
    let riff_size = data_size
        .checked_add(riff_overhead(format))
        .and_then(|size| size.checked_add(data_size & 1))
        .ok_or(MusicError::Limit("WAV RIFF size exceeds 32-bit limit"))?;
    output.write_all(b"RIFF")?;
    output.write_all(&riff_size.to_le_bytes())?;
    output.write_all(b"WAVEfmt ")?;
    // More than two channels must use WAVE_FORMAT_EXTENSIBLE to avoid an
    // ambiguous legacy PCM header. The channel mask is intentionally zero:
    // our Decoder API carries no speaker-location metadata to invent.
    let extensible = format.channels > 2;
    output.write_all(&(if extensible { 40u32 } else { 16u32 }).to_le_bytes())?;
    output.write_all(&(if extensible { 0xfffeu16 } else { 1u16 }).to_le_bytes())?;
    output.write_all(&u16::from(format.channels).to_le_bytes())?;
    output.write_all(&format.sample_rate.to_le_bytes())?;
    output.write_all(
        &format
            .sample_rate
            .checked_mul(bytes_per_frame)
            .ok_or(MusicError::Limit("WAV byte-rate overflow"))?
            .to_le_bytes(),
    )?;
    output.write_all(&(bytes_per_frame as u16).to_le_bytes())?;
    output.write_all(&u16::from(format.bits_per_sample).to_le_bytes())?;
    if extensible {
        output.write_all(&22u16.to_le_bytes())?;
        output.write_all(&u16::from(format.bits_per_sample).to_le_bytes())?;
        output.write_all(&0u32.to_le_bytes())?;
        output.write_all(&PCM_GUID)?;
    }
    output.write_all(b"data")?;
    output.write_all(&data_size.to_le_bytes())?;
    Ok(())
}

fn encode_samples(samples: &[i32], sample_bytes: usize) -> Result<Vec<u8>> {
    let capacity = samples
        .len()
        .checked_mul(sample_bytes)
        .ok_or(MusicError::Limit("PCM export buffer size overflows"))?;
    let mut bytes = Vec::with_capacity(capacity);
    for &value in samples {
        match sample_bytes {
            1 => bytes.push((value + 128) as u8),
            2 => bytes.extend_from_slice(&(value as i16).to_le_bytes()),
            3 => bytes.extend_from_slice(&value.to_le_bytes()[..3]),
            4 => bytes.extend_from_slice(&value.to_le_bytes()),
            _ => return Err(MusicError::Unsupported("unsupported integer PCM width")),
        }
    }
    Ok(bytes)
}

fn export_wav(mut decoder: Box<dyn Decoder>, dest: &str) -> Result<()> {
    let format = decoder.format();
    let mut file = OpenOptions::new().write(true).create_new(true).open(dest)?;
    write_header(&mut file, format, 0)?;
    let sample_bytes = usize::from(format.bits_per_sample / 8);
    let mut bytes_written = 0u64;
    while let Some(block) = decoder.next_block()? {
        if block.format != format {
            return Err(MusicError::Invalid("format changed during decode"));
        }
        let bytes = encode_samples(&block.samples, sample_bytes)?;
        let total = bytes_written
            .checked_add(bytes.len() as u64)
            .ok_or(MusicError::Limit("WAV size counter overflow"))?;
        // The worst case needs an odd-data padding byte as well as the header.
        if total > u64::from(u32::MAX - riff_overhead(format) - 1) {
            return Err(MusicError::Limit(
                "WAV RIFF size limit reached; use RF64 in future",
            ));
        }
        file.write_all(&bytes)?;
        bytes_written = total;
    }
    if bytes_written & 1 != 0 {
        file.write_all(&[0])?;
    }
    file.seek(SeekFrom::Start(0))?;
    write_header(&mut file, format, bytes_written as u32)?;
    println!("decoded {bytes_written} bytes of original integer PCM into {dest}");
    Ok(())
}

fn export_wav_range(
    mut decoder: Box<dyn Decoder>,
    start_frame: u64,
    frame_count: u64,
    dest: &str,
) -> Result<()> {
    if !decoder.supports_seek() {
        return Err(MusicError::Unsupported(
            "sample-accurate seek is unsupported",
        ));
    }
    if frame_count == 0 {
        return Err(MusicError::Invalid(
            "seek range must contain at least one frame",
        ));
    }
    let total_frames = decoder.total_frames().ok_or(MusicError::Unsupported(
        "seek range needs a declared frame count",
    ))?;
    let end_frame = start_frame
        .checked_add(frame_count)
        .ok_or(MusicError::Limit("seek range frame count overflows"))?;
    if end_frame > total_frames {
        return Err(MusicError::Invalid("seek range exceeds end of track"));
    }

    let format = decoder.format();
    let sample_bytes = usize::from(format.bits_per_sample / 8);
    let data_size = frame_count
        .checked_mul(u64::from(format.channels))
        .and_then(|bytes| bytes.checked_mul(sample_bytes as u64))
        .ok_or(MusicError::Limit("seek range PCM byte count overflows"))?;
    let max_data_size = u64::from(u32::MAX - riff_overhead(format) - 1);
    if data_size > max_data_size {
        return Err(MusicError::Limit("seek range exceeds WAV RIFF size limit"));
    }
    let mut header = Vec::new();
    write_header(&mut header, format, data_size as u32)?;
    decoder.seek_frame(start_frame)?;

    let mut file = OpenOptions::new().write(true).create_new(true).open(dest)?;
    file.write_all(&header)?;
    let mut frames_remaining = frame_count;
    while frames_remaining > 0 {
        let block = decoder.next_block()?.ok_or(MusicError::Invalid(
            "decoder ended before seek range completed",
        ))?;
        if block.format != format {
            return Err(MusicError::Invalid("format changed during seek range"));
        }
        let block_frames = block.frames();
        if block_frames == 0 {
            return Err(MusicError::Invalid("decoder returned an empty seek block"));
        }
        let write_frames = frames_remaining.min(block_frames as u64) as usize;
        let sample_count = write_frames
            .checked_mul(usize::from(format.channels))
            .ok_or(MusicError::Limit("seek range sample count overflows"))?;
        let bytes = encode_samples(&block.samples[..sample_count], sample_bytes)?;
        file.write_all(&bytes)?;
        frames_remaining -= write_frames as u64;
    }
    if data_size & 1 != 0 {
        file.write_all(&[0])?;
    }
    println!("decoded {frame_count} frames from sample {start_frame} into {dest}");
    Ok(())
}
fn inspect(mut decoder: Box<dyn Decoder>) -> Result<()> {
    let format = decoder.format();
    let mut frames = 0u64;
    let mut blocks = 0u64;
    while let Some(block) = decoder.next_block()? {
        blocks += 1;
        frames += block.frames() as u64;
    }
    println!(
        "{} Hz | {} ch | {} bits | {frames} frames | {blocks} blocks | CRC verified when FLAC",
        format.sample_rate, format.channels, format.bits_per_sample
    );
    Ok(())
}
#[cfg(all(target_os = "macos", feature = "macos-output"))]
fn play(input: Box<dyn Decoder>, _device: Option<&str>) -> Result<()> {
    use rust_music::{MacAudioQueueSink, PlaybackState, Player};
    let mut player = Player::new(MacAudioQueueSink::default());
    player.load(input)?;
    player.play()?;
    while player.step()?.state == PlaybackState::Playing {}
    Ok(())
}
#[cfg(all(target_os = "linux", feature = "alsa-output"))]
fn play(input: Box<dyn Decoder>, device: Option<&str>) -> Result<()> {
    use rust_music::{AlsaSink, PlaybackState, Player};
    let device = device.ok_or(MusicError::Invalid(
        "Linux playback requires explicit hw:CARD,DEVICE",
    ))?;
    let mut player = Player::new(AlsaSink::new(device, device.starts_with("hw:"))?);
    player.load(input)?;
    player.play()?;
    while player.step()?.state == PlaybackState::Playing {}
    Ok(())
}
#[cfg(not(any(
    all(target_os = "macos", feature = "macos-output"),
    all(target_os = "linux", feature = "alsa-output")
)))]
fn play(_input: Box<dyn Decoder>, _device: Option<&str>) -> Result<()> {
    Err(MusicError::Unsupported(
        "native playback needs --features macos-output (macOS) or alsa-output (Linux)",
    ))
}

#[cfg(any(
    all(target_os = "macos", feature = "macos-output"),
    all(target_os = "linux", feature = "alsa-output")
))]
enum UnitRequest {
    Message(Envelope<Request>),
    Eof,
}

#[cfg(any(
    all(target_os = "macos", feature = "macos-output"),
    all(target_os = "linux", feature = "alsa-output")
))]
struct ProcessFocus {
    grant: Arc<AtomicBool>,
}

#[cfg(any(
    all(target_os = "macos", feature = "macos-output"),
    all(target_os = "linux", feature = "alsa-output")
))]
struct ProcessLease(Arc<AtomicBool>);
#[cfg(any(
    all(target_os = "macos", feature = "macos-output"),
    all(target_os = "linux", feature = "alsa-output")
))]
impl Drop for ProcessLease {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

#[cfg(any(
    all(target_os = "macos", feature = "macos-output"),
    all(target_os = "linux", feature = "alsa-output")
))]
impl AudioLease for ProcessLease {}

#[cfg(any(
    all(target_os = "macos", feature = "macos-output"),
    all(target_os = "linux", feature = "alsa-output")
))]
impl AudioFocus for ProcessFocus {
    fn try_acquire_music(&self) -> Option<Box<dyn AudioLease>> {
        self.grant
            .compare_exchange(true, false, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| Box::new(ProcessLease(Arc::clone(&self.grant))) as Box<dyn AudioLease>)
    }
}

#[cfg(all(target_os = "macos", feature = "macos-output"))]
fn serve(device: Option<&str>) -> Result<()> {
    if device.is_some() {
        return Err(MusicError::Invalid(
            "macOS process unit does not accept a device argument",
        ));
    }
    serve_with_sink(|| Ok(rust_music::MacAudioQueueSink::default()))
}

#[cfg(all(target_os = "linux", feature = "alsa-output"))]
fn serve(device: Option<&str>) -> Result<()> {
    let device = device.ok_or(MusicError::Invalid(
        "Linux package process requires explicit hw:CARD,DEVICE",
    ))?;
    if !device.starts_with("hw:") {
        return Err(MusicError::Invalid(
            "Linux package process requires hw:CARD,DEVICE",
        ));
    }
    let device = device.to_owned();
    serve_with_sink(move || rust_music::AlsaSink::new(&device, true))
}

#[cfg(not(any(
    all(target_os = "macos", feature = "macos-output"),
    all(target_os = "linux", feature = "alsa-output")
)))]
fn serve(_device: Option<&str>) -> Result<()> {
    Err(MusicError::Unsupported(
        "package process playback needs macos-output or alsa-output",
    ))
}

#[cfg(any(
    all(target_os = "macos", feature = "macos-output"),
    all(target_os = "linux", feature = "alsa-output")
))]
fn serve_with_sink<S, F>(make_sink: F) -> Result<()>
where
    S: PcmSink + 'static,
    F: FnOnce() -> Result<S> + Send + 'static,
{
    let input = std::io::stdin();
    let output = std::io::stdout();
    serve_stream(make_sink, input.lock(), output.lock())
}

#[cfg(any(
    all(target_os = "macos", feature = "macos-output"),
    all(target_os = "linux", feature = "alsa-output")
))]
fn serve_stream<S, F, I, O>(make_sink: F, mut input: I, mut output: O) -> Result<()>
where
    S: PcmSink + 'static,
    F: FnOnce() -> Result<S> + Send + 'static,
    I: std::io::Read,
    O: Write,
{
    let (request_tx, request_rx) = mpsc::sync_channel::<UnitRequest>(1);
    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    let worker = thread::spawn(move || process_worker(make_sink, request_rx, reply_tx));
    let mut negotiated = false;
    let mut last_request_id = 0u64;

    let protocol_result = (|| {
        while let Some(payload) = read_frame(&mut input)
            .map_err(|_| MusicError::Invalid("invalid package process frame"))?
        {
            let request = Envelope::<Request>::decode(&payload)
                .map_err(|_| MusicError::Invalid("invalid package process request"))?;
            if request.package_id != MUSIC_PACKAGE_ID {
                return Err(MusicError::Invalid("package process identity mismatch"));
            }
            if !negotiated {
                let Request::Hello {
                    min_version,
                    max_version,
                } = request.body
                else {
                    return Err(MusicError::Invalid("package process handshake required"));
                };
                if request.request_id != 1
                    || min_version > PROTOCOL_VERSION
                    || max_version < PROTOCOL_VERSION
                {
                    return Err(MusicError::Unsupported("package protocol version mismatch"));
                }
                let reply = Envelope {
                    package_id: MUSIC_PACKAGE_ID.into(),
                    request_id: request.request_id,
                    body: Reply::Hello {
                        selected_version: PROTOCOL_VERSION,
                    },
                };
                let bytes = reply
                    .encode()
                    .map_err(|_| MusicError::Invalid("cannot encode package reply"))?;
                write_frame(&mut output, &bytes)
                    .map_err(|_| MusicError::Output("package process write failed".into()))?;
                negotiated = true;
                last_request_id = 1;
                continue;
            }
            if last_request_id.checked_add(1) != Some(request.request_id) {
                return Err(MusicError::Invalid("package request sequence mismatch"));
            }
            last_request_id = request.request_id;
            let shutdown = request.body == Request::Shutdown;
            request_tx
                .send(UnitRequest::Message(request.clone()))
                .map_err(|_| MusicError::Output("package playback worker unavailable".into()))?;
            let reply = reply_rx
                .recv()
                .map_err(|_| MusicError::Output("package playback worker stopped".into()))?
                .map_err(|_| MusicError::Output("package playback worker failed".into()))?;
            let reply = Envelope {
                package_id: MUSIC_PACKAGE_ID.into(),
                request_id: request.request_id,
                body: reply,
            };
            let bytes = reply
                .encode()
                .map_err(|_| MusicError::Invalid("cannot encode package reply"))?;
            write_frame(&mut output, &bytes)
                .map_err(|_| MusicError::Output("package process write failed".into()))?;
            if shutdown {
                break;
            }
        }
        Ok(())
    })();

    let _ = request_tx.send(UnitRequest::Eof);
    let worker_result = worker
        .join()
        .map_err(|_| MusicError::Output("package playback worker panicked".into()))?;
    protocol_result?;
    worker_result
}

#[cfg(any(
    all(target_os = "macos", feature = "macos-output"),
    all(target_os = "linux", feature = "alsa-output")
))]
fn process_worker<S, F>(
    make_sink: F,
    requests: mpsc::Receiver<UnitRequest>,
    replies: SyncSender<std::result::Result<Reply, rust_music_ipc::ProtocolError>>,
) -> Result<()>
where
    S: PcmSink,
    F: FnOnce() -> Result<S>,
{
    let focus = ProcessFocus {
        grant: Arc::new(AtomicBool::new(false)),
    };
    let mut slot = MusicSlot::new(LazySink::new(make_sink), Library::empty());
    loop {
        match requests.recv_timeout(Duration::from_millis(10)) {
            Ok(UnitRequest::Eof) | Err(RecvTimeoutError::Disconnected) => {
                focus.grant.store(false, Ordering::Release);
                let _ = slot.handle(Command::Stop, &focus);
                return Ok(());
            }
            Ok(UnitRequest::Message(request)) => {
                let is_shutdown = request.body == Request::Shutdown;
                let reply = handle_unit_request(&mut slot, &focus, request);
                let _ = replies.send(Ok(reply));
                if is_shutdown {
                    focus.grant.store(false, Ordering::Release);
                    let _ = slot.handle(Command::Stop, &focus);
                    return Ok(());
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                if slot.status().playback.state == rust_music::PlaybackState::Playing
                    && slot.handle(Command::Poll, &focus).is_err()
                {
                    focus.grant.store(false, Ordering::Release);
                }
            }
        }
    }
}

#[cfg(any(
    all(target_os = "macos", feature = "macos-output"),
    all(target_os = "linux", feature = "alsa-output")
))]
fn handle_unit_request<S: PcmSink>(
    slot: &mut MusicSlot<S>,
    focus: &ProcessFocus,
    request: Envelope<Request>,
) -> Reply {
    let page = match &request.body {
        Request::Snapshot { page } => *page,
        _ => 0,
    };
    let result = match request.body {
        Request::Hello { .. } => {
            return process_error(ErrorCode::InvalidRequest, "handshake already completed")
        }
        Request::Activate { width, height } | Request::Resume { width, height } => {
            if !valid_viewport(width, height) {
                return process_error(ErrorCode::InvalidRequest, "unsupported module viewport");
            }
            Ok(())
        }
        Request::Resize { width, height } => {
            if !valid_viewport(width, height) {
                return process_error(ErrorCode::InvalidRequest, "unsupported module viewport");
            }
            Ok(())
        }
        Request::Suspend | Request::Stop => {
            focus.grant.store(false, Ordering::Release);
            slot.handle(Command::Stop, focus).map(|_| ())
        }
        Request::ScanLibrary { root } => {
            let root = std::path::Path::new(&root);
            if !root.is_absolute() {
                return process_error(ErrorCode::InvalidRequest, "library root must be absolute");
            }
            scan(root, ScanLimits::default())
                .and_then(|library| slot.replace_library(library).map(|_| ()))
        }
        Request::Select { index } => slot
            .handle(Command::Select(usize::from(index)), focus)
            .map(|_| ()),
        Request::Previous => slot.handle(Command::Previous, focus).map(|_| ()),
        Request::Next => slot.handle(Command::Next, focus).map(|_| ()),
        Request::Play => {
            focus.grant.store(true, Ordering::Release);
            let result = slot.handle(Command::Play, focus).map(|_| ());
            if result.is_err() {
                focus.grant.store(false, Ordering::Release);
            }
            result
        }
        Request::Snapshot { .. } => Ok(()),
        Request::Shutdown => {
            focus.grant.store(false, Ordering::Release);
            let _ = slot.handle(Command::Stop, focus);
            return Reply::Ack;
        }
    };
    match result {
        Ok(()) => Reply::View(make_view(slot, page)),
        Err(error) => error_reply(error),
    }
}

#[cfg(any(
    all(target_os = "macos", feature = "macos-output"),
    all(target_os = "linux", feature = "alsa-output")
))]
fn valid_viewport(width: u16, height: u16) -> bool {
    (320..=4096).contains(&width) && (240..=4096).contains(&height)
}

#[cfg(any(
    all(target_os = "macos", feature = "macos-output"),
    all(target_os = "linux", feature = "alsa-output")
))]
struct LazySink<S, F> {
    sink: Option<S>,
    make_sink: Option<F>,
}

#[cfg(any(
    all(target_os = "macos", feature = "macos-output"),
    all(target_os = "linux", feature = "alsa-output")
))]
impl<S, F> LazySink<S, F> {
    fn new(make_sink: F) -> Self {
        Self {
            sink: None,
            make_sink: Some(make_sink),
        }
    }
}

#[cfg(any(
    all(target_os = "macos", feature = "macos-output"),
    all(target_os = "linux", feature = "alsa-output")
))]
impl<S: PcmSink, F: FnOnce() -> Result<S>> PcmSink for LazySink<S, F> {
    fn open(&mut self, format: AudioFormat) -> Result<()> {
        if self.sink.is_none() {
            let make_sink = self.make_sink.take().ok_or(MusicError::Output(
                "audio sink factory already consumed".into(),
            ))?;
            self.sink = Some(make_sink()?);
        }
        self.sink.as_mut().unwrap().open(format)
    }

    fn write(&mut self, block: &rust_music::PcmBlock) -> Result<()> {
        self.sink
            .as_mut()
            .ok_or(MusicError::Output("audio sink is not open".into()))?
            .write(block)
    }

    fn finish(&mut self) -> Result<()> {
        match self.sink.as_mut() {
            Some(sink) => sink.finish(),
            None => Ok(()),
        }
    }
}

#[cfg(any(
    all(target_os = "macos", feature = "macos-output"),
    all(target_os = "linux", feature = "alsa-output")
))]
fn process_error(code: ErrorCode, message: &str) -> Reply {
    Reply::Error {
        code,
        message: message
            .chars()
            .filter(|c| !c.is_control())
            .take(MAX_ERROR_BYTES)
            .collect(),
    }
}

#[cfg(any(
    all(target_os = "macos", feature = "macos-output"),
    all(target_os = "linux", feature = "alsa-output")
))]
fn error_reply(error: MusicError) -> Reply {
    match error {
        MusicError::Unsupported(_) => process_error(
            ErrorCode::Unsupported,
            "lossless format or device profile is unsupported",
        ),
        MusicError::Invalid(_) => process_error(
            ErrorCode::InvalidRequest,
            "invalid music command or media state",
        ),
        MusicError::Limit(_) => process_error(
            ErrorCode::InvalidRequest,
            "music request exceeded a resource bound",
        ),
        MusicError::Io(_) => {
            process_error(ErrorCode::Decoder, "local music file could not be read")
        }
        MusicError::Output(_) => {
            process_error(ErrorCode::Output, "audio output could not start or stop")
        }
    }
}

#[cfg(any(
    all(target_os = "macos", feature = "macos-output"),
    all(target_os = "linux", feature = "alsa-output")
))]
fn make_view<S: PcmSink>(slot: &MusicSlot<S>, page: u16) -> View {
    let status = slot.status();
    let library = slot.library();
    let total = library.len().min(u16::MAX as usize);
    let offset = usize::from(page)
        .saturating_mul(MAX_TRACKS_PER_VIEW)
        .min(total);
    let tracks = library
        .tracks()
        .iter()
        .enumerate()
        .skip(offset)
        .take(MAX_TRACKS_PER_VIEW)
        .map(|(index, track)| TrackView {
            index: index as u16,
            title: bounded_title(track.display_title()),
            container: match track.container {
                LibraryContainer::Flac => AudioContainer::Flac,
                LibraryContainer::Wave => AudioContainer::Wave,
                LibraryContainer::Aiff => AudioContainer::Aiff,
                LibraryContainer::Caf => AudioContainer::Caf,
                LibraryContainer::Mp4 => AudioContainer::Mp4,
            },
        })
        .collect();
    let playback = match status.playback.state {
        rust_music::PlaybackState::Empty => Playback::Empty,
        rust_music::PlaybackState::Ready => Playback::Ready,
        rust_music::PlaybackState::Playing => Playback::Playing,
        rust_music::PlaybackState::Stopped => Playback::Stopped,
        rust_music::PlaybackState::Finished => Playback::Finished,
        rust_music::PlaybackState::Failed => Playback::Failed,
    };
    View {
        playback,
        selected: status.selection.and_then(|index| u16::try_from(index).ok()),
        track_count: total as u16,
        page_offset: offset as u16,
        tracks,
        format: status.playback.format,
        frames_delivered: status.playback.frames_delivered,
        total_frames: status.playback.total_frames,
        audio_claimed: status.has_audio_focus,
    }
}

#[cfg(any(
    all(target_os = "macos", feature = "macos-output"),
    all(target_os = "linux", feature = "alsa-output")
))]
fn bounded_title(value: &str) -> String {
    let mut result = String::new();
    for ch in value.chars() {
        if ch.is_control() {
            continue;
        }
        if result.len() + ch.len_utf8() > MAX_TITLE_BYTES {
            break;
        }
        result.push(ch);
    }
    result
}

fn run() -> Result<()> {
    let mut args = env::args();
    let _binary = args.next();
    let action = args.next().ok_or(MusicError::Invalid("usage: rust-music-cli scan DIRECTORY | preview DIRECTORY OUTPUT.pbm | inspect FILE | decode FILE OUTPUT.wav | decode-range FILE START_FRAME FRAME_COUNT OUTPUT.wav | play FILE [ALSA_DEVICE] | serve [ALSA_DEVICE]"))?;
    if action == "serve" {
        let device = args.next();
        if args.next().is_some() {
            return Err(MusicError::Invalid("too many package process arguments"));
        }
        return serve(device.as_deref());
    }
    let path = args
        .next()
        .ok_or(MusicError::Invalid("missing input filename"))?;
    match action.as_str() {
        "scan" => {
            use rust_music_library::{scan, ScanLimits};
            if args.next().is_some() { return Err(MusicError::Invalid("unexpected scan arguments")); }
            let library = scan(&path, ScanLimits::default())?;
            for (index, track) in library.tracks().iter().enumerate() {
                println!("{index}: {:?} | {:?} | {} bytes", track.relative_path,
                    track.container, track.size_bytes);
            }
            println!("{} tracks | {} entries checked | {} skipped",
                library.len(), library.examined_entries, library.skipped_entries);
            Ok(())
        }
        "preview" => {
            use rust_music_host::MusicSlot;
            use rust_music_library::{scan, ScanLimits};
            use rust_music_panel::{render, PanelView};
            let output = args.next().ok_or(MusicError::Invalid("preview requires OUTPUT.pbm"))?;
            if args.next().is_some() {
                return Err(MusicError::Invalid("unexpected preview arguments"));
            }
            let library = scan(&path, ScanLimits::default())?;
            let slot = MusicSlot::new(NullSink::default(), library);
            let frame = render(PanelView::from_slot(&slot));
            let mut dest = OpenOptions::new().write(true).create_new(true).open(&output)?;
            dest.write_all(b"P4\n128 64\n")?;
            dest.write_all(frame.pixels())?;
            println!("128x64 monochrome MUSIC preview: {output}; playback not started");
            Ok(())
        }
        "inspect" if args.next().is_none() => inspect(open_file(path)?),
        "play" => {
            let device = args.next();
            if args.next().is_some() { return Err(MusicError::Invalid("too many playback arguments")); }
            play(open_file(path)?, device.as_deref())
        }
        "decode" => {
            let output = args.next().ok_or(MusicError::Invalid("missing output .wav filename"))?;
            if args.next().is_some() { return Err(MusicError::Invalid("unexpected extra command arguments")); }
            export_wav(open_file(path)?, &output)
        }
        "decode-range" => {
            let start = args
                .next()
                .ok_or(MusicError::Invalid("decode-range requires START_FRAME"))?
                .parse::<u64>()
                .map_err(|_| MusicError::Invalid("START_FRAME must be an unsigned integer"))?;
            let count = args
                .next()
                .ok_or(MusicError::Invalid("decode-range requires FRAME_COUNT"))?
                .parse::<u64>()
                .map_err(|_| MusicError::Invalid("FRAME_COUNT must be an unsigned integer"))?;
            let output = args
                .next()
                .ok_or(MusicError::Invalid("decode-range requires OUTPUT.wav"))?;
            if args.next().is_some() {
                return Err(MusicError::Invalid("unexpected decode-range arguments"));
            }
            export_wav_range(open_file(path)?, start, count, &output)
        }
        _ => Err(MusicError::Invalid("usage: rust-music-cli scan DIRECTORY | preview DIRECTORY OUTPUT.pbm | inspect FILE | decode FILE OUTPUT.wav | decode-range FILE START_FRAME FRAME_COUNT OUTPUT.wav | play FILE [ALSA_DEVICE] | serve [ALSA_DEVICE]")),
    }
}
fn main() {
    if let Err(error) = run() {
        eprintln!("rust_music: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(any(
        all(target_os = "macos", feature = "macos-output"),
        all(target_os = "linux", feature = "alsa-output")
    ))]
    use rust_music_ipc::{read_frame, write_frame, Envelope, Reply, Request, PROTOCOL_VERSION};

    fn header(format: AudioFormat, data_bytes: u32) -> Vec<u8> {
        let mut out = Vec::new();
        write_header(&mut out, format, data_bytes).unwrap();
        out
    }

    #[test]
    fn mono_eight_bit_odd_data_requires_riff_padding() {
        let h = header(AudioFormat::new(44_100, 1, 8).unwrap(), 5);
        assert_eq!(h.len(), 44);
        assert_eq!(u32::from_le_bytes(h[4..8].try_into().unwrap()), 42);
        assert_eq!(&h[36..40], b"data");
        assert_eq!(u32::from_le_bytes(h[40..44].try_into().unwrap()), 5);
    }

    struct EmptyDecoder(AudioFormat);
    impl Decoder for EmptyDecoder {
        fn format(&self) -> AudioFormat {
            self.0
        }
        fn next_block(&mut self) -> Result<Option<rust_music::PcmBlock>> {
            Ok(None)
        }
    }

    #[test]
    fn export_refuses_to_overwrite_existing_audio() {
        use std::{
            fs,
            time::{SystemTime, UNIX_EPOCH},
        };
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "rust-music-no-clobber-{}-{stamp}.wav",
            std::process::id()
        ));
        fs::write(&path, b"KEEP_EXISTING_MUSIC").unwrap();
        let format = AudioFormat::new(44_100, 2, 16).unwrap();
        assert!(export_wav(Box::new(EmptyDecoder(format)), path.to_str().unwrap()).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"KEEP_EXISTING_MUSIC");
        fs::remove_file(path).unwrap();
    }

    struct SeekableDecoder {
        format: AudioFormat,
        samples: Vec<i32>,
        cursor: usize,
    }

    impl Decoder for SeekableDecoder {
        fn format(&self) -> AudioFormat {
            self.format
        }
        fn total_frames(&self) -> Option<u64> {
            Some((self.samples.len() / usize::from(self.format.channels)) as u64)
        }
        fn supports_seek(&self) -> bool {
            true
        }
        fn seek_frame(&mut self, frame: u64) -> Result<()> {
            let sample = usize::try_from(frame)
                .ok()
                .and_then(|frame| frame.checked_mul(usize::from(self.format.channels)))
                .ok_or(MusicError::Limit("test seek offset overflows"))?;
            if sample > self.samples.len() {
                return Err(MusicError::Invalid("test seek beyond end"));
            }
            self.cursor = sample;
            Ok(())
        }
        fn next_block(&mut self) -> Result<Option<rust_music::PcmBlock>> {
            if self.cursor == self.samples.len() {
                return Ok(None);
            }
            let samples = self.samples[self.cursor..].to_vec();
            self.cursor = self.samples.len();
            Ok(Some(rust_music::PcmBlock::new(self.format, samples)?))
        }
    }

    #[test]
    fn decode_range_seeks_and_writes_exact_requested_frames() {
        use std::{
            fs,
            time::{SystemTime, UNIX_EPOCH},
        };

        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "rust-music-seek-range-{}-{stamp}.wav",
            std::process::id()
        ));
        let format = AudioFormat::new(44_100, 2, 16).unwrap();
        let decoder = SeekableDecoder {
            format,
            samples: vec![10, 11, 20, 21, 30, 31, 40, 41],
            cursor: 0,
        };
        export_wav_range(Box::new(decoder), 1, 2, path.to_str().unwrap()).unwrap();
        let bytes = fs::read(&path).unwrap();
        assert_eq!(u32::from_le_bytes(bytes[40..44].try_into().unwrap()), 8);
        assert_eq!(&bytes[44..], &[20, 0, 21, 0, 30, 0, 31, 0]);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn decode_range_rejects_out_of_bounds_before_creating_output() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "rust-music-seek-range-invalid-{}-{stamp}.wav",
            std::process::id()
        ));
        let format = AudioFormat::new(44_100, 2, 16).unwrap();
        let decoder = SeekableDecoder {
            format,
            samples: vec![10, 11, 20, 21],
            cursor: 0,
        };
        assert!(export_wav_range(Box::new(decoder), 1, 2, path.to_str().unwrap()).is_err());
        assert!(!path.exists());
    }

    #[test]
    fn decode_range_rejects_unexportable_format_before_creating_output() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "rust-music-seek-range-format-{}-{stamp}.wav",
            std::process::id()
        ));
        let format = AudioFormat::new(44_100, 2, 12).unwrap();
        let decoder = SeekableDecoder {
            format,
            samples: vec![10, 11, 20, 21],
            cursor: 0,
        };
        assert!(export_wav_range(Box::new(decoder), 0, 1, path.to_str().unwrap()).is_err());
        assert!(!path.exists());
    }

    #[test]
    fn six_channel_export_uses_extensible_integer_pcm() {
        let h = header(AudioFormat::new(96_000, 6, 24).unwrap(), 18);
        assert_eq!(h.len(), 68);
        assert_eq!(u32::from_le_bytes(h[4..8].try_into().unwrap()), 78);
        assert_eq!(&h[12..16], b"fmt ");
        assert_eq!(u32::from_le_bytes(h[16..20].try_into().unwrap()), 40);
        assert_eq!(u16::from_le_bytes(h[20..22].try_into().unwrap()), 0xfffe);
        assert_eq!(u16::from_le_bytes(h[38..40].try_into().unwrap()), 24);
        assert_eq!(u32::from_le_bytes(h[40..44].try_into().unwrap()), 0);
        assert_eq!(&h[44..60], PCM_GUID.as_slice());
        assert_eq!(&h[60..64], b"data");
    }

    #[cfg(any(
        all(target_os = "macos", feature = "macos-output"),
        all(target_os = "linux", feature = "alsa-output")
    ))]
    #[test]
    fn package_stream_handshakes_activates_without_io_and_shuts_down() {
        let id = rust_music_host::MUSIC_PACKAGE_ID;
        let requests = [
            Envelope {
                package_id: id.into(),
                request_id: 1,
                body: Request::Hello {
                    min_version: PROTOCOL_VERSION,
                    max_version: PROTOCOL_VERSION,
                },
            },
            Envelope {
                package_id: id.into(),
                request_id: 2,
                body: Request::Activate {
                    width: 480,
                    height: 320,
                },
            },
            Envelope {
                package_id: id.into(),
                request_id: 3,
                body: Request::Shutdown,
            },
        ];
        let mut input = Vec::new();
        for request in requests {
            write_frame(&mut input, &request.encode().unwrap()).unwrap();
        }
        let mut output = Vec::new();
        let sink_created = Arc::new(AtomicBool::new(false));
        let sink_created_in_worker = Arc::clone(&sink_created);
        serve_stream::<NullSink, _, _, _>(
            move || {
                sink_created_in_worker.store(true, Ordering::Release);
                Ok(NullSink::default())
            },
            std::io::Cursor::new(input),
            &mut output,
        )
        .unwrap();
        assert!(!sink_created.load(Ordering::Acquire));
        let mut replies = output.as_slice();
        let hello = Envelope::<Reply>::decode(&read_frame(&mut replies).unwrap().unwrap()).unwrap();
        assert_eq!(hello.request_id, 1);
        assert_eq!(
            hello.body,
            Reply::Hello {
                selected_version: PROTOCOL_VERSION
            }
        );
        let activate =
            Envelope::<Reply>::decode(&read_frame(&mut replies).unwrap().unwrap()).unwrap();
        assert_eq!(activate.request_id, 2);
        let Reply::View(view) = activate.body else {
            panic!("activation returns view");
        };
        assert_eq!(view.playback, rust_music_ipc::Playback::Empty);
        assert_eq!(view.track_count, 0);
        assert!(!view.audio_claimed);
        let shutdown =
            Envelope::<Reply>::decode(&read_frame(&mut replies).unwrap().unwrap()).unwrap();
        assert_eq!(shutdown.request_id, 3);
        assert_eq!(shutdown.body, Reply::Ack);
        assert_eq!(read_frame(&mut replies).unwrap(), None);
    }

    #[cfg(any(
        all(target_os = "macos", feature = "macos-output"),
        all(target_os = "linux", feature = "alsa-output")
    ))]
    #[test]
    fn package_stream_rejects_skipped_request_ids() {
        let id = rust_music_host::MUSIC_PACKAGE_ID;
        let hello = Envelope {
            package_id: id.into(),
            request_id: 1,
            body: Request::Hello {
                min_version: PROTOCOL_VERSION,
                max_version: PROTOCOL_VERSION,
            },
        };
        let skipped = Envelope {
            package_id: id.into(),
            request_id: 3,
            body: Request::Shutdown,
        };
        let mut input = Vec::new();
        write_frame(&mut input, &hello.encode().unwrap()).unwrap();
        write_frame(&mut input, &skipped.encode().unwrap()).unwrap();
        let mut output = Vec::new();
        assert!(serve_stream::<NullSink, _, _, _>(
            || Ok(NullSink::default()),
            std::io::Cursor::new(input),
            &mut output,
        )
        .is_err());
    }

    #[cfg(any(
        all(target_os = "macos", feature = "macos-output"),
        all(target_os = "linux", feature = "alsa-output")
    ))]
    #[test]
    fn package_stream_rejects_wrong_identity_before_dispatch() {
        let request = Envelope {
            package_id: "org.rustboy.radio".into(),
            request_id: 1,
            body: Request::Hello {
                min_version: PROTOCOL_VERSION,
                max_version: PROTOCOL_VERSION,
            },
        };
        let mut input = Vec::new();
        write_frame(&mut input, &request.encode().unwrap()).unwrap();
        let mut output = Vec::new();
        let sink_created = Arc::new(AtomicBool::new(false));
        let sink_created_in_worker = Arc::clone(&sink_created);
        assert!(serve_stream::<NullSink, _, _, _>(
            move || {
                sink_created_in_worker.store(true, Ordering::Release);
                Ok(NullSink::default())
            },
            std::io::Cursor::new(input),
            &mut output,
        )
        .is_err());
        assert!(output.is_empty());
        assert!(!sink_created.load(Ordering::Acquire));
    }

    #[cfg(any(
        all(target_os = "macos", feature = "macos-output"),
        all(target_os = "linux", feature = "alsa-output")
    ))]
    #[test]
    fn package_stream_rejects_activation_before_handshake() {
        let request = Envelope {
            package_id: rust_music_host::MUSIC_PACKAGE_ID.into(),
            request_id: 1,
            body: Request::Activate {
                width: 480,
                height: 320,
            },
        };
        let mut input = Vec::new();
        write_frame(&mut input, &request.encode().unwrap()).unwrap();
        let mut output = Vec::new();
        let sink_created = Arc::new(AtomicBool::new(false));
        let sink_created_in_worker = Arc::clone(&sink_created);
        assert!(serve_stream::<NullSink, _, _, _>(
            move || {
                sink_created_in_worker.store(true, Ordering::Release);
                Ok(NullSink::default())
            },
            std::io::Cursor::new(input),
            &mut output,
        )
        .is_err());
        assert!(output.is_empty());
        assert!(!sink_created.load(Ordering::Acquire));
    }
}
