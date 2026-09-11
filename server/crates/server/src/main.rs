use std::{net::SocketAddr, path::PathBuf};

use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info,sqlx=warn".into()))
        .init();

    let data_dir = PathBuf::from(std::env::var("TIMECARD_DATA_DIR").unwrap_or_else(|_| "./data".into()));
    let db = timecard_server::open_db(&data_dir).await?;
    let state = timecard_server::build_state(db, data_dir);
    timecard_server::spawn_housekeeping(state.clone());
    let app = timecard_server::build_app(state);

    let bind: SocketAddr = std::env::var("TIMECARD_BIND")
        .unwrap_or_else(|_| "0.0.0.0:8080".into())
        .parse()?;
    tracing::info!("timecard-server listening on http://{bind}");
    let listener = tokio::net::TcpListener::bind(bind).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
