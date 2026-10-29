//! Offline decoder acceptance harness. Native device output is a separate gate.
use rust_music::{open_file, AudioFormat, Decoder, MusicError, Result};
use std::{env, fs::File, io::{Seek, SeekFrom, Write}};

fn write_header<W: Write>(output: &mut W, format: AudioFormat, data_size: u32) -> Result<()> {
    // Native FLAC may carry unusual bit depths; WAV uses whole bytes on disk.
    if !matches!(format.bits_per_sample, 8 | 16 | 24 | 32) {
        return Err(MusicError::Unsupported("WAV export supports 8/16/24/32-bit source"));
    }
    let bytes_per_frame = u32::from(format.channels) * u32::from(format.bits_per_sample / 8);
    output.write_all(b"RIFF")?;
    output.write_all(&(data_size + 36).to_le_bytes())?;
    output.write_all(b"WAVEfmt ")?;
    output.write_all(&16u32.to_le_bytes())?;
    output.write_all(&1u16.to_le_bytes())?;
    output.write_all(&u16::from(format.channels).to_le_bytes())?;
    output.write_all(&format.sample_rate.to_le_bytes())?;
    output.write_all(&format.sample_rate.checked_mul(bytes_per_frame)
        .ok_or(MusicError::Limit("WAV byte-rate overflow"))?.to_le_bytes())?;
    output.write_all(&(bytes_per_frame as u16).to_le_bytes())?;
    output.write_all(&u16::from(format.bits_per_sample).to_le_bytes())?;
    output.write_all(b"data")?;
    output.write_all(&data_size.to_le_bytes())?;
    Ok(())
}
fn export_wav(mut decoder: Box<dyn Decoder>, dest: &str) -> Result<()> {
    let format = decoder.format();
    let mut file = File::create(dest)?;
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
        file.write_all(&bytes)?;
        bytes_written = bytes_written.checked_add(bytes.len() as u64)
            .ok_or(MusicError::Limit("WAV size counter overflow"))?;
        if bytes_written > u64::from(u32::MAX - 36) {
            return Err(MusicError::Limit("WAV RIFF size limit reached; use RF64 in future"));
        }
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
    let action = args.next().ok_or(MusicError::Invalid("usage: rust-music-cli scan DIRECTORY | inspect FILE | decode FILE OUTPUT.wav | play FILE [ALSA_DEVICE]"))?;
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
        _ => Err(MusicError::Invalid("usage: rust-music-cli scan DIRECTORY | inspect FILE | decode FILE OUTPUT.wav | play FILE [ALSA_DEVICE]")),
    }
}
fn main() {
    if let Err(error) = run() {
        eprintln!("rust_music: {error}");
        std::process::exit(1);
    }
}
