//! HID transport worker.
//!
//! Runs `hidapi` on a dedicated thread because HID reads are blocking. The UI
//! communicates through [`HidCommand`] / [`HidEvent`] channels.
//!
//! The V9 PRO exposes one HID interface with six top-level collections; hidapi
//! (like Windows HID) enumerates each collection as a separate device path:
//!
//! * usage page `0xFF82` — command channel (input+output report id 85)
//! * usage page `0xFF90` — notification channel (input report id 85)
//! * usage page `0xFF22` — firmware string (feature report id 170)
//!
//! We therefore keep up to three handles for one physical device.

use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crossbeam_channel::{bounded, Receiver, Sender};
use hidapi::{DeviceInfo, HidApi, HidDevice};

use crate::backend::protocol::{self, Frame, ResetTarget};
use crate::model::device::{family_for, BatteryState, DeviceStatus, HidDeviceDescriptor};

/// Usage pages of the V9 PRO topology.
const USAGE_COMMAND: u16 = 0xFF82;
const USAGE_NOTIFY: u16 = 0xFF90;
const USAGE_FIRMWARE: u16 = 0xFF22;
/// Report id used for commands/notifications on the legacy headset family.
const REPORT_ID_LEGACY: u8 = 85;

/// Requests sent from the UI to the HID worker.
#[derive(Debug, Clone)]
pub enum HidCommand {
    /// Re-enumerate devices.
    Rescan,
    /// Ask the device for battery / sleep status.
    RefreshStatus,
    /// Set the device voice-prompt volume (1..=16).
    SetPromptVolume(u8),
    /// Set the auto power-off timeout in minutes (0 = never).
    SetAutoShutdown(u16),
    /// Factory reset the given target(s).
    FactoryReset(ResetTarget),
    /// Stop the worker.
    Shutdown,
}

/// Events emitted by the HID worker.
#[derive(Debug, Clone)]
pub enum HidEvent {
    /// Enumerated supported devices (deduplicated per physical device).
    Devices(Vec<HidDeviceDescriptor>),
    /// A device was opened successfully.
    Connected {
        descriptor: HidDeviceDescriptor,
        firmware: Option<String>,
    },
    /// The open device disappeared or could not be read.
    Disconnected,
    /// Updated status (battery, prompt volume, ...).
    Status(Box<DeviceStatus>),
    /// Result of a command write.
    CommandResult { cmd: u8, ok: bool },
    /// Human-readable diagnostic message (also forwarded to the log).
    Log(String),
}

/// Handle to the HID worker thread.
pub struct HidService {
    cmd_tx: Sender<HidCommand>,
    evt_rx: Receiver<HidEvent>,
    join: Option<JoinHandle<()>>,
}

impl HidService {
    /// Start the worker thread.
    pub fn start() -> Self {
        let (cmd_tx, cmd_rx) = bounded(32);
        let (evt_tx, evt_rx) = bounded(256);
        let join = thread::Builder::new()
            .name("mchose-hid".into())
            .spawn(move || worker(cmd_rx, evt_tx))
            .expect("spawn hid worker");
        Self {
            cmd_tx,
            evt_rx,
            join: Some(join),
        }
    }

    pub fn send(&self, cmd: HidCommand) {
        if let Err(err) = self.cmd_tx.send(cmd) {
            log::warn!("HID worker is gone: {err}");
        }
    }

    /// Non-blocking drain of pending worker events.
    pub fn drain(&self, max: usize) -> Vec<HidEvent> {
        let mut out = Vec::new();
        while out.len() < max {
            match self.evt_rx.try_recv() {
                Ok(event) => out.push(event),
                Err(_) => break,
            }
        }
        out
    }
}

impl Drop for HidService {
    fn drop(&mut self) {
        let _ = self.cmd_tx.send(HidCommand::Shutdown);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

/// One physical device's collection handles.
struct DeviceHandles {
    descriptor: HidDeviceDescriptor,
    command: Option<HidDevice>,
    notify: Option<HidDevice>,
    firmware: Option<HidDevice>,
    status: DeviceStatus,
    last_poll: Instant,
    last_scan_attempt: Instant,
}

fn worker(cmd_rx: Receiver<HidCommand>, evt_tx: Sender<HidEvent>) {
    let api = match HidApi::new() {
        Ok(api) => api,
        Err(err) => {
            let _ = evt_tx.send(HidEvent::Log(format!("hidapi init failed: {err}")));
            return;
        }
    };

    let mut handles: Option<DeviceHandles> = None;
    let mut last_scan = Instant::now();
    let mut next_status_poll = Instant::now();

    loop {
        // 1. Handle UI commands (non-blocking with a short timeout).
        match cmd_rx.recv_timeout(Duration::from_millis(50)) {
            Ok(HidCommand::Shutdown) | Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                break;
            }
            Ok(HidCommand::Rescan) => {
                handles = None;
                last_scan = Instant::now() - Duration::from_secs(10);
            }
            Ok(HidCommand::RefreshStatus) => {
                if let Some(device) = handles.as_mut() {
                    request_status(&evt_tx, device);
                }
            }
            Ok(HidCommand::SetPromptVolume(value)) => {
                if let Some(device) = handles.as_mut() {
                    let frame = Frame::set(
                        protocol::cmd::SET_PROMPT_VOLUME,
                        protocol::prompt_volume_payload(value),
                    );
                    send_command(&evt_tx, device, frame);
                }
            }
            Ok(HidCommand::SetAutoShutdown(minutes)) => {
                if let Some(device) = handles.as_mut() {
                    let frame = Frame::set(
                        protocol::cmd::SET_AUTO_SHUTDOWN,
                        protocol::auto_shutdown_payload(minutes),
                    );
                    send_command(&evt_tx, device, frame);
                }
            }
            Ok(HidCommand::FactoryReset(target)) => {
                if let Some(device) = handles.as_mut() {
                    let frame = Frame::set(protocol::cmd::FACTORY_RESET, vec![target as u8]);
                    send_command(&evt_tx, device, frame);
                }
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
        }

        // 2. Re-enumerate when no device is open.
        if handles.is_none() && last_scan.elapsed() >= Duration::from_secs(2) {
            last_scan = Instant::now();
            match enumerate(&api) {
                Ok(descriptors) => {
                    let _ = evt_tx.send(HidEvent::Devices(descriptors.clone()));
                    if let Some(descriptor) = descriptors.first() {
                        match open_device(&api, descriptor.clone()) {
                            Ok(device) => {
                                let _ = evt_tx.send(HidEvent::Connected {
                                    descriptor: descriptor.clone(),
                                    firmware: device.status.headset_firmware.clone(),
                                });
                                let _ = evt_tx.send(HidEvent::Status(Box::new(
                                    device.status.clone(),
                                )));
                                next_status_poll = Instant::now();
                                handles = Some(device);
                            }
                            Err(err) => {
                                let _ = evt_tx.send(HidEvent::Log(format!(
                                    "cannot open {}: {err}",
                                    descriptor.product_name
                                )));
                            }
                        }
                    }
                }
                Err(err) => {
                    let _ = evt_tx.send(HidEvent::Log(format!("HID enumeration failed: {err}")));
                }
            }
        }

        // 3. Read notifications and command responses.
        if let Some(device) = handles.as_mut() {
            let mut disconnected = false;
            if let Some(notify) = device.notify.as_mut() {
                let mut buf = [0u8; 128];
                match notify.read_timeout(&mut buf, 20) {
                    Ok(0) => {}
                    Ok(n) => {
                        if let Some(frame) = decode_input_report(&buf[..n]) {
                            handle_input_frame(&evt_tx, device, &frame);
                        } else if let Some((battery, status)) = decode_battery_report(&buf[..n]) {
                            apply_battery(&evt_tx, device, battery, status);
                        }
                    }
                    Err(err) => {
                        let _ = evt_tx.send(HidEvent::Log(format!("HID read error: {err}")));
                        disconnected = true;
                    }
                }
            }
            // Also read command responses from the command channel.
            if let Some(command) = device.command.as_mut() {
                let mut buf = [0u8; 128];
                match command.read_timeout(&mut buf, 10) {
                    Ok(0) => {}
                    Ok(n) => {
                        if let Some(frame) = decode_input_report(&buf[..n]) {
                            handle_input_frame(&evt_tx, device, &frame);
                        } else if let Some((battery, status)) = decode_battery_report(&buf[..n]) {
                            apply_battery(&evt_tx, device, battery, status);
                        }
                    }
                    Err(err) => {
                        let _ = evt_tx.send(HidEvent::Log(format!(
                            "HID command channel read error: {err}"
                        )));
                        disconnected = true;
                    }
                }
            }

            // 4. Periodic status poll.
            if next_status_poll.elapsed() >= Duration::from_secs(15) {
                next_status_poll = Instant::now();
                request_status(&evt_tx, device);
            }

            if disconnected {
                handles = None;
                let _ = evt_tx.send(HidEvent::Disconnected);
            }
        }
    }

    let _ = evt_tx.send(HidEvent::Disconnected);
}

/// Enumerate supported MCHOSE devices, one descriptor per physical device.
fn enumerate(api: &HidApi) -> Result<Vec<HidDeviceDescriptor>, hidapi::HidError> {
    let infos: Vec<&DeviceInfo> = api
        .device_list()
        .filter(|info| family_for(info.vendor_id(), info.product_id()).is_some())
        .collect();

    // Group by (vid, pid, serial) and keep a representative entry.
    let mut out: Vec<HidDeviceDescriptor> = Vec::new();
    for info in infos {
        let vid = info.vendor_id();
        let pid = info.product_id();
        let serial = info.serial_number().map(|s| s.to_string());
        let product = info
            .product_string()
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("MCHOSE {vid:04X}:{pid:04X}"));
        let manufacturer = info.manufacturer_string().map(|s| s.to_string());
        if out
            .iter()
            .any(|d| d.vid == vid && d.pid == pid && d.serial == serial)
        {
            continue;
        }
        out.push(HidDeviceDescriptor {
            vid,
            pid,
            product_name: product,
            manufacturer,
            serial,
            path: info.path().to_string_lossy().to_string(),
        });
    }
    Ok(out)
}

fn open_device(api: &HidApi, descriptor: HidDeviceDescriptor) -> Result<DeviceHandles, String> {
    // Collect the collection paths of this physical device.
    let mut command_path = None;
    let mut notify_path = None;
    let mut firmware_path = None;
    let mut fallback_path = None;
    for info in api.device_list() {
        if info.vendor_id() != descriptor.vid || info.product_id() != descriptor.pid {
            continue;
        }
        let path = info.path().to_string_lossy().to_string();
        match info.usage_page() {
            USAGE_COMMAND => command_path = Some(path.clone()),
            USAGE_NOTIFY => notify_path = Some(path.clone()),
            USAGE_FIRMWARE => firmware_path = Some(path.clone()),
            _ => {
                if fallback_path.is_none() {
                    fallback_path = Some(path.clone());
                }
            }
        }
        if fallback_path.is_none() {
            fallback_path = Some(path);
        }
    }

    let command_path = command_path.or(fallback_path.clone());
    let open = |path: &Option<String>| -> Option<HidDevice> {
        let path = path.as_ref()?;
        let c_path = std::ffi::CString::new(path.clone()).ok()?;
        match api.open_path(&c_path) {
            Ok(device) => Some(device),
            Err(err) => {
                log::warn!("cannot open HID path {path}: {err}");
                None
            }
        }
    };

    let command = open(&command_path);
    let notify = open(&notify_path);
    let firmware = open(&firmware_path);
    if command.is_none() && notify.is_none() {
        return Err("no HID collection could be opened".into());
    }

    let mut handles = DeviceHandles {
        descriptor,
        command,
        notify,
        firmware,
        status: DeviceStatus::default(),
        last_poll: Instant::now(),
        last_scan_attempt: Instant::now(),
    };

    // Read headset (target 0) and dongle (target 1) firmware versions.
    if let Some(device) = handles.firmware.as_mut() {
        handles.status.headset_firmware = read_firmware_string(device, 0);
        handles.status.dongle_firmware = read_firmware_string(device, 1);
    }
    Ok(handles)
}

/// Read one firmware version string via feature report 170.
///
/// Vendor flow (recovered prior art): write `[0x01, target, 0 * 61]` in the 63-byte
/// feature payload, wait ~300 ms, then read the report back. `target` is 0 for the
/// headset MCU and 1 for the 2.4G dongle. The reply starts with the feature report
/// id (0xAA) followed by `0x01` and four version bytes.
fn read_firmware_string(device: &mut HidDevice, target: u8) -> Option<String> {
    let request = protocol::firmware_request(target);
    let mut write_buf = [0u8; 64];
    write_buf[0] = protocol::FEATURE_FIRMWARE;
    write_buf[1..].copy_from_slice(&request);
    if let Err(err) = device.send_feature_report(&write_buf) {
        log::debug!("firmware feature write failed (target {target}): {err}");
        return None;
    }
    std::thread::sleep(Duration::from_millis(300));
    let mut read_buf = [0u8; 65];
    read_buf[0] = protocol::FEATURE_FIRMWARE;
    match device.get_feature_report(&mut read_buf) {
        Ok(n) => protocol::decode_firmware_reply(&read_buf[..n.max(1)]),
        Err(err) => {
            log::debug!("firmware feature read failed (target {target}): {err}");
            None
        }
    }
}

/// Split a raw HID read buffer into (report id, payload).
fn split_report(data: &[u8]) -> Option<(u8, &[u8])> {
    data.split_first().map(|(id, rest)| (*id, rest))
}

/// Decode a framed command/notification report.
fn decode_input_report(data: &[u8]) -> Option<Frame> {
    let (report_id, payload) = split_report(data)?;
    if report_id != REPORT_ID_LEGACY {
        // Another collection's report (consumer control etc.).
        return None;
    }
    Frame::decode(payload).ok()
}

/// Decode the unframed vendor battery/status reply (payload begins with 0x65).
fn decode_battery_report(data: &[u8]) -> Option<(BatteryState, protocol::ChargeStatus)> {
    let (_, payload) = split_report(data)?;
    protocol::decode_battery_reply(payload)
}

/// Update the device status from an unframed battery reply.
fn apply_battery(
    evt_tx: &Sender<HidEvent>,
    device: &mut DeviceHandles,
    battery: BatteryState,
    status: protocol::ChargeStatus,
) {
    device.status.battery = Some(battery);
    device.status.asleep = status.is_asleep();
    if let Some(last) = device.status.last_seen_ms.as_mut() {
        *last = 0;
    }
    log::debug!("battery reply: {battery:?} ({status:?})");
    let _ = evt_tx.send(HidEvent::Status(Box::new(device.status.clone())));
}

fn handle_input_frame(
    evt_tx: &Sender<HidEvent>,
    device: &mut DeviceHandles,
    frame: &Frame,
) {
    if frame.cmd_type == protocol::CmdType::Notify || frame.cmd_type == protocol::CmdType::Request {
        match frame.cmd {
            protocol::notify::BATTERY_SLEEP | protocol::notify::BATTERY => {
                if let Some((battery, asleep)) = BatteryState::from_notify_payload(&frame.payload) {
                    device.status.battery = Some(battery);
                    device.status.asleep = asleep;
                    let _ = evt_tx.send(HidEvent::Status(Box::new(device.status.clone())));
                }
            }
            protocol::notify::EQ_MODE => {
                log::debug!("device reported EQ mode {:?}", frame.payload);
            }
            other => {
                log::trace!(
                    "unhandled HID frame cmd {other} type {:?} payload {:02X?}",
                    frame.cmd_type,
                    frame.payload
                );
            }
        }
    }
}

fn request_status(evt_tx: &Sender<HidEvent>, device: &mut DeviceHandles) {
    // Battery/status uses the vendor's *unframed* 0x65 request (see
    // findings-prior-art.md §4); prompt volume and auto-shutdown are framed requests.
    send_raw_command(evt_tx, device, &protocol::battery_request());
    send_command(
        evt_tx,
        device,
        Frame::request(protocol::cmd::GET_PROMPT_VOLUME, Vec::new()),
    );
    send_command(
        evt_tx,
        device,
        Frame::request(protocol::cmd::GET_AUTO_SHUTDOWN, Vec::new()),
    );
    device.status.last_seen_ms = Some(0);
}

/// Write a raw 63-byte report payload on the command report id.
fn send_raw_command(evt_tx: &Sender<HidEvent>, device: &mut DeviceHandles, payload: &[u8; 63]) {
    let mut buf = [0u8; 64];
    buf[0] = REPORT_ID_LEGACY;
    buf[1..].copy_from_slice(payload);
    if let Err(err) = write_report(device, REPORT_ID_LEGACY, &buf) {
        let _ = evt_tx.send(HidEvent::Log(format!("HID raw write failed: {err}")));
    }
}

fn send_command(evt_tx: &Sender<HidEvent>, device: &mut DeviceHandles, frame: Frame) {
    let mut buf = [0u8; 64];
    buf[0] = REPORT_ID_LEGACY;
    let encoded = frame.encode();
    buf[1..].copy_from_slice(&encoded);
    match write_report(device, REPORT_ID_LEGACY, &buf) {
        Ok(_) => {
            let _ = evt_tx.send(HidEvent::CommandResult {
                cmd: frame.cmd,
                ok: true,
            });
        }
        Err(err) => {
            let _ = evt_tx.send(HidEvent::Log(format!(
                "HID write failed (cmd {}): {err}",
                frame.cmd
            )));
            let _ = evt_tx.send(HidEvent::CommandResult {
                cmd: frame.cmd,
                ok: false,
            });
        }
    }
}

/// Write a report on the collection that actually declares `report_id`.
///
/// Windows rejects a write when the report id does not belong to the collection
/// (`WriteFile` error 1). The vendor report id 85 belongs to the 0xFF90
/// notification collection; the 0xFF82 command collection declares report id 65.
fn write_report(
    device: &mut DeviceHandles,
    report_id: u8,
    buf: &[u8; 64],
) -> Result<usize, hidapi::HidError> {
    let preferred_notify = report_id == REPORT_ID_LEGACY;
    let primary = if preferred_notify {
        device.notify.as_mut().or(device.command.as_mut())
    } else {
        device.command.as_mut().or(device.notify.as_mut())
    };
    match primary {
        Some(handle) => handle.write(buf),
        None => Err(hidapi::HidError::IncompleteSendError {
            sent: 0,
            all: buf.len(),
        }),
    }
}

