# AGENTS.md

A guide for coding agents working on T.O.R.E Hangar: a small, standalone Rust
editor for Fighters Anthology (FA) LIB archives and the resources inside them
(PT/NT/JT/OT/SEE/ECM definitions, SH shapes, PIC textures, PCM audio).
Read `README.md` for an overview, `docs/MANUAL.md` for user-facing features,
`docs/ARCHITECTURE.md` for format and writer decisions, and `tore-hangar-design/BRAND.md` before touching UI.

## Layout

```
crates/hangar-core/   Portable formats + editing history. #![no_std] + alloc.
crates/hangar-app/    Binary `tore-hangar`: shared UI, CLI, platform backends.
  src/ui.rs           App struct, events, Canvas/Draw commands, theme import.
  src/ui_view.rs      Layout, drawing and hit regions for every workspace.
  src/ui_*.rs         Feature slices as further `impl App` blocks (media/paint,
                      mesh, animation, materials, hardpoints, graft, libraries,
                      dependencies, clone/export, envelope, color, browser).
  src/windows.rs      Win32/GDI backend, no CRT, no_std (custom entry point).
  src/linux.rs        Xlib backend for local development.
  src/cli.rs          Linux-only CLI, smoke test and headless check commands.
  src/saving.rs       Shared staged-save/backup protocol.
tore-hangar-design/   Design system. tokens/theme.rs is compiled into the app
                      via #[path]; components/*/README.md specify each widget.
docs/                 MANUAL, CHANGELOG, ARCHITECTURE, VALIDATION,
                      COMPATIBILITY, WINDOWS-TEST.
tools/check_pe.py     Audits Windows executables' headers and import list.
```

## Build and check

Toolchain is pinned in `rust-toolchain.toml` (1.91.1). Always pass `--locked`.
Linux development needs X11 headers (libx11) and an X11/Xwayland session.

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked          # core unit tests
cargo run --locked -- --smoke-test       # headless shared-UI smoke test
cargo run --locked -- --demo             # synthetic demo, no game data
cargo run --locked -- --help             # CLI commands (list/inspect/validate/...)
```

Windows targets cross-build from Linux with the bundled `rust-lld`; see
`.cargo/config.toml` and the README "Windows builds" section. After any change
that could add a Win32 import, build both targets and run
`python3 tools/check_pe.py [--legacy] <exe>`. CI (`.github/workflows/windows.yml`)
runs fmt, core tests, clippy, release builds, the PE audit and the smoke test
on both `i686-pc-windows-msvc` and `x86_64-pc-windows-msvc`.

All of fmt, clippy (`-D warnings`), tests and the smoke test must pass before
committing.

## Hard constraints

- **Windows 98/ME is a target.** The i686 build has no CRT and links against a
  fixed allow-list of ANSI Kernel32/User32/GDI32/WinMM functions in
  `tools/check_pe.py`. Do not add Win32 APIs casually; if one is truly needed,
  confirm it exists on Win98, add it to the allow-list, and say why.
- **No floating point.** Neither crate uses `f32`/`f64`; camera, raster and
  geometry math are integer/fixed-point so the legacy runtime needs no float
  helpers. Keep it that way.
- **hangar-core stays `no_std` + `alloc`** with no OS dependencies. File I/O,
  windows and audio belong in the app's platform backends.
- **Dependencies are deliberately minimal** (only `miniz_oxide`). Do not add
  crates without a strong reason; anything added must be `no_std`-friendly and
  noted in `THIRD_PARTY_NOTICES.md`.
- **Keep native stack frames small.** Large buffers (palettes, images) go on
  the heap (`Box`), as existing code does.
- **No game data in the repo.** `.gitignore` excludes LIB/SH/PT/EXE files;
  never commit retail payloads, extracted resources or screenshots of them.
  `.local/` is a git-ignored scratch area for local probes.

## Editing and file-format rules

These protect users' game files. Follow them in any new feature.

- **Lossless by default.** Unedited archives round-trip byte-for-byte;
  untouched payloads keep their bytes and compression flags. Edits change only
  the bytes they own. A no-op edit must not re-encode anything.
- **Bounded parsing.** Every read is bounds-checked (`slice`, `u16_at`,
  `u32_at` in `hangar-core/src/lib.rs`) and every scan has explicit limits.
  Malformed input returns an error string; it must never panic.
- **Never execute or load resource code.** PL/PE modules are inspected as inert
  bytes only.
- **Unknown data is preserved, not guessed.** Unrecognized SH records, BRF
  fields or opaque binaries stay untouched; a shape with records the writer
  does not understand is read-only. Never silently flatten or drop records.
- **Source units only.** Values stay in stored units; do not invent unit
  conversions or gameplay semantics without evidence.
- **Retail LIB names are protected** (`hangar-core/src/save.rs`). Do not
  weaken the list or add an unlock switch. Custom saves go through
  `saving.rs` (stage, numbered `.bak`, install). Exports are create-new.
- **Undo is entry-granular** (`hangar-core/src/document.rs`). A user action
  that touches several entries (a stroke, a graft, a clone) is one undo step;
  use `Document::transaction` to validate and apply it atomically.

Format evidence comes from the sibling TORE Fighters project (format docs,
Python tools, Rust readers) and from user-owned retail LIBs. Treat both as
read-only references; ask the user where they are if you need them. Do not
hard-code their locations in code or docs.

## UI conventions

- All drawing goes through `Canvas`/`Draw` commands and hit regions emitted by
  the shared UI, so both backends render the same thing and the smoke test can
  click controls. Do not draw directly in a platform backend.
- Colors, spacing and metrics come from `theme` (`tore-hangar-design/tokens/
  theme.rs`). Do not hard-code colors. Edit `tokens.json` and regenerate if a
  token must change.
- GDI-friendly: solid fills and 1px lines only; no alpha, gradients, blurs or
  decorative animation. Minimum window is 800 x 600; check both 800x600 and
  1280x800.
- `amber` means selected, active or changed, never decoration. `steel` means
  time/reference. Changed values show saved vs current with a reset control.
- Copy is terse and technical: noun labels, sentence-case verb actions, file
  names in caps as stored, no "OK"/"Submit", no exclamation marks. Status
  messages say what happened and what it affects.
- Blender muscle memory: G/R/S with X/Y/Z axis lock and numeric entry, Tab for
  Edit mode, MMB orbit, numpad views, Ctrl+Z / Ctrl+Shift+Z.
- Do not ship controls for unimplemented features. If something is not
  supported, say so in the UI rather than showing an inert widget.

## Tests

- Core logic gets `#[cfg(test)]` unit tests in the same module. Build fixtures
  synthetically in code; tests must not need retail files.
- UI features get a `smoke_*` method on `App` in their `ui_*.rs` file, wired
  into `--smoke-test` in `cli.rs`. Smoke tests drive the real event and hit
  region paths (click rendered controls, check undo returns original bytes,
  save/reopen) and must pass headless on Linux and Windows CI.
- Checks that need real game data are separate CLI commands (`--paint-check`,
  `--panel-check`, ...) run manually against user-supplied LIBs and are never
  part of CI.
- When a change needs Windows or real-game acceptance, add steps to
  `docs/WINDOWS-TEST.md` and record what was actually verified in
  `docs/VALIDATION.md`. Do not claim game compatibility that was not tested.

## Code style

- Match surrounding code: compact, sparse comments, explicit error strings
  (`hangar_core::Result<T>` is `Result<T, String>`).
- Prefer extending an existing `ui_*.rs` or core module over new abstractions;
  add a new module when a feature has its own data model.
- Keep docs current: user-visible changes go in `docs/MANUAL.md` and
  `docs/CHANGELOG.md` (and the README highlights if notable); format/writer
  decisions go in `docs/ARCHITECTURE.md`. The workspace version lives in the
  root `Cargo.toml`; keep the README version badge in step with it.

## Commits

One logical change per commit, imperative summary line describing the result
(e.g. "Add collapsible library trees and reviewed cross-library moves").
Run the full check list first. Do not commit build output or game data.
