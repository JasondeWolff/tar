use egui_phosphor::regular as icons;
use uuid::Uuid;

use crate::{project::Project, render_graph::RenderGraphInfo};

#[derive(Debug, Clone, Copy, PartialEq)]
enum Severity {
    Error,
    Warning,
}

enum MessageOrigin {
    File(Uuid),
    RenderGraph,
}

struct ConsoleMessage {
    severity: Severity,
    text: String,
    origin: MessageOrigin,
    line: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConsoleTab {
    id: Uuid,
    show_errors: bool,
    show_warnings: bool,
}

impl Default for ConsoleTab {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            show_errors: true,
            show_warnings: true,
        }
    }
}

impl ConsoleTab {
    const ROW_HEIGHT: f32 = 40.0;
    const ICON_LEFT_PAD: f32 = 8.0;
    const TEXT_LEFT_PAD: f32 = 8.0;

    const ERROR_COLOR: egui::Color32 = egui::Color32::from_rgb(220, 50, 50);
    const WARNING_COLOR: egui::Color32 = egui::Color32::from_rgb(230, 180, 30);

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, project: &Project, rg_info: &RenderGraphInfo) {
        let messages = self.collect_messages(project, rg_info);

        ui.add_space(4.0);
        self.draw_toolbar(ui, &messages);
        ui.separator();

        let filtered: Vec<&ConsoleMessage> = messages
            .iter()
            .filter(|m| match m.severity {
                Severity::Error => self.show_errors,
                Severity::Warning => self.show_warnings,
            })
            .collect();

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show_rows(ui, Self::ROW_HEIGHT, filtered.len(), |ui, row_range| {
                for i in row_range {
                    self.draw_message_row(ui, project, filtered[i]);
                }
            });
    }

    fn collect_messages(
        &self,
        project: &Project,
        rg_info: &RenderGraphInfo,
    ) -> Vec<ConsoleMessage> {
        let rg = project.render_graph();
        let mut messages = Vec::new();

        for (id, shader) in rg.shaders_iter() {
            for (err, line) in shader.get_errors() {
                messages.push(ConsoleMessage {
                    severity: Severity::Error,
                    text: err.clone(),
                    origin: MessageOrigin::File(*id),
                    line: *line,
                });
            }
            for (warn, line) in shader.get_warnings() {
                messages.push(ConsoleMessage {
                    severity: Severity::Warning,
                    text: warn.clone(),
                    origin: MessageOrigin::File(*id),
                    line: *line,
                });
            }
        }

        if let Some(rg_err) = &rg_info.error {
            messages.push(ConsoleMessage {
                severity: Severity::Error,
                text: rg_err.clone(),
                origin: MessageOrigin::RenderGraph,
                line: None,
            });
        }

        for rg_warn in &rg_info.warnings {
            messages.push(ConsoleMessage {
                severity: Severity::Warning,
                text: rg_warn.clone(),
                origin: MessageOrigin::RenderGraph,
                line: None,
            });
        }

        messages
    }

    fn draw_toolbar(&mut self, ui: &mut egui::Ui, messages: &[ConsoleMessage]) {
        let error_count = messages
            .iter()
            .filter(|m| m.severity == Severity::Error)
            .count();
        let warning_count = messages
            .iter()
            .filter(|m| m.severity == Severity::Warning)
            .count();

        ui.horizontal(|ui| {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(8.0);

                // Error toggle
                let error_label =
                    egui::RichText::new(format!("{} {}", icons::X_CIRCLE, error_count));
                let error_btn = if self.show_errors {
                    egui::Button::new(error_label.color(egui::Color32::WHITE))
                        .fill(Self::ERROR_COLOR)
                } else {
                    egui::Button::new(error_label)
                };
                if ui.add(error_btn).clicked() {
                    self.show_errors = !self.show_errors;
                }

                // Warning toggle
                let warning_label =
                    egui::RichText::new(format!("{} {}", icons::WARNING, warning_count));
                let warning_btn = if self.show_warnings {
                    egui::Button::new(warning_label.color(egui::Color32::BLACK))
                        .fill(Self::WARNING_COLOR)
                } else {
                    egui::Button::new(warning_label)
                };
                if ui.add(warning_btn).clicked() {
                    self.show_warnings = !self.show_warnings;
                }
            });
        });
    }

    fn draw_message_row(&self, ui: &mut egui::Ui, project: &Project, message: &ConsoleMessage) {
        let row_rect = ui
            .allocate_space(egui::vec2(ui.available_width(), Self::ROW_HEIGHT))
            .1;

        // Alternating row background
        let row_bg = if (row_rect.min.y / Self::ROW_HEIGHT) as i32 % 2 == 0 {
            ui.visuals().faint_bg_color
        } else {
            egui::Color32::TRANSPARENT
        };
        ui.painter().rect_filled(row_rect, 0.0, row_bg);

        let (icon, icon_color) = match message.severity {
            Severity::Error => (icons::X_CIRCLE, Self::ERROR_COLOR),
            Severity::Warning => (icons::WARNING, Self::WARNING_COLOR),
        };

        // Draw icon
        let icon_pos = egui::pos2(
            row_rect.min.x + Self::ICON_LEFT_PAD,
            row_rect.min.y + Self::ROW_HEIGHT * 0.5,
        );
        ui.painter().text(
            icon_pos,
            egui::Align2::LEFT_CENTER,
            icon,
            egui::FontId::proportional(24.0),
            icon_color,
        );

        // Draw message text (upper portion of the row)
        let text_left = row_rect.min.x + Self::ICON_LEFT_PAD + 24.0 + Self::TEXT_LEFT_PAD;
        let text_pos = egui::pos2(text_left, row_rect.min.y + Self::ROW_HEIGHT * 0.35);
        ui.painter().text(
            text_pos,
            egui::Align2::LEFT_CENTER,
            &message.text,
            egui::FontId::proportional(13.0),
            ui.visuals().text_color(),
        );

        // Draw origin name (bottom-left, smaller and dimmer — Unity style)

        let mut origin_name = match &message.origin {
            MessageOrigin::File(file) => project
                .code_files
                .get_file(*file)
                .map(|file| file.relative_path().file_name().unwrap_or_default())
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default(),
            MessageOrigin::RenderGraph => "Render Graph".to_owned(),
        };

        if !origin_name.is_empty() {
            if let Some(line) = message.line {
                origin_name = format!("{origin_name} ({line})");
            }

            let file_pos = egui::pos2(text_left, row_rect.min.y + Self::ROW_HEIGHT * 0.75);
            ui.painter().text(
                file_pos,
                egui::Align2::LEFT_CENTER,
                &origin_name,
                egui::FontId::proportional(11.0),
                ui.visuals().weak_text_color(),
            );
        }
    }
}
