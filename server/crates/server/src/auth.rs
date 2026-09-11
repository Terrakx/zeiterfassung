//! Anmeldung, Sessions, Rollenprüfung.

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    extract::{FromRequestParts, State},
    http::request::Parts,
    routing::{get, post},
    Json, Router,
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use chrono::Duration;
use rand::{distributions::Alphanumeric, Rng};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::SqlitePool;

use crate::{
    db::{self, Employee},
    error::{bad, ApiResult, AppError},
    time, AppState,
};

const COOKIE: &str = "tc_session";
const SESSION_HOURS: i64 = 12;

pub fn hash_secret(secret: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Ok(Argon2::default()
        .hash_password(secret.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("hash: {e}"))?
        .to_string())
}

pub fn verify_secret(secret: &str, hash: &str) -> bool {
    PasswordHash::new(hash)
        .map(|h| Argon2::default().verify_password(secret.as_bytes(), &h).is_ok())
        .unwrap_or(false)
}

/// Legt beim ersten Start einen Admin an.
pub async fn seed_admin(db: &SqlitePool) -> anyhow::Result<()> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM employees").fetch_one(db).await?;
    if count > 0 {
        return Ok(());
    }
    let pw = std::env::var("TIMECARD_ADMIN_PASSWORD").unwrap_or_else(|_| "admin".into());
    let today = time::fmt_date(time::today_local());
    sqlx::query(
        "INSERT INTO employees (personalnr, vorname, nachname, username, password_hash, rolle, eintritt, durchrechnung_start)
         VALUES ('0', 'System', 'Administrator', 'admin', ?, 'admin', ?, ?)",
    )
    .bind(hash_secret(&pw)?)
    .bind(&today)
    .bind(&today)
    .execute(db)
    .await?;
    tracing::warn!("Erster Start: Benutzer 'admin' angelegt. Passwort bitte sofort ändern.");
    Ok(())
}

fn new_token() -> String {
    rand::thread_rng().sample_iter(&Alphanumeric).take(48).map(char::from).collect()
}

async fn create_session(db: &SqlitePool, employee_id: i64) -> ApiResult<String> {
    let token = new_token();
    let now = time::now_utc();
    sqlx::query("INSERT INTO sessions (token, employee_id, created_at, expires_at) VALUES (?,?,?,?)")
        .bind(&token)
        .bind(employee_id)
        .bind(time::fmt_utc(now))
        .bind(time::fmt_utc(now + Duration::hours(SESSION_HOURS)))
        .execute(db)
        .await?;
    Ok(token)
}

async fn session_user(db: &SqlitePool, token: &str) -> ApiResult<Option<Employee>> {
    let now = time::fmt_utc(time::now_utc());
    let emp = sqlx::query_as::<_, Employee>(
        "SELECT e.* FROM sessions s JOIN employees e ON e.id = s.employee_id
         WHERE s.token = ? AND s.expires_at > ? AND e.aktiv = 1",
    )
    .bind(token)
    .bind(&now)
    .fetch_optional(db)
    .await?;
    Ok(emp)
}

fn session_cookie(token: String) -> Cookie<'static> {
    Cookie::build((COOKIE, token))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .max_age(::time::Duration::hours(SESSION_HOURS))
        .build()
}

/// Angemeldeter Benutzer (Cookie-Session).
pub struct CurrentUser(pub Employee);

impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let jar = CookieJar::from_headers(&parts.headers);
        let token = jar.get(COOKIE).map(|c| c.value().to_string()).ok_or(AppError::Unauthorized)?;
        let user = session_user(&state.db, &token).await?.ok_or(AppError::Unauthorized)?;
        Ok(CurrentUser(user))
    }
}

/// Angemeldeter Admin.
pub struct AdminUser(pub Employee);

impl FromRequestParts<AppState> for AdminUser {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let CurrentUser(user) = CurrentUser::from_request_parts(parts, state).await?;
        if !user.is_admin() {
            return Err(AppError::Forbidden);
        }
        Ok(AdminUser(user))
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/login", post(login))
        .route("/auth/logout", post(logout))
        .route("/auth/me", get(me))
        .route("/auth/password", post(change_password))
        .route("/auth/pin", post(change_pin))
}

#[derive(Deserialize)]
struct LoginReq {
    username: String,
    password: String,
}

async fn login(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    jar: CookieJar,
    Json(req): Json<LoginReq>,
) -> ApiResult<(CookieJar, Json<Value>)> {
    let username = req.username.trim().to_lowercase();
    let keys = [format!("login:{username}"), format!("ip:{}", crate::ratelimit::client_key(&headers))];
    for k in &keys {
        if let Some(secs) = state.limiter.locked(k) {
            return Err(AppError::TooMany(format!("Zu viele Fehlversuche. Bitte in {} Minuten erneut versuchen.", secs.div_ceil(60))));
        }
    }
    let emp = sqlx::query_as::<_, Employee>("SELECT * FROM employees WHERE lower(username) = ? AND aktiv = 1")
        .bind(&username)
        .fetch_optional(&state.db)
        .await?;
    let ok = match &emp {
        Some(e) => e.password_hash.as_deref().map(|h| verify_secret(&req.password, h)).unwrap_or(false),
        None => {
            // Gleiche Laufzeit wie eine echte Prüfung, damit Benutzernamen nicht erratbar sind.
            let _ = verify_secret(&req.password, "$argon2id$v=19$m=19456,t=2,p=1$AAAAAAAAAAAAAAAAAAAAAA$AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");
            false
        }
    };
    if !ok {
        for k in &keys {
            if state.limiter.failure(k) {
                db::audit(&state.db, None, "login_gesperrt", Some(k.clone()), None, None).await?;
            }
        }
        return Err(AppError::Unauthorized);
    }
    let emp = emp.unwrap();
    for k in &keys {
        state.limiter.success(k);
    }
    let token = create_session(&state.db, emp.id).await?;
    db::audit(&state.db, Some(emp.id), "login", None, None, None).await?;
    Ok((jar.add(session_cookie(token)), Json(user_json(&emp))))
}

async fn logout(State(state): State<AppState>, jar: CookieJar) -> ApiResult<CookieJar> {
    if let Some(c) = jar.get(COOKIE) {
        sqlx::query("DELETE FROM sessions WHERE token = ?").bind(c.value()).execute(&state.db).await?;
    }
    Ok(jar.remove(Cookie::build(COOKIE).path("/").build()))
}

async fn me(CurrentUser(user): CurrentUser) -> Json<Value> {
    Json(user_json(&user))
}

pub fn user_json(e: &Employee) -> Value {
    json!({
        "id": e.id,
        "personalnr": e.personalnr,
        "vorname": e.vorname,
        "nachname": e.nachname,
        "username": e.username,
        "rolle": e.rolle,
        "hat_pin": e.pin_hash.is_some(),
    })
}

#[derive(Deserialize)]
struct PasswordReq {
    altes_passwort: String,
    neues_passwort: String,
}

async fn change_password(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Json(req): Json<PasswordReq>,
) -> ApiResult<Json<Value>> {
    let ok = user.password_hash.as_deref().map(|h| verify_secret(&req.altes_passwort, h)).unwrap_or(false);
    if !ok {
        return Err(bad("Altes Passwort stimmt nicht"));
    }
    validate_password(&req.neues_passwort)?;
    sqlx::query("UPDATE employees SET password_hash = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?")
        .bind(hash_secret(&req.neues_passwort)?)
        .bind(user.id)
        .execute(&state.db)
        .await?;
    db::audit(&state.db, Some(user.id), "passwort_geaendert", Some(format!("employee:{}", user.id)), None, None).await?;
    Ok(Json(json!({"ok": true})))
}

#[derive(Deserialize)]
struct PinReq {
    passwort: String,
    pin: String,
}

async fn change_pin(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Json(req): Json<PinReq>,
) -> ApiResult<Json<Value>> {
    let ok = user.password_hash.as_deref().map(|h| verify_secret(&req.passwort, h)).unwrap_or(false);
    if !ok {
        return Err(bad("Passwort stimmt nicht"));
    }
    validate_pin(&req.pin)?;
    sqlx::query("UPDATE employees SET pin_hash = ? WHERE id = ?")
        .bind(hash_secret(&req.pin)?)
        .bind(user.id)
        .execute(&state.db)
        .await?;
    Ok(Json(json!({"ok": true})))
}

pub fn validate_password(pw: &str) -> ApiResult<()> {
    if pw.chars().count() < 8 {
        return Err(bad("Passwort muss mindestens 8 Zeichen haben"));
    }
    Ok(())
}

pub fn validate_pin(pin: &str) -> ApiResult<()> {
    if pin.len() < 4 || pin.len() > 8 || !pin.chars().all(|c| c.is_ascii_digit()) {
        return Err(bad("PIN muss aus 4 bis 8 Ziffern bestehen"));
    }
    Ok(())
}
