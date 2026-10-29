# G6 micro-panel acceptance ledger

Updated: 2026-10-29. **Local Rust build and unit validation pass**; physical
Rust_Boy screen and control acceptance remain open.

| Gate | Expected | Local evidence | Status |
| --- | --- | --- | --- |
| P6-01 | Standalone zero-third-party `rust-music-panel` | Offline audit: 12 crates, 21 local path edges, zero registry/Git packages | PASS |
| P6-02 | 128x64, exactly 1024 bytes | `MonoFrame` uses `[u8; WIDTH*HEIGHT/8]`; workspace test checks size and bounds | PASS (local Rust test) |
| P6-03 | No autoplay, network, device, UI backend | Pure renderer uses borrowed `PanelView`, local glyphs and action mapping | PASS (local Rust test) |
| P6-04 | Input maps to explicit controls | Navigation maps to host commands; Settings requests host navigation | PASS (local Rust test) |
| P6-05 | CLI preview without physical sound | Local CLI rendered a synthetic WAV catalog to P4 PBM with exactly 1024 framebuffer bytes; no audio output opened | PASS (local execution) |
| P6-06 | Research index/regression fixture check | `scripts/check_literature.py` plus `test_flac_reference.py --fixtures-only` | PASS (19 sources, 16 rules; 7 reference FLAC cases) |
| P6-07 | Offline build, lint, tests, native output, hardware screen | Workspace gate and macOS feature check pass; no physical DAC or Rust_Boy display was connected/validated | PARTIAL; hardware NOT TESTED |

The development preview created from the same locally authored glyph table
is a **visual layout QA artifact**, not a physical Rust_Boy screen test. No
measured CPU/RSS or DAC/GUI hardware results are claimed.

## Next acceptance commands

```sh
sh scripts/offline-gate.sh
cargo run --locked --offline -p rust-music-cli -- preview "$HOME/Music" /tmp/music-panel.pbm
cargo test --locked --offline -p rust-music-panel
```

The shell must supply an actual Settings navigation handler, forward physical
buttons to `translate_button()`, call `MusicSlot::handle()` from the proper
worker, and verify exclusive focus with deployed Radio/TV on real devices.
