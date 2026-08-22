# omaboy

A Game Boy / Game Boy Color emulator that dresses like your
[Omarchy](https://omarchy.org) desktop. Rust core, QML shell.

Built for playing the classic Pokémon games: Red/Blue/Yellow (DMG, MBC1/MBC5)
and Gold/Silver/Crystal (GBC, MBC3 with real-time clock — day & night cycles
survive across sessions).

## Omarchy integration

- Reads the active theme from `~/.local/state/omarchy/current/theme/colors.toml`
  and **hot-reloads when you switch themes** — window, accent border, status
  bar, overlays, everything.
- Original Game Boy (non-color) games use the classic green palette by
  default; press `p` to cycle to grayscale or the "omarchy" palette — a
  4-shade ramp derived from the theme's background→foreground colors, if you
  want Pokémon Red to look like your terminal.
- Walker-style keyboard-driven game library (`esc`), JetBrainsMono, no chrome.
- Sets Wayland app-id `omaboy` for Hyprland window rules.

## Build & install

Needs: `qt6-base` `qt6-declarative` `qt6-multimedia` `sdl3` `cmake` `rust`

```sh
cmake -B build
cmake --build build -j
cmake --install build --prefix ~/.local   # binary + launcher entry
```

Run `omaboy` (or from your launcher), or `omaboy path/to/game.gbc`.

## Omarchy bar plugin

This repo doubles as an [Omarchy shell plugin](https://omarchyplugins.com) —
a bar widget that lists your recent games and ROM library and launches them
in omaboy:

```sh
omarchy plugin add https://github.com/diegopluna/omaboy --enable
```

> **Note:** `omarchy plugin add` only installs the bar widget — it does
> **not** build the emulator. Build and install omaboy first (section
> above), or the widget will point you at the build instructions.

Left-click the gamepad glyph for the game picker, right-click to open the
emulator. The widget finds the `omaboy` binary on your PATH (install the app
as above). The panel can also be summoned from a hotkey:
`omarchy shell io.github.diegopluna.omaboy toggle`.

To remove: `omarchy plugin remove io.github.diegopluna.omaboy` (the emulator
itself, if installed, is just `~/.local/bin/omaboy` plus its desktop entry
and icon — delete those to uninstall it).

## Usage

Drop your ROMs (`.gb`, `.gbc`, or zipped) in `~/Games` — or press `esc` →
`ctrl+d` to point the library somewhere else. Battery saves (`.sav`, standard
format, compatible with other emulators) and RTC state (`.rtc`) are written
next to the ROM, autosaved every 15 s and on pause/exit.

Default keys (all rebindable in settings, `f2`):

| key | action |
|---|---|
| arrows | d-pad |
| `x` / `z` | A / B |
| `enter` / `shift` | start / select |
| `space` (hold) | turbo (2×/4×/8×, set in settings) |
| `tab` | pause |
| `f5` / `f8` | save / load state |
| `f6` | next state slot (3 per game) |
| `f12` | screenshot → `~/Pictures/omaboy/` |
| `esc` | game library |
| `f1` / `f2` | help / settings |
| `r` | reset |
| `p` | cycle palette (classic / mono / omarchy) |
| `m`, `+`, `-` | mute, volume |
| `f` / `f11` | fullscreen |

**Controllers** just work: plug in any gamepad (SDL3 hotplug — Xbox,
PlayStation, 8BitDo, whatever) and play. D-pad or left stick moves; the
east button is A and south is B, matching the physical Game Boy layout;
start/back are start/select; hold the right shoulder or right trigger
for turbo; left shoulder pauses. The controller drives the game library
too: move with the stick, A to launch, B to close.

Settings (`f2`) also has: pause-on-focus-loss (auto-resumes when you come
back), integer scaling, GBC color correction, resume-last-game-on-launch,
and FPS display. Save states are full machine snapshots (`.st1`–`.st3` next
to the ROM) and refuse to load into the wrong game.

## Architecture

- `core/` — Rust (`staticlib`, C ABI): SM83 CPU with sub-instruction memory
  timing, scanline PPU (DMG + CGB), 4-channel APU at 48 kHz, MBC1/2/3+RTC/5,
  CGB double-speed, HDMA, battery saves. Passes blargg `cpu_instrs` (11/11)
  and `instr_timing`, plus dmg-acid2 and cgb-acid2.
- `src/` — Qt glue: worker-thread emulation paced against the audio ring
  buffer, `QAudioSink` output, Omarchy theme watcher, save persistence.
- `qml/` — the UI.

`core/target/release/headless <rom> [frames] [out.ppm]` runs the core without
a UI (prints serial output from test ROMs, dumps the framebuffer).

## Legal

omaboy is a clean-room emulator: it contains no Nintendo code, no boot ROM,
and no game data — the hardware behaviour is reimplemented from publicly
available documentation. It does not ship games; play cartridges you own,
dumped yourself. "Game Boy" and "Nintendo" are trademarks of Nintendo,
used here only to describe compatibility. MIT licensed (see LICENSE).
