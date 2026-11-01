use std::{
    fs,
    path::PathBuf,
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

struct TemporaryRoots(PathBuf);

impl TemporaryRoots {
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after Unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "rust-music-state-cli-batch-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(path.join("library")).expect("create temporary library");
        fs::create_dir_all(path.join("home")).expect("create temporary home");
        Self(path)
    }

    fn library(&self) -> PathBuf {
        self.0.join("library")
    }

    fn home(&self) -> PathBuf {
        self.0.join("home")
    }
}

impl Drop for TemporaryRoots {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn playlist_add_many_accepts_multiple_cli_path_arguments() {
    let roots = TemporaryRoots::new();
    let output = Command::new(env!("CARGO_BIN_EXE_rust-music-cli"))
        .env("HOME", roots.home())
        .env_remove("XDG_DATA_HOME")
        .args([
            "state",
            "playlist",
            roots
                .library()
                .to_str()
                .expect("temporary library path is UTF-8"),
            "create",
            "UI Batch",
        ])
        .output()
        .expect("create playlist through the CLI process");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let output = Command::new(env!("CARGO_BIN_EXE_rust-music-cli"))
        .env("HOME", roots.home())
        .env_remove("XDG_DATA_HOME")
        .args([
            "state",
            "playlist",
            roots
                .library()
                .to_str()
                .expect("temporary library path is UTF-8"),
            "add-many",
            "UI Batch",
            "one.flac",
            "two.wav",
            "three.aiff",
        ])
        .output()
        .expect("add multiple paths through the CLI process");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("CLI state output is UTF-8");
    assert!(stdout.contains("playlist\t5549204261746368\t3\n"));
    assert!(stdout.contains("entry\t5549204261746368\t2\t74687265652e61696666\n"));
}
