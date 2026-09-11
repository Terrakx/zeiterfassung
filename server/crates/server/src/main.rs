mod absences;
mod auth;
mod calc;
mod db;
mod employees;
mod error;
mod export;
mod holidays;
mod punches;
mod reports;
mod settings;
mod statics;
mod time;

use std::{net::SocketAddr, path::PathBuf, sync::Arc};

use axum::{routing::get, Router};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use tower_http::{compression::CompressionLayer, trace::TraceLayer};
use tracing_subscriber::EnvFilter;

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub data_dir: Arc<PathBuf>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info,sqlx=warn".into()))
        .init();

    let data_dir = PathBuf::from(std::env::var("TIMECARD_DATA_DIR").unwrap_or_else(|_| "./data".into()));
    std::fs::create_dir_all(data_dir.join("exports"))?;
    let db_path = data_dir.join("timecard.sqlite");

    let opts = SqliteConnectOptions::new()
        .filename(&db_path)
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .foreign_keys(true)
        .busy_timeout(std::time::Duration::from_secs(5));
    let db = SqlitePoolOptions::new().max_connections(8).connect_with(opts).await?;
    sqlx::migrate!("../../migrations").run(&db).await?;

    auth::seed_admin(&db).await?;
    settings::ensure_defaults(&db).await?;

    let state = AppState { db, data_dir: Arc::new(data_dir) };

    let api = Router::new()
        .merge(auth::router())
        .merge(settings::router())
        .merge(employees::router())
        .merge(punches::router())
        .merge(absences::router())
        .merge(holidays::router())
        .merge(calc::router())
        .merge(reports::router())
        .merge(export::router())
        .route("/health", get(|| async { "ok" }));

    let app = Router::new()
        .nest("/api", api)
        .fallback(statics::serve)
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let bind: SocketAddr = std::env::var("TIMECARD_BIND")
        .unwrap_or_else(|_| "0.0.0.0:8080".into())
        .parse()?;
    tracing::info!("timecard-server listening on http://{bind}");
    let listener = tokio::net::TcpListener::bind(bind).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
