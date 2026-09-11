//! BMD-Export (CSV für „Abrechnungen importieren“) und Periodenabschluss der Durchrechnung.

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::Response,
    routing::{get, post},
    Json, Router,
};
use chrono::{Datelike, Duration, NaiveDate};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use timecard_domain::{AbsenceKind, AbsenceUnit};

use crate::{
    absences,
    auth::AdminUser,
    calc,
    db::{self, Employee},
    error::{bad, ApiResult, AppError},
    holidays, settings, time, AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/export/preview", get(preview))
        .route("/export/bmd", post(create))
        .route("/export/runs", get(runs))
        .route("/export/runs/{id}/download", get(download))
        .route("/export/runs/{id}/preview", get(preview_file))
        .route("/export/periods", get(periods))
        .route("/export/period-close", post(period_close))
}

/// Eine Zeile der BMD-Schnittstelle. Spaltenreihenfolge laut Kundenschnittstelle plus ABM_DIVNLZID.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BmdRow {
    pub monat: u32,
    pub firma: String,
    pub ma: String,
    pub lohnart: String,
    pub menge: String,
    pub betrag: String,
    pub monat_a: String,
    pub nlz_k: String,
    pub nlz_v: String,
    pub nlz_b: String,
    pub nlz_ver: String,
    pub divnlz: String,
    /// Nur zur Anzeige
    pub mitarbeiter: String,
    pub beschreibung: String,
}

pub const HEADER: &str = "MONAT;FIRMA;MA;LOHNART;MENGE;BETRAG;MONAT_A;NLZ_K;NLZ_V;NLZ_B;NLZ_VER;ABM_DIVNLZID";

impl BmdRow {
    pub fn csv_line(&self) -> String {
        [
            self.monat.to_string(),
            self.firma.clone(),
            self.ma.clone(),
            self.lohnart.clone(),
            self.menge.clone(),
            self.betrag.clone(),
            self.monat_a.clone(),
            self.nlz_k.clone(),
            self.nlz_v.clone(),
            self.nlz_b.clone(),
            self.nlz_ver.clone(),
            self.divnlz.clone(),
        ]
        .join(";")
    }
}

/// Dezimalzahl mit Komma, ohne unnötige Nullen (10 → "10", 0,5 → "0,5", −15 → "-15").
fn dec(v: f64) -> String {
    let s = format!("{:.4}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    s.replace('.', ",")
}

fn de(d: NaiveDate) -> String {
    time::fmt_date_de(d)
}

/// Abrechnungsmonat = Monat der Übermittlung = Datenmonat + 1.
pub fn abrechnungsmonat(from: NaiveDate) -> u32 {
    if from.month() == 12 { 1 } else { from.month() + 1 }
}

/// Baut alle Exportzeilen für den Datenmonat.
pub async fn build_rows(db: &SqlitePool, monat: &str, verbuchung: u32) -> ApiResult<Vec<BmdRow>> {
    let (from, to) = calc::month_range(monat)?;
    let s = settings::load(db).await?;
    if s.bmd_firmennr.trim().is_empty() {
        return Err(bad("BMD-Firmennummer fehlt in den Einstellungen"));
    }
    let abm = abrechnungsmonat(from);
    let emps = sqlx::query_as::<_, Employee>("SELECT * FROM employees WHERE eintritt <= ? AND personalnr != '0' ORDER BY CAST(personalnr AS INTEGER)")
        .bind(time::fmt_date(to))
        .fetch_all(db)
        .await?;
    let mut hol = Vec::new();
    for y in from.year()..=to.year() {
        hol.extend(holidays::for_year(db, y).await?.into_iter().map(|h| h.date));
    }
    let mut rows = Vec::new();
    for e in emps {
        if e.austritt.as_deref().map(|a| a < time::fmt_date(from).as_str()).unwrap_or(false) {
            continue;
        }
        let schedules = db::schedules_for(db, e.id).await?;
        let base = |beschreibung: String| BmdRow {
            monat: abm,
            firma: s.bmd_firmennr.trim().to_string(),
            ma: e.personalnr.clone(),
            lohnart: String::new(),
            menge: String::new(),
            betrag: String::new(),
            monat_a: String::new(),
            nlz_k: String::new(),
            nlz_v: String::new(),
            nlz_b: String::new(),
            nlz_ver: String::new(),
            divnlz: String::new(),
            mitarbeiter: e.display_name(),
            beschreibung,
        };

        // Nichtleistungszeiten mit Beginn im Datenmonat
        let abs = sqlx::query_as::<_, absences::AbsenceRow>(
            "SELECT * FROM absences WHERE employee_id = ? AND status = 'genehmigt' AND von BETWEEN ? AND ? ORDER BY von",
        )
        .bind(e.id).bind(time::fmt_date(from)).bind(time::fmt_date(to))
        .fetch_all(db)
        .await?;
        for a in &abs {
            let Some(kind) = AbsenceKind::parse(&a.art) else { continue };
            let Some(typ) = kind.bmd_nlz_type() else { continue };
            let is_krank = matches!(kind, AbsenceKind::Krank | AbsenceKind::Arbeitsunfall | AbsenceKind::Freizeitunfall);
            if is_krank && !s.bmd_krank_exportieren {
                continue;
            }
            let von = time::parse_date(&a.von).unwrap();
            let bis = time::parse_date(&a.bis).unwrap();
            let unit = AbsenceUnit::parse(&a.einheit).unwrap_or(AbsenceUnit::Tag);
            let mut r = base(format!("{} {}–{}", kind.label(), de(von), de(bis)));
            r.nlz_k = format!("{}{:02}", verbuchung, typ);
            r.nlz_v = de(von);
            r.nlz_b = de(bis);
            if kind == AbsenceKind::Absonderung {
                r.divnlz = s.bmd_absonderung_divnlz.clone();
            }
            match unit {
                AbsenceUnit::Tag => {}
                AbsenceUnit::HalberTag => {
                    r.nlz_ver = "0,5".into();
                    r.beschreibung.push_str(" (halber Tag)");
                }
                AbsenceUnit::Stunden => {
                    let h = a.wert.unwrap_or(0.0);
                    let target = db::schedule_at(&schedules, von).map(|w| w.week_model().target_for(von)).unwrap_or(480);
                    r.menge = dec(h);
                    r.nlz_ver = if target > 0 { dec(h * 60.0 / target as f64) } else { String::new() };
                    r.beschreibung.push_str(&format!(" ({} h)", dec(h)));
                }
            }
            rows.push(r);
        }

        // Gutstunden je Topf: Aufbau (Periodenabschluss, Korrektur, Übertrag) minus Zeitausgleich im Monat
        let pot_default = absences::default_pot(db, &e, to).await?;
        let mut pots: std::collections::BTreeMap<i64, i64> = Default::default();
        let entries: Vec<(i64, i64)> = sqlx::query_as(
            "SELECT topf, minuten FROM credit_hours_entries WHERE employee_id = ? AND topf != 0 AND art IN ('periodenabschluss','korrektur','uebertrag') AND datum BETWEEN ? AND ?",
        )
        .bind(e.id).bind(time::fmt_date(from)).bind(time::fmt_date(to))
        .fetch_all(db)
        .await?;
        for (topf, min) in entries {
            *pots.entry(topf).or_default() += min;
        }
        let za = absences::approved_between(db, e.id, from, to).await?;
        for a in za.iter().filter(|a| a.art == "zeitausgleich") {
            let f = time::parse_date(&a.von).unwrap().max(from);
            let t = time::parse_date(&a.bis).unwrap().min(to);
            let mut d = f;
            while d <= t {
                if let Some(sch) = db::schedule_at(&schedules, d) {
                    let target = sch.week_model().target_for(d);
                    if target > 0 && !hol.contains(&d) {
                        let m = match a.einheit.as_str() {
                            "halber_tag" => (target / 2) as i64,
                            "stunden" => (a.wert.unwrap_or(0.0) * 60.0).round() as i64,
                            _ => target as i64,
                        };
                        *pots.entry(pot_default).or_default() -= m;
                    }
                }
                d += Duration::days(1);
            }
        }
        for (topf, min) in pots {
            if min == 0 {
                continue;
            }
            let mut r = base(format!("Gutstunden Topf {} {}", topf, if min > 0 { "Aufbau" } else { "Abbau" }));
            r.nlz_k = format!("{}{:02}", verbuchung, topf % 100);
            r.nlz_v = de(from);
            r.nlz_b = de(to);
            r.nlz_ver = dec(min as f64 / 60.0);
            rows.push(r);
        }
    }
    Ok(rows)
}

#[derive(Deserialize)]
struct MonthQuery {
    monat: String,
}

async fn preview(State(state): State<AppState>, AdminUser(_): AdminUser, Query(q): Query<MonthQuery>) -> ApiResult<Json<Value>> {
    let prior: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM export_runs WHERE datenmonat = ?").bind(&q.monat).fetch_one(&state.db).await?;
    let verbuchung = if prior == 0 { 3 } else { 2 };
    let rows = build_rows(&state.db, &q.monat, verbuchung).await?;
    Ok(Json(json!({ "verbuchungsart": verbuchung, "art": if prior == 0 { "voll" } else { "korrektur" }, "header": HEADER, "rows": rows })))
}

async fn create(State(state): State<AppState>, AdminUser(admin): AdminUser, Json(q): Json<MonthQuery>) -> ApiResult<Json<Value>> {
    let (from, to) = calc::month_range(&q.monat)?;
    if to >= time::today_local() {
        return Err(AppError::Conflict("Der Datenmonat ist noch nicht vorbei".into()));
    }
    let open: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM employees e WHERE e.aktiv = 1 AND e.personalnr != '0' AND e.eintritt <= ?
           AND NOT EXISTS (SELECT 1 FROM month_closures m WHERE m.employee_id = e.id AND m.monat = ?)",
    )
    .bind(time::fmt_date(to)).bind(&q.monat).fetch_one(&state.db).await?;
    if open > 0 {
        return Err(AppError::Conflict(format!("{open} Mitarbeiter ohne Monatsabschluss. Bitte zuerst alle Monate abschließen.")));
    }
    let prior: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM export_runs WHERE datenmonat = ?").bind(&q.monat).fetch_one(&state.db).await?;
    let (verbuchung, art) = if prior == 0 { (3, "voll") } else { (2, "korrektur") };
    let rows = build_rows(&state.db, &q.monat, verbuchung).await?;
    let s = settings::load(&state.db).await?;
    let mut text = String::new();
    if s.bmd_kopfzeile {
        text.push_str(HEADER);
        text.push_str("\r\n");
    }
    for r in &rows {
        text.push_str(&r.csv_line());
        text.push_str("\r\n");
    }
    let bytes: Vec<u8> = if s.bmd_zeichensatz == "utf-8" {
        text.into_bytes()
    } else {
        encoding_rs::WINDOWS_1252.encode(&text).0.into_owned()
    };
    let stamp = time::to_local(time::now_utc()).format("%Y%m%d%H%M%S");
    let name = format!("{stamp}_NLZ_{}_{}.csv", s.bmd_firmennr.trim(), q.monat);
    let dir = state.data_dir.join("exports").join("bmd");
    std::fs::create_dir_all(&dir).map_err(|e| anyhow::anyhow!(e))?;
    std::fs::write(dir.join(&name), &bytes).map_err(|e| anyhow::anyhow!(e))?;
    let rel = format!("exports/bmd/{name}");
    let hash = hex::encode(Sha256::digest(&bytes));
    let r = sqlx::query("INSERT INTO export_runs (datenmonat, abrechnungsmonat, art, datei, sha256, zeilen, erstellt_von, inhalt) VALUES (?,?,?,?,?,?,?,?)")
        .bind(&q.monat).bind(abrechnungsmonat(from) as i64).bind(art).bind(&rel).bind(&hash).bind(rows.len() as i64).bind(admin.id)
        .bind(serde_json::to_string(&rows).unwrap_or_default())
        .execute(&state.db).await?;
    db::audit(&state.db, Some(admin.id), "bmd_export", None, None, Some(json!({"id": r.last_insert_rowid(), "datei": rel, "zeilen": rows.len(), "art": art}))).await?;
    Ok(Json(json!({"id": r.last_insert_rowid(), "datei": name, "zeilen": rows.len(), "art": art, "sha256": hash})))
}

async fn runs(State(state): State<AppState>, AdminUser(_): AdminUser, Query(q): Query<MonthQuery>) -> ApiResult<Json<Vec<Value>>> {
    let rows: Vec<(i64, String, i64, String, String, String, i64, String)> = sqlx::query_as(
        "SELECT id, datenmonat, abrechnungsmonat, art, datei, sha256, zeilen, created_at FROM export_runs WHERE datenmonat = ? ORDER BY id DESC",
    )
    .bind(&q.monat).fetch_all(&state.db).await?;
    Ok(Json(rows.into_iter().map(|(id, dm, am, art, datei, sha, z, at)| json!({
        "id": id, "datenmonat": dm, "abrechnungsmonat": am, "art": art,
        "datei": std::path::Path::new(&datei).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or(datei.clone()),
        "sha256": sha, "zeilen": z, "created_at": at,
    })).collect()))
}

async fn run_file(db: &SqlitePool, id: i64) -> ApiResult<(String, String)> {
    let row: Option<(String, String)> = sqlx::query_as("SELECT datei, inhalt FROM export_runs WHERE id = ?").bind(id).fetch_optional(db).await?;
    row.ok_or(AppError::NotFound)
}

async fn download(State(state): State<AppState>, AdminUser(_): AdminUser, Path(id): Path<i64>) -> ApiResult<Response> {
    let (rel, _) = run_file(&state.db, id).await?;
    let bytes = std::fs::read(state.data_dir.join(&rel)).map_err(|e| anyhow::anyhow!("Datei nicht lesbar: {e}"))?;
    let name = std::path::Path::new(&rel).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/csv")
        .header(header::CONTENT_DISPOSITION, format!("attachment; filename=\"{name}\""))
        .body(Body::from(bytes))
        .unwrap())
}

async fn preview_file(State(state): State<AppState>, AdminUser(_): AdminUser, Path(id): Path<i64>) -> ApiResult<Json<Value>> {
    let (rel, inhalt) = run_file(&state.db, id).await?;
    let rows: Vec<BmdRow> = serde_json::from_str(&inhalt).unwrap_or_default();
    Ok(Json(json!({"datei": rel, "header": HEADER, "rows": rows})))
}

// ---------------------------------------------------------------------------
// Periodenabschluss der Durchrechnung
// ---------------------------------------------------------------------------

fn add_months(d: NaiveDate, months: i64) -> NaiveDate {
    let total = d.year() as i64 * 12 + d.month0() as i64 + months;
    let y = total.div_euclid(12) as i32;
    let m = total.rem_euclid(12) as u32 + 1;
    let last = NaiveDate::from_ymd_opt(if m == 12 { y + 1 } else { y }, if m == 12 { 1 } else { m + 1 }, 1).unwrap() - Duration::days(1);
    NaiveDate::from_ymd_opt(y, m, d.day().min(last.day())).unwrap()
}

/// Endet eine Durchrechnungsperiode des Mitarbeiters innerhalb [from, to]? Liefert das Enddatum.
pub fn period_end_in(emp: &Employee, from: NaiveDate, to: NaiveDate) -> Option<NaiveDate> {
    let start = time::parse_date(&emp.durchrechnung_start)?;
    let n = emp.durchrechnung_monate.max(1);
    let mut k = 1;
    loop {
        let end = add_months(start, n * k) - Duration::days(1);
        if end > to {
            return None;
        }
        if end >= from {
            return Some(end);
        }
        k += 1;
        if k > 1200 {
            return None;
        }
    }
}

/// Vorschlag für den Periodenabschluss eines Mitarbeiters zum Periodenende.
pub async fn period_proposal(db: &SqlitePool, emp: &Employee, end: NaiveDate) -> ApiResult<Value> {
    let saldo = calc::saldo_until(db, emp, end).await?;
    let schedules = db::schedules_for(db, emp.id).await?;
    let sch = db::schedule_at(&schedules, end);
    let max_plus = sch.and_then(|s| s.uebertrag_max_plus_min).map(|m| m as i32);
    let max_minus = sch.and_then(|s| s.uebertrag_max_minus_min).map(|m| m as i32);
    let transfer = match max_plus {
        Some(mp) => (saldo - mp).max(0),
        None => saldo.max(0),
    };
    let done: Option<i64> = sqlx::query_scalar("SELECT minuten FROM credit_hours_entries WHERE employee_id = ? AND art = 'periodenabschluss' AND datum = ?")
        .bind(emp.id).bind(time::fmt_date(end)).fetch_optional(db).await?;
    let topf = absences::default_pot(db, emp, end).await?;
    let mut hinweise = Vec::new();
    if let Some(mm) = max_minus {
        if saldo < -mm {
            hinweise.push(format!("Minussaldo {} unterschreitet die Übertragsgrenze von −{}", time::fmt_hm(saldo), time::fmt_hm(mm)));
        }
    }
    if saldo < 0 && max_minus.is_none() {
        hinweise.push("Minussaldo wird übertragen. Ein Abzug ist nur mit Vereinbarung zulässig.".into());
    }
    Ok(json!({
        "periode_ende": time::fmt_date(end),
        "saldo_min": saldo,
        "uebertrag_max_plus_min": max_plus,
        "uebertrag_max_minus_min": max_minus,
        "vorschlag_min": transfer,
        "topf": topf,
        "erledigt_min": done,
        "hinweise": hinweise,
    }))
}

async fn periods(State(state): State<AppState>, AdminUser(_): AdminUser, Query(q): Query<MonthQuery>) -> ApiResult<Json<Vec<Value>>> {
    let (from, to) = calc::month_range(&q.monat)?;
    let emps = sqlx::query_as::<_, Employee>("SELECT * FROM employees WHERE aktiv = 1 AND personalnr != '0' AND eintritt <= ? ORDER BY nachname, vorname")
        .bind(time::fmt_date(to)).fetch_all(&state.db).await?;
    let mut out = Vec::new();
    for e in emps {
        if let Some(end) = period_end_in(&e, from, to) {
            let mut p = period_proposal(&state.db, &e, end).await?;
            p["employee_id"] = json!(e.id);
            p["name"] = json!(e.display_name());
            out.push(p);
        }
    }
    Ok(Json(out))
}

#[derive(Deserialize)]
struct PeriodCloseReq {
    employee_id: i64,
    datum: String,
    minuten: i64,
    topf: Option<i64>,
}

async fn period_close(State(state): State<AppState>, AdminUser(admin): AdminUser, Json(req): Json<PeriodCloseReq>) -> ApiResult<Json<Value>> {
    let emp = db::get_employee(&state.db, req.employee_id).await?;
    let d = time::parse_date(&req.datum).ok_or_else(|| bad("Datum ungültig"))?;
    calc::ensure_month_open(&state.db, emp.id, d).await?;
    let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM credit_hours_entries WHERE employee_id = ? AND art = 'periodenabschluss' AND datum = ?")
        .bind(emp.id).bind(&req.datum).fetch_one(&state.db).await?;
    if exists > 0 {
        return Err(AppError::Conflict("Periode ist zu diesem Datum bereits abgeschlossen".into()));
    }
    let topf = match req.topf { Some(t) => t, None => absences::default_pot(&state.db, &emp, d).await? };
    sqlx::query("INSERT INTO credit_hours_entries (employee_id, datum, topf, minuten, art, grund, erfasst_von) VALUES (?,?,?,?,'periodenabschluss',?,?)")
        .bind(emp.id).bind(&req.datum).bind(topf).bind(req.minuten).bind(format!("Periodenabschluss zum {}", de(d))).bind(admin.id)
        .execute(&state.db).await?;
    db::audit(&state.db, Some(admin.id), "periodenabschluss", Some(format!("employee:{}", emp.id)), None, Some(json!({"datum": req.datum, "topf": topf, "minuten": req.minuten}))).await?;
    Ok(Json(json!({"ok": true})))
}
