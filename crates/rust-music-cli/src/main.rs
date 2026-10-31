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
use rust_music_library::ScanLimits;
#[cfg(any(
    all(target_os = "macos", feature = "macos-output"),
    all(target_os = "linux", feature = "alsa-output")
))]
use rust_music_library::{scan, AudioContainer as LibraryContainer, Library};
use std::{
    env,
    fs::OpenOptions,
    io::{Read, Seek, SeekFrom, Write},
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

const MAX_CATALOG_OUTPUT_BYTES: usize = 20 * 1024 * 1024;
const MAX_STATE_TSV_BYTES: usize = 6 * 1024 * 1024;
const STANDALONE_MAX_TRACKS: usize = 20_000;
const STANDALONE_SCAN_LIMITS: ScanLimits = ScanLimits {
    max_entries: 100_000,
    max_tracks: STANDALONE_MAX_TRACKS,
    max_depth: 6,
};

const MANAGED_PARENT_STDIN_FLAG: &str = "--managed-parent-stdin";

/// Opt-in lifetime supervision for one-shot jobs launched by the desktop app.
/// The caller must provide a pipe on stdin and keep its write end open until
/// the job is no longer owned by the app. Reading at most one byte keeps this
/// watcher bounded; any input is treated as an invalid supervision signal.
fn supervise_parent_stdin() -> Result<()> {
    std::thread::Builder::new()
        .name("rust-music-parent-watch".into())
        .spawn(|| {
            let mut input = std::io::stdin().lock();
            let mut byte = [0u8; 1];
            #[cfg(all(debug_assertions, feature = "test-support"))]
            {
                let stderr = std::io::stderr();
                let mut output = stderr.lock();
                let _ = writeln!(
                    output,
                    "rust_music: managed child pid={} status=watching-parent",
                    std::process::id()
                );
                let _ = output.flush();
            }
            let (status, code) = match input.read(&mut byte) {
                Ok(0) => ("parent-disconnected", 0),
                Ok(_) => ("unexpected-parent-data", 1),
                Err(_) => ("parent-pipe-error", 1),
            };
            let stderr = std::io::stderr();
            let mut output = stderr.lock();
            let _ = writeln!(
                output,
                "rust_music: managed child pid={} status={status}",
                std::process::id()
            );
            let _ = output.flush();
            std::process::exit(code);
        })
        .map_err(MusicError::Io)?;
    Ok(())
}

fn append_hex(output: &mut String, value: &str) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in value.as_bytes() {
        output.push(HEX[usize::from(byte >> 4)] as char);
        output.push(HEX[usize::from(byte & 0x0f)] as char);
    }
}

fn container_name(container: rust_music_library::AudioContainer) -> &'static str {
    match container {
        rust_music_library::AudioContainer::Flac => "flac",
        rust_music_library::AudioContainer::Wave => "wave",
        rust_music_library::AudioContainer::Aiff => "aiff",
        rust_music_library::AudioContainer::Caf => "caf",
        rust_music_library::AudioContainer::Mp4 => "mp4",
    }
}

fn catalog_tsv(root: &std::path::Path, query_text: &str) -> Result<String> {
    use rust_music_library::{scan, Track, TrackQuery, TrackSort, MAX_PAGE_SIZE};

    let library = scan(root, STANDALONE_SCAN_LIMITS)?;
    let mut output = String::new();
    let mut emitted = 0usize;
    let mut append_track = |index: usize, track: &Track| -> Result<()> {
        let relative = track
            .relative_path
            .to_str()
            .ok_or(MusicError::Invalid("catalog path is not UTF-8"))?;
        output.push_str("track\t");
        use std::fmt::Write as _;
        write!(output, "{}\t{}\t", index, container_name(track.container))
            .expect("writing to String cannot fail");
        append_hex(&mut output, track.display_title());
        output.push('\t');
        append_hex(
            &mut output,
            track
                .tags
                .as_ref()
                .and_then(|tags| tags.artist())
                .unwrap_or(""),
        );
        output.push('\t');
        append_hex(
            &mut output,
            track
                .tags
                .as_ref()
                .and_then(|tags| tags.album())
                .unwrap_or(""),
        );
        output.push('\t');
        append_hex(&mut output, relative);
        output.push('\n');
        emitted = emitted.saturating_add(1);
        if output.len() > MAX_CATALOG_OUTPUT_BYTES {
            return Err(MusicError::Limit("catalog output exceeds its size bound"));
        }
        Ok(())
    };

    if query_text.is_empty() {
        // Library tracks are already path-sorted. Avoid rescanning and
        // resorting all 20k rows once per 200-row page during desktop import.
        for (index, track) in library.tracks().iter().enumerate() {
            append_track(index, track)?;
        }
    } else {
        let mut offset = 0usize;
        loop {
            let page = library.query(&TrackQuery {
                text: query_text.to_owned(),
                sort_by: TrackSort::Path,
                offset,
                limit: MAX_PAGE_SIZE,
                ..TrackQuery::default()
            })?;
            for track in &page.tracks {
                let index = library
                    .tracks()
                    .binary_search_by(|candidate| candidate.relative_path.cmp(&track.relative_path))
                    .map_err(|_| MusicError::Invalid("catalog track index is inconsistent"))?;
                append_track(index, track)?;
            }
            if offset.saturating_add(page.tracks.len()) >= page.total_matches {
                break;
            }
            offset = offset.saturating_add(page.tracks.len());
        }
    }
    drop(append_track);
    use std::fmt::Write as _;
    writeln!(
        output,
        "summary\t{}\t{}\t{}",
        emitted, library.examined_entries, library.skipped_entries
    )
    .expect("writing to String cannot fail");
    if output.len() > MAX_CATALOG_OUTPUT_BYTES {
        return Err(MusicError::Limit("catalog output exceeds its size bound"));
    }
    Ok(output)
}

fn resolve_library_track(
    root: &std::path::Path,
    relative: &std::path::Path,
) -> Result<std::path::PathBuf> {
    use rust_music_library::scan;
    use std::path::Component;

    if relative.as_os_str().is_empty()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(MusicError::Invalid(
            "managed library path must be relative and safe",
        ));
    }
    let library = scan(root, STANDALONE_SCAN_LIMITS)?;
    let index = library
        .tracks()
        .iter()
        .position(|track| track.relative_path == relative)
        .ok_or(MusicError::Invalid(
            "selected library track is missing or unsupported",
        ))?;
    library.checked_path(index)
}

fn platform_state_base() -> Result<std::path::PathBuf> {
    use std::path::PathBuf;

    #[cfg(target_os = "macos")]
    let base = env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or(MusicError::Invalid("HOME is unavailable for music state"))?
        .join("Library/Application Support/Rust Music");

    #[cfg(target_os = "windows")]
    let base = env::var_os("APPDATA")
        .map(PathBuf::from)
        .ok_or(MusicError::Invalid(
            "APPDATA is unavailable for music state",
        ))?
        .join("Rust Music");

    #[cfg(all(unix, not(target_os = "macos")))]
    let base = if let Some(xdg) = env::var_os("XDG_DATA_HOME") {
        PathBuf::from(xdg).join("rust-music")
    } else {
        env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or(MusicError::Invalid("HOME is unavailable for music state"))?
            .join(".local/share/rust-music")
    };

    #[cfg(not(any(unix, windows)))]
    let base = env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or(MusicError::Invalid("HOME is unavailable for music state"))?
        .join(".local/share/rust-music");

    Ok(base)
}

fn root_state_key(root: &std::path::Path) -> Result<String> {
    let metadata = std::fs::symlink_metadata(root)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(MusicError::Invalid(
            "state root must be a real library directory",
        ));
    }
    let canonical = root.canonicalize()?;
    let bytes = path_identity_bytes(&canonical);
    // FNV-1a 128 is used only to keep root paths out of state filenames and
    // separate per-root records; it is not an authentication primitive.
    let mut hash = 0x6c62272e07bb014262b821756295c58du128;
    for byte in bytes {
        hash ^= u128::from(byte);
        hash = hash.wrapping_mul(0x0000000001000000000000000000013bu128);
    }
    let mut encoded = String::with_capacity(32);
    use std::fmt::Write as _;
    write!(&mut encoded, "{hash:032x}").expect("writing to String cannot fail");
    Ok(encoded)
}

#[cfg(unix)]
fn path_identity_bytes(path: &std::path::Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().to_vec()
}

#[cfg(windows)]
fn path_identity_bytes(path: &std::path::Path) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt;
    path.as_os_str()
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect()
}

#[cfg(not(any(unix, windows)))]
fn path_identity_bytes(path: &std::path::Path) -> Vec<u8> {
    path.to_string_lossy().as_bytes().to_vec()
}

fn state_file_path(
    root: &std::path::Path,
    base: &std::path::Path,
    extension: &str,
) -> Result<std::path::PathBuf> {
    if !matches!(extension, "rmq" | "rmp") {
        return Err(MusicError::Invalid("unsupported music state file type"));
    }
    let key = root_state_key(root)?;
    Ok(base.join(format!("root-{key}.{extension}")))
}

fn load_queue_state(path: &std::path::Path) -> Result<rust_music_library::queue::PlaybackQueue> {
    use rust_music_library::queue::PlaybackQueue;
    match std::fs::symlink_metadata(path) {
        Ok(_) => PlaybackQueue::load_from(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(PlaybackQueue::new()),
        Err(error) => Err(error.into()),
    }
}

fn load_playlist_state(
    path: &std::path::Path,
) -> Result<rust_music_library::playlist_store::PlaylistStore> {
    use rust_music_library::playlist_store::PlaylistStore;
    match std::fs::symlink_metadata(path) {
        Ok(_) => PlaylistStore::load_from(path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(PlaylistStore::new()),
        Err(error) => Err(error.into()),
    }
}

fn ensure_state_base(base: &std::path::Path) -> Result<()> {
    match std::fs::symlink_metadata(base) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                let mut builder = std::fs::DirBuilder::new();
                builder.recursive(true).mode(0o700).create(base)?;
            }
            #[cfg(not(unix))]
            std::fs::create_dir_all(base)?;
        }
        Err(error) => return Err(error.into()),
    }
    let metadata = std::fs::symlink_metadata(base)?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(MusicError::Invalid(
            "music state directory must be a real directory",
        ));
    }
    Ok(())
}

fn validate_existing_state_base(base: &std::path::Path) -> Result<()> {
    match std::fs::symlink_metadata(base) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => Err(
            MusicError::Invalid("music state directory must be a real directory"),
        ),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn queue_listing(queue: &rust_music_library::queue::PlaybackQueue) -> Result<String> {
    use rust_music_library::queue::RepeatMode;
    use std::fmt::Write as _;

    let mut output = String::new();
    output.push_str("meta\trepeat\t");
    output.push_str(match queue.repeat_mode() {
        RepeatMode::Off => "off\n",
        RepeatMode::One => "one\n",
        RepeatMode::All => "all\n",
    });
    output.push_str("meta\tshuffle\t");
    match queue.shuffle_seed() {
        Some(seed) => writeln!(output, "{seed}").expect("writing to String cannot fail"),
        None => output.push_str("-\n"),
    }
    output.push_str("meta\tcurrent\t");
    match queue.current_index() {
        Some(index) => writeln!(output, "{index}").expect("writing to String cannot fail"),
        None => output.push_str("-\n"),
    }
    for (index, path) in queue.entries().iter().enumerate() {
        let path = path
            .to_str()
            .ok_or(MusicError::Invalid("queue path is not UTF-8"))?;
        write!(
            output,
            "entry\t{index}\t{}\t",
            u8::from(queue.current_index() == Some(index))
        )
        .expect("writing to String cannot fail");
        append_hex(&mut output, path);
        output.push('\n');
        if output.len() > MAX_STATE_TSV_BYTES {
            return Err(MusicError::Limit(
                "music state listing exceeds its size bound",
            ));
        }
    }
    Ok(output)
}

fn playlist_listing(store: &rust_music_library::playlist_store::PlaylistStore) -> Result<String> {
    use std::fmt::Write as _;

    let mut playlists = store.playlists().iter().collect::<Vec<_>>();
    playlists.sort_by(|left, right| left.name().cmp(right.name()));
    let mut output = String::new();
    for item in playlists {
        output.push_str("playlist\t");
        append_hex(&mut output, item.name());
        writeln!(output, "\t{}", item.playlist().len()).expect("writing to String cannot fail");
        for (index, path) in item.playlist().entries().iter().enumerate() {
            let path = path
                .to_str()
                .ok_or(MusicError::Invalid("playlist path is not UTF-8"))?;
            output.push_str("entry\t");
            append_hex(&mut output, item.name());
            write!(output, "\t{index}\t").expect("writing to String cannot fail");
            append_hex(&mut output, path);
            output.push('\n');
            if output.len() > MAX_STATE_TSV_BYTES {
                return Err(MusicError::Limit(
                    "music state listing exceeds its size bound",
                ));
            }
        }
        if output.len() > MAX_STATE_TSV_BYTES {
            return Err(MusicError::Limit(
                "music state listing exceeds its size bound",
            ));
        }
    }
    Ok(output)
}

fn parse_state_index(value: &str) -> Result<usize> {
    if value.is_empty() || value.len() > 10 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(MusicError::Invalid("music state index is invalid"));
    }
    value
        .parse::<usize>()
        .map_err(|_| MusicError::Invalid("music state index is invalid"))
}

fn state_command(
    base: &std::path::Path,
    category: &str,
    root: &std::path::Path,
    operation: &str,
    arguments: &[String],
) -> Result<String> {
    if category.len() > 16 || operation.len() > 32 || root.as_os_str().len() > 4096 {
        return Err(MusicError::Limit(
            "music state command exceeds its size bound",
        ));
    }
    validate_existing_state_base(base)?;
    match category {
        "queue" => state_queue_command(base, root, operation, arguments),
        "playlist" => state_playlist_command(base, root, operation, arguments),
        _ => Err(MusicError::Invalid(
            "state category must be queue or playlist",
        )),
    }
}

fn state_queue_command(
    base: &std::path::Path,
    root: &std::path::Path,
    operation: &str,
    arguments: &[String],
) -> Result<String> {
    use rust_music_library::queue::{PlaybackQueue, RepeatMode};
    use std::path::PathBuf;

    let queue_path = state_file_path(root, base, "rmq")?;
    let mut queue = load_queue_state(&queue_path)?;
    let mut changed = false;
    let mut candidate_available = None;
    match operation {
        "list" if arguments.is_empty() => {}
        "add" if arguments.len() == 1 => {
            queue.enqueue(PathBuf::from(&arguments[0]))?;
            changed = true;
        }
        "remove" if arguments.len() == 1 => {
            let index = parse_state_index(&arguments[0])?;
            queue
                .remove(index)
                .ok_or(MusicError::Invalid("queue index is out of bounds"))?;
            changed = true;
        }
        "select" if arguments.len() == 1 => {
            queue.select(parse_state_index(&arguments[0])?)?;
            changed = true;
        }
        "next" if arguments.is_empty() => {
            // Selection remains at the current entry when Repeat Off reaches
            // the boundary, so report whether a candidate really advanced.
            // The app uses this to avoid replaying the boundary track.
            candidate_available = Some(queue.next_candidate().is_some());
            changed = true;
        }
        "previous" if arguments.is_empty() => {
            candidate_available = Some(queue.previous_candidate().is_some());
            changed = true;
        }
        "repeat" if arguments.len() == 1 => {
            queue.set_repeat_mode(match arguments[0].as_str() {
                "off" => RepeatMode::Off,
                "one" => RepeatMode::One,
                "all" => RepeatMode::All,
                _ => return Err(MusicError::Invalid("repeat must be off, one, or all")),
            });
            changed = true;
        }
        "shuffle" if arguments.len() == 1 => {
            if arguments[0].len() > 20 {
                return Err(MusicError::Invalid("shuffle seed is invalid"));
            }
            let seed = arguments[0]
                .parse::<u64>()
                .map_err(|_| MusicError::Invalid("shuffle seed is invalid"))?;
            queue.shuffle(seed);
            changed = true;
        }
        "shuffle-off" if arguments.is_empty() => {
            queue.clear_shuffle();
            changed = true;
        }
        "clear" if arguments.is_empty() => {
            queue = PlaybackQueue::new();
            changed = true;
        }
        "add-playlist" if arguments.len() == 1 => {
            let playlist_path = state_file_path(root, base, "rmp")?;
            let store = load_playlist_state(&playlist_path)?;
            let playlist = store
                .get(&arguments[0])
                .ok_or(MusicError::Invalid("named playlist does not exist"))?;
            for path in playlist.entries() {
                queue.enqueue(path.clone())?;
            }
            changed = true;
        }
        _ => {
            return Err(MusicError::Invalid(
                "invalid queue state command or arguments",
            ))
        }
    }
    if changed {
        ensure_state_base(base)?;
        queue.save_atomic(&queue_path)?;
    }
    let mut listing = queue_listing(&queue)?;
    if let Some(available) = candidate_available {
        use std::fmt::Write as _;
        writeln!(
            listing,
            "meta\tcandidate-available\t{}",
            if available { "yes" } else { "no" }
        )
        .expect("writing to String cannot fail");
        if listing.len() > MAX_STATE_TSV_BYTES {
            return Err(MusicError::Limit(
                "music state listing exceeds its size bound",
            ));
        }
    }
    Ok(listing)
}

fn state_playlist_command(
    base: &std::path::Path,
    root: &std::path::Path,
    operation: &str,
    arguments: &[String],
) -> Result<String> {
    use std::path::PathBuf;

    let playlist_path = state_file_path(root, base, "rmp")?;
    let mut store = load_playlist_state(&playlist_path)?;
    let changed = match operation {
        "list" if arguments.is_empty() => false,
        "create" if arguments.len() == 1 => {
            store.create(&arguments[0])?;
            true
        }
        "rename" if arguments.len() == 2 => {
            store.rename(&arguments[0], &arguments[1])?;
            true
        }
        "delete" if arguments.len() == 1 => store
            .remove(&arguments[0])
            .map(|_| true)
            .ok_or(MusicError::Invalid("named playlist does not exist"))?,
        "add" if arguments.len() == 2 => {
            store.push_entry(&arguments[0], PathBuf::from(&arguments[1]))?;
            true
        }
        "remove" if arguments.len() == 2 => {
            let index = parse_state_index(&arguments[1])?;
            store.remove_entry(&arguments[0], index)?;
            true
        }
        _ => {
            return Err(MusicError::Invalid(
                "invalid playlist state command or arguments",
            ))
        }
    };
    if changed {
        ensure_state_base(base)?;
        store.save_atomic(&playlist_path)?;
    }
    playlist_listing(&store)
}

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
#[cfg(any(
    all(target_os = "macos", feature = "macos-output"),
    all(target_os = "linux", feature = "alsa-output")
))]
fn play_with_sink<S: PcmSink>(
    input: Box<dyn Decoder>,
    player: &mut rust_music::Player<S>,
    parent_watch: Option<&ParentWatch>,
) -> Result<()> {
    use rust_music::PlaybackState;
    player.load(input)?;
    if parent_watch.is_some_and(ParentWatch::is_disconnected) {
        return player.stop();
    }
    player.play()?;
    while player.step()?.state == PlaybackState::Playing {
        if parent_watch.is_some_and(ParentWatch::is_disconnected) {
            return player.stop();
        }
    }
    Ok(())
}

#[cfg(all(target_os = "macos", feature = "macos-output"))]
fn play(
    input: Box<dyn Decoder>,
    _device: Option<&str>,
    parent_watch: Option<&ParentWatch>,
) -> Result<()> {
    let mut player = rust_music::Player::new(rust_music::MacAudioQueueSink::default());
    play_with_sink(input, &mut player, parent_watch)
}
#[cfg(all(target_os = "linux", feature = "alsa-output"))]
fn play(
    input: Box<dyn Decoder>,
    device: Option<&str>,
    parent_watch: Option<&ParentWatch>,
) -> Result<()> {
    let device = device.ok_or(MusicError::Invalid(
        "Linux playback requires explicit hw:CARD,DEVICE",
    ))?;
    let mut player = rust_music::Player::new(rust_music::AlsaSink::new(
        device,
        device.starts_with("hw:"),
    )?);
    play_with_sink(input, &mut player, parent_watch)
}
#[cfg(not(any(
    all(target_os = "macos", feature = "macos-output"),
    all(target_os = "linux", feature = "alsa-output")
)))]
fn play(
    _input: Box<dyn Decoder>,
    _device: Option<&str>,
    _parent_watch: Option<&ParentWatch>,
) -> Result<()> {
    Err(MusicError::Unsupported(
        "native playback needs --features macos-output (macOS) or alsa-output (Linux)",
    ))
}

#[cfg(any(
    all(target_os = "macos", feature = "macos-output"),
    all(target_os = "linux", feature = "alsa-output")
))]
struct ParentWatch {
    disconnected: std::sync::Arc<AtomicBool>,
    playback_finished: std::sync::Arc<AtomicBool>,
}

#[cfg(not(any(
    all(target_os = "macos", feature = "macos-output"),
    all(target_os = "linux", feature = "alsa-output")
)))]
struct ParentWatch;

#[cfg(any(
    all(target_os = "macos", feature = "macos-output"),
    all(target_os = "linux", feature = "alsa-output")
))]
impl ParentWatch {
    fn is_disconnected(&self) -> bool {
        self.disconnected.load(Ordering::Acquire)
    }
}

#[cfg(any(
    all(target_os = "macos", feature = "macos-output"),
    all(target_os = "linux", feature = "alsa-output")
))]
impl Drop for ParentWatch {
    fn drop(&mut self) {
        self.playback_finished.store(true, Ordering::Release);
    }
}

#[cfg(any(
    all(target_os = "macos", feature = "macos-output"),
    all(target_os = "linux", feature = "alsa-output")
))]
fn watch_parent_disconnect<R>(mut reader: R) -> ParentWatch
where
    R: std::io::Read + Send + 'static,
{
    let disconnected = std::sync::Arc::new(AtomicBool::new(false));
    let playback_finished = std::sync::Arc::new(AtomicBool::new(false));
    let signal = std::sync::Arc::clone(&disconnected);
    let finished = std::sync::Arc::clone(&playback_finished);
    thread::spawn(move || {
        let mut bytes = [0u8; 256];
        loop {
            match reader.read(&mut bytes) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
        }
        signal.store(true, Ordering::Release);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while !finished.load(Ordering::Acquire) {
            if std::time::Instant::now() >= deadline {
                // The parent is gone and the playback thread did not release
                // the native sink in time. Process exit closes OS resources
                // and prevents a detached playback process from surviving.
                std::process::exit(2);
            }
            thread::sleep(Duration::from_millis(10));
        }
    });
    ParentWatch {
        disconnected,
        playback_finished,
    }
}

#[cfg(any(
    all(target_os = "macos", feature = "macos-output"),
    all(target_os = "linux", feature = "alsa-output")
))]
fn play_managed(path: &std::path::Path, device: Option<&str>) -> Result<()> {
    let parent_watch = watch_parent_disconnect(std::io::stdin());
    play(open_file(path)?, device, Some(&parent_watch))
}

#[cfg(not(any(
    all(target_os = "macos", feature = "macos-output"),
    all(target_os = "linux", feature = "alsa-output")
)))]
fn play_managed(_path: &std::path::Path, _device: Option<&str>) -> Result<()> {
    Err(MusicError::Unsupported(
        "managed playback needs a native audio output feature",
    ))
}

#[cfg(all(
    debug_assertions,
    feature = "test-support",
    any(
        all(target_os = "macos", feature = "macos-output"),
        all(target_os = "linux", feature = "alsa-output")
    )
))]
fn run_parent_guard_test() -> Result<()> {
    let parent_watch = watch_parent_disconnect(std::io::stdin());
    while !parent_watch.is_disconnected() {
        thread::sleep(Duration::from_millis(5));
    }
    println!("parent-stdin-closed");
    Ok(())
}

#[cfg(all(
    debug_assertions,
    feature = "test-support",
    any(
        all(target_os = "macos", feature = "macos-output"),
        all(target_os = "linux", feature = "alsa-output")
    )
))]
struct TestPacedNullSink(NullSink);

#[cfg(all(
    debug_assertions,
    feature = "test-support",
    any(
        all(target_os = "macos", feature = "macos-output"),
        all(target_os = "linux", feature = "alsa-output")
    )
))]
impl PcmSink for TestPacedNullSink {
    fn open(&mut self, format: AudioFormat) -> Result<()> {
        self.0.open(format)
    }

    fn write(&mut self, block: &rust_music::PcmBlock) -> Result<()> {
        self.0.write(block)?;
        thread::sleep(Duration::from_millis(2));
        Ok(())
    }

    fn pause(&mut self) -> Result<()> {
        self.0.pause()
    }

    fn resume(&mut self) -> Result<()> {
        self.0.resume()
    }

    fn finish(&mut self) -> Result<()> {
        self.0.finish()
    }
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
fn serve(device: Option<&str>, desktop: bool) -> Result<()> {
    if device.is_some() {
        return Err(MusicError::Invalid(
            "macOS process unit does not accept a device argument",
        ));
    }
    let limits = if desktop {
        STANDALONE_SCAN_LIMITS
    } else {
        ScanLimits::default()
    };
    serve_with_sink_and_limits(|| Ok(rust_music::MacAudioQueueSink::default()), limits)
}

#[cfg(all(target_os = "linux", feature = "alsa-output"))]
fn serve(device: Option<&str>, desktop: bool) -> Result<()> {
    let device = device.ok_or(MusicError::Invalid(
        "Linux package process requires explicit hw:CARD,DEVICE",
    ))?;
    if !device.starts_with("hw:") {
        return Err(MusicError::Invalid(
            "Linux package process requires hw:CARD,DEVICE",
        ));
    }
    let device = device.to_owned();
    let limits = if desktop {
        STANDALONE_SCAN_LIMITS
    } else {
        ScanLimits::default()
    };
    serve_with_sink_and_limits(move || rust_music::AlsaSink::new(&device, true), limits)
}

#[cfg(not(any(
    all(target_os = "macos", feature = "macos-output"),
    all(target_os = "linux", feature = "alsa-output")
)))]
fn serve(_device: Option<&str>, _desktop: bool) -> Result<()> {
    Err(MusicError::Unsupported(
        "package process playback needs macos-output or alsa-output",
    ))
}

#[cfg(any(
    all(target_os = "macos", feature = "macos-output"),
    all(target_os = "linux", feature = "alsa-output")
))]
#[cfg(any(
    all(target_os = "macos", feature = "macos-output"),
    all(target_os = "linux", feature = "alsa-output")
))]
fn serve_with_sink_and_limits<S, F>(make_sink: F, scan_limits: ScanLimits) -> Result<()>
where
    S: PcmSink + 'static,
    F: FnOnce() -> Result<S> + Send + 'static,
{
    let input = std::io::stdin();
    let output = std::io::stdout();
    serve_stream_with_limits(make_sink, input.lock(), output.lock(), scan_limits)
}

#[cfg(any(
    all(target_os = "macos", feature = "macos-output"),
    all(target_os = "linux", feature = "alsa-output")
))]
#[cfg(test)]
fn serve_stream<S, F, I, O>(make_sink: F, input: I, output: O) -> Result<()>
where
    S: PcmSink + 'static,
    F: FnOnce() -> Result<S> + Send + 'static,
    I: std::io::Read,
    O: Write,
{
    serve_stream_with_limits(make_sink, input, output, ScanLimits::default())
}

#[cfg(any(
    all(target_os = "macos", feature = "macos-output"),
    all(target_os = "linux", feature = "alsa-output")
))]
fn serve_stream_with_limits<S, F, I, O>(
    make_sink: F,
    mut input: I,
    mut output: O,
    scan_limits: ScanLimits,
) -> Result<()>
where
    S: PcmSink + 'static,
    F: FnOnce() -> Result<S> + Send + 'static,
    I: std::io::Read,
    O: Write,
{
    let (request_tx, request_rx) = mpsc::sync_channel::<UnitRequest>(1);
    let (reply_tx, reply_rx) = mpsc::sync_channel(1);
    let worker =
        thread::spawn(move || process_worker(make_sink, request_rx, reply_tx, scan_limits));
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
    scan_limits: ScanLimits,
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
                let reply =
                    handle_unit_request_with_limits(&mut slot, &focus, request, scan_limits);
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
#[cfg(test)]
fn handle_unit_request<S: PcmSink>(
    slot: &mut MusicSlot<S>,
    focus: &ProcessFocus,
    request: Envelope<Request>,
) -> Reply {
    handle_unit_request_with_limits(slot, focus, request, ScanLimits::default())
}

#[cfg(any(
    all(target_os = "macos", feature = "macos-output"),
    all(target_os = "linux", feature = "alsa-output")
))]
fn handle_unit_request_with_limits<S: PcmSink>(
    slot: &mut MusicSlot<S>,
    focus: &ProcessFocus,
    request: Envelope<Request>,
    scan_limits: ScanLimits,
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
            scan(root, scan_limits).and_then(|library| slot.replace_library(library).map(|_| ()))
        }
        Request::Select { index } => slot
            .handle(Command::Select(usize::from(index)), focus)
            .map(|_| ()),
        Request::Previous => slot.handle(Command::Previous, focus).map(|_| ()),
        Request::Next => slot.handle(Command::Next, focus).map(|_| ()),
        Request::Pause => {
            focus.grant.store(false, Ordering::Release);
            slot.handle(Command::Pause, focus).map(|_| ())
        }
        Request::ResumePlayback => {
            focus.grant.store(true, Ordering::Release);
            let result = slot.handle(Command::ResumePlayback, focus).map(|_| ());
            if result.is_err() {
                focus.grant.store(false, Ordering::Release);
            }
            result
        }
        Request::SetVolumePercent { percent } => slot
            .handle(Command::SetVolumePercent(percent), focus)
            .map(|_| ()),
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

    fn pause(&mut self) -> Result<()> {
        self.sink
            .as_mut()
            .ok_or(MusicError::Output("audio sink is not open".into()))?
            .pause()
    }

    fn resume(&mut self) -> Result<()> {
        self.sink
            .as_mut()
            .ok_or(MusicError::Output("audio sink is not open".into()))?
            .resume()
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
        MusicError::Cancelled(_) => process_error(
            ErrorCode::InvalidRequest,
            "music library scan was cancelled",
        ),
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
        rust_music::PlaybackState::Paused => Playback::Paused,
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
    let action = args.next().ok_or(MusicError::Invalid("usage: rust-music-cli scan DIRECTORY | catalog [--managed-parent-stdin] DIRECTORY [QUERY] | state [--managed-parent-stdin] queue|playlist ROOT COMMAND [ARGS] | preview DIRECTORY OUTPUT.pbm | inspect FILE | decode FILE OUTPUT.wav | decode-range FILE START_FRAME FRAME_COUNT OUTPUT.wav | play FILE [ALSA_DEVICE] | play-managed-root DIRECTORY RELATIVE_PATH [ALSA_DEVICE] | serve [--desktop] [ALSA_DEVICE]"))?;
    #[cfg(all(
        debug_assertions,
        feature = "test-support",
        any(
            all(target_os = "macos", feature = "macos-output"),
            all(target_os = "linux", feature = "alsa-output")
        )
    ))]
    if action == "__test-parent-guard" {
        if args.next().is_some() {
            return Err(MusicError::Invalid(
                "unexpected parent guard test arguments",
            ));
        }
        return run_parent_guard_test();
    }
    #[cfg(all(
        debug_assertions,
        feature = "test-support",
        any(
            all(target_os = "macos", feature = "macos-output"),
            all(target_os = "linux", feature = "alsa-output")
        )
    ))]
    if action == "__test-serve-paced-null" {
        if args.next().is_some() {
            return Err(MusicError::Invalid(
                "unexpected paced-null server test arguments",
            ));
        }
        return serve_with_sink_and_limits(
            || Ok(TestPacedNullSink(NullSink::default())),
            ScanLimits::default(),
        );
    }
    if action == "serve" {
        let first = args.next();
        let desktop = first.as_deref() == Some("--desktop");
        let device = if desktop { args.next() } else { first };
        if args.next().is_some() {
            return Err(MusicError::Invalid("too many package process arguments"));
        }
        return serve(device.as_deref(), desktop);
    }
    if action == "play-managed" {
        let path = args.next().ok_or(MusicError::Invalid(
            "play-managed requires an input filename",
        ))?;
        let device = args.next();
        if args.next().is_some() {
            return Err(MusicError::Invalid("too many managed playback arguments"));
        }
        return play_managed(std::path::Path::new(&path), device.as_deref());
    }
    if action == "play-managed-root" {
        let root = args.next().ok_or(MusicError::Invalid(
            "play-managed-root requires a library root",
        ))?;
        let relative = args.next().ok_or(MusicError::Invalid(
            "play-managed-root requires a relative library path",
        ))?;
        let device = args.next();
        if args.next().is_some() {
            return Err(MusicError::Invalid(
                "too many managed library playback arguments",
            ));
        }
        let checked =
            resolve_library_track(std::path::Path::new(&root), std::path::Path::new(&relative))?;
        return play_managed(&checked, device.as_deref());
    }
    if action == "state" {
        let first = args
            .next()
            .ok_or(MusicError::Invalid("state requires queue or playlist"))?;
        let managed = first == MANAGED_PARENT_STDIN_FLAG;
        let category = if managed {
            args.next()
                .ok_or(MusicError::Invalid("state requires queue or playlist"))?
        } else {
            first
        };
        let root = args
            .next()
            .ok_or(MusicError::Invalid("state requires a library root"))?;
        let operation = args
            .next()
            .ok_or(MusicError::Invalid("state requires a command"))?;
        let mut arguments = Vec::new();
        for argument in args {
            if arguments.len() >= 2 || argument.len() > 4096 {
                return Err(MusicError::Limit("too many or oversized state arguments"));
            }
            arguments.push(argument);
        }
        if managed {
            supervise_parent_stdin()?;
        }
        let output = state_command(
            &platform_state_base()?,
            &category,
            std::path::Path::new(&root),
            &operation,
            &arguments,
        )?;
        print!("{output}");
        return Ok(());
    }
    if action == "catalog" {
        let first = args
            .next()
            .ok_or(MusicError::Invalid("catalog requires a directory"))?;
        let (managed, path) = if first == MANAGED_PARENT_STDIN_FLAG {
            (
                true,
                args.next()
                    .ok_or(MusicError::Invalid("managed catalog requires a directory"))?,
            )
        } else {
            (false, first)
        };
        let query = args.next().unwrap_or_default();
        if args.next().is_some() {
            return Err(MusicError::Invalid("too many catalog arguments"));
        }
        if managed {
            supervise_parent_stdin()?;
        }
        let output = catalog_tsv(std::path::Path::new(&path), &query)?;
        #[cfg(all(debug_assertions, feature = "test-support"))]
        if let Ok(delay) = env::var("RUST_MUSIC_TEST_MANAGED_CATALOG_DELAY_MS") {
            if let Ok(milliseconds) = delay.parse::<u64>() {
                if milliseconds <= 5_000 {
                    std::thread::sleep(std::time::Duration::from_millis(milliseconds));
                }
            }
        }
        print!("{output}");
        return Ok(());
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
            play(open_file(path)?, device.as_deref(), None)
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
        _ => Err(MusicError::Invalid("usage: rust-music-cli scan DIRECTORY | catalog DIRECTORY [QUERY] | state queue|playlist ROOT COMMAND [ARGS] | preview DIRECTORY OUTPUT.pbm | inspect FILE | decode FILE OUTPUT.wav | decode-range FILE START_FRAME FRAME_COUNT OUTPUT.wav | play FILE [ALSA_DEVICE] | play-managed-root DIRECTORY RELATIVE_PATH [ALSA_DEVICE] | serve [ALSA_DEVICE]")),
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
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn header(format: AudioFormat, data_bytes: u32) -> Vec<u8> {
        let mut out = Vec::new();
        write_header(&mut out, format, data_bytes).unwrap();
        out
    }

    fn temp_root(label: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("rust-music-{label}-{}-{stamp}", std::process::id()));
        fs::create_dir(&root).unwrap();
        root
    }

    #[test]
    fn catalog_output_is_bounded_tsv_with_utf8_hex_and_text_search() {
        let root = temp_root("catalog");
        fs::write(root.join("Café.wav"), b"RIFF1234WAVE").unwrap();
        fs::write(root.join("Other.flac"), b"fLaC").unwrap();

        let output = catalog_tsv(&root, "CAFÉ").unwrap();
        assert_eq!(
            output,
            "track\t0\twave\t436166c3a9\t\t\t436166c3a92e776176\nsummary\t1\t2\t0\n"
        );
        assert!(!output.contains(root.to_str().unwrap()));
        assert!(catalog_tsv(&root, "bad\nquery").is_err());
        assert!(catalog_tsv(&root, &"x".repeat(513)).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn desktop_catalog_reaches_twenty_thousand_tracks_with_stable_indexes() {
        let root = temp_root("catalog-20k");
        for index in 0..STANDALONE_MAX_TRACKS {
            fs::write(root.join(format!("track-{index:05}.wav")), b"RIFF1234WAVE").unwrap();
        }

        let output = catalog_tsv(&root, "").unwrap();
        assert!(output.contains("track\t19999\twave\t"));
        assert!(output.ends_with("summary\t20000\t20000\t0\n"));
        assert!(output.len() < MAX_CATALOG_OUTPUT_BYTES);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn managed_library_resolution_rejects_traversal_and_missing_tracks() {
        let root = temp_root("managed-root");
        fs::write(root.join("song.wav"), b"RIFF1234WAVE").unwrap();
        assert_eq!(
            resolve_library_track(&root, std::path::Path::new("song.wav")).unwrap(),
            root.canonicalize().unwrap().join("song.wav")
        );
        assert!(resolve_library_track(&root, std::path::Path::new("../song.wav")).is_err());
        assert!(resolve_library_track(&root, std::path::Path::new("missing.wav")).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    fn state_call(
        base: &std::path::Path,
        category: &str,
        root: &std::path::Path,
        operation: &str,
        arguments: &[&str],
    ) -> Result<String> {
        let arguments = arguments
            .iter()
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>();
        state_command(base, category, root, operation, &arguments)
    }

    #[test]
    fn queue_and_named_playlist_state_persist_restart_and_expand_playlist_selection_only() {
        let root = temp_root("state-workflow-root");
        let base = temp_root("state-workflow-base").join("app-support");
        fs::write(root.join("one.flac"), b"fLaC").unwrap();

        let empty = state_call(&base, "queue", &root, "list", &[]).unwrap();
        assert_eq!(
            empty,
            "meta\trepeat\toff\nmeta\tshuffle\t-\nmeta\tcurrent\t-\n"
        );
        state_call(&base, "playlist", &root, "create", &["Café set"]).unwrap();
        state_call(
            &base,
            "playlist",
            &root,
            "add",
            &["Café set", "Album/one.flac"],
        )
        .unwrap();
        state_call(
            &base,
            "playlist",
            &root,
            "add",
            &["Café set", "Album/two.wav"],
        )
        .unwrap();

        let playlist = state_call(&base, "playlist", &root, "list", &[]).unwrap();
        assert_eq!(
            playlist,
            "playlist\t436166c3a920736574\t2\nentry\t436166c3a920736574\t0\t416c62756d2f6f6e652e666c6163\nentry\t436166c3a920736574\t1\t416c62756d2f74776f2e776176\n"
        );
        let queue = state_call(&base, "queue", &root, "add-playlist", &["Café set"]).unwrap();
        assert_eq!(
            queue,
            "meta\trepeat\toff\nmeta\tshuffle\t-\nmeta\tcurrent\t-\nentry\t0\t0\t416c62756d2f6f6e652e666c6163\nentry\t1\t0\t416c62756d2f74776f2e776176\n"
        );
        let repeated = state_call(&base, "queue", &root, "repeat", &["all"]).unwrap();
        assert!(repeated.contains("meta\trepeat\tall\n"));
        let shuffled = state_call(&base, "queue", &root, "shuffle", &["12345"]).unwrap();
        assert!(shuffled.contains("meta\tshuffle\t12345\n"));
        let unshuffled = state_call(&base, "queue", &root, "shuffle-off", &[]).unwrap();
        assert!(unshuffled.contains("meta\tshuffle\t-\n"));
        let selected = state_call(&base, "queue", &root, "next", &[]).unwrap();
        assert!(selected.contains("meta\tcurrent\t0\n"));
        assert!(selected
            .lines()
            .any(|line| line.starts_with("entry\t0\t1\t")));
        assert!(!selected.contains(root.to_str().unwrap()));

        let listed = state_call(&base, "queue", &root, "list", &[]).unwrap();
        assert!(selected.starts_with(&listed));
        assert!(!listed.contains("candidate-available"));
        assert_eq!(
            state_call(&base, "playlist", &root, "list", &[]).unwrap(),
            playlist
        );

        state_call(&base, "playlist", &root, "rename", &["Café set", "renamed"]).unwrap();
        assert!(state_call(&base, "playlist", &root, "list", &[])
            .unwrap()
            .starts_with("playlist\t72656e616d6564\t2\n"));
        state_call(&base, "playlist", &root, "remove", &["renamed", "0"]).unwrap();
        assert!(state_call(&base, "playlist", &root, "delete", &["renamed"]).is_ok());
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(base.parent().unwrap()).unwrap();
    }

    #[test]
    fn queue_navigation_reports_repeat_boundaries_and_seeded_order() {
        let root = temp_root("queue-automatic-advance-root");
        let base = temp_root("queue-automatic-advance-base");
        for path in ["one.flac", "two.wav", "three.aiff"] {
            state_call(&base, "queue", &root, "add", &[path]).unwrap();
        }

        // At Repeat Off's end there is no new candidate. Keep the current
        // entry selected for display and report no automatic restart.
        state_call(&base, "queue", &root, "repeat", &["off"]).unwrap();
        state_call(&base, "queue", &root, "select", &["2"]).unwrap();
        let ended = state_call(&base, "queue", &root, "next", &[]).unwrap();
        assert!(ended.contains("meta\tcandidate-available\tno\n"));
        assert!(ended.contains("meta\tcurrent\t2\n"));
        assert!(ended.lines().any(|line| line.starts_with("entry\t2\t1\t")));

        // Repeat Off's first-item Previous boundary is also a no-op rather
        // than a request to restart the already playing selection.
        state_call(&base, "queue", &root, "select", &["0"]).unwrap();
        let previous_boundary = state_call(&base, "queue", &root, "previous", &[]).unwrap();
        assert!(previous_boundary.contains("meta\tcandidate-available\tno\n"));
        assert!(previous_boundary.contains("meta\tcurrent\t0\n"));
        state_call(&base, "queue", &root, "select", &["2"]).unwrap();

        // Repeat One does produce a candidate, but it is the same item.
        state_call(&base, "queue", &root, "repeat", &["one"]).unwrap();
        let repeated_one = state_call(&base, "queue", &root, "next", &[]).unwrap();
        assert!(repeated_one.contains("meta\tcandidate-available\tyes\n"));
        assert!(repeated_one.contains("meta\tcurrent\t2\n"));

        // Repeat All wraps to the first item.
        state_call(&base, "queue", &root, "repeat", &["all"]).unwrap();
        let repeated_all = state_call(&base, "queue", &root, "next", &[]).unwrap();
        assert!(repeated_all.contains("meta\tcandidate-available\tyes\n"));
        assert!(repeated_all.contains("meta\tcurrent\t0\n"));

        // A fixed shuffle seed produces an explicit order; advancing from its
        // final entry wraps to the first item in that saved order.
        let shuffled = state_call(&base, "queue", &root, "shuffle", &["12345"]).unwrap();
        let paths = shuffled
            .lines()
            .filter_map(|line| {
                let mut fields = line.split('\t');
                if fields.next() != Some("entry") {
                    return None;
                }
                let index = fields.next()?.parse::<usize>().ok()?;
                let _selected = fields.next()?;
                let path = fields.next()?;
                Some((index, path.to_owned()))
            })
            .collect::<Vec<_>>();
        assert_eq!(paths.len(), 3);
        let final_index = paths.len() - 1;
        state_call(&base, "queue", &root, "select", &[&final_index.to_string()]).unwrap();
        let shuffled_next = state_call(&base, "queue", &root, "next", &[]).unwrap();
        assert!(shuffled_next.contains("meta\tcandidate-available\tyes\n"));
        assert!(shuffled_next.contains("meta\tcurrent\t0\n"));
        assert!(shuffled_next.lines().any(|line| {
            let mut fields = line.split('\t');
            fields.next() == Some("entry")
                && fields.next() == Some("0")
                && fields.next() == Some("1")
                && fields.next() == Some(paths[0].1.as_str())
        }));

        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn state_files_are_root_isolated_and_corruption_is_not_overwritten() {
        let root_a = temp_root("state-root-a");
        let root_b = temp_root("state-root-b");
        let base = temp_root("state-isolation-base");
        state_call(&base, "queue", &root_a, "add", &["a.flac"]).unwrap();
        let queue_a_path = state_file_path(&root_a, &base, "rmq").unwrap();
        let queue_b_path = state_file_path(&root_b, &base, "rmq").unwrap();
        assert_ne!(queue_a_path, queue_b_path);
        assert!(!queue_a_path
            .to_string_lossy()
            .contains(root_a.to_str().unwrap()));
        assert_eq!(
            state_call(&base, "queue", &root_b, "list", &[]).unwrap(),
            "meta\trepeat\toff\nmeta\tshuffle\t-\nmeta\tcurrent\t-\n"
        );

        fs::write(&queue_a_path, b"not a queue").unwrap();
        assert!(state_call(&base, "queue", &root_a, "list", &[]).is_err());
        assert_eq!(fs::read(&queue_a_path).unwrap(), b"not a queue");
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let linked_base = base.with_extension("symlink");
            symlink(&base, &linked_base).unwrap();
            assert!(state_call(&linked_base, "queue", &root_b, "list", &[]).is_err());
            fs::remove_file(linked_base).unwrap();
        }
        fs::remove_dir_all(root_a).unwrap();
        fs::remove_dir_all(root_b).unwrap();
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn state_commands_reject_traversal_unsupported_files_and_bad_indices() {
        let root = temp_root("state-bounds-root");
        let base = temp_root("state-bounds-base");
        assert!(state_call(&base, "queue", &root, "add", &["../outside.flac"]).is_err());
        assert!(state_call(&base, "queue", &root, "add", &["bad.mp3"]).is_err());
        assert!(state_call(&base, "queue", &root, "remove", &["0"]).is_err());
        state_call(&base, "playlist", &root, "create", &["safe"]).unwrap();
        assert!(state_call(&base, "playlist", &root, "add", &["safe", "../outside.wav"]).is_err());
        assert!(state_call(&base, "playlist", &root, "remove", &["safe", "42949672960"]).is_err());
        assert!(state_call(&base, "playlist", &root, "create", &["\t"]).is_err());
        assert!(state_call(&base, "other", &root, "list", &[]).is_err());
        let playlist = state_call(&base, "playlist", &root, "list", &[]).unwrap();
        assert!(playlist.starts_with("playlist\t73616665\t0\n"));
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn adding_playlist_is_atomic_when_queue_limit_would_be_exceeded() {
        use rust_music_library::{
            playlist_store::PlaylistStore,
            queue::{PlaybackQueue, MAX_QUEUE_ENTRIES},
        };

        let root = temp_root("state-queue-overflow-root");
        let base = temp_root("state-queue-overflow-base");
        let playlist_path = state_file_path(&root, &base, "rmp").unwrap();
        let mut playlists = PlaylistStore::new();
        playlists.create("two").unwrap();
        playlists.push_entry("two", "one.flac".into()).unwrap();
        playlists.push_entry("two", "two.flac".into()).unwrap();
        ensure_state_base(&base).unwrap();
        playlists.save_atomic(&playlist_path).unwrap();

        let queue_path = state_file_path(&root, &base, "rmq").unwrap();
        let mut queue = PlaybackQueue::new();
        for _ in 0..MAX_QUEUE_ENTRIES - 1 {
            queue.enqueue("existing.wav".into()).unwrap();
        }
        queue.save_atomic(&queue_path).unwrap();
        assert!(state_call(&base, "queue", &root, "add-playlist", &["two"]).is_err());
        let restored = PlaybackQueue::load_from(&queue_path).unwrap();
        assert_eq!(restored, queue);
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(base).unwrap();
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
    fn package_stream_runs_play_pause_resume_volume_stop_end_to_end() {
        let root = temp_root("package-transport-stream");
        let format = AudioFormat::new(44_100, 2, 16).unwrap();
        let mut wave = header(format, 8);
        wave.extend_from_slice(&[0; 8]);
        fs::write(root.join("track.wav"), wave).unwrap();

        let id = rust_music_host::MUSIC_PACKAGE_ID;
        let requests = [
            Request::Hello {
                min_version: PROTOCOL_VERSION,
                max_version: PROTOCOL_VERSION,
            },
            Request::ScanLibrary {
                root: root.to_string_lossy().into_owned(),
            },
            Request::Play,
            Request::Pause,
            Request::ResumePlayback,
            Request::SetVolumePercent { percent: 25 },
            Request::Stop,
            Request::Shutdown,
        ];
        let request_count = requests.len();
        let mut input = Vec::new();
        for (offset, body) in requests.into_iter().enumerate() {
            let request = Envelope {
                package_id: id.into(),
                request_id: offset as u64 + 1,
                body,
            };
            write_frame(&mut input, &request.encode().unwrap()).unwrap();
        }
        let mut output = Vec::new();
        serve_stream::<NullSink, _, _, _>(
            || Ok(NullSink::default()),
            std::io::Cursor::new(input),
            &mut output,
        )
        .unwrap();

        let mut frames = output.as_slice();
        let replies = (0..request_count)
            .map(|_| {
                let payload = read_frame(&mut frames).unwrap().unwrap();
                Envelope::<Reply>::decode(&payload).unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            replies[0].body,
            Reply::Hello {
                selected_version: PROTOCOL_VERSION
            }
        );
        let playback = replies[1..7]
            .iter()
            .map(|reply| match &reply.body {
                Reply::View(view) => view.playback,
                other => panic!("expected playback view, got {other:?}"),
            })
            .collect::<Vec<_>>();
        assert_eq!(
            playback,
            [
                rust_music_ipc::Playback::Stopped,
                rust_music_ipc::Playback::Playing,
                rust_music_ipc::Playback::Paused,
                rust_music_ipc::Playback::Playing,
                rust_music_ipc::Playback::Playing,
                rust_music_ipc::Playback::Stopped,
            ]
        );
        assert_eq!(replies[7].body, Reply::Ack);
        assert_eq!(read_frame(&mut frames).unwrap(), None);
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(any(
        all(target_os = "macos", feature = "macos-output"),
        all(target_os = "linux", feature = "alsa-output")
    ))]
    #[test]
    fn desktop_ipc_scan_accepts_twenty_thousand_while_host_scan_stays_small() {
        let root = temp_root("ipc-track-bound");
        for index in 0..2_049 {
            fs::write(root.join(format!("track-{index:04}.wav")), b"RIFF1234WAVE").unwrap();
        }
        let focus = ProcessFocus {
            grant: Arc::new(AtomicBool::new(false)),
        };
        let mut host_slot = MusicSlot::new(NullSink::default(), Library::empty());
        let request = |request_id, body| Envelope {
            package_id: rust_music_host::MUSIC_PACKAGE_ID.into(),
            request_id,
            body,
        };
        let scan_request = || Request::ScanLibrary {
            root: root.to_string_lossy().into_owned(),
        };

        let host_reply = handle_unit_request(&mut host_slot, &focus, request(1, scan_request()));
        assert!(matches!(host_reply, Reply::Error { .. }));

        let mut desktop_slot = MusicSlot::new(NullSink::default(), Library::empty());
        let desktop_reply = handle_unit_request_with_limits(
            &mut desktop_slot,
            &focus,
            request(2, scan_request()),
            STANDALONE_SCAN_LIMITS,
        );
        let Reply::View(view) = desktop_reply else {
            panic!("desktop scan should accept its 20k bound");
        };
        assert_eq!(view.track_count, 2_049);

        let selected = handle_unit_request_with_limits(
            &mut desktop_slot,
            &focus,
            request(3, Request::Select { index: 2_048 }),
            STANDALONE_SCAN_LIMITS,
        );
        let Reply::View(view) = selected else {
            panic!("desktop should select the last host-addressable track");
        };
        assert_eq!(view.selected, Some(2_048));
        assert!(!view.audio_claimed);
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(any(
        all(target_os = "macos", feature = "macos-output"),
        all(target_os = "linux", feature = "alsa-output")
    ))]
    #[test]
    fn package_transport_pause_resume_volume_and_stop_preserve_lease_contract() {
        let root = temp_root("transport-control");
        let format = AudioFormat::new(44_100, 2, 16).unwrap();
        let mut wave = header(format, 4);
        wave.extend_from_slice(&[1, 0, 2, 0]);
        fs::write(root.join("track.wav"), wave).unwrap();

        let focus = ProcessFocus {
            grant: Arc::new(AtomicBool::new(false)),
        };
        let mut slot = MusicSlot::new(NullSink::default(), Library::empty());
        let request = |request_id, body| Envelope {
            package_id: rust_music_host::MUSIC_PACKAGE_ID.into(),
            request_id,
            body,
        };
        let scanned = handle_unit_request(
            &mut slot,
            &focus,
            request(
                1,
                Request::ScanLibrary {
                    root: root.to_string_lossy().into_owned(),
                },
            ),
        );
        assert!(matches!(scanned, Reply::View(_)));
        let started = handle_unit_request(&mut slot, &focus, request(2, Request::Play));
        let Reply::View(started) = started else {
            panic!("play should return a view");
        };
        assert_eq!(started.playback, rust_music_ipc::Playback::Playing);
        assert!(started.audio_claimed);

        let paused = handle_unit_request(&mut slot, &focus, request(3, Request::Pause));
        let Reply::View(paused) = paused else {
            panic!("pause should return a view");
        };
        assert_eq!(paused.playback, rust_music_ipc::Playback::Paused);
        assert!(!paused.audio_claimed);

        let volume = handle_unit_request(
            &mut slot,
            &focus,
            request(4, Request::SetVolumePercent { percent: 50 }),
        );
        assert!(matches!(volume, Reply::View(_)));
        let resumed = handle_unit_request(&mut slot, &focus, request(5, Request::ResumePlayback));
        let Reply::View(resumed) = resumed else {
            panic!("resume should return a view");
        };
        assert_eq!(resumed.playback, rust_music_ipc::Playback::Playing);
        assert!(resumed.audio_claimed);

        let stopped = handle_unit_request(&mut slot, &focus, request(6, Request::Stop));
        let Reply::View(stopped) = stopped else {
            panic!("stop should return a view");
        };
        assert_eq!(stopped.playback, rust_music_ipc::Playback::Stopped);
        assert!(!stopped.audio_claimed);
        fs::remove_dir_all(root).unwrap();
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
