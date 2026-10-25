# Battery, hardware EQ profiles and Bluetooth

These features were added after comparing the native app with the community Linux
controller `JoaoKSS/MCHOSE_v9_PRO_Controller` (Python/PyQt6). The vendor commands
were reimplemented independently in Rust; no code was copied (the reference project
has no license).

## Vendor HID commands (unframed, report id 0x55)

| Purpose | Request (64-byte report) | Reply |
| --- | --- | --- |
| Battery + charge status | `55 65 01` + zeros | `55 65 <percent> <status>` (status 3 = charging, 4 = full, 26 = sleep) |
| Read hardware EQ profile | `55 11` + zeros | `55 11 <mode>` (0 = Game 1, 1 = Game 2, 2 = Music) |
| Set hardware EQ profile | `55 21 01 <mode>` + zeros | echoed/`55 11` read-back |

The commands are sent on the notification collection (usage page `0xFF90`, report id
`0x55`). The app polls them every 5 s and on connect. `src/backend/protocol.rs`
contains the encoders/decoders and unit tests; `src/backend/hid.rs` routes the replies.

**Verification state:** the framed protocol and firmware feature report work on the
tested Windows unit, but the unframed battery/EQ queries received no reply there
(Windows hidapi, all report ids/collections). The community project targets Linux
hidraw, so the feature is expected to work on Linux; the UI degrades to
"unknown"/no active profile when the device stays silent. A live-USB Linux test is the
remaining verification step.

## Hardware EQ profiles

The headset DSP stores three profiles (Game 1, Game 2, Music), independent of the
Windows C-Media Equalizer APO. The Equalizer page exposes them as a card at the top;
they are enabled only with the 2.4G dongle (`ConnectionMode::Wireless24G`), matching the
vendor behaviour (Bluetooth does not support hardware EQ switches).

## Bluetooth

When no USB/2.4G HID device is open, the app polls for a Bluetooth-connected MCHOSE
headset:

- **Linux:** `bluetoothctl devices Connected` + `bluetoothctl info <MAC>`; parses
  `Battery Percentage: 0x55 (85)` and `Charging: yes`.
- **Windows:** enumerates present Bluetooth PnP devices and reads
  `DEVPKEY_Bluetooth_Battery` through PowerShell in a background thread.
- **macOS:** best-effort parse of `system_profiler SPBluetoothDataType -json`.

Bluetooth exposes only battery/connection state; hardware EQ and DSP controls are
unavailable (same limitation as the vendor app).

## Audio fallback

The portable backend now tries PipeWire (`wpctl`), PulseAudio (`pactl`) and finally
ALSA (`amixer sget/sset Master|Capture`) for volume/mute on Linux.
