mod db;
mod state;
mod routes;

use anyhow::Result;
use axum::Router;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("imaging_server=debug,tower_http=debug")
        .init();

    let db_path = std::path::PathBuf::from("imaging-server.db");
    let database = db::Database::open(&db_path)?;

    let app_state = state::AppState::new(database);

    let needs_seed = {
        let db = app_state.db.lock().await;
        db.list_images().map(|imgs| imgs.is_empty()).unwrap_or(true)
    };
    if needs_seed {
        let state_clone = app_state.clone();
        tokio::task::spawn_blocking(move || state_clone.seed_sample_images()).await?;
        info!("Seeded sample images into database");
    }

    let app = Router::new()
        .merge(routes::api_routes())
        .with_state(app_state);

    let addr = "0.0.0.0:7443";
    info!("Imaging server listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
