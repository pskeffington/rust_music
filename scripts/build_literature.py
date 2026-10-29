#!/usr/bin/env python3
"""Single-source, deterministic machine literature index for Rust Music."""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DOCS = ROOT / "docs"
UPDATED = "2026-10-29"
CATEGORIES = {
    "standards": "Codec, container, and native API standards",
    "reference-codecs": "Comparison decoders and conformance tools (never dependencies)",
    "reference-players": "Comparable music players and product architecture",
    "engineering": "Rust code quality, portability, and security",
    "host-integration": "Embedding boundary with Rust Boy and sibling modules",
}

# Evidence is a design reference; no listed third-party code is imported.
DATA = [
    ("RFC 9639: FLAC", "standards", "https://www.rfc-editor.org/rfc/rfc9639.html", "official-spec", "high", "Implement native FLAC frames, metadata, CRC, Rice, fixed and LPC, channel reconstruction.", "Decode/CRC tests and IETF corpus; verify PCM against reference.", "G2", "IETF standards text; write our own decoder"),
    ("IETF FLAC decoder testbench", "reference-codecs", "https://github.com/ietf-wg-cellar/flac-test-files", "official-test-corpus", "high", "Use subset/uncommon/faulty cases, including rejection and hang-resistance.", "Run testbench, compare PCM/STREAMINFO MD5 and record pass counts.", "G2", "CC0 test corpus; fixture provenance tracked"),
    ("Xiph FLAC reference", "reference-codecs", "https://github.com/xiph/flac", "reference-implementation", "high", "Differential oracle only; do not bundle or copy decoder code.", "Compare PCM and error behavior with pinned reference version.", "G2", "Reference libraries BSD-like; tools GPL; no code reuse"),
    ("Claxon pure Rust FLAC", "reference-codecs", "https://github.com/ruuda/claxon", "project-readme", "high", "Study how small a lossless Rust decoder can be; compare performance methodology.", "Benchmark decoded samples/s on same hardware with a documented corpus.", "G2", "Apache-2.0; architecture only; do not copy"),
    ("Symphonia audio", "reference-codecs", "https://github.com/pdeljanov/Symphonia", "project-readme", "medium", "Feature-controlled decoder comparison; explicitly excluded from runtime.", "No Symphonia dependency in cargo metadata or lockfile.", "G1", "Open source; design reference, not a dependency"),
    ("Music Player Daemon", "reference-players", "https://github.com/MusicPlayerDaemon/MPD", "project-readme", "high", "Separate codec, music database, control, and physical outputs.", "Each module builds without UI or native audio libs.", "G1", "GPL project; do not transplant code"),
    ("Termusic", "reference-players", "https://github.com/tramhao/termusic", "project-readme", "medium", "Study keyboard-first playlists and compact status, not dependency surface.", "Micro controls tested at small display sizes.", "G5", "Mixed MIT/GPL code; behavior-only study"),
    ("Firefly TUI music player", "reference-players", "https://github.com/ly-ns0/firefly", "project-readme", "medium", "Contrast compact transport controls and library paths.", "Compare startup/memory with equivalent local file library.", "G5", "Review upstream license; no code reuse"),
    ("Apple ALAC reference and cookie", "standards", "https://github.com/macosforge/alac", "official-project", "high", "Design independent ALAC prediction and CAF/MP4 demux; do not link or copy the reference.", "Differential ALAC PCM golden vectors and malformed MP4 atoms.", "G4", "Apache-2.0 reference; independent implementation planned"),
    ("Apple AudioQueueNewOutput", "standards", "https://developer.apple.com/documentation/audiotoolbox/audioqueuenewoutput", "official-api", "high", "Thin native PCM output ABI; decode on worker, not playback callback.", "macOS signed PCM format negotiation, device swap and drain tests.", "G3", "OS API; adapter code owned by Rust Music"),
    ("Windows WASAPI", "standards", "https://learn.microsoft.com/en-us/windows/win32/coreaudio/wasapi", "official-api", "high", "Thin Windows audio endpoint adapter with no resampling surprises.", "WASAPI shared/exclusive bit-match acceptance on device.", "G3", "OS API; adapter planned"),
    ("WASAPI exclusive streams", "standards", "https://learn.microsoft.com/en-us/windows/win32/coreaudio/exclusive-mode-streams", "official-api", "medium", "Prefer capability negotiation, warn before exclusive device seizure.", "Windows format capability and system-audio behavior tests.", "G3", "Official platform documentation"),
    ("Android NDK AAudio", "standards", "https://developer.android.com/ndk/guides/audio/aaudio/aaudio", "official-api", "high", "Thin Android output ABI for deployable micro player.", "Actual Android 26+ format, focus, standby and output tests.", "G5", "Android native API; adapter planned"),
    ("MPD ALSA output guidance", "engineering", "https://mpd.readthedocs.io/en/stable/plugins.html", "official-project", "high", "Avoid implicit dmix/resample for strict audio; request direct hw device explicitly.", "Linux ALSA direct-device format/no-resample field verification.", "G3", "Documentation/behavior reference"),
    ("Rust API Guidelines", "engineering", "https://rust-lang.github.io/api-guidelines/", "official-guide", "high", "Typed errors and portable Read-driven decoders with explicit no-panics.", "Fmt/clippy/test plus public rustdoc review.", "G0", "Guidance, not a runtime dependency"),
    ("Rust cargo features", "engineering", "https://doc.rust-lang.org/cargo/reference/features.html", "official-guide", "medium", "Keep optional OS backends gated and dependency-free core default.", "Cargo tree and MSRV target matrices.", "G0", "Official Rust documentation"),
    ("Rust Radio standalone embedding", "host-integration", "https://github.com/pskeffington/rust_radio/blob/main/docs/EMBEDDING.md", "host-contract", "high", "Reuse module shape: independent lib, explicit Play, host-owned storage and output focus.", "Rust Boy adapter can load and unload without second audio stream.", "G5", "Sibling repository interface reference only"),
    ("Linux ALSA kernel PCM UAPI", "standards", "https://github.com/torvalds/linux/blob/master/include/uapi/sound/asound.h", "official-spec", "high", "Use local Rust ioctl structs against /dev/snd device nodes, not the libasound sound library or a daemon.", "64-bit ABI layout tests plus playback hardware negotiation and underrun testing.", "G3", "Linux kernel interface, not a bundled decoder or userspace sound SDK"),
    ("Cargo offline and locked builds", "engineering", "https://doc.rust-lang.org/cargo/reference/config.html#netoffline", "official-guide", "high", "Require offline locked workspace builds with no registry/git packages and no build.rs hooks.", "Run the built-in Rust audit and offline build gate from disconnected workstation.", "G0", "Rust toolchain itself is an explicit prerequisite; no dependencies are downloaded"),
]

STANDARD_DATA = [
    ("Independent codecs", "No runtime decoder deps: do not link FLAC/ALAC, Symphonia, Claxon, FFmpeg, or libFLAC.", "G0", "implemented", [1, 3, 4, 5], "cargo metadata proves decoder dependency closure is internal only"),
    ("Integer PCM", "Preserve integer samples and explicit sample format; do not silently apply floats, DSP, gain or resampling.", "G1", "implemented", [1, 14], "golden integer PCM bit patterns match source"),
    ("Bounded parsing", "Bound metadata, block sizes, allocation, Rice unary and file chunk scanning.", "G2", "implemented", [1, 2], "malformed and oversize inputs fail with typed errors"),
    ("FLAC CRC", "Check frame header CRC8 and full-frame CRC16 before writing PCM.", "G2", "implemented", [1, 2], "corruption test rejects changed CRC"),
    ("FLAC MD5", "Verify decoded PCM STREAMINFO MD5 across the complete file; distinguish zero digest.", "G2", "implemented", [1, 2, 3], "unit checksum vectors plus full-file reference PCM/MD5 parity on GitHub Actions"),
    ("Explicit audio start", "Constructors, metadata scanning and module registration never autoplay or open output devices.", "G1", "implemented", [6, 17], "player state tests: Ready does not call sink.open"),
    ("Single output focus", "Rust Boy coordinates active TV, RADIO and MUSIC audio ownership; one active sink at a time.", "G5", "planned", [6, 17], "integration tests demonstrate only one device owner"),
    ("Native output negotiation", "Do not label an output bit-perfect until hardware format/rate/channel path is verified.", "G3", "planned", [10, 11, 12, 14], "record exact device format and loopback sample comparison"),
    ("No callback allocations", "Audio callback is nonblocking, allocation-free, and free of filesystem work.", "G3", "planned", [10, 13], "stress callback and underrun/fault insertion"),
    ("No unexpected IO", "Track selection and indexing never trigger network calls or scanning unrelated roots.", "G5", "planned", [6, 17], "air-gapped tests and bounded explicit library scan"),
    ("Corpus differential", "Test valid/uncommon/faulty FLAC and compare output to pinned reference.", "G2", "planned", [2, 3], "corpus report with SHA, exact fixture counts and outcomes"),
    ("Portability", "Pure headless core compiles independent of macOS/Windows/Linux/Android OS backends.", "G0", "implemented", [10, 11, 13, 16], "cross-target no-default-feature check"),
    ("Machine index", "Every source and standard is uniquely identified with evidence and validation gates.", "G0", "implemented", [15], "scripts/check_literature.py succeeds offline"),
    ("ALAC truthfulness", "Do not mark ALAC as supported until demux and decoder pass corpus comparison.", "G4", "planned", [9], "ALAC source/PCM signed comparison with golden tests"),
    ("Local-only dependency closure", "Every Cargo dependency stays inside this workspace; reject crates.io, git, build.rs and undeclared native libraries.", "G0", "implemented", [16, 19], "cargo run --locked --offline -p rust-music-audit proves manifest, lock and FFI boundaries"),
    ("Direct Linux kernel PCM", "Do not link libasound, PulseAudio, JACK or PipeWire; Linux PCM sink must use only a /dev/snd kernel device.", "G3", "implemented", [14, 18], "Feature-gated native ABI size checks, real PCM device setup and DAC loopback evidence"),
]

def records():
    sources = []
    for i, (title, category, url, evidence, priority, decision, validation, gate, license_note) in enumerate(DATA, 1):
        sources.append(dict(id=f"LIT-{i:03d}", title=title, category=category, url=url,
            evidence_level=evidence, priority=priority, decision=decision,
            validation=validation, gate=gate, license_note=license_note,
            dependency_policy="reference-only"))
    standards = []
    for i, (title, rule, gate, status, evidence, acceptance) in enumerate(STANDARD_DATA, 1):
        standards.append(dict(id=f"RUST-{i:03d}", title=title, rule=rule, gate=gate,
            status=status, evidence_ids=[f"LIT-{n:03d}" for n in evidence], acceptance=acceptance))
    return dict(schema_version=1, updated=UPDATED,
                canonical_matrix="docs/LITERATURE_MATRIX.md", canonical_index="docs/LITERATURE_INDEX.md",
                categories=CATEGORIES, sources=sources, standards=standards)

def render(matrix):
    intro = "# Rust Music independent-codec literature matrix\n\nUpdated: " + UPDATED + "\n\n"
    intro += "All sources are research references **only**. No third-party decoder/player implementation is copied or linked.\n"
    intro += "Project claims are not a hardware benchmark; status is recorded in the registry.\n\n"
    intro += "[Machine-readable registry](literature-index.json) | [Navigation](LITERATURE_INDEX.md) | [Architecture](ARCHITECTURE.md) | [Roadmap index](roadmap-index.json)\n\n"
    rows = [intro, "## Engineering transfer matrix\n\n", "| ID | Source | Decision | Validation | Gate | License boundary |\n",
            "| --- | --- | --- | --- | --- | --- |\n"]
    for s in matrix["sources"]:
        cells = [f'**{s["id"]}**', f'[{s["title"]}]({s["url"]})', s["decision"], s["validation"], s["gate"], s["license_note"]]
        rows.append("| " + " | ".join(cell.replace("|", "\\|") for cell in cells) + " |\n")
    rows += ["\n## Independent Rust coding standards\n\n", "| ID | Rule | Gate | Implementation status | Evidence | Acceptance |\n",
             "| --- | --- | --- | --- | --- | --- |\n"]
    for s in matrix["standards"]:
        cells = [f'**{s["id"]}**', s["rule"], s["gate"], s["status"], ", ".join(s["evidence_ids"]), s["acceptance"]]
        rows.append("| " + " | ".join(cells) + " |\n")
    index = ["# Rust Music literature navigation\n\n", f"Updated: {UPDATED}. See [matrix](LITERATURE_MATRIX.md), [JSON index](literature-index.json) and [roadmap routing](roadmap-index.json).\n\n"]
    for key, name in matrix["categories"].items():
        index.append(f"## {name}\n\n")
        for s in matrix["sources"]:
            if s["category"] == key:
                index.append(f'- **{s["id"]}** [{s["title"]}]({s["url"]}) - {s["decision"]} Gate {s["gate"]}.\n')
        index.append("\n")
    return "".join(rows), "".join(index)

def main():
    data = records()
    DOCS.mkdir(exist_ok=True)
    (DOCS / "literature-index.json").write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")
    matrix, index = render(data)
    (DOCS / "LITERATURE_MATRIX.md").write_text(matrix, encoding="utf-8")
    (DOCS / "LITERATURE_INDEX.md").write_text(index, encoding="utf-8")
    print(f"Generated {len(data['sources'])} literature records and {len(data['standards'])} engineering rules")

if __name__ == "__main__": main()
