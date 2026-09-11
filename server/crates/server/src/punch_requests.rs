//! Korrekturanträge zu Stempelungen: Mitarbeiter beantragen, Verwaltung entscheidet.

use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use chrono::NaiveDateTime;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::{FromRow, SqlitePool};
use timecard_domain::PunchKind;

use crate::{
    auth::{AdminUser, CurrentUser},
    calc,
    db::{self},
    error::{bad, ApiResult, AppError},
    time, AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/punch-requests", get(list_own).post(create_own))
        .route("/punch-requests/{id}/withdraw", post(withdraw_own))
        .route("/punch-requests/{id}/decide", post(decide))
        .route("/admin/punch-requests", get(list_admin))
}

#[derive(Debug, Clone, FromRow, serde::Serialize)]
pub struct PunchRequestRow {
    pub id: i64,
    pub employee_id: i64,
    pub typ: String,
    pub punch_id: Option<i64>,
    pub datum: String,
    pub zeit: Option<String>,
    pub art: Option<String>,
    pub begruendung: String,
    pub status: String,
    pub beantragt_at: String,
    pub entschieden_von: Option<i64>,
    pub entschieden_at: Option<String>,
    pub entscheidung_kommentar: Option<String>,
}

pub async fn open_count(db: &SqlitePool, employee_id: i64, from: &str, to: &str) -> ApiResult<i64> {
    Ok(sqlx::query_scalar("SELECT COUNT(*) FROM punch_requests WHERE employee_id = ? AND status = 'beantragt' AND datum BETWEEN ? AND ?")
        .bind(employee_id).bind(from).bind(to).fetch_one(db).await?)
}

#[derive(Deserialize)]
struct CreateReq {
    typ: String,
    punch_id: Option<i64>,
    datum: String,
    zeit: Option<String>,
    art: Option<String>,
    begruendung: String,
}

async fn create_own(State(state): State<AppState>, CurrentUser(user): CurrentUser, Json(req): Json<CreateReq>) -> ApiResult<Json<Value>> {
    let date = time::parse_date(&req.datum).ok_or_else(|| bad("Datum ungültig"))?;
    if date > time::today_local() {
        return Err(bad("Korrekturen nur für vergangene Tage oder heute"));
    }
    if req.begruendung.trim().is_empty() {
        return Err(bad("Begründung ist Pflicht"));
    }
    calc::ensure_month_open(&state.db, user.id, date).await?;
    match req.typ.as_str() {
        "einfuegen" => {
            let z = req.zeit.as_deref().ok_or_else(|| bad("Uhrzeit fehlt"))?;
            NaiveDateTime::parse_from_str(&format!("{}T{}", req.datum, z), "%Y-%m-%dT%H:%M").map_err(|_| bad("Uhrzeit im Format HH:MM"))?;
            PunchKind::parse(req.art.as_deref().unwrap_or("")).ok_or_else(|| bad("Stempelart ungültig"))?;
        }
        "stornieren" => {
            let pid = req.punch_id.ok_or_else(|| bad("Stempelung fehlt"))?;
            let owner: Option<i64> = sqlx::query_scalar("SELECT employee_id FROM punches WHERE id = ? AND storniert_at IS NULL").bind(pid).fetch_optional(&state.db).await?;
            if owner != Some(user.id) {
                return Err(AppError::NotFound);
            }
            let pending: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM punch_requests WHERE punch_id = ? AND status = 'beantragt'").bind(pid).fetch_one(&state.db).await?;
            if pending > 0 {
                return Err(AppError::Conflict("Für diese Stempelung liegt bereits ein Antrag vor".into()));
            }
        }
        _ => return Err(bad("Typ ungültig")),
    }
    let r = sqlx::query("INSERT INTO punch_requests (employee_id, typ, punch_id, datum, zeit, art, begruendung) VALUES (?,?,?,?,?,?,?)")
        .bind(user.id).bind(&req.typ).bind(req.punch_id).bind(&req.datum).bind(&req.zeit).bind(&req.art).bind(req.begruendung.trim())
        .execute(&state.db).await?;
    db::audit(&state.db, Some(user.id), "korrekturantrag_gestellt", Some(format!("punch_request:{}", r.last_insert_rowid())), None, Some(json!({"typ": req.typ, "datum": req.datum, "zeit": req.zeit, "art": req.art}))).await?;
    Ok(Json(json!({"id": r.last_insert_rowid()})))
}

#[derive(Deserialize)]
struct RangeQ {
    von: Option<String>,
    bis: Option<String>,
}

fn row_json(r: &PunchRequestRow, name: Option<&str>) -> Value {
    let mut v = serde_json::to_value(r).unwrap_or(Value::Null);
    if let Some(n) = name {
        v["name"] = json!(n);
    }
    v
}

async fn list_own(State(state): State<AppState>, CurrentUser(user): CurrentUser, Query(q): Query<RangeQ>) -> ApiResult<Json<Vec<Value>>> {
    let rows = sqlx::query_as::<_, PunchRequestRow>(
        "SELECT * FROM punch_requests WHERE employee_id = ? AND datum >= ? AND datum <= ? ORDER BY datum DESC, id DESC LIMIT 200",
    )
    .bind(user.id).bind(q.von.unwrap_or_else(|| "0000-00-00".into())).bind(q.bis.unwrap_or_else(|| "9999-12-31".into()))
    .fetch_all(&state.db).await?;
    Ok(Json(rows.iter().map(|r| row_json(r, None)).collect()))
}

async fn withdraw_own(State(state): State<AppState>, CurrentUser(user): CurrentUser, Path(id): Path<i64>) -> ApiResult<Json<Value>> {
    let r = sqlx::query("UPDATE punch_requests SET status = 'storniert' WHERE id = ? AND employee_id = ? AND status = 'beantragt'")
        .bind(id).bind(user.id).execute(&state.db).await?;
    if r.rows_affected() == 0 {
        return Err(AppError::Conflict("Nur offene Anträge können zurückgezogen werden".into()));
    }
    Ok(Json(json!({"ok": true})))
}

#[derive(Deserialize)]
struct StatusQ {
    status: Option<String>,
}

async fn list_admin(State(state): State<AppState>, AdminUser(_): AdminUser, Query(q): Query<StatusQ>) -> ApiResult<Json<Vec<Value>>> {
    let status = q.status.unwrap_or_else(|| "beantragt".into());
    let rows: Vec<(i64, String, String)> = sqlx::query_as(
        "SELECT p.id, e.vorname || ' ' || e.nachname, e.personalnr FROM punch_requests p JOIN employees e ON e.id = p.employee_id
         WHERE (? = 'alle' OR p.status = ?) ORDER BY p.beantragt_at DESC LIMIT 300",
    )
    .bind(&status).bind(&status).fetch_all(&state.db).await?;
    let mut out = Vec::new();
    for (id, name, pnr) in rows {
        let r = sqlx::query_as::<_, PunchRequestRow>("SELECT * FROM punch_requests WHERE id = ?").bind(id).fetch_one(&state.db).await?;
        let mut v = row_json(&r, Some(&name));
        v["personalnr"] = json!(pnr);
        if let Some(pid) = r.punch_id {
            let p: Option<(String, String)> = sqlx::query_as("SELECT ts_utc, art FROM punches WHERE id = ?").bind(pid).fetch_optional(&state.db).await?;
            if let Some((ts, art)) = p {
                v["punch"] = json!({"zeit": time::parse_utc(&ts).map(|t| time::to_local(t).format("%d.%m.%Y %H:%M").to_string()), "art": art});
            }
        }
        out.push(v);
    }
    Ok(Json(out))
}

#[derive(Deserialize)]
struct DecideReq {
    status: String,
    kommentar: Option<String>,
}

async fn decide(State(state): State<AppState>, AdminUser(admin): AdminUser, Path(id): Path<i64>, Json(req): Json<DecideReq>) -> ApiResult<Json<Value>> {
    if req.status != "genehmigt" && req.status != "abgelehnt" {
        return Err(bad("Status muss genehmigt oder abgelehnt sein"));
    }
    let r = sqlx::query_as::<_, PunchRequestRow>("SELECT * FROM punch_requests WHERE id = ?").bind(id).fetch_optional(&state.db).await?.ok_or(AppError::NotFound)?;
    if r.status != "beantragt" {
        return Err(AppError::Conflict("Antrag ist nicht mehr offen".into()));
    }
    let date = time::parse_date(&r.datum).ok_or_else(|| bad("Datum ungültig"))?;
    if req.status == "genehmigt" {
        calc::ensure_month_open(&state.db, r.employee_id, date).await?;
        let grund = format!("Korrekturantrag: {}", r.begruendung);
        match r.typ.as_str() {
            "einfuegen" => {
                let local = NaiveDateTime::parse_from_str(&format!("{}T{}", r.datum, r.zeit.clone().unwrap_or_default()), "%Y-%m-%dT%H:%M").map_err(|_| bad("Zeit ungültig"))?;
                sqlx::query("INSERT INTO punches (employee_id, ts_utc, art, quelle, erfasst_von, kommentar) VALUES (?,?,?,'admin',?,?)")
                    .bind(r.employee_id).bind(time::fmt_utc(time::local_to_utc(local))).bind(&r.art).bind(admin.id).bind(&grund)
                    .execute(&state.db).await?;
            }
            _ => {
                let pid = r.punch_id.ok_or_else(|| bad("Stempelung fehlt"))?;
                let u = sqlx::query("UPDATE punches SET storniert_at = ?, storniert_von = ?, storno_grund = ? WHERE id = ? AND storniert_at IS NULL")
                    .bind(time::fmt_utc(time::now_utc())).bind(admin.id).bind(&grund).bind(pid).execute(&state.db).await?;
                if u.rows_affected() == 0 {
                    return Err(AppError::Conflict("Stempelung existiert nicht mehr oder ist bereits storniert".into()));
                }
            }
        }
    }
    sqlx::query("UPDATE punch_requests SET status = ?, entschieden_von = ?, entschieden_at = ?, entscheidung_kommentar = ? WHERE id = ?")
        .bind(&req.status).bind(admin.id).bind(time::fmt_utc(time::now_utc())).bind(&req.kommentar).bind(id)
        .execute(&state.db).await?;
    db::audit(&state.db, Some(admin.id), "korrekturantrag_entschieden", Some(format!("punch_request:{id}")), Some(row_json(&r, None)), Some(json!({"status": req.status, "kommentar": req.kommentar}))).await?;
    Ok(Json(json!({"ok": true})))
}
