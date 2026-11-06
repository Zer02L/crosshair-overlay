use crate::config::CrosshairConfig;
use crate::sync::{ConfigSource, SharedState};
use axum::{
    extract::State,
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        Html, IntoResponse, Response,
    },
    routing::get,
    Json, Router,
};
use futures_util::stream::Stream;
use std::convert::Infallible;
use std::sync::Arc;
use tower_http::cors::CorsLayer;

const INDEX_HTML: &str = include_str!("web_ui.html");

pub fn start_web_server(
    shared: Arc<SharedState>,
    port: u16,
) {
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
                    .route("/api/events", get(sse_handler))
                    .layer(CorsLayer::permissive())
                    .with_state(shared);

                let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
                let listener = match tokio::net::TcpListener::bind(addr).await {
                    Ok(l) => l,
                    Err(e) => {
                        eprintln!("Не удалось привязать веб-сервер к порту {}: {:?}", port, e);
                        return;
                    }
                };

                let local_ip = crate::sync::get_local_ip().unwrap_or_else(|| "127.0.0.1".into());
                println!(
                    "🌐 [Web UI] Панель управления доступна: http://localhost:{} (для мобильных: http://{}:{})",
                    port, local_ip, port
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
    State(shared): State<Arc<SharedState>>,
) -> Json<CrosshairConfig> {
    Json(shared.get_config())
}

async fn update_config(
    State(shared): State<Arc<SharedState>>,
    Json(new_config): Json<CrosshairConfig>,
) -> Response {
    shared.update_config(new_config, ConfigSource::Web);
    StatusCode::OK.into_response()
}

async fn sse_handler(
    State(shared): State<Arc<SharedState>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = shared.subscribe();
    let stream = futures_util::stream::unfold(rx, |mut rx| async move {
        match rx.recv().await {
            Ok(cfg) => {
                let json = serde_json::to_string(&cfg).unwrap_or_default();
                Some((Ok(Event::default().data(json)), rx))
            }
            Err(_) => None,
        }
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}
