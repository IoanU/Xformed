use axum::{
    extract::{DefaultBodyLimit, State},
    http::StatusCode,
    response::Html,
    routing::{get, post},
    Json, Router,
};
use converters::{handle_convert, ConvertRequest, ConvertResponse};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

fn app() -> Router {
    Router::new()
        .route(
            "/",
            get(|| async { Html(include_str!("../static/index.html")) }),
        )
        .route(
            "/app.js",
            get(|| async {
                (
                    [(
                        axum::http::header::CONTENT_TYPE,
                        "text/javascript; charset=utf-8",
                    )],
                    include_str!("../static/app.js"),
                )
            }),
        )
        .route(
            "/style.css",
            get(|| async {
                (
                    [(axum::http::header::CONTENT_TYPE, "text/css; charset=utf-8")],
                    include_str!("../static/style.css"),
                )
            }),
        )
        .route(
            "/effects.js",
            get(|| async {
                (
                    [(
                        axum::http::header::CONTENT_TYPE,
                        "text/javascript; charset=utf-8",
                    )],
                    include_str!("../static/effects.js"),
                )
            }),
        )
        .route(
            "/collection.js",
            get(|| async {
                (
                    [(
                        axum::http::header::CONTENT_TYPE,
                        "text/javascript; charset=utf-8",
                    )],
                    include_str!("../static/collection.js"),
                )
            }),
        )
        .route(
            "/download.js",
            get(|| async {
                (
                    [(
                        axum::http::header::CONTENT_TYPE,
                        "text/javascript; charset=utf-8",
                    )],
                    include_str!("../static/download.js"),
                )
            }),
        )
        .route(
            "/vendor/anime.umd.min.js",
            get(|| async {
                (
                    [(
                        axum::http::header::CONTENT_TYPE,
                        "text/javascript; charset=utf-8",
                    )],
                    include_str!("../static/vendor/anime.umd.min.js"),
                )
            }),
        )
        .route(
            "/runtime.js",
            get(|| async {
                (
                    [(
                        axum::http::header::CONTENT_TYPE,
                        "text/javascript; charset=utf-8",
                    )],
                    include_str!("../static/runtime.js"),
                )
            }),
        )
        .route(
            "/examples.js",
            get(|| async {
                (
                    [(axum::http::header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
                    include_str!("../static/examples.js"),
                )
            }),
        )
        .route("/health", get(|| async { "ok" }))
        .route("/convert", post(convert))
        .layer(DefaultBodyLimit::max(17_000_000))
        .with_state(Arc::new(Semaphore::new(2)))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new("info"))
        .with(tracing_subscriber::fmt::layer())
        .init();
    let address = std::env::var("XFORMED_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".into());
    let listener = tokio::net::TcpListener::bind(&address).await?;
    tracing::info!("Xformed Studio: http://{address}");
    axum::serve(listener, app()).await?;
    Ok(())
}

type ApiError = (StatusCode, Json<serde_json::Value>);
async fn convert(
    State(slots): State<Arc<Semaphore>>,
    Json(req): Json<ConvertRequest>,
) -> Result<Json<ConvertResponse>, ApiError> {
    let permit = slots.try_acquire_owned().map_err(|_| {
        (
            StatusCode::TOO_MANY_REQUESTS,
            Json(serde_json::json!({"error":"Studio is busy. Try again shortly."})),
        )
    })?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        handle_convert(req)
    })
    .await
    .map_err(|err| {
        tracing::error!(%err, "conversion worker failed");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error":"Conversion worker failed"})),
        )
    })?
    .map(Json)
    .map_err(|err| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error":err.to_string()})),
        )
    })
}
