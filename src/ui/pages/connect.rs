//! Connect screen: discover and select a supported MCHOSE device.
//!
//! Mirrors the original `/index` ("connect device") page: a card per connected
//! device with the render name and connection icon, plus a scan action.

use egui::{CornerRadius, RichText, Stroke, Vec2};

use crate::app::MHubApp;
use crate::backend::hid::HidCommand;
use crate::ui::pages::Page;
use crate::ui::theme::Palette;
use crate::ui::widgets;

pub fn draw(app: &mut MHubApp, ui: &mut egui::Ui, palette: Palette) {
    ui.add_space(8.0);
    ui.label(
        RichText::new(app.i18n.t("connect.title"))
            .strong()
            .size(24.0)
            .color(palette.text),
    );
    ui.label(
        RichText::new(app.i18n.t("connect.subtitle"))
            .size(13.0)
            .color(palette.text_weak),
    );
    ui.add_space(18.0);

    ui.horizontal(|ui| {
        if ui
            .add(
                egui::Button::new(
                    RichText::new(format!("⟳  {}", app.i18n.t("connect.scan")))
                        .color(palette.accent_text),
                )
                .fill(palette.accent)
                .corner_radius(CornerRadius::same(10)),
            )
            .clicked()
        {
            app.hid.send(HidCommand::Rescan);
            app.refresh_status();
        }
    });
    ui.add_space(14.0);

    if app.devices.is_empty() {
        widgets::card(ui, palette, |ui| {
            ui.add_space(22.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("◎").size(34.0).color(palette.text_weak));
                ui.add_space(6.0);
                ui.label(
                    RichText::new(app.i18n.t("connect.none"))
                        .size(13.0)
                        .color(palette.text_weak),
                );
            });
            ui.add_space(22.0);
        });
        return;
    }

    let width = ui.available_width();
    let columns = ((width / 300.0).floor() as usize).max(1);
    let devices = app.devices.clone();
    let mut selected: Option<crate::model::HidDeviceDescriptor> = None;
    for chunk in devices.chunks(columns) {
        ui.horizontal(|ui| {
            for device in chunk {
                let card_width = (width / columns as f32) - 14.0;
                ui.allocate_ui(Vec2::new(card_width, 0.0), |ui| {
                    let response = draw_device_card(app, ui, palette, device);
                    if response {
                        selected = Some(device.clone());
                    }
                });
            }
        });
        ui.add_space(10.0);
    }

    if let Some(device) = selected {
        app.selected_device = Some(device);
        app.refresh_status();
        app.set_page(Page::Home);
    }
}

/// Returns true when the user chose this device.
fn draw_device_card(
    app: &MHubApp,
    ui: &mut egui::Ui,
    palette: Palette,
    device: &crate::model::HidDeviceDescriptor,
) -> bool {
    let frame = egui::Frame::new()
        .fill(palette.card)
        .stroke(Stroke::new(1.0_f32, palette.border))
        .corner_radius(CornerRadius::same(16))
        .inner_margin(egui::Margin::symmetric(18, 16));
    let mut configure = false;
    frame.show(ui, |ui| {
        ui.set_min_height(190.0);
        let family = crate::model::device::family_for(device.vid, device.pid);
        ui.horizontal(|ui| {
            ui.label(RichText::new("◑").size(28.0).color(palette.text_weak));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                widgets::chip(ui, palette, app.i18n.t("connect.connected"));
            });
        });
        ui.add_space(16.0);
        let name = family
            .map(|f| f.name)
            .filter(|n| !n.is_empty())
            .unwrap_or(device.product_name.as_str());
        ui.label(RichText::new(name).strong().size(17.0).color(palette.text));
        ui.label(
            RichText::new(format!(
                "{} · VID {:04X} PID {:04X}",
                app.i18n.t("connect.wireless24g"),
                device.vid,
                device.pid
            ))
            .size(11.5)
            .color(palette.text_weak),
        );
        if let Some(serial) = &device.serial {
            if !serial.trim().is_empty() {
                ui.label(
                    RichText::new(format!("SN {serial}"))
                        .size(10.5)
                        .color(palette.text_weak),
                );
            }
        }
        ui.add_space(16.0);
        if ui
            .add(
                egui::Button::new(
                    RichText::new(app.i18n.t("connect.select"))
                        .color(palette.accent_text)
                        .size(12.5),
                )
                .fill(palette.accent)
                .corner_radius(CornerRadius::same(10))
                .min_size(Vec2::new(110.0, 30.0)),
            )
            .clicked()
        {
            configure = true;
        }
        if family.is_none() {
            ui.label(
                RichText::new(app.i18n.t("connect.unsupported"))
                    .size(10.5)
                    .color(palette.warning),
            );
        }
    });
    configure
}
