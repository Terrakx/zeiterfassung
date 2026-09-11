//! Zeiterfassung: HTTP-API, Datenbank und Berichte. `main.rs` startet den Server, die Tests bauen
//! den Router über [`build_app`] direkt.

pub mod absences;
pub mod admin;
pub mod auth;
pub mod calc;
pub mod db;
pub mod employees;
pub mod error;
pub mod export;
pub mod holidays;
pub mod punch_requests;
pub mod punches;
pub mod ratelimit;
pub mod reports;
pub mod settings;
pub mod statics;
pub mod time;

use std::{path::PathBuf, sync::Arc};

use axum::{
    http::{header, HeaderValue},
    routing::get,
    Router,
};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use tower_http::{compression::CompressionLayer, set_header::SetResponseHeaderLayer, trace::TraceLayer};

#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
    pub data_dir: Arc<PathBuf>,
    pub limiter: ratelimit::Limiter,
}

/// Öffnet (oder erzeugt) die Datenbank, führt Migrationen aus und legt den Admin an.
pub async fn open_db(data_dir: &PathBuf) -> anyhow::Result<SqlitePool> {
    std::fs::create_dir_all(data_dir.join("exports"))?;
    let opts = SqliteConnectOptions::new()
        .filename(data_dir.join("timecard.sqlite"))
        .create_if_missing(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .foreign_keys(true)
        .busy_timeout(std::time::Duration::from_secs(5));
    let db = SqlitePoolOptions::new().max_connections(8).connect_with(opts).await?;
    init_db(&db).await?;
    Ok(db)
}

/// In-Memory-Datenbank für Tests.
pub async fn open_memory_db() -> anyhow::Result<SqlitePool> {
    let opts = SqliteConnectOptions::new().filename(":memory:").foreign_keys(true);
    let db = SqlitePoolOptions::new().max_connections(1).connect_with(opts).await?;
    init_db(&db).await?;
    Ok(db)
}

async fn init_db(db: &SqlitePool) -> anyhow::Result<()> {
    sqlx::migrate!("../../migrations").run(db).await?;
    auth::seed_admin(db).await?;
    settings::ensure_defaults(db).await?;
    Ok(())
}

pub fn build_state(db: SqlitePool, data_dir: PathBuf) -> AppState {
    AppState { db, data_dir: Arc::new(data_dir), limiter: ratelimit::Limiter::default() }
}

pub fn build_app(state: AppState) -> Router {
    let api = Router::new()
        .merge(auth::router())
        .merge(settings::router())
        .merge(employees::router())
        .merge(punches::router())
        .merge(punch_requests::router())
        .merge(absences::router())
        .merge(holidays::router())
        .merge(calc::router())
        .merge(reports::router())
        .merge(export::router())
        .merge(admin::router())
        .route("/health", get(|| async { "ok" }))
        .route("/time", get(|| async { axum::Json(serde_json::json!({"utc": time::fmt_utc(time::now_utc())})) }));

    // Inline-Styles kommen aus den Svelte-Komponenten, Schriften und Skripte sind gebündelt. Das
    // Startskript von SvelteKit steht inline in der index.html und wird über seinen Hash erlaubt.
    let hashes: String = statics::inline_script_hashes().iter().map(|h| format!(" 'sha256-{h}'")).collect();
    let csp = format!(
        "default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; font-src 'self'; \
         script-src 'self'{hashes}; connect-src 'self'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'"
    );
    let csp = HeaderValue::from_str(&csp).expect("csp header");

    Router::new()
        .nest("/api", api)
        .fallback(statics::serve)
        .layer(SetResponseHeaderLayer::if_not_present(header::CONTENT_SECURITY_POLICY, csp))
        .layer(SetResponseHeaderLayer::if_not_present(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff")))
        .layer(SetResponseHeaderLayer::if_not_present(header::X_FRAME_OPTIONS, HeaderValue::from_static("DENY")))
        .layer(SetResponseHeaderLayer::if_not_present(header::REFERRER_POLICY, HeaderValue::from_static("same-origin")))
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Räumt abgelaufene Sessions und alte Temp-Verzeichnisse periodisch auf.
pub fn spawn_housekeeping(state: AppState) {
    tokio::spawn(async move {
        loop {
            let now = time::fmt_utc(time::now_utc());
            if let Err(e) = sqlx::query("DELETE FROM sessions WHERE expires_at < ?").bind(&now).execute(&state.db).await {
                tracing::warn!("Session-Bereinigung fehlgeschlagen: {e}");
            }
            // Temporäre LaTeX-Verzeichnisse, die älter als eine Stunde sind (laufende Kompilate bleiben)
            if let Ok(entries) = std::fs::read_dir(state.data_dir.join("tmp")) {
                for e in entries.flatten() {
                    let old = e.metadata().and_then(|m| m.modified()).map(|t| t.elapsed().map(|d| d.as_secs() > 3600).unwrap_or(false)).unwrap_or(false);
                    if old && e.path().is_dir() {
                        let _ = std::fs::remove_dir_all(e.path());
                    }
                }
            }
            state.limiter.prune();
            tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
        }
    });
}
