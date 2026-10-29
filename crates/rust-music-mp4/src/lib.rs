//! Strict, bounded MP4/M4A sample demux for ALAC only.
//!
//! This module accepts one self-contained audio track with one ALAC sample
//! description. AAC, encrypted and fragmented MP4, external data references,
//! edits that trim/shift samples, and unsupported sample-table variants fail
//! closed. Samples are read and decoded one packet at a time.
#![forbid(unsafe_code)]

use rust_music_alac::{decode_packet, AlacConfig};
use rust_music_core::{AudioFormat, Decoder, MusicError, PcmBlock, Result};
use std::io::{Read, Seek, SeekFrom};

const MAX_FTYP_BYTES: u64 = 1024 * 1024;
const MAX_MOOV_BYTES: u64 = 32 * 1024 * 1024;
const MAX_BOXES: usize = 4096;
const MAX_MDAT_BOXES: usize = 256;
const MAX_SAMPLES: usize = 1_000_000;
const MAX_TABLE_ENTRIES: usize = 65_536;
const MAX_PACKET_BYTES: u32 = 8 * 1024 * 1024;

#[derive(Clone, Copy)]
struct BoxRef<'a> {
    kind: [u8; 4],
    payload: &'a [u8],
}

#[derive(Clone, Copy)]
struct BoxHeader {
    kind: [u8; 4],
    payload_start: u64,
    end: u64,
}

#[derive(Clone, Copy, Debug)]
struct MdatRange {
    start: u64,
    end: u64,
}

#[derive(Clone, Copy, Debug)]
struct Sample {
    offset: u64,
    size: u32,
    duration: u32,
}

struct SampleTables {
    config: AlacConfig,
    samples: Vec<Sample>,
    total_frames: u64,
}

/// A streaming decoder for a deliberately narrow ISO BMFF audio profile.
/// The movie metadata and sample index are bounded; packet payloads are read
/// only when requested and decoded one at a time.
pub struct Mp4AlacDecoder<R: Read + Seek + Send> {
    input: R,
    config: AlacConfig,
    samples: Vec<Sample>,
    total_frames: u64,
    next_sample: usize,
}

impl<R: Read + Seek + Send> Mp4AlacDecoder<R> {
    pub fn new(mut input: R) -> Result<Self> {
        let file_len = input.seek(SeekFrom::End(0))?;
        if file_len < 24 {
            return Err(MusicError::Invalid("MP4 file is too small"));
        }
        input.seek(SeekFrom::Start(0))?;

        let mut offset = 0u64;
        let mut top_level_count = 0usize;
        let mut saw_ftyp = false;
        let mut moov_payload = None;
        let mut mdats = Vec::new();
        while offset < file_len {
            if top_level_count >= MAX_BOXES {
                return Err(MusicError::Limit("MP4 top-level box count exceeds bound"));
            }
            input.seek(SeekFrom::Start(offset))?;
            let header = read_box_header(&mut input, offset, file_len, true)?;
            if top_level_count == 0 && header.kind != *b"ftyp" {
                return Err(MusicError::Invalid("MP4 must begin with ftyp"));
            }
            top_level_count += 1;
            match &header.kind {
                b"ftyp" => {
                    if saw_ftyp || offset != 0 {
                        return Err(MusicError::Invalid("duplicate or misplaced MP4 ftyp"));
                    }
                    let length = header.end - header.payload_start;
                    if !(8..=MAX_FTYP_BYTES).contains(&length) {
                        return Err(MusicError::Limit(
                            "MP4 ftyp size is outside configured bounds",
                        ));
                    }
                    let mut payload = vec![0; length as usize];
                    input.read_exact(&mut payload)?;
                    validate_ftyp(&payload)?;
                    saw_ftyp = true;
                }
                b"moov" => {
                    if moov_payload.is_some() {
                        return Err(MusicError::Invalid("duplicate MP4 moov"));
                    }
                    let length = header.end - header.payload_start;
                    if length > MAX_MOOV_BYTES {
                        return Err(MusicError::Limit("MP4 moov exceeds configured bound"));
                    }
                    let mut payload = vec![0; length as usize];
                    input.read_exact(&mut payload)?;
                    moov_payload = Some(payload);
                }
                b"mdat" => {
                    if mdats.len() >= MAX_MDAT_BOXES {
                        return Err(MusicError::Limit("MP4 mdat box count exceeds bound"));
                    }
                    mdats.push(MdatRange {
                        start: header.payload_start,
                        end: header.end,
                    });
                }
                b"moof" => {
                    return Err(MusicError::Unsupported("fragmented MP4 is unsupported"));
                }
                _ => {}
            }
            offset = header.end;
        }
        if !saw_ftyp || mdats.is_empty() {
            return Err(MusicError::Invalid("MP4 ftyp or mdat is missing"));
        }
        let moov = moov_payload.ok_or(MusicError::Invalid("MP4 moov is missing"))?;
        let tables = parse_movie(&moov, &mdats)?;
        Ok(Self {
            input,
            config: tables.config,
            samples: tables.samples,
            total_frames: tables.total_frames,
            next_sample: 0,
        })
    }
}

impl<R: Read + Seek + Send> Decoder for Mp4AlacDecoder<R> {
    fn format(&self) -> AudioFormat {
        self.config.format
    }

    fn total_frames(&self) -> Option<u64> {
        Some(self.total_frames)
    }

    fn next_block(&mut self) -> Result<Option<PcmBlock>> {
        let Some(sample) = self.samples.get(self.next_sample).copied() else {
            return Ok(None);
        };
        let mut packet = vec![0; sample.size as usize];
        self.input.seek(SeekFrom::Start(sample.offset))?;
        self.input.read_exact(&mut packet)?;
        let block = decode_packet(self.config, &packet)?;
        if block.frames() != sample.duration as usize {
            return Err(MusicError::Invalid(
                "ALAC decoded frame count disagrees with MP4 sample timing",
            ));
        }
        self.next_sample += 1;
        Ok(Some(block))
    }
}

fn read_box_header<R: Read + Seek>(
    input: &mut R,
    start: u64,
    file_len: u64,
    top_level: bool,
) -> Result<BoxHeader> {
    let mut base = [0u8; 8];
    input.read_exact(&mut base)?;
    let size32 = u32::from_be_bytes(base[..4].try_into().unwrap());
    let kind: [u8; 4] = base[4..8].try_into().unwrap();
    let mut header_len = 8u64;
    let size = match size32 {
        0 if top_level && kind == *b"mdat" => file_len
            .checked_sub(start)
            .ok_or(MusicError::Invalid("invalid MP4 size-zero mdat"))?,
        0 => return Err(MusicError::Invalid("MP4 nested boxes cannot use size zero")),
        1 => {
            let mut extended = [0u8; 8];
            input.read_exact(&mut extended)?;
            header_len = 16;
            u64::from_be_bytes(extended)
        }
        value => u64::from(value),
    };
    if kind == *b"uuid" {
        let mut user_type = [0u8; 16];
        input.read_exact(&mut user_type)?;
        header_len = header_len
            .checked_add(16)
            .ok_or(MusicError::Limit("MP4 box header overflow"))?;
    }
    if size < header_len {
        return Err(MusicError::Invalid("MP4 box is smaller than its header"));
    }
    let end = start
        .checked_add(size)
        .ok_or(MusicError::Limit("MP4 box end overflows"))?;
    if end > file_len || end <= start {
        return Err(MusicError::Invalid("MP4 box exceeds its parent or file"));
    }
    Ok(BoxHeader {
        kind,
        payload_start: start + header_len,
        end,
    })
}

fn parse_boxes(data: &[u8]) -> Result<Vec<BoxRef<'_>>> {
    let mut boxes = Vec::new();
    let mut offset = 0usize;
    while offset < data.len() {
        if boxes.len() >= MAX_BOXES || data.len() - offset < 8 {
            return Err(MusicError::Invalid(
                "truncated or excessive MP4 child boxes",
            ));
        }
        let size32 = u32::from_be_bytes(data[offset..offset + 4].try_into().unwrap());
        let kind: [u8; 4] = data[offset + 4..offset + 8].try_into().unwrap();
        let (size, mut header_len) = match size32 {
            0 => return Err(MusicError::Invalid("MP4 nested boxes cannot use size zero")),
            1 => {
                if data.len() - offset < 16 {
                    return Err(MusicError::Invalid("truncated extended MP4 box"));
                }
                (
                    usize::try_from(u64::from_be_bytes(
                        data[offset + 8..offset + 16].try_into().unwrap(),
                    ))
                    .map_err(|_| MusicError::Limit("MP4 box is too large"))?,
                    16usize,
                )
            }
            value => (value as usize, 8usize),
        };
        if kind == *b"uuid" {
            header_len = header_len
                .checked_add(16)
                .ok_or(MusicError::Limit("MP4 UUID header overflow"))?;
        }
        if size < header_len || size > data.len() - offset {
            return Err(MusicError::Invalid("MP4 child box exceeds parent bounds"));
        }
        let payload_start = offset + header_len;
        let end = offset + size;
        boxes.push(BoxRef {
            kind,
            payload: &data[payload_start..end],
        });
        offset = end;
    }
    Ok(boxes)
}

fn one<'a>(boxes: &'a [BoxRef<'a>], kind: &[u8; 4]) -> Result<&'a BoxRef<'a>> {
    let mut matches = boxes.iter().filter(|item| &item.kind == kind);
    let found = matches
        .next()
        .ok_or(MusicError::Invalid("required MP4 box is missing"))?;
    if matches.next().is_some() {
        return Err(MusicError::Invalid("duplicate MP4 structural box"));
    }
    Ok(found)
}

fn optional_one<'a>(boxes: &'a [BoxRef<'a>], kind: &[u8; 4]) -> Result<Option<&'a BoxRef<'a>>> {
    let mut matches = boxes.iter().filter(|item| &item.kind == kind);
    let found = matches.next();
    if matches.next().is_some() {
        return Err(MusicError::Invalid("duplicate MP4 structural box"));
    }
    Ok(found)
}

fn full_box(payload: &[u8], version: u8) -> Result<()> {
    if payload.len() < 4 || payload[0] != version || payload[1..4] != [0, 0, 0] {
        return Err(MusicError::Unsupported(
            "unsupported MP4 full-box version or flags",
        ));
    }
    Ok(())
}

fn be_u32(bytes: &[u8]) -> u32 {
    u32::from_be_bytes(bytes.try_into().unwrap())
}

fn be_u64(bytes: &[u8]) -> u64 {
    u64::from_be_bytes(bytes.try_into().unwrap())
}

fn validate_ftyp(payload: &[u8]) -> Result<()> {
    if payload.len() < 8 || (payload.len() - 8) % 4 != 0 {
        return Err(MusicError::Invalid("malformed MP4 ftyp"));
    }
    const BRANDS: [[u8; 4]; 6] = [*b"M4A ", *b"M4B ", *b"isom", *b"iso2", *b"mp41", *b"mp42"];
    let major: [u8; 4] = payload[..4].try_into().unwrap();
    let compatible: Vec<[u8; 4]> = payload[8..]
        .chunks_exact(4)
        .map(|brand| <[u8; 4]>::try_from(brand).unwrap())
        .collect();
    if !BRANDS.contains(&major) && !compatible.iter().any(|brand| BRANDS.contains(brand)) {
        return Err(MusicError::Unsupported("unsupported MP4 file brand"));
    }
    if major == *b"M4P " || compatible.contains(b"M4P ") {
        return Err(MusicError::Unsupported("DRM-protected M4P is unsupported"));
    }
    Ok(())
}

fn parse_movie(moov: &[u8], mdats: &[MdatRange]) -> Result<SampleTables> {
    let boxes = parse_boxes(moov)?;
    for forbidden in [b"mvex", b"pssh"] {
        if boxes.iter().any(|item| item.kind == forbidden[..]) {
            return Err(MusicError::Unsupported(
                "fragmented or protected MP4 is unsupported",
            ));
        }
    }
    let movie_timescale = parse_mvhd(one(&boxes, b"mvhd")?.payload)?;
    let tracks: Vec<_> = boxes.iter().filter(|item| item.kind == *b"trak").collect();
    if tracks.len() != 1 {
        return Err(MusicError::Unsupported(
            "exactly one MP4 track is supported",
        ));
    }
    let track_boxes = parse_boxes(tracks[0].payload)?;
    parse_tkhd(one(&track_boxes, b"tkhd")?.payload)?;
    let mdia = parse_boxes(one(&track_boxes, b"mdia")?.payload)?;
    let (media_timescale, declared_media_duration) = parse_mdhd(one(&mdia, b"mdhd")?.payload)?;
    let handler = one(&mdia, b"hdlr")?.payload;
    full_box(handler, 0)?;
    if handler.len() < 24 || &handler[8..12] != b"soun" {
        return Err(MusicError::Unsupported("MP4 track is not an audio track"));
    }
    let minf = parse_boxes(one(&mdia, b"minf")?.payload)?;
    validate_local_data_reference(
        one(&parse_boxes(one(&minf, b"dinf")?.payload)?, b"dref")?.payload,
    )?;
    let stbl = parse_boxes(one(&minf, b"stbl")?.payload)?;
    for forbidden in [b"senc", b"saiz", b"saio", b"sgpd", b"sbgp"] {
        if stbl.iter().any(|item| item.kind == forbidden[..]) {
            return Err(MusicError::Unsupported(
                "protected or grouped MP4 samples are unsupported",
            ));
        }
    }
    if stbl.iter().any(|item| item.kind == *b"stz2") {
        return Err(MusicError::Unsupported(
            "compact MP4 sample sizes are unsupported",
        ));
    }
    let config = parse_stsd(one(&stbl, b"stsd")?.payload)?;
    if let Some(edts) = optional_one(&track_boxes, b"edts")? {
        let edits = parse_boxes(edts.payload)?;
        validate_noop_edit(one(&edits, b"elst")?.payload, movie_timescale)?;
    }
    if media_timescale != config.format.sample_rate {
        return Err(MusicError::Unsupported(
            "ALAC media timescale must equal its sample rate",
        ));
    }
    let sizes = parse_stsz(one(&stbl, b"stsz")?.payload)?;
    if config.max_frame_bytes != 0 && sizes.iter().any(|&size| size > config.max_frame_bytes) {
        return Err(MusicError::Limit(
            "MP4 ALAC packet exceeds cookie frame-size bound",
        ));
    }
    let durations = parse_stts(one(&stbl, b"stts")?.payload, sizes.len())?;
    let chunks = parse_chunk_offsets(&stbl)?;
    let samples_per_chunk = parse_stsc(one(&stbl, b"stsc")?.payload, chunks.len())?;
    let samples = expand_samples(&sizes, &durations, &chunks, &samples_per_chunk, mdats)?;
    let total_frames = durations.iter().try_fold(0u64, |total, &duration| {
        total
            .checked_add(u64::from(duration))
            .ok_or(MusicError::Limit("MP4 decoded duration overflows"))
    })?;
    if total_frames != declared_media_duration {
        return Err(MusicError::Invalid(
            "MP4 media duration disagrees with sample table",
        ));
    }
    if let Some(edts) = optional_one(&track_boxes, b"edts")? {
        let edits = parse_boxes(edts.payload)?;
        validate_edit_duration(
            one(&edits, b"elst")?.payload,
            movie_timescale,
            media_timescale,
            total_frames,
        )?;
    }
    Ok(SampleTables {
        config,
        samples,
        total_frames,
    })
}

fn parse_mvhd(payload: &[u8]) -> Result<u32> {
    let version = payload
        .first()
        .copied()
        .ok_or(MusicError::Invalid("empty MP4 mvhd"))?;
    full_box(payload, version)?;
    let (offset, expected) = match version {
        0 => (12, 100),
        1 => (20, 112),
        _ => return Err(MusicError::Unsupported("unsupported MP4 mvhd version")),
    };
    if payload.len() != expected {
        return Err(MusicError::Invalid("invalid MP4 mvhd size"));
    }
    let timescale = be_u32(&payload[offset..offset + 4]);
    if timescale == 0 {
        return Err(MusicError::Invalid("zero MP4 movie timescale"));
    }
    Ok(timescale)
}

fn parse_mdhd(payload: &[u8]) -> Result<(u32, u64)> {
    let version = payload
        .first()
        .copied()
        .ok_or(MusicError::Invalid("empty MP4 mdhd"))?;
    full_box(payload, version)?;
    let (offset, expected, duration_bytes) = match version {
        0 => (12, 24, 4),
        1 => (20, 36, 8),
        _ => return Err(MusicError::Unsupported("unsupported MP4 mdhd version")),
    };
    if payload.len() != expected {
        return Err(MusicError::Invalid("invalid MP4 mdhd size"));
    }
    let timescale = be_u32(&payload[offset..offset + 4]);
    let duration = if duration_bytes == 4 {
        u64::from(be_u32(&payload[offset + 4..offset + 8]))
    } else {
        be_u64(&payload[offset + 4..offset + 12])
    };
    if timescale == 0 || duration == u64::MAX {
        return Err(MusicError::Invalid("invalid or unknown MP4 media duration"));
    }
    Ok((timescale, duration))
}

fn parse_tkhd(payload: &[u8]) -> Result<()> {
    let version = payload
        .first()
        .copied()
        .ok_or(MusicError::Invalid("empty MP4 tkhd"))?;
    if payload.len() < 4 {
        return Err(MusicError::Invalid("truncated MP4 tkhd"));
    }
    let (expected, track_id_offset) = match version {
        0 => (84, 12),
        1 => (96, 20),
        _ => return Err(MusicError::Unsupported("unsupported MP4 tkhd version")),
    };
    let flags = u32::from_be_bytes([0, payload[1], payload[2], payload[3]]);
    if payload.len() != expected
        || payload[1..3] != [0, 0]
        || payload[3] & !0x07 != 0
        || flags & 1 == 0
        || be_u32(&payload[track_id_offset..track_id_offset + 4]) == 0
    {
        return Err(MusicError::Invalid("invalid MP4 track header"));
    }
    Ok(())
}

fn validate_noop_edit(payload: &[u8], _movie_timescale: u32) -> Result<()> {
    let version = payload
        .first()
        .copied()
        .ok_or(MusicError::Invalid("empty MP4 elst"))?;
    full_box(payload, version)?;
    if !matches!(version, 0 | 1) || payload.len() < 8 || be_u32(&payload[4..8]) != 1 {
        return Err(MusicError::Unsupported(
            "only one no-op MP4 edit is supported",
        ));
    }
    let expected = if version == 0 { 20 } else { 28 };
    if payload.len() != expected {
        return Err(MusicError::Invalid("invalid MP4 edit list size"));
    }
    let media_time_offset = if version == 0 { 12 } else { 16 };
    let rate_offset = media_time_offset + 4;
    if be_u32(&payload[media_time_offset..rate_offset]) != 0
        || payload[rate_offset..rate_offset + 4] != [0, 1, 0, 0]
    {
        return Err(MusicError::Unsupported(
            "trimmed or shifted MP4 edits are unsupported",
        ));
    }
    Ok(())
}

fn validate_edit_duration(
    payload: &[u8],
    movie_timescale: u32,
    media_timescale: u32,
    media_duration: u64,
) -> Result<()> {
    let version = payload[0];
    let segment_duration = if version == 0 {
        u64::from(be_u32(&payload[8..12]))
    } else {
        be_u64(&payload[8..16])
    };
    if u128::from(segment_duration) * u128::from(media_timescale)
        != u128::from(media_duration) * u128::from(movie_timescale)
    {
        return Err(MusicError::Invalid(
            "MP4 edit duration disagrees with media",
        ));
    }
    Ok(())
}

fn validate_local_data_reference(payload: &[u8]) -> Result<()> {
    full_box(payload, 0)?;
    if payload.len() < 8 || be_u32(&payload[4..8]) != 1 {
        return Err(MusicError::Unsupported(
            "exactly one MP4 data reference is supported",
        ));
    }
    let entries = parse_boxes(&payload[8..])?;
    if entries.len() != 1 || entries[0].kind != *b"url " {
        return Err(MusicError::Unsupported(
            "external MP4 data references are unsupported",
        ));
    }
    let url = entries[0].payload;
    if url.len() != 4 || url[0] != 0 || url[1..4] != [0, 0, 1] {
        return Err(MusicError::Unsupported(
            "external MP4 data references are unsupported",
        ));
    }
    Ok(())
}

fn parse_stsd(payload: &[u8]) -> Result<AlacConfig> {
    full_box(payload, 0)?;
    if payload.len() < 8 || be_u32(&payload[4..8]) != 1 {
        return Err(MusicError::Unsupported(
            "exactly one MP4 sample description is supported",
        ));
    }
    let entries = parse_boxes(&payload[8..])?;
    if entries.len() != 1 || entries[0].kind != *b"alac" {
        return Err(MusicError::Unsupported("MP4 sample codec must be ALAC"));
    }
    let sample = entries[0].payload;
    if sample.len() < 28 || sample[0..6] != [0; 6] {
        return Err(MusicError::Invalid("invalid ALAC sound sample entry"));
    }
    let data_reference = u16::from_be_bytes(sample[6..8].try_into().unwrap());
    let version = u16::from_be_bytes(sample[8..10].try_into().unwrap());
    let channels = u16::from_be_bytes(sample[16..18].try_into().unwrap());
    let sample_size = u16::from_be_bytes(sample[18..20].try_into().unwrap());
    let sample_rate = be_u32(&sample[24..28]);
    if data_reference != 1
        || version != 0
        || sample[20..24] != [0, 0, 0, 0]
        || sample_rate & 0xffff != 0
    {
        return Err(MusicError::Unsupported(
            "unsupported ALAC sound entry version or reference",
        ));
    }
    let child_boxes = parse_boxes(&sample[28..])?;
    if child_boxes.len() != 1 || child_boxes[0].kind != *b"alac" {
        return Err(MusicError::Unsupported(
            "ALAC entry extensions are unsupported",
        ));
    }
    let cookie = child_boxes[0].payload;
    if cookie.len() != 28 || cookie[..4] != [0; 4] {
        return Err(MusicError::Invalid(
            "invalid ALAC sample-description cookie",
        ));
    }
    let config = AlacConfig::parse(&cookie[4..])?;
    if channels != u16::from(config.channels)
        || sample_size != u16::from(config.bit_depth)
        || sample_rate >> 16 != config.format.sample_rate
    {
        return Err(MusicError::Invalid(
            "ALAC sample entry disagrees with its cookie",
        ));
    }
    Ok(config)
}

fn parse_stsz(payload: &[u8]) -> Result<Vec<u32>> {
    full_box(payload, 0)?;
    if payload.len() < 12 {
        return Err(MusicError::Invalid("truncated MP4 stsz"));
    }
    let fixed_size = be_u32(&payload[4..8]);
    let count = usize::try_from(be_u32(&payload[8..12]))
        .map_err(|_| MusicError::Limit("MP4 sample count exceeds bound"))?;
    if count == 0 || count > MAX_SAMPLES {
        return Err(MusicError::Limit(
            "MP4 sample count is outside configured bounds",
        ));
    }
    if fixed_size != 0 {
        if payload.len() != 12 || fixed_size > MAX_PACKET_BYTES {
            return Err(MusicError::Invalid("invalid fixed MP4 sample size"));
        }
        return Ok(vec![fixed_size; count]);
    }
    let size_bytes = count
        .checked_mul(4)
        .ok_or(MusicError::Limit("MP4 sample-size table overflows"))?;
    if payload.len() != 12 + size_bytes {
        return Err(MusicError::Invalid(
            "MP4 stsz length disagrees with sample count",
        ));
    }
    let mut sizes = Vec::with_capacity(count);
    for bytes in payload[12..].chunks_exact(4) {
        let size = be_u32(bytes);
        if size == 0 || size > MAX_PACKET_BYTES {
            return Err(MusicError::Limit(
                "MP4 ALAC packet size is outside configured bounds",
            ));
        }
        sizes.push(size);
    }
    Ok(sizes)
}

fn parse_stts(payload: &[u8], expected_samples: usize) -> Result<Vec<u32>> {
    full_box(payload, 0)?;
    if payload.len() < 8 {
        return Err(MusicError::Invalid("truncated MP4 stts"));
    }
    let count = usize::try_from(be_u32(&payload[4..8]))
        .map_err(|_| MusicError::Limit("MP4 stts entry count exceeds bound"))?;
    if count == 0 || count > MAX_TABLE_ENTRIES || payload.len() != 8 + count * 8 {
        return Err(MusicError::Invalid("invalid MP4 stts entry table"));
    }
    let mut durations = Vec::with_capacity(expected_samples);
    for row in payload[8..].chunks_exact(8) {
        let samples = usize::try_from(be_u32(&row[..4]))
            .map_err(|_| MusicError::Limit("MP4 stts sample count exceeds bound"))?;
        let duration = be_u32(&row[4..]);
        if samples == 0 || duration == 0 || duration > 65_535 {
            return Err(MusicError::Invalid("invalid MP4 ALAC sample duration"));
        }
        let end = durations
            .len()
            .checked_add(samples)
            .ok_or(MusicError::Limit("MP4 stts sample count overflows"))?;
        if end > expected_samples {
            return Err(MusicError::Invalid("MP4 stts exceeds sample table"));
        }
        durations.resize(end, duration);
    }
    if durations.len() != expected_samples {
        return Err(MusicError::Invalid("MP4 stts sample count mismatch"));
    }
    Ok(durations)
}

fn parse_chunk_offsets(stbl: &[BoxRef<'_>]) -> Result<Vec<u64>> {
    let stco = optional_one(stbl, b"stco")?;
    let co64 = optional_one(stbl, b"co64")?;
    if stco.is_some() == co64.is_some() {
        return Err(MusicError::Invalid(
            "MP4 requires exactly one chunk-offset table",
        ));
    }
    let (payload, wide) = if let Some(box_ref) = stco {
        (box_ref.payload, false)
    } else {
        (co64.unwrap().payload, true)
    };
    full_box(payload, 0)?;
    if payload.len() < 8 {
        return Err(MusicError::Invalid("truncated MP4 chunk-offset table"));
    }
    let count = usize::try_from(be_u32(&payload[4..8]))
        .map_err(|_| MusicError::Limit("MP4 chunk count exceeds bound"))?;
    let width = if wide { 8 } else { 4 };
    if count == 0 || count > MAX_SAMPLES || payload.len() != 8 + count * width {
        return Err(MusicError::Invalid("invalid MP4 chunk-offset table"));
    }
    let mut offsets = Vec::with_capacity(count);
    for row in payload[8..].chunks_exact(width) {
        offsets.push(if wide {
            be_u64(row)
        } else {
            u64::from(be_u32(row))
        });
    }
    Ok(offsets)
}

fn parse_stsc(payload: &[u8], chunk_count: usize) -> Result<Vec<(u32, u32)>> {
    full_box(payload, 0)?;
    if payload.len() < 8 {
        return Err(MusicError::Invalid("truncated MP4 stsc"));
    }
    let count = usize::try_from(be_u32(&payload[4..8]))
        .map_err(|_| MusicError::Limit("MP4 stsc entry count exceeds bound"))?;
    if count == 0 || count > MAX_TABLE_ENTRIES || payload.len() != 8 + count * 12 {
        return Err(MusicError::Invalid("invalid MP4 stsc table"));
    }
    let mut entries = Vec::with_capacity(count);
    let mut prior_first = 0;
    for row in payload[8..].chunks_exact(12) {
        let first_chunk = be_u32(&row[..4]);
        let samples_per_chunk = be_u32(&row[4..8]);
        let description = be_u32(&row[8..]);
        if first_chunk == 0
            || (entries.is_empty() && first_chunk != 1)
            || first_chunk <= prior_first
            || first_chunk as usize > chunk_count
            || samples_per_chunk == 0
            || description != 1
        {
            return Err(MusicError::Invalid("invalid MP4 sample-to-chunk entry"));
        }
        entries.push((first_chunk, samples_per_chunk));
        prior_first = first_chunk;
    }
    Ok(entries)
}

fn expand_samples(
    sizes: &[u32],
    durations: &[u32],
    chunks: &[u64],
    samples_per_chunk: &[(u32, u32)],
    mdats: &[MdatRange],
) -> Result<Vec<Sample>> {
    if sizes.len() != durations.len() || chunks.is_empty() {
        return Err(MusicError::Invalid("inconsistent MP4 sample tables"));
    }
    let mut samples = Vec::with_capacity(sizes.len());
    let mut row_index = 0usize;
    let mut sample_index = 0usize;
    let mut previous_end = 0u64;
    for (chunk_index, &chunk_offset) in chunks.iter().enumerate() {
        while row_index + 1 < samples_per_chunk.len()
            && samples_per_chunk[row_index + 1].0 as usize <= chunk_index + 1
        {
            row_index += 1;
        }
        let count = usize::try_from(samples_per_chunk[row_index].1)
            .map_err(|_| MusicError::Limit("MP4 samples per chunk exceed bound"))?;
        if sample_index
            .checked_add(count)
            .is_none_or(|end| end > sizes.len())
        {
            return Err(MusicError::Invalid(
                "MP4 chunks exceed declared sample count",
            ));
        }
        let mut offset = chunk_offset;
        for index in sample_index..sample_index + count {
            let size = sizes[index];
            let end = offset
                .checked_add(u64::from(size))
                .ok_or(MusicError::Limit("MP4 sample extent overflows"))?;
            if offset < previous_end
                || !mdats
                    .iter()
                    .any(|mdat| offset >= mdat.start && end <= mdat.end)
            {
                return Err(MusicError::Invalid(
                    "MP4 sample lies outside mdat or overlaps",
                ));
            }
            samples.push(Sample {
                offset,
                size,
                duration: durations[index],
            });
            previous_end = end;
            offset = end;
        }
        sample_index += count;
    }
    if sample_index != sizes.len() {
        return Err(MusicError::Invalid("MP4 chunks do not cover every sample"));
    }
    Ok(samples)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn decodes_generated_m4a_packet_to_exact_integer_pcm() {
        let file = include_bytes!("../tests/fixtures/generated_alac24.m4a");
        let mut decoder = Mp4AlacDecoder::new(Cursor::new(file.as_slice())).unwrap();
        assert_eq!(decoder.format(), AudioFormat::new(44_100, 2, 24).unwrap());
        assert_eq!(decoder.total_frames(), Some(64));
        let mut actual = Vec::new();
        while let Some(block) = decoder.next_block().unwrap() {
            assert_eq!(block.frames(), 64);
            actual.extend(block.samples);
        }
        let expected: Vec<i32> = (0..64)
            .flat_map(|frame| [-7_000_000 + frame * 17_111, 4_000_000 - frame * 9_999])
            .collect();
        assert_eq!(actual, expected);
        assert!(decoder.next_block().unwrap().is_none());
    }

    #[test]
    fn rejects_generated_aac_and_wrong_sample_entry() {
        let aac = include_bytes!("../tests/fixtures/generated_aac.m4a");
        assert!(matches!(
            Mp4AlacDecoder::new(Cursor::new(aac.as_slice())),
            Err(MusicError::Unsupported(_))
        ));

        let mut wrong_entry = include_bytes!("../tests/fixtures/generated_alac24.m4a").to_vec();
        let stsd_type = wrong_entry
            .windows(4)
            .position(|bytes| bytes == b"stsd")
            .unwrap();
        let entry_type = stsd_type + 16;
        assert_eq!(&wrong_entry[entry_type..entry_type + 4], b"alac");
        wrong_entry[entry_type..entry_type + 4].copy_from_slice(b"mp4a");
        assert!(matches!(
            Mp4AlacDecoder::new(Cursor::new(wrong_entry)),
            Err(MusicError::Unsupported(_))
        ));

        let mut encrypted_entry = include_bytes!("../tests/fixtures/generated_alac24.m4a").to_vec();
        let entry_type = encrypted_entry
            .windows(4)
            .position(|bytes| bytes == b"stsd")
            .unwrap()
            + 16;
        encrypted_entry[entry_type..entry_type + 4].copy_from_slice(b"enca");
        assert!(matches!(
            Mp4AlacDecoder::new(Cursor::new(encrypted_entry)),
            Err(MusicError::Unsupported(_))
        ));

        let mut m4p = include_bytes!("../tests/fixtures/generated_alac24.m4a").to_vec();
        m4p[8..12].copy_from_slice(b"M4P ");
        assert!(matches!(
            Mp4AlacDecoder::new(Cursor::new(m4p)),
            Err(MusicError::Unsupported(_))
        ));
    }

    #[test]
    fn rejects_external_mp4_data_reference() {
        let mut external = include_bytes!("../tests/fixtures/generated_alac24.m4a").to_vec();
        let url_type = external
            .windows(4)
            .position(|bytes| bytes == b"url ")
            .unwrap();
        external[url_type + 7] = 0;
        assert!(matches!(
            Mp4AlacDecoder::new(Cursor::new(external)),
            Err(MusicError::Unsupported(_))
        ));
    }

    #[test]
    fn rejects_sample_offsets_outside_mdat_and_truncated_file() {
        let mut bad_offset = include_bytes!("../tests/fixtures/generated_alac24.m4a").to_vec();
        let stco_type = bad_offset
            .windows(4)
            .position(|bytes| bytes == b"stco")
            .unwrap();
        bad_offset[stco_type + 12..stco_type + 16].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(Mp4AlacDecoder::new(Cursor::new(bad_offset)).is_err());

        let truncated = &include_bytes!("../tests/fixtures/generated_alac24.m4a")[..40];
        assert!(Mp4AlacDecoder::new(Cursor::new(truncated)).is_err());
    }

    #[test]
    fn rejects_every_truncated_prefix_of_generated_m4a() {
        let file = include_bytes!("../tests/fixtures/generated_alac24.m4a");
        for end in 0..file.len() {
            assert!(
                Mp4AlacDecoder::new(Cursor::new(&file[..end])).is_err(),
                "accepted MP4 prefix {end}"
            );
        }
    }
}
