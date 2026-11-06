use crate::config::CrosshairConfig;
use crate::render;
use smithay_client_toolkit::reexports::calloop::channel::{channel, Event as ChannelEvent};
use smithay_client_toolkit::reexports::calloop::EventLoop;
use smithay_client_toolkit::reexports::calloop_wayland_source::WaylandSource;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_layer, delegate_output, delegate_registry, delegate_shm,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    shell::{
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
        WaylandSurface,
    },
    shm::{slot::SlotPool, Shm, ShmHandler},
};
use std::path::Path;
use tiny_skia::Pixmap;
use wayland_client::{
    globals::registry_queue_init,
    protocol::{wl_output, wl_shm, wl_surface},
    Connection, QueueHandle,
};

pub struct WaylandOverlay {
    registry_state: RegistryState,
    output_state: OutputState,
    shm: Shm,
    pool: SlotPool,
    layer: LayerSurface,
    config: CrosshairConfig,
    width: u32,
    height: u32,
    exit: bool,
}

impl CompositorHandler for WaylandOverlay {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_factor: i32,
    ) {
    }

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
    }

    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for WaylandOverlay {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }
}

impl LayerShellHandler for WaylandOverlay {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        self.exit = true;
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        if configure.new_size.0 != 0 && configure.new_size.1 != 0 {
            self.width = configure.new_size.0;
            self.height = configure.new_size.1;
        }
        self.draw();
    }
}

impl ShmHandler for WaylandOverlay {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl ProvidesRegistryState for WaylandOverlay {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState];
}

delegate_compositor!(WaylandOverlay);
delegate_output!(WaylandOverlay);
delegate_shm!(WaylandOverlay);
delegate_layer!(WaylandOverlay);
delegate_registry!(WaylandOverlay);

impl wayland_client::Dispatch<wayland_client::protocol::wl_region::WlRegion, ()> for WaylandOverlay {
    fn event(
        _state: &mut Self,
        _proxy: &wayland_client::protocol::wl_region::WlRegion,
        _event: wayland_client::protocol::wl_region::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &QueueHandle<Self>,
    ) {
    }
}

impl WaylandOverlay {
    pub fn update_config(&mut self, new_config: CrosshairConfig) {
        let size_changed = self.config.window_size != new_config.window_size;
        self.config = new_config;

        if size_changed {
            let win_size = self.config.window_size;
            self.width = win_size;
            self.height = win_size;
            self.layer.set_size(win_size, win_size);
        }

        self.draw();
    }

    pub fn draw(&mut self) {
        let width = self.width;
        let height = self.height;
        if width == 0 || height == 0 {
            return;
        }

        let stride = (width * 4) as i32;

        let (buffer, canvas) = match self
            .pool
            .create_buffer(width as i32, height as i32, stride, wl_shm::Format::Argb8888)
        {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Не удалось создать буфер Wayland SHM: {:?}", e);
                return;
            }
        };

        // Рендерим прицел в tiny-skia с субпиксельным сглаживанием
        let mut pixmap = match Pixmap::new(width, height) {
            Some(p) => p,
            None => return,
        };

        let mut pixmap_mut = pixmap.as_mut();
        render::render_crosshair(&mut pixmap_mut, &self.config);

        // Копируем пиксели из tiny-skia (RGBA premultiplied) в Wayland SHM (ARGB8888 / little-endian BGRA)
        let src = pixmap.data();
        for (i, chunk) in canvas.chunks_exact_mut(4).enumerate() {
            let r = src[i * 4];
            let g = src[i * 4 + 1];
            let b = src[i * 4 + 2];
            let a = src[i * 4 + 3];

            // На little-endian архитектуре x86_64 wl_shm::Format::Argb8888 ожидает байты [B, G, R, A]
            chunk[0] = b;
            chunk[1] = g;
            chunk[2] = r;
            chunk[3] = a;
        }

        let wl_surface = self.layer.wl_surface();
        wl_surface.damage_buffer(0, 0, width as i32, height as i32);
        if let Err(e) = buffer.attach_to(wl_surface) {
            eprintln!("Ошибка buffer.attach_to: {:?}", e);
            return;
        }
        self.layer.commit();
    }
}

pub fn run_wayland_overlay<P: AsRef<Path>>(
    config: CrosshairConfig,
    config_path: P,
) -> Result<(), Box<dyn std::error::Error>> {
    let conn = Connection::connect_to_env()?;
    let (globals, event_queue) = registry_queue_init(&conn)?;
    let qh = event_queue.handle();

    let mut event_loop: EventLoop<WaylandOverlay> =
        EventLoop::try_new().expect("Failed to initialize the calloop event loop!");
    let loop_handle = event_loop.handle();

    // Регистрируем источник событий Wayland в calloop
    WaylandSource::new(conn.clone(), event_queue)
        .insert(loop_handle.clone())
        .map_err(|e| format!("Failed to insert WaylandSource into event loop: {:?}", e))?;

    // Создаем канал для приема обновленного конфига от file watcher'а
    let (tx, rx) = channel::<CrosshairConfig>();

    // Регистрируем канал как источник событий в цикле calloop
    loop_handle
        .insert_source(rx, |_event, _metadata, app: &mut WaylandOverlay| {
            if let ChannelEvent::Msg(new_cfg) = _event {
                println!(
                    "⚡ [Live Reload] Конфигурация обновлена: окно {}x{}, цвет RGBA({}, {}, {}, {}), размер {}px",
                    new_cfg.window_size,
                    new_cfg.window_size,
                    new_cfg.color.r,
                    new_cfg.color.g,
                    new_cfg.color.b,
                    new_cfg.color.a,
                    new_cfg.line_length,
                );
                app.update_config(new_cfg);
            }
        })
        .map_err(|e| format!("Failed to insert channel source: {:?}", e))?;

    // Запускаем watcher файла конфигурации
    let _watcher = crate::watcher::start_config_watcher(config_path, tx)?;
    println!("👀 [Live Reload] Отслеживание изменений файла конфига включено.");

    let compositor = CompositorState::bind(&globals, &qh)?;
    let layer_shell = LayerShell::bind(&globals, &qh)?;
    let shm = Shm::bind(&globals, &qh)?;

    let surface = compositor.create_surface(&qh);

    // ВАЖНО: делаем область ввода пустой (None input region через пустой wl_region),
    // чтобы все клики мыши, движения и прокрутка проходили насквозь в игру под прицелом!
    let empty_region = compositor.wl_compositor().create_region(&qh, ());
    surface.set_input_region(Some(&empty_region));
    empty_region.destroy();

    let win_size = config.window_size;

    // ВАЖНО: Layer::Overlay отображается поверх полноэкранных окон и игр (включая Proton/Wine/Steam)!
    let layer = layer_shell.create_layer_surface(
        &qh,
        surface,
        Layer::Overlay,
        Some("crosshair-overlay"),
        None,
    );

    // Anchor::empty() центрирует окно ровно посередине экрана
    layer.set_anchor(Anchor::empty());
    // Не перехватывает клавиатурный фокус
    layer.set_keyboard_interactivity(KeyboardInteractivity::None);
    layer.set_size(win_size, win_size);
    layer.commit();

    let pool = SlotPool::new((win_size * win_size * 4) as usize, &shm)?;

    let mut app = WaylandOverlay {
        registry_state: RegistryState::new(&globals),
        output_state: OutputState::new(&globals, &qh),
        shm,
        pool,
        layer,
        config,
        width: win_size,
        height: win_size,
        exit: false,
    };

    println!("Wayland Layer-Shell Overlay успешно запущен поверх всех окон (включая игры).");

    while !app.exit {
        event_loop.dispatch(None, &mut app)?;
    }

    Ok(())
}
