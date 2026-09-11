//! Systemfunktionen für Administratoren: Audit-Log, Backup.

use axum::{
    body::Body,
    extract::{Query, State},
    http::{header, StatusCode},
    response::Response,
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{auth::AdminUser, error::ApiResult, time, AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/admin/audit", get(audit))
        .route("/admin/backup", get(backup))
        .route("/admin/open-count", get(open_count))
}

/// Offene Vorgänge für die Verwaltung (Badge in der Tab-Leiste).
async fn open_count(State(state): State<AppState>, AdminUser(_): AdminUser) -> ApiResult<Json<Value>> {
    let (abw, korr): (i64, i64) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM absences a JOIN employees e ON e.id = a.employee_id WHERE a.status = 'beantragt' AND e.aktiv = 1),
                (SELECT COUNT(*) FROM punch_requests p JOIN employees e ON e.id = p.employee_id WHERE p.status = 'beantragt' AND e.aktiv = 1)",
    )
    .fetch_one(&state.db)
    .await?;
    Ok(Json(json!({"abwesenheiten": abw, "korrekturen": korr, "gesamt": abw + korr})))
}

#[derive(Deserialize)]
struct AuditQuery {
    limit: Option<i64>,
    employee_id: Option<i64>,
}

async fn audit(State(state): State<AppState>, AdminUser(_): AdminUser, Query(q): Query<AuditQuery>) -> ApiResult<Json<Vec<Value>>> {
    let limit = q.limit.unwrap_or(200).clamp(1, 2000);
    let ziel = q.employee_id.map(|id| format!("employee:{id}"));
    let rows: Vec<(i64, String, Option<i64>, Option<String>, Option<String>, String, Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT a.id, a.ts, a.actor_id, e.vorname, e.nachname, a.aktion, a.ziel, a.vorher, a.nachher
         FROM audit_log a LEFT JOIN employees e ON e.id = a.actor_id
         WHERE (? IS NULL OR a.ziel = ?)
         ORDER BY a.id DESC LIMIT ?",
    )
    .bind(&ziel).bind(&ziel).bind(limit)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows.into_iter().map(|(id, ts, actor, vn, nn, aktion, ziel, vorher, nachher)| json!({
        "id": id, "ts": ts, "actor_id": actor,
        "actor": match (vn, nn) { (Some(v), Some(n)) => format!("{v} {n}"), _ => "System".into() },
        "aktion": aktion, "ziel": ziel,
        "vorher": vorher.and_then(|v| serde_json::from_str::<Value>(&v).ok()),
        "nachher": nachher.and_then(|v| serde_json::from_str::<Value>(&v).ok()),
    })).collect()))
}

/// Konsistente Kopie der Datenbank (VACUUM INTO) zum Herunterladen.
async fn backup(State(state): State<AppState>, AdminUser(admin): AdminUser) -> ApiResult<Response> {
    let dir = state.data_dir.join("backups");
    std::fs::create_dir_all(&dir).map_err(|e| anyhow::anyhow!(e))?;
    let name = format!("timecard-{}.sqlite", time::to_local(time::now_utc()).format("%Y%m%d-%H%M%S"));
    let path = dir.join(&name);
    let path_str = path.to_string_lossy().replace('\\', "/").replace('\'', "''");
    sqlx::query(&format!("VACUUM INTO '{path_str}'")).execute(&state.db).await?;
    let bytes = std::fs::read(&path).map_err(|e| anyhow::anyhow!(e))?;
    crate::db::audit(&state.db, Some(admin.id), "backup_erstellt", None, None, Some(json!({"datei": name}))).await?;
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/vnd.sqlite3")
        .header(header::CONTENT_DISPOSITION, format!("attachment; filename=\"{name}\""))
        .body(Body::from(bytes))
        .unwrap())
}
