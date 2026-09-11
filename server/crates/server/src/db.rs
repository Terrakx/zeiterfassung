//! Gemeinsame Modelle und Hilfsfunktionen für die Datenbank.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, SqlitePool};
use timecard_domain::WeekModel;

use crate::error::{AppError, ApiResult};

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Employee {
    pub id: i64,
    pub personalnr: String,
    pub vorname: String,
    pub nachname: String,
    pub username: String,
    #[serde(skip)]
    pub password_hash: Option<String>,
    #[serde(skip)]
    pub pin_hash: Option<String>,
    pub rolle: String,
    pub aktiv: bool,
    pub eintritt: String,
    pub austritt: Option<String>,
    pub urlaubsanspruch_tage: f64,
    pub urlaubsjahr_beginn_mm_dd: String,
    pub durchrechnung_monate: i64,
    pub durchrechnung_start: String,
    pub gutstunden_topf: Option<i64>,
    pub stempelt: bool,
}

impl Employee {
    pub fn is_admin(&self) -> bool {
        self.rolle == "admin"
    }
    pub fn display_name(&self) -> String {
        format!("{} {}", self.vorname, self.nachname)
    }
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct WorkSchedule {
    pub id: i64,
    pub employee_id: i64,
    pub gueltig_ab: String,
    pub mo_min: i64,
    pub di_min: i64,
    pub mi_min: i64,
    pub do_min: i64,
    pub fr_min: i64,
    pub sa_min: i64,
    pub so_min: i64,
    pub pause_auto: bool,
    pub gleitzeit: bool,
    pub gleitzeit_von: Option<String>,
    pub gleitzeit_bis: Option<String>,
    pub kernzeit_von: Option<String>,
    pub kernzeit_bis: Option<String>,
    pub uebertrag_max_plus_min: Option<i64>,
    pub uebertrag_max_minus_min: Option<i64>,
}

impl WorkSchedule {
    pub fn week_model(&self) -> WeekModel {
        WeekModel {
            minutes: [
                self.mo_min as i32,
                self.di_min as i32,
                self.mi_min as i32,
                self.do_min as i32,
                self.fr_min as i32,
                self.sa_min as i32,
                self.so_min as i32,
            ],
        }
    }
}

pub async fn get_employee(db: &SqlitePool, id: i64) -> ApiResult<Employee> {
    sqlx::query_as::<_, Employee>("SELECT * FROM employees WHERE id = ?")
        .bind(id)
        .fetch_optional(db)
        .await?
        .ok_or(AppError::NotFound)
}

/// Alle Wochenmodelle eines Mitarbeiters, aufsteigend nach gueltig_ab.
pub async fn schedules_for(db: &SqlitePool, employee_id: i64) -> ApiResult<Vec<WorkSchedule>> {
    Ok(sqlx::query_as::<_, WorkSchedule>(
        "SELECT * FROM work_schedules WHERE employee_id = ? ORDER BY gueltig_ab",
    )
    .bind(employee_id)
    .fetch_all(db)
    .await?)
}

/// Das am Datum gültige Wochenmodell.
pub fn schedule_at(schedules: &[WorkSchedule], date: NaiveDate) -> Option<&WorkSchedule> {
    let d = date.format("%Y-%m-%d").to_string();
    schedules.iter().rev().find(|s| s.gueltig_ab <= d)
}

pub async fn audit(
    db: &SqlitePool,
    actor: Option<i64>,
    aktion: &str,
    ziel: Option<String>,
    vorher: Option<serde_json::Value>,
    nachher: Option<serde_json::Value>,
) -> ApiResult<()> {
    sqlx::query("INSERT INTO audit_log (actor_id, aktion, ziel, vorher, nachher) VALUES (?,?,?,?,?)")
        .bind(actor)
        .bind(aktion)
        .bind(ziel)
        .bind(vorher.map(|v| v.to_string()))
        .bind(nachher.map(|v| v.to_string()))
        .execute(db)
        .await?;
    Ok(())
}
