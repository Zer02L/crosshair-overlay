use crate::config::{CrosshairConfig, RgbaColor};
use crate::sync::{ConfigSource, SharedState};
use eframe::egui::{self, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub struct SettingsApp {
    shared: Arc<SharedState>,
    config: CrosshairConfig,
    last_version: u64,
    local_ip: String,
    copied_time: Option<Instant>,
    status_msg: Option<(String, Instant)>,
}

impl SettingsApp {
    pub fn new(cc: &eframe::CreationContext<'_>, shared: Arc<SharedState>) -> Self {
        shared.set_gui_context(cc.egui_ctx.clone());

        // Настраиваем современную темную тему
        let mut visuals = egui::Visuals::dark();
        visuals.override_text_color = Some(Color32::from_rgb(230, 235, 245));
        visuals.panel_fill = Color32::from_rgb(15, 18, 26);
        visuals.window_fill = Color32::from_rgb(18, 22, 32);
        visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(22, 27, 40);
        visuals.widgets.inactive.bg_fill = Color32::from_rgb(28, 35, 51);
        visuals.widgets.hovered.bg_fill = Color32::from_rgb(38, 48, 70);
        visuals.widgets.active.bg_fill = Color32::from_rgb(45, 58, 85);
        cc.egui_ctx.set_visuals(visuals);

        let config = shared.get_config();
        let last_version = shared.get_version();
        let local_ip = crate::sync::get_local_ip().unwrap_or_else(|| "127.0.0.1".into());

        Self {
            shared,
            config,
            last_version,
            local_ip,
            copied_time: None,
            status_msg: None,
        }
    }

    fn draw_preview(&self, ui: &mut egui::Ui) {
        let preview_size = Vec2::new(160.0, 160.0);
        let (response, painter) = ui.allocate_painter(preview_size, Sense::hover());
        let rect = response.rect;
        let center = rect.center();

        // Фон превью с сеткой
        painter.rect_filled(rect, 8.0, Color32::from_rgb(13, 16, 24));
        painter.rect_stroke(
            rect,
            8.0,
            Stroke::new(1.0, Color32::from_rgb(35, 43, 62)),
            egui::StrokeKind::Inside,
        );

        // Опорная разметка прицеливания (сетка)
        let grid_col = Color32::from_rgba_unmultiplied(255, 255, 255, 25);
        painter.line_segment(
            [Pos2::new(center.x - 12.0, center.y), Pos2::new(center.x + 12.0, center.y)],
            Stroke::new(0.5, grid_col),
        );
        painter.line_segment(
            [Pos2::new(center.x, center.y - 12.0), Pos2::new(center.x, center.y + 12.0)],
            Stroke::new(0.5, grid_col),
        );

        let cfg = &self.config;
        let color = Color32::from_rgba_unmultiplied(cfg.color.r, cfg.color.g, cfg.color.b, cfg.color.a);
        let outline_color = Color32::from_rgba_unmultiplied(
            cfg.outline_color.r,
            cfg.outline_color.g,
            cfg.outline_color.b,
            cfg.outline_color.a,
        );
        let ot = if cfg.outline_enabled { cfg.outline_thickness } else { 0.0 };
        let len = cfg.line_length;
        let thick = cfg.line_thickness;
        let gap = cfg.gap;
        let half_t = thick / 2.0;

        let draw_bar = |x1: f32, y1: f32, x2: f32, y2: f32| {
            if ot > 0.0 {
                let out_rect = Rect::from_min_max(
                    Pos2::new(x1 - ot, y1 - ot),
                    Pos2::new(x2 + ot, y2 + ot),
                );
                painter.rect_filled(out_rect, 0.0, outline_color);
            }
            let bar_rect = Rect::from_min_max(Pos2::new(x1, y1), Pos2::new(x2, y2));
            painter.rect_filled(bar_rect, 0.0, color);
        };

        if len > 0.0 {
            // Правая линия
            draw_bar(center.x + gap, center.y - half_t, center.x + gap + len, center.y + half_t);
            // Левая линия
            draw_bar(center.x - gap - len, center.y - half_t, center.x - gap, center.y + half_t);
            // Нижняя линия
            draw_bar(center.x - half_t, center.y + gap, center.x + half_t, center.y + gap + len);
            // Верхняя линия (если не T-стиль)
            if !cfg.t_style {
                draw_bar(center.x - half_t, center.y - gap - len, center.x + half_t, center.y - gap);
            }
        }

        // Центральная точка
        if cfg.center_dot && cfg.center_dot_radius > 0.0 {
            if ot > 0.0 {
                painter.circle_filled(center, cfg.center_dot_radius + ot, outline_color);
            }
            painter.circle_filled(center, cfg.center_dot_radius, color);
        }
    }
}

impl eframe::App for SettingsApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // 1. Проверяем изменения извне (от мобильного пульта или файла)
        let current_version = self.shared.get_version();
        if current_version != self.last_version {
            self.config = self.shared.get_config();
            self.last_version = current_version;
        }

        let old_config = self.config.clone();

        egui::CentralPanel::default().show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add_space(4.0);

                // --- Заголовок и статус ---
                ui.horizontal(|ui| {
                    ui.heading("🎯 Crosshair Overlay");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new("● LIVE SYNC")
                                .color(Color32::from_rgb(52, 211, 153))
                                .strong(),
                        );
                    });
                });

                ui.add_space(6.0);

                // --- Карточка мобильного пульта управления ---
                egui::Frame::new()
                    .fill(Color32::from_rgb(20, 26, 38))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(42, 54, 78)))
                    .corner_radius(8.0)
                    .inner_margin(egui::Margin::same(10))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label("📱");
                            let url = format!("http://{}:4848", self.local_ip);
                            ui.vertical(|ui| {
                                ui.label(
                                    egui::RichText::new("Мобильный пульт управления:")
                                        .small()
                                        .color(Color32::from_rgb(160, 175, 200)),
                                );
                                ui.monospace(
                                    egui::RichText::new(&url)
                                        .color(Color32::from_rgb(96, 165, 250))
                                        .strong(),
                                );
                            });

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let is_recently_copied = self
                                    .copied_time
                                    .map_or(false, |t| t.elapsed() < Duration::from_secs(2));

                                let btn_text = if is_recently_copied {
                                    "✓ Скопировано"
                                } else {
                                    "📋 Копировать"
                                };

                                if ui.button(btn_text).clicked() {
                                    ui.ctx().copy_text(url);
                                    self.copied_time = Some(Instant::now());
                                }
                            });
                        });
                    });

                ui.add_space(10.0);

                // --- Секция: Интерактивное превью + Быстрые пресеты ---
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new("Превью в реальном времени").strong());
                        self.draw_preview(ui);
                    });

                    ui.add_space(10.0);

                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new("Быстрые пресеты").strong());
                        ui.add_space(4.0);

                        egui::Grid::new("presets_grid")
                            .spacing(Vec2::new(6.0, 6.0))
                            .show(ui, |ui| {
                                if ui.button("🎯 CS Classic").clicked() {
                                    self.config.line_length = 10.0;
                                    self.config.line_thickness = 2.0;
                                    self.config.gap = 4.0;
                                    self.config.center_dot = false;
                                    self.config.t_style = false;
                                    self.config.outline_enabled = true;
                                    self.config.color = RgbaColor::new(0, 255, 100, 255);
                                }
                                if ui.button("🔴 Cyber Dot").clicked() {
                                    self.config.line_length = 0.0;
                                    self.config.gap = 0.0;
                                    self.config.center_dot = true;
                                    self.config.center_dot_radius = 2.5;
                                    self.config.outline_enabled = true;
                                    self.config.color = RgbaColor::new(0, 230, 255, 255);
                                }
                                ui.end_row();

                                if ui.button("⚡ T-Style").clicked() {
                                    self.config.line_length = 11.0;
                                    self.config.line_thickness = 2.0;
                                    self.config.gap = 3.0;
                                    self.config.center_dot = false;
                                    self.config.t_style = true;
                                    self.config.outline_enabled = true;
                                    self.config.color = RgbaColor::new(255, 220, 0, 255);
                                }
                                if ui.button("🔻 Apex Red").clicked() {
                                    self.config.line_length = 8.0;
                                    self.config.line_thickness = 1.5;
                                    self.config.gap = 2.0;
                                    self.config.center_dot = false;
                                    self.config.t_style = false;
                                    self.config.outline_enabled = true;
                                    self.config.color = RgbaColor::new(255, 45, 85, 255);
                                }
                                ui.end_row();

                                if ui.button("🟣 Valorant").clicked() {
                                    self.config.line_length = 6.0;
                                    self.config.line_thickness = 2.0;
                                    self.config.gap = 3.0;
                                    self.config.center_dot = false;
                                    self.config.t_style = false;
                                    self.config.outline_enabled = true;
                                    self.config.color = RgbaColor::new(220, 50, 255, 255);
                                }
                                if ui.button("⚪ Minimalist").clicked() {
                                    self.config.line_length = 5.0;
                                    self.config.line_thickness = 1.5;
                                    self.config.gap = 3.0;
                                    self.config.center_dot = true;
                                    self.config.center_dot_radius = 1.0;
                                    self.config.outline_enabled = true;
                                    self.config.color = RgbaColor::new(255, 255, 255, 255);
                                }
                                ui.end_row();
                            });
                    });
                });

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(8.0);

                // --- Геометрия линий ---
                ui.label(egui::RichText::new("Размеры линий").strong());
                ui.horizontal(|ui| {
                    ui.label("Длина (Length):");
                    ui.add(egui::Slider::new(&mut self.config.line_length, 0.0..=40.0).suffix(" px"));
                });
                ui.horizontal(|ui| {
                    ui.label("Толщина (Thickness):");
                    ui.add(egui::Slider::new(&mut self.config.line_thickness, 0.5..=15.0).suffix(" px"));
                });
                ui.horizontal(|ui| {
                    ui.label("Зазор от центра (Gap):");
                    ui.add(egui::Slider::new(&mut self.config.gap, 0.0..=35.0).suffix(" px"));
                });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                // --- Центральная точка и стиль ---
                ui.label(egui::RichText::new("Стиль и точка в центре").strong());
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.config.center_dot, "Точка в центре (Center Dot)");
                    ui.add_space(10.0);
                    ui.checkbox(&mut self.config.t_style, "T-образный (без верхней линии)");
                });

                if self.config.center_dot {
                    ui.horizontal(|ui| {
                        ui.label("Радиус точки:");
                        ui.add(egui::Slider::new(&mut self.config.center_dot_radius, 0.5..=10.0).suffix(" px"));
                    });
                }

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                // --- Цвет прицела ---
                ui.label(egui::RichText::new("Цвет прицела").strong());
                ui.horizontal(|ui| {
                    let mut color_arr = [
                        self.config.color.r,
                        self.config.color.g,
                        self.config.color.b,
                        self.config.color.a,
                    ];
                    ui.color_edit_button_srgba_unmultiplied(&mut color_arr);
                    if color_arr != [self.config.color.r, self.config.color.g, self.config.color.b, self.config.color.a] {
                        self.config.color.r = color_arr[0];
                        self.config.color.g = color_arr[1];
                        self.config.color.b = color_arr[2];
                        self.config.color.a = color_arr[3];
                    }

                    ui.label(format!(
                        "RGBA({}, {}, {}, {})",
                        self.config.color.r, self.config.color.g, self.config.color.b, self.config.color.a
                    ));

                    // Быстрые палитры
                    if ui.button("🟢").on_hover_text("Неоновый зеленый").clicked() {
                        self.config.color = RgbaColor::new(0, 255, 100, 255);
                    }
                    if ui.button("🔵").on_hover_text("Неоновый синий").clicked() {
                        self.config.color = RgbaColor::new(0, 200, 255, 255);
                    }
                    if ui.button("🔴").on_hover_text("Ярко-красный").clicked() {
                        self.config.color = RgbaColor::new(255, 45, 85, 255);
                    }
                    if ui.button("🟡").on_hover_text("Желтый").clicked() {
                        self.config.color = RgbaColor::new(255, 235, 0, 255);
                    }
                    if ui.button("🟣").on_hover_text("Пурпурный").clicked() {
                        self.config.color = RgbaColor::new(220, 50, 255, 255);
                    }
                    if ui.button("⚪").on_hover_text("Белый").clicked() {
                        self.config.color = RgbaColor::new(255, 255, 255, 255);
                    }
                });

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                // --- Контурная обводка (Outline) ---
                ui.label(egui::RichText::new("Контурная обводка (Outline)").strong());
                ui.checkbox(&mut self.config.outline_enabled, "Включить обводку для контраста");

                if self.config.outline_enabled {
                    ui.horizontal(|ui| {
                        ui.label("Толщина обводки:");
                        ui.add(egui::Slider::new(&mut self.config.outline_thickness, 0.5..=6.0).suffix(" px"));
                    });

                    ui.horizontal(|ui| {
                        ui.label("Цвет обводки:");
                        let mut out_arr = [
                            self.config.outline_color.r,
                            self.config.outline_color.g,
                            self.config.outline_color.b,
                            self.config.outline_color.a,
                        ];
                        ui.color_edit_button_srgba_unmultiplied(&mut out_arr);
                        if out_arr != [
                            self.config.outline_color.r,
                            self.config.outline_color.g,
                            self.config.outline_color.b,
                            self.config.outline_color.a,
                        ] {
                            self.config.outline_color.r = out_arr[0];
                            self.config.outline_color.g = out_arr[1];
                            self.config.outline_color.b = out_arr[2];
                            self.config.outline_color.a = out_arr[3];
                        }

                        if ui.button("Черная обводка").clicked() {
                            self.config.outline_color = RgbaColor::new(0, 0, 0, 220);
                        }
                    });
                }

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(8.0);

                // --- Нижняя панель действий ---
                ui.horizontal(|ui| {
                    if ui.button("🔄 Сброс по умолчанию").clicked() {
                        self.config = CrosshairConfig::default();
                        self.status_msg = Some(("Сброшено на настройки по умолчанию".into(), Instant::now()));
                    }

                    if ui.button("💾 Сохранить").clicked() {
                        self.shared.update_config(self.config.clone(), ConfigSource::Gui);
                        self.status_msg = Some(("Конфигурация сохранена на диск".into(), Instant::now()));
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(egui::RichText::new("❌ Закрыть всё").color(Color32::from_rgb(248, 113, 113))).clicked() {
                            std::process::exit(0);
                        }
                    });
                });

                if let Some((msg, time)) = &self.status_msg {
                    if time.elapsed() < Duration::from_secs(3) {
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new(msg).small().color(Color32::from_rgb(52, 211, 153)));
                    }
                }

                ui.add_space(8.0);
            });
        });

        // 3. Если пользователь изменил любой параметр в GUI — мгновенно синхронизируем!
        if self.config != old_config {
            self.shared.update_config(self.config.clone(), ConfigSource::Gui);
            self.last_version = self.shared.get_version();
        }
    }
}

pub fn run_gui(shared: Arc<SharedState>) -> eframe::Result {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([460.0, 740.0])
            .with_min_inner_size([400.0, 520.0])
            .with_title("🎯 Crosshair Overlay Settings")
            .with_active(true),
        ..Default::default()
    };

    eframe::run_native(
        "Crosshair Overlay Settings",
        native_options,
        Box::new(|cc| Ok(Box::new(SettingsApp::new(cc, shared)))),
    )
}
