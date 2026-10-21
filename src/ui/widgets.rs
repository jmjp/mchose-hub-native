//! Reusable widgets in the MCHOSE HUB visual language: rounded cards, toggles,
//! labelled sliders, segmented selectors and the EQ curve painter.

use egui::{Color32, CornerRadius, Response, RichText, Sense, Stroke, Ui, Vec2};

use crate::ui::theme::Palette;

/// Draw a rounded card and return the inner ui plus the card response.
pub fn card<R>(
    ui: &mut Ui,
    palette: Palette,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> (Response, R) {
    let frame = egui::Frame::new()
        .fill(palette.card)
        .stroke(Stroke::new(1.0_f32, palette.border))
        .corner_radius(CornerRadius::same(16))
        .inner_margin(egui::Margin::symmetric(18, 16));
    let inner = frame.show(ui, add_contents);
    (inner.response, inner.inner)
}

/// Section title with optional description.
pub fn section_header(ui: &mut Ui, palette: Palette, title: &str, description: Option<&str>) {
    ui.label(RichText::new(title).strong().size(16.0).color(palette.text));
    if let Some(description) = description {
        ui.label(
            RichText::new(description)
                .size(12.0)
                .color(palette.text_weak),
        );
    }
    ui.add_space(4.0);
}

/// Horizontal rule.
pub fn divider(ui: &mut Ui, palette: Palette) {
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), 1.0),
        Sense::hover(),
    );
    ui.painter()
        .rect_filled(rect, CornerRadius::same(0), palette.border);
}

/// Apple-style toggle switch. Returns true when the value changed.
pub fn toggle(ui: &mut Ui, palette: Palette, value: &mut bool) -> egui::Response {
    let desired = Vec2::new(38.0, 22.0);
    let (rect, response) = ui.allocate_exact_size(desired, Sense::click());
    let changed = response.clicked();
    if changed {
        *value = !*value;
    }
    let hovered = response.hovered();
    let bg = if *value {
        if hovered {
            palette.accent.gamma_multiply(0.85)
        } else {
            palette.accent
        }
    } else if hovered {
        palette.card_hover
    } else {
        palette.track
    };
    ui.painter().rect_filled(rect, CornerRadius::same(11), bg);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(11),
        Stroke::new(1.0_f32, palette.border),
        egui::StrokeKind::Inside,
    );
    let knob_x = if *value {
        rect.right() - 11.0
    } else {
        rect.left() + 11.0
    };
    ui.painter().circle_filled(
        egui::pos2(knob_x, rect.center().y),
        8.0,
        if *value {
            palette.accent_text
        } else {
            palette.text_weak
        },
    );
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A settings row: title on the left, toggle on the right.
/// Returns the toggle's response for tooltips.
pub fn toggle_row(
    ui: &mut Ui,
    palette: Palette,
    title: &str,
    description: Option<&str>,
    value: &mut bool,
    enabled: bool,
) -> egui::Response {
    let mut response = None;
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(RichText::new(title).color(palette.text));
            if let Some(description) = description {
                ui.label(
                    RichText::new(description)
                        .size(11.5)
                        .color(palette.text_weak),
                );
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_enabled_ui(enabled, |ui| {
                // The switch itself must be the reported response; the previous
                // implementation returned the row container, so callers checking
                // `.clicked()` never saw a toggle click and the setting appeared dead.
                response = Some(toggle(ui, palette, value));
            });
        });
    });
    response.unwrap_or_else(|| ui.response())
}

/// A labelled slider row with a numeric value box.
///
/// Returns true when the value changed this frame.
pub fn slider_row(
    ui: &mut Ui,
    palette: Palette,
    label: &str,
    value: &mut i32,
    range: std::ops::RangeInclusive<i32>,
    suffix: &str,
    enabled: bool,
) -> bool {
    let mut changed = false;
    ui.add_enabled_ui(enabled, |ui| {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(label)
                    .size(12.5)
                    .color(if enabled { palette.text_weak } else { palette.text_weak.gamma_multiply(0.6) }),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut current = *value;
                let drag = ui.add(
                    egui::DragValue::new(&mut current)
                        .range(range.clone())
                        .speed(0.25)
                        .suffix(suffix),
                );
                if drag.changed() && current != *value {
                    *value = current;
                    changed = true;
                }
            });
        });
        let mut slider_value = *value;
        let slider = ui.add(
            egui::Slider::new(&mut slider_value, range.clone())
                .show_value(false)
                .trailing_fill(false),
        );
        if slider.changed() && slider_value != *value {
            *value = slider_value;
            changed = true;
        }
    });
    changed
}

/// A slider operating on f32 (0..=1 volumes).
pub fn slider_row_f32(
    ui: &mut Ui,
    palette: Palette,
    label: &str,
    value: &mut f32,
    percent_label: &str,
    enabled: bool,
) -> bool {
    let mut changed = false;
    ui.add_enabled_ui(enabled, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new(label).size(12.5).color(palette.text_weak));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("{:.0}%", *value * 100.0))
                        .size(12.0)
                        .color(palette.text),
                );
                let _ = percent_label;
            });
        });
        let slider = ui.add(
            egui::Slider::new(value, 0.0..=1.0)
                .show_value(false)
                .trailing_fill(true),
        );
        changed = slider.changed();
    });
    changed
}

/// Segmented button group. Returns true when the selection changed.
pub fn segmented<T: PartialEq + Copy>(
    ui: &mut Ui,
    palette: Palette,
    current: &mut T,
    options: &[(T, &str)],
) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        for (value, label) in options {
            let selected = *value == *current;
            let text_color = if selected {
                palette.accent_text
            } else {
                palette.text
            };
            let fill = if selected {
                palette.accent
            } else {
                palette.card
            };
            let button = egui::Button::new(RichText::new(*label).color(text_color).size(12.5))
                .fill(fill)
                .stroke(Stroke::new(1.0_f32, palette.border))
                .corner_radius(CornerRadius::same(10));
            if ui.add(button).clicked() {
                *current = *value;
                changed = true;
            }
        }
    });
    changed
}

/// A small "chip" label, e.g. battery/firmware in the header.
pub fn chip(ui: &mut Ui, palette: Palette, text: &str) -> egui::Response {
    let frame = egui::Frame::new()
        .fill(palette.card)
        .stroke(Stroke::new(1.0_f32, palette.border))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(egui::Margin::symmetric(10, 4));
    frame
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(12.0).color(palette.text_weak));
        })
        .response
}

/// Draw the 10-band EQ curve. `bands` are dB values, `enabled` dims the curve.
pub fn eq_curve(ui: &mut Ui, palette: Palette, bands: &[i32; 10], enabled: bool) {
    let height = 180.0;
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(ui.available_width().max(200.0), height),
        Sense::hover(),
    );
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(12), palette.card_hover);

    // Horizontal dB grid lines (-12, 0, +12).
    let min_db = crate::model::audio::EQ_GAIN_MIN as f32;
    let max_db = crate::model::audio::EQ_GAIN_MAX as f32;
    for db in [min_db, 0.0, max_db] {
        let t = (max_db - db) / (max_db - min_db);
        let y = rect.top() + 20.0 + t * (rect.height() - 40.0);
        let stroke = if db == 0.0 {
            Stroke::new(1.0_f32, palette.border)
        } else {
            Stroke::new(1.0_f32, palette.border.gamma_multiply(0.6))
        };
        painter.line_segment(
            [
                egui::pos2(rect.left() + 14.0, y),
                egui::pos2(rect.right() - 14.0, y),
            ],
            stroke,
        );
    }

    let inner = rect.shrink2(Vec2::new(22.0, 26.0));
    let count = bands.len().max(1) as f32;
    let points: Vec<egui::Pos2> = bands
        .iter()
        .enumerate()
        .map(|(i, db)| {
            let x = inner.left() + inner.width() * (i as f32 / (count - 1.0).max(1.0));
            let clamped = (*db as f32).clamp(min_db, max_db);
            let t = (max_db - clamped) / (max_db - min_db);
            egui::pos2(x, inner.top() + t * inner.height())
        })
        .collect();

    let curve_color = if enabled {
        palette.accent
    } else {
        palette.text_weak.gamma_multiply(0.7)
    };

    // Filled area under the curve.
    if points.len() >= 2 {
        let mut mesh = egui::Mesh::default();
        let fill = curve_color.gamma_multiply(0.12);
        let base_y = inner.bottom();
        for point in &points {
            mesh.colored_vertex(*point, fill);
        }
        let base_index = mesh.vertices.len() as u32;
        mesh.colored_vertex(egui::pos2(inner.left(), base_y), fill);
        mesh.colored_vertex(egui::pos2(inner.right(), base_y), Color32::TRANSPARENT);
        for i in 0..points.len() as u32 - 1 {
            mesh.add_triangle(i, i + 1, base_index);
            mesh.add_triangle(i + 1, base_index + 1, base_index);
        }
        painter.add(egui::Shape::mesh(mesh));
    }

    painter.add(egui::Shape::line(points.clone(), Stroke::new(2.0_f32, curve_color)));
    for point in &points {
        painter.circle_filled(*point, 4.0, curve_color);
        painter.circle_stroke(*point, 4.0, Stroke::new(1.0_f32, palette.card));
    }

    // Frequency labels.
    for (i, hz) in crate::model::audio::EQ_FREQUENCIES_HZ.iter().enumerate() {
        let x = inner.left() + inner.width() * (i as f32 / (count - 1.0).max(1.0));
        let label = if *hz >= 1000 {
            format!("{:.0}k", *hz as f32 / 1000.0)
        } else {
            hz.to_string()
        };
        painter.text(
            egui::pos2(x, rect.bottom() - 10.0),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(9.5),
            palette.text_weak,
        );
    }
}

/// A pill-style nav item used in the sidebar.
pub fn nav_item(
    ui: &mut Ui,
    palette: Palette,
    icon: &str,
    label: &str,
    selected: bool,
) -> egui::Response {
    let height = 38.0;
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), height),
        Sense::click(),
    );
    let hovered = response.hovered();
    let fill = if selected {
        palette.accent
    } else if hovered {
        palette.card_hover
    } else {
        Color32::TRANSPARENT
    };
    let text_color = if selected {
        palette.accent_text
    } else {
        palette.text
    };
    ui.painter().rect_filled(rect, CornerRadius::same(10), fill);
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(Vec2::new(12.0, 0.0)))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
    );
    child.label(RichText::new(icon).size(14.0).color(text_color));
    child.label(RichText::new(label).size(13.5).color(text_color));
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Read-only key/value row.
pub fn kv_row(ui: &mut Ui, palette: Palette, key: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(key).size(12.5).color(palette.text_weak));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(value).size(12.5).color(palette.text));
        });
    });
}
