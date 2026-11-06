mod config;
mod gui;
mod render;
mod sync;
mod watcher;
mod wayland_overlay;
mod web_server;

use config::CrosshairConfig;
use std::num::NonZeroU32;
use std::sync::Arc;
use tiny_skia::Pixmap;
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId, WindowLevel};

#[cfg(target_os = "linux")]
use winit::platform::x11::WindowAttributesExtX11;

struct CrosshairApp {
    config: CrosshairConfig,
    window: Option<Arc<Window>>,
    context: Option<softbuffer::Context<Arc<Window>>>,
    surface: Option<softbuffer::Surface<Arc<Window>, Arc<Window>>>,
}

impl CrosshairApp {
    fn new(config: CrosshairConfig) -> Self {
        Self {
            config,
            window: None,
            context: None,
            surface: None,
        }
    }

    fn draw(&mut self) {
        let (Some(window), Some(surface)) = (&self.window, &mut self.surface) else {
            return;
        };

        let size = window.inner_size();
        let width = size.width;
        let height = size.height;

        if width == 0 || height == 0 {
            return;
        }

        let (Some(nz_w), Some(nz_h)) = (NonZeroU32::new(width), NonZeroU32::new(height)) else {
            return;
        };

        if surface.resize(nz_w, nz_h).is_err() {
            return;
        }

        let mut pixmap = match Pixmap::new(width, height) {
            Some(p) => p,
            None => return,
        };

        let mut pixmap_mut = pixmap.as_mut();
        render::render_crosshair(&mut pixmap_mut, &self.config);

        if let Ok(mut buffer) = surface.buffer_mut() {
            let src = pixmap.data();
            for (i, dst_pixel) in buffer.iter_mut().enumerate() {
                let r = src[i * 4] as u32;
                let g = src[i * 4 + 1] as u32;
                let b = src[i * 4 + 2] as u32;
                let a = src[i * 4 + 3] as u32;

                *dst_pixel = (a << 24) | (r << 16) | (g << 8) | b;
            }

            let _ = buffer.present();
        }
    }
}

impl ApplicationHandler for CrosshairApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let win_size = self.config.window_size;

        let monitor = event_loop.primary_monitor().or_else(|| event_loop.available_monitors().next());

        let pos = if let Some(m) = monitor {
            let m_size = m.size();
            let m_pos = m.position();
            PhysicalPosition::new(
                m_pos.x + (m_size.width as i32 - win_size as i32) / 2,
                m_pos.y + (m_size.height as i32 - win_size as i32) / 2,
            )
        } else {
            PhysicalPosition::new(100, 100)
        };

        let mut window_attrs = WindowAttributes::default()
            .with_title("Crosshair Overlay")
            .with_inner_size(PhysicalSize::new(win_size, win_size))
            .with_position(pos)
            .with_transparent(true)
            .with_decorations(false)
            .with_resizable(false)
            .with_window_level(WindowLevel::AlwaysOnTop);

        #[cfg(target_os = "linux")]
        {
            window_attrs = window_attrs.with_override_redirect(true);
        }

        let window = match event_loop.create_window(window_attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                eprintln!("Не удалось создать окно оверлея: {:?}", e);
                return;
            }
        };

        if let Err(e) = window.set_cursor_hittest(false) {
            eprintln!("Предупреждение: не удалось включить сквозной клик: {:?}", e);
        }

        let context = softbuffer::Context::new(window.clone()).expect("Failed to create softbuffer context");
        let surface = softbuffer::Surface::new(&context, window.clone()).expect("Failed to create surface");

        self.window = Some(window);
        self.context = Some(context);
        self.surface = Some(surface);

        self.draw();
    }

    fn window_event(&mut self, _event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::RedrawRequested => {
                self.draw();
            }
            WindowEvent::CloseRequested => {
                std::process::exit(0);
            }
            _ => (),
        }
    }
}

fn main() {
    println!("=== Crosshair X Alternative (High Performance Rust Overlay) ===");

    let args: Vec<String> = std::env::args().collect();
    let no_gui = args.iter().any(|a| a == "--no-gui" || a == "--headless");

    // Загружаем конфиг или создаем дефолтный файл crosshair_config.json
    let config_path = "crosshair_config.json";
    let config = CrosshairConfig::load_or_create(config_path);
    println!(
        "Конфигурация загружена: окно {}x{}, цвет RGBA({}, {}, {}, {})",
        config.window_size,
        config.window_size,
        config.color.r,
        config.color.g,
        config.color.b,
        config.color.a
    );

    // 1. Если мы работаем под Wayland (KDE Plasma, Sway, Hyprland и др.),
    // используем нативный Wayland Layer-Shell (Layer::Overlay)
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        println!("Обнаружен Wayland, инициализация нативного Layer-Shell оверлея...");

        // Создаем канал для приема обновленного конфига оверлеем
        let (tx, rx) = wayland_overlay::create_overlay_channel();

        // Инициализируем единое разделяемое состояние приложения (Unified Sync State)
        let shared = sync::SharedState::new(
            config.clone(),
            std::path::PathBuf::from(config_path),
            tx,
        );

        // Запускаем watcher файла конфигурации
        let _watcher = match watcher::start_config_watcher(config_path, shared.clone()) {
            Ok(w) => {
                println!("👀 [Live Reload] Отслеживание изменений файла конфига включено.");
                Some(w)
            }
            Err(e) => {
                eprintln!("Предупреждение: не удалось запустить file watcher: {:?}", e);
                None
            }
        };

        // Запускаем встроенный локальный веб-сервер и API для мобильных/ПК пультов управления
        web_server::start_web_server(shared.clone(), 4848);

        if no_gui {
            println!("🚀 Запуск в фоновом/headless режиме (--no-gui).");
            if let Err(err) = wayland_overlay::run_wayland_overlay(config, rx) {
                eprintln!("Ошибка Wayland Layer-Shell: {:?}", err);
            }
            return;
        }

        // Запуск оверлея в отдельном потоке
        let overlay_config = config.clone();
        let overlay_handle = std::thread::Builder::new()
            .name("wayland-overlay".into())
            .spawn(move || {
                if let Err(err) = wayland_overlay::run_wayland_overlay(overlay_config, rx) {
                    eprintln!("Ошибка потока Wayland оверлея: {:?}", err);
                }
            })
            .expect("Failed to spawn wayland overlay thread");

        // Запуск нативного окна настроек на egui в главном потоке
        println!("🖥️ Запуск нативного окна настроек на egui...");
        if let Err(e) = gui::run_gui(shared) {
            eprintln!("Ошибка GUI окна настроек: {:?}", e);
        }

        println!("💡 Окно настроек закрыто. Оверлей и веб-сервер продолжают работать в фоне.");
        println!("   (Для выхода нажмите Ctrl+C в терминале или кнопку 'Закрыть всё' в меню)");
        let _ = overlay_handle.join();
        return;
    }

    // 2. Fallback для чистого X11 / XWayland
    println!("Предупреждение: WAYLAND_DISPLAY не найден, переключение на X11 fallback...");
    let event_loop = EventLoop::new().expect("Failed to create EventLoop");
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = CrosshairApp::new(config);
    event_loop.run_app(&mut app).expect("Event loop error");
}
