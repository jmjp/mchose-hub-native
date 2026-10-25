//! Application state machine: owns backends, configuration, page routing and the
//! event pump between the HID worker and the UI.

use std::time::{Duration, Instant};

use crate::backend::bluetooth::{self, BluetoothStatus};
use crate::backend::hid::{HidCommand, HidEvent, HidService};
use crate::backend::{open_audio_backend, AudioBackend, BackendError};
use crate::config::AppConfig;
use crate::i18n::I18n;
use crate::model::audio::{AudioChange, AudioState, Capabilities, TestChannel};
use crate::model::device::{DeviceStatus, HidDeviceDescriptor};
use crate::model::presets::PresetDb;
use crate::ui::pages::Page;

/// A transient notification shown at the bottom of the window.
#[derive(Debug, Clone)]
pub struct Toast {
    pub text: String,
    pub is_error: bool,
    pub created: Instant,
}

impl Toast {
    fn new(text: impl Into<String>, is_error: bool) -> Self {
        Self {
            text: text.into(),
            is_error,
            created: Instant::now(),
        }
    }
}

/// EQ editor transient state.
#[derive(Debug, Clone, Default)]
pub struct EqEditor {
    /// Name buffer while the "save preset" dialog is open.
    pub name_buffer: String,
    pub save_dialog_open: bool,
}

/// Confirmation dialog state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmAction {
    FactoryReset,
    ResetSettings,
}

/// The application.
pub struct MHubApp {
    pub config: AppConfig,
    pub i18n: I18n,
    pub presets: PresetDb,
    pub audio_backend: Box<dyn AudioBackend>,
    pub hid: HidService,
    pub audio: AudioState,
    pub caps: Capabilities,
    pub status: DeviceStatus,
    /// Bluetooth headset state, when no USB/2.4G HID device is active.
    pub bluetooth: Option<BluetoothStatus>,
    bt_rx: Option<crossbeam_channel::Receiver<Option<BluetoothStatus>>>,
    last_bt_poll: Instant,
    pub devices: Vec<HidDeviceDescriptor>,
    pub selected_device: Option<HidDeviceDescriptor>,
    pub page: Page,
    pub toasts: Vec<Toast>,
    pub eq_editor: EqEditor,
    pub confirm: Option<ConfirmAction>,
    /// Notification-area icon is active (Windows only).
    pub tray_active: bool,
    tray_events: Option<crossbeam_channel::Receiver<crate::tray::TrayEvent>>,
    /// Set when the tray menu asked to exit, so the close hook does not hide the window.
    pub quitting: bool,
    /// Product photo of the connected model (used by the parameter page).
    pub product_image: Option<egui::TextureHandle>,
    hid_window: isize,
    startup_hidden: bool,
    pub test_channel: Option<TestChannel>,
    pub last_hid_scan: Instant,
    pub last_tick: Instant,
    pub save_pending: Option<Instant>,
    pub exit_requested: bool,
    pub log_dirty: bool,
}

impl MHubApp {
    pub fn new(cc: &eframe::CreationContext<'_>, config: AppConfig) -> Self {
        let system_lang = crate::config::system_language();
        let i18n = I18n::new(config.language.as_deref(), &system_lang);
        crate::ui::theme::install_fonts(&cc.egui_ctx);
        crate::ui::theme::apply(&cc.egui_ctx, config.theme);

        let mut audio_backend = open_audio_backend();
        let caps = audio_backend.capabilities();
        let mut audio = audio_backend.read().unwrap_or_default();

        let mut presets = PresetDb::load_factory();
        presets.selected = config.selected_preset.clone();
        if let Some(preset) = presets.selected_preset() {
            audio.eq_bands = preset.bands();
        }

        let page = config
            .last_page
            .as_deref()
            .and_then(Page::from_id)
            .unwrap_or(Page::Home);

        let product_image = load_product_image(&cc.egui_ctx);

        let hid = HidService::start();
        hid.send(HidCommand::Rescan);

        let hid_window = main_window_handle(cc);
        let tray_events = if hid_window != 0 {
            crate::tray::init(hid_window)
        } else {
            log::warn!("no Win32 window handle; tray icon disabled");
            None
        };
        let tray_active = tray_events.is_some();

        Self {
            config,
            i18n,
            presets,
            audio_backend,
            hid,
            audio,
            caps,
            status: DeviceStatus::default(),
            bluetooth: None,
            bt_rx: None,
            last_bt_poll: Instant::now(),
            devices: Vec::new(),
            selected_device: None,
            page,
            toasts: Vec::new(),
            eq_editor: EqEditor::default(),
            confirm: None,
            test_channel: None,
            last_hid_scan: Instant::now(),
            last_tick: Instant::now(),
            save_pending: None,
            exit_requested: false,
            log_dirty: false,
            tray_active,
            tray_events,
            quitting: false,
            product_image,
            hid_window,
            startup_hidden: false,
        }
    }

    fn handle_tray_events(&mut self, ctx: &egui::Context) {
        let Some(receiver) = self.tray_events.clone() else {
            return;
        };
        while let Ok(event) = receiver.try_recv() {
            match event {
                crate::tray::TrayEvent::Show => {
                    crate::tray::focus_main_window();
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                crate::tray::TrayEvent::Quit => {
                    self.quitting = true;
                    self.exit_requested = true;
                }
            }
        }
    }

    pub fn connected(&self) -> bool {
        self.selected_device.is_some()
    }

    /// Apply an audio change to the backend and mirror it into the local state.
    pub fn apply_audio(&mut self, change: AudioChange) {
        match self.audio_backend.apply(change) {
            Ok(()) => {
                self.apply_to_state(change);
                self.schedule_save();
                self.mark_dirty();
            }
            Err(err) => {
                let message = format!("{}: {err}", self.i18n.t("toast.audio_error"));
                log::warn!("{message}");
                self.toast(message, true);
            }
        }
    }

    /// Mirror a change in the in-memory state (called after a successful write).
    pub fn apply_to_state(&mut self, change: AudioChange) {
        match change {
            AudioChange::EqEnabled(v) => self.audio.eq_enabled = v,
            AudioChange::EqBand { band, value } => {
                if band < self.audio.eq_bands.len() {
                    self.audio.eq_bands[band] = value;
                }
            }
            AudioChange::EffectEnabled { kind, enabled } => {
                self.audio.effect_mut(kind).enabled = enabled
            }
            AudioChange::EffectLevel { kind, level } => self.audio.effect_mut(kind).level = level,
            AudioChange::BassCutoffHz(hz) => {
                self.audio.effect_mut(crate::model::EffectKind::VirtualBass).cutoff_hz = hz
            }
            AudioChange::AdaptiveMode(mode) => {
                self.audio
                    .effect_mut(crate::model::EffectKind::AdaptiveVolume)
                    .mode = mode
            }
            AudioChange::VoiceNoiseSuppression(v) => {
                self.audio
                    .effect_mut(crate::model::EffectKind::VoiceClarity)
                    .noise_suppression = v
            }
            AudioChange::MagicVoiceEnabled(v) => self.audio.mic.magic_voice = v,
            AudioChange::MagicVoiceType(v) => self.audio.mic.magic_voice_type = v,
            AudioChange::AiNoiseReduction(v) => self.audio.mic.ai_noise_reduction = v,
            AudioChange::NoiseSuppression(v) => self.audio.mic.noise_suppression = v,
            AudioChange::SpeakerVolume(v) => self.audio.volume.speaker = v,
            AudioChange::SpeakerMute(v) => self.audio.volume.speaker_muted = v,
            AudioChange::MicVolume(v) => self.audio.volume.mic = v,
            AudioChange::MicMute(v) => self.audio.volume.mic_muted = v,
            AudioChange::SurroundEnabled(v) => self.audio.surround.enabled = v,
            AudioChange::SurroundMode(v) => self.audio.surround.mode = v,
            AudioChange::SurroundRoom(v) => self.audio.surround.room = v,
            AudioChange::SurroundAmplification(v) => self.audio.surround_amplification = v,
            AudioChange::MicListen(v) => self.audio.mic.mic_listen = v,
        }
    }

    /// Play a speaker-test channel through the audio backend.
    pub fn play_test(&mut self, channel: TestChannel) {
        match self.audio_backend.test_sound(channel) {
            Ok(()) => {
                self.test_channel = Some(channel);
            }
            Err(err) => {
                let message = format!("{}: {err}", self.i18n.t("toast.audio_error"));
                log::warn!("{message}");
                self.toast(message, true);
            }
        }
    }

    pub fn stop_test(&mut self) {
        if let Err(err) = self.audio_backend.stop_test_sound() {
            log::warn!("stop test sound: {err}");
        }
        self.test_channel = None;
    }

    pub fn toast(&mut self, text: impl Into<String>, is_error: bool) {
        self.toasts.push(Toast::new(text, is_error));
        if self.toasts.len() > 4 {
            self.toasts.remove(0);
        }
    }

    pub fn mark_dirty(&mut self) {
        // egui repaints on interaction; this hook exists for worker-driven updates.
    }

    /// Persist configuration after a short debounce.
    pub fn schedule_save(&mut self) {
        self.save_pending = Some(Instant::now());
    }

    pub fn set_page(&mut self, page: Page) {
        self.page = page;
        self.config.last_page = Some(page.id().to_string());
        self.schedule_save();
    }

    pub fn set_language(&mut self, lang: &str) {
        self.config.language = Some(lang.to_string());
        let system = crate::config::system_language();
        self.i18n = I18n::new(Some(lang), &system);
        self.schedule_save();
    }

    pub fn set_theme(&mut self, theme: crate::config::ThemeMode) {
        self.config.theme = theme;
        self.schedule_save();
    }

    pub fn apply_preset(&mut self, preset_id: &str) {
        let Some(preset) = self.presets.find(preset_id).cloned() else {
            return;
        };
        let bands = preset.bands();
        for (band, value) in bands.iter().enumerate() {
            if self.audio.eq_bands[band] != *value {
                self.apply_audio(AudioChange::EqBand { band, value: *value });
            }
        }
        self.presets.selected = Some(preset_id.to_string());
        self.config.selected_preset = Some(preset_id.to_string());
        self.schedule_save();
        let text = self.i18n.t("toast.preset_applied").to_string();
        self.toast(text, false);
    }

    /// Export the current EQ bands as a user preset.
    pub fn save_preset(&mut self, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        let id = format!("user-{}", slug(name));
        let preset = crate::model::EqPreset {
            id: id.clone(),
            group: None,
            names: crate::model::LocalizedNames {
                en: name.to_string(),
                zh: name.to_string(),
                pt: name.to_string(),
            },
            data: self.audio.eq_bands.to_vec(),
        };
        self.presets.upsert_user(preset);
        self.presets.selected = Some(id.clone());
        self.config.selected_preset = Some(id);
        self.persist_user_presets();
        self.schedule_save();
        let text = self.i18n.t("eq.saved").to_string();
        self.toast(text, false);
    }

    pub(crate) fn persist_user_presets(&self) {
        let path = crate::config::preset_file();
        match serde_json::to_string_pretty(&self.presets.user) {
            Ok(json) => {
                if let Err(err) = std::fs::write(&path, json) {
                    log::warn!("cannot write user presets: {err}");
                }
            }
            Err(err) => log::warn!("cannot serialize user presets: {err}"),
        }
    }

    /// Delete a user preset and persist the change.
    pub fn delete_user_preset(&mut self, id: &str) {
        if self.presets.delete_user(id) {
            if self.config.selected_preset.as_deref() == Some(id) {
                self.config.selected_preset = None;
            }
            self.persist_user_presets();
            self.schedule_save();
        }
    }

    fn load_user_presets(&mut self) {
        let path = crate::config::preset_file();
        if let Ok(text) = std::fs::read_to_string(&path) {
            match serde_json::from_str::<Vec<crate::model::EqPreset>>(&text) {
                Ok(user) => self.presets.user = user,
                Err(err) => log::warn!("ignoring invalid user presets: {err}"),
            }
        }
    }

    /// Pump HID worker events and periodic maintenance.
    pub fn tick(&mut self, ctx: &egui::Context) {
        // Drain HID events.
        for event in self.hid.drain(32) {
            match event {
                HidEvent::Devices(devices) => {
                    self.devices = devices;
                    if self.selected_device.is_none() {
                        // Auto-select the first supported device like the original app.
                        if let Some(first) = self.devices.first() {
                            self.selected_device = Some(first.clone());
                        }
                    }
                }
                HidEvent::Connected {
                    descriptor,
                    firmware,
                } => {
                    log::info!("device connected: {}", descriptor.product_name);
                    self.bluetooth = None;
                    self.status = DeviceStatus {
                        connection: crate::model::device::ConnectionMode::Wireless24G,
                        headset_firmware: firmware,
                        ..DeviceStatus::default()
                    };
                    self.selected_device = Some(descriptor);
                }
                HidEvent::Disconnected => {
                    log::info!("device disconnected");
                    self.selected_device = None;
                    self.status = DeviceStatus::default();
                }
                HidEvent::Status(status) => {
                    self.status = *status;
                }
                HidEvent::CommandResult { cmd, ok } => {
                    if !ok {
                        log::warn!("HID command {cmd} failed");
                    }
                }
                HidEvent::Log(message) => log::info!("{message}"),
            }
        }

        self.poll_bluetooth();

        // Periodically rescan while nothing is connected.
        if self.selected_device.is_none() && self.last_hid_scan.elapsed() > Duration::from_secs(3) {
            self.last_hid_scan = Instant::now();
            self.hid.send(HidCommand::Rescan);
        }

        // Debounced config save.
        if let Some(pending) = self.save_pending {
            if pending.elapsed() > Duration::from_millis(400) {
                self.save_pending = None;
                if let Err(err) = self.config.save() {
                    log::warn!("cannot save config: {err}");
                }
            }
        }

        // Expire toasts.
        self.toasts
            .retain(|toast| toast.created.elapsed() < Duration::from_secs(4));

        // Repaint at ~30 Hz for HID updates, or sooner when a UI event happened.
        if !self.toasts.is_empty() || self.selected_device.is_none() {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }

    /// Poll Bluetooth battery in the background (only while no HID device is open).
    fn poll_bluetooth(&mut self) {
        if let Some(receiver) = self.bt_rx.as_ref() {
            if let Ok(status) = receiver.try_recv() {
                self.bt_rx = None;
                self.apply_bluetooth(status);
            }
        }
        if self.selected_device.is_some() {
            self.bluetooth = None;
            return;
        }
        if self.bt_rx.is_none() && self.last_bt_poll.elapsed() >= Duration::from_secs(15) {
            self.last_bt_poll = Instant::now();
            let (tx, rx) = crossbeam_channel::bounded(1);
            self.bt_rx = Some(rx);
            std::thread::Builder::new()
                .name("mchose-bt".into())
                .spawn(move || {
                    let _ = tx.send(bluetooth::query());
                })
                .ok();
        }
    }

    fn apply_bluetooth(&mut self, status: Option<BluetoothStatus>) {
        match status {
            Some(bt) => {
                if self.selected_device.is_some() {
                    return;
                }
                log::info!("Bluetooth device: {} ({}%)", bt.name, bt.percent);
                self.status.connection = crate::model::device::ConnectionMode::Bluetooth;
                self.status.battery = Some(crate::model::device::BatteryState {
                    percent: bt.percent,
                    charging: bt.charging,
                });
                self.status.bluetooth_name = Some(bt.name.clone());
                self.status.asleep = false;
                self.status.hardware_eq_mode = None;
                self.status.headset_firmware = None;
                self.status.dongle_firmware = None;
                self.bluetooth = Some(bt);
            }
            None => {
                if self.bluetooth.take().is_some() {
                    self.status.battery = None;
                    self.status.bluetooth_name = None;
                    self.status.connection =
                        crate::model::device::ConnectionMode::Wireless24G;
                }
            }
        }
    }

    /// Refresh device status through the HID worker.
    pub fn refresh_status(&mut self) {
        self.hid.send(HidCommand::RefreshStatus);
    }

    /// True when the audio backend reports an error string for the UI.
    pub fn backend_status(&self) -> String {
        let key = self.audio_backend.kind().label_key();
        self.i18n.t(key).to_string()
    }

    pub fn backend_error_hint(&self) -> Option<String> {
        if self.caps.eq || self.caps.effects || self.caps.mic_dsp {
            None
        } else {
            Some(self.i18n.t("settings.compat_text").to_string())
        }
    }
}

impl MHubApp {
    pub fn last_error(&self) -> Option<&BackendError> {
        None
    }
}

fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

impl eframe::App for MHubApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.handle_tray_events(ctx);

        if !self.startup_hidden {
            self.startup_hidden = true;
            if self.config.start_minimized && self.tray_active {
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            }
        }

        // Close-to-tray: intercept the window close request while the tray icon exists.
        if ctx.input(|input| input.viewport().close_requested())
            && self.config.close_to_tray
            && self.tray_active
            && !self.quitting
        {
            self.config.save().ok();
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }

        self.tick(ctx);
        crate::ui::theme::apply(ctx, self.config.theme);
        crate::ui::draw(self, ctx, frame);

        if self.exit_requested {
            self.config.save().ok();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

/// Load persisted user presets after construction (kept separate so `new` stays testable).
pub fn load_presets(app: &mut MHubApp) {
    app.load_user_presets();
}


/// Decode the bundled product photo into an egui texture (once at startup).
fn load_product_image(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let bytes = include_bytes!("../assets/product_v9pro.png");
    let image = image::load_from_memory(bytes).ok()?.to_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    let color = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
    Some(ctx.load_texture(
        "product_v9pro",
        color,
        egui::TextureOptions::LINEAR,
    ))
}

/// Raw Win32 HWND of the eframe window, or 0 when unavailable.
#[cfg(windows)]
fn main_window_handle(cc: &eframe::CreationContext<'_>) -> isize {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    match cc.window_handle() {
        Ok(handle) => match handle.as_raw() {
            RawWindowHandle::Win32(window) => window.hwnd.get(),
            _ => 0,
        },
        Err(err) => {
            log::warn!("cannot obtain window handle: {err}");
            0
        }
    }
}

#[cfg(not(windows))]
fn main_window_handle(_cc: &eframe::CreationContext<'_>) -> isize {
    // No Win32 tray integration on this platform; the window stays visible.
    0
}
