//! Betriebseinstellungen und Branding.

use axum::{extract::State, routing::get, Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::SqlitePool;

use crate::{
    auth::AdminUser,
    db,
    error::{bad, ApiResult},
    time, AppState,
};

const KEY: &str = "app";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    // Branding
    pub firmenname: String,
    pub logo_data_url: Option<String>,
    pub primaerfarbe: String,
    pub fusszeile: String,
    pub terminal_dunkel: bool,
    // BMD
    pub bmd_firmennr: String,
    pub bmd_zeichensatz: String,          // "windows-1252" | "utf-8"
    pub bmd_krank_exportieren: bool,
    pub bmd_kopfzeile: bool,              // Doku-Zeile mit Feldnamen als erste Zeile
    pub bmd_absonderung_divnlz: String,   // "101"
    pub topf_vollzeit: i64,               // 307
    pub topf_teilzeit: i64,               // 311
    pub topf_feiertag: i64,               // 308
    // Arbeitszeit
    pub kv_wochenstunden: f64,            // 40 oder 38,5
    pub pause_schwelle_min: i64,
    pub pause_dauer_min: i64,
    pub rundung_min: i64,                 // 0 = keine Rundung
    // Stempelsperren: betreffen nur „Kommen“ über Portal und Terminal, nicht Nachträge der Verwaltung
    pub stempeln_wochenende: bool,        // Kommen an Samstag/Sonntag erlaubt
    pub stempeln_feiertag: bool,          // Kommen an gesetzlichen/betrieblichen Feiertagen erlaubt
    pub stempeln_von: String,             // "HH:MM" oder leer = kein Stempelfenster
    pub stempeln_bis: String,             // "HH:MM"; liegt bis vor von, geht das Fenster über Mitternacht
    // Urlaub
    pub urlaub_halbe_tage: bool,
    pub urlaub_stunden: bool,
    pub urlaub_verfall_auto: bool,        // Verjährung nach § 4 Abs 5 UrlG automatisch buchen (Standard aus: EuGH C-619/16, C-684/16)
    pub urlaub_hinweis: String,
    // PDF
    pub unterschrift_1: String,
    pub unterschrift_2: String,
    pub bundesland: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            firmenname: "Zeiterfassung".into(),
            logo_data_url: None,
            primaerfarbe: "#2f6f8f".into(),
            fusszeile: String::new(),
            terminal_dunkel: true,
            bmd_firmennr: String::new(),
            bmd_zeichensatz: "windows-1252".into(),
            bmd_krank_exportieren: false,
            bmd_kopfzeile: false,
            bmd_absonderung_divnlz: "101".into(),
            topf_vollzeit: 307,
            topf_teilzeit: 311,
            topf_feiertag: 308,
            kv_wochenstunden: 40.0,
            pause_schwelle_min: 360,
            pause_dauer_min: 30,
            rundung_min: 0,
            stempeln_wochenende: true,
            stempeln_feiertag: true,
            stempeln_von: String::new(),
            stempeln_bis: String::new(),
            urlaub_halbe_tage: false,
            urlaub_stunden: false,
            urlaub_verfall_auto: false,
            urlaub_hinweis: "Urlaub ist nach dem Urlaubsgesetz in ganzen Arbeitstagen zu verbrauchen. \
Ein stundenweiser Verbrauch ist gesetzlich nicht vorgesehen und nur ausnahmsweise auf Wunsch und im \
Interesse des Arbeitnehmers mit ausdrücklicher Vereinbarung vertretbar. Bitte vor Verwendung rechtlich prüfen."
                .into(),
            unterschrift_1: "Arbeitnehmer:in".into(),
            unterschrift_2: "Arbeitgeber:in / Vorgesetzte:r".into(),
            bundesland: String::new(),
        }
    }
}

pub async fn load(db: &SqlitePool) -> ApiResult<Settings> {
    let row: Option<String> = sqlx::query_scalar("SELECT value FROM settings WHERE key = ?")
        .bind(KEY)
        .fetch_optional(db)
        .await?;
    Ok(row.and_then(|v| serde_json::from_str(&v).ok()).unwrap_or_default())
}

pub async fn save(db: &SqlitePool, s: &Settings) -> ApiResult<()> {
    sqlx::query("INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
        .bind(KEY)
        .bind(serde_json::to_string(s).map_err(|e| anyhow::anyhow!(e))?)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn ensure_defaults(db: &SqlitePool) -> anyhow::Result<()> {
    let s = load(db).await?;
    save(db, &s).await?;
    Ok(())
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/settings/public", get(public))
        .route("/settings", get(get_all).put(put_all))
}

/// Branding ohne Anmeldung (Login-Seite, Terminal).
async fn public(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    let s = load(&state.db).await?;
    Ok(Json(json!({
        "firmenname": s.firmenname,
        "logo_data_url": s.logo_data_url,
        "primaerfarbe": s.primaerfarbe,
        "fusszeile": s.fusszeile,
        "terminal_dunkel": s.terminal_dunkel,
        "urlaub_halbe_tage": s.urlaub_halbe_tage,
        "urlaub_stunden": s.urlaub_stunden,
        "urlaub_hinweis": s.urlaub_hinweis,
    })))
}

async fn get_all(State(state): State<AppState>, AdminUser(_): AdminUser) -> ApiResult<Json<Settings>> {
    Ok(Json(load(&state.db).await?))
}

async fn put_all(
    State(state): State<AppState>,
    AdminUser(admin): AdminUser,
    Json(new): Json<Settings>,
) -> ApiResult<Json<Settings>> {
    let old = load(&state.db).await?;
    let mut new = new;
    new.stempeln_von = new.stempeln_von.trim().to_string();
    new.stempeln_bis = new.stempeln_bis.trim().to_string();
    match (new.stempeln_von.is_empty(), new.stempeln_bis.is_empty()) {
        (true, true) => {}
        (false, false) if time::parse_hm(&new.stempeln_von).is_some() && time::parse_hm(&new.stempeln_bis).is_some() => {}
        _ => return Err(bad("Stempelfenster: beide Uhrzeiten als HH:MM angeben oder beide leer lassen")),
    }
    save(&state.db, &new).await?;
    db::audit(
        &state.db,
        Some(admin.id),
        "einstellungen_geaendert",
        None,
        Some(serde_json::to_value(&old).unwrap_or(Value::Null)),
        Some(serde_json::to_value(&new).unwrap_or(Value::Null)),
    )
    .await?;
    Ok(Json(new))
}
