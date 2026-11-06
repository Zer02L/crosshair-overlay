use crate::config::CrosshairConfig;
use tiny_skia::{FillRule, Paint, PathBuilder, PixmapMut, Rect, Transform};

pub fn render_crosshair(pixmap: &mut PixmapMut, config: &CrosshairConfig) {
    // 1. Очищаем пиксмап (полностью прозрачный фон)
    pixmap.fill(tiny_skia::Color::TRANSPARENT);

    let center_x = pixmap.width() as f32 / 2.0;
    let center_y = pixmap.height() as f32 / 2.0;

    let main_color = config.color.to_tiny_skia();
    let outline_color = config.outline_color.to_tiny_skia();

    let mut main_paint = Paint::default();
    main_paint.set_color(main_color);
    main_paint.anti_alias = true;

    let mut outline_paint = Paint::default();
    outline_paint.set_color(outline_color);
    outline_paint.anti_alias = true;

    // Вспомогательная функция для отрисовки прямоугольной линии с опциональной обводкой
    let mut draw_bar = |x: f32, y: f32, w: f32, h: f32| {
        if config.outline_enabled && config.outline_thickness > 0.0 {
            let ot = config.outline_thickness;
            if let Some(out_rect) = Rect::from_xywh(x - ot, y - ot, w + ot * 2.0, h + ot * 2.0) {
                let mut pb = PathBuilder::new();
                pb.push_rect(out_rect);
                if let Some(path) = pb.finish() {
                    pixmap.fill_path(&path, &outline_paint, FillRule::Winding, Transform::identity(), None);
                }
            }
        }

        if let Some(rect) = Rect::from_xywh(x, y, w, h) {
            let mut pb = PathBuilder::new();
            pb.push_rect(rect);
            if let Some(path) = pb.finish() {
                pixmap.fill_path(&path, &main_paint, FillRule::Winding, Transform::identity(), None);
            }
        }
    };

    let half_t = config.line_thickness / 2.0;
    let gap = config.gap;
    let len = config.line_length;
    let thick = config.line_thickness;

    // 1. Правая линия
    draw_bar(center_x + gap, center_y - half_t, len, thick);

    // 2. Левая линия
    draw_bar(center_x - gap - len, center_y - half_t, len, thick);

    // 3. Нижняя линия
    draw_bar(center_x - half_t, center_y + gap, thick, len);

    // 4. Верхняя линия (только если не T-style)
    if !config.t_style {
        draw_bar(center_x - half_t, center_y - gap - len, thick, len);
    }

    // 5. Центральная точка
    if config.center_dot && config.center_dot_radius > 0.0 {
        let r = config.center_dot_radius;
        if config.outline_enabled && config.outline_thickness > 0.0 {
            let out_r = r + config.outline_thickness;
            let mut pb = PathBuilder::new();
            pb.push_circle(center_x, center_y, out_r);
            if let Some(path) = pb.finish() {
                pixmap.fill_path(&path, &outline_paint, FillRule::Winding, Transform::identity(), None);
            }
        }

        let mut pb = PathBuilder::new();
        pb.push_circle(center_x, center_y, r);
        if let Some(path) = pb.finish() {
            pixmap.fill_path(&path, &main_paint, FillRule::Winding, Transform::identity(), None);
        }
    }
}
