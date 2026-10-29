//! Offline decoder acceptance harness. Native device output is a separate gate.
use rust_music::{open_file, AudioFormat, Decoder, MusicError, NullSink, Result};
use std::{env, fs::OpenOptions, io::{Seek, SeekFrom, Write}};

const PCM_GUID: [u8; 16] = [1, 0, 0, 0, 0, 0, 0x10, 0, 0x80, 0, 0, 0xaa, 0, 0x38, 0x9b, 0x71];

fn riff_overhead(format: AudioFormat) -> u32 {
    if format.channels > 2 { 60 } else { 36 }
}

fn write_header<W: Write>(output: &mut W, format: AudioFormat, data_size: u32) -> Result<()> {
    if !matches!(format.bits_per_sample, 8 | 16 | 24 | 32) {
        return Err(MusicError::Unsupported("WAV export supports 8/16/24/32-bit source"));
    }
    let bytes_per_frame = u32::from(format.channels) * u32::from(format.bits_per_sample / 8);
    // RIFF size includes the one-byte pad after odd-length PCM data. The data
    // chunk's length field itself remains the original unpadded byte count.
    let riff_size = data_size.checked_add(riff_overhead(format))
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
    output.write_all(&format.sample_rate.checked_mul(bytes_per_frame)
        .ok_or(MusicError::Limit("WAV byte-rate overflow"))?.to_le_bytes())?;
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
fn export_wav(mut decoder: Box<dyn Decoder>, dest: &str) -> Result<()> {
    let format = decoder.format();
    let mut file = OpenOptions::new().write(true).create_new(true).open(dest)?;
    write_header(&mut file, format, 0)?;
    let sample_bytes = usize::from(format.bits_per_sample / 8);
    let mut bytes_written = 0u64;
    while let Some(block) = decoder.next_block()? {
        if block.format != format { return Err(MusicError::Invalid("format changed during decode")); }
        let mut bytes = Vec::with_capacity(block.samples.len() * sample_bytes);
        for value in block.samples {
            match sample_bytes {
                1 => bytes.push((value + 128) as u8),
                2 => bytes.extend_from_slice(&(value as i16).to_le_bytes()),
                3 => bytes.extend_from_slice(&(value as i32).to_le_bytes()[..3]),
                4 => bytes.extend_from_slice(&value.to_le_bytes()),
                _ => return Err(MusicError::Unsupported("unsupported integer PCM width")),
            }
        }
        let total = bytes_written.checked_add(bytes.len() as u64)
            .ok_or(MusicError::Limit("WAV size counter overflow"))?;
        // The worst case needs an odd-data padding byte as well as the header.
        if total > u64::from(u32::MAX - riff_overhead(format) - 1) {
            return Err(MusicError::Limit("WAV RIFF size limit reached; use RF64 in future"));
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
fn inspect(mut decoder: Box<dyn Decoder>) -> Result<()> {
    let format = decoder.format();
    let mut frames = 0u64;
    let mut blocks = 0u64;
    while let Some(block) = decoder.next_block()? {
        blocks += 1;
        frames += block.frames() as u64;
    }
    println!("{} Hz | {} ch | {} bits | {frames} frames | {blocks} blocks | CRC verified when FLAC",
        format.sample_rate, format.channels, format.bits_per_sample);
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
    let device = device.ok_or(MusicError::Invalid("Linux playback requires explicit hw:CARD,DEVICE"))?;
    let mut player = Player::new(AlsaSink::new(device, device.starts_with("hw:"))?);
    player.load(input)?;
    player.play()?;
    while player.step()?.state == PlaybackState::Playing {}
    Ok(())
}
#[cfg(not(any(all(target_os = "macos", feature = "macos-output"),
              all(target_os = "linux", feature = "alsa-output"))))]
fn play(_input: Box<dyn Decoder>, _device: Option<&str>) -> Result<()> {
    Err(MusicError::Unsupported("native playback needs --features macos-output (macOS) or alsa-output (Linux)"))
}

fn run() -> Result<()> {
    let mut args = env::args();
    let _binary = args.next();
    let action = args.next().ok_or(MusicError::Invalid("usage: rust-music-cli scan DIRECTORY | preview DIRECTORY OUTPUT.pbm | inspect FILE | decode FILE OUTPUT.wav | play FILE [ALSA_DEVICE]"))?;
    let path = args.next().ok_or(MusicError::Invalid("missing input filename"))?;
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
        _ => Err(MusicError::Invalid("usage: rust-music-cli scan DIRECTORY | preview DIRECTORY OUTPUT.pbm | inspect FILE | decode FILE OUTPUT.wav | play FILE [ALSA_DEVICE]")),
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
        fn format(&self) -> AudioFormat { self.0 }
        fn next_block(&mut self) -> Result<Option<rust_music::PcmBlock>> { Ok(None) }
    }

    #[test]
    fn export_refuses_to_overwrite_existing_audio() {
        use std::{fs, time::{SystemTime, UNIX_EPOCH}};
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = std::env::temp_dir().join(format!("rust-music-no-clobber-{}-{stamp}.wav", std::process::id()));
        fs::write(&path, b"KEEP_EXISTING_MUSIC").unwrap();
        let format = AudioFormat::new(44_100, 2, 16).unwrap();
        assert!(export_wav(Box::new(EmptyDecoder(format)), path.to_str().unwrap()).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"KEEP_EXISTING_MUSIC");
        fs::remove_file(path).unwrap();
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
}
