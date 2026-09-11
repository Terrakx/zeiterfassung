//! Brücke zwischen Datenbank und Rechenkern: Tages- und Monatsauswertung, Salden.

use std::collections::HashMap;

use axum::{
    extract::{Path, Query, State},
    routing::get,
    Json, Router,
};
use chrono::{Datelike, Duration, NaiveDate};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use timecard_domain::{
    compute_day, day::AbsenceMinutes, AbsenceKind, AbsenceUnit, DayInput, DayResult, PauseRule, Punch, PunchKind,
};

use crate::{
    absences::{self, AbsenceRow},
    auth::{AdminUser, CurrentUser},
    db::{self, Employee, WorkSchedule},
    error::{bad, ApiResult, AppError},
    holidays, punches, settings, time, AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/calc/month", get(month_own))
        .route("/employees/{id}/month", get(month_admin))
        .route("/calc/overview", get(overview))
        .route("/admin/calendar", get(calendar))
}

/// Abwesenheitskalender aller aktiven Mitarbeitenden für einen Monat (beantragt und genehmigt).
async fn calendar(State(state): State<AppState>, AdminUser(_): AdminUser, Query(q): Query<MonthQuery>) -> ApiResult<Json<Value>> {
    let (from, to) = month_range(&q.monat)?;
    let emps = sqlx::query_as::<_, Employee>("SELECT * FROM employees WHERE aktiv = 1 AND personalnr != '0' ORDER BY nachname, vorname")
        .fetch_all(&state.db).await?;
    let hol: Vec<Value> = holidays::for_year(&state.db, from.year()).await?
        .into_iter().filter(|h| h.date >= from && h.date <= to)
        .map(|h| json!({"datum": time::fmt_date(h.date), "name": h.name})).collect();
    let mut rows = Vec::new();
    for e in emps {
        let abs = sqlx::query_as::<_, AbsenceRow>(
            "SELECT * FROM absences WHERE employee_id = ? AND status IN ('beantragt','genehmigt') AND von <= ? AND bis >= ? ORDER BY von",
        )
        .bind(e.id).bind(time::fmt_date(to)).bind(time::fmt_date(from)).fetch_all(&state.db).await?;
        let schedules = db::schedules_for(&state.db, e.id).await?;
        let mut days = serde_json::Map::new();
        let mut d = from;
        while d <= to {
            let working = db::schedule_at(&schedules, d).map(|s| s.week_model().is_working_day(d)).unwrap_or(false);
            if let Some(a) = abs.iter().find(|a| a.covers(d)) {
                days.insert(time::fmt_date(d), json!({"art": a.art, "label": a.label(), "status": a.status, "einheit": a.einheit}));
            } else if !working {
                days.insert(time::fmt_date(d), json!({"frei": true}));
            }
            d += Duration::days(1);
        }
        rows.push(json!({"id": e.id, "name": e.display_name(), "personalnr": e.personalnr, "tage": days}));
    }
    Ok(Json(json!({"monat": q.monat, "von": time::fmt_date(from), "bis": time::fmt_date(to), "feiertage": hol, "rows": rows})))
}

#[derive(Debug, Clone, Serialize)]
pub struct AbsenceView {
    pub id: i64,
    pub art: String,
    pub label: String,
    pub minutes: i32,
    pub einheit: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct DayView {
    #[serde(flatten)]
    pub result: DayResult,
    pub weekday: String,
    pub holiday_name: Option<String>,
    pub is_working_day: bool,
    /// Tag liegt in der Zukunft: Soll wird angezeigt, aber nicht in Differenz und Saldo gerechnet.
    pub future: bool,
    pub absences: Vec<AbsenceView>,
    pub punches: Vec<Value>,
}

/// Alles, was für die Berechnung eines Zeitraums nötig ist, einmal geladen.
pub struct Context {
    pub emp: Employee,
    pub schedules: Vec<WorkSchedule>,
    pub holidays: HashMap<NaiveDate, String>,
    pub punches: Vec<punches::PunchRow>,
    /// Lokale Stempelung und zugeordneter Schichttag, parallel zu `punches` (nur gültige Zeilen).
    shifts: Vec<(usize, Punch, NaiveDate)>,
    pub absences: Vec<AbsenceRow>,
    pub pause_rule: PauseRule,
}

/// Maximaler Abstand zwischen zwei Stempelungen einer Schicht (Stunden).
const SHIFT_MAX_GAP_H: i64 = 16;

impl Context {
    pub async fn load(db: &SqlitePool, emp: &Employee, from: NaiveDate, to: NaiveDate) -> ApiResult<Self> {
        let schedules = db::schedules_for(db, emp.id).await?;
        let mut hol = HashMap::new();
        for y in from.year()..=to.year() {
            for h in holidays::for_year(db, y).await? {
                hol.insert(h.date, h.name);
            }
        }
        // Eine Woche früher laden (Ruhezeit, Wochensummen) und einen Tag später (Nachtschicht,
        // die am letzten Tag beginnt und nach Mitternacht endet).
        let punches = punches::punches_between(db, emp.id, from - Duration::days(7), to + Duration::days(1)).await?;
        let local: Vec<(usize, Punch)> = punches.iter().enumerate().filter_map(|(i, r)| r.local().map(|p| (i, p))).collect();
        let only: Vec<Punch> = local.iter().map(|(_, p)| p.clone()).collect();
        let dates = timecard_domain::assign_shift_dates(&only, SHIFT_MAX_GAP_H);
        let shifts = local.into_iter().zip(dates).map(|((i, p), d)| (i, p, d)).collect();
        let absences = absences::approved_between(db, emp.id, from, to).await?;
        let s = settings::load(db).await?;
        let pause_rule = PauseRule {
            threshold_min: s.pause_schwelle_min as i32,
            required_min: s.pause_dauer_min as i32,
            auto_deduct: false,
        };
        Ok(Self { emp: emp.clone(), schedules, holidays: hol, punches, shifts, absences, pause_rule })
    }

    fn target(&self, date: NaiveDate) -> (i32, bool) {
        let d = time::fmt_date(date);
        if d < self.emp.eintritt || self.emp.austritt.as_deref().map(|a| d.as_str() > a).unwrap_or(false) {
            return (0, false);
        }
        match db::schedule_at(&self.schedules, date) {
            Some(s) => (s.week_model().target_for(date), s.pause_auto),
            None => (0, false),
        }
    }

    fn flex_frame(&self, date: NaiveDate) -> Option<(chrono::NaiveTime, chrono::NaiveTime)> {
        let s = db::schedule_at(&self.schedules, date)?;
        if !s.gleitzeit {
            return None;
        }
        let von = chrono::NaiveTime::parse_from_str(s.gleitzeit_von.as_deref()?, "%H:%M").ok()?;
        let bis = chrono::NaiveTime::parse_from_str(s.gleitzeit_bis.as_deref()?, "%H:%M").ok()?;
        Some((von, bis))
    }

    pub fn day(&self, date: NaiveDate) -> DayView {
        let (target, pause_auto) = self.target(date);
        let is_holiday = self.holidays.contains_key(&date);
        let mut day_punches: Vec<Punch> = Vec::new();
        let mut punch_json = Vec::new();
        let mut prev_end: Option<chrono::NaiveDateTime> = None;
        for (i, p, shift_date) in &self.shifts {
            let r = &self.punches[*i];
            if *shift_date == date {
                let next_day = p.at.date() != date;
                punch_json.push(json!({
                    "id": r.id, "zeit": format!("{}{}", p.at.format("%H:%M"), if next_day { "+1" } else { "" }), "art": r.art,
                    "quelle": r.quelle, "kommentar": r.kommentar,
                }));
                day_punches.push(p.clone());
            } else if *shift_date < date && p.kind == PunchKind::ClockOut {
                prev_end = Some(prev_end.map_or(p.at, |e| e.max(p.at)));
            }
        }
        let mut abs_views = Vec::new();
        let mut abs_minutes = Vec::new();
        for a in &self.absences {
            if !a.covers(date) {
                continue;
            }
            let Some(kind) = AbsenceKind::parse(&a.art) else { continue };
            let unit = AbsenceUnit::parse(&a.einheit).unwrap_or(AbsenceUnit::Tag);
            // Abwesenheit zählt nur an Arbeitstagen; am Feiertag geht das Feiertagsentgelt vor.
            let minutes = if target == 0 || is_holiday {
                0
            } else {
                match unit {
                    AbsenceUnit::Tag => target,
                    AbsenceUnit::HalberTag => target / 2,
                    AbsenceUnit::Stunden => ((a.wert.unwrap_or(0.0)) * 60.0).round() as i32,
                }
            };
            abs_views.push(AbsenceView { id: a.id, art: a.art.clone(), label: kind.label().into(), minutes, einheit: a.einheit.clone() });
            abs_minutes.push(AbsenceMinutes { kind, minutes });
        }
        let mut rule = self.pause_rule;
        rule.auto_deduct = pause_auto;
        let future = date > time::today_local();
        let mut result = compute_day(&DayInput {
            date,
            target_min: target,
            is_holiday,
            punches: &day_punches,
            absences: &abs_minutes,
            pause_rule: rule,
            previous_day_end: prev_end,
            flex_frame: self.flex_frame(date),
        });
        if future {
            result.diff_min = 0;
            result.holiday_min = 0;
            result.paid_absence_min = 0;
            result.warnings.clear();
        }
        DayView {
            result,
            weekday: weekday_de(date).into(),
            holiday_name: self.holidays.get(&date).cloned(),
            is_working_day: target > 0,
            future,
            absences: abs_views,
            punches: punch_json,
        }
    }

    /// Tage im Zeitraum, ergänzt um Wochenwarnungen (Mo–So, an den letzten Arbeitstag der Woche gehängt).
    /// Der Context muss dafür die volle Woche des ersten Tages enthalten; sonst wird die Woche
    /// nur mit den vorhandenen Tagen bewertet.
    pub fn days(&self, from: NaiveDate, to: NaiveDate) -> Vec<DayView> {
        let mut out = Vec::new();
        let mut d = from;
        while d <= to {
            out.push(self.day(d));
            d += Duration::days(1);
        }
        // Wochensummen: Woche beginnt Montag. Tage vor `from` derselben Woche werden mitgerechnet,
        // damit ein Monatsanfang mitten in der Woche korrekt bewertet wird.
        let mut i = 0;
        while i < out.len() {
            let week_start = out[i].result.date - Duration::days(out[i].result.date.weekday().num_days_from_monday() as i64);
            let week_end = week_start + Duration::days(6);
            let mut sum: i32 = 0;
            let mut d = week_start;
            while d < from {
                sum += self.day(d).result.worked_min;
                d += Duration::days(1);
            }
            let mut last_idx = i;
            let mut j = i;
            while j < out.len() && out[j].result.date <= week_end {
                sum += out[j].result.worked_min;
                if out[j].result.worked_min > 0 {
                    last_idx = j;
                }
                j += 1;
            }
            if sum > 3600 {
                out[last_idx].result.warnings.push(timecard_domain::Warning::WeekOver60h { worked_min: sum });
            } else if sum > 3000 {
                out[last_idx].result.warnings.push(timecard_domain::Warning::WeekOver50h { worked_min: sum });
            }
            i = j;
        }
        out
    }
}

pub fn weekday_de(d: NaiveDate) -> &'static str {
    ["Mo", "Di", "Mi", "Do", "Fr", "Sa", "So"][d.weekday().num_days_from_monday() as usize]
}

pub async fn day_for(db: &SqlitePool, emp: &Employee, date: NaiveDate) -> ApiResult<DayView> {
    Ok(Context::load(db, emp, date, date).await?.day(date))
}

/// Beginn der Saldoberechnung: Erfassungsbeginn (durchrechnung_start), frühestens Eintritt.
pub fn saldo_start(emp: &Employee) -> ApiResult<NaiveDate> {
    let e = time::parse_date(&emp.eintritt).ok_or_else(|| bad("Eintritt ungültig"))?;
    let d = time::parse_date(&emp.durchrechnung_start).unwrap_or(e);
    Ok(d.max(e))
}

/// Gleitzeitsaldo am Ende von `date`: Σ Differenzen seit Erfassungsbeginn, abzüglich der in
/// Gutstundentöpfe übertragenen Minuten, zuzüglich manueller Saldo-Buchungen (Anfangswert, Korrektur).
pub async fn saldo_until(db: &SqlitePool, emp: &Employee, date: NaiveDate) -> ApiResult<i32> {
    // Jüngster abgeschlossener Monat mit festgehaltenem Saldo, dessen Ende vor `date` liegt.
    let snap: Option<(String, i64)> = sqlx::query_as(
        "SELECT monat, saldo_ende_min FROM month_closures
         WHERE employee_id = ? AND saldo_ende_min IS NOT NULL AND monat < ?
         ORDER BY monat DESC LIMIT 1",
    )
    .bind(emp.id)
    .bind(date.format("%Y-%m").to_string())
    .fetch_optional(db)
    .await?;
    let (base, from, after) = match snap {
        Some((monat, saldo)) => {
            let (_, end) = month_range(&monat)?;
            (saldo as i32, end + Duration::days(1), Some(time::fmt_date(end)))
        }
        None => (0, saldo_start(emp)?, None),
    };
    let sum: i32 = if date < from {
        0
    } else {
        let ctx = Context::load(db, emp, from, date).await?;
        ctx.days(from, date).iter().map(|d| d.result.diff_min).sum()
    };
    let (transferred, manual): (i64, i64) = sqlx::query_as(
        "SELECT COALESCE(SUM(CASE WHEN art = 'periodenabschluss' THEN minuten ELSE 0 END),0),
                COALESCE(SUM(CASE WHEN art = 'saldo' THEN minuten ELSE 0 END),0)
         FROM credit_hours_entries WHERE employee_id = ? AND datum <= ? AND datum > ?",
    )
    .bind(emp.id)
    .bind(time::fmt_date(date))
    .bind(after.unwrap_or_else(|| "0000-00-00".into()))
    .fetch_one(db)
    .await?;
    Ok(base + sum - transferred as i32 + manual as i32)
}

pub async fn ensure_month_open(db: &SqlitePool, employee_id: i64, date: NaiveDate) -> ApiResult<()> {
    let m = date.format("%Y-%m").to_string();
    let closed: Option<String> = sqlx::query_scalar("SELECT geschlossen_at FROM month_closures WHERE employee_id = ? AND monat = ?")
        .bind(employee_id)
        .bind(&m)
        .fetch_optional(db)
        .await?;
    if closed.is_some() {
        return Err(AppError::Conflict(format!("Monat {m} ist abgeschlossen. Abschluss zuerst aufheben.")));
    }
    Ok(())
}

pub fn month_range(monat: &str) -> ApiResult<(NaiveDate, NaiveDate)> {
    let first = NaiveDate::parse_from_str(&format!("{monat}-01"), "%Y-%m-%d").map_err(|_| bad("Monat im Format YYYY-MM"))?;
    let next = if first.month() == 12 {
        NaiveDate::from_ymd_opt(first.year() + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(first.year(), first.month() + 1, 1)
    }
    .unwrap();
    Ok((first, next - Duration::days(1)))
}

#[derive(Serialize)]
pub struct MonthView {
    pub monat: String,
    pub employee: Value,
    pub days: Vec<DayView>,
    pub soll_min: i32,
    pub ist_min: i32,
    pub pause_min: i32,
    pub abwesenheit_min: i32,
    pub feiertag_min: i32,
    pub diff_min: i32,
    pub saldo_start_min: i32,
    pub saldo_ende_min: i32,
    pub abwesenheit_nach_art: Vec<Value>,
    pub urlaub: Value,
    pub gutstunden: Value,
    pub warnungen: usize,
    pub geschlossen: Option<Value>,
}

pub async fn month_view(db: &SqlitePool, emp: &Employee, monat: &str) -> ApiResult<MonthView> {
    let (from, to) = month_range(monat)?;
    let ctx = Context::load(db, emp, from, to).await?;
    let days = ctx.days(from, to);
    let saldo_start = saldo_until(db, emp, from - Duration::days(1)).await?;
    let diff: i32 = days.iter().map(|d| d.result.diff_min).sum();
    let (transferred, manual): (i64, i64) = sqlx::query_as(
        "SELECT COALESCE(SUM(CASE WHEN art = 'periodenabschluss' THEN minuten ELSE 0 END),0),
                COALESCE(SUM(CASE WHEN art = 'saldo' THEN minuten ELSE 0 END),0)
         FROM credit_hours_entries WHERE employee_id = ? AND datum BETWEEN ? AND ?",
    )
    .bind(emp.id).bind(time::fmt_date(from)).bind(time::fmt_date(to)).fetch_one(db).await?;
    let mut by_kind: HashMap<String, (String, i32, f64)> = HashMap::new();
    for d in &days {
        for a in &d.absences {
            let e = by_kind.entry(a.art.clone()).or_insert((a.label.clone(), 0, 0.0));
            e.1 += a.minutes;
            e.2 += match a.einheit.as_str() { "halber_tag" => 0.5, "stunden" => 0.0, _ => 1.0 };
        }
    }
    let mut abwesenheit_nach_art: Vec<Value> = by_kind
        .into_iter()
        .map(|(art, (label, min, tage))| json!({"art": art, "label": label, "minuten": min, "tage": tage}))
        .collect();
    abwesenheit_nach_art.sort_by(|a, b| a["art"].as_str().cmp(&b["art"].as_str()));
    let closed = sqlx::query_as::<_, (String, Option<i64>, Option<String>)>(
        "SELECT geschlossen_at, geschlossen_von, pdf_pfad FROM month_closures WHERE employee_id = ? AND monat = ?",
    )
    .bind(emp.id).bind(monat).fetch_optional(db).await?;
    let urlaub = absences::vacation_account(db, emp, to).await?;
    let gutstunden = absences::credit_account(db, emp, to).await?;
    Ok(MonthView {
        monat: monat.into(),
        employee: json!({"id": emp.id, "name": emp.display_name(), "personalnr": emp.personalnr}),
        soll_min: days.iter().map(|d| d.result.target_min).sum(),
        ist_min: days.iter().map(|d| d.result.worked_min).sum(),
        pause_min: days.iter().map(|d| d.result.break_min).sum(),
        abwesenheit_min: days.iter().map(|d| d.result.paid_absence_min).sum(),
        feiertag_min: days.iter().map(|d| d.result.holiday_min).sum(),
        diff_min: diff,
        saldo_start_min: saldo_start,
        saldo_ende_min: saldo_start + diff - transferred as i32 + manual as i32,
        abwesenheit_nach_art,
        urlaub,
        gutstunden,
        warnungen: days.iter().map(|d| d.result.warnings.len()).sum(),
        geschlossen: closed.map(|(at, von, pdf)| json!({"at": at, "von": von, "pdf": pdf})),
        days,
    })
}

#[derive(Serialize)]
pub struct YearMonth {
    pub monat: String,
    pub soll_min: i32,
    pub ist_min: i32,
    pub abwesenheit_min: i32,
    pub feiertag_min: i32,
    pub diff_min: i32,
    pub uebertragen_min: i32,
    pub saldo_ende_min: i32,
    pub geschlossen: bool,
    pub warnungen: usize,
}

#[derive(Serialize)]
pub struct YearView {
    pub jahr: i32,
    pub employee: Value,
    pub saldo_start_min: i32,
    pub monate: Vec<YearMonth>,
    pub soll_min: i32,
    pub ist_min: i32,
    pub abwesenheit_min: i32,
    pub feiertag_min: i32,
    pub diff_min: i32,
    pub saldo_ende_min: i32,
    pub urlaub_rest: f64,
}

/// Jahresübersicht eines Mitarbeiters: ein Context für das ganze Jahr, Summen je Monat.
pub async fn year_view(db: &SqlitePool, emp: &Employee, jahr: i32) -> ApiResult<YearView> {
    let from = NaiveDate::from_ymd_opt(jahr, 1, 1).ok_or_else(|| bad("Jahr ungültig"))?;
    let to = NaiveDate::from_ymd_opt(jahr, 12, 31).unwrap();
    let ctx = Context::load(db, emp, from, to).await?;
    let days = ctx.days(from, to);
    let saldo_start = saldo_until(db, emp, from - Duration::days(1)).await?;
    let entries: Vec<(String, String, i64)> = sqlx::query_as(
        "SELECT substr(datum,1,7), art, minuten FROM credit_hours_entries WHERE employee_id = ? AND datum BETWEEN ? AND ? AND art IN ('periodenabschluss','saldo')",
    )
    .bind(emp.id).bind(time::fmt_date(from)).bind(time::fmt_date(to)).fetch_all(db).await?;
    let closed: Vec<(String,)> = sqlx::query_as("SELECT monat FROM month_closures WHERE employee_id = ? AND monat LIKE ?")
        .bind(emp.id).bind(format!("{jahr}-%")).fetch_all(db).await?;
    let mut saldo = saldo_start;
    let mut monate = Vec::new();
    for m in 1..=12u32 {
        let key = format!("{jahr}-{m:02}");
        let md: Vec<&DayView> = days.iter().filter(|d| d.result.date.month() == m).collect();
        let diff: i32 = md.iter().map(|d| d.result.diff_min).sum();
        let transferred: i32 = entries.iter().filter(|e| e.0 == key && e.1 == "periodenabschluss").map(|e| e.2 as i32).sum();
        let manual: i32 = entries.iter().filter(|e| e.0 == key && e.1 == "saldo").map(|e| e.2 as i32).sum();
        saldo += diff - transferred + manual;
        monate.push(YearMonth {
            monat: key.clone(),
            soll_min: md.iter().map(|d| d.result.target_min).sum(),
            ist_min: md.iter().map(|d| d.result.worked_min).sum(),
            abwesenheit_min: md.iter().map(|d| d.result.paid_absence_min).sum(),
            feiertag_min: md.iter().map(|d| d.result.holiday_min).sum(),
            diff_min: diff,
            uebertragen_min: transferred,
            saldo_ende_min: saldo,
            geschlossen: closed.iter().any(|c| c.0 == key),
            warnungen: md.iter().map(|d| d.result.warnings.len()).sum(),
        });
    }
    let urlaub = absences::vacation_account(db, emp, to.min(time::today_local())).await?;
    Ok(YearView {
        jahr,
        employee: json!({"id": emp.id, "name": emp.display_name(), "personalnr": emp.personalnr}),
        saldo_start_min: saldo_start,
        soll_min: monate.iter().map(|x| x.soll_min).sum(),
        ist_min: monate.iter().map(|x| x.ist_min).sum(),
        abwesenheit_min: monate.iter().map(|x| x.abwesenheit_min).sum(),
        feiertag_min: monate.iter().map(|x| x.feiertag_min).sum(),
        diff_min: monate.iter().map(|x| x.diff_min).sum(),
        saldo_ende_min: saldo,
        urlaub_rest: urlaub["rest"].as_f64().unwrap_or(0.0),
        monate,
    })
}

#[derive(Deserialize)]
pub struct MonthQuery {
    pub monat: String,
}

async fn month_own(State(state): State<AppState>, CurrentUser(user): CurrentUser, Query(q): Query<MonthQuery>) -> ApiResult<Json<MonthView>> {
    Ok(Json(month_view(&state.db, &user, &q.monat).await?))
}

async fn month_admin(State(state): State<AppState>, AdminUser(_): AdminUser, Path(id): Path<i64>, Query(q): Query<MonthQuery>) -> ApiResult<Json<MonthView>> {
    let emp = db::get_employee(&state.db, id).await?;
    Ok(Json(month_view(&state.db, &emp, &q.monat).await?))
}

/// Admin-Dashboard: Anwesenheit und Saldo aller aktiven Mitarbeiter.
async fn overview(State(state): State<AppState>, AdminUser(_): AdminUser) -> ApiResult<Json<Vec<Value>>> {
    let emps = sqlx::query_as::<_, Employee>("SELECT * FROM employees WHERE aktiv = 1 ORDER BY nachname, vorname")
        .fetch_all(&state.db)
        .await?;
    let today = time::today_local();
    let mut out = Vec::new();
    for e in emps {
        let ctx = Context::load(&state.db, &e, today, today).await?;
        let all: Vec<Punch> = ctx.punches.iter().filter_map(|r| r.local()).collect();
        let day = ctx.day(today);
        let saldo = saldo_until(&state.db, &e, today).await?;
        let open_requests: i64 = sqlx::query_scalar(
            "SELECT (SELECT COUNT(*) FROM absences WHERE employee_id = ? AND status = 'beantragt')
                  + (SELECT COUNT(*) FROM punch_requests WHERE employee_id = ? AND status = 'beantragt')",
        )
        .bind(e.id).bind(e.id).fetch_one(&state.db).await?;
        out.push(json!({
            "id": e.id,
            "name": e.display_name(),
            "personalnr": e.personalnr,
            "rolle": e.rolle,
            "zustand": punches::presence_state(&all),
            "heute_ist_min": day.result.worked_min,
            "heute_soll_min": day.result.target_min,
            "abwesenheit": day.absences.first().map(|a| a.label.clone()),
            "saldo_min": saldo,
            "offene_antraege": open_requests,
            "warnungen_heute": day.result.warnings.len(),
        }));
    }
    Ok(Json(out))
}
