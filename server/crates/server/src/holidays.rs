//! Feiertage: gesetzliche (berechnet) plus betriebliche (Admin).

use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::SqlitePool;
use timecard_domain::{austrian_holidays, Holiday};

use crate::{auth::{AdminUser, CurrentUser}, db, error::{bad, ApiResult}, time, AppState};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/holidays", get(list).post(add))
        .route("/holidays/{datum}", axum::routing::delete(remove))
}

pub async fn for_year(db: &SqlitePool, year: i32) -> ApiResult<Vec<Holiday>> {
    let mut out = austrian_holidays(year);
    let rows: Vec<(String, String)> = sqlx::query_as("SELECT datum, name FROM custom_holidays WHERE datum LIKE ?")
        .bind(format!("{year}-%"))
        .fetch_all(db)
        .await?;
    for (d, name) in rows {
        if let Some(date) = time::parse_date(&d) {
            out.retain(|h| h.date != date);
            out.push(Holiday { date, name });
        }
    }
    out.sort_by_key(|h| h.date);
    Ok(out)
}

#[derive(Deserialize)]
struct YearQuery {
    jahr: Option<i32>,
}

async fn list(State(state): State<AppState>, CurrentUser(_): CurrentUser, Query(q): Query<YearQuery>) -> ApiResult<Json<Vec<Value>>> {
    let year = q.jahr.unwrap_or_else(|| chrono::Datelike::year(&time::today_local()));
    let custom: Vec<String> = sqlx::query_scalar("SELECT datum FROM custom_holidays").fetch_all(&state.db).await?;
    Ok(Json(
        for_year(&state.db, year)
            .await?
            .into_iter()
            .map(|h| {
                let d = time::fmt_date(h.date);
                json!({"datum": d, "name": h.name, "betrieblich": custom.contains(&d)})
            })
            .collect(),
    ))
}

#[derive(Deserialize)]
struct HolidayReq {
    datum: String,
    name: String,
}

async fn add(State(state): State<AppState>, AdminUser(admin): AdminUser, Json(req): Json<HolidayReq>) -> ApiResult<Json<Value>> {
    time::parse_date(&req.datum).ok_or_else(|| bad("Datum ungültig"))?;
    if req.name.trim().is_empty() {
        return Err(bad("Name fehlt"));
    }
    sqlx::query("INSERT INTO custom_holidays (datum, name) VALUES (?, ?) ON CONFLICT(datum) DO UPDATE SET name = excluded.name")
        .bind(&req.datum).bind(req.name.trim()).execute(&state.db).await?;
    db::audit(&state.db, Some(admin.id), "feiertag_angelegt", Some(format!("holiday:{}", req.datum)), None, Some(json!({"name": req.name}))).await?;
    Ok(Json(json!({"ok": true})))
}

async fn remove(State(state): State<AppState>, AdminUser(admin): AdminUser, Path(datum): Path<String>) -> ApiResult<Json<Value>> {
    sqlx::query("DELETE FROM custom_holidays WHERE datum = ?").bind(&datum).execute(&state.db).await?;
    db::audit(&state.db, Some(admin.id), "feiertag_geloescht", Some(format!("holiday:{datum}")), None, None).await?;
    Ok(Json(json!({"ok": true})))
}
