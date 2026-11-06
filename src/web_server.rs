use crate::config::CrosshairConfig;
use crate::watcher::ConfigUpdateSender;
use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::get,
    Json, Router,
};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tower_http::cors::CorsLayer;

const INDEX_HTML: &str = include_str!("web_ui.html");

#[derive(Clone)]
struct AppState {
    config: Arc<Mutex<CrosshairConfig>>,
    config_path: PathBuf,
    sender: Arc<dyn ConfigUpdateSender>,
}

pub fn start_web_server<S: ConfigUpdateSender>(
    config: CrosshairConfig,
    config_path: PathBuf,
    sender: S,
    port: u16,
) {
    let state = AppState {
        config: Arc::new(Mutex::new(config)),
        config_path,
        sender: Arc::new(sender),
    };

    std::thread::Builder::new()
        .name("web-server".into())
        .spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Не удалось создать Tokio runtime для веб-сервера: {:?}", e);
                    return;
                }
            };

            rt.block_on(async move {
                let app = Router::new()
                    .route("/", get(serve_index))
                    .route("/api/config", get(get_config).post(update_config))
                    .layer(CorsLayer::permissive())
                    .with_state(state);

                let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
                let listener = match tokio::net::TcpListener::bind(addr).await {
                    Ok(l) => l,
                    Err(e) => {
                        eprintln!("Не удалось привязать веб-сервер к порту {}: {:?}", port, e);
                        return;
                    }
                };

                println!(
                    "🌐 [Web UI] Панель управления доступна: http://localhost:{} (для мобильных: http://<IP-ПК>:{})",
                    port, port
                );

                if let Err(e) = axum::serve(listener, app).await {
                    eprintln!("Ошибка веб-сервера: {:?}", e);
                }
            });
        })
        .expect("Failed to spawn web server thread");
}

async fn serve_index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

async fn get_config(
    State(state): State<AppState>,
) -> Json<CrosshairConfig> {
    let cfg = state.config.lock().unwrap().clone();
    Json(cfg)
}

async fn update_config(
    State(state): State<AppState>,
    Json(new_config): Json<CrosshairConfig>,
) -> Response {
    // 1. Обновляем конфиг в памяти
    {
        let mut cur = state.config.lock().unwrap();
        *cur = new_config.clone();
    }

    // 2. Уведомляем оверлей для мгновенной перерисовки
    state.sender.send_config(new_config.clone());

    // 3. Сохраняем в файл crosshair_config.json
    if let Ok(json) = serde_json::to_string_pretty(&new_config) {
        let _ = std::fs::write(&state.config_path, json);
    }

    StatusCode::OK.into_response()
}
