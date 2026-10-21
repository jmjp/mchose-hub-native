# MCHOSE HUB — Implementation Plan (Rust + egui, no WebView)

Version: 1.0 · Derived from `report.md` and the findings sub-documents.
Target app: a native Rust/egui desktop "M HUB" companion for MCHOSE headsets, with the
**MCHOSE V9 PRO (VID 0x291D / PID 0x385D)** as the reference device, reproducing the
original (Electron + remote Vue) application's user workflows without a browser engine.

---

## 1. What "core functionality" means for this target

The installed app is a shell around a remote UI plus two control planes:

* **A. Host-side audio DSP plane** — 10-band EQ, treble/bass/volume-balance/vocal-clarity
  enhancements, Xear 7.1 surround, mic DSP (AI-NR, steady NR, magic voice, voice clarity),
  volumes/mute, test sounds. Implemented in the original by `osConfLib.dll` (C-Media ConfLib)
  → **KS properties on the C-Media audio driver**. This is the V9 PRO's actual settings plane
  (`audioRuntimeType = "cmedia"`, legacy layout).
* **B. Device HID plane** — identification, firmware/dongle versions, battery/sleep state,
  prompt volume, auto-shutdown, factory reset, (and for new-layout devices on-device PEQ).
  Implemented in the original renderer with **WebHID** using the 0xAA 63-byte frame protocol
  (report id 85 for the V9 PRO family).

The reimplementation therefore ships **both planes**:

| Plane | Rust implementation |
| --- | --- |
| A (audio DSP) | Trait `AudioBackend`. Primary: **`ConfApiBackend`** — dynamic FFI to the vendor `osConfLib.dll` (loads only if present; full parity). Fallback: **`CoreAudioBackend`** — Windows Core Audio (`IAudioEndpointVolume`) for volume/mute only; other controls become read-only with an explicit reason in the UI. A `MockBackend` is used for development/tests. |
| B (HID) | `HidBackend` via `hidapi` — enumerate by VID/PID, read feature report 170 name/firmware, command frames on report id 85, notify listener decoding battery/sleep. All frames go through a unit-tested codec (`protocol.rs`). |

Features that depend on vendor-only components are **not** reimplemented; the UI keeps
the original information architecture but clearly marks them as unsupported (documented in
README + in-app "About/Compatibility" page):

* THX spatial-audio runtime and installer (proprietary gRPC service).
* Driver/APO installation, firmware OTA flashing, cloud account/store/RGB-sync, per-app
  game profiles, lyrics screens, K20 GT screen, BetterNCM patching.

---

## 2. Technology choices

| Concern | Choice | Rationale |
| --- | --- | --- |
| UI | `eframe` + `egui` (latest stable, pinned after `cargo search`) | pure native, GPU/CPU raster, no WebView; the original's Element-Plus-like aesthetic is reproduced with custom theme tokens |
| Windowing/tray | `eframe` window; `tray-icon` (optional feature) | close-to-tray behaviour observed in the original (`hideWhenClose`, custom tray) |
| HID | `hidapi` crate (Windows: HIDAPI/HidD over `SetupAPI`) | mature; matches report/feature usage; no driver install |
| Audio DSP | `libloading` → `osConfLib.dll` (C-Media ConfLib); fallback `windows` crate Core Audio | The vendor library is a generic C-Media component already installed by the original app (and by C-Media driver installs). Documented as adapter; UI degrades gracefully when absent. |
| Config | `serde` + `serde_json` under `%APPDATA%\MCHOSE_HUB_Native\` | mirrors the original's JSON configs, no registry state |
| Logging | `log` + `env_logger` + in-app ring buffer | replaces the original's daily log files |
| i18n | embedded dictionaries (en-US, pt-BR, zh-CN) + `system locale` autodetect | original ships 9 locales; the three most relevant are included, the rest fall back to en-US |
| Errors | `thiserror` / `anyhow` | typed backend errors surfaced to the UI |

No async runtime is required: HID polling runs on a worker thread and communicates with the UI
through `crossbeam-channel`; audio calls are synchronous and cheap (KS property get/set).

---

## 3. Repository layout

```
mchose-hub-rs/
├── Cargo.toml
├── README.md                     # build/run, parity matrix, limitations
├── assets/
│   ├── presets.json              # 10-band EQ factory presets (ported from eq.json)
│   └── icon.png
└── src/
    ├── main.rs                   # entry: logging, config, backends, eframe::run_native
    ├── app.rs                    # MHubApp state machine, page routing, event pump
    ├── config.rs                 # AppConfig (settings) + paths
    ├── i18n.rs                   # tiny dictionary-based translator
    ├── logging.rs                # log file + in-app ring buffer
    ├── model/
    │   ├── mod.rs
    │   ├── device.rs             # Identity, DeviceStatus (battery/charging/sleep), family table
    │   ├── audio.rs              # AudioState: EQ bands, effects, surround, mic, volumes
    │   └── presets.rs            # preset DB, import/export
    ├── backend/
    │   ├── mod.rs                # AudioBackend / DeviceBackend traits, events
    │   ├── protocol.rs           # 0xAA frame codec + command enums (+unit tests)
    │   ├── hid.rs                # hidapi enumeration + command/notify transport
    │   ├── confapi.rs            # osConfLib.dll FFI adapter (PropertyControl facade)
    │   ├── coreaudio.rs          # Windows fallback: endpoint volume/mute
    │   └── mock.rs               # in-memory backend for UI tests/dev
    └── ui/
        ├── mod.rs
        ├── theme.rs              # light/dark tokens, spacing, fonts (HUB-like)
        ├── widgets.rs            # card, switch, slider row, section title, EQ curve plot
        └── pages/
            ├── mod.rs            # page enum + sidebar definition
            ├── connect.rs        # device discovery/select (connect screen)
            ├── home.rs           # device dashboard + status sidebar
            ├── routine.rs        # audio routine (volume, NR/AI-NR, magic voice, mic listen, effect cards)
            ├── eq.rs             # 10-band EQ curve editor + preset library
            ├── effects.rs        # treble / bass / volume balance / vocal clarity cards
            ├── surround.rs       # Xear virtual 7.1 (enable/mode/room, speaker test)
            ├── mic.rs            # mic page (NR/AI/leveling/listen/voice clarity)
            ├── firmware.rs       # versions, power management, factory reset (read-only OTA)
            └── settings.rs       # theme/language/startup/close behaviour/about/compatibility/log
```

---

## 4. Dataflow

```
             ┌──────────── eframe/egui UI (ui/pages/*) ────────────┐
             │  reads AppState, sends UiAction via channel          │
             └───────────────┬─────────────────────────────────────┘
                             │
                    app.rs (state machine, 30 Hz tick)
             ┌───────────────┴───────────────┐
   AudioBackend (trait)                DeviceBackend (trait)
   ├ ConfApiBackend (osConfLib FFI)    └ HidBackend (hidapi worker thread)
   └ CoreAudioBackend (fallback)            │  protocol codec
                                            └─ events: battery/sleep/notify, hotplug
   config.rs (JSON)  i18n.rs  logging.rs
```

* UI never blocks on device I/O: `HidBackend` owns a worker thread with a command channel and
  an event channel; the app drains events each frame and requests a repaint.
* Audio setters debounce like the original (150 ms) in the UI layer.

## 5. Device/audio model (ported behaviour)

* **Identity**: VID `0x291D`, PID `0x385D` (V9 PRO); protocol/legacy layout and report id 85;
  additional family entries (VID/PID ↔ model name/report id) live in a table in `model::device`
  so future devices can be added without touching the UI.
* **Status**: battery percent + charging + sleep from notify frames (cmd 11/34: byte1 bit7 =
  charging, low 7 bits = percent); hidden when sleep. Firmware/dongle versions from descriptors
  where available (feature report 170 UTF-8 string for the family; otherwise `unknown`).
* **Audio state**: `eq_enabled`, `eq_bands[10]` (dB, step 1), effect blocks
  (bright/level, virtual bass/level/cutoff, adaptive volume/level/mode, voice clarity/level/noise),
  surround (enable, mode music/movie, room small/mid/big), mic (AI-NR, NR, magic voice type,
  mic listen), volumes/mute (speaker + prompt 1..16).
* **Presets**: factory presets ported from `eq.json` (game & music groups, Chinese + English
  names mapped to i18n-able labels), plus user preset CRUD persisted as JSON.

## 6. UI/UX plan (workflow parity)

Original workflow → native screen mapping:

| Original screen (`/headset/...`) | Native page | Controls |
| --- | --- | --- |
| `/index` connect-device cards | `connect.rs` | device cards, connection mode icon, (re)scan |
| device dashboard/sidebar | `home.rs` | device identity, battery, firmware, link to manual |
| `audio_routine` | `routine.rs` | volume+mute, AI-NR, steady NR, magic voice, mic listen, effect cards, test/Windows-sound shortcut |
| EQ library + editor | `eq.rs` | 10-band curve, preset grid, save-as/rename/import/export, enable switch |
| `audio_speaker` (4 cards) | `effects.rs` | treble, bass (+cutoff), volume balance (+mode), vocal clarity (+noise) |
| `audio_surround` (Xear) | `surround.rs` | enable, mode (music/movie), room size, speaker test |
| `audio_mic` | `mic.rs` | mic cards, mic-disabled recovery instructions |
| `otherSetting` | `firmware.rs` | versions, auto-shutdown, device power-off, factory reset |
| header/settings/theme | `settings.rs` | light/dark/system, language, autostart, close-to-tray, about/compatibility, log |

Visual language ported from the renderer: fixed header with product wordmark and window controls,
left sidebar with device icon + nav items (Routine / EQ / Effects / Surround / Mic / Firmware /
Settings), content cards with 16 px radius, light theme `#F2F3F5`-ish background with white cards,
dark theme equivalent, acumin-style sans (bundled font if available, else default egui font).

Window behaviour: 1120×720 default, min 960×600, custom titlebar area drawn in egui with
minimise/maximise/close (frameless window), remembered size/position, close-to-tray optional.

## 7. Work breakdown & order

1. **Scaffold** – Cargo project, dependencies, config/logging/i18n, theme tokens, empty pages.
2. **Protocol + HID** – frame codec with unit tests; hidapi enumeration/feature report; worker
   thread; battery notify decode; hotplug rescan.
3. **Audio backends** – `ConfApiBackend` FFI (osConfLib facade incl. `PropertyControl` get/set
   conversions), `CoreAudioBackend` fallback, `MockBackend`; capability detection that drives UI
   enabled/disabled states.
4. **Model + presets** – state structs, EQ preset JSON (ported from eq.json), import/export.
5. **UI shell** – frameless window, header, sidebar, page router, theme switch, i18n.
6. **Feature pages** – connect, home, routine, eq, effects, surround, mic, firmware, settings.
7. **Persistence + tray + polish** – settings/debounce, system tray, log viewer, about/compat page.
8. **Verification** – `cargo test` (protocol/presets/config), `cargo clippy`, build release,
   manual smoke test with the connected V9 PRO; document what was verifiable on hardware.

## 8. Risks and mitigations

| Risk | Mitigation |
| --- | --- |
| `osConfLib.dll` absent on other machines | CoreAudio fallback + capability model; UI explains which controls need the C-Media driver package (installed by MCHOSE HUB/driver install) |
| HID protocol variants (report id per family; new-layout devices) | Family table + protocol version field; V9 PRO path is the verified reference; unknown families surface "unsupported" instead of guessing |
| Battery/firmware unknown until runtime confirmation | UI shows "—"/unknown; no fabricated values; runtime capture recorded in `findings-runtime.md` |
| Remote UI changes over time | We implement the documented v1.1.26 workflow, not the remote page itself |
| Firmware OTA is dangerous | Explicitly out of scope; UI links to official app and shows versions |
| Vendor DLL loading (ABI/arch) | 64-bit only, `libloading` with typed signatures from observed koffi declarations; failures degrade gracefully |

## 9. Definition of done for this task

* `cargo build --release` and `cargo test` green; `cargo clippy` without warnings (allow doc lints).
* App launches natively, detects the connected V9 PRO, shows identity/battery/firmware state,
  and performs the implemented audio settings through the vendor adapter when available.
* All UI pages navigable and persisted; unsupported features clearly marked and documented.
* `docs/report.md` + sub-documents complete; `mchose-hub-rs/README.md` documents architecture,
  parity matrix, build/run and known limitations.

---

## 10. Implementation status (delivered)

| Plan step | Status | Notes |
| --- | --- | --- |
| 1. Scaffold (crate, config, logging, i18n, theme) | ✅ | `mchose-hub-rs/` with eframe 0.31, JSON config under `%APPDATA%\MCHOSE_HUB_Native`, en/pt/zh dictionaries, CJK font loading, light/dark palette |
| 2. Protocol + HID worker | ✅ | `backend/protocol.rs` (+10 unit tests), `backend/hid.rs` worker with collection/report routing, firmware feature report, battery attempts, notification listener, hotplug rescan |
| 3. Audio backends | ✅ with environment caveat | `backend/winaudio.rs` writes the recovered C-Media `PROPERTYKEY`s through `IPropertyStore` and uses Core Audio for volume/mute; capability probing falls back to Core Audio when no FX property store exists (the state of the analysis machine). Mock backend for tests |
| 4. Model + presets | ✅ | Semantic state/ranges/defaults corrected against the vendor spec (`docs/review-rust-ui.md`); factory presets ported verbatim from the device-local `eq.json` (13 game + 11 music); user presets/import/export |
| 5. UI shell | ✅ | Frameless custom titlebar, sidebar navigation, page router, dialogs, toasts, theme switching |
| 6. Feature pages | ✅ | connect, home, routine (hero + 3 columns), eq, effects, surround + 8-channel test, mic, firmware, settings/about/log — screenshots in `mchose-hub-re/shots/60…67` |
| 7. Persistence, tray, polish | ✅ | Config/presets/log persistence, auto-start via HKCU Run, **native tray icon + close-to-tray** (Shell_NotifyIcon on a message-only window thread) and start-minimised behaviour |
| 8. Verification | ✅ | `cargo test` 10/10, `cargo build` clean, `--probe` hardware validation, UI screenshots, vendor-spec review applied |

### Deviations from the original plan

1. **Audio backend: vendor ConfLib FFI (resolved).** The initial plan proposed an
   `osConfLib.dll` FFI adapter, then native/runtime analysis showed the vendor effect
   values are not standard endpoint properties and that the official app runs an
   elevated AudioSDK host. That made the plain property-store path insufficient. The
   ABI was then recovered from the shipped DLL + Ghidra decompilation and implemented
   in `backend/confapi.rs`; it initialises ConfLib in-process, enumerates one render
   and one capture device and performs GET/SET round-trips. Verified on hardware
   (`--probe`: capabilities eq/effects/mic_dsp true, live band state read, EQ enable
   write/read-back/restore passed). The endpoint property-store path remains as a
   fallback when the vendor DLL is absent.
2. **HID command writes now route per collection.** Hardware probing showed report 85
   belongs to the 0xFF90 collection and report 65 to 0xFF82; the worker selects the
   collection that declares the report id instead of always using the command channel.
3. **Routine page** follows the corrected vendor composition (hero + 3 columns,
   including the previously missing surround-amplification toggle and mic volume/mute),
   but does not reproduce the original hero *image* (CDN asset, not shipped).
4. **Connect/home page** is a native device card rather than the original remote iframe
   web page; this is a deliberate parity trade-off documented in the README.
5. **Firmware display** shows the raw feature-report version word (`0x00000204`) because
   the mapping to the published version strings could not be verified on the unit.
