//! Offline dependency and native link audit written in Rust std only.
//! No TOML, shell, network, registry, Git, or system audio library is needed.
use std::{collections::{BTreeMap, BTreeSet}, env, fs, path::{Path, PathBuf}};

#[derive(Debug, Default)]
struct Audit {
    crates: BTreeMap<String, PathBuf>,
    dependencies: Vec<(String, String, PathBuf)>,
    rust_files: usize,
    allowed_os_links: usize,
}

fn read(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}
fn requirement(ok: bool, context: impl Into<String>) -> Result<(), String> {
    if ok { Ok(()) } else { Err(context.into()) }
}

// Strictly allow in-workspace dependencies of the form:
// rust-music-x = { path = "../rust-music-x" [, optional = true] }
fn parse_dependency(line: &str) -> Result<(&str, &str), String> {
    let (name, body) = line.split_once('=').ok_or(format!("invalid dependency: {line}"))?;
    let name = name.trim();
    let body = body.trim();
    requirement((name == "rust-music" || name.starts_with("rust-music-")), format!("external crate name: {name}"))?;
    requirement(body.starts_with('{') && body.ends_with('}'), format!("non-local dependency: {line}"))?;
    let inside = body[1..body.len()-1].trim();
    let mut path = None;
    for field in inside.split(',') {
        let (key, val) = field.trim().split_once('=').ok_or(format!("invalid dependency field: {line}"))?;
        let key = key.trim();
        let val = val.trim();
        match key {
            "path" => {
                requirement(path.is_none() && val.starts_with('"') && val.ends_with('"'), format!("invalid local path: {line}"))?;
                path = Some(&val[1..val.len()-1]);
            }
            "optional" => requirement(val == "true" || val == "false", format!("invalid optional flag: {line}"))?,
            _ => return Err(format!("prohibited dependency field {key}: {line}")),
        }
    }
    let path = path.ok_or(format!("missing local path: {line}"))?;
    requirement(!path.is_empty() && !path.starts_with('/'), format!("unsafe path: {line}"))?;
    Ok((name, path))
}

fn parse_package_manifest(root: &Path, path: &Path, report: &mut Audit) -> Result<(), String> {
    let content = read(&path.join("Cargo.toml"))?;
    let mut section = String::new();
    let mut name = None;
    let mut found_dependencies = Vec::new();
    for raw in content.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() { continue; }
        if line.starts_with('[') {
            section = line.to_owned();
            requirement(!section.contains("[patch") && !section.contains("[replace")
                && !section.contains("[workspace.dependencies"),
                format!("unexpected registry/source override in {}", path.display()))?;
            // Nested dependency tables can hide registry or version sources;
            // disallow them instead of attempting a partial TOML parser.
            requirement(!section.contains("dependencies."), format!("nested dependency table in {}", path.display()))?;
            continue;
        }
        if section == "[package]" {
            if let Some(rest) = line.strip_prefix("name = ") {
                name = Some(rest.trim_matches('"').to_string());
            }
            requirement(!line.starts_with("build =") && !line.starts_with("links ="),
                        format!("build hook or native link declared in {}", path.display()))?;
        }
        if matches!(section.as_str(), "[dependencies]" | "[dev-dependencies]" | "[build-dependencies]") ||
           (section.starts_with("[target.") && section.contains(".dependencies")) {
            let (dep, rel) = parse_dependency(line)?;
            let destination = path.join(rel).canonicalize().map_err(|e| format!("invalid dependency path {rel}: {e}"))?;
            requirement(destination.starts_with(root), format!("path escapes workspace: {rel}"))?;
            requirement(destination.join("Cargo.toml").is_file(), format!("missing Cargo.toml at {rel}"))?;
            found_dependencies.push((dep.to_owned(), destination));
        }
    }
    let name = name.ok_or(format!("package name missing in {}", path.display()))?;
    requirement((name == "rust-music" || name.starts_with("rust-music-")), format!("unexpected local package name: {name}"))?;
    requirement(report.crates.insert(name.clone(), path.to_owned()).is_none(), format!("duplicate package {name}"))?;
    for (dep, dest) in found_dependencies {
        report.dependencies.push((name.clone(), dep, dest));
    }
    Ok(())
}

fn validate_lock(root: &Path, crates: &BTreeMap<String, PathBuf>) -> Result<(), String> {
    let lock = root.join("Cargo.lock");
    requirement(lock.is_file(), "Cargo.lock missing: commit a locally generated offline lockfile")?;
    let content = read(&lock)?;
    let mut package_names = BTreeSet::new();
    for raw in content.lines() {
        let line = raw.trim();
        requirement(!line.starts_with("source =") && !line.starts_with("checksum ="),
                    format!("non-local package in Cargo.lock: {line}"))?;
        if let Some(name) = line.strip_prefix("name = ") {
            package_names.insert(name.trim_matches('"').to_string());
        }
    }
    let expected = crates.keys().cloned().collect::<BTreeSet<_>>();
    requirement(package_names == expected,
                format!("Cargo.lock packages {:?} disagree with workspace {:?}",package_names, expected))
}

fn scan_rust(path: &Path, report: &mut Audit) -> Result<(), String> {
    for entry in fs::read_dir(path).map_err(|e| format!("{}: {e}", path.display()))? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_dir() { scan_rust(&path, report)?; continue; }
        requirement(path.file_name().and_then(|v| v.to_str()) != Some("build.rs"),
                    format!("build.rs not permitted: {}", path.display()))?;
        if let Some(extension) = path.extension().and_then(|v| v.to_str()) {
            requirement(!["so", "dylib", "dll", "a", "lib", "o", "exe", "wasm"].contains(&extension),
                        format!("prebuilt native binary forbidden: {}", path.display()))?;
        }
        if path.extension().and_then(|v| v.to_str()) != Some("rs") { continue; }
        report.rust_files += 1;
        let content = read(&path)?;
        let is_macos = path.to_string_lossy().ends_with("rust-music-native/src/macos.rs");
        let is_linux = path.to_string_lossy().ends_with("rust-music-native/src/alsa.rs");
        for (index, raw) in content.lines().enumerate() {
            let line = raw.trim();
            if line.starts_with("//") { continue; }
            let loc = format!("{}:{}", path.display(), index + 1);
            let code_only = line.split('"').step_by(2).collect::<String>();
            for forbidden in ["std::net::", "TcpStream", "UdpSocket", "reqwest::", "Command::new(",
                              "include_bytes!(\"http", "include_str!(\"http", "dlopen(",
                              "libloading::", "snd_pcm_open(", "asound"] {
                requirement(!code_only.contains(forbidden), format!("forbidden external/network use {forbidden}: {loc}"))?;
            }
            if line.starts_with("#[link(") {
                if is_macos && line == "#[link(name = \"AudioToolbox\", kind = \"framework\")]" {
                    report.allowed_os_links += 1;
                } else { return Err(format!("unapproved system library link: {loc}: {line}")); }
            }
            if line.contains("extern \"C\"") {
                requirement(is_macos || is_linux, format!("unapproved native FFI boundary: {loc}"))?;
            }
        }
    }
    Ok(())
}

fn audit(root: &Path) -> Result<Audit, String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let config = read(&root.join(".cargo/config.toml"))?;
    requirement(config.lines().any(|line| line.trim() == "offline = true"),
                "Cargo offline default missing from .cargo/config.toml")?;
    requirement(!config.contains("replace-with") && !config.contains("registries.")
                && !config.contains("git-fetch-with-cli"),
                "Cargo source/network override is forbidden")?;
    let manifest = read(&root.join("Cargo.toml"))?;
    requirement(manifest.contains("resolver = \"2\""), "resolver 2 required")?;
    for raw in manifest.lines() {
        let line = raw.trim();
        requirement(!line.starts_with("[workspace.dependencies")
                 && !line.starts_with("[patch")
                 && !line.starts_with("[replace")
                 && !line.starts_with("[source")
                 && !line.starts_with("[target."),
                 format!("unexpected workspace dependency/source override: {line}"))?;
    }
    let mut members = BTreeSet::new();
    let mut in_members = false;
    for raw in manifest.lines() {
        let line = raw.trim();
        if line == "members = [" { in_members = true; continue; }
        if in_members {
            if line == "]" { in_members = false; continue; }
            let member = line.trim_end_matches(',').trim().trim_matches('"');
            requirement((member == "crates/rust-music" || member.starts_with("crates/rust-music-")), format!("invalid member: {member}"))?;
            members.insert(root.join(member).canonicalize().map_err(|e| e.to_string())?);
        }
    }
    requirement(!in_members && !members.is_empty(), "workspace members list missing")?;
    let mut report = Audit::default();
    for path in &members { parse_package_manifest(&root, path, &mut report)?; }
    // No extra crate can hide outside the declared workspace.
    for entry in fs::read_dir(root.join("crates")).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.is_dir() && path.join("Cargo.toml").exists() {
            requirement(members.contains(&path.canonicalize().map_err(|e| e.to_string())?),
                        format!("undeclared local crate: {}", path.display()))?;
        }
    }
    for (parent, dep, actual) in &report.dependencies {
        let intended = report.crates.get(dep).ok_or(format!("{parent}: dependency {dep} missing from workspace"))?;
        requirement(actual == intended, format!("{parent}: dependency {dep} path mismatches"))?;
    }
    validate_lock(&root, &report.crates)?;
    scan_rust(&root.join("crates"), &mut report)?;
    requirement(report.allowed_os_links == 1, "expected exactly one approved Apple system framework link")?;
    Ok(report)
}

fn main() {
    let root = env::args_os().nth(1).unwrap_or_else(|| ".".into());
    match audit(Path::new(&root)) {
        Ok(report) => println!("PASS: {} local crates; {} path-only internal edges; {} Rust sources; 0 registry/git crates; 0 third-party system links; {} OS framework link", report.crates.len(), report.dependencies.len(), report.rust_files, report.allowed_os_links),
        Err(error) => { eprintln!("FAIL: Rust Music offline dependency audit: {error}"); std::process::exit(1); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_registry_or_non_local_dependency_syntax() {
        assert_eq!(parse_dependency("rust-music-core = { path = \"../rust-music-core\" }").unwrap(),
                   ("rust-music-core", "../rust-music-core"));
        assert!(parse_dependency("serde = \"1\"").is_err());
        assert!(parse_dependency("rust-music-core = { git = \"https://example.test\" }").is_err());
        assert!(parse_dependency("rust-music-core = { path = \"../rust-music-core\", version = \"1\" }").is_err());
        assert!(parse_dependency("rust-music-core = { path = \"/tmp/evil\" }").is_err());
    }
}
