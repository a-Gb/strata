//! Shared application styling, control primitives, and GPU bootstrap.
#![allow(clippy::redundant_pub_crate, clippy::wildcard_imports)]

use super::*;

pub(super) fn configure_workbench_style(context: &egui::Context) {
    context.set_theme(egui::Theme::Dark);
    let mut style = (*context.global_style()).clone();
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = UI_SHELL_BG;
    visuals.window_fill = UI_RAIL_BG;
    visuals.extreme_bg_color = egui::Color32::from_rgb(10, 14, 17);
    visuals.faint_bg_color = UI_RAIL_ALT;
    visuals.code_bg_color = egui::Color32::from_rgb(13, 19, 23);
    visuals.text_edit_bg_color = Some(egui::Color32::from_rgb(17, 23, 27));
    visuals.override_text_color = Some(UI_TEXT);
    visuals.weak_text_color = Some(UI_MUTED);
    visuals.selection.bg_fill = egui::Color32::from_rgb(17, 91, 132);
    visuals.selection.stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
    visuals.hyperlink_color = UI_CYAN;
    visuals.slider_trailing_fill = true;
    visuals.widgets.noninteractive.bg_fill = UI_RAIL_BG;
    visuals.widgets.noninteractive.weak_bg_fill = UI_RAIL_ALT;
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, UI_BORDER);
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, UI_TEXT);
    visuals.widgets.inactive.bg_fill = UI_RAIL_ALT;
    visuals.widgets.inactive.weak_bg_fill = UI_RAIL_ALT;
    visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, UI_BORDER);
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, UI_TEXT);
    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(38, 53, 62);
    visuals.widgets.hovered.weak_bg_fill = egui::Color32::from_rgb(38, 53, 62);
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, UI_CYAN);
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
    visuals.widgets.active.bg_fill = egui::Color32::from_rgb(16, 99, 145);
    visuals.widgets.active.weak_bg_fill = egui::Color32::from_rgb(16, 99, 145);
    visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, UI_CYAN);
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
    visuals.widgets.open = visuals.widgets.active;
    let widget_radius = egui::CornerRadius::same(4);
    visuals.widgets.noninteractive.corner_radius = widget_radius;
    visuals.widgets.inactive.corner_radius = widget_radius;
    visuals.widgets.hovered.corner_radius = widget_radius;
    visuals.widgets.active.corner_radius = widget_radius;
    visuals.widgets.open.corner_radius = widget_radius;
    visuals.widgets.noninteractive.expansion = 0.0;
    visuals.widgets.inactive.expansion = 0.0;
    visuals.widgets.hovered.expansion = 0.0;
    visuals.widgets.active.expansion = 0.0;
    visuals.widgets.open.expansion = 0.0;
    style.visuals = visuals;
    style.animation_time = 0.0;
    style.spacing.item_spacing = egui::vec2(7.0, 5.0);
    style.spacing.button_padding = egui::vec2(9.0, 5.0);
    style.spacing.interact_size.y = 27.0;
    style
        .text_styles
        .insert(egui::TextStyle::Heading, egui::FontId::proportional(17.0));
    style
        .text_styles
        .insert(egui::TextStyle::Body, egui::FontId::proportional(13.0));
    style
        .text_styles
        .insert(egui::TextStyle::Button, egui::FontId::proportional(12.5));
    style
        .text_styles
        .insert(egui::TextStyle::Small, egui::FontId::proportional(10.5));
    style
        .text_styles
        .insert(egui::TextStyle::Monospace, egui::FontId::monospace(11.5));
    context.set_global_style(style);
}

pub(super) fn initialize_gpu_backend(
    context: &eframe::CreationContext<'_>,
) -> (Option<WgpuP1Backend>, String) {
    let Some(render_state) = context.wgpu_render_state.as_ref() else {
        return (
            None,
            "CPU fallback · WGPU renderer state unavailable".to_owned(),
        );
    };
    let backend = match WgpuP1Backend::from_device(
        &render_state.adapter,
        render_state.device.clone(),
        render_state.queue.clone(),
    ) {
        Ok(backend) => backend,
        Err(error) => return (None, format!("CPU fallback · GPU compile failed: {error}")),
    };
    match backend.verify() {
        Ok(report) => (
            Some(backend),
            format!(
                "GPU VERIFIED · {} / {} · Δ≤{:.2e}",
                report.backend, report.adapter_name, report.maximum_component_error
            ),
        ),
        Err(error) => (
            None,
            format!("CPU fallback · GPU differential failed: {error}"),
        ),
    }
}

pub(super) fn rail_frame(fill: egui::Color32, margin: i8) -> egui::Frame {
    egui::Frame::new()
        .fill(fill)
        .inner_margin(egui::Margin::same(margin))
        .stroke(egui::Stroke::new(1.0, UI_BORDER))
}

pub(super) fn control_section(
    ui: &mut egui::Ui,
    number: usize,
    title: &str,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    egui::Frame::new()
        .fill(UI_CARD_BG)
        .stroke(egui::Stroke::new(1.0, UI_BORDER))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::same(9))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(number.to_string())
                        .monospace()
                        .strong()
                        .color(UI_CYAN),
                );
                ui.label(egui::RichText::new(title).strong().size(11.0));
            });
            ui.add_space(3.0);
            ui.separator();
            ui.add_space(3.0);
            add_contents(ui);
        });
    ui.add_space(7.0);
}

pub(super) fn rail_title(ui: &mut egui::Ui, title: &str, trailing: &str) {
    let available = ui.available_width();
    let gap = ui.spacing().item_spacing.x;
    let trailing_width = (available * 0.48).clamp(72.0, 180.0);
    let title_width = (available - trailing_width - gap).max(48.0);
    ui.horizontal(|ui| {
        ui.add_sized(
            [title_width, 20.0],
            egui::Label::new(egui::RichText::new(title).strong().size(11.5)).truncate(),
        );
        ui.allocate_ui_with_layout(
            egui::vec2(trailing_width, 20.0),
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| {
                ui.add(
                    egui::Label::new(egui::RichText::new(trailing).color(UI_MUTED).size(10.5))
                        .truncate(),
                )
                .on_hover_text(trailing);
            },
        );
    });
    ui.separator();
    ui.add_space(4.0);
}

pub(super) fn rail_group_label(ui: &mut egui::Ui, label: &str) {
    ui.label(
        egui::RichText::new(label)
            .strong()
            .size(10.5)
            .color(UI_MUTED),
    );
}

pub(super) fn rail_selectable(
    ui: &mut egui::Ui,
    selected: bool,
    label: impl Into<String>,
) -> egui::Response {
    let button = egui::Button::new(label.into())
        .selected(selected)
        .frame(true)
        .frame_when_inactive(true)
        .truncate()
        .corner_radius(egui::CornerRadius::same(4));
    ui.add_sized([ui.available_width(), RAIL_CONTROL_HEIGHT], button)
}

pub(super) fn rail_segmented<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    selected: &mut T,
    options: &[(T, &str)],
) -> bool {
    if options.is_empty() {
        return false;
    }
    let previous = *selected;
    let segment_width = rail_segment_width(ui.available_width(), options.len());
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = RAIL_SEGMENT_GAP;
        for &(value, label) in options {
            let is_selected = *selected == value;
            let button = egui::Button::new(label)
                .selected(is_selected)
                .frame(true)
                .frame_when_inactive(true)
                .truncate()
                .corner_radius(egui::CornerRadius::same(4));
            if ui
                .add_sized([segment_width, RAIL_CONTROL_HEIGHT], button)
                .clicked()
            {
                *selected = value;
            }
        }
    });
    *selected != previous
}

pub(super) fn rail_segment_width(available_width: f32, option_count: usize) -> f32 {
    let Ok(option_count) = u16::try_from(option_count) else {
        return 0.0;
    };
    if option_count == 0 {
        return 0.0;
    }
    let gaps = RAIL_SEGMENT_GAP * f32::from(option_count.saturating_sub(1));
    ((available_width - gaps).max(0.0) / f32::from(option_count)).floor()
}

pub(super) fn rail_projection_grid(ui: &mut egui::Ui, selected: &mut ProjectionKind) -> bool {
    let previous = *selected;
    for row in ProjectionKind::BASIC.chunks(3) {
        ui.columns(3, |columns| {
            for (column, projection) in columns.iter_mut().zip(row.iter().copied()) {
                if rail_selectable(column, *selected == projection, projection.short_label())
                    .on_hover_text(projection.label())
                    .clicked()
                {
                    *selected = projection;
                }
            }
        });
    }
    *selected != previous
}

pub(super) fn rail_projection_combo(
    ui: &mut egui::Ui,
    id: &str,
    label: &str,
    value: &mut ProjectionKind,
) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [24.0, RAIL_CONTROL_HEIGHT],
            egui::Label::new(egui::RichText::new(label).strong().monospace()),
        );
        egui::ComboBox::from_id_salt(id)
            .selected_text(value.short_label())
            .width((ui.available_width() - 4.0).max(80.0))
            .show_ui(ui, |ui| {
                for projection in ProjectionKind::BASIC {
                    ui.selectable_value(value, projection, projection.label());
                }
                ui.separator();
                ui.label(
                    egui::RichText::new("P1 ANALYTICAL PROJECTIONS")
                        .weak()
                        .size(9.5),
                );
                for projection in ProjectionKind::P1 {
                    ui.selectable_value(value, projection, projection.label());
                }
                ui.separator();
                ui.label(
                    egui::RichText::new("ADVANCED ADDRESS PATHS")
                        .weak()
                        .size(9.5),
                );
                for projection in [
                    ProjectionKind::PolarAddressPath,
                    ProjectionKind::HelicalAddressPath,
                ] {
                    ui.selectable_value(value, projection, projection.label());
                }
            });
    });
}

pub(super) fn rail_dimensions(ui: &mut egui::Ui, dimensions: &mut ProjectionDimensions) {
    rail_segmented(
        ui,
        dimensions,
        &[
            (ProjectionDimensions::Two, "2D"),
            (ProjectionDimensions::Three, "3D"),
        ],
    );
}

pub(super) fn rail_slider_row(
    ui: &mut egui::Ui,
    label: &str,
    value: impl Into<String>,
    slider: egui::Slider<'_>,
) -> bool {
    let value = value.into();
    let label_width = 88.0_f32.min(ui.available_width() * 0.32);
    let value_width = 50.0_f32.min(ui.available_width() * 0.2);
    let slider_width = ui
        .spacing()
        .item_spacing
        .x
        .mul_add(-2.0, ui.available_width() - label_width - value_width)
        .max(32.0);
    ui.horizontal(|ui| {
        ui.add_sized(
            [label_width, RAIL_CONTROL_HEIGHT],
            egui::Label::new(egui::RichText::new(label).size(11.5)).truncate(),
        );
        let changed = ui
            .add_sized(
                [slider_width, RAIL_CONTROL_HEIGHT],
                slider.show_value(false),
            )
            .on_hover_text(format!("{label}: {value}"))
            .changed();
        ui.allocate_ui_with_layout(
            egui::vec2(value_width, RAIL_CONTROL_HEIGHT),
            egui::Layout::right_to_left(egui::Align::Center),
            |ui| {
                ui.label(
                    egui::RichText::new(value)
                        .monospace()
                        .size(10.5)
                        .color(UI_TEXT),
                );
            },
        );
        changed
    })
    .inner
}

pub(super) fn rail_action(ui: &mut egui::Ui, label: &str) -> bool {
    ui.add_sized(
        [ui.available_width(), RAIL_CONTROL_HEIGHT],
        egui::Button::new(label)
            .frame(true)
            .truncate()
            .corner_radius(egui::CornerRadius::same(4)),
    )
    .clicked()
}

pub(super) fn rail_action_enabled(ui: &mut egui::Ui, enabled: bool, label: &str) -> bool {
    ui.add_enabled_ui(enabled, |ui| rail_action(ui, label))
        .inner
}

pub(super) fn dossier_metric(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.vertical(|ui| {
        ui.label(
            egui::RichText::new(label)
                .monospace()
                .size(9.0)
                .color(UI_MUTED),
        );
        ui.add(
            egui::Label::new(
                egui::RichText::new(value)
                    .monospace()
                    .strong()
                    .size(11.0)
                    .color(UI_TEXT),
            )
            .truncate(),
        )
        .on_hover_text(value);
    });
}

pub(super) fn comparison_source_card(
    ui: &mut egui::Ui,
    label: &str,
    name: &str,
    detail: &str,
    attached: bool,
) {
    egui::Frame::new()
        .fill(egui::Color32::from_rgb(20, 29, 34))
        .stroke(egui::Stroke::new(
            1.0,
            if attached { UI_TEAL } else { UI_BORDER },
        ))
        .corner_radius(egui::CornerRadius::same(5))
        .inner_margin(egui::Margin::same(11))
        .show(ui, |ui| {
            ui.label(
                egui::RichText::new(label)
                    .monospace()
                    .strong()
                    .size(10.0)
                    .color(if attached { UI_TEAL } else { UI_MUTED }),
            );
            ui.add(egui::Label::new(egui::RichText::new(name).strong().size(12.0)).truncate())
                .on_hover_text(name);
            ui.weak(detail);
            ui.add_space(8.0);
            ui.colored_label(
                if attached { UI_TEAL } else { UI_MUTED },
                if attached { "ATTACHED" } else { "REQUIRED" },
            );
        });
}

pub(super) const fn dossier_link_state_label(state: DossierLinkState) -> &'static str {
    match state {
        DossierLinkState::Candidate => "○",
        DossierLinkState::Supported => "●",
        DossierLinkState::Tested => "◐",
        DossierLinkState::Rejected => "×",
        DossierLinkState::Context => "↳",
    }
}
