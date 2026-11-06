use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RgbaColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl RgbaColor {
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn to_tiny_skia(&self) -> tiny_skia::Color {
        tiny_skia::Color::from_rgba8(self.r, self.g, self.b, self.a)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CrosshairConfig {
    /// Размер окна оверлея в пикселях (компактное окно 128x128 не нагружает GPU/ОЗУ)
    pub window_size: u32,
    
    /// Длина каждой из 4 линий прицела
    pub line_length: f32,
    
    /// Толщина линий
    pub line_thickness: f32,
    
    /// Зазор (gap) от центра до начала линий
    pub gap: f32,
    
    /// Наличие точки в центре
    pub center_dot: bool,
    
    /// Радиус центральной точки
    pub center_dot_radius: f32,
    
    /// Стиль буквы T (без верхней линии, популярно в CS2/Valorant)
    pub t_style: bool,
    
    /// Наличие внешней обводки (контура) для контраста на любом фоне
    pub outline_enabled: bool,
    pub outline_thickness: f32,
    pub outline_color: RgbaColor,

    /// Основной цвет прицела (по умолчанию неоновый зеленый)
    pub color: RgbaColor,
}

impl Default for CrosshairConfig {
    fn default() -> Self {
        Self {
            window_size: 128,
            line_length: 10.0,
            line_thickness: 2.0,
            gap: 5.0,
            center_dot: false,
            center_dot_radius: 1.5,
            t_style: false,
            outline_enabled: true,
            outline_thickness: 1.0,
            outline_color: RgbaColor::new(0, 0, 0, 220),
            color: RgbaColor::new(0, 255, 100, 255), // Киберспортивный зеленый
        }
    }
}

impl CrosshairConfig {
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let data = fs::read_to_string(path)?;
        let cfg = serde_json::from_str(&data)?;
        Ok(cfg)
    }

    pub fn load_or_create<P: AsRef<Path>>(path: P) -> Self {
        let path = path.as_ref();
        if path.exists() {
            if let Ok(cfg) = Self::load_from_file(path) {
                return cfg;
            }
        }

        let default_cfg = Self::default();
        if let Ok(json) = serde_json::to_string_pretty(&default_cfg) {
            let _ = fs::write(path, json);
        }
        default_cfg
    }
}
