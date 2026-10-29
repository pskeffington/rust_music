//! Bounded CAF demultiplexer for signed integer LPCM and locally decoded ALAC.
//! Unsupported formats, layouts, packet tables and codecs fail closed.
#![forbid(unsafe_code)]

use rust_music_alac::{decode_packet, AlacConfig};
use rust_music_core::{AudioFormat, Decoder, MusicError, PcmBlock, Result};
use std::io::{Read, Seek, SeekFrom};

const HEADER_BYTES: u64 = 8;
const CHUNK_HEADER_BYTES: u64 = 12;
const DESC_BYTES: usize = 32;
const MAX_CHUNKS: usize = 512;
const MAX_CHUNK_BYTES: u64 = 16 * 1024 * 1024;
const MAX_TOTAL_METADATA: u64 = 64 * 1024 * 1024;
const MAX_PACKETS: usize = 1_000_000;
const MAX_PACKET_BYTES: u32 = 8 * 1024 * 1024;
const BLOCK_FRAMES: usize = 4096;
const FLAG_LPCM_FLOAT: u32 = 1;
const FLAG_LPCM_LITTLE_ENDIAN: u32 = 2;

fn u16be(data: &[u8]) -> u16 {
    u16::from_be_bytes(data.try_into().unwrap())
}

fn u32be(data: &[u8]) -> u32 {
    u32::from_be_bytes(data.try_into().unwrap())
}

fn u64be(data: &[u8]) -> u64 {
    u64::from_be_bytes(data.try_into().unwrap())
}

fn i64be(data: &[u8]) -> i64 {
    i64::from_be_bytes(data.try_into().unwrap())
}

/// CAF stores sample rate as IEEE-754 binary64. Accept exact whole-Hz values
/// using integer decomposition so no floating-point conversion rounds it.
fn integer_rate(bits: u64) -> Result<u32> {
    let sign = bits >> 63;
    let exponent = ((bits >> 52) & 0x7ff) as i32;
    let fraction = bits & ((1u64 << 52) - 1);
    if sign != 0 || exponent == 0 || exponent == 0x7ff {
        return Err(MusicError::Invalid(
            "CAF sample rate is not positive finite binary64",
        ));
    }
    let significand = (1u64 << 52) | fraction;
    let shift = exponent - 1023 - 52;
    let rate = if shift >= 0 {
        if shift >= 32 || significand > (u32::MAX as u64 >> shift) {
            return Err(MusicError::Unsupported("CAF sample rate is out of range"));
        }
        significand << shift
    } else {
        let right = (-shift) as u32;
        if right >= 64 || significand & ((1u64 << right) - 1) != 0 {
            return Err(MusicError::Unsupported(
                "fractional CAF sample rate is unsupported",
            ));
        }
        significand >> right
    };
    u32::try_from(rate).map_err(|_| MusicError::Unsupported("CAF sample rate is out of range"))
}

#[derive(Clone, Copy)]
struct Description {
    rate: u32,
    codec: [u8; 4],
    flags: u32,
    bytes_per_packet: u32,
    frames_per_packet: u32,
    channels: u8,
    bits: u8,
}

fn parse_description(bytes: &[u8]) -> Result<Description> {
    if bytes.len() != DESC_BYTES {
        return Err(MusicError::Invalid(
            "CAF desc chunk must be exactly 32 bytes",
        ));
    }
    let rate = integer_rate(u64be(&bytes[..8]))?;
    let codec: [u8; 4] = bytes[8..12].try_into().unwrap();
    let channels = u8::try_from(u32be(&bytes[24..28]))
        .map_err(|_| MusicError::Unsupported("CAF channel count exceeds supported range"))?;
    let bits = u8::try_from(u32be(&bytes[28..32]))
        .map_err(|_| MusicError::Unsupported("CAF bit depth exceeds supported range"))?;
    if channels == 0 {
        return Err(MusicError::Invalid("CAF channel count is zero"));
    }
    Ok(Description {
        rate,
        codec,
        flags: u32be(&bytes[12..16]),
        bytes_per_packet: u32be(&bytes[16..20]),
        frames_per_packet: u32be(&bytes[20..24]),
        channels,
        bits,
    })
}

fn parse_caf_alac_cookie(bytes: &[u8]) -> Result<AlacConfig> {
    let raw_cookie = if bytes.len() == 24 {
        bytes
    } else if bytes.len() == 48
        && u32be(&bytes[..4]) == 12
        && &bytes[4..8] == b"frma"
        && &bytes[8..12] == b"alac"
        && u32be(&bytes[12..16]) == 36
        && &bytes[16..20] == b"alac"
        && bytes[20..24] == [0; 4]
    {
        &bytes[24..48]
    } else {
        return Err(MusicError::Invalid(
            "CAF ALAC cookie has an unsupported atom layout",
        ));
    };
    AlacConfig::parse(raw_cookie)
}

#[derive(Clone, Copy)]
enum PcmEndian {
    Big,
    Little,
}

#[derive(Clone, Copy)]
struct PcmSpec {
    endian: PcmEndian,
    sample_bytes: usize,
    stride: usize,
    format: AudioFormat,
}

fn pcm_spec(desc: Description) -> Result<PcmSpec> {
    if desc.channels > 2 {
        return Err(MusicError::Unsupported(
            "CAF LPCM currently supports mono or stereo only",
        ));
    }
    if desc.flags & FLAG_LPCM_FLOAT != 0
        || desc.flags & !(FLAG_LPCM_FLOAT | FLAG_LPCM_LITTLE_ENDIAN) != 0
    {
        return Err(MusicError::Unsupported(
            "CAF floating-point or reserved LPCM flags are unsupported",
        ));
    }
    if desc.frames_per_packet != 1 || desc.bytes_per_packet == 0 {
        return Err(MusicError::Unsupported(
            "CAF LPCM requires constant one-frame packets",
        ));
    }
    if !matches!(desc.bits, 8 | 16 | 24 | 32) {
        return Err(MusicError::Unsupported(
            "CAF LPCM requires 8/16/24/32-bit integer samples",
        ));
    }
    let sample_bytes = usize::from(desc.bits).div_ceil(8);
    let channels = usize::from(desc.channels);
    let stride = usize::try_from(desc.bytes_per_packet)
        .map_err(|_| MusicError::Limit("CAF LPCM packet size is too large"))?
        .checked_div(channels)
        .ok_or(MusicError::Invalid("CAF LPCM packet has no channels"))?;
    if stride * channels != desc.bytes_per_packet as usize
        || !((stride == sample_bytes) || (desc.bits == 24 && stride == 4))
    {
        return Err(MusicError::Unsupported(
            "CAF LPCM packing or channel stride is unsupported",
        ));
    }
    Ok(PcmSpec {
        endian: if desc.flags & FLAG_LPCM_LITTLE_ENDIAN != 0 {
            PcmEndian::Little
        } else {
            PcmEndian::Big
        },
        sample_bytes,
        stride,
        format: AudioFormat::new(desc.rate, desc.channels, desc.bits)?,
    })
}

#[derive(Clone, Copy)]
struct PacketRecord {
    offset: u64,
    bytes: u32,
    frames: u32,
    trim_after: u32,
}

struct PacketTable {
    packet_count: u64,
    valid_frames: u64,
    priming_frames: u64,
    remainder_frames: u64,
    sizes: Vec<(u32, u32)>,
}

fn read_varint(data: &[u8], cursor: &mut usize) -> Result<u32> {
    let mut value = 0u32;
    let mut first_group = true;
    for _ in 0..5 {
        let byte = *data
            .get(*cursor)
            .ok_or(MusicError::Invalid("truncated CAF packet table"))?;
        *cursor += 1;
        let group = u32::from(byte & 0x7f);
        if first_group && byte & 0x80 != 0 && group == 0 {
            return Err(MusicError::Invalid("noncanonical CAF packet-table integer"));
        }
        first_group = false;
        if value > (u32::MAX >> 7) {
            return Err(MusicError::Limit("CAF packet table integer overflow"));
        }
        value = (value << 7)
            .checked_add(group)
            .ok_or(MusicError::Limit("CAF packet table integer overflow"))?;
        if byte & 0x80 == 0 {
            return Ok(value);
        }
    }
    Err(MusicError::Invalid("overlong CAF packet table integer"))
}

fn parse_packet_table(data: &[u8], desc: Description, config: AlacConfig) -> Result<PacketTable> {
    if data.len() < 24 {
        return Err(MusicError::Invalid(
            "CAF pakt chunk is shorter than its header",
        ));
    }
    let signed_packets = i64be(&data[..8]);
    let signed_valid = i64be(&data[8..16]);
    let signed_priming = i64::from(i32::from_be_bytes(data[16..20].try_into().unwrap()));
    let signed_remainder = i64::from(i32::from_be_bytes(data[20..24].try_into().unwrap()));
    if signed_packets < 0 || signed_valid < 0 || signed_priming < 0 || signed_remainder < 0 {
        return Err(MusicError::Invalid("negative CAF packet-table value"));
    }
    let packet_count = signed_packets as u64;
    if packet_count > MAX_PACKETS as u64 {
        return Err(MusicError::Limit(
            "CAF packet count exceeds configured bound",
        ));
    }
    let mut sizes = Vec::new();
    if desc.bytes_per_packet == 0 || desc.frames_per_packet == 0 {
        if packet_count == 0 {
            return Err(MusicError::Invalid(
                "variable CAF packets require packet descriptions",
            ));
        }
        sizes.reserve(packet_count as usize);
        let mut cursor = 24;
        for _ in 0..packet_count {
            let bytes = if desc.bytes_per_packet == 0 {
                read_varint(data, &mut cursor)?
            } else {
                desc.bytes_per_packet
            };
            let frames = if desc.frames_per_packet == 0 {
                read_varint(data, &mut cursor)?
            } else {
                desc.frames_per_packet
            };
            if bytes == 0 || bytes > MAX_PACKET_BYTES || frames == 0 || frames > config.frame_length
            {
                return Err(MusicError::Limit(
                    "CAF ALAC packet description exceeds configured bounds",
                ));
            }
            sizes.push((bytes, frames));
        }
        if data[cursor..].iter().any(|byte| *byte != 0) {
            return Err(MusicError::Invalid(
                "nonzero trailing CAF packet-table data",
            ));
        }
    } else if packet_count != 0 || data[24..].iter().any(|byte| *byte != 0) {
        return Err(MusicError::Invalid(
            "constant-packet CAF table has unexpected packet entries",
        ));
    }
    Ok(PacketTable {
        packet_count,
        valid_frames: signed_valid as u64,
        priming_frames: signed_priming as u64,
        remainder_frames: signed_remainder as u64,
        sizes,
    })
}

enum Payload {
    Pcm {
        spec: PcmSpec,
        frame_bytes: usize,
    },
    Alac {
        config: AlacConfig,
        packets: Vec<PacketRecord>,
    },
}

/// Streaming CAF decoder. It reads bounded PCM blocks or one bounded ALAC
/// packet at a time and never invokes AudioToolbox or another OS codec.
pub struct CafDecoder<R: Read + Seek + Send> {
    input: R,
    payload: Payload,
    format: AudioFormat,
    data_offset: u64,
    data_bytes: u64,
    pcm_cursor: u64,
    packet_cursor: usize,
    front_trim: u64,
    total_frames: u64,
    output_frames: u64,
}

impl<R: Read + Seek + Send> CafDecoder<R> {
    pub fn new(mut input: R) -> Result<Self> {
        let file_end = input.seek(SeekFrom::End(0))?;
        if file_end < HEADER_BYTES {
            return Err(MusicError::Invalid("truncated CAF header"));
        }
        input.seek(SeekFrom::Start(0))?;
        let mut header = [0u8; 8];
        input.read_exact(&mut header)?;
        if &header[..4] != b"caff" || u16be(&header[4..6]) != 1 || u16be(&header[6..8]) != 0 {
            return Err(MusicError::Unsupported("unsupported CAF header or version"));
        }
        let mut position = HEADER_BYTES;
        let mut metadata_bytes = 0u64;
        let mut desc = None;
        let mut cookie = None;
        let mut pakt = None;
        let mut data = None;
        let mut channel_layout = None;
        let mut data_is_last = false;

        for chunk_index in 0..MAX_CHUNKS {
            if position == file_end {
                break;
            }
            if file_end - position < CHUNK_HEADER_BYTES {
                return Err(MusicError::Invalid("truncated CAF chunk header"));
            }
            input.seek(SeekFrom::Start(position))?;
            let mut chunk_header = [0u8; CHUNK_HEADER_BYTES as usize];
            input.read_exact(&mut chunk_header)?;
            let kind: [u8; 4] = chunk_header[..4].try_into().unwrap();
            let signed_size = i64be(&chunk_header[4..12]);
            let payload_offset = position + CHUNK_HEADER_BYTES;
            let (chunk_size, next_position) = if signed_size == -1 && &kind == b"data" {
                (file_end - payload_offset, file_end)
            } else {
                if signed_size < 0 {
                    return Err(MusicError::Invalid("invalid negative CAF chunk size"));
                }
                let size = signed_size as u64;
                let next = payload_offset
                    .checked_add(size)
                    .ok_or(MusicError::Limit("CAF chunk offset overflow"))?;
                if next > file_end {
                    return Err(MusicError::Invalid("CAF chunk exceeds file boundary"));
                }
                (size, next)
            };
            if chunk_index == 0 && (&kind != b"desc" || chunk_size != DESC_BYTES as u64) {
                return Err(MusicError::Invalid(
                    "CAF desc must be the first chunk and exactly 32 bytes",
                ));
            }
            match &kind {
                b"desc" => {
                    if desc.is_some() || chunk_size != DESC_BYTES as u64 {
                        return Err(MusicError::Invalid("duplicate or invalid CAF desc chunk"));
                    }
                    let mut bytes = [0u8; DESC_BYTES];
                    input.read_exact(&mut bytes)?;
                    desc = Some(parse_description(&bytes)?);
                }
                b"kuki" => {
                    if cookie.is_some() || chunk_size > 4096 {
                        return Err(MusicError::Invalid("duplicate or oversized CAF kuki chunk"));
                    }
                    let mut bytes = vec![0u8; chunk_size as usize];
                    input.read_exact(&mut bytes)?;
                    cookie = Some(bytes);
                    metadata_bytes += chunk_size;
                }
                b"pakt" => {
                    if pakt.is_some() || !(24..=MAX_CHUNK_BYTES).contains(&chunk_size) {
                        return Err(MusicError::Invalid("duplicate or invalid CAF pakt chunk"));
                    }
                    let mut bytes = vec![0u8; chunk_size as usize];
                    input.read_exact(&mut bytes)?;
                    pakt = Some(bytes);
                    metadata_bytes = metadata_bytes
                        .checked_add(chunk_size)
                        .ok_or(MusicError::Limit("CAF metadata size overflow"))?;
                }
                b"data" => {
                    if data.is_some() || chunk_size < 4 {
                        return Err(MusicError::Invalid("duplicate or short CAF data chunk"));
                    }
                    if signed_size == -1 && next_position != file_end {
                        return Err(MusicError::Invalid(
                            "unknown-sized CAF data must end the file",
                        ));
                    }
                    input.seek(SeekFrom::Start(payload_offset))?;
                    let mut edit_count = [0u8; 4];
                    input.read_exact(&mut edit_count)?;
                    data = Some((payload_offset + 4, chunk_size - 4));
                    data_is_last = signed_size == -1;
                }
                b"chan" => {
                    if channel_layout.is_some() || chunk_size != 12 {
                        return Err(MusicError::Unsupported(
                            "CAF custom or duplicate channel layouts are unsupported",
                        ));
                    }
                    let mut layout = [0u8; 12];
                    input.read_exact(&mut layout)?;
                    let tag = u32be(&layout[..4]);
                    if u32be(&layout[4..8]) != 0 || u32be(&layout[8..12]) != 0 {
                        return Err(MusicError::Unsupported(
                            "CAF channel bitmaps and explicit descriptions are unsupported",
                        ));
                    }
                    channel_layout = Some(tag);
                }
                _ => {
                    metadata_bytes = metadata_bytes
                        .checked_add(chunk_size)
                        .ok_or(MusicError::Limit("CAF metadata size overflow"))?;
                    if chunk_size > MAX_CHUNK_BYTES {
                        return Err(MusicError::Limit(
                            "CAF metadata chunk exceeds configured bound",
                        ));
                    }
                }
            }
            if metadata_bytes > MAX_TOTAL_METADATA {
                return Err(MusicError::Limit("CAF metadata exceeds aggregate bound"));
            }
            position = next_position;
            if data_is_last {
                break;
            }
        }
        if position != file_end {
            return Err(MusicError::Limit(
                "CAF chunk count exceeds configured bound",
            ));
        }
        let desc = desc.ok_or(MusicError::Invalid("CAF desc chunk is missing"))?;
        if let Some(tag) = channel_layout {
            let supported = matches!(
                (tag, desc.channels),
                (0x0064_0001, 1) | (0x0065_0002, 2) | (0x0066_0002, 2)
            );
            if !supported {
                return Err(MusicError::Unsupported(
                    "CAF channel layout tag is unsupported or disagrees with channel count",
                ));
            }
        }
        let (data_offset, data_bytes) =
            data.ok_or(MusicError::Invalid("CAF data chunk is missing"))?;
        let payload = match &desc.codec {
            b"lpcm" => {
                if cookie.is_some() {
                    return Err(MusicError::Invalid(
                        "CAF LPCM must not contain an ALAC cookie",
                    ));
                }
                let spec = pcm_spec(desc)?;
                let frame_bytes = desc.bytes_per_packet as usize;
                if data_bytes % frame_bytes as u64 != 0 {
                    return Err(MusicError::Invalid("CAF LPCM data ends in a partial frame"));
                }
                Payload::Pcm { spec, frame_bytes }
            }
            b"alac" => {
                if desc.flags != 0 || desc.bits != 0 || desc.channels > 2 {
                    return Err(MusicError::Unsupported(
                        "CAF ALAC description flags, depth or channel count are unsupported",
                    ));
                }
                let cookie = cookie.ok_or(MusicError::Invalid("CAF ALAC cookie is required"))?;
                let config = parse_caf_alac_cookie(&cookie)?;
                if config.format.sample_rate != desc.rate
                    || config.channels != desc.channels
                    || (desc.frames_per_packet != 0
                        && desc.frames_per_packet != config.frame_length)
                    || data_bytes > (MAX_PACKETS as u64) * u64::from(MAX_PACKET_BYTES)
                {
                    return Err(MusicError::Invalid(
                        "CAF ALAC description disagrees with its cookie or data bounds",
                    ));
                }
                let packets =
                    build_packets(data_offset, data_bytes, desc, config, pakt.as_deref())?;
                Payload::Alac { config, packets }
            }
            _ => {
                return Err(MusicError::Unsupported(
                    "CAF codec is not ALAC or integer LPCM",
                ))
            }
        };
        let (front_trim, total_frames) = match (&payload, pakt.as_deref(), &desc.codec) {
            (Payload::Pcm { frame_bytes, .. }, table, _) => {
                let physical_frames = data_bytes / *frame_bytes as u64;
                if let Some(table) = table {
                    let parsed = parse_pcm_packet_table(table, physical_frames)?;
                    if parsed.0 != 0 || parsed.1 != 0 {
                        return Err(MusicError::Unsupported(
                            "CAF LPCM priming or remainder trimming is unsupported",
                        ));
                    }
                    (0, parsed.2)
                } else {
                    (0, physical_frames)
                }
            }
            (Payload::Alac { packets, config }, table, _) => {
                if let Some(bytes) = table {
                    let parsed = parse_packet_table(bytes, desc, *config)?;
                    let physical_frames = declared_packet_frames(desc, packets.len(), &parsed)?;
                    validate_packet_accounting(&parsed, packets.len(), physical_frames)?;
                    (parsed.priming_frames, parsed.valid_frames)
                } else {
                    let physical_frames =
                        packets.iter().map(|packet| u64::from(packet.frames)).sum();
                    (0, physical_frames)
                }
            }
        };
        let format = match &payload {
            Payload::Pcm { spec, .. } => spec.format,
            Payload::Alac { config, .. } => config.format,
        };
        Ok(Self {
            input,
            payload,
            format,
            data_offset,
            data_bytes,
            pcm_cursor: 0,
            packet_cursor: 0,
            front_trim,
            total_frames,
            output_frames: 0,
        })
    }
}

fn parse_pcm_packet_table(bytes: &[u8], physical_frames: u64) -> Result<(u64, u64, u64)> {
    if bytes.len() != 24 {
        return Err(MusicError::Invalid(
            "CAF LPCM packet table must contain only its fixed header",
        ));
    }
    let packets = i64be(&bytes[..8]);
    let valid = i64be(&bytes[8..16]);
    let priming = i32::from_be_bytes(bytes[16..20].try_into().unwrap());
    let remainder = i32::from_be_bytes(bytes[20..24].try_into().unwrap());
    if packets != 0
        || valid < 0
        || priming < 0
        || remainder < 0
        || valid as u64 + priming as u64 + remainder as u64 != physical_frames
    {
        return Err(MusicError::Invalid(
            "CAF LPCM packet table does not match PCM frame count",
        ));
    }
    if priming != 0 || remainder != 0 {
        return Err(MusicError::Unsupported(
            "CAF LPCM priming or remainder trimming is unsupported",
        ));
    }
    Ok((0, 0, valid as u64))
}

fn validate_packet_accounting(
    table: &PacketTable,
    packets: usize,
    physical_frames: u64,
) -> Result<()> {
    if table.packet_count != 0 && table.packet_count != packets as u64 {
        return Err(MusicError::Invalid(
            "CAF packet table count disagrees with data chunk",
        ));
    }
    let accounted = table
        .valid_frames
        .checked_add(table.priming_frames)
        .and_then(|n| n.checked_add(table.remainder_frames))
        .ok_or(MusicError::Limit("CAF packet-table frame total overflow"))?;
    if accounted != physical_frames
        || table.priming_frames > physical_frames
        || table.remainder_frames > physical_frames
    {
        return Err(MusicError::Invalid(
            "CAF packet-table frames disagree with decoded packet duration",
        ));
    }
    Ok(())
}

fn declared_packet_frames(desc: Description, packets: usize, table: &PacketTable) -> Result<u64> {
    if desc.frames_per_packet != 0 {
        return u64::from(desc.frames_per_packet)
            .checked_mul(packets as u64)
            .ok_or(MusicError::Limit("CAF ALAC frame count overflow"));
    }
    table.sizes.iter().try_fold(0u64, |total, (_, frames)| {
        total
            .checked_add(u64::from(*frames))
            .ok_or(MusicError::Limit("CAF ALAC frame count overflow"))
    })
}

fn build_packets(
    data_offset: u64,
    data_bytes: u64,
    desc: Description,
    config: AlacConfig,
    pakt_bytes: Option<&[u8]>,
) -> Result<Vec<PacketRecord>> {
    if (desc.bytes_per_packet == 0 || desc.frames_per_packet == 0) && pakt_bytes.is_none() {
        return Err(MusicError::Invalid("variable-size CAF ALAC requires pakt"));
    }
    if data_bytes == 0 {
        return Ok(Vec::new());
    }
    if data_bytes > u64::from(MAX_PACKET_BYTES) * MAX_PACKETS as u64 {
        return Err(MusicError::Limit("CAF ALAC data exceeds configured bound"));
    }
    let table = pakt_bytes
        .map(|bytes| parse_packet_table(bytes, desc, config))
        .transpose()?;
    let mut packet_sizes = Vec::new();
    let mut packet_frames = Vec::new();
    let mut packet_trims = Vec::new();
    if desc.bytes_per_packet != 0 && desc.frames_per_packet != 0 {
        if data_bytes % u64::from(desc.bytes_per_packet) != 0 {
            return Err(MusicError::Invalid(
                "CAF fixed-size ALAC data ends in a partial packet",
            ));
        }
        let count = usize::try_from(data_bytes / u64::from(desc.bytes_per_packet))
            .map_err(|_| MusicError::Limit("CAF packet count exceeds address space"))?;
        if count > MAX_PACKETS {
            return Err(MusicError::Limit(
                "CAF packet count exceeds configured bound",
            ));
        }
        packet_sizes.resize(count, desc.bytes_per_packet);
        packet_frames.resize(count, desc.frames_per_packet);
        packet_trims.resize(count, 0);
        if let Some(table) = &table {
            if table.packet_count != 0 || !table.sizes.is_empty() {
                return Err(MusicError::Invalid(
                    "constant-size CAF ALAC has variable packet descriptions",
                ));
            }
            if table.remainder_frames > u64::from(desc.frames_per_packet) {
                return Err(MusicError::Invalid(
                    "CAF ALAC remainder exceeds packet size",
                ));
            }
            if let Some(last) = packet_trims.last_mut() {
                *last = table.remainder_frames as u32;
            }
        }
    } else {
        let table = table
            .as_ref()
            .ok_or(MusicError::Invalid("variable-size CAF ALAC requires pakt"))?;
        if table.sizes.is_empty() {
            return Err(MusicError::Invalid(
                "variable-size CAF ALAC packet list is empty",
            ));
        }
        for &(bytes, frames) in &table.sizes {
            packet_sizes.push(bytes);
            packet_frames.push(frames);
            packet_trims.push(0);
        }
        if table.remainder_frames != 0 {
            if table.remainder_frames > u64::from(*packet_frames.last().unwrap_or(&0)) {
                return Err(MusicError::Invalid(
                    "CAF ALAC remainder exceeds final packet",
                ));
            }
            if let Some(last) = packet_trims.last_mut() {
                *last = table.remainder_frames as u32;
            }
        }
    }
    let total_packet_bytes: u64 = packet_sizes.iter().map(|size| u64::from(*size)).sum();
    if total_packet_bytes != data_bytes {
        return Err(MusicError::Invalid(
            "CAF packet sizes do not exactly cover data chunk",
        ));
    }
    let mut packets = Vec::with_capacity(packet_sizes.len());
    let mut offset = data_offset;
    for ((bytes, frames), trim_after) in packet_sizes
        .into_iter()
        .zip(packet_frames)
        .zip(packet_trims)
    {
        if bytes == 0
            || bytes > MAX_PACKET_BYTES
            || frames == 0
            || frames > config.frame_length
            || trim_after >= frames
        {
            return Err(MusicError::Limit(
                "CAF ALAC packet is outside configured limits",
            ));
        }
        packets.push(PacketRecord {
            offset,
            bytes,
            frames,
            trim_after,
        });
        offset = offset
            .checked_add(u64::from(bytes))
            .ok_or(MusicError::Limit("CAF packet offset overflow"))?;
    }
    Ok(packets)
}

impl<R: Read + Seek + Send> Decoder for CafDecoder<R> {
    fn format(&self) -> AudioFormat {
        self.format
    }

    fn total_frames(&self) -> Option<u64> {
        Some(self.total_frames)
    }

    fn next_block(&mut self) -> Result<Option<PcmBlock>> {
        if self.output_frames >= self.total_frames {
            return Ok(None);
        }
        match &self.payload {
            Payload::Pcm { spec, frame_bytes } => {
                let physical_frames = self.data_bytes / *frame_bytes as u64;
                let frames_left = physical_frames.saturating_sub(self.pcm_cursor);
                if frames_left == 0 {
                    return Err(MusicError::Invalid(
                        "CAF PCM ended before declared frame count",
                    ));
                }
                let frames = frames_left.min(BLOCK_FRAMES as u64) as usize;
                let byte_len = frames
                    .checked_mul(*frame_bytes)
                    .ok_or(MusicError::Limit("CAF PCM block size overflow"))?;
                let mut bytes = vec![0u8; byte_len];
                self.input.seek(SeekFrom::Start(
                    self.data_offset + self.pcm_cursor * *frame_bytes as u64,
                ))?;
                self.input.read_exact(&mut bytes)?;
                let mut samples = Vec::with_capacity(frames * usize::from(spec.format.channels));
                for sample in bytes.chunks_exact(spec.stride) {
                    let value = decode_integer_sample(sample, spec)?;
                    samples.push(value);
                }
                self.pcm_cursor += frames as u64;
                let block = PcmBlock::new(self.format, samples)?;
                self.output_frames += block.frames() as u64;
                Ok(Some(block))
            }
            Payload::Alac { config, packets } => {
                while let Some(packet) = packets.get(self.packet_cursor).copied() {
                    self.packet_cursor += 1;
                    let mut bytes = vec![0u8; packet.bytes as usize];
                    self.input.seek(SeekFrom::Start(packet.offset))?;
                    self.input.read_exact(&mut bytes)?;
                    let mut block = decode_packet(*config, &bytes)?;
                    let decoded_frames = block.frames() as u32;
                    let minimum_frames = packet.frames - packet.trim_after;
                    if decoded_frames < minimum_frames || decoded_frames > packet.frames {
                        return Err(MusicError::Invalid(
                            "CAF ALAC packet frame count disagrees with table",
                        ));
                    }
                    let kept_packet_frames = decoded_frames.min(minimum_frames) as usize;
                    block
                        .samples
                        .truncate(kept_packet_frames * usize::from(self.format.channels));
                    let skip_frames = self.front_trim.min(block.frames() as u64) as usize;
                    self.front_trim -= skip_frames as u64;
                    if skip_frames != 0 {
                        block
                            .samples
                            .drain(..skip_frames * usize::from(self.format.channels));
                    }
                    let frames_available = block.frames() as u64;
                    let keep_frames = frames_available.min(self.total_frames - self.output_frames);
                    if keep_frames == 0 {
                        continue;
                    }
                    block
                        .samples
                        .truncate(keep_frames as usize * usize::from(self.format.channels));
                    self.output_frames += keep_frames;
                    return Ok(Some(block));
                }
                Err(MusicError::Invalid(
                    "CAF ALAC packets ended before valid frame count",
                ))
            }
        }
    }
}

fn decode_integer_sample(bytes: &[u8], spec: &PcmSpec) -> Result<i32> {
    let sample = if spec.sample_bytes == 3 && spec.stride == 4 {
        match spec.endian {
            PcmEndian::Big => &bytes[..3],
            PcmEndian::Little => &bytes[1..],
        }
    } else {
        &bytes[..spec.sample_bytes]
    };
    let mut raw = 0u32;
    match spec.endian {
        PcmEndian::Big => {
            for byte in sample {
                raw = (raw << 8) | u32::from(*byte);
            }
        }
        PcmEndian::Little => {
            for (index, byte) in sample.iter().enumerate() {
                raw |= u32::from(*byte) << (index * 8);
            }
        }
    }
    let bits = spec.format.bits_per_sample;
    let sign = 1u32 << (bits - 1);
    let signed = if bits == 32 {
        raw as i32
    } else if raw & sign != 0 {
        (raw | (!0u32 << bits)) as i32
    } else {
        raw as i32
    };
    Ok(signed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn caf_chunk(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut out = kind.to_vec();
        out.extend_from_slice(&(payload.len() as i64).to_be_bytes());
        out.extend_from_slice(payload);
        out
    }

    fn desc(
        rate: f64,
        codec: &[u8; 4],
        flags: u32,
        bytes: u32,
        frames: u32,
        channels: u32,
        bits: u32,
    ) -> Vec<u8> {
        let mut out = rate.to_bits().to_be_bytes().to_vec();
        out.extend_from_slice(codec);
        out.extend_from_slice(&flags.to_be_bytes());
        out.extend_from_slice(&bytes.to_be_bytes());
        out.extend_from_slice(&frames.to_be_bytes());
        out.extend_from_slice(&channels.to_be_bytes());
        out.extend_from_slice(&bits.to_be_bytes());
        out
    }

    fn file(chunks: &[Vec<u8>]) -> Vec<u8> {
        let mut out = b"caff\x00\x01\x00\x00".to_vec();
        for chunk in chunks {
            out.extend_from_slice(chunk);
        }
        out
    }

    fn pcm_file(description: Vec<u8>, samples: &[u8]) -> Vec<u8> {
        let mut data = vec![0, 0, 0, 0];
        data.extend_from_slice(samples);
        file(&[caf_chunk(b"desc", &description), caf_chunk(b"data", &data)])
    }

    #[test]
    fn decodes_packed_24_bit_big_endian_lpcm_exactly() {
        let desc = desc(48_000.0, b"lpcm", 0, 6, 1, 2, 24);
        let bytes = pcm_file(
            desc,
            &[0x80, 0, 0, 0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0, 0, 42],
        );
        let mut decoder = CafDecoder::new(Cursor::new(bytes)).unwrap();
        assert_eq!(decoder.format(), AudioFormat::new(48_000, 2, 24).unwrap());
        assert_eq!(decoder.total_frames(), Some(2));
        assert_eq!(
            decoder.next_block().unwrap().unwrap().samples,
            [i32::MIN >> 8, 8_388_607, -1, 42]
        );
        assert!(decoder.next_block().unwrap().is_none());
    }

    #[test]
    fn accepts_standard_stereo_layout_and_rejects_mismatched_layout() {
        let description = desc(44_100.0, b"lpcm", 0, 4, 1, 2, 16);
        let layout = [0x00, 0x65, 0x00, 0x02, 0, 0, 0, 0, 0, 0, 0, 0];
        let data = [0, 0, 0, 0, 0, 1, 0, 2];
        let bytes = file(&[
            caf_chunk(b"desc", &description),
            caf_chunk(b"chan", &layout),
            caf_chunk(b"data", &data),
        ]);
        assert!(CafDecoder::new(Cursor::new(bytes)).is_ok());
        let mismatched = [0x00, 0x64, 0x00, 0x01, 0, 0, 0, 0, 0, 0, 0, 0];
        let bytes = file(&[
            caf_chunk(b"desc", &description),
            caf_chunk(b"chan", &mismatched),
            caf_chunk(b"data", &data),
        ]);
        assert!(matches!(
            CafDecoder::new(Cursor::new(bytes)),
            Err(MusicError::Unsupported(_))
        ));
    }

    #[test]
    fn accepts_unknown_sized_data_only_as_final_audio_payload() {
        let description = desc(44_100.0, b"lpcm", 0, 2, 1, 1, 16);
        let mut bytes = b"caff\x00\x01\x00\x00".to_vec();
        bytes.extend_from_slice(&caf_chunk(b"desc", &description));
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&(-1i64).to_be_bytes());
        bytes.extend_from_slice(&[0, 0, 0, 0, 0x80, 0, 0x7f, 0xff]);
        let mut decoder = CafDecoder::new(Cursor::new(bytes)).unwrap();
        assert_eq!(decoder.total_frames(), Some(2));
        assert_eq!(
            decoder.next_block().unwrap().unwrap().samples,
            [i16::MIN as i32, i16::MAX as i32]
        );
        assert!(decoder.next_block().unwrap().is_none());
    }

    #[test]
    fn decodes_unpacked_little_endian_24_bit_and_odd_unknown_chunk() {
        let desc = desc(44_100.0, b"lpcm", FLAG_LPCM_LITTLE_ENDIAN, 8, 1, 2, 24);
        let data = vec![0, 0, 0, 0, 0, 0, 0, 0x80, 0, 0xff, 0xff, 0x7f];
        let unknown = caf_chunk(b"info", &[9, 8, 7]);
        let bytes = file(&[
            caf_chunk(b"desc", &desc),
            unknown,
            caf_chunk(b"data", &data),
        ]);
        let mut decoder = CafDecoder::new(Cursor::new(bytes)).unwrap();
        assert_eq!(
            decoder.next_block().unwrap().unwrap().samples,
            [i32::MIN >> 8, 8_388_607]
        );
        assert!(decoder.next_block().unwrap().is_none());
    }

    #[test]
    fn decodes_signed_8_16_and_32_bit_lpcm_in_network_and_little_endian_order() {
        let cases: [(Vec<u8>, Vec<i32>); 3] = [
            (
                pcm_file(
                    desc(44_100.0, b"lpcm", 0, 1, 1, 1, 8),
                    &[0x80, 0xff, 0, 0x7f],
                ),
                vec![-128, -1, 0, 127],
            ),
            (
                pcm_file(
                    desc(44_100.0, b"lpcm", FLAG_LPCM_LITTLE_ENDIAN, 2, 1, 1, 16),
                    &[0, 0x80, 0xff, 0x7f],
                ),
                vec![-32768, 32767],
            ),
            (
                pcm_file(
                    desc(96_000.0, b"lpcm", 0, 4, 1, 1, 32),
                    &[0x80, 0, 0, 0, 0x7f, 0xff, 0xff, 0xff],
                ),
                vec![i32::MIN, i32::MAX],
            ),
        ];
        for (bytes, expected) in cases {
            let mut decoder = CafDecoder::new(Cursor::new(bytes)).unwrap();
            let mut actual = Vec::new();
            while let Some(block) = decoder.next_block().unwrap() {
                actual.extend(block.samples);
            }
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn decodes_generated_alac_packet_with_packet_table_and_exact_pcm() {
        let cookie = [
            0, 0, 0x10, 0, 0, 24, 40, 10, 14, 2, 0, 0, 0, 0, 0x60, 4, 0, 0x20, 0x4c, 0xc0, 0, 0,
            0xac, 0x44,
        ];
        let packet = include_bytes!("../../rust-music-alac/tests/fixtures/alac24_packet.bin");
        let description = desc(44_100.0, b"alac", 0, 0, 4096, 2, 0);
        let mut audio = vec![0, 0, 0, 0];
        audio.extend_from_slice(packet);
        let mut table = Vec::new();
        table.extend_from_slice(&1i64.to_be_bytes());
        table.extend_from_slice(&64i64.to_be_bytes());
        table.extend_from_slice(&0i32.to_be_bytes());
        table.extend_from_slice(&4032i32.to_be_bytes());
        let len = packet.len() as u32;
        let mut n = len;
        let mut encoded = Vec::new();
        loop {
            encoded.push((n & 0x7f) as u8);
            n >>= 7;
            if n == 0 {
                break;
            }
        }
        encoded.reverse();
        let continuation_bytes = encoded.len().saturating_sub(1);
        for byte in encoded.iter_mut().take(continuation_bytes) {
            *byte |= 0x80;
        }
        table.extend_from_slice(&encoded);
        let bytes = file(&[
            caf_chunk(b"desc", &description),
            caf_chunk(b"kuki", &cookie),
            caf_chunk(b"pakt", &table),
            caf_chunk(b"data", &audio),
        ]);
        let mut decoder = CafDecoder::new(Cursor::new(bytes)).unwrap();
        let mut actual = Vec::new();
        while let Some(block) = decoder.next_block().unwrap() {
            actual.extend(block.samples);
        }
        let expected: Vec<i32> = (0..64)
            .flat_map(|frame| [-7_000_000 + frame * 17_111, 4_000_000 - frame * 9_999])
            .collect();
        assert_eq!(actual, expected);
        assert_eq!(decoder.total_frames(), Some(64));
    }

    #[test]
    fn rejects_lossy_codec_and_bad_description_or_data_bounds() {
        let mut lossy = vec![0, 0, 0, 0];
        lossy.extend_from_slice(&[1, 2]);
        let aac = file(&[
            caf_chunk(b"desc", &desc(44_100.0, b"aac ", 2, 0, 1024, 2, 0)),
            caf_chunk(b"data", &lossy),
        ]);
        assert!(matches!(
            CafDecoder::new(Cursor::new(aac)),
            Err(MusicError::Unsupported(_))
        ));
        let bad_rate = pcm_file(desc(44_100.5, b"lpcm", 0, 2, 1, 1, 16), &[0, 0]);
        assert!(CafDecoder::new(Cursor::new(bad_rate)).is_err());
        let truncated = file(&[
            caf_chunk(b"desc", &desc(44_100.0, b"lpcm", 0, 2, 1, 1, 16)),
            caf_chunk(b"data", &[0, 0, 0, 0, 1]),
        ]);
        assert!(CafDecoder::new(Cursor::new(truncated)).is_err());
        let float = pcm_file(
            desc(44_100.0, b"lpcm", FLAG_LPCM_FLOAT, 4, 1, 1, 32),
            &[0, 0, 0, 0],
        );
        assert!(matches!(
            CafDecoder::new(Cursor::new(float)),
            Err(MusicError::Unsupported(_))
        ));
        let layout = file(&[
            caf_chunk(b"desc", &desc(44_100.0, b"lpcm", 0, 4, 1, 2, 16)),
            caf_chunk(b"chan", &[0; 12]),
            caf_chunk(b"data", &[0, 0, 0, 0, 0, 0, 0, 0]),
        ]);
        assert!(matches!(
            CafDecoder::new(Cursor::new(layout)),
            Err(MusicError::Unsupported(_))
        ));
        let multichannel = pcm_file(desc(48_000.0, b"lpcm", 0, 6, 1, 3, 16), &[0; 6]);
        assert!(matches!(
            CafDecoder::new(Cursor::new(multichannel)),
            Err(MusicError::Unsupported(_))
        ));
        let negative_chunk = file(&[
            caf_chunk(b"desc", &desc(44_100.0, b"lpcm", 0, 2, 1, 1, 16)),
            {
                let mut chunk = b"info".to_vec();
                chunk.extend_from_slice(&(-2i64).to_be_bytes());
                chunk
            },
        ]);
        assert!(CafDecoder::new(Cursor::new(negative_chunk)).is_err());
    }

    #[test]
    fn every_prefix_truncation_fails_closed() {
        let bytes = pcm_file(desc(44_100.0, b"lpcm", 0, 2, 1, 1, 16), &[0, 1, 0, 2]);
        for end in 0..bytes.len() {
            assert!(
                CafDecoder::new(Cursor::new(&bytes[..end])).is_err(),
                "prefix {end}"
            );
        }
    }

    #[test]
    fn caf_variable_integer_is_bounded_and_canonical() {
        assert_eq!(read_varint(&[0x81, 0x00], &mut 0).unwrap(), 128);
        assert!(read_varint(&[0x80, 0x00], &mut 0).is_err());
        assert!(read_varint(&[0x90, 0x80, 0x80, 0x80, 0], &mut 0).is_err());
    }
}
