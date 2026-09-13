//! Stempelungen: Terminal, Portal, Admin-Korrekturen.

use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{FromRow, SqlitePool};
use timecard_domain::{Punch, PunchKind};

use crate::{
    auth::{self, AdminUser, CurrentUser},
    calc,
    db::{self, Employee},
    error::{bad, ApiResult, AppError},
    holidays,
    settings::{self, Settings},
    time, AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/punch", post(punch_portal))
        .route("/punch/status", get(status_portal))
        .route("/terminal/punch", post(punch_terminal))
        .route("/punches", get(list_own))
        .route("/employees/{id}/punches", get(list_admin).post(insert_admin))
        .route("/punches/{id}/storno", post(storno))
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct PunchRow {
    pub id: i64,
    pub employee_id: i64,
    pub ts_utc: String,
    pub art: String,
    pub quelle: String,
    pub erfasst_von: Option<i64>,
    pub kommentar: Option<String>,
    pub storniert_at: Option<String>,
    pub storniert_von: Option<i64>,
    pub storno_grund: Option<String>,
}

impl PunchRow {
    pub fn local(&self) -> Option<Punch> {
        let ts = time::parse_utc(&self.ts_utc)?;
        Some(Punch { at: time::to_local(ts), kind: PunchKind::parse(&self.art)? })
    }
}

/// Aktive Stempelungen im lokalen Datumsbereich [from, to] (inklusive).
pub async fn punches_between(db: &SqlitePool, employee_id: i64, from: NaiveDate, to: NaiveDate) -> ApiResult<Vec<PunchRow>> {
    let start = time::local_to_utc(from.and_hms_opt(0, 0, 0).unwrap());
    let end = time::local_to_utc((to + chrono::Duration::days(1)).and_hms_opt(0, 0, 0).unwrap());
    Ok(sqlx::query_as::<_, PunchRow>(
        "SELECT * FROM punches WHERE employee_id = ? AND storniert_at IS NULL AND ts_utc >= ? AND ts_utc < ? ORDER BY ts_utc",
    )
    .bind(employee_id)
    .bind(time::fmt_utc(start))
    .bind(time::fmt_utc(end))
    .fetch_all(db)
    .await?)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PresenceState {
    Draussen,
    Arbeitet,
    Pause,
}

pub fn presence_state(punches: &[Punch]) -> PresenceState {
    let mut s = PresenceState::Draussen;
    for p in punches {
        s = match (s, p.kind) {
            (PresenceState::Draussen, PunchKind::ClockIn) => PresenceState::Arbeitet,
            (PresenceState::Arbeitet, PunchKind::ClockOut) => PresenceState::Draussen,
            (PresenceState::Arbeitet, PunchKind::BreakStart) => PresenceState::Pause,
            (PresenceState::Pause, PunchKind::BreakEnd) => PresenceState::Arbeitet,
            (PresenceState::Pause, PunchKind::ClockOut) => PresenceState::Draussen,
            (s, _) => s,
        };
    }
    s
}

fn allowed(state: PresenceState, kind: PunchKind) -> bool {
    matches!(
        (state, kind),
        (PresenceState::Draussen, PunchKind::ClockIn)
            | (PresenceState::Arbeitet, PunchKind::ClockOut)
            | (PresenceState::Arbeitet, PunchKind::BreakStart)
            | (PresenceState::Pause, PunchKind::BreakEnd)
            | (PresenceState::Pause, PunchKind::ClockOut)
    )
}

/// Grund, warum „Kommen“ zum Zeitpunkt `at` (lokal) durch die Betriebseinstellungen gesperrt ist.
/// `holiday` ist der Name des Feiertags am Tag von `at`, falls es einer ist.
pub fn clock_in_block(s: &Settings, at: NaiveDateTime, holiday: Option<&str>) -> Option<String> {
    use chrono::{Datelike, Timelike, Weekday};
    if let (false, Some(name)) = (s.stempeln_feiertag, holiday) {
        return Some(format!("Stempeln am Feiertag ({name}) ist nicht vorgesehen."));
    }
    if !s.stempeln_wochenende && matches!(at.weekday(), Weekday::Sat | Weekday::Sun) {
        return Some("Stempeln am Wochenende ist nicht vorgesehen.".into());
    }
    if let (Some(von), Some(bis)) = (settings::parse_hm(&s.stempeln_von), settings::parse_hm(&s.stempeln_bis)) {
        let now = at.hour() * 60 + at.minute();
        let inside = if von <= bis { (von..=bis).contains(&now) } else { now >= von || now <= bis };
        if !inside {
            return Some(format!("Kommen ist nur zwischen {} und {} Uhr möglich.", s.stempeln_von, s.stempeln_bis));
        }
    }
    None
}

/// Aktuelle Stempelsperre für „Kommen“ laut Einstellungen und Feiertagskalender.
async fn current_clock_in_block(db: &SqlitePool) -> ApiResult<Option<String>> {
    use chrono::Datelike;
    let s = settings::load(db).await?;
    let at = time::to_local(time::now_utc());
    let holiday = if s.stempeln_feiertag {
        None
    } else {
        holidays::for_year(db, at.year()).await?.into_iter().find(|h| h.date == at.date()).map(|h| h.name)
    };
    Ok(clock_in_block(&s, at, holiday.as_deref()))
}

/// Stempelung mit Zustandsprüfung. Offene Schicht vom Vortag wird berücksichtigt.
async fn do_punch(db: &SqlitePool, emp: &Employee, kind: PunchKind, quelle: &str, kommentar: Option<String>) -> ApiResult<Value> {
    if !emp.stempelt {
        return Err(AppError::Forbidden);
    }
    if kind == PunchKind::ClockIn {
        if let Some(grund) = current_clock_in_block(db).await? {
            return Err(AppError::Conflict(format!("{grund} Bitte an die Verwaltung wenden.")));
        }
    }
    let now = time::now_utc();
    let today = time::to_local(now).date();
    let yesterday = today - chrono::Duration::days(1);
    let rows = punches_between(db, emp.id, yesterday, today).await?;
    let all: Vec<Punch> = rows.iter().filter_map(|r| r.local()).collect();
    let state = presence_state(&all);
    if !allowed(state, kind) {
        let msg = match state {
            PresenceState::Draussen => "Sie sind nicht eingestempelt. Bitte zuerst „Kommen“ drücken.",
            PresenceState::Arbeitet => "Sie sind bereits eingestempelt.",
            PresenceState::Pause => "Sie sind in der Pause. Bitte „Pause Ende“ oder „Gehen“ drücken.",
        };
        return Err(AppError::Conflict(msg.into()));
    }
    // Doppelklick-Schutz: gleiche Art innerhalb von 60 Sekunden
    if let Some(last) = rows.last() {
        if let Some(ts) = time::parse_utc(&last.ts_utc) {
            if (now - ts).num_seconds() < 60 && last.art == kind.as_str() {
                return Err(AppError::Conflict("Stempelung wurde soeben bereits erfasst".into()));
            }
        }
    }
    sqlx::query("INSERT INTO punches (employee_id, ts_utc, art, quelle, erfasst_von, kommentar) VALUES (?,?,?,?,?,?)")
        .bind(emp.id)
        .bind(time::fmt_utc(now))
        .bind(kind.as_str())
        .bind(quelle)
        .bind(emp.id)
        .bind(kommentar)
        .execute(db)
        .await?;
    status_json(db, emp).await
}

pub async fn status_json(db: &SqlitePool, emp: &Employee) -> ApiResult<Value> {
    let now = time::now_utc();
    let today = time::to_local(now).date();
    let rows = punches_between(db, emp.id, today - chrono::Duration::days(1), today).await?;
    let all: Vec<Punch> = rows.iter().filter_map(|r| r.local()).collect();
    let state = presence_state(&all);
    // Laufende Nachtschicht: begann die offene Schicht gestern, wird dieser Schichttag angezeigt.
    let yesterday = today - chrono::Duration::days(1);
    let ctx = calc::Context::load(db, emp, yesterday, today).await?;
    let today_view = ctx.day(today);
    let day = if state != PresenceState::Draussen && today_view.punches.is_empty() {
        let y = ctx.day(yesterday);
        if y.result.open_shift { y } else { today_view }
    } else {
        today_view
    };
    // Saldo bis gestern: der laufende Tag ist erst nach „Gehen“ aussagekräftig.
    let saldo = calc::saldo_until(db, emp, yesterday).await?;
    let sperre = if state == PresenceState::Draussen { current_clock_in_block(db).await? } else { None };
    Ok(json!({
        "name": emp.display_name(),
        "personalnr": emp.personalnr,
        "zustand": state,
        "sperre": sperre,
        "jetzt": time::to_local(now).format("%H:%M").to_string(),
        "heute": day.punches,
        "tag": day,
        "saldo_min": saldo,
    }))
}

#[derive(Deserialize)]
struct PunchReq {
    art: String,
    kommentar: Option<String>,
}

async fn punch_portal(State(state): State<AppState>, CurrentUser(user): CurrentUser, Json(req): Json<PunchReq>) -> ApiResult<Json<Value>> {
    let kind = PunchKind::parse(&req.art).ok_or_else(|| bad("Unbekannte Stempelart"))?;
    Ok(Json(do_punch(&state.db, &user, kind, "portal", req.kommentar).await?))
}

async fn status_portal(State(state): State<AppState>, CurrentUser(user): CurrentUser) -> ApiResult<Json<Value>> {
    Ok(Json(status_json(&state.db, &user).await?))
}

#[derive(Deserialize)]
struct TerminalReq {
    personalnr: String,
    pin: String,
    /// Leer = nur Status abfragen
    art: Option<String>,
}

async fn punch_terminal(State(state): State<AppState>, headers: axum::http::HeaderMap, Json(req): Json<TerminalReq>) -> ApiResult<Json<Value>> {
    let nr = req.personalnr.trim().trim_start_matches('0').to_string();
    let key = format!("terminal:{nr}");
    if let Some(secs) = state.limiter.locked(&key) {
        return Err(AppError::TooMany(format!("Zu viele Fehlversuche. Bitte in {} Minuten erneut versuchen.", secs.div_ceil(60))));
    }
    let _ = headers;
    let emp = sqlx::query_as::<_, Employee>("SELECT * FROM employees WHERE personalnr = ? AND aktiv = 1")
        .bind(&nr)
        .fetch_optional(&state.db)
        .await?
        .or(sqlx::query_as::<_, Employee>("SELECT * FROM employees WHERE personalnr = ? AND aktiv = 1")
            .bind(req.personalnr.trim())
            .fetch_optional(&state.db)
            .await?);
    let ok = emp.as_ref().filter(|e| e.stempelt).and_then(|e| e.pin_hash.as_deref()).map(|h| auth::verify_secret(&req.pin, h)).unwrap_or(false);
    if !ok {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        if state.limiter.failure(&key) {
            db::audit(&state.db, None, "terminal_gesperrt", Some(key.clone()), None, None).await?;
        }
        return Err(AppError::Unauthorized);
    }
    state.limiter.success(&key);
    let emp = emp.unwrap();
    match req.art.as_deref().filter(|a| !a.is_empty()) {
        Some(a) => {
            let kind = PunchKind::parse(a).ok_or_else(|| bad("Unbekannte Stempelart"))?;
            Ok(Json(do_punch(&state.db, &emp, kind, "terminal", None).await?))
        }
        None => Ok(Json(status_json(&state.db, &emp).await?)),
    }
}

#[derive(Deserialize)]
pub struct RangeQuery {
    pub von: String,
    pub bis: String,
}

pub fn parse_range(q: &RangeQuery) -> ApiResult<(NaiveDate, NaiveDate)> {
    let from = time::parse_date(&q.von).ok_or_else(|| bad("von ungültig"))?;
    let to = time::parse_date(&q.bis).ok_or_else(|| bad("bis ungültig"))?;
    if to < from || (to - from).num_days() > 1100 {
        return Err(bad("Zeitraum ungültig (maximal drei Jahre)"));
    }
    Ok((from, to))
}

fn row_json(r: &PunchRow) -> Value {
    let local = r.local();
    json!({
        "id": r.id,
        "employee_id": r.employee_id,
        "ts_utc": r.ts_utc,
        "datum": local.as_ref().map(|p| time::fmt_date(p.at.date())),
        "zeit": local.as_ref().map(|p| p.at.format("%H:%M").to_string()),
        "art": r.art,
        "quelle": r.quelle,
        "erfasst_von": r.erfasst_von,
        "kommentar": r.kommentar,
        "storniert_at": r.storniert_at,
        "storno_grund": r.storno_grund,
    })
}

async fn list_own(State(state): State<AppState>, CurrentUser(user): CurrentUser, Query(q): Query<RangeQuery>) -> ApiResult<Json<Vec<Value>>> {
    let (from, to) = parse_range(&q)?;
    Ok(Json(punches_between(&state.db, user.id, from, to).await?.iter().map(row_json).collect()))
}

/// Admin sieht auch stornierte Stempelungen.
async fn list_admin(State(state): State<AppState>, AdminUser(_): AdminUser, Path(id): Path<i64>, Query(q): Query<RangeQuery>) -> ApiResult<Json<Vec<Value>>> {
    let (from, to) = parse_range(&q)?;
    let start = time::local_to_utc(from.and_hms_opt(0, 0, 0).unwrap());
    let end = time::local_to_utc((to + chrono::Duration::days(1)).and_hms_opt(0, 0, 0).unwrap());
    let rows = sqlx::query_as::<_, PunchRow>("SELECT * FROM punches WHERE employee_id = ? AND ts_utc >= ? AND ts_utc < ? ORDER BY ts_utc")
        .bind(id)
        .bind(time::fmt_utc(start))
        .bind(time::fmt_utc(end))
        .fetch_all(&state.db)
        .await?;
    Ok(Json(rows.iter().map(row_json).collect()))
}

#[derive(Deserialize)]
struct AdminPunchReq {
    /// Lokalzeit "YYYY-MM-DDTHH:MM"
    zeit: String,
    art: String,
    kommentar: Option<String>,
}

async fn insert_admin(State(state): State<AppState>, AdminUser(admin): AdminUser, Path(id): Path<i64>, Json(req): Json<AdminPunchReq>) -> ApiResult<Json<Value>> {
    db::get_employee(&state.db, id).await?;
    let kind = PunchKind::parse(&req.art).ok_or_else(|| bad("Unbekannte Stempelart"))?;
    let local: NaiveDateTime = NaiveDateTime::parse_from_str(&req.zeit, "%Y-%m-%dT%H:%M").map_err(|_| bad("Zeit im Format YYYY-MM-DDTHH:MM"))?;
    let ts: DateTime<Utc> = time::local_to_utc(local);
    calc::ensure_month_open(&state.db, id, local.date()).await?;
    let r = sqlx::query("INSERT INTO punches (employee_id, ts_utc, art, quelle, erfasst_von, kommentar) VALUES (?,?,?,'admin',?,?)")
        .bind(id)
        .bind(time::fmt_utc(ts))
        .bind(kind.as_str())
        .bind(admin.id)
        .bind(&req.kommentar)
        .execute(&state.db)
        .await?;
    db::audit(&state.db, Some(admin.id), "stempelung_eingefuegt", Some(format!("employee:{id}")), None, Some(json!({"punch_id": r.last_insert_rowid(), "zeit": req.zeit, "art": req.art, "kommentar": req.kommentar}))).await?;
    Ok(Json(json!({"id": r.last_insert_rowid()})))
}

#[derive(Deserialize)]
struct StornoReq {
    grund: String,
}

async fn storno(State(state): State<AppState>, AdminUser(admin): AdminUser, Path(id): Path<i64>, Json(req): Json<StornoReq>) -> ApiResult<Json<Value>> {
    if req.grund.trim().is_empty() {
        return Err(bad("Begründung ist Pflicht"));
    }
    let row = sqlx::query_as::<_, PunchRow>("SELECT * FROM punches WHERE id = ?").bind(id).fetch_optional(&state.db).await?.ok_or(AppError::NotFound)?;
    if row.storniert_at.is_some() {
        return Err(AppError::Conflict("Bereits storniert".into()));
    }
    if let Some(p) = row.local() {
        calc::ensure_month_open(&state.db, row.employee_id, p.at.date()).await?;
    }
    sqlx::query("UPDATE punches SET storniert_at = ?, storniert_von = ?, storno_grund = ? WHERE id = ?")
        .bind(time::fmt_utc(time::now_utc()))
        .bind(admin.id)
        .bind(req.grund.trim())
        .bind(id)
        .execute(&state.db)
        .await?;
    db::audit(&state.db, Some(admin.id), "stempelung_storniert", Some(format!("punch:{id}")), Some(row_json(&row)), Some(json!({"grund": req.grund}))).await?;
    Ok(Json(json!({"ok": true})))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(date: &str, hm: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(&format!("{date} {hm}"), "%Y-%m-%d %H:%M").unwrap()
    }

    #[test]
    fn keine_sperre_in_standardeinstellung() {
        let s = Settings::default();
        assert_eq!(clock_in_block(&s, at("2026-09-12", "03:00"), Some("Feiertag")), None);
    }

    #[test]
    fn wochenende_und_feiertag() {
        let s = Settings { stempeln_wochenende: false, stempeln_feiertag: false, ..Settings::default() };
        assert!(clock_in_block(&s, at("2026-09-12", "09:00"), None).unwrap().contains("Wochenende")); // Samstag
        assert_eq!(clock_in_block(&s, at("2026-09-14", "09:00"), None), None); // Montag
        assert!(clock_in_block(&s, at("2026-10-26", "09:00"), Some("Nationalfeiertag")).unwrap().contains("Nationalfeiertag"));
    }

    #[test]
    fn stempelfenster_auch_ueber_mitternacht() {
        let s = Settings { stempeln_von: "06:00".into(), stempeln_bis: "20:00".into(), ..Settings::default() };
        assert_eq!(clock_in_block(&s, at("2026-09-14", "06:00"), None), None);
        assert_eq!(clock_in_block(&s, at("2026-09-14", "20:00"), None), None);
        assert!(clock_in_block(&s, at("2026-09-14", "05:59"), None).is_some());
        assert!(clock_in_block(&s, at("2026-09-14", "22:30"), None).is_some());
        let nacht = Settings { stempeln_von: "20:00".into(), stempeln_bis: "06:00".into(), ..Settings::default() };
        assert_eq!(clock_in_block(&nacht, at("2026-09-14", "23:00"), None), None);
        assert_eq!(clock_in_block(&nacht, at("2026-09-14", "05:00"), None), None);
        assert!(clock_in_block(&nacht, at("2026-09-14", "12:00"), None).is_some());
    }
}
