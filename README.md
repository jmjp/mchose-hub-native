# M HUB Native

![CI](https://github.com/jmjp/mchose-hub-native/actions/workflows/ci.yml/badge.svg)
![Release](https://github.com/jmjp/mchose-hub-native/actions/workflows/release.yml/badge.svg)
![License: MIT](https://img.shields.io/badge/license-MIT-blue)

A **WebView-free, native Rust + egui** reimplementation of the MCHOSE HUB
("M HUB") desktop companion for MCHOSE headsets — reconstructed by reverse
engineering the official Electron client and its vendor components.

The reference target is the **MCHOSE V9 PRO** headset (USB `VID 0x291D / PID 0x385D`).

> Independent interoperability project. Not affiliated with or endorsed by MCHOSE.
> Vendor binaries/assets are not redistributed; the vendor driver packages are used
> only when the official MCHOSE HUB is installed locally.

## Features

- Native, frameless egui app; light/dark theme follows the operating system
- Device discovery, identity, firmware status and the recovered 0xAA HID protocol
- 10-band equalizer, factory preset library, user presets, import/export
- Sound effects: treble enhancement, dynamic bass (cutoff), volume balance, vocal clarity
- Microphone DSP: AI/steady noise reduction, magic voice; microphone monitoring
- Volume/mute, speaker test sounds, system sound-settings shortcut
- Native **C-Media ConfLib** integration for vendor DSP control on Windows
- Driver install/repair (`pnputil`), tray icon, close-to-tray, start-with-Windows,
  start-minimised
- English / Brazilian Portuguese / Simplified Chinese UI

## Platform support

| Platform | Status | Audio | HID | Tray / auto-start |
| --- | --- | --- | --- | --- |
| Windows 10/11 x64 | full, tested | C-Media ConfLib DSP + Core Audio | hidapi | native tray + registry Run entry |
| Linux (X11/Wayland) | implemented, CI-built | `wpctl` (PipeWire) or `pactl`; volume/mute only | hidapi (`libudev` + udev rule) | no tray; XDG autostart |
| macOS | implemented, CI-built | `osascript` output volume/mute | hidapi (Input Monitoring) | no tray; LaunchAgent |

**Vendor DSP parity is Windows-only.** The equalizer, sound effects, microphone
processing/monitoring, driver/APO installation, firmware OTA, THX and the vendor OSD
are C-Media/MCHOSE Windows components. On Linux/macOS the app runs with system
volume/mute and HID device status, and reports the DSP controls as unavailable.

## Build & run

```bash
cargo build --release
cargo run --release        # or target/release/mchose-hub-native
```

Linux build prerequisites: `libudev-dev`, `libxkbcommon-dev`, `libwayland-dev`,
`libx11-xcb-dev`, `pkg-config` and a C toolchain. For HID access add a udev rule
granting your user access to `291d:385d` (or run with suitable permissions).

macOS needs the Xcode command line tools and Input Monitoring/Accessibility permission
for HID access.

### Hardware/protocol probe

```bash
cargo run -- --probe
```

Prints a JSON report (HID topology, firmware feature report, battery query matrix,
notification listen, audio capabilities/state) and writes `probe-report.json` into the
app data directory. It never flashes firmware or resets the device.

### Tests / lint

```bash
cargo test
cargo clippy
```

## Downloads

Prebuilt binaries for Windows, Linux and macOS are attached to every
[release](https://github.com/jmjp/mchose-hub-native/releases), built by GitHub Actions
(`.github/workflows/release.yml`, triggered by `v*` tags). Each release exposes both:

- **raw binaries** for direct download — `mchose-hub-native-windows-x86_64.exe`,
  `mchose-hub-native-linux-x86_64`, `mchose-hub-native-macos-x86_64`;
- **archives** (`.zip` / `.tar.gz`) that bundle the README/LICENSE/NOTICE and preserve
  the executable bit;
- **`SHA256SUMS.txt`** for integrity verification.

GitHub does not preserve file permissions on release downloads: after downloading the
Linux/macOS raw binaries run `chmod +x`, and on macOS allow the unsigned app under
System Settings -> Privacy & Security (or `xattr -d com.apple.quarantine <file>`).

## Data & configuration

Stored under `%APPDATA%\MCHOSE_HUB_Native\` on Windows and
`$XDG_CONFIG_HOME`/`~/.config`, `~/Library/Application Support` equivalents elsewhere:
`config.json`, `eq_presets.json`, `logs/`.

## Reverse-engineering documentation

- [`docs/report.md`](docs/report.md) — full report: architecture, UI, HID/audio protocol, evidence
- [`docs/plan.md`](docs/plan.md) — implementation plan and delivered status
- [`docs/findings-renderer.md`](docs/findings-renderer.md) — recovered remote renderer UI, routes, schemas
- [`docs/findings-runtime.md`](docs/findings-runtime.md) — live IPC/HID captures and runtime evidence
- [`docs/findings-native.md`](docs/findings-native.md) — native DLL analysis (ConfLib, OTA, helpers)
- [`docs/findings-prior-art.md`](docs/findings-prior-art.md) — prior-art cross-check
- [`docs/review-rust-ui.md`](docs/review-rust-ui.md) — UI review against the vendor spec
- [`docs/screenshots`](docs/screenshots) — native app screenshots

## Limitations

- Battery level is not reported by this headset (the official app shows none either;
  every HID query stays silent).
- Virtual 7.1 writes are not implemented (vendor control-handler encoding; UI read-only).
- Firmware OTA, THX, cloud account/store/RGB, per-app profiles and the vendor OSD are
  intentionally not reimplemented.
- Connect/home is a native approximation of the original remote web page.
- Linux/macOS code paths are CI-compiled but were not runtime-tested on real hardware
  during development (only Windows was).

## License

MIT — see [`LICENSE`](LICENSE). Third-party names and trademarks belong to their
respective owners; see [`NOTICE.md`](NOTICE.md).
