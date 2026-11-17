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

    let app_state = state::AppState::new_with_samples();

    let app = Router::new()
        .merge(routes::api_routes())
        .with_state(app_state);

    let addr = "0.0.0.0:7443";
    info!("Imaging server listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
