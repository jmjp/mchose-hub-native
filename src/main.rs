//! M HUB Native — a WebView-free Rust/egui reimplementation of the MCHOSE HUB
//! headset companion.
//!
//! The application reconstructs the original Electron app's headset workflows:
//! device discovery/status over the vendor HID protocol, and host-side audio DSP
//! control (10-band EQ, enhancement effects, microphone processing, volumes and
//! test sounds) through the Windows property store of the C-Media driver.
//!
//! See `mchose-hub-re/docs/report.md` for the reverse-engineering findings and
//! `mchose-hub-re/docs/plan.md` for the design rationale.
//!
//! The HID protocol module and the recovered C-Media property-name table keep the
//! complete device-family surface even when a given control is not used by the
//! current UI, so unused-item warnings are intentionally allowed crate-wide.
#![allow(dead_code)]

mod app;
mod backend;
mod config;
mod i18n;
mod logging;
mod model;
mod probe;
#[cfg(windows)]
mod tray;
#[cfg(not(windows))]
mod tray {
    use crossbeam_channel::Receiver;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum TrayEvent {
        Show,
        Quit,
    }

    pub fn init(_main_window: isize) -> Option<Receiver<TrayEvent>> {
        None
    }

    pub fn focus_main_window() {}
}
mod autostart;
mod ui;

use app::MHubApp;
use config::AppConfig;

fn main() -> eframe::Result<()> {
    let config = AppConfig::load();
    let _ = config::ensure_dirs();
    let _ = logging::init(&config.log_level);
    log::info!(
        "M HUB Native {} starting (data dir: {})",
        env!("CARGO_PKG_VERSION"),
        config::data_dir_display()
    );

    // Hardware/protocol probe mode (`mchose-hub-native --probe`).
    if std::env::args().any(|arg| arg == "--probe") {
        match probe::run() {
            Ok(()) => std::process::exit(0),
            Err(err) => {
                eprintln!("probe failed: {err}");
                log::error!("probe failed: {err}");
                std::process::exit(1);
            }
        }
    }

    let viewport = egui::ViewportBuilder::default()
        .with_title("M HUB Native")
        .with_inner_size(config.window_size)
        .with_min_inner_size([960.0, 620.0])
        .with_decorations(false)
        .with_app_id("mchose-hub-native");

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    let result = eframe::run_native(
        "M HUB Native",
        options,
        Box::new(move |cc| {
            let mut app = MHubApp::new(cc, config);
            app::load_presets(&mut app);
            Ok(Box::new(app))
        }),
    );

    if let Err(err) = &result {
        log::error!("eframe terminated with error: {err}");
    }
    result
}
