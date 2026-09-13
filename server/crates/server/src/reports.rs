//! Monatsabschluss und PDF-Berichte (LaTeX).

use std::path::{Path, PathBuf};

use axum::{
    body::Body,
    extract::{Path as AxPath, Query, State},
    http::{header, StatusCode},
    response::Response,
    routing::{get, post},
    Json, Router,
};
use chrono::Datelike;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

use crate::{
    auth::{AdminUser, CurrentUser},
    calc::{self, MonthView},
    db::{self, Employee},
    error::{bad, ApiResult, AppError},
    settings::{self, Settings},
    time, AppState,
};

const TEMPLATE: &str = include_str!("../../../../latex/monatsbericht.tex.j2");
const TEMPLATE_URLAUB: &str = include_str!("../../../../latex/urlaubskartei.tex.j2");
const TEMPLATE_URLAUB_UEBERSICHT: &str = include_str!("../../../../latex/urlaubsuebersicht.tex.j2");
const TEMPLATE_JAHR: &str = include_str!("../../../../latex/jahresuebersicht.tex.j2");

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/reports/month-status", get(month_status))
        .route("/reports/close", post(close_one))
        .route("/reports/close-all", post(close_all))
        .route("/reports/reopen", post(reopen))
        .route("/reports/month/{id}/{monat}/pdf", get(month_pdf))
        .route("/reports/month/{monat}/zip", get(month_zip))
        .route("/reports/vacation/{id}/pdf", get(vacation_pdf))
        .route("/reports/vacation-overview/pdf", get(vacation_overview_pdf))
        .route("/reports/year", get(year_json))
        .route("/reports/year/{jahr}/pdf", get(year_pdf))
}

#[derive(Deserialize)]
struct YearQuery {
    jahr: i32,
}

async fn active_employees_in_year(db: &SqlitePool, jahr: i32) -> ApiResult<Vec<Employee>> {
    let emps = sqlx::query_as::<_, Employee>(
        "SELECT * FROM employees WHERE stempelt = 1 AND eintritt <= ? AND (austritt IS NULL OR austritt >= ?) ORDER BY nachname, vorname",
    )
    .bind(format!("{jahr}-12-31")).bind(format!("{jahr}-01-01"))
    .fetch_all(db).await?;
    Ok(emps)
}

async fn year_json(State(state): State<AppState>, AdminUser(_): AdminUser, Query(q): Query<YearQuery>) -> ApiResult<Json<Vec<calc::YearView>>> {
    let mut out = Vec::new();
    for e in active_employees_in_year(&state.db, q.jahr).await? {
        out.push(calc::year_view(&state.db, &e, q.jahr).await?);
    }
    Ok(Json(out))
}

fn signed_or_empty(min: i32) -> String {
    if min == 0 { "0:00".into() } else { signed(min) }
}

async fn year_pdf(State(state): State<AppState>, AdminUser(_): AdminUser, AxPath(jahr): AxPath<i32>) -> ApiResult<Response> {
    let s = settings::load(&state.db).await?;
    let mut rows = Vec::new();
    for e in active_employees_in_year(&state.db, jahr).await? {
        let y = calc::year_view(&state.db, &e, jahr).await?;
        rows.push(json!({
            "personalnr": tex(&e.personalnr),
            "name": tex(&e.display_name()),
            "start": signed(y.saldo_start_min),
            "monate": y.monate.iter().map(|m| json!({
                "diff": if m.soll_min == 0 && m.diff_min == 0 { String::new() } else { signed_or_empty(m.diff_min) },
                "neg": m.diff_min < 0,
                "geschlossen": m.geschlossen,
            })).collect::<Vec<_>>(),
            "soll": time::fmt_hm(y.soll_min),
            "ist": time::fmt_hm(y.ist_min + y.abwesenheit_min + y.feiertag_min),
            "diff": signed(y.diff_min),
            "diff_neg": y.diff_min < 0,
            "saldo": signed(y.saldo_ende_min),
            "saldo_neg": y.saldo_ende_min < 0,
            "urlaub": fmt_days(y.urlaub_rest),
        }));
    }
    let mut ctx = base_ctx(&state.data_dir, &s);
    ctx.insert("jahr".into(), json!(jahr));
    ctx.insert("rows".into(), json!(rows));
    let bytes = render_template(&state.data_dir, TEMPLATE_JAHR, &Value::Object(ctx)).await?;
    Ok(pdf_response(bytes, &format!("Jahresuebersicht_{jahr}.pdf")))
}

#[derive(Deserialize)]
struct MonthQuery {
    monat: String,
}

/// Prüfliste für den Monatsabschluss aller aktiven Mitarbeiter.
async fn month_status(State(state): State<AppState>, AdminUser(_): AdminUser, Query(q): Query<MonthQuery>) -> ApiResult<Json<Value>> {
    let (from, to) = calc::month_range(&q.monat)?;
    let emps = sqlx::query_as::<_, Employee>("SELECT * FROM employees WHERE aktiv = 1 AND stempelt = 1 AND eintritt <= ? ORDER BY nachname, vorname")
        .bind(time::fmt_date(to))
        .fetch_all(&state.db)
        .await?;
    let mut rows = Vec::new();
    for e in emps {
        if e.austritt.as_deref().map(|a| a < time::fmt_date(from).as_str()).unwrap_or(false) {
            continue;
        }
        let mv = calc::month_view(&state.db, &e, &q.monat).await?;
        let blockers = blockers_for(&state.db, &e, &mv).await?;
        rows.push(json!({
            "employee_id": e.id,
            "name": e.display_name(),
            "personalnr": e.personalnr,
            "soll_min": mv.soll_min,
            "ist_min": mv.ist_min + mv.abwesenheit_min + mv.feiertag_min,
            "diff_min": mv.diff_min,
            "saldo_ende_min": mv.saldo_ende_min,
            "warnungen": mv.warnungen,
            "blocker": blockers,
            "geschlossen": mv.geschlossen.as_ref().and_then(|g| g["at"].as_str().map(String::from)),
        }));
    }
    let abrechnungsmonat = if from.month() == 12 { 1 } else { from.month() + 1 };
    Ok(Json(json!({ "monat": q.monat, "abrechnungsmonat": abrechnungsmonat, "rows": rows })))
}

async fn blockers_for(db: &SqlitePool, e: &Employee, mv: &MonthView) -> ApiResult<Vec<String>> {
    let mut b = Vec::new();
    if mv.days.iter().any(|d| d.result.open_shift) {
        b.push("offene Stempelung".into());
    }
    let (from, to) = calc::month_range(&mv.monat)?;
    let open: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM absences WHERE employee_id = ? AND status = 'beantragt' AND von <= ? AND bis >= ?")
        .bind(e.id).bind(time::fmt_date(to)).bind(time::fmt_date(from)).fetch_one(db).await?;
    if open > 0 {
        b.push(format!("{open} offene Anträge"));
    }
    let open_pr = crate::punch_requests::open_count(db, e.id, &time::fmt_date(from), &time::fmt_date(to)).await?;
    if open_pr > 0 {
        b.push(format!("{open_pr} offene Korrekturanträge"));
    }
    let schedules = db::schedules_for(db, e.id).await?;
    if db::schedule_at(&schedules, to).is_none() {
        b.push("kein Wochenmodell".into());
    }
    if mv.days.iter().any(|d| d.result.warnings.iter().any(|w| matches!(w, timecard_domain::Warning::InvalidSequence { .. }))) {
        b.push("ungültige Stempelfolge".into());
    }
    if to >= time::today_local() {
        b.push("Monat noch nicht vorbei".into());
    }
    Ok(b)
}

#[derive(Deserialize)]
struct CloseReq {
    employee_id: i64,
    monat: String,
}

async fn close_one(State(state): State<AppState>, AdminUser(admin): AdminUser, Json(req): Json<CloseReq>) -> ApiResult<Json<Value>> {
    let emp = db::get_employee(&state.db, req.employee_id).await?;
    let r = close_month(&state, &admin, &emp, &req.monat).await?;
    Ok(Json(r))
}

async fn close_month(state: &AppState, admin: &Employee, emp: &Employee, monat: &str) -> ApiResult<Value> {
    let mv = calc::month_view(&state.db, emp, monat).await?;
    if mv.geschlossen.is_some() {
        return Err(AppError::Conflict("Monat ist bereits abgeschlossen".into()));
    }
    let blockers = blockers_for(&state.db, emp, &mv).await?;
    if !blockers.is_empty() {
        return Err(AppError::Conflict(format!("Abschluss nicht möglich: {}", blockers.join(", "))));
    }
    let s = settings::load(&state.db).await?;
    let pdf = render_month_pdf(&state.db, &state.data_dir, &s, emp, &mv, false).await?;
    let dir = state.data_dir.join("exports").join(&monat[..4]).join(monat);
    std::fs::create_dir_all(&dir).map_err(|e| anyhow::anyhow!(e))?;
    let file = dir.join(format!("{}_{}_{}.pdf", monat, emp.personalnr, safe_name(&emp.display_name())));
    std::fs::write(&file, &pdf).map_err(|e| anyhow::anyhow!(e))?;
    let hash = hex::encode(Sha256::digest(&pdf));
    let rel = file.strip_prefix(&*state.data_dir).unwrap_or(&file).to_string_lossy().replace('\\', "/");
    sqlx::query("INSERT INTO month_closures (employee_id, monat, geschlossen_at, geschlossen_von, pdf_pfad, pdf_sha256, saldo_ende_min) VALUES (?,?,?,?,?,?,?)")
        .bind(emp.id).bind(monat).bind(time::fmt_utc(time::now_utc())).bind(admin.id).bind(&rel).bind(&hash).bind(mv.saldo_ende_min)
        .execute(&state.db).await?;
    db::audit(&state.db, Some(admin.id), "monat_abgeschlossen", Some(format!("employee:{}", emp.id)), None, Some(json!({"monat": monat, "pdf": rel, "sha256": hash}))).await?;
    Ok(json!({"ok": true, "pdf": rel, "sha256": hash}))
}

#[derive(Deserialize)]
struct CloseAllReq {
    monat: String,
}

async fn close_all(State(state): State<AppState>, AdminUser(admin): AdminUser, Json(req): Json<CloseAllReq>) -> ApiResult<Json<Value>> {
    let (from, to) = calc::month_range(&req.monat)?;
    let emps = sqlx::query_as::<_, Employee>("SELECT * FROM employees WHERE aktiv = 1 AND stempelt = 1 AND eintritt <= ?")
        .bind(time::fmt_date(to)).fetch_all(&state.db).await?;
    let mut done = 0;
    let mut skipped = Vec::new();
    for e in emps {
        if e.austritt.as_deref().map(|a| a < time::fmt_date(from).as_str()).unwrap_or(false) {
            continue;
        }
        match close_month(&state, &admin, &e, &req.monat).await {
            Ok(_) => done += 1,
            Err(AppError::Conflict(m)) => skipped.push(format!("{}: {m}", e.display_name())),
            Err(other) => return Err(other),
        }
    }
    Ok(Json(json!({"geschlossen": done, "uebersprungen": skipped})))
}

#[derive(Deserialize)]
struct ReopenReq {
    employee_id: i64,
    monat: String,
    grund: String,
}

async fn reopen(State(state): State<AppState>, AdminUser(admin): AdminUser, Json(req): Json<ReopenReq>) -> ApiResult<Json<Value>> {
    if req.grund.trim().is_empty() {
        return Err(bad("Begründung ist Pflicht"));
    }
    let r = sqlx::query("DELETE FROM month_closures WHERE employee_id = ? AND monat = ?")
        .bind(req.employee_id).bind(&req.monat).execute(&state.db).await?;
    if r.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    // Spätere Zwischenstände könnten sich durch die Korrektur ändern: verwerfen, Sperre bleibt.
    sqlx::query("UPDATE month_closures SET saldo_ende_min = NULL WHERE employee_id = ? AND monat > ?")
        .bind(req.employee_id).bind(&req.monat).execute(&state.db).await?;
    db::audit(&state.db, Some(admin.id), "monatsabschluss_aufgehoben", Some(format!("employee:{}", req.employee_id)), None, Some(json!({"monat": req.monat, "grund": req.grund}))).await?;
    Ok(Json(json!({"ok": true})))
}

#[derive(Deserialize)]
struct PdfQuery {
    vorschau: Option<String>,
}

/// PDF eines Monats: abgeschlossen → gespeicherte Datei; sonst Vorschau (nur Admin oder eigener Monat).
async fn month_pdf(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    AxPath((id, monat)): AxPath<(i64, String)>,
    Query(q): Query<PdfQuery>,
) -> ApiResult<Response> {
    if !user.is_admin() && user.id != id {
        return Err(AppError::Forbidden);
    }
    let emp = db::get_employee(&state.db, id).await?;
    let closed: Option<(Option<String>,)> = sqlx::query_as("SELECT pdf_pfad FROM month_closures WHERE employee_id = ? AND monat = ?")
        .bind(id).bind(&monat).fetch_optional(&state.db).await?;
    let filename = format!("Zeitaufzeichnung_{}_{}.pdf", monat, safe_name(&emp.display_name()));
    if let Some((Some(rel),)) = closed {
        let bytes = std::fs::read(state.data_dir.join(&rel)).map_err(|e| anyhow::anyhow!("PDF nicht lesbar: {e}"))?;
        return Ok(pdf_response(bytes, &filename));
    }
    if q.vorschau.is_none() && !user.is_admin() {
        return Err(AppError::NotFound);
    }
    let mv = calc::month_view(&state.db, &emp, &monat).await?;
    let s = settings::load(&state.db).await?;
    let bytes = render_month_pdf(&state.db, &state.data_dir, &s, &emp, &mv, true).await?;
    Ok(pdf_response(bytes, &format!("VORSCHAU_{filename}")))
}

fn pdf_response(bytes: Vec<u8>, filename: &str) -> Response {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/pdf")
        .header(header::CONTENT_DISPOSITION, format!("inline; filename=\"{filename}\""))
        .body(Body::from(bytes))
        .unwrap()
}

async fn month_zip(State(state): State<AppState>, AdminUser(_): AdminUser, AxPath(monat): AxPath<String>) -> ApiResult<Response> {
    calc::month_range(&monat)?;
    let rows: Vec<(String,)> = sqlx::query_as("SELECT pdf_pfad FROM month_closures WHERE monat = ? AND pdf_pfad IS NOT NULL ORDER BY employee_id")
        .bind(&monat).fetch_all(&state.db).await?;
    if rows.is_empty() {
        return Err(AppError::NotFound);
    }
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut buf);
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (rel,) in rows {
            let p = state.data_dir.join(&rel);
            if let Ok(bytes) = std::fs::read(&p) {
                let name = Path::new(&rel).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or(rel.clone());
                zip.start_file(name, opts).map_err(|e| anyhow::anyhow!(e))?;
                std::io::Write::write_all(&mut zip, &bytes).map_err(|e| anyhow::anyhow!(e))?;
            }
        }
        zip.finish().map_err(|e| anyhow::anyhow!(e))?;
    }
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/zip")
        .header(header::CONTENT_DISPOSITION, format!("attachment; filename=\"Zeitaufzeichnungen_{monat}.zip\""))
        .body(Body::from(buf.into_inner()))
        .unwrap())
}

pub fn safe_name(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'ä' => "ae".to_string(), 'ö' => "oe".to_string(), 'ü' => "ue".to_string(),
            'Ä' => "Ae".to_string(), 'Ö' => "Oe".to_string(), 'Ü' => "Ue".to_string(), 'ß' => "ss".to_string(),
            c if c.is_ascii_alphanumeric() => c.to_string(),
            _ => "_".to_string(),
        })
        .collect()
}

// ---------------------------------------------------------------------------
// LaTeX
// ---------------------------------------------------------------------------

/// Escaped Text für LaTeX.
pub fn tex(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\textbackslash{}"),
            '&' | '%' | '$' | '#' | '_' | '{' | '}' => { out.push('\\'); out.push(c); }
            '~' => out.push_str("\\textasciitilde{}"),
            '^' => out.push_str("\\textasciicircum{}"),
            '\n' => out.push(' '),
            _ => out.push(c),
        }
    }
    out
}

fn hm_or_empty(min: i32) -> String {
    if min == 0 { String::new() } else { time::fmt_hm(min) }
}

fn signed(min: i32) -> String {
    if min > 0 { format!("+{}", time::fmt_hm(min)) } else { time::fmt_hm(min) }
}

pub async fn render_month_pdf(db: &SqlitePool, data_dir: &Path, s: &Settings, emp: &Employee, mv: &MonthView, vorschau: bool) -> ApiResult<Vec<u8>> {
    let (from, to) = calc::month_range(&mv.monat)?;
    let labels = schedule_labels(db, emp, to).await?;
    let mut days = Vec::new();
    for d in &mv.days {
        let weekend = d.weekday == "Sa" || d.weekday == "So";
        // Der Nachweis enthält keine Hinweise: AZG-Prüfungen, Nacherfassungen und Korrekturen
        // sind intern und bleiben der Monatsübersicht, dem Abschluss und dem Protokoll vorbehalten.
        let abw: Vec<String> = d
            .absences
            .iter()
            .map(|a| if a.einheit == "tag" { a.label.clone() } else { format!("{} {}", a.label, time::fmt_hm(a.minutes)) })
            .chain(d.holiday_name.iter().map(|h| format!("Feiertag: {h}")))
            .collect();
        let diff_shown = !(d.result.target_min == 0 && d.result.diff_min == 0) && !d.future;
        days.push(json!({
            "datum": d.result.date.format("%d.%m.").to_string(),
            "tag": d.weekday,
            "weekend": weekend,
            "holiday": d.holiday_name.is_some(),
            "soll": hm_or_empty(d.result.target_min),
            "kommen": d.result.first_in.map(|t| t.format("%H:%M").to_string()).unwrap_or_default(),
            "gehen": d.result.last_out.map(|t| t.format("%H:%M").to_string()).unwrap_or_else(|| if d.result.open_shift { "offen".into() } else { String::new() }),
            "pause": hm_or_empty(d.result.break_min),
            "ist": hm_or_empty(d.result.worked_min),
            "abwesenheit": tex(&abw.join(", ")),
            "diff": if diff_shown { signed(d.result.diff_min) } else { String::new() },
            "diff_pos": diff_shown && d.result.diff_min > 0,
            "diff_neg": diff_shown && d.result.diff_min < 0,
        }));
    }
    let transferred = mv.saldo_start_min + mv.diff_min - mv.saldo_ende_min;
    let saldo_color = |min: i32| if min > 0 { "ok" } else if min < 0 { "err" } else { "ink" };
    let kpis = [
        ("Soll", time::fmt_hm(mv.soll_min), "ink"),
        ("Ist gesamt", time::fmt_hm(mv.ist_min), "ink"),
        ("Differenz", signed(mv.diff_min), saldo_color(mv.diff_min)),
        ("Saldo Beginn", signed(mv.saldo_start_min), saldo_color(mv.saldo_start_min)),
        ("Saldo Ende", signed(mv.saldo_ende_min), saldo_color(mv.saldo_ende_min)),
        ("Resturlaub", format!("{} Tage", fmt_days(mv.urlaub["rest"].as_f64().unwrap_or(0.0))), "ink"),
    ]
    .into_iter()
    .map(|(label, value, color)| json!({ "label": label, "value": value, "color": color }))
    .collect::<Vec<_>>();
    // Kompakte Kontenzeile unter der Tabelle: Nichtleistungszeit je Art, Feiertage, Übertrag, Gutstunden.
    let mut konten: Vec<String> = mv
        .abwesenheit_nach_art
        .iter()
        .map(|a| {
            format!(
                "{} {} Tage / {}",
                a["label"].as_str().unwrap_or(""),
                fmt_days(a["tage"].as_f64().unwrap_or(0.0)),
                time::fmt_hm(a["minuten"].as_i64().unwrap_or(0) as i32)
            )
        })
        .collect();
    if mv.feiertag_min != 0 {
        konten.push(format!("Feiertagsausfall {}", time::fmt_hm(mv.feiertag_min)));
    }
    if transferred != 0 {
        konten.push(format!("In Gutstunden übertragen {}", time::fmt_hm(transferred)));
    }
    for p in mv.gutstunden["toepfe"].as_array().into_iter().flatten() {
        konten.push(format!("Gutstunden Topf {} {}", p["topf"], signed(p["saldo_min"].as_i64().unwrap_or(0) as i32)));
    }
    let konten = if konten.is_empty() { None } else { Some(tex(&format!("Bezahlte Nichtleistungszeit {} · {}", time::fmt_hm(mv.abwesenheit_min), konten.join(" · ")))) };
    let mut ctx = base_ctx(data_dir, s);
    ctx.extend(
        json!({
            "mitarbeiter": tex(&emp.display_name()),
            "personalnr": tex(&emp.personalnr),
            "monat": &mv.monat,
            "monat_label": tex(&month_label_de(&mv.monat)),
            "vorschau": vorschau,
            "hash": &data_hash(mv)[..12],
            "wochenmodell": tex(&labels.wochenmodell),
            "wochenstunden": tex(&labels.wochenstunden),
            "durchrechnung": tex(&format!("{} Monate ab {}", emp.durchrechnung_monate, time::fmt_date_de(time::parse_date(&emp.durchrechnung_start).unwrap_or(from)))),
            "gleitzeit": labels.gleitzeit.as_deref().map(tex),
            "kpis": kpis,
            "days": days,
            "soll": time::fmt_hm(mv.soll_min),
            "ist": time::fmt_hm(mv.ist_min),
            "pause": hm_or_empty(mv.pause_min),
            "abwesenheit": hm_or_empty(mv.abwesenheit_min),
            "diff": signed(mv.diff_min),
            "diff_pos": mv.diff_min > 0,
            "diff_neg": mv.diff_min < 0,
            "konten": konten,
        })
        .as_object()
        .cloned()
        .unwrap_or_default(),
    );
    render_template(data_dir, TEMPLATE, &Value::Object(ctx)).await
}

async fn render_template(data_dir: &Path, template: &str, ctx: &Value) -> ApiResult<Vec<u8>> {
    // Eigenes Arbeitsverzeichnis je Lauf; das Logo wird hineinkopiert und nur mit seinem Dateinamen
    // referenziert, damit weder relative Datenverzeichnisse noch Leerzeichen im Pfad LaTeX stören.
    let dir = data_dir.join("tmp").join(format!("tex-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).map_err(|e| anyhow::anyhow!(e))?;
    let mut ctx = ctx.clone();
    if let Some(src) = ctx["logo"].as_str().map(PathBuf::from) {
        let name = format!("logo.{}", src.extension().and_then(|e| e.to_str()).unwrap_or("png"));
        match std::fs::copy(&src, dir.join(&name)) {
            Ok(_) => ctx["logo"] = json!(name),
            Err(e) => {
                tracing::warn!("Logo {} nicht kopierbar, PDF ohne Logo: {e}", src.display());
                ctx["logo"] = Value::Null;
            }
        }
    }
    let mut env = minijinja::Environment::new();
    env.set_auto_escape_callback(|_| minijinja::AutoEscape::None);
    env.add_template("m", template).map_err(|e| anyhow::anyhow!("Template: {e}"))?;
    let tex_src = env.get_template("m").unwrap().render(&ctx).map_err(|e| anyhow::anyhow!("Template render: {e}"))?;
    compile_latex(&dir, &tex_src).await
}

fn logo_for(data_dir: &Path, s: &Settings) -> Option<String> {
    match &s.logo_data_url {
        Some(url) if url.starts_with("data:image/png") || url.starts_with("data:image/jpeg") => {
            decode_logo(data_dir, url).ok().map(|p| p.to_string_lossy().replace('\\', "/"))
        }
        _ => None,
    }
}

fn base_ctx(data_dir: &Path, s: &Settings) -> serde_json::Map<String, Value> {
    let mut m = serde_json::Map::new();
    m.insert("primary_hex".into(), json!(s.primaerfarbe.trim_start_matches('#').to_uppercase()));
    m.insert("firmenname".into(), json!(tex(&s.firmenname)));
    m.insert("fusszeile".into(), json!(tex(&s.fusszeile)));
    m.insert("erstellt_am".into(), json!(time::to_local(time::now_utc()).format("%d.%m.%Y %H:%M").to_string()));
    m.insert("logo".into(), json!(logo_for(data_dir, s)));
    m.insert("initial".into(), json!(tex(&s.firmenname.trim().chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default())));
    m.insert("unterschrift_1".into(), json!(tex(&s.unterschrift_1)));
    m.insert("unterschrift_2".into(), json!(tex(&s.unterschrift_2)));
    m
}

fn de_opt(v: &Value) -> String {
    v.as_str().and_then(time::parse_date).map(time::fmt_date_de).unwrap_or_default()
}

fn year_opt(v: &Value) -> String {
    v.as_str().and_then(time::parse_date).map(|d| d.format("%Y").to_string()).unwrap_or_default()
}

fn vacation_row(acct: &Value) -> serde_json::Map<String, Value> {
    let f = |k: &str| fmt_days(acct[k].as_f64().unwrap_or(0.0));
    let verfall = acct["verfall_manuell"].as_f64().unwrap_or(0.0) + acct["verfall_auto"].as_f64().unwrap_or(0.0);
    let nv = acct["naechster_verfall"].as_object().map(|n| {
        format!("{} Tage aus {} können ab {} verjähren", fmt_days(n["tage"].as_f64().unwrap_or(0.0)), year_opt(&n["aus_urlaubsjahr"]), de_opt(&n["am"]))
    });
    let mut m = serde_json::Map::new();
    m.insert("jahr".into(), json!(format!("{} – {}", de_opt(&acct["urlaubsjahr_von"]), de_opt(&acct["urlaubsjahr_bis"]))));
    m.insert("anspruch".into(), json!(f("anspruch")));
    m.insert("aliquot".into(), json!(acct["anspruch_aliquot"].as_bool().unwrap_or(false)));
    m.insert("uebertrag".into(), json!(f("uebertrag")));
    m.insert("korrektur".into(), json!(if acct["korrektur"].as_f64().unwrap_or(0.0) != 0.0 { Some(f("korrektur")) } else { None }));
    m.insert("verfall".into(), json!(if verfall != 0.0 { Some(fmt_days(verfall)) } else { None }));
    m.insert("verbraucht".into(), json!(f("verbrauch_bis_stichtag")));
    m.insert("geplant".into(), json!(f("geplant")));
    m.insert("rest".into(), json!(f("rest")));
    m.insert("rest_gesamt".into(), json!(fmt_days(acct["rest"].as_f64().unwrap_or(0.0) + acct["geplant"].as_f64().unwrap_or(0.0))));
    m.insert("naechster_verfall".into(), json!(nv.map(|t| tex(&t))));
    m
}

/// Bezeichnung des Urlaubsjahres: Kalenderjahr, sonst der Zeitraum.
fn vacation_year_label(acct: &Value) -> String {
    match acct["urlaubsjahr_von"].as_str().and_then(time::parse_date) {
        Some(d) if d.month() == 1 && d.day() == 1 => d.format("%Y").to_string(),
        Some(_) => format!("{} – {}", de_opt(&acct["urlaubsjahr_von"]), de_opt(&acct["urlaubsjahr_bis"])),
        None => String::new(),
    }
}

fn signed_days(d: f64) -> String {
    if d > 0.0 { format!("+{}", fmt_days(d)) } else if d < 0.0 { format!("−{}", fmt_days(-d)) } else { "–".into() }
}

/// Buchungen der Urlaubskartei als Kontoauszug mit laufendem Rest: Übertrag und Anspruch zu Jahresbeginn,
/// danach Einträge der Verwaltung und Verbrauch chronologisch.
fn vacation_ledger(acct: &Value, as_of: chrono::NaiveDate) -> Vec<Value> {
    let arr = |k: &str| acct[k].as_array().cloned().unwrap_or_default();
    let year_start = acct["urlaubsjahr_von"].as_str().and_then(time::parse_date).unwrap_or(as_of);
    let mut rows: Vec<(chrono::NaiveDate, u8, String, String, f64, String)> = Vec::new();
    let uebertrag = acct["uebertrag"].as_f64().unwrap_or(0.0);
    if uebertrag != 0.0 {
        rows.push((year_start, 0, "Übertrag".into(), "aus Vorjahren".into(), uebertrag, String::new()));
    }
    let anspruch = acct["anspruch"].as_f64().unwrap_or(0.0);
    let aliquot = acct["anspruch_aliquot"].as_bool().unwrap_or(false);
    rows.push((year_start, 1, "Anspruch".into(), format!("Urlaubsjahr {}", vacation_year_label(acct)), anspruch, if aliquot { "aliquot nach Eintritt".into() } else { String::new() }));
    for e in arr("eintraege") {
        let art = e["art"].as_str().unwrap_or("");
        if art == "anspruch" || art == "uebertrag" {
            continue; // bereits in Anspruch/Übertrag enthalten
        }
        let datum = e["created_at"].as_str().and_then(|c| time::parse_date(&c[..c.len().min(10)])).unwrap_or(year_start);
        let label = match art { "korrektur" => "Korrektur", "verfall" => "Verfall", other => other };
        rows.push((datum, 2, label.into(), e["grund"].as_str().unwrap_or("").into(), e["tage"].as_f64().unwrap_or(0.0), String::new()));
    }
    for b in arr("buchungen") {
        let von = b["von"].as_str().and_then(time::parse_date).unwrap_or(year_start);
        let bis = b["bis"].as_str().and_then(time::parse_date).unwrap_or(von);
        let mut bezug = if von == bis { time::fmt_date_de(von) } else { format!("{} – {}", time::fmt_date_de(von), time::fmt_date_de(bis)) };
        if b["einheit"].as_str().unwrap_or("tag") != "tag" {
            bezug.push_str(&format!(" ({})", b["einheit"].as_str().unwrap_or("")));
        }
        let mut bem: Vec<String> = Vec::new();
        if von > as_of {
            bem.push("geplant, noch nicht angetreten".into());
        }
        if let Some(k) = b["kommentar"].as_str().filter(|k| !k.trim().is_empty()) {
            bem.push(k.trim().into());
        }
        rows.push((von, 3, b["label"].as_str().unwrap_or("Urlaub").into(), bezug, -b["tage"].as_f64().unwrap_or(0.0), bem.join(" · ")));
    }
    rows.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
    let mut rest = 0.0;
    rows.into_iter()
        .map(|(datum, _, art, bezug, tage, bem)| {
            rest += tage;
            json!({
                "datum": time::fmt_date_de(datum),
                "art": tex(&art),
                "bezug": tex(&bezug),
                "tage": signed_days(tage),
                "neg": tage < 0.0,
                "null": tage == 0.0,
                "rest": fmt_days(rest),
                "bem": tex(&bem),
            })
        })
        .collect()
}

#[derive(Deserialize)]
struct StichtagQuery {
    stichtag: Option<String>,
}

async fn vacation_pdf(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    AxPath(id): AxPath<i64>,
    Query(q): Query<StichtagQuery>,
) -> ApiResult<Response> {
    if !user.is_admin() && user.id != id {
        return Err(AppError::Forbidden);
    }
    let emp = db::get_employee(&state.db, id).await?;
    let as_of = q.stichtag.as_deref().and_then(time::parse_date).unwrap_or_else(time::today_local);
    let acct = crate::absences::vacation_account(&state.db, &emp, as_of).await?;
    let s = settings::load(&state.db).await?;
    let mut ctx = base_ctx(&state.data_dir, &s);
    for (k, v) in vacation_row(&acct) {
        ctx.insert(k, v);
    }
    ctx.insert("mitarbeiter".into(), json!(tex(&emp.display_name())));
    ctx.insert("personalnr".into(), json!(tex(&emp.personalnr)));
    ctx.insert("eintritt".into(), json!(time::parse_date(&emp.eintritt).map(time::fmt_date_de).unwrap_or_default()));
    ctx.insert("anspruch_jahr".into(), json!(fmt_days(emp.urlaubsanspruch_tage)));
    ctx.insert("stichtag".into(), json!(time::fmt_date_de(as_of)));
    ctx.insert("urlaubsjahr".into(), json!(tex(&vacation_year_label(&acct))));
    ctx.insert("jahr_von_kurz".into(), json!(year_opt(&acct["urlaubsjahr_von"])));
    let arr = |k: &str| acct[k].as_array().cloned().unwrap_or_default();
    ctx.insert("offene".into(), json!(arr("offene_ansprueche").iter().map(|b| json!({
        "jahr": year_opt(&b["aus_urlaubsjahr"]),
        "tage": fmt_days(b["tage"].as_f64().unwrap_or(0.0)),
        "verfall_am": de_opt(&b["verfall_am"]),
    })).collect::<Vec<_>>()));
    ctx.insert("buchungen".into(), json!(vacation_ledger(&acct, as_of)));
    ctx.insert("historie".into(), json!(arr("historie").iter().map(|h| json!({
        "jahr": year_opt(&h["urlaubsjahr_von"]),
        "anspruch": fmt_days(h["anspruch"].as_f64().unwrap_or(0.0)),
        "uebertrag": fmt_days(h["uebertrag"].as_f64().unwrap_or(0.0)),
        "korrektur": fmt_days(h["korrektur"].as_f64().unwrap_or(0.0)),
        "verbrauch": fmt_days(h["verbrauch"].as_f64().unwrap_or(0.0)),
        "verfall": fmt_days(h["verfall_manuell"].as_f64().unwrap_or(0.0) + h["verfall_auto"].as_f64().unwrap_or(0.0)),
        "rest": fmt_days(h["rest"].as_f64().unwrap_or(0.0)),
    })).collect::<Vec<_>>()));
    let bytes = render_template(&state.data_dir, TEMPLATE_URLAUB, &Value::Object(ctx)).await?;
    Ok(pdf_response(bytes, &format!("Urlaubskartei_{}_{}.pdf", as_of.format("%Y"), safe_name(&emp.display_name()))))
}

async fn vacation_overview_pdf(State(state): State<AppState>, AdminUser(_): AdminUser, Query(q): Query<StichtagQuery>) -> ApiResult<Response> {
    let as_of = q.stichtag.as_deref().and_then(time::parse_date).unwrap_or_else(time::today_local);
    let emps = sqlx::query_as::<_, Employee>("SELECT * FROM employees WHERE aktiv = 1 AND stempelt = 1 ORDER BY nachname, vorname")
        .fetch_all(&state.db).await?;
    let mut rows = Vec::new();
    for e in emps {
        let acct = crate::absences::vacation_account(&state.db, &e, as_of).await?;
        let mut r = vacation_row(&acct);
        r.insert("personalnr".into(), json!(tex(&e.personalnr)));
        r.insert("name".into(), json!(tex(&e.display_name())));
        let verfall = r.get("verfall").and_then(|v| v.as_str().map(String::from)).unwrap_or_else(|| "0".into());
        r.insert("verfall".into(), json!(verfall));
        let nv = r.get("naechster_verfall").and_then(|v| v.as_str().map(String::from)).unwrap_or_default();
        r.insert("naechster_verfall".into(), json!(nv));
        rows.push(Value::Object(r));
    }
    let s = settings::load(&state.db).await?;
    let mut ctx = base_ctx(&state.data_dir, &s);
    ctx.insert("stichtag".into(), json!(time::fmt_date_de(as_of)));
    ctx.insert("rows".into(), json!(rows));
    let bytes = render_template(&state.data_dir, TEMPLATE_URLAUB_UEBERSICHT, &Value::Object(ctx)).await?;
    Ok(pdf_response(bytes, &format!("Urlaubsuebersicht_{}.pdf", time::fmt_date(as_of))))
}

fn fmt_days(d: f64) -> String {
    if (d - d.round()).abs() < 1e-9 { format!("{}", d.round() as i64) } else { format!("{:.1}", d).replace('.', ",") }
}

fn month_label_de(monat: &str) -> String {
    let names = ["Jänner", "Februar", "März", "April", "Mai", "Juni", "Juli", "August", "September", "Oktober", "November", "Dezember"];
    let (y, m) = monat.split_once('-').unwrap_or((monat, "1"));
    let idx = m.parse::<usize>().unwrap_or(1).clamp(1, 12) - 1;
    format!("{} {}", names[idx], y)
}

fn data_hash(mv: &MonthView) -> String {
    let s = serde_json::to_string(&mv.days).unwrap_or_default();
    hex::encode(Sha256::digest(s.as_bytes()))
}

struct ScheduleLabels {
    wochenmodell: String,
    wochenstunden: String,
    gleitzeit: Option<String>,
}

async fn schedule_labels(db: &SqlitePool, emp: &Employee, at: chrono::NaiveDate) -> ApiResult<ScheduleLabels> {
    let schedules = db::schedules_for(db, emp.id).await?;
    let Some(s) = db::schedule_at(&schedules, at) else {
        return Ok(ScheduleLabels { wochenmodell: "–".into(), wochenstunden: "–".into(), gleitzeit: None });
    };
    let m = s.week_model();
    let gleitzeit = s.gleitzeit.then(|| {
        format!(
            "{}–{}{}",
            s.gleitzeit_von.clone().unwrap_or_default(),
            s.gleitzeit_bis.clone().unwrap_or_default(),
            match (&s.kernzeit_von, &s.kernzeit_bis) { (Some(a), Some(b)) => format!(", Kernzeit {a}–{b}"), _ => String::new() }
        )
    });
    Ok(ScheduleLabels {
        wochenmodell: m.minutes.iter().map(|x| fmt_hours(*x)).collect::<Vec<_>>().join(" / "),
        wochenstunden: format!("{} h", fmt_hours(m.weekly_minutes())),
        gleitzeit,
    })
}

fn fmt_hours(min: i32) -> String {
    let h = min as f64 / 60.0;
    if (h - h.round()).abs() < 1e-9 { format!("{}", h.round() as i64) } else { format!("{:.2}", h).replace('.', ",").trim_end_matches('0').to_string() }
}

fn decode_logo(data_dir: &Path, url: &str) -> anyhow::Result<PathBuf> {
    let (meta, b64) = url.split_once(',').ok_or_else(|| anyhow::anyhow!("logo"))?;
    let ext = if meta.contains("png") { "png" } else { "jpg" };
    let bytes = base64_decode(b64)?;
    let dir = data_dir.join("tmp");
    std::fs::create_dir_all(&dir)?;
    let p = dir.join(format!("logo.{ext}"));
    std::fs::write(&p, bytes)?;
    Ok(p)
}

fn base64_decode(s: &str) -> anyhow::Result<Vec<u8>> {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut buf = 0u32;
    let mut bits = 0;
    for c in s.bytes() {
        if c == b'=' || c == b'\n' || c == b'\r' { continue; }
        let v = T.iter().position(|&t| t == c).ok_or_else(|| anyhow::anyhow!("base64"))? as u32;
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
            buf &= (1 << bits) - 1;
        }
    }
    Ok(out)
}

/// Kompiliert `main.tex` im vorbereiteten Arbeitsverzeichnis `dir` und räumt es bei Erfolg weg.
async fn compile_latex(dir: &Path, tex_src: &str) -> ApiResult<Vec<u8>> {
    std::fs::write(dir.join("main.tex"), tex_src).map_err(|e| anyhow::anyhow!(e))?;
    let cmd = std::env::var("TIMECARD_LATEX").unwrap_or_else(|_| "latexmk".into());
    let run = tokio::process::Command::new(&cmd)
        .args(["-lualatex", "-interaction=nonstopmode", "-halt-on-error", "-file-line-error", "main.tex"])
        .current_dir(&dir)
        .output();
    let out = match tokio::time::timeout(std::time::Duration::from_secs(120), run).await {
        Ok(Ok(o)) => o,
        Ok(Err(e)) => return Err(anyhow::anyhow!("LaTeX ({cmd}) nicht startbar: {e}").into()),
        Err(_) => return Err(anyhow::anyhow!("LaTeX-Zeitüberschreitung").into()),
    };
    let pdf = dir.join("main.pdf");
    if !out.status.success() || !pdf.exists() {
        let log = std::fs::read_to_string(dir.join("main.log")).unwrap_or_default();
        let err_lines: Vec<&str> = log.lines().filter(|l| l.starts_with('!') || l.contains(":error:") || l.contains("main.tex:")).take(8).collect();
        tracing::error!("LaTeX fehlgeschlagen in {}: {}", dir.display(), err_lines.join(" | "));
        return Err(anyhow::anyhow!("PDF-Erzeugung fehlgeschlagen: {}", err_lines.join(" | ")).into());
    }
    let bytes = std::fs::read(&pdf).map_err(|e| anyhow::anyhow!(e))?;
    let _ = std::fs::remove_dir_all(dir);
    Ok(bytes)
}
