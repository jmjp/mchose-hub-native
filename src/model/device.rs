//! Device identity, supported-family table and live status model.

use serde::{Deserialize, Serialize};

/// MCHOSE USB vendor id (V9 PRO: 10525).
pub const MCHOSE_VID: u16 = 0x291D;

/// Audio DSP runtime used by a device family in the original application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioRuntime {
    /// Legacy C-Media AudioSDK path: 10-band EQ, Xear 7.1, LFX/GFX effect properties.
    CMedia,
    /// New layout: THX spatial audio runtime + on-device parametric EQ over HID.
    Thx,
}

/// A supported MCHOSE device family.
#[derive(Debug, Clone, Copy)]
pub struct DeviceFamily {
    pub vid: u16,
    pub pid: u16,
    /// Marketing/render name used in the UI (matches the original `renderName`).
    pub name: &'static str,
    /// HID report id used for the 0xAA command/notify protocol.
    pub report_id: u8,
    pub audio_runtime: AudioRuntime,
    /// `true` for devices with the newer on-device PEQ ("new layout").
    pub new_layout: bool,
}

impl DeviceFamily {
    pub fn is_verified(&self) -> bool {
        // Only the V9 PRO path has been exercised against the real hardware while
        // writing this reimplementation; the others are table entries for the UI.
        self.vid == MCHOSE_VID && self.pid == 0x385D
    }
}

/// Families known to the original application's device table (partial: audio devices).
pub static FAMILIES: &[DeviceFamily] = &[DeviceFamily {
    vid: MCHOSE_VID,
    pid: 0x385D,
    name: "V9 Pro",
    report_id: 85,
    audio_runtime: AudioRuntime::CMedia,
    new_layout: false,
}];

/// Look up a supported device family by USB ids.
pub fn family_for(vid: u16, pid: u16) -> Option<&'static DeviceFamily> {
    FAMILIES.iter().find(|f| f.vid == vid && f.pid == pid)
}

/// How the device is attached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ConnectionMode {
    Wired,
    #[default]
    Wireless24G,
    Bluetooth,
}

impl ConnectionMode {
    pub fn label_key(self) -> &'static str {
        match self {
            ConnectionMode::Wired => "connect.wired",
            ConnectionMode::Wireless24G => "connect.wireless24g",
            ConnectionMode::Bluetooth => "connect.bluetooth",
        }
    }
}

/// Battery / charging state reported by the device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BatteryState {
    pub percent: u8,
    pub charging: bool,
}

impl BatteryState {
    pub fn is_full(&self) -> bool {
        self.percent >= 100
    }
}

/// Static identity of a connected device.
#[derive(Debug, Clone)]
pub struct DeviceIdentity {
    pub vid: u16,
    pub pid: u16,
    pub family: &'static DeviceFamily,
    pub product_name: String,
    pub manufacturer: Option<String>,
    pub serial: Option<String>,
    pub connection: ConnectionMode,
}

impl DeviceIdentity {
    pub fn display_name(&self) -> &str {
        if self.family.name.is_empty() {
            &self.product_name
        } else {
            self.family.name
        }
    }
}

/// Live device status as shown in the header/sidebar.
#[derive(Debug, Clone, Default)]
pub struct DeviceStatus {
    pub battery: Option<BatteryState>,
    pub asleep: bool,
    /// Headset MCU firmware version (HID feature report / notify).
    pub headset_firmware: Option<String>,
    /// Dongle/receiver firmware version, when reported.
    pub dongle_firmware: Option<String>,
    /// Prompt volume 1..16 (device HID, legacy family).
    pub prompt_volume: Option<u8>,
    /// Auto shutdown in minutes, 0 = never.
    pub auto_shutdown_minutes: Option<u16>,
    /// The connection was last seen `Duration` ago.
    pub last_seen_ms: Option<u64>,
}

impl DeviceStatus {
    pub fn battery_label(&self) -> Option<String> {
        if self.asleep {
            return None;
        }
        self.battery.map(|b| format!("{}%", b.percent))
    }
}

/// Combined snapshot the UI renders.
#[derive(Debug, Clone)]
pub struct DeviceSnapshot {
    pub identity: DeviceIdentity,
    pub status: DeviceStatus,
}

/// Descriptor returned by the HID discovery worker.
#[derive(Debug, Clone)]
pub struct HidDeviceDescriptor {
    pub vid: u16,
    pub pid: u16,
    pub product_name: String,
    pub manufacturer: Option<String>,
    pub serial: Option<String>,
    pub path: String,
}

/// Whether a freshly discovered device should be considered connected.
pub fn is_supported(vid: u16, pid: u16) -> bool {
    family_for(vid, pid).is_some()
}
