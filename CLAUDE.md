# omaboy — agent notes

Game Boy / Game Boy Color emulator: Rust core behind a C ABI, Qt6/QML shell,
themed live by the user's Omarchy desktop. Built for the classic Pokémon
games (MBC3 RTC, CGB double speed, HDMA all matter — don't break them).

## Build, test, install

```sh
cmake -B build && cmake --build build -j     # make generator; Ninja is NOT installed
cmake --install build --prefix ~/.local      # binary, .desktop, icon
(cd core && cargo test)                      # save-state round-trip tests
```

- CMake drives `cargo build --release` via `add_custom_command`; the Qt app
  links `core/target/release/libomaboy_core.a`.
- Versioning: bump `project(... VERSION x.y.z)` in CMakeLists.txt **and**
  `version` in core/Cargo.toml together; update CHANGELOG.md.
- AUR: `packaging/aur/PKGBUILD` builds from the GitHub release tag. On each
  release: bump `pkgver`, update `sha256sums` from the new tarball,
  regenerate `.SRCINFO` (`makepkg --printsrcinfo`), test `makepkg -f`, and
  push both files to the AUR repo (`ssh://aur@aur.archlinux.org/omaboy.git`).

## Testing the emulator core

- `core/target/release/headless <rom> [frames] [out.ppm]` — runs without a
  UI, prints serial output (blargg tests report there), dumps the last frame.
- Accuracy baseline that must keep passing: blargg `cpu_instrs` (11/11),
  `instr_timing`, dmg-acid2, cgb-acid2 (ROMs: github.com/retrio/gb-test-roms
  and mattcurrie's acid2 releases).
- GUI verification: `omaboy --open-settings` opens the settings overlay at
  launch; capture with `grim -g "<x>,<y> <w>x<h>"` using geometry from
  `hyprctl clients -j`. **Never inject keys (wtype/ydotool) while the user
  is active** — focus bounces and keystrokes land in their windows.

## Architecture map

- `core/src/` (Rust): `cpu.rs` ticks the bus on every memory access
  (sub-instruction timing — keep that model when adding opcodes/hardware).
  `bus.rs` owns ppu/apu/timer/cart/joypad. `state.rs` is the save-state
  serializer: **append new fields at the end and bump its VERSION const**;
  states are fingerprinted against ROM header bytes 0x134..0x150. The APU
  is deliberately not serialized (games re-drive it; audio self-corrects).
- `core/src/lib.rs` is the FFI surface; `src/gbcore.h` must mirror it by
  hand — update both together.
- `src/` (C++): `emulator.cpp` runs the core on a worker thread paced by
  audio-ring fill (±3% around 59.73 Hz); every core call goes through
  `m_coreMutex`. Options are Q_PROPERTYs persisted via QSettings
  (`~/.config/omaboy/omaboy.conf`). `inputmap.cpp` owns rebindable keys
  (esc/f1/f2/f11 reserved). `gamepad.cpp` is SDL3 controller input
  (fixed mapping, hotplug, d-pad+stick merged; test it with an SDL
  virtual gamepad — no hardware needed). `theme.cpp` parses Omarchy
  colors.
- `qml/`: Main window + walker-style overlays (RomBrowser, SettingsOverlay,
  HelpOverlay). Overlays anchor **above** the status bar so its hints stay
  visible; new global shortcuts belong in the status bar hint line too.

## Omarchy theming rules

- Active theme: `~/.local/state/omarchy/current/theme/colors.toml` (flat
  `key = "#hex"` pairs); `theme.name` in the same dir is rewritten on every
  switch — that's the hot-reload watch trigger.
- Never hardcode UI colors; use the `theme` context properties. UI text is
  lowercase, monospace (JetBrainsMono Nerd Font with fixed-font fallback).
- Game pixels are exempt from theming by default: DMG palette defaults to
  classic green (user preference); the theme-derived palette is opt-in.

## Omarchy shell plugin (manifest.json + plugin/)

The repo is also an installable omarchy-shell plugin (bar-widget kind): a
launcher panel that spawns the omaboy binary via `Quickshell.execDetached`.
Plugin QML runs inside omarchy-shell's engine — pure QML/JS only, no native
code. Use the shell's own components (`qs.Ui`: BarWidget, WidgetButton,
Panel/KeyboardPanel, PanelHero(title/meta/detail), PanelSectionHeader,
Button — check property names in ~/.local/share/omarchy/shell/Ui/ before
using them). Validate with `omarchy plugin validate .`; the shell hot-reloads
the installed copy in ~/.config/omarchy/plugins/io.github.diegopluna.omaboy/
on file change (QML errors appear in `journalctl --user`). Keep
manifest.json's `version` in lockstep with the app version.

## Sidecar files (next to the ROM)

`.sav` battery RAM (standard, other-emulator compatible) · `.rtc` clock
(44-byte custom, wall-clock catch-up on load) · `.st1`–`.st3` save states.
