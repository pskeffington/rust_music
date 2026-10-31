//! Helper for an external macOS RSS sampler. It generates only synthetic
//! WAV signatures or FLAC comments without frames, pauses before scanning, and
//! holds the scanned catalog alive for a second RSS sample.

use rust_music_library::{scan, ScanLimits};
use std::{
    env,
    fs::{self, OpenOptions},
    io::{self, BufRead, Read, Write},
    path::PathBuf,
};

const MAX_TRACKS: usize = 20_000;
const MAX_PROTOCOL_LINE_BYTES: u64 = 64;
const SIGNATURE: &[u8; 12] = b"RIFF1234WAVE";

fn argument_value(args: &mut impl Iterator<Item = String>, name: &str) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("missing value for {name}"))
}

fn flac_with_comments() -> Vec<u8> {
    let comments = [
        b"TITLE=Synthetic tagged track".as_slice(),
        b"ARTIST=Rust Music profile".as_slice(),
        b"ALBUM=Memory accounting fixture".as_slice(),
    ];
    let mut block = Vec::new();
    let vendor = b"Rust Music memory profile";
    block.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
    block.extend_from_slice(vendor);
    block.extend_from_slice(&(comments.len() as u32).to_le_bytes());
    for comment in comments {
        block.extend_from_slice(&(comment.len() as u32).to_le_bytes());
        block.extend_from_slice(comment);
    }

    let length = block.len();
    let mut bytes = b"fLaC".to_vec();
    bytes.extend_from_slice(&[0, 0, 0, 34]); // STREAMINFO, non-final
    bytes.extend_from_slice(&[0; 34]);
    bytes.extend_from_slice(&[
        0x84, // Last metadata block: Vorbis comment.
        ((length >> 16) & 0xff) as u8,
        ((length >> 8) & 0xff) as u8,
        (length & 0xff) as u8,
    ]);
    bytes.extend_from_slice(&block);
    bytes
}

fn expect_command(input: &mut impl BufRead, expected: &[u8]) -> io::Result<()> {
    let mut line = Vec::with_capacity(MAX_PROTOCOL_LINE_BYTES as usize);
    let read = input
        .take(MAX_PROTOCOL_LINE_BYTES + 1)
        .read_until(b'\n', &mut line)?;
    if read == 0 || read > MAX_PROTOCOL_LINE_BYTES as usize || line.last() != Some(&b'\n') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "profile control line is missing or exceeds 64 bytes",
        ));
    }
    if line.as_slice() != [expected, b"\n"].concat() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unexpected profile control command",
        ));
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    if args.next().as_deref() != Some("--root") {
        return Err(
            "usage: profile_micro_library_memory --root EMPTY_TEMP_DIR --tracks COUNT".into(),
        );
    }
    let root = PathBuf::from(argument_value(&mut args, "--root")?);
    if args.next().as_deref() != Some("--tracks") {
        return Err("expected --tracks".into());
    }
    let tracks: usize = argument_value(&mut args, "--tracks")?
        .parse()
        .map_err(|_| "track count must be an integer")?;
    if tracks > MAX_TRACKS {
        return Err(format!("track count exceeds {MAX_TRACKS}").into());
    }
    if args.next().as_deref() != Some("--tagged-count") {
        return Err("expected --tagged-count".into());
    }
    let tagged: usize = argument_value(&mut args, "--tagged-count")?
        .parse()
        .map_err(|_| "tagged count must be an integer")?;
    if tagged > tracks {
        return Err("tagged count cannot exceed total track count".into());
    }
    if args.next().is_some() {
        return Err("unexpected argument".into());
    }

    let root_metadata = fs::symlink_metadata(&root)?;
    if !root_metadata.is_dir() || root_metadata.file_type().is_symlink() {
        return Err("fixture root must be a real directory".into());
    }
    if fs::read_dir(&root)?.next().is_some() {
        return Err("fixture root must be empty".into());
    }

    let tagged_fixture = flac_with_comments();
    for index in 0..tracks {
        let (suffix, contents) = if index < tagged {
            ("flac", tagged_fixture.as_slice())
        } else {
            ("wav", SIGNATURE.as_slice())
        };
        let path = root.join(format!("track-{index:05}.{suffix}"));
        let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
        file.write_all(contents)?;
    }

    println!(
        "READY pid={} tracks={tracks} tagged={tagged}",
        std::process::id()
    );
    io::stdout().flush()?;
    let stdin = io::stdin();
    let mut input = stdin.lock();
    expect_command(&mut input, b"scan")?;

    let library = scan(
        &root,
        ScanLimits {
            max_entries: 100_000,
            max_tracks: MAX_TRACKS,
            max_depth: 6,
        },
    )?;
    if library.len() != tracks {
        return Err(format!("expected {tracks} tracks, scanned {}", library.len()).into());
    }
    let tagged_scanned = library
        .tracks()
        .iter()
        .filter(|track| track.tags.is_some())
        .count();
    if tagged_scanned != tagged {
        return Err(format!("expected {tagged} tagged tracks, scanned {tagged_scanned}").into());
    }
    println!("SCANNED tracks={} tagged={tagged_scanned}", library.len());
    io::stdout().flush()?;

    expect_command(&mut input, b"release")?;
    println!("RELEASED");
    Ok(())
}
