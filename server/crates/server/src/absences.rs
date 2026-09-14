//! Abwesenheiten: Anträge, Genehmigung, Admin-Buchungen, Urlaubs- und Gutstundenkonto.

use axum::{
    extract::{Path, Query, State},
    routing::{get, post},
    Json, Router,
};
use chrono::{Datelike, Duration, NaiveDate};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{FromRow, SqlitePool};
use timecard_domain::{vacation_days_in_range, AbsenceKind, AbsenceUnit};

use crate::{
    auth::{AdminUser, CurrentUser},
    calc,
    db::{self, Employee},
    error::{bad, ApiResult, AppError},
    holidays, punches::{parse_range, RangeQuery}, settings, time, AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/absences", get(list_own).post(request_own))
        .route("/absences/account", get(account_own))
        .route("/absences/{id}/withdraw", post(withdraw_own))
        .route("/admin/absences", get(list_admin))
        .route("/absences/{id}/decide", post(decide))
        .route("/absences/{id}/storno", post(storno))
        .route("/employees/{id}/absences", get(list_for_employee).post(create_admin))
        .route("/employees/{id}/vacation", get(vacation_admin).post(vacation_entry))
        .route("/employees/{id}/vacation/opening", post(vacation_opening))
        .route("/employees/{id}/credit", get(credit_admin).post(credit_entry))
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct AbsenceRow {
    pub id: i64,
    pub employee_id: i64,
    pub art: String,
    pub von: String,
    pub bis: String,
    pub einheit: String,
    pub wert: Option<f64>,
    pub status: String,
    pub kommentar: Option<String>,
    pub beantragt_von: Option<i64>,
    pub beantragt_at: String,
    pub entschieden_von: Option<i64>,
    pub entschieden_at: Option<String>,
    pub entscheidung_kommentar: Option<String>,
}

impl AbsenceRow {
    pub fn covers(&self, date: NaiveDate) -> bool {
        let d = time::fmt_date(date);
        self.von <= d && d <= self.bis
    }
    pub fn label(&self) -> String {
        AbsenceKind::parse(&self.art).map(|k| k.label().to_string()).unwrap_or_else(|| self.art.clone())
    }
}

fn row_json(r: &AbsenceRow) -> Value {
    let mut v = serde_json::to_value(r).unwrap_or(Value::Null);
    v["label"] = json!(r.label());
    v
}

pub async fn approved_between(db: &SqlitePool, employee_id: i64, from: NaiveDate, to: NaiveDate) -> ApiResult<Vec<AbsenceRow>> {
    Ok(sqlx::query_as::<_, AbsenceRow>(
        "SELECT * FROM absences WHERE employee_id = ? AND status = 'genehmigt' AND von <= ? AND bis >= ? ORDER BY von",
    )
    .bind(employee_id)
    .bind(time::fmt_date(to))
    .bind(time::fmt_date(from))
    .fetch_all(db)
    .await?)
}

#[derive(Deserialize)]
pub struct AbsenceReq {
    art: String,
    von: String,
    bis: String,
    einheit: Option<String>,
    wert: Option<f64>,
    kommentar: Option<String>,
}

struct Validated {
    kind: AbsenceKind,
    from: NaiveDate,
    to: NaiveDate,
    unit: AbsenceUnit,
    wert: Option<f64>,
}

async fn validate(db: &SqlitePool, emp: &Employee, req: &AbsenceReq, by_admin: bool, exclude_id: Option<i64>) -> ApiResult<Validated> {
    let kind = AbsenceKind::parse(&req.art).ok_or_else(|| bad("Unbekannte Abwesenheitsart"))?;
    if !by_admin && !kind.employee_may_request() {
        return Err(bad(format!("{} kann nur durch die Verwaltung gebucht werden", kind.label())));
    }
    let from = time::parse_date(&req.von).ok_or_else(|| bad("Von-Datum ungültig"))?;
    let to = time::parse_date(&req.bis).ok_or_else(|| bad("Bis-Datum ungültig"))?;
    if to < from {
        return Err(bad("Bis liegt vor Von"));
    }
    if (to - from).num_days() > 366 {
        return Err(bad("Zeitraum zu lang"));
    }
    let unit = AbsenceUnit::parse(req.einheit.as_deref().unwrap_or("tag")).ok_or_else(|| bad("Einheit ungültig"))?;
    let s = settings::load(db).await?;
    if kind.consumes_vacation() {
        if unit == AbsenceUnit::HalberTag && !s.urlaub_halbe_tage {
            return Err(bad("Halbe Urlaubstage sind nicht freigegeben"));
        }
        if unit == AbsenceUnit::Stunden && !s.urlaub_stunden {
            return Err(bad("Stundenweiser Urlaub ist nicht freigegeben"));
        }
    }
    let wert = match unit {
        AbsenceUnit::Stunden => {
            let w = req.wert.ok_or_else(|| bad("Stundenwert fehlt"))?;
            if w <= 0.0 || w > 24.0 {
                return Err(bad("Stundenwert ungültig"));
            }
            if from != to {
                return Err(bad("Stundenweise Abwesenheit nur für einen einzelnen Tag"));
            }
            Some(w)
        }
        AbsenceUnit::HalberTag => {
            if from != to {
                return Err(bad("Halber Tag nur für einen einzelnen Tag"));
            }
            None
        }
        AbsenceUnit::Tag => None,
    };
    // Überschneidung mit offenen oder genehmigten Abwesenheiten
    let overlapping = sqlx::query_as::<_, AbsenceRow>(
        "SELECT * FROM absences WHERE employee_id = ? AND status IN ('beantragt','genehmigt') AND von <= ? AND bis >= ? AND id != ?",
    )
    .bind(emp.id).bind(&req.bis).bind(&req.von).bind(exclude_id.unwrap_or(-1))
    .fetch_all(db)
    .await?;
    // Stundenweise und halbe Tage dürfen sich mit anderen stundenweisen am selben Tag überschneiden.
    let conflict = overlapping.iter().find(|o| !(unit != AbsenceUnit::Tag && o.einheit != "tag"));
    if let Some(o) = conflict {
        return Err(AppError::Conflict(format!("Überschneidung mit {} ({} bis {}), Status {}", o.label(), time::fmt_date_de(time::parse_date(&o.von).unwrap()), time::fmt_date_de(time::parse_date(&o.bis).unwrap()), o.status)));
    }
    if !by_admin {
        for d in [from, to] {
            calc::ensure_month_open(db, emp.id, d).await?;
        }
    }
    Ok(Validated { kind, from, to, unit, wert })
}

async fn insert(db: &SqlitePool, emp: &Employee, v: &Validated, req: &AbsenceReq, status: &str, actor: i64) -> ApiResult<i64> {
    let r = sqlx::query(
        "INSERT INTO absences (employee_id, art, von, bis, einheit, wert, status, kommentar, beantragt_von, entschieden_von, entschieden_at)
         VALUES (?,?,?,?,?,?,?,?,?,?,?)",
    )
    .bind(emp.id)
    .bind(v.kind.as_str())
    .bind(time::fmt_date(v.from))
    .bind(time::fmt_date(v.to))
    .bind(v.unit.as_str())
    .bind(v.wert)
    .bind(status)
    .bind(req.kommentar.as_deref().map(str::trim).filter(|k| !k.is_empty()))
    .bind(actor)
    .bind(if status == "genehmigt" { Some(actor) } else { None })
    .bind(if status == "genehmigt" { Some(time::fmt_utc(time::now_utc())) } else { None })
    .execute(db)
    .await?;
    Ok(r.last_insert_rowid())
}

async fn list_own(State(state): State<AppState>, CurrentUser(user): CurrentUser, Query(q): Query<RangeQuery>) -> ApiResult<Json<Vec<Value>>> {
    let (from, to) = parse_range(&q)?;
    let rows = sqlx::query_as::<_, AbsenceRow>("SELECT * FROM absences WHERE employee_id = ? AND von <= ? AND bis >= ? ORDER BY von DESC")
        .bind(user.id).bind(time::fmt_date(to)).bind(time::fmt_date(from)).fetch_all(&state.db).await?;
    Ok(Json(rows.iter().map(row_json).collect()))
}

async fn request_own(State(state): State<AppState>, CurrentUser(user): CurrentUser, Json(req): Json<AbsenceReq>) -> ApiResult<Json<Value>> {
    if !user.stempelt {
        return Err(AppError::Forbidden);
    }
    let v = validate(&state.db, &user, &req, false, None).await?;
    if v.kind.consumes_vacation() {
        let acct = vacation_account(&state.db, &user, v.to).await?;
        let needed = vacation_days_for(&state.db, &user, &v).await?;
        let rest = acct["rest"].as_f64().unwrap_or(0.0);
        if needed > rest + 1e-9 {
            return Err(bad(format!("Nicht genügend Resturlaub: benötigt {needed:.1}, verfügbar {rest:.1} Tage")));
        }
    }
    let id = insert(&state.db, &user, &v, &req, "beantragt", user.id).await?;
    db::audit(&state.db, Some(user.id), "abwesenheit_beantragt", Some(format!("absence:{id}")), None, Some(json!({"art": req.art, "von": req.von, "bis": req.bis}))).await?;
    Ok(Json(json!({"id": id})))
}

async fn withdraw_own(State(state): State<AppState>, CurrentUser(user): CurrentUser, Path(id): Path<i64>) -> ApiResult<Json<Value>> {
    let r = sqlx::query("UPDATE absences SET status = 'storniert' WHERE id = ? AND employee_id = ? AND status = 'beantragt'")
        .bind(id).bind(user.id).execute(&state.db).await?;
    if r.rows_affected() == 0 {
        return Err(AppError::Conflict("Nur offene Anträge können zurückgezogen werden".into()));
    }
    db::audit(&state.db, Some(user.id), "antrag_zurueckgezogen", Some(format!("absence:{id}")), None, None).await?;
    Ok(Json(json!({"ok": true})))
}

#[derive(Deserialize)]
struct AdminListQuery {
    status: Option<String>,
    von: Option<String>,
    bis: Option<String>,
}

async fn list_admin(State(state): State<AppState>, AdminUser(_): AdminUser, Query(q): Query<AdminListQuery>) -> ApiResult<Json<Vec<Value>>> {
    let status = q.status.unwrap_or_else(|| "beantragt".into());
    let von = q.von.unwrap_or_else(|| "0000-00-00".into());
    let bis = q.bis.unwrap_or_else(|| "9999-12-31".into());
    let rows = sqlx::query_as::<_, (i64, String, String, String, String, String, String, String, Option<f64>, String, Option<String>, String)>(
        "SELECT a.id, e.vorname, e.nachname, e.personalnr, a.art, a.von, a.bis, a.einheit, a.wert, a.status, a.kommentar, a.beantragt_at
         FROM absences a JOIN employees e ON e.id = a.employee_id
         WHERE (? = 'alle' OR a.status = ?) AND a.von <= ? AND a.bis >= ?
         ORDER BY a.beantragt_at DESC LIMIT 500",
    )
    .bind(&status).bind(&status).bind(&bis).bind(&von)
    .fetch_all(&state.db)
    .await?;
    Ok(Json(rows.into_iter().map(|(id, vn, nn, pnr, art, von, bis, einheit, wert, status, kommentar, at)| json!({
        "id": id, "name": format!("{vn} {nn}"), "personalnr": pnr, "art": art,
        "label": AbsenceKind::parse(&art).map(|k| k.label()).unwrap_or(""),
        "von": von, "bis": bis, "einheit": einheit, "wert": wert, "status": status, "kommentar": kommentar, "beantragt_at": at,
    })).collect()))
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
    let row = sqlx::query_as::<_, AbsenceRow>("SELECT * FROM absences WHERE id = ?").bind(id).fetch_optional(&state.db).await?.ok_or(AppError::NotFound)?;
    if row.status != "beantragt" {
        return Err(AppError::Conflict("Antrag ist nicht mehr offen".into()));
    }
    if req.status == "genehmigt" {
        let emp = db::get_employee(&state.db, row.employee_id).await?;
        // Nochmals auf Überschneidung prüfen (könnte inzwischen entstanden sein)
        let r = AbsenceReq { art: row.art.clone(), von: row.von.clone(), bis: row.bis.clone(), einheit: Some(row.einheit.clone()), wert: row.wert, kommentar: None };
        validate(&state.db, &emp, &r, true, Some(id)).await?;
    }
    sqlx::query("UPDATE absences SET status = ?, entschieden_von = ?, entschieden_at = ?, entscheidung_kommentar = ? WHERE id = ?")
        .bind(&req.status).bind(admin.id).bind(time::fmt_utc(time::now_utc())).bind(&req.kommentar).bind(id)
        .execute(&state.db).await?;
    db::audit(&state.db, Some(admin.id), "antrag_entschieden", Some(format!("absence:{id}")), Some(row_json(&row)), Some(json!({"status": req.status, "kommentar": req.kommentar}))).await?;
    Ok(Json(json!({"ok": true})))
}

#[derive(Deserialize)]
struct StornoReq {
    grund: String,
}

async fn storno(State(state): State<AppState>, AdminUser(admin): AdminUser, Path(id): Path<i64>, Json(req): Json<StornoReq>) -> ApiResult<Json<Value>> {
    if req.grund.trim().is_empty() {
        return Err(bad("Begründung ist Pflicht"));
    }
    let row = sqlx::query_as::<_, AbsenceRow>("SELECT * FROM absences WHERE id = ?").bind(id).fetch_optional(&state.db).await?.ok_or(AppError::NotFound)?;
    if row.status == "storniert" {
        return Err(AppError::Conflict("Bereits storniert".into()));
    }
    for d in [&row.von, &row.bis] {
        if let Some(d) = time::parse_date(d) {
            calc::ensure_month_open(&state.db, row.employee_id, d).await?;
        }
    }
    sqlx::query("UPDATE absences SET status = 'storniert', entscheidung_kommentar = ?, entschieden_von = ?, entschieden_at = ? WHERE id = ?")
        .bind(req.grund.trim()).bind(admin.id).bind(time::fmt_utc(time::now_utc())).bind(id).execute(&state.db).await?;
    db::audit(&state.db, Some(admin.id), "abwesenheit_storniert", Some(format!("absence:{id}")), Some(row_json(&row)), Some(json!({"grund": req.grund}))).await?;
    Ok(Json(json!({"ok": true})))
}

async fn list_for_employee(State(state): State<AppState>, AdminUser(_): AdminUser, Path(id): Path<i64>, Query(q): Query<RangeQuery>) -> ApiResult<Json<Vec<Value>>> {
    let (from, to) = parse_range(&q)?;
    let rows = sqlx::query_as::<_, AbsenceRow>("SELECT * FROM absences WHERE employee_id = ? AND von <= ? AND bis >= ? ORDER BY von DESC")
        .bind(id).bind(time::fmt_date(to)).bind(time::fmt_date(from)).fetch_all(&state.db).await?;
    Ok(Json(rows.iter().map(row_json).collect()))
}

/// Admin bucht direkt (Krankenstand, Urlaub, Absonderung ...). Gilt sofort als genehmigt.
async fn create_admin(State(state): State<AppState>, AdminUser(admin): AdminUser, Path(id): Path<i64>, Json(req): Json<AbsenceReq>) -> ApiResult<Json<Value>> {
    let emp = db::get_employee(&state.db, id).await?;
    let v = validate(&state.db, &emp, &req, true, None).await?;
    for d in [v.from, v.to] {
        calc::ensure_month_open(&state.db, emp.id, d).await?;
    }
    let mut hinweise = Vec::new();
    if v.kind.consumes_vacation() {
        let acct = vacation_account(&state.db, &emp, v.to).await?;
        let needed = vacation_days_for(&state.db, &emp, &v).await?;
        let rest = acct["rest"].as_f64().unwrap_or(0.0);
        if needed > rest + 1e-9 {
            hinweise.push(format!("Resturlaub wird negativ: benötigt {needed:.1}, verfügbar {rest:.1} Tage"));
        }
    }
    let new_id = insert(&state.db, &emp, &v, &req, "genehmigt", admin.id).await?;
    db::audit(&state.db, Some(admin.id), "abwesenheit_gebucht", Some(format!("absence:{new_id}")), None, Some(json!({"employee_id": id, "art": req.art, "von": req.von, "bis": req.bis, "einheit": v.unit.as_str(), "wert": v.wert}))).await?;
    Ok(Json(json!({"id": new_id, "hinweise": hinweise})))
}

// ---------------------------------------------------------------------------
// Urlaubskonto
// ---------------------------------------------------------------------------

/// Beginn des Urlaubsjahres, in dem `date` liegt.
pub fn vacation_year_start(emp: &Employee, date: NaiveDate) -> NaiveDate {
    let (mm, dd) = emp
        .urlaubsjahr_beginn_mm_dd
        .split_once('-')
        .and_then(|(m, d)| Some((m.parse::<u32>().ok()?, d.parse::<u32>().ok()?)))
        .unwrap_or((1, 1));
    let candidate = NaiveDate::from_ymd_opt(date.year(), mm, dd)
        .or_else(|| NaiveDate::from_ymd_opt(date.year(), mm, 28))
        .unwrap();
    if candidate <= date {
        candidate
    } else {
        NaiveDate::from_ymd_opt(date.year() - 1, mm, dd).or_else(|| NaiveDate::from_ymd_opt(date.year() - 1, mm, 28)).unwrap()
    }
}

/// Verjährung eines Anspruchs nach § 4 Abs 5 UrlG: Ende des zweiten Urlaubsjahres nach dem, in dem er entstand.
fn expiry_date(emp: &Employee, bucket_start: NaiveDate) -> NaiveDate {
    let e1 = next_year_start(emp, bucket_start);
    let e2 = next_year_start(emp, e1);
    next_year_start(emp, e2) - Duration::days(1)
}

fn next_year_start(emp: &Employee, start: NaiveDate) -> NaiveDate {
    vacation_year_start(emp, NaiveDate::from_ymd_opt(start.year() + 1, start.month(), start.day().min(28)).unwrap())
}

/// Urlaubstage, die eine Abwesenheit verbraucht.
async fn vacation_days_for(db: &SqlitePool, emp: &Employee, v: &Validated) -> ApiResult<f64> {
    let schedules = db::schedules_for(db, emp.id).await?;
    let mut hol = Vec::new();
    for y in v.from.year()..=v.to.year() {
        hol.extend(holidays::for_year(db, y).await?.into_iter().map(|h| h.date));
    }
    Ok(match v.unit {
        AbsenceUnit::Tag => {
            let mut days = 0.0;
            let mut d = v.from;
            while d <= v.to {
                if let Some(s) = db::schedule_at(&schedules, d) {
                    days += vacation_days_in_range(&s.week_model(), d, d, &hol);
                }
                d += Duration::days(1);
            }
            days
        }
        AbsenceUnit::HalberTag => 0.5,
        AbsenceUnit::Stunden => {
            let target = db::schedule_at(&schedules, v.from).map(|s| s.week_model().target_for(v.from)).unwrap_or(480);
            if target == 0 { 0.0 } else { v.wert.unwrap_or(0.0) * 60.0 / target as f64 }
        }
    })
}

async fn consumed_days(db: &SqlitePool, emp: &Employee, from: NaiveDate, to: NaiveDate) -> ApiResult<(f64, Vec<Value>)> {
    let rows = sqlx::query_as::<_, AbsenceRow>(
        "SELECT * FROM absences WHERE employee_id = ? AND status = 'genehmigt' AND art IN ('urlaub','pers_feiertag') AND von <= ? AND bis >= ? ORDER BY von",
    )
    .bind(emp.id).bind(time::fmt_date(to)).bind(time::fmt_date(from)).fetch_all(db).await?;
    let mut total = 0.0;
    let mut list = Vec::new();
    for r in rows {
        let rf = time::parse_date(&r.von).unwrap().max(from);
        let rt = time::parse_date(&r.bis).unwrap().min(to);
        let v = Validated { kind: AbsenceKind::Urlaub, from: rf, to: rt, unit: AbsenceUnit::parse(&r.einheit).unwrap_or(AbsenceUnit::Tag), wert: r.wert };
        let d = vacation_days_for(db, emp, &v).await?;
        total += d;
        let mut j = row_json(&r);
        j["tage"] = json!(d);
        list.push(j);
    }
    Ok((total, list))
}

/// Anspruch eines Urlaubsjahres: expliziter Eintrag oder automatisch (erstes Jahr aliquot in halben Tagen).
fn entitlement_for(emp: &Employee, eintritt: NaiveDate, year: NaiveDate, year_end: NaiveDate, explicit: Option<f64>) -> f64 {
    if let Some(e) = explicit {
        return e;
    }
    let start = year.max(eintritt);
    let days_in_year = (year_end - year).num_days() + 1;
    let days_employed = (year_end - start).num_days() + 1;
    if days_employed >= days_in_year {
        emp.urlaubsanspruch_tage
    } else {
        (emp.urlaubsanspruch_tage * days_employed as f64 / days_in_year as f64 * 2.0).round() / 2.0
    }
}

/// Urlaubskonto zum Stichtag mit Verbrauch nach FIFO (ältester Anspruch zuerst) und Verfall nach
/// § 4 Abs 5 UrlG: ein Anspruch verjährt zwei Jahre nach Ende des Urlaubsjahres, in dem er entstand.
/// Der Übertrag ergibt sich aus den offenen Ansprüchen der Vorjahre, sofern kein expliziter
/// Übertrag-Eintrag erfasst ist (dieser ersetzt alle älteren Ansprüche und gilt als Anspruch des Vorjahres).
pub async fn vacation_account(db: &SqlitePool, emp: &Employee, as_of: NaiveDate) -> ApiResult<Value> {
    let eintritt = time::parse_date(&emp.eintritt).ok_or_else(|| bad("Eintritt ungültig"))?;
    let s = settings::load(db).await?;
    let target_year = vacation_year_start(emp, as_of);
    // Urlaubsjahr, in dem die Zeiterfassung beginnt: Vorjahre davor sind nicht erfasst und zählen nur
    // über einen expliziten Übertrag (Resturlaub bei Erstanlage). Ohne Übertrag gilt 0.
    let erfassung_ab = time::parse_date(&emp.durchrechnung_start).unwrap_or(eintritt);
    let start_year = vacation_year_start(emp, calc::saldo_start(emp)?);
    let eintritt_year = vacation_year_start(emp, eintritt);
    // Die Durchrechnung beginnt frühestens im Erfassungsjahr; liegt der Stichtag davor, ist auch dort
    // nichts erfasst. Frühere Jahre werden nicht durchgerechnet – ihr Ergebnis würde ohnehin verworfen.
    let first_year = start_year.min(target_year);
    let mut uebertrag_offen = false;
    let mut year = first_year;
    let entries = sqlx::query_as::<_, (i64, String, String, f64, Option<String>, String)>(
        "SELECT id, urlaubsjahr, art, tage, grund, created_at FROM vacation_entries WHERE employee_id = ? ORDER BY urlaubsjahr, id",
    )
    .bind(emp.id).fetch_all(db).await?;
    // Offene Ansprüche je Ursprungsjahr (Start des Urlaubsjahres, Rest), aufsteigend = FIFO
    let mut buckets: Vec<(NaiveDate, f64)> = Vec::new();
    let mut history: Vec<Value> = Vec::new();
    let mut result = json!({});
    for _ in 0..80 {
        let year_end = next_year_start(emp, year) - Duration::days(1);
        let ys = time::fmt_date(year);
        let year_entries: Vec<_> = entries.iter().filter(|e| e.1 == ys).collect();
        let explicit_anspruch = year_entries.iter().filter(|e| e.2 == "anspruch").map(|e| e.3).last();
        let anspruch = entitlement_for(emp, eintritt, year, year_end, explicit_anspruch);
        if let Some(u) = year_entries.iter().filter(|e| e.2 == "uebertrag").map(|e| e.3).last() {
            // Expliziter Übertrag ersetzt die durchgerechneten Vorjahre; gilt als Anspruch des Vorjahres.
            buckets.clear();
            if u > 0.0 {
                buckets.push((vacation_year_start(emp, year - Duration::days(1)), u));
            }
        } else if year == first_year && year > eintritt_year {
            // Erstes durchgerechnetes Jahr ohne Erstanlage: Vorjahre gelten als 0, bis der Resturlaub erfasst ist.
            uebertrag_offen = true;
        }
        let uebertrag: f64 = buckets.iter().map(|b| b.1).sum();
        let korrektur: f64 = year_entries.iter().filter(|e| e.2 == "korrektur").map(|e| e.3).sum();
        let verfall_manuell: f64 = year_entries.iter().filter(|e| e.2 == "verfall").map(|e| e.3).sum();
        buckets.push((year, anspruch + korrektur));
        buckets.sort_by_key(|b| b.0);
        // Manueller Verfall (negativ) wird vom ältesten Anspruch abgezogen
        let mut manual = -verfall_manuell;
        for b in buckets.iter_mut() {
            if manual <= 0.0 { break; }
            let take = b.1.min(manual);
            b.1 -= take;
            manual -= take;
        }
        // Verbrauch im Jahr, chronologisch, FIFO
        let (verbrauch, list) = consumed_days(db, emp, year, year_end).await?;
        let mut rest_consume = verbrauch;
        for b in buckets.iter_mut() {
            if rest_consume <= 1e-9 { break; }
            let take = b.1.min(rest_consume);
            b.1 -= take;
            rest_consume -= take;
        }
        let ueberzogen = rest_consume.max(0.0);
        buckets.retain(|b| b.1 > 1e-9);
        // Verfall am Jahresende: Ansprüche, deren Verjährungsdatum in diesem Jahr liegt
        let mut verfall_auto = 0.0;
        let year_over = year_end < as_of;
        if s.urlaub_verfall_auto && year_over {
            let (expired, keep): (Vec<_>, Vec<_>) = buckets.iter().cloned().partition(|b| expiry_date(emp, b.0) <= year_end);
            verfall_auto = expired.iter().map(|b| b.1).sum();
            buckets = keep;
        }
        let rest: f64 = buckets.iter().map(|b| b.1).sum::<f64>() - ueberzogen;
        let rest = if rest == 0.0 { 0.0 } else { rest }; // kein -0
        let year_json = json!({
            "urlaubsjahr_von": ys,
            "urlaubsjahr_bis": time::fmt_date(year_end),
            "anspruch": anspruch,
            "anspruch_aliquot": explicit_anspruch.is_none() && anspruch != emp.urlaubsanspruch_tage,
            "uebertrag": uebertrag,
            "korrektur": korrektur,
            "verfall_manuell": verfall_manuell,
            "verfall_auto": -verfall_auto,
            "verbrauch": verbrauch,
            "rest": rest,
        });
        if year == target_year {
            let (verbrauch_bis_stichtag, _) = consumed_days(db, emp, year, as_of).await?;
            let expiry_of = |start: NaiveDate| time::fmt_date(expiry_date(emp, start));
            // Nächster Verfall: ältester offener Anspruch
            let next_expiry = buckets.first().map(|b| json!({"tage": b.1, "am": expiry_of(b.0), "aus_urlaubsjahr": time::fmt_date(b.0)}));
            let mut r = year_json.clone();
            r["verbrauch_bis_stichtag"] = json!(verbrauch_bis_stichtag);
            r["geplant"] = json!(verbrauch - verbrauch_bis_stichtag);
            // Rest ohne die bereits genehmigten, noch nicht angetretenen Tage
            r["rest_gesamt"] = json!(rest + (verbrauch - verbrauch_bis_stichtag));
            r["einheit"] = json!("tage");
            r["buchungen"] = json!(list);
            r["eintraege"] = json!(year_entries.iter().map(|e| json!({"id": e.0, "art": e.2, "tage": e.3, "grund": e.4, "created_at": e.5})).collect::<Vec<_>>());
            r["offene_ansprueche"] = json!(buckets.iter().map(|b| json!({"aus_urlaubsjahr": time::fmt_date(b.0), "tage": b.1, "verfall_am": expiry_of(b.0)})).collect::<Vec<_>>());
            r["naechster_verfall"] = json!(next_expiry);
            r["verfall_auto_aktiv"] = json!(s.urlaub_verfall_auto);
            r["erfassung_ab"] = json!(time::fmt_date(erfassung_ab));
            r["uebertrag_offen"] = json!(uebertrag_offen);
            // Einträge in Urlaubsjahren vor dem Erfassungsbeginn fließen nicht ein; die Verwaltung soll das sehen.
            r["eintraege_vor_erfassung"] = json!(entries.iter().filter(|e| time::parse_date(&e.1).map(|d| d < start_year).unwrap_or(false)).count());
            r["historie"] = json!(history);
            result = r;
            break;
        }
        if year >= start_year {
            history.push(year_json);
        }
        year = next_year_start(emp, year);
        if year > as_of {
            break;
        }
    }
    Ok(result)
}

// ---------------------------------------------------------------------------
// Gutstundenkonto
// ---------------------------------------------------------------------------

/// Standard-Topf des Mitarbeiters: Vollzeit → 307, Teilzeit → 311, außer explizit gesetzt.
pub async fn default_pot(db: &SqlitePool, emp: &Employee, at: NaiveDate) -> ApiResult<i64> {
    if let Some(t) = emp.gutstunden_topf {
        return Ok(t);
    }
    let s = settings::load(db).await?;
    let schedules = db::schedules_for(db, emp.id).await?;
    let weekly = db::schedule_at(&schedules, at).map(|w| w.week_model().weekly_minutes()).unwrap_or(0);
    Ok(if (weekly as f64) >= s.kv_wochenstunden * 60.0 - 0.5 { s.topf_vollzeit } else { s.topf_teilzeit })
}

/// Gutstundenkonto je Topf bis zum Stichtag. Verbrauch durch genehmigten Zeitausgleich wird
/// dynamisch aus den Abwesenheiten berechnet und dem Standard-Topf zugeordnet.
pub async fn credit_account(db: &SqlitePool, emp: &Employee, as_of: NaiveDate) -> ApiResult<Value> {
    let entries = sqlx::query_as::<_, (i64, String, i64, i64, String, Option<String>)>(
        "SELECT id, datum, topf, minuten, art, grund FROM credit_hours_entries WHERE employee_id = ? AND datum <= ? ORDER BY datum, id",
    )
    .bind(emp.id).bind(time::fmt_date(as_of)).fetch_all(db).await?;
    let mut pots: std::collections::BTreeMap<i64, (i64, i64)> = Default::default();
    for e in entries.iter().filter(|e| e.2 != 0) {
        let p = pots.entry(e.2).or_default();
        if e.3 >= 0 { p.0 += e.3 } else { p.1 += -e.3 }
    }
    let pot = default_pot(db, emp, as_of).await?;
    let eintritt = time::parse_date(&emp.eintritt).unwrap_or(as_of);
    let za = approved_between(db, emp.id, eintritt, as_of).await?;
    let schedules = db::schedules_for(db, emp.id).await?;
    let mut hol = Vec::new();
    for y in eintritt.year()..=as_of.year() {
        hol.extend(holidays::for_year(db, y).await?.into_iter().map(|h| h.date));
    }
    let mut za_min = 0i64;
    for a in za.iter().filter(|a| a.art == "zeitausgleich") {
        let f = time::parse_date(&a.von).unwrap();
        let t = time::parse_date(&a.bis).unwrap().min(as_of);
        let mut d = f;
        while d <= t {
            if let Some(s) = db::schedule_at(&schedules, d) {
                let target = s.week_model().target_for(d);
                if target > 0 && !hol.contains(&d) {
                    za_min += match a.einheit.as_str() {
                        "halber_tag" => (target / 2) as i64,
                        "stunden" => (a.wert.unwrap_or(0.0) * 60.0).round() as i64,
                        _ => target as i64,
                    };
                }
            }
            d += Duration::days(1);
        }
    }
    let p = pots.entry(pot).or_default();
    p.1 += za_min;
    Ok(json!({
        "standard_topf": pot,
        "toepfe": pots.iter().map(|(t, (auf, ab))| json!({"topf": t, "aufbau_min": auf, "abbau_min": ab, "saldo_min": auf - ab})).collect::<Vec<_>>(),
        "zeitausgleich_min": za_min,
        "eintraege": entries.iter().map(|e| json!({"id": e.0, "datum": e.1, "topf": e.2, "minuten": e.3, "art": e.4, "grund": e.5})).collect::<Vec<_>>(),
    }))
}

async fn account_own(State(state): State<AppState>, CurrentUser(user): CurrentUser) -> ApiResult<Json<Value>> {
    let today = time::today_local();
    Ok(Json(json!({
        "urlaub": vacation_account(&state.db, &user, today).await?,
        "gutstunden": credit_account(&state.db, &user, today).await?,
        "saldo_min": calc::saldo_until(&state.db, &user, today).await?,
    })))
}

#[derive(Deserialize)]
struct AsOfQuery {
    stichtag: Option<String>,
}

async fn vacation_admin(State(state): State<AppState>, AdminUser(_): AdminUser, Path(id): Path<i64>, Query(q): Query<AsOfQuery>) -> ApiResult<Json<Value>> {
    let emp = db::get_employee(&state.db, id).await?;
    let as_of = q.stichtag.as_deref().and_then(time::parse_date).unwrap_or_else(time::today_local);
    Ok(Json(vacation_account(&state.db, &emp, as_of).await?))
}

#[derive(Deserialize)]
struct VacationEntryReq {
    urlaubsjahr: String,
    art: String,
    tage: f64,
    grund: String,
}

async fn vacation_entry(State(state): State<AppState>, AdminUser(admin): AdminUser, Path(id): Path<i64>, Json(req): Json<VacationEntryReq>) -> ApiResult<Json<Value>> {
    db::get_employee(&state.db, id).await?;
    if !["anspruch", "uebertrag", "korrektur", "verfall"].contains(&req.art.as_str()) {
        return Err(bad("Art ungültig"));
    }
    time::parse_date(&req.urlaubsjahr).ok_or_else(|| bad("Urlaubsjahr (Startdatum) ungültig"))?;
    if req.grund.trim().is_empty() {
        return Err(bad("Begründung ist Pflicht"));
    }
    let r = sqlx::query("INSERT INTO vacation_entries (employee_id, urlaubsjahr, art, tage, grund, erfasst_von) VALUES (?,?,?,?,?,?)")
        .bind(id).bind(&req.urlaubsjahr).bind(&req.art).bind(req.tage).bind(req.grund.trim()).bind(admin.id)
        .execute(&state.db).await?;
    db::audit(&state.db, Some(admin.id), "urlaub_eintrag", Some(format!("employee:{id}")), None, Some(json!({"id": r.last_insert_rowid(), "art": req.art, "tage": req.tage, "urlaubsjahr": req.urlaubsjahr, "grund": req.grund}))).await?;
    Ok(Json(json!({"id": r.last_insert_rowid()})))
}

#[derive(Deserialize)]
pub struct OpeningReq {
    /// Resturlaub in Tagen zum Beginn der Zeiterfassung (0 = voll verbraucht)
    pub rest_tage: f64,
}

/// Wertebereich des Resturlaubs bei Erstanlage; wird auch vor dem Anlegen eines Mitarbeiters geprüft.
pub fn validate_opening_balance(rest_tage: f64) -> ApiResult<()> {
    if !rest_tage.is_finite() || rest_tage < 0.0 || rest_tage > 365.0 {
        return Err(bad("Resturlaub muss zwischen 0 und 365 Tagen liegen"));
    }
    Ok(())
}

/// Resturlaub bei Erstanlage: setzt den Stand zum Erfassungsbeginn („Zeiterfassung ab“) als Übertrag
/// des Vorjahres; liegt der Rest unter dem Anspruch des laufenden Urlaubsjahres, wird die Differenz
/// als Korrektur gebucht (Verbrauch vor Erfassungsbeginn). Ersetzt alle früheren Erstanlage-Buchungen
/// des Mitarbeiters – auch in anderen Urlaubsjahren, falls „Zeiterfassung ab“ inzwischen geändert wurde.
/// Läuft in der übergebenen Transaktion, damit die Anlage eines Mitarbeiters atomar bleibt.
pub async fn book_opening_balance(tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>, emp: &Employee, admin_id: i64, rest_tage: f64) -> ApiResult<Value> {
    validate_opening_balance(rest_tage)?;
    let eintritt = time::parse_date(&emp.eintritt).ok_or_else(|| bad("Eintritt ungültig"))?;
    let stichtag = calc::saldo_start(emp)?;
    let year = vacation_year_start(emp, stichtag);
    let year_end = next_year_start(emp, year) - Duration::days(1);
    let ys = time::fmt_date(year);
    let explicit: Option<f64> = sqlx::query_scalar(
        "SELECT tage FROM vacation_entries WHERE employee_id = ? AND urlaubsjahr = ? AND art = 'anspruch' AND erstanlage = 0 ORDER BY id DESC LIMIT 1",
    )
    .bind(emp.id).bind(&ys).fetch_optional(&mut **tx).await?;
    let anspruch = entitlement_for(emp, eintritt, year, year_end, explicit);
    let delta = rest_tage - anspruch;
    let grund = format!("Erstanlage: Resturlaub {} Tage zum {}", crate::reports::fmt_days(rest_tage), time::fmt_date_de(stichtag));
    sqlx::query("DELETE FROM vacation_entries WHERE employee_id = ? AND erstanlage = 1")
        .bind(emp.id).execute(&mut **tx).await?;
    sqlx::query("INSERT INTO vacation_entries (employee_id, urlaubsjahr, art, tage, grund, erfasst_von, erstanlage) VALUES (?,?,?,?,?,?,1)")
        .bind(emp.id).bind(&ys).bind("uebertrag").bind(delta.max(0.0)).bind(&grund).bind(admin_id)
        .execute(&mut **tx).await?;
    if delta < 0.0 {
        sqlx::query("INSERT INTO vacation_entries (employee_id, urlaubsjahr, art, tage, grund, erfasst_von, erstanlage) VALUES (?,?,?,?,?,?,1)")
            .bind(emp.id).bind(&ys).bind("korrektur").bind(delta).bind(&grund).bind(admin_id)
            .execute(&mut **tx).await?;
    }
    Ok(json!({"rest_tage": rest_tage, "urlaubsjahr": ys, "stichtag": time::fmt_date(stichtag), "anspruch": anspruch, "uebertrag": delta.max(0.0), "korrektur": delta.min(0.0)}))
}

pub async fn audit_opening_balance(db: &SqlitePool, emp: &Employee, admin_id: i64, booked: &Value) -> ApiResult<()> {
    db::audit(db, Some(admin_id), "urlaub_erstanlage", Some(format!("employee:{}", emp.id)), None,
        Some(json!({"rest_tage": booked["rest_tage"], "stichtag": booked["stichtag"], "urlaubsjahr": booked["urlaubsjahr"], "anspruch": booked["anspruch"]}))).await
}

/// Erstanlage als eigene Transaktion buchen und protokollieren.
pub async fn set_opening_balance(db: &SqlitePool, emp: &Employee, admin_id: i64, rest_tage: f64) -> ApiResult<Value> {
    let mut tx = db.begin().await?;
    let booked = book_opening_balance(&mut tx, emp, admin_id, rest_tage).await?;
    tx.commit().await?;
    audit_opening_balance(db, emp, admin_id, &booked).await?;
    Ok(booked)
}

async fn vacation_opening(State(state): State<AppState>, AdminUser(admin): AdminUser, Path(id): Path<i64>, Json(req): Json<OpeningReq>) -> ApiResult<Json<Value>> {
    let emp = db::get_employee(&state.db, id).await?;
    Ok(Json(set_opening_balance(&state.db, &emp, admin.id, req.rest_tage).await?))
}

async fn credit_admin(State(state): State<AppState>, AdminUser(_): AdminUser, Path(id): Path<i64>, Query(q): Query<AsOfQuery>) -> ApiResult<Json<Value>> {
    let emp = db::get_employee(&state.db, id).await?;
    let as_of = q.stichtag.as_deref().and_then(time::parse_date).unwrap_or_else(time::today_local);
    let mut v = credit_account(&state.db, &emp, as_of).await?;
    v["saldo_min"] = json!(calc::saldo_until(&state.db, &emp, as_of).await?);
    Ok(Json(v))
}

#[derive(Deserialize)]
struct CreditEntryReq {
    datum: String,
    topf: i64,
    minuten: i64,
    art: String,
    grund: String,
}

async fn credit_entry(State(state): State<AppState>, AdminUser(admin): AdminUser, Path(id): Path<i64>, Json(req): Json<CreditEntryReq>) -> ApiResult<Json<Value>> {
    db::get_employee(&state.db, id).await?;
    if !["korrektur", "auszahlung", "uebertrag", "saldo"].contains(&req.art.as_str()) {
        return Err(bad("Art ungültig (korrektur, auszahlung, uebertrag, saldo)"));
    }
    if (req.art == "saldo") != (req.topf == 0) {
        return Err(bad("Art 'saldo' gehört zu Topf 0 (Gleitzeitsaldo), alle anderen Arten zu einem NLZ-Topf"));
    }
    let d = time::parse_date(&req.datum).ok_or_else(|| bad("Datum ungültig"))?;
    calc::ensure_month_open(&state.db, id, d).await?;
    if req.grund.trim().is_empty() {
        return Err(bad("Begründung ist Pflicht"));
    }
    let r = sqlx::query("INSERT INTO credit_hours_entries (employee_id, datum, topf, minuten, art, grund, erfasst_von) VALUES (?,?,?,?,?,?,?)")
        .bind(id).bind(&req.datum).bind(req.topf).bind(req.minuten).bind(&req.art).bind(req.grund.trim()).bind(admin.id)
        .execute(&state.db).await?;
    db::audit(&state.db, Some(admin.id), "gutstunden_eintrag", Some(format!("employee:{id}")), None, Some(json!({"id": r.last_insert_rowid(), "topf": req.topf, "minuten": req.minuten, "art": req.art, "grund": req.grund}))).await?;
    Ok(Json(json!({"id": r.last_insert_rowid()})))
}
