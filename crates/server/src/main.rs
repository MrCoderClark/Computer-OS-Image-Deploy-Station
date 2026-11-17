use anyhow::Result;
use axum::{routing::get, Router};
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("imaging_server=debug,tower_http=debug")
        .init();

    let app = Router::new()
        .route("/health", get(health))
        .route("/api/images", get(list_images));

    let addr = "0.0.0.0:7443";
    info!("Imaging server listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health() -> &'static str {
    "ok"
}

async fn list_images() -> axum::Json<Vec<imaging_common::types::ImageRecord>> {
    axum::Json(vec![])
}
