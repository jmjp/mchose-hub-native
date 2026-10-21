//! Device dashboard: identity, status, backend state and shortcuts.

use egui::{RichText, Vec2};

use crate::app::MHubApp;
use crate::ui::pages::Page;
use crate::ui::theme::Palette;
use crate::ui::widgets;

pub fn draw(app: &mut MHubApp, ui: &mut egui::Ui, palette: Palette) {
    ui.label(
        RichText::new(app.i18n.t("device.overview"))
            .strong()
            .size(22.0)
            .color(palette.text),
    );
    ui.add_space(12.0);

    let width = ui.available_width();
    let two_columns = width > 760.0;
    if two_columns {
        // `ui.columns` gives both cards a fixed, equal width and a proper content
        // height; allocate_ui(.., 0.0) collapsed the vertical layout and made the
        // key/value rows overlap.
        let spacing = ui.spacing().item_spacing.x;
        ui.spacing_mut().item_spacing.x = 12.0;
        ui.columns(2, |columns| {
            draw_status_card(app, &mut columns[0], palette);
            draw_audio_card(app, &mut columns[1], palette);
        });
        ui.spacing_mut().item_spacing.x = spacing;
    } else {
        draw_status_card(app, ui, palette);
        ui.add_space(12.0);
        draw_audio_card(app, ui, palette);
    }

    ui.add_space(14.0);
    widgets::section_header(
        ui,
        palette,
        app.i18n.t("routine.title"),
        Some(app.i18n.t("routine.subtitle")),
    );
    let mut navigate: Option<Page> = None;
    let shortcuts = [
        (Page::Routine, "nav.routine", "♪"),
        (Page::Eq, "nav.eq", "≡"),
        (Page::Effects, "nav.effects", "◆"),
        (Page::Surround, "nav.surround", "◎"),
        (Page::Mic, "nav.mic", "○"),
    ];
    // Fit exactly five shortcut cards into the available width (accounting for the
    // real item spacing and the frame margins) so the last card is never clipped.
    let gap = ui.spacing().item_spacing.x;
    let card_width = ((width - gap * 4.0) / 5.0).max(120.0) - 1.0;
    ui.horizontal_wrapped(|ui| {
        for (page, key, icon) in shortcuts {
            ui.allocate_ui_with_layout(
                Vec2::new(card_width, 92.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    let frame = egui::Frame::new()
                        .fill(palette.card)
                        .stroke(egui::Stroke::new(1.0_f32, palette.border))
                        .corner_radius(egui::CornerRadius::same(14))
                        .inner_margin(egui::Margin::symmetric(14, 14));
                    let response = frame.show(ui, |ui| {
                        ui.set_min_width(card_width - 30.0);
                        ui.vertical(|ui| {
                            ui.label(RichText::new(icon).size(20.0).color(palette.text));
                            ui.add_space(6.0);
                            ui.label(
                                RichText::new(app.i18n.t(key))
                                    .size(12.5)
                                    .color(palette.text_weak),
                            );
                        });
                    });
                    if response.response.interact(egui::Sense::click()).clicked() {
                        navigate = Some(page);
                    }
                },
            );
        }
    });
    if let Some(page) = navigate {
        app.set_page(page);
    }
}

fn draw_status_card(app: &MHubApp, ui: &mut egui::Ui, palette: Palette) {
    widgets::card(ui, palette, |ui| {
        widgets::section_header(ui, palette, app.i18n.t("device.model"), None);
        if let Some(device) = &app.selected_device {
            let family = crate::model::device::family_for(device.vid, device.pid);
            let name = family
                .map(|f| f.name)
                .filter(|n| !n.is_empty())
                .unwrap_or(device.product_name.as_str());
            ui.label(RichText::new(name).strong().size(16.0).color(palette.text));
            widgets::kv_row(
                ui,
                palette,
                app.i18n.t("device.ids"),
                &format!("{:04X}:{:04X}", device.vid, device.pid),
            );
            if let Some(serial) = &device.serial {
                if !serial.trim().is_empty() {
                    widgets::kv_row(ui, palette, "SN", serial);
                }
            }
        }
        widgets::divider(ui, palette);
        widgets::kv_row(
            ui,
            palette,
            app.i18n.t("device.connection"),
            app.i18n.t("connect.wireless24g"),
        );
        let battery = match (&app.status, app.status.battery_label()) {
            (status, Some(label)) if status.battery.map(|b| b.charging).unwrap_or(false) => {
                format!("⚡ {label}")
            }
            (_, Some(label)) => label,
            _ => app.i18n.t("device.unknown").to_string(),
        };
        widgets::kv_row(ui, palette, app.i18n.t("device.battery"), &battery);
        let firmware = app
            .status
            .headset_firmware
            .clone()
            .unwrap_or_else(|| app.i18n.t("device.unknown").to_string());
        widgets::kv_row(ui, palette, app.i18n.t("device.firmware"), &firmware);
        let dongle = app
            .status
            .dongle_firmware
            .clone()
            .unwrap_or_else(|| app.i18n.t("device.unknown").to_string());
        widgets::kv_row(ui, palette, app.i18n.t("device.dongle"), &dongle);
    });
}

fn draw_audio_card(app: &MHubApp, ui: &mut egui::Ui, palette: Palette) {
    widgets::card(ui, palette, |ui| {
        widgets::section_header(ui, palette, app.i18n.t("settings.backend"), None);
        ui.label(
            RichText::new(app.backend_status())
                .size(13.0)
                .color(palette.text),
        );
        if let Some(endpoint) = app.audio_backend.endpoint_name() {
            ui.label(
                RichText::new(endpoint)
                    .size(11.5)
                    .color(palette.text_weak),
            );
        }
        widgets::divider(ui, palette);
        let caps = app.caps;
        let rows = [
            (app.i18n.t("nav.eq"), caps.eq),
            (app.i18n.t("nav.effects"), caps.effects),
            (app.i18n.t("nav.mic"), caps.mic_dsp),
            (app.i18n.t("nav.surround"), caps.surround),
            (app.i18n.t("device.refresh"), caps.hid_status),
        ];
        for (label, available) in rows {
            let value = if available {
                app.i18n.t("common.on")
            } else {
                app.i18n.t("common.unsupported")
            };
            let color = if available {
                palette.success
            } else {
                palette.text_weak
            };
            ui.horizontal(|ui| {
                ui.label(RichText::new(label).size(12.5).color(palette.text_weak));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new(value).size(12.0).color(color));
                });
            });
        }
        if let Some(hint) = app.backend_error_hint() {
            ui.add_space(4.0);
            ui.label(RichText::new(hint).size(11.0).color(palette.warning));
        }
    });
}
