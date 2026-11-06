use crate::config::CrosshairConfig;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub trait ConfigUpdateSender: Send + 'static {
    fn send_config(&self, config: CrosshairConfig);
}

impl ConfigUpdateSender for smithay_client_toolkit::reexports::calloop::channel::Sender<CrosshairConfig> {
    fn send_config(&self, config: CrosshairConfig) {
        let _ = self.send(config);
    }
}

impl ConfigUpdateSender for std::sync::mpsc::Sender<CrosshairConfig> {
    fn send_config(&self, config: CrosshairConfig) {
        let _ = self.send(config);
    }
}

pub fn start_config_watcher<P: AsRef<Path>, S: ConfigUpdateSender>(
    config_path: P,
    sender: S,
) -> notify::Result<RecommendedWatcher> {
    let path_buf: PathBuf = config_path.as_ref().to_path_buf();
    let abs_path = std::fs::canonicalize(&path_buf).unwrap_or(path_buf.clone());
    
    // Отслеживаем родительскую директорию или сам файл,
    // так как многие редакторы (vim, vscode) сохраняют файл через временный файл + rename/atomic replace
    let watch_target = abs_path.parent().unwrap_or(Path::new(".")).to_path_buf();
    let target_filename = abs_path.file_name().map(|n| n.to_os_string());

    let mut last_event = Instant::now() - Duration::from_secs(10);

    let mut watcher = RecommendedWatcher::new(
        move |res: notify::Result<Event>| {
            if let Ok(event) = res {
                let is_target = match &target_filename {
                    Some(name) => event.paths.iter().any(|p| p.file_name() == Some(name.as_os_str())),
                    None => true,
                };

                if is_target {
                    match event.kind {
                        EventKind::Modify(_) | EventKind::Create(_) => {
                            // Debounce 100ms, чтобы избежать повторных чтений при сохранении
                            let now = Instant::now();
                            if now.duration_since(last_event) > Duration::from_millis(100) {
                                last_event = now;
                                // Даем микро-паузу для завершения записи файла на диск
                                std::thread::sleep(Duration::from_millis(20));
                                if let Ok(new_cfg) = CrosshairConfig::load_from_file(&abs_path) {
                                    sender.send_config(new_cfg);
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        },
        notify::Config::default(),
    )?;

    watcher.watch(&watch_target, RecursiveMode::NonRecursive)?;
    Ok(watcher)
}
