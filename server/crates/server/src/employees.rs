//! Stammdaten: Mitarbeiter und Wochenmodelle (Admin).

use axum::{
    extract::{Path, State},
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    auth::{self, AdminUser},
    db::{self, Employee, WorkSchedule},
    error::{bad, ApiResult, AppError},
    time, AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/employees", get(list).post(create))
        .route("/employees/{id}", get(get_one).put(update).delete(deactivate))
        .route("/employees/{id}/password", post(reset_password))
        .route("/employees/{id}/pin", post(reset_pin))
        .route("/employees/{id}/schedules", get(list_schedules).post(add_schedule))
        .route("/employees/{id}/schedules/{sid}", axum::routing::delete(delete_schedule))
}

async fn list(State(state): State<AppState>, AdminUser(_): AdminUser) -> ApiResult<Json<Vec<Employee>>> {
    Ok(Json(
        sqlx::query_as::<_, Employee>("SELECT * FROM employees ORDER BY aktiv DESC, nachname, vorname")
            .fetch_all(&state.db)
            .await?,
    ))
}

async fn get_one(State(state): State<AppState>, AdminUser(_): AdminUser, Path(id): Path<i64>) -> ApiResult<Json<Value>> {
    let e = db::get_employee(&state.db, id).await?;
    let s = db::schedules_for(&state.db, id).await?;
    Ok(Json(json!({ "employee": e, "schedules": s })))
}

#[derive(Deserialize)]
struct EmployeeReq {
    personalnr: String,
    vorname: String,
    nachname: String,
    username: String,
    rolle: Option<String>,
    eintritt: String,
    austritt: Option<String>,
    urlaubsanspruch_tage: Option<f64>,
    urlaubsjahr_beginn_mm_dd: Option<String>,
    durchrechnung_monate: Option<i64>,
    durchrechnung_start: Option<String>,
    gutstunden_topf: Option<i64>,
    // nur bei Anlage
    passwort: Option<String>,
    pin: Option<String>,
    /// Stunden Mo..So, z. B. [8,8,8,8,8,0,0]
    wochenmodell: Option<[f64; 7]>,
}

fn validate(req: &EmployeeReq) -> ApiResult<()> {
    if req.personalnr.trim().is_empty() || !req.personalnr.trim().chars().all(|c| c.is_ascii_digit()) {
        return Err(bad("Personalnummer muss numerisch sein (BMD-Mitarbeiternummer)"));
    }
    if req.vorname.trim().is_empty() || req.nachname.trim().is_empty() {
        return Err(bad("Vor- und Nachname sind Pflicht"));
    }
    if req.username.trim().len() < 2 {
        return Err(bad("Benutzername zu kurz"));
    }
    time::parse_date(&req.eintritt).ok_or_else(|| bad("Eintrittsdatum ungültig (YYYY-MM-DD)"))?;
    if let Some(a) = &req.austritt {
        if !a.is_empty() {
            time::parse_date(a).ok_or_else(|| bad("Austrittsdatum ungültig"))?;
        }
    }
    if let Some(r) = &req.rolle {
        if r != "admin" && r != "mitarbeiter" {
            return Err(bad("Rolle ungültig"));
        }
    }
    if let Some(m) = &req.urlaubsjahr_beginn_mm_dd {
        if m.len() != 5 || &m[2..3] != "-" {
            return Err(bad("Urlaubsjahr-Beginn im Format MM-DD"));
        }
    }
    if let Some(t) = req.gutstunden_topf {
        if !(300..=399).contains(&t) {
            return Err(bad("Gutstundentopf muss ein NLZ-Kennzeichen 3xx sein"));
        }
    }
    Ok(())
}

async fn create(
    State(state): State<AppState>,
    AdminUser(admin): AdminUser,
    Json(req): Json<EmployeeReq>,
) -> ApiResult<Json<Value>> {
    validate(&req)?;
    let pw = req.passwort.as_deref().unwrap_or("");
    let password_hash = if pw.is_empty() {
        None
    } else {
        auth::validate_password(pw)?;
        Some(auth::hash_secret(pw)?)
    };
    let pin_hash = match req.pin.as_deref() {
        Some(p) if !p.is_empty() => {
            auth::validate_pin(p)?;
            Some(auth::hash_secret(p)?)
        }
        _ => None,
    };
    let durchrechnung_start = req.durchrechnung_start.clone().unwrap_or_else(|| req.eintritt.clone());
    let mut tx = state.db.begin().await?;
    let res = sqlx::query(
        "INSERT INTO employees (personalnr, vorname, nachname, username, password_hash, pin_hash, rolle, eintritt, austritt,
            urlaubsanspruch_tage, urlaubsjahr_beginn_mm_dd, durchrechnung_monate, durchrechnung_start, gutstunden_topf)
         VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
    )
    .bind(req.personalnr.trim())
    .bind(req.vorname.trim())
    .bind(req.nachname.trim())
    .bind(req.username.trim())
    .bind(password_hash)
    .bind(pin_hash)
    .bind(req.rolle.clone().unwrap_or_else(|| "mitarbeiter".into()))
    .bind(&req.eintritt)
    .bind(req.austritt.clone().filter(|a| !a.is_empty()))
    .bind(req.urlaubsanspruch_tage.unwrap_or(25.0))
    .bind(req.urlaubsjahr_beginn_mm_dd.clone().unwrap_or_else(|| "01-01".into()))
    .bind(req.durchrechnung_monate.unwrap_or(3))
    .bind(&durchrechnung_start)
    .bind(req.gutstunden_topf)
    .execute(&mut *tx)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(ref d) if d.is_unique_violation() => AppError::Conflict("Personalnummer oder Benutzername existiert bereits".into()),
        other => AppError::Db(other),
    })?;
    let id = res.last_insert_rowid();
    let hours = req.wochenmodell.unwrap_or([8.0, 8.0, 8.0, 8.0, 8.0, 0.0, 0.0]);
    insert_schedule(&mut tx, id, &req.eintritt, hours, false).await?;
    // Urlaubsanspruch für das laufende Urlaubsjahr anlegen
    tx.commit().await?;
    db::audit(&state.db, Some(admin.id), "mitarbeiter_angelegt", Some(format!("employee:{id}")), None, Some(json!({"personalnr": req.personalnr}))).await?;
    let e = db::get_employee(&state.db, id).await?;
    Ok(Json(json!({ "employee": e })))
}

async fn insert_schedule(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    employee_id: i64,
    gueltig_ab: &str,
    hours: [f64; 7],
    pause_auto: bool,
) -> ApiResult<i64> {
    let m: Vec<i64> = hours.iter().map(|h| (h * 60.0).round() as i64).collect();
    let r = sqlx::query(
        "INSERT INTO work_schedules (employee_id, gueltig_ab, mo_min, di_min, mi_min, do_min, fr_min, sa_min, so_min, pause_auto)
         VALUES (?,?,?,?,?,?,?,?,?,?)
         ON CONFLICT(employee_id, gueltig_ab) DO UPDATE SET
            mo_min=excluded.mo_min, di_min=excluded.di_min, mi_min=excluded.mi_min, do_min=excluded.do_min,
            fr_min=excluded.fr_min, sa_min=excluded.sa_min, so_min=excluded.so_min, pause_auto=excluded.pause_auto",
    )
    .bind(employee_id)
    .bind(gueltig_ab)
    .bind(m[0]).bind(m[1]).bind(m[2]).bind(m[3]).bind(m[4]).bind(m[5]).bind(m[6])
    .bind(pause_auto)
    .execute(&mut **tx)
    .await?;
    Ok(r.last_insert_rowid())
}

async fn update(
    State(state): State<AppState>,
    AdminUser(admin): AdminUser,
    Path(id): Path<i64>,
    Json(req): Json<EmployeeReq>,
) -> ApiResult<Json<Value>> {
    validate(&req)?;
    let old = db::get_employee(&state.db, id).await?;
    if old.id == admin.id && req.rolle.as_deref() == Some("mitarbeiter") {
        return Err(bad("Eigene Admin-Rolle kann nicht entfernt werden"));
    }
    sqlx::query(
        "UPDATE employees SET personalnr=?, vorname=?, nachname=?, username=?, rolle=?, eintritt=?, austritt=?,
            urlaubsanspruch_tage=?, urlaubsjahr_beginn_mm_dd=?, durchrechnung_monate=?, durchrechnung_start=?, gutstunden_topf=?,
            updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ?",
    )
    .bind(req.personalnr.trim())
    .bind(req.vorname.trim())
    .bind(req.nachname.trim())
    .bind(req.username.trim())
    .bind(req.rolle.clone().unwrap_or(old.rolle.clone()))
    .bind(&req.eintritt)
    .bind(req.austritt.clone().filter(|a| !a.is_empty()))
    .bind(req.urlaubsanspruch_tage.unwrap_or(old.urlaubsanspruch_tage))
    .bind(req.urlaubsjahr_beginn_mm_dd.clone().unwrap_or(old.urlaubsjahr_beginn_mm_dd.clone()))
    .bind(req.durchrechnung_monate.unwrap_or(old.durchrechnung_monate))
    .bind(req.durchrechnung_start.clone().unwrap_or(old.durchrechnung_start.clone()))
    .bind(req.gutstunden_topf)
    .bind(id)
    .execute(&state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(ref d) if d.is_unique_violation() => AppError::Conflict("Personalnummer oder Benutzername existiert bereits".into()),
        other => AppError::Db(other),
    })?;
    let new = db::get_employee(&state.db, id).await?;
    db::audit(&state.db, Some(admin.id), "mitarbeiter_geaendert", Some(format!("employee:{id}")),
        Some(serde_json::to_value(&old).unwrap_or(Value::Null)), Some(serde_json::to_value(&new).unwrap_or(Value::Null))).await?;
    Ok(Json(json!({ "employee": new })))
}

async fn deactivate(State(state): State<AppState>, AdminUser(admin): AdminUser, Path(id): Path<i64>) -> ApiResult<Json<Value>> {
    if id == admin.id {
        return Err(bad("Eigener Benutzer kann nicht deaktiviert werden"));
    }
    let e = db::get_employee(&state.db, id).await?;
    let new_state = !e.aktiv;
    sqlx::query("UPDATE employees SET aktiv = ? WHERE id = ?").bind(new_state).bind(id).execute(&state.db).await?;
    sqlx::query("DELETE FROM sessions WHERE employee_id = ?").bind(id).execute(&state.db).await?;
    db::audit(&state.db, Some(admin.id), if new_state { "mitarbeiter_aktiviert" } else { "mitarbeiter_deaktiviert" }, Some(format!("employee:{id}")), None, None).await?;
    Ok(Json(json!({ "aktiv": new_state })))
}

#[derive(Deserialize)]
struct SecretReq {
    wert: String,
}

async fn reset_password(State(state): State<AppState>, AdminUser(admin): AdminUser, Path(id): Path<i64>, Json(req): Json<SecretReq>) -> ApiResult<Json<Value>> {
    auth::validate_password(&req.wert)?;
    db::get_employee(&state.db, id).await?;
    sqlx::query("UPDATE employees SET password_hash = ? WHERE id = ?").bind(auth::hash_secret(&req.wert)?).bind(id).execute(&state.db).await?;
    sqlx::query("DELETE FROM sessions WHERE employee_id = ?").bind(id).execute(&state.db).await?;
    db::audit(&state.db, Some(admin.id), "passwort_zurueckgesetzt", Some(format!("employee:{id}")), None, None).await?;
    Ok(Json(json!({"ok": true})))
}

async fn reset_pin(State(state): State<AppState>, AdminUser(admin): AdminUser, Path(id): Path<i64>, Json(req): Json<SecretReq>) -> ApiResult<Json<Value>> {
    auth::validate_pin(&req.wert)?;
    db::get_employee(&state.db, id).await?;
    sqlx::query("UPDATE employees SET pin_hash = ? WHERE id = ?").bind(auth::hash_secret(&req.wert)?).bind(id).execute(&state.db).await?;
    db::audit(&state.db, Some(admin.id), "pin_zurueckgesetzt", Some(format!("employee:{id}")), None, None).await?;
    Ok(Json(json!({"ok": true})))
}

async fn list_schedules(State(state): State<AppState>, AdminUser(_): AdminUser, Path(id): Path<i64>) -> ApiResult<Json<Vec<WorkSchedule>>> {
    Ok(Json(db::schedules_for(&state.db, id).await?))
}

#[derive(Deserialize)]
struct ScheduleReq {
    gueltig_ab: String,
    stunden: [f64; 7],
    pause_auto: Option<bool>,
    gleitzeit: Option<bool>,
    gleitzeit_von: Option<String>,
    gleitzeit_bis: Option<String>,
    kernzeit_von: Option<String>,
    kernzeit_bis: Option<String>,
    uebertrag_max_plus_min: Option<i64>,
    uebertrag_max_minus_min: Option<i64>,
}

async fn add_schedule(
    State(state): State<AppState>,
    AdminUser(admin): AdminUser,
    Path(id): Path<i64>,
    Json(req): Json<ScheduleReq>,
) -> ApiResult<Json<Vec<WorkSchedule>>> {
    db::get_employee(&state.db, id).await?;
    time::parse_date(&req.gueltig_ab).ok_or_else(|| bad("Datum ungültig"))?;
    if req.stunden.iter().any(|h| !(0.0..=24.0).contains(h)) {
        return Err(bad("Stunden je Tag müssen zwischen 0 und 24 liegen"));
    }
    let mut tx = state.db.begin().await?;
    let sid = insert_schedule(&mut tx, id, &req.gueltig_ab, req.stunden, req.pause_auto.unwrap_or(false)).await?;
    sqlx::query(
        "UPDATE work_schedules SET gleitzeit=?, gleitzeit_von=?, gleitzeit_bis=?, kernzeit_von=?, kernzeit_bis=?,
            uebertrag_max_plus_min=?, uebertrag_max_minus_min=? WHERE employee_id=? AND gueltig_ab=?",
    )
    .bind(req.gleitzeit.unwrap_or(false))
    .bind(&req.gleitzeit_von).bind(&req.gleitzeit_bis).bind(&req.kernzeit_von).bind(&req.kernzeit_bis)
    .bind(req.uebertrag_max_plus_min).bind(req.uebertrag_max_minus_min)
    .bind(id).bind(&req.gueltig_ab)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    db::audit(&state.db, Some(admin.id), "wochenmodell_gesetzt", Some(format!("employee:{id}")), None, Some(json!({"schedule_id": sid, "gueltig_ab": req.gueltig_ab, "stunden": req.stunden}))).await?;
    Ok(Json(db::schedules_for(&state.db, id).await?))
}

async fn delete_schedule(State(state): State<AppState>, AdminUser(admin): AdminUser, Path((id, sid)): Path<(i64, i64)>) -> ApiResult<Json<Vec<WorkSchedule>>> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM work_schedules WHERE employee_id = ?").bind(id).fetch_one(&state.db).await?;
    if count <= 1 {
        return Err(bad("Das letzte Wochenmodell kann nicht gelöscht werden"));
    }
    sqlx::query("DELETE FROM work_schedules WHERE id = ? AND employee_id = ?").bind(sid).bind(id).execute(&state.db).await?;
    db::audit(&state.db, Some(admin.id), "wochenmodell_geloescht", Some(format!("employee:{id}")), Some(json!({"schedule_id": sid})), None).await?;
    Ok(Json(db::schedules_for(&state.db, id).await?))
}
