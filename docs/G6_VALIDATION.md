# G6 micro-panel acceptance ledger

Updated: 2026-10-29. **Source and static validation only**, not a cargo or
physical Rust_Boy certification.

| Gate | Expected | Local evidence | Status |
| --- | --- | --- | --- |
| P6-01 | Standalone zero-third-party `rust-music-panel` | Workspace TOML inspected; 11 internal crates, 19 local links, zero registry/Git packages | Static PASS |
| P6-02 | 128x64, exactly 1024 bytes | `MonoFrame` uses `[u8; WIDTH*HEIGHT/8]` and tests `size_of()` | Source implemented; Rust test pending |
| P6-03 | No autoplay, network, device, UI backend | Renderer uses only borrowed `PanelView`, local glyph bitmaps and a pure action map | Static PASS; Rust test pending |
| P6-04 | Input maps to explicit controls | `Previous/Next/Confirm/Stop/Settings` map to host commands or OpenSettings; tests included | Source implemented; Rust test pending |
| P6-05 | CLI preview without physical sound | `preview DIRECTORY OUTPUT.pbm` renders `NullSink`/P4 bitmap after explicit bounded scan | Source implemented; CLI test pending |
| P6-06 | Research index/regression fixture check | `scripts/check_literature.py` plus `test_flac_reference.py --fixtures-only` | PASS (19 sources, 16 rules; 7 reference FLAC cases) |
| P6-07 | Offline build, lint, tests, native output, hardware screen | cargo/rustc not installed in current environment; GitHub jobs previously `startup_failure` | BLOCKED / NOT VERIFIED |

The development preview created from the same locally authored glyph table
is a **visual layout QA artifact**, not the output of a compiled Rust binary.
No measured CPU/RSS or DAC/GUI hardware results are claimed.

## Next acceptance commands

```sh
sh scripts/offline-gate.sh
cargo run --locked --offline -p rust-music-cli -- preview "$HOME/Music" /tmp/music-panel.pbm
cargo test --locked --offline -p rust-music-panel
```

The shell must supply an actual Settings navigation handler, forward physical
buttons to `translate_button()`, call `MusicSlot::handle()` from the proper
worker, and verify exclusive focus with deployed Radio/TV on real devices.
