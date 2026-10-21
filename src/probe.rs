//! Hardware probe (`--probe`): validates the HID protocol and the audio backend
//! against the connected device, printing a JSON report.
//!
//! This is the executable form of the runtime verification described in
//! `mchose-hub-re/docs/findings-runtime.md`: it enumerates the HID topology,
//! reads firmware versions from feature report 170, sends the unframed 0x65
//! battery request and the framed battery query across the vendor report ids,
//! and dumps the audio backend capabilities/state.
//!
//! It never flashes firmware, resets the device, or changes audio settings.

use std::ffi::CString;
use std::time::{Duration, Instant};

use hidapi::{HidApi, HidDevice};
use serde_json::{json, Value};

use crate::backend::protocol;
use crate::model::device::family_for;

/// Report id used by the vendor renderer for the legacy headset family.
const VENDOR_REPORT_ID: u8 = 85;
/// Report id of the 0xFF82 command collection (input+output).
const COMMAND_REPORT_ID: u8 = 65;

/// Run the probe, print the JSON report and write it to the app data directory.
pub fn run() -> Result<(), String> {
    let api = HidApi::new().map_err(|err| format!("hidapi init failed: {err}"))?;

    let mut collections = Vec::new();
    for info in api.device_list() {
        let vid = info.vendor_id();
        let pid = info.product_id();
        if family_for(vid, pid).is_none() {
            continue;
        }
        collections.push(json!({
            "path": info.path().to_string_lossy(),
            "vendor_id": format!("0x{vid:04X}"),
            "product_id": format!("0x{pid:04X}"),
            "usage_page": info.usage_page(),
            "usage": info.usage(),
            "product": info.product_string(),
            "manufacturer": info.manufacturer_string(),
            "serial": info.serial_number(),
        }));
    }

    // Pick the collection paths by usage page.
    let mut command_path = None;
    let mut notify_path = None;
    let mut firmware_path = None;
    for info in api.device_list() {
        if family_for(info.vendor_id(), info.product_id()).is_none() {
            continue;
        }
        let path = info.path().to_string_lossy().to_string();
        match info.usage_page() {
            0xFF82 => command_path = Some(path),
            0xFF90 => notify_path = Some(path),
            0xFF22 => firmware_path = Some(path),
            _ => {}
        }
    }

    let mut report = json!({
        "collections": collections,
        "command_path": command_path,
        "notify_path": notify_path,
        "firmware_path": firmware_path,
    });

    // Firmware version via feature report 170 on the 0xFF22 collection.
    if let Some(path) = firmware_path.as_ref() {
        match open(&api, path) {
            Ok(device) => {
                let mut firmware = serde_json::Map::new();
                for (target, label) in [(0u8, "headset"), (1u8, "dongle")] {
                    firmware.insert(label.to_string(), read_firmware(&device, target));
                }
                report["firmware"] = Value::Object(firmware);
            }
            Err(err) => report["firmware_error"] = json!(err),
        }
    }

    // Battery query matrix: both vendor report ids on both collections, reading
    // replies from both channels afterwards.
    let command = command_path
        .as_ref()
        .and_then(|path| open(&api, path).ok());
    let notify = notify_path
        .as_ref()
        .and_then(|path| open(&api, path).ok());

    let mut attempts = Vec::new();
    if let Some(device) = command.as_ref() {
        attempts.push(query_matrix("command", device, notify.as_ref(), COMMAND_REPORT_ID));
        attempts.push(query_matrix("command", device, notify.as_ref(), VENDOR_REPORT_ID));
    }
    if let Some(device) = notify.as_ref() {
        attempts.push(query_matrix("notify", device, command.as_ref(), VENDOR_REPORT_ID));
        attempts.push(query_matrix("notify", device, command.as_ref(), COMMAND_REPORT_ID));
    }
    report["battery_queries"] = json!(attempts);

    // Unsolicited notifications on the notify collection.
    if let Some(device) = notify.as_ref() {
        report["notify_frames"] = json!(listen(device, Duration::from_millis(1200)));
    }

    // Short-report hypothesis: the vendor renderer sends a 15-byte WebHID report
    // (`11 00 ...`) which becomes a 16-byte buffer including the report id here.
    if let Some(device) = notify.as_ref() {
        let mut short = Vec::new();
        for (label, payload) in [
            ("cmd11_15", {
                let mut data = vec![0u8; 15];
                data[0] = 0x11;
                data
            }),
            ("raw65_15", {
                let mut data = vec![0u8; 15];
                data[0] = 0x65;
                data[1] = 0x01;
                data
            }),
        ] {
            let mut buf = vec![0u8; payload.len() + 1];
            buf[0] = 85;
            buf[1..].copy_from_slice(&payload);
            match device.write(&buf) {
                Ok(n) => {
                    let frames = collect(device, None, Duration::from_millis(1800));
                    short.push(json!({ "label": label, "write": format!("{n} bytes"), "read": frames }));
                }
                Err(err) => short.push(json!({ "label": label, "write_error": err.to_string() })),
            }
        }
        report["short_reports"] = json!(short);
    }

    // Audio backend capability/state snapshot (property store + Core Audio).
    let mut backend = crate::backend::open_audio_backend();
    let caps = backend.capabilities();
    let state = backend.read();
    report["audio"] = json!({
        "backend": format!("{:?}", backend.kind()),
        "capabilities": caps,
        "state": match &state {
            Ok(state) => json!(state),
            Err(err) => json!(err.to_string()),
        },
        "endpoint": backend.endpoint_name(),
    });

    // DSP write verification: toggle the EQ enable flag, read it back, restore it.
    // This proves the ConfLib SET path works end-to-end without changing any tone.
    if let Ok(initial) = &state {
        let target = !initial.eq_enabled;
        let apply_result = backend.apply(crate::model::audio::AudioChange::EqEnabled(target));
        let read_back = backend.read().map(|state| state.eq_enabled);
        let restore = backend.apply(crate::model::audio::AudioChange::EqEnabled(initial.eq_enabled));
        report["dsp_write_test"] = json!({
            "initial": initial.eq_enabled,
            "target": target,
            "apply_ok": apply_result.is_ok(),
            "apply_error": apply_result.err().map(|err| err.to_string()),
            "read_back": read_back.ok(),
            "restore_ok": restore.is_ok(),
        });
    }

    // Alternative property-store routes (diagnostic for the C-Media FX store).
    #[cfg(windows)]
    {
        report["store_diagnostics"] = crate::backend::winaudio::diagnose_stores();
    }

    let text = serde_json::to_string_pretty(&report).map_err(|err| err.to_string())?;
    println!("{text}");
    let path = crate::config::data_dir().join("probe-report.json");
    let _ = std::fs::write(&path, &text);
    log::info!("probe report written to {}", path.display());
    Ok(())
}

fn open(api: &HidApi, path: &str) -> Result<HidDevice, String> {
    let c_path = CString::new(path).map_err(|err| err.to_string())?;
    api.open_path(&c_path).map_err(|err| err.to_string())
}

fn read_firmware(device: &HidDevice, target: u8) -> Value {
    let request = protocol::firmware_request(target);
    let mut write_buf = [0u8; 64];
    write_buf[0] = protocol::FEATURE_FIRMWARE;
    write_buf[1..].copy_from_slice(&request);
    if let Err(err) = device.send_feature_report(&write_buf) {
        return json!({ "write_error": err.to_string() });
    }
    std::thread::sleep(Duration::from_millis(300));
    let mut read_buf = [0u8; 65];
    read_buf[0] = protocol::FEATURE_FIRMWARE;
    match device.get_feature_report(&mut read_buf) {
        Ok(n) => {
            let hex = hex(&read_buf[..n]);
            log::info!("firmware feature report (target {target}): {hex}");
            json!({
                "raw_hex": hex,
                "decoded": protocol::decode_firmware_reply(&read_buf[..n]),
            })
        }
        Err(err) => json!({ "read_error": err.to_string() }),
    }
}

/// Send one battery query variant and collect replies from both channels.
fn query_matrix(
    label: &str,
    writer: &HidDevice,
    reader_other: Option<&HidDevice>,
    report_id: u8,
) -> Value {
    let mut result = json!({ "collection": label, "report_id": report_id });

    // 1. Unframed vendor request (0x65 0x01 ...).
    let payload = protocol::battery_request();
    result["unframed"] = write_probe(writer, reader_other, report_id, &payload);

    // 2. Framed GET_BATTERY_SLEEP (cmd 11).
    let frame = protocol::Frame::request(protocol::cmd::GET_BATTERY_SLEEP, Vec::new());
    result["framed"] = write_probe(writer, reader_other, report_id, &frame.encode());
    result
}

fn write_probe(
    writer: &HidDevice,
    reader_other: Option<&HidDevice>,
    report_id: u8,
    payload: &[u8; 63],
) -> Value {
    let mut buf = [0u8; 64];
    buf[0] = report_id;
    buf[1..].copy_from_slice(payload);
    match writer.write(&buf) {
        Ok(n) => {
            let frames = collect(writer, reader_other, Duration::from_millis(2500));
            json!({ "write": format!("{n} bytes"), "read": frames })
        }
        Err(err) => json!({ "write_error": err.to_string() }),
    }
}

/// Read frames from the writer and the other collection for `duration`.
fn collect(writer: &HidDevice, other: Option<&HidDevice>, duration: Duration) -> Value {
    let start = Instant::now();
    let mut groups: Vec<Value> = Vec::new();
    while start.elapsed() < duration && groups.len() < 8 {
        let mut frames = Vec::new();
        let mut buf = [0u8; 128];
        if let Ok(n) = writer.read_timeout(&mut buf, 60) {
            if n > 0 {
                frames.push(format!("self: {}", hex(&buf[..n])));
            }
        }
        if let Some(other) = other {
            let mut buf = [0u8; 128];
            if let Ok(n) = other.read_timeout(&mut buf, 60) {
                if n > 0 {
                    frames.push(format!("other: {}", hex(&buf[..n])));
                }
            }
        }
        if !frames.is_empty() {
            groups.push(json!(frames));
        }
    }
    json!(groups)
}

fn listen(device: &HidDevice, duration: Duration) -> Vec<String> {
    let start = Instant::now();
    let mut frames = Vec::new();
    while start.elapsed() < duration && frames.len() < 16 {
        let mut buf = [0u8; 128];
        match device.read_timeout(&mut buf, 100) {
            Ok(0) => {}
            Ok(n) => frames.push(hex(&buf[..n])),
            Err(_) => break,
        }
    }
    frames
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}
