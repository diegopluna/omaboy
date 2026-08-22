# Changelog

All notable changes to omaboy are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project
uses [Semantic Versioning](https://semver.org/).

## [0.4.0] - 2026-08-22

### Added
- **Controller support** (SDL3): plug in any gamepad and play — hotplug
  with on-screen connect/disconnect toasts, no configuration. D-pad and
  left stick both drive the GB d-pad (merged, with hysteresis on the
  stick); east button = A, south = B (physical Game Boy layout);
  start/back = start/select; right shoulder or right trigger = turbo
  (hold); left shoulder or guide = pause. The controller also navigates
  the game library: stick/d-pad to move, A/start to launch, B to close.
  Works while the window is unfocused (input is evdev-level).

## [0.3.0] - 2026-08-21

### Added
- **Omarchy shell plugin** (`manifest.json` + `plugin/`): this repo now
  doubles as an installable bar widget —
  `omarchy plugin add https://github.com/diegopluna/omaboy --enable`.
  A gamepad glyph in the bar opens a panel listing your recent games (read
  from omaboy's config) and ROM library; one click launches the game in the
  emulator. Right-click opens omaboy directly. Detects a missing omaboy
  binary and points at the build instructions.

## [0.2.0] - 2026-08-21

### Added
- **Save states**: full machine snapshots, 3 slots per game (`f5` save,
  `f8` load, `f6` cycle slot). Versioned format, fingerprinted against the
  ROM header so a state can never load into the wrong game, atomic on
  failure (a corrupt file leaves the running game untouched).
- **Rebindable controls**: every gameplay and emulator action can be
  remapped from the settings overlay (press-to-capture, conflicts auto
  unbind, reset-to-defaults). `esc`/`f1`/`f2`/`f11` stay reserved.
- **Settings overlay** (`f2`): keyboard-driven, walker-style.
- Quality of life: pause on focus loss with auto-resume, turbo speed
  selector (2×/4×/8×), integer scaling toggle, GBC color correction toggle,
  resume-last-game-on-launch, FPS display toggle, volume row.
- Screenshots (`f12`) saved to `~/Pictures/omaboy/` at 4× scale.
- Context-sensitive shortcut hints in the status bar, visible in every
  state (home, library, playing); overlays no longer cover the bar.
- Help overlay reflects current (rebound) keys.
- `--version` flag; `--open-settings` dev flag.

### Changed
- **Rebranded omulator → omaboy** (binary, Wayland app-id, desktop entry,
  QML module, Rust crate). Existing settings migrate automatically.
- Original Game Boy games default to the classic green palette; the
  omarchy theme-derived palette is now opt-in (`p` to cycle).

## [0.1.0] - 2026-08-21

### Added
- Game Boy / Game Boy Color emulator core in Rust (C ABI static library):
  SM83 CPU with sub-instruction memory timing, scanline PPU (DMG + CGB),
  4-channel APU at 48 kHz, MBC1/MBC2/MBC3+RTC/MBC5, CGB double speed,
  HDMA, zipped ROM support. Passes blargg `cpu_instrs` (11/11) and
  `instr_timing`, plus dmg-acid2 and cgb-acid2.
- Battery saves in the standard `.sav` format with autosave; MBC3
  real-time clock persisted with wall-clock catch-up (`.rtc`).
- Qt 6 / QML front-end with live [Omarchy](https://omarchy.org) theming:
  parses the active theme's `colors.toml` and hot-reloads on theme switch.
- Walker-style keyboard-driven game library with recents and fuzzy filter.
- Audio-clocked frame pacing (emulation locked to the audio ring buffer).
- Headless test harness (`core/target/release/headless`).
