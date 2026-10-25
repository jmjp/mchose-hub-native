//! MCHOSE HID frame protocol (legacy headset family).
//!
//! Recovered from the original application's renderer bundle
//! (`cdn.mchose.com.cn/customPage/prod/assets/*`, file `crc32-*.js`) by the
//! renderer analysis in `findings-renderer.md` §10, and independently described
//! by the user's earlier WebHID driver reconstruction (`findings-prior-art.md`).
//!
//! Every command travels in a **63-byte report payload** (64-byte HID report
//! including the report id byte, which hidapi handles separately):
//!
//! ```text
//! [0]      0xAA                  frame start
//! [1]      0x01                  protocol version
//! [2]      flags                 bit0 = checksum present (vendor sends 0)
//! [3]      length                payload byte count + 1 (the cmd byte)
//! [4..6]   cmdType[0], cmdType[1]  [0,1] request/response, [0,2] set,
//!                                  [0,4] notify, [0,3] image, [0,6] OTA request
//! [6]      cmd                   command id
//! [7..]    payload
//! [7+n]    checksum              XOR of flags..payload (unused while flags == 0)
//! ```
//!
//! The report id used by the V9 PRO family is 85 (`0x55`). Other families use
//! different ids (188 for K20, 170 for G9 V2) but share the frame layout.

use crate::model::device::BatteryState;

/// Payload size of a command report (report id excluded).
pub const REPORT_DATA_LEN: usize = 63;
/// Frame start marker.
pub const FRAME_START: u8 = 0xAA;
/// Protocol version byte.
pub const FRAME_VERSION: u8 = 1;
/// HID report id used by the V9 PRO / legacy headset family.
pub const REPORT_ID_V9: u8 = 85;

/// Command-type pairs used in bytes 4..6.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmdType {
    /// Host request / device response (`[0, 1]`).
    Request,
    /// Host set (`[0, 2]`).
    Set,
    /// Device notification (`[0, 4]`).
    Notify,
    /// Image upload (`[0, 3]`).
    UpImg,
    /// OTA request (`[0, 6]`).
    OtaRequest,
}

impl CmdType {
    pub fn pair(self) -> [u8; 2] {
        match self {
            CmdType::Request => [0, 1],
            CmdType::Set => [0, 2],
            CmdType::Notify => [0, 4],
            CmdType::UpImg => [0, 3],
            CmdType::OtaRequest => [0, 6],
        }
    }

    pub fn from_pair(pair: [u8; 2]) -> Option<CmdType> {
        match pair {
            [0, 1] => Some(CmdType::Request),
            [0, 2] => Some(CmdType::Set),
            [0, 4] => Some(CmdType::Notify),
            [0, 3] => Some(CmdType::UpImg),
            [0, 6] => Some(CmdType::OtaRequest),
            _ => None,
        }
    }
}

/// Command ids of the legacy headset protocol.
pub mod cmd {
    /// Set equalizer mode (legacy: 0..=2).
    pub const SET_EQ_MODE: u8 = 1;
    /// Read equalizer state.
    pub const GET_EQ_STATE: u8 = 4;
    /// Read battery / sleep state.
    pub const GET_BATTERY_SLEEP: u8 = 11;
    /// Read the current equalizer mode.
    pub const GET_CURRENT_EQ_MODE: u8 = 12;
    /// Read the voice-prompt volume (1..=16).
    pub const GET_PROMPT_VOLUME: u8 = 13;
    /// Read the auto power-off timeout (minutes, 0 = never).
    pub const GET_AUTO_SHUTDOWN: u8 = 14;
    /// Write the full parametric EQ (new-layout devices).
    pub const SET_PEQ_ALL: u8 = 16;
    /// Write the parametric EQ master gain (new-layout devices).
    pub const SET_PEQ_ALL_GAIN: u8 = 18;
    /// Restore an EQ mode to its default curve.
    pub const RESTORE_EQ_MODE: u8 = 25;
    /// Set the auto power-off timeout.
    pub const SET_AUTO_SHUTDOWN: u8 = 26;
    /// Factory reset. Payload target: 1 headset, 2 dongle, 3 all.
    pub const FACTORY_RESET: u8 = 27;
    /// Set the voice-prompt volume.
    pub const SET_PROMPT_VOLUME: u8 = 33;
}

/// Device notification command ids.
pub mod notify {
    /// Battery / sleep status (`BatterySleep` notify).
    pub const BATTERY_SLEEP: u8 = 34;
    /// Older battery notification id (same payload as [`BATTERY_SLEEP`]).
    pub const BATTERY: u8 = 11;
    /// Equalizer mode changed.
    pub const EQ_MODE: u8 = 1;
}

/// Unframed vendor battery request marker (first payload byte).
pub const BATTERY_MARKER: u8 = 0x65;
/// Feature report id that carries the firmware-version string.
pub const FEATURE_FIRMWARE: u8 = 170;
/// First payload byte of a firmware feature report request.
pub const FIRMWARE_REQUEST: u8 = 0x01;

/// Battery charge-state byte values used by the unframed vendor reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChargeStatus {
    Discharging,
    Charging,
    Full,
    Sleep,
    Unknown(u8),
}

impl ChargeStatus {
    pub fn from_byte(value: u8) -> Self {
        match value {
            2 => ChargeStatus::Discharging,
            3 => ChargeStatus::Charging,
            4 => ChargeStatus::Full,
            26 => ChargeStatus::Sleep,
            other => ChargeStatus::Unknown(other),
        }
    }

    pub fn is_charging(self) -> bool {
        matches!(self, ChargeStatus::Charging)
    }

    pub fn is_asleep(self) -> bool {
        matches!(self, ChargeStatus::Sleep)
    }
}

/// Build the unframed battery/status request sent on the command report id.
///
/// Vendor layout (see `findings-prior-art.md` §4): `[report_id, 0x65, 0x01, 0x00 * 61]`.
pub fn battery_request() -> [u8; REPORT_DATA_LEN] {
    let mut payload = [0u8; REPORT_DATA_LEN];
    payload[0] = BATTERY_MARKER;
    payload[1] = 0x01;
    payload
}

/// Decode the unframed battery reply (payload with the report-id byte stripped).
///
/// `payload[0] == 0x65`, `payload[1]` = level percent, `payload[2]` = charge state.
pub fn decode_battery_reply(payload: &[u8]) -> Option<(BatteryState, ChargeStatus)> {
    if payload.len() < 3 || payload[0] != BATTERY_MARKER {
        return None;
    }
    let percent = payload[1];
    let status = ChargeStatus::from_byte(payload[2]);
    let charging = status.is_charging();
    Some((
        BatteryState {
            percent,
            charging,
        },
        status,
    ))
}

/// Unframed vendor EQ-profile query marker (reply carries the active profile).
pub const EQ_QUERY_MARKER: u8 = 0x11;
/// Unframed vendor EQ-profile set marker.
pub const EQ_SET_MARKER: u8 = 0x21;
/// Hardware EQ profiles exposed by the headset DSP (Game 1, Game 2, Music).
pub const EQ_PROFILE_COUNT: u8 = 3;

/// Build the unframed vendor EQ-profile query (`[0x11, 0 * 62]`).
pub fn eq_query_request() -> [u8; REPORT_DATA_LEN] {
    let mut payload = [0u8; REPORT_DATA_LEN];
    payload[0] = EQ_QUERY_MARKER;
    payload
}

/// Build the unframed vendor EQ-profile set request (`[0x21, 0x01, mode, ...]`).
pub fn eq_set_request(mode: u8) -> [u8; REPORT_DATA_LEN] {
    let mut payload = [0u8; REPORT_DATA_LEN];
    payload[0] = EQ_SET_MARKER;
    payload[1] = 0x01;
    payload[2] = mode.min(EQ_PROFILE_COUNT - 1);
    payload
}

/// A reply received on the unframed vendor channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VendorReply {
    /// Battery level and charge state (`0x65` reply).
    Battery {
        battery: BatteryState,
        status: ChargeStatus,
    },
    /// Hardware EQ profile (`0x11`/`0x21` reply).
    EqMode(u8),
}

/// Decode an unframed vendor reply (report id already stripped).
///
/// * battery reply: `[0x65, percent, charge_status, ...]`
/// * EQ reply: `[0x11, mode, ...]` (some firmware echoes `[0x21, 0x01, mode, ...]`)
pub fn decode_vendor_reply(payload: &[u8]) -> Option<VendorReply> {
    match payload.first().copied()? {
        BATTERY_MARKER => {
            if payload.len() < 3 {
                return None;
            }
            let status = ChargeStatus::from_byte(payload[2]);
            Some(VendorReply::Battery {
                battery: BatteryState {
                    percent: payload[1],
                    charging: status.is_charging(),
                },
                status,
            })
        }
        EQ_QUERY_MARKER => payload.get(1).copied().map(VendorReply::EqMode),
        EQ_SET_MARKER => {
            // Set replies observed as [0x21, 0x01, mode]; accept both layouts.
            let mode = if payload.get(1) == Some(&0x01) {
                payload.get(2).copied()
            } else {
                payload.get(1).copied()
            };
            mode.map(VendorReply::EqMode)
        }
        _ => None,
    }
}

/// Build the feature-report request used to read firmware version strings.
///
/// `target`: 0 = headset MCU, 1 = dongle/receiver.
pub fn firmware_request(target: u8) -> [u8; REPORT_DATA_LEN] {
    let mut payload = [0u8; REPORT_DATA_LEN];
    payload[0] = FIRMWARE_REQUEST;
    payload[1] = target;
    payload
}

/// Parse a firmware feature report reply (`[0xAA, 0x01, d0, d1, d2, d3, ...]`).
///
/// On the tested V9 PRO the four version bytes are not ASCII digits
/// (headset: `00 00 02 04`, dongle: `00 00 00 04`), so the raw version word is
/// returned as `0x`-prefixed hex. ASCII-encoded versions are passed through.
pub fn decode_firmware_reply(payload: &[u8]) -> Option<String> {
    if payload.len() < 6 || payload[0] != FEATURE_FIRMWARE || payload[1] != 0x01 {
        return None;
    }
    let version = &payload[2..6];
    if version.iter().all(|b| b.is_ascii_graphic() || *b == b' ') {
        let text: String = version.iter().map(|b| *b as char).collect();
        let trimmed = text.trim().to_string();
        if !trimmed.is_empty() && trimmed.chars().any(|c| c.is_ascii_alphanumeric()) {
            return Some(trimmed);
        }
    }
    // Non-ASCII layout on the V9 PRO: the 16-bit version word is big-endian in the
    // last two bytes (dongle `00 00 00 04` -> "0004", matching the vendor UI; the
    // headset unit reports `00 00 02 04` -> "0204").
    let word = u16::from_be_bytes([version[2], version[3]]);
    Some(format!("{word:04X}"))
}

/// Build the on-device parametric-EQ gain payload (new-layout devices only).
///
/// `[mode, pointCount, gain_i16_be ...]`, gains in 0.1 dB steps. Only used by
/// new-layout families; the V9 PRO applies EQ host-side.
pub fn peq_gains_payload(mode: u8, gains_db: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(2 + gains_db.len() * 2);
    out.push(mode);
    out.push(gains_db.len() as u8);
    for gain in gains_db {
        let clamped = gain.clamp(-12.0, 12.0);
        let raw = (clamped * 10.0).round() as i16;
        out.extend_from_slice(&raw.to_be_bytes());
    }
    out
}

/// Factory-reset targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetTarget {
    Headset = 1,
    Dongle = 2,
    All = 3,
}

/// A protocol frame (without the HID report id).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub cmd_type: CmdType,
    pub cmd: u8,
    pub payload: Vec<u8>,
}

/// Frame decoding errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FrameError {
    #[error("report is too short")]
    TooShort,
    #[error("bad frame start marker")]
    BadStart,
    #[error("unsupported frame version")]
    BadVersion,
    #[error("declared length exceeds report size")]
    BadLength,
    #[error("unknown command type pair")]
    BadCmdType,
    #[error("checksum mismatch")]
    BadChecksum,
}

impl Frame {
    pub fn new(cmd_type: CmdType, cmd: u8, payload: Vec<u8>) -> Self {
        Self {
            cmd_type,
            cmd,
            payload,
        }
    }

    /// Convenience constructor for a plain host request.
    pub fn request(cmd: u8, payload: Vec<u8>) -> Self {
        Self::new(CmdType::Request, cmd, payload)
    }

    /// Convenience constructor for a host set operation.
    pub fn set(cmd: u8, payload: Vec<u8>) -> Self {
        Self::new(CmdType::Set, cmd, payload)
    }

    /// XOR checksum exactly as the vendor renderer computes it.
    pub fn checksum(&self) -> u8 {
        self.checksum_inner()
    }

    fn checksum_inner(&self) -> u8 {
        let mut acc = 0u8;
        acc ^= 0u8; // flags are always 0 for host-originated frames
        acc ^= (self.payload.len() as u8).wrapping_add(1);
        let [a, b] = self.cmd_type.pair();
        acc ^= a;
        acc ^= b;
        acc ^= self.cmd;
        for byte in &self.payload {
            acc ^= *byte;
        }
        acc
    }

    /// Serialize into the 63-byte report payload.
    pub fn encode(&self) -> [u8; REPORT_DATA_LEN] {
        let mut out = [0u8; REPORT_DATA_LEN];
        let payload = &self.payload[..self.payload.len().min(REPORT_DATA_LEN - 8)];
        out[0] = FRAME_START;
        out[1] = FRAME_VERSION;
        out[2] = 0; // checksum flag disabled, matching the vendor's host frames
        out[3] = payload.len() as u8 + 1;
        let [a, b] = self.cmd_type.pair();
        out[4] = a;
        out[5] = b;
        out[6] = self.cmd;
        out[7..7 + payload.len()].copy_from_slice(payload);
        out[7 + payload.len()] = self.checksum_inner();
        out
    }

    /// Parse a 63-byte report payload (report id already stripped).
    pub fn decode(data: &[u8]) -> Result<Frame, FrameError> {
        if data.len() < 8 {
            return Err(FrameError::TooShort);
        }
        if data[0] != FRAME_START {
            return Err(FrameError::BadStart);
        }
        if data[1] != FRAME_VERSION {
            return Err(FrameError::BadVersion);
        }
        let flags = data[2];
        let declared = data[3] as usize;
        if declared < 1 || 7 + declared > data.len() {
            return Err(FrameError::BadLength);
        }
        let cmd_type = CmdType::from_pair([data[4], data[5]]).ok_or(FrameError::BadCmdType)?;
        let cmd = data[6];
        let payload_len = declared - 1;
        let payload = data[7..7 + payload_len].to_vec();
        let frame = Frame {
            cmd_type,
            cmd,
            payload,
        };
        if flags & 1 == 1 {
            let expected = data[7 + payload_len];
            if frame.checksum_inner() != expected {
                return Err(FrameError::BadChecksum);
            }
        }
        Ok(frame)
    }
}

impl BatteryState {
    /// Decode a `BatterySleep` payload: `[sleep_state, charging<<7 | percent]`.
    ///
    /// The sleep byte semantics are not fully documented by the vendor; the
    /// renderer treats a non-zero state as "asleep". Unverified.
    pub fn from_notify_payload(payload: &[u8]) -> Option<(BatteryState, bool)> {
        if payload.len() < 2 {
            return None;
        }
        let asleep = payload[0] != 0;
        let raw = payload[1];
        let charging = raw & 0x80 != 0;
        let percent = raw & 0x7F;
        Some((
            BatteryState {
                percent,
                charging,
            },
            asleep,
        ))
    }
}

/// Encode a prompt-volume set payload (clamped to the vendor's 1..=16 range).
pub fn prompt_volume_payload(value: u8) -> Vec<u8> {
    vec![value.clamp(1, 16)]
}

/// Encode an auto-shutdown set payload (minutes, little-endian u16).
pub fn auto_shutdown_payload(minutes: u16) -> Vec<u8> {
    vec![(minutes & 0xFF) as u8, (minutes >> 8) as u8]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_frame_layout_matches_vendor_example() {
        // The vendor renderer builds: AA 01 00 01 00 01 0C 00 ... for GET_CURRENT_EQ_MODE(12).
        let frame = Frame::request(cmd::GET_CURRENT_EQ_MODE, vec![]);
        let bytes = frame.encode();
        assert_eq!(bytes[0], 0xAA);
        assert_eq!(bytes[1], 0x01);
        assert_eq!(bytes[2], 0x00);
        assert_eq!(bytes[3], 0x01);
        assert_eq!(&bytes[4..7], &[0, 1, 12]);
        assert_eq!(bytes.len(), 63);
    }

    #[test]
    fn set_frame_roundtrip() {
        let frame = Frame::set(cmd::SET_PROMPT_VOLUME, prompt_volume_payload(9));
        let encoded = frame.encode();
        let decoded = Frame::decode(&encoded).expect("valid frame");
        assert_eq!(decoded, frame);
        assert_eq!(decoded.payload, vec![9]);
    }

    #[test]
    fn checksum_flag_is_validated_when_present() {
        let frame = Frame::set(cmd::SET_AUTO_SHUTDOWN, auto_shutdown_payload(30));
        let mut encoded = frame.encode();
        encoded[2] = 1; // ask for checksum validation
        let decoded = Frame::decode(&encoded).expect("valid checksum");
        assert_eq!(decoded.cmd, cmd::SET_AUTO_SHUTDOWN);

        encoded[8] ^= 0xFF; // corrupt payload
        assert_eq!(Frame::decode(&encoded), Err(FrameError::BadChecksum));
    }

    #[test]
    fn battery_notify_decodes_charging_and_percent() {
        let (battery, asleep) = BatteryState::from_notify_payload(&[0, 0x80 | 64]).unwrap();
        assert_eq!(battery.percent, 64);
        assert!(battery.charging);
        assert!(!asleep);
    }

    #[test]
    fn vendor_battery_reply_decodes_level_and_charge_state() {
        let reply = protocol_reply(&[0x65, 87, 3]);
        assert_eq!(
            reply,
            Some(VendorReply::Battery {
                battery: BatteryState {
                    percent: 87,
                    charging: true
                },
                status: ChargeStatus::Charging,
            })
        );
    }

    #[test]
    fn vendor_eq_reply_decodes_profile() {
        assert_eq!(decode_vendor_reply(&[0x11, 2]), Some(VendorReply::EqMode(2)));
        assert_eq!(
            decode_vendor_reply(&[0x21, 0x01, 1]),
            Some(VendorReply::EqMode(1))
        );
    }

    #[test]
    fn eq_set_request_carries_profile() {
        let request = eq_set_request(2);
        assert_eq!(&request[..3], &[0x21, 0x01, 2]);
        assert_eq!(eq_query_request()[0], 0x11);
    }

    fn protocol_reply(bytes: &[u8]) -> Option<VendorReply> {
        decode_vendor_reply(bytes)
    }

    #[test]
    fn long_payload_is_truncated_to_report_size() {
        let frame = Frame::request(cmd::SET_PEQ_ALL, vec![0u8; 200]);
        let encoded = frame.encode();
        assert_eq!(encoded[3] as usize, (REPORT_DATA_LEN - 8) + 1);
    }
}
