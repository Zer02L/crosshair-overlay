use crate::config::CrosshairConfig;
use crate::watcher::ConfigUpdateSender;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigSource {
    Gui,
    Web,
    FileWatcher,
}

pub struct SharedState {
    config: Mutex<CrosshairConfig>,
    version: AtomicU64,
    config_path: PathBuf,
    overlay_sender: Arc<dyn ConfigUpdateSender>,
    gui_ctx: Mutex<Option<eframe::egui::Context>>,
}

impl SharedState {
    pub fn new<S: ConfigUpdateSender>(
        config: CrosshairConfig,
        config_path: PathBuf,
        overlay_sender: S,
    ) -> Arc<Self> {
        Arc::new(Self {
            config: Mutex::new(config),
            version: AtomicU64::new(1),
            config_path,
            overlay_sender: Arc::new(overlay_sender),
            gui_ctx: Mutex::new(None),
        })
    }

    pub fn set_gui_context(&self, ctx: eframe::egui::Context) {
        if let Ok(mut lock) = self.gui_ctx.lock() {
            *lock = Some(ctx);
        }
    }

    pub fn get_config(&self) -> CrosshairConfig {
        self.config.lock().unwrap().clone()
    }

    pub fn get_version(&self) -> u64 {
        self.version.load(Ordering::SeqCst)
    }

    pub fn update_config(&self, new_cfg: CrosshairConfig, source: ConfigSource) {
        let mut changed = false;
        {
            let mut cur = self.config.lock().unwrap();
            if *cur != new_cfg {
                *cur = new_cfg.clone();
                self.version.fetch_add(1, Ordering::SeqCst);
                changed = true;
            }
        }

        if !changed {
            return;
        }

        // 1. Мгновенно уведомляем Wayland оверлей для перерисовки
        self.overlay_sender.send_config(new_cfg.clone());

        // 2. Если изменение пришло не из диска, атомарно сохраняем в файл crosshair_config.json
        if source != ConfigSource::FileWatcher {
            if let Ok(json) = serde_json::to_string_pretty(&new_cfg) {
                let _ = std::fs::write(&self.config_path, json);
            }
        }

        // 3. Если изменение пришло не из Desktop GUI (например, из смартфона или файла),
        // запрашиваем перерисовку в egui, чтобы ползунки на экране мгновенно сдвинулись
        if source != ConfigSource::Gui {
            if let Ok(guard) = self.gui_ctx.lock() {
                if let Some(ctx) = guard.as_ref() {
                    ctx.request_repaint();
                }
            }
        }
    }
}

impl ConfigUpdateSender for Arc<SharedState> {
    fn send_config(&self, config: CrosshairConfig) {
        self.update_config(config, ConfigSource::FileWatcher);
    }
}

pub fn get_local_ip() -> Option<String> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    let addr = socket.local_addr().ok()?;
    Some(addr.ip().to_string())
}
