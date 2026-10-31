use rust_music_library::{scan, ScanLimits, TrackQuery, TrackSort};
use std::{
    fs,
    path::PathBuf,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

const SIZES: [usize; 5] = [0, 1, 100, 2_000, 20_000];
const SEARCH_SAMPLES: usize = 51;

struct TempRoot(PathBuf);

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let root_path =
        std::env::temp_dir().join(format!("rust-music-profile-{}-{stamp}", std::process::id()));
    fs::create_dir(&root_path)?;
    let root = TempRoot(root_path);

    for size in SIZES {
        let library_root = root.0.join(format!("tracks-{size}"));
        fs::create_dir(&library_root)?;
        let fixture_start = Instant::now();
        for index in 0..size {
            let path = library_root.join(format!("track-{index:05}.wav"));
            // The explicit scanner checks the RIFF/WAVE signature only; no
            // audio payload is generated, decoded, or played by this profile.
            fs::write(path, b"RIFF1234WAVE")?;
        }
        let fixture_time = fixture_start.elapsed();

        let build_start = Instant::now();
        let library = scan(
            &library_root,
            ScanLimits {
                max_entries: 100_000,
                max_tracks: 20_000,
                max_depth: 6,
            },
        )?;
        let build_time = build_start.elapsed();
        if library.len() != size {
            return Err(format!("expected {size} tracks, scanned {}", library.len()).into());
        }

        let mut search_times = Vec::with_capacity(SEARCH_SAMPLES);
        for _ in 0..SEARCH_SAMPLES {
            let start = Instant::now();
            let page = library.query(&TrackQuery {
                text: "track-".into(),
                sort_by: TrackSort::Title,
                limit: 50,
                ..TrackQuery::default()
            })?;
            search_times.push(start.elapsed());
            if page.total_matches != size {
                return Err(format!("expected {size} matches, got {}", page.total_matches).into());
            }
        }
        search_times.sort_unstable();
        let p50 = search_times[SEARCH_SAMPLES / 2];
        let p95_index = (SEARCH_SAMPLES * 95).div_ceil(100) - 1;
        let p95 = search_times[p95_index];
        println!(
            "tracks={size} fixture_ms={} catalog_build_ms={} search_p50_ns={} search_p95_ns={}",
            fixture_time.as_millis(),
            build_time.as_millis(),
            p50.as_nanos(),
            p95.as_nanos(),
        );
    }

    Ok(())
}
