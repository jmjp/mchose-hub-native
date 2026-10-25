//! Bluetooth battery/status integration.
//!
//! The V9 PRO can also connect over Classic Bluetooth. On Linux, BlueZ exposes
//! the battery through `org.bluez.Battery1` and `bluetoothctl info` prints it; on
//! Windows the PnP device property `DEVPKEY_Bluetooth_Battery` carries it. macOS
//! exposes it through `system_profiler` for connected audio devices.
//!
//! Only the battery/connection state is available over Bluetooth: the vendor's
//! hardware EQ profiles and the DSP controls require the 2.4G dongle.

use std::process::Command;

/// Battery/status of a connected MCHOSE headset on Bluetooth.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BluetoothStatus {
    pub name: String,
    pub percent: u8,
    pub charging: bool,
}

/// Query the currently connected MCHOSE headset over Bluetooth.
pub fn query() -> Option<BluetoothStatus> {
    imp()
}

fn is_mchose(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("mchose") || lower.contains("v9 pro") || lower.contains("v9_pro")
}

#[cfg(target_os = "linux")]
fn imp() -> Option<BluetoothStatus> {
    let output = Command::new("bluetoothctl")
        .args(["devices", "Connected"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        // "Device AA:BB:CC:DD:EE:FF MCHOSE V9 PRO"
        let Some(rest) = line.strip_prefix("Device ") else {
            continue;
        };
        let mut parts = rest.splitn(2, ' ');
        let address = parts.next().unwrap_or_default();
        let name = parts.next().unwrap_or_default().trim();
        if name.is_empty() || !is_mchose(name) {
            continue;
        }
        let info = Command::new("bluetoothctl").args(["info", address]).output().ok()?;
        let info_text = String::from_utf8_lossy(&info.stdout);
        if let Some(percent) = parse_battery_percentage(&info_text) {
            let charging = info_text
                .lines()
                .any(|l| l.trim().eq_ignore_ascii_case("Charging: yes"));
            return Some(BluetoothStatus {
                name: name.to_string(),
                percent,
                charging,
            });
        }
    }
    None
}

/// Parse `Battery Percentage: 0x55 (85)` / `Battery Percentage: 85` lines.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn parse_battery_percentage(text: &str) -> Option<u8> {
    for line in text.lines() {
        if !line.contains("Battery Percentage:") {
            continue;
        }
        let value = line.rsplit(':').next()?.trim();
        if let Some(open) = value.rfind('(') {
            if let Some(close) = value[open..].find(')') {
                if let Ok(percent) = value[open + 1..open + close].trim().parse::<u8>() {
                    return Some(percent.min(100));
                }
            }
        }
        if let Ok(percent) = value.trim_end_matches('%').parse::<u8>() {
            return Some(percent.min(100));
        }
    }
    None
}

#[cfg(windows)]
fn imp() -> Option<BluetoothStatus> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let script = r#"$ErrorActionPreference='SilentlyContinue'; Get-PnpDevice -Class Bluetooth -PresentOnly | ForEach-Object { $b=(Get-PnpDeviceProperty -InstanceId $_.InstanceId -KeyName 'DEVPKEY_Bluetooth_Battery' -ErrorAction SilentlyContinue).Data; if ($null -ne $b) { "$($_.FriendlyName)|$b" } }"#;
    let output = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        let Some((name, value)) = line.rsplit_once('|') else {
            continue;
        };
        if !is_mchose(name) {
            continue;
        }
        if let Ok(percent) = value.trim().parse::<u8>() {
            return Some(BluetoothStatus {
                name: name.trim().to_string(),
                percent: percent.min(100),
                charging: false,
            });
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn imp() -> Option<BluetoothStatus> {
    let output = Command::new("system_profiler")
        .args(["SPBluetoothDataType", "-json"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    // Keep this dependency-free: look for the device name and the nearest
    // battery percentage in the JSON text.
    for chunk in text.split('"') {
        if is_mchose(chunk) {
            let tail = &text[text.find(chunk).unwrap_or(0)..];
            if let Some(index) = tail.find("Battery Level") {
                let rest = &tail[index..];
                if let Some(percent_index) = rest.find('%') {
                    let digits: String = rest[..percent_index]
                        .chars()
                        .rev()
                        .take_while(|c| c.is_ascii_digit())
                        .collect::<String>()
                        .chars()
                        .rev()
                        .collect();
                    if let Ok(percent) = digits.parse::<u8>() {
                        return Some(BluetoothStatus {
                            name: chunk.to_string(),
                            percent: percent.min(100),
                            charging: false,
                        });
                    }
                }
            }
        }
    }
    None
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
fn imp() -> Option<BluetoothStatus> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bluez_battery_formats() {
        assert_eq!(
            parse_battery_percentage("Battery Percentage: 0x55 (85)"),
            Some(85)
        );
        assert_eq!(parse_battery_percentage("Battery Percentage: 72"), Some(72));
        assert_eq!(parse_battery_percentage("nothing here"), None);
    }

    #[test]
    fn recognizes_mchose_names() {
        assert!(is_mchose("MCHOSE V9 PRO"));
        assert!(is_mchose("V9 PRO"));
        assert!(!is_mchose("WH-1000XM5"));
    }
}
