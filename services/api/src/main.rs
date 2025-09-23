
use axum::{routing::{get, post}, Json, Router};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use converters::{ConvertRequest, ConvertResponse, handle_convert};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new("info"))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/convert", post(convert));

    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 8080)).await.unwrap();
    tracing::info!("listening on http://127.0.0.1:8080");
    axum::serve(listener, app).await.unwrap();
}

async fn convert(Json(req): Json<ConvertRequest>) -> Json<ConvertResponse> {
    let resp = handle_convert(req).expect("convert failed");
    Json(resp)
}
