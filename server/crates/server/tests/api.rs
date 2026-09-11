//! Integrationstests: Router mit In-Memory-SQLite, HTTP über tower::oneshot.

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    Router,
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

struct Client {
    app: Router,
    cookie: Option<String>,
}

impl Client {
    async fn new() -> Self {
        std::env::set_var("TIMECARD_ADMIN_PASSWORD", "admin-test");
        let db = timecard_server::open_memory_db().await.expect("db");
        let dir = tempfile::tempdir().expect("tempdir");
        let state = timecard_server::build_state(db, dir.keep());
        Self { app: timecard_server::build_app(state), cookie: None }
    }

    async fn call(&mut self, method: &str, path: &str, body: Option<Value>) -> (StatusCode, Value, axum::http::HeaderMap) {
        let mut req = Request::builder().method(method).uri(format!("/api{path}"));
        if let Some(c) = &self.cookie {
            req = req.header(header::COOKIE, c.clone());
        }
        let req = match body {
            Some(b) => req.header(header::CONTENT_TYPE, "application/json").body(Body::from(b.to_string())).unwrap(),
            None => req.body(Body::empty()).unwrap(),
        };
        let res = self.app.clone().oneshot(req).await.unwrap();
        let status = res.status();
        let headers = res.headers().clone();
        if let Some(sc) = headers.get(header::SET_COOKIE) {
            let v = sc.to_str().unwrap();
            if v.starts_with("tc_session=") {
                self.cookie = Some(v.split(';').next().unwrap().to_string());
            }
        }
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let json = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, json, headers)
    }

    async fn login(&mut self, user: &str, pw: &str) -> StatusCode {
        self.call("POST", "/auth/login", Some(json!({"username": user, "password": pw}))).await.0
    }
}

#[tokio::test]
async fn health_and_security_headers() {
    let mut c = Client::new().await;
    let (st, _, h) = c.call("GET", "/health", None).await;
    assert_eq!(st, StatusCode::OK);
    assert!(h.get("content-security-policy").is_some());
    assert_eq!(h.get("x-frame-options").unwrap(), "DENY");
}

#[tokio::test]
async fn login_flow() {
    let mut c = Client::new().await;
    assert_eq!(c.call("GET", "/auth/me", None).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(c.login("admin", "falsch").await, StatusCode::UNAUTHORIZED);
    assert_eq!(c.login("admin", "admin-test").await, StatusCode::OK);
    let (st, me, _) = c.call("GET", "/auth/me", None).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(me["rolle"], "admin");
    assert_eq!(c.call("POST", "/auth/logout", None).await.0, StatusCode::OK);
}

#[tokio::test]
async fn lockout_after_repeated_failures() {
    let mut c = Client::new().await;
    for _ in 0..5 {
        assert_eq!(c.login("admin", "falsch").await, StatusCode::UNAUTHORIZED);
    }
    assert_eq!(c.login("admin", "admin-test").await, StatusCode::TOO_MANY_REQUESTS);
}

async fn create_employee(c: &mut Client, nr: &str, user: &str, hours: [f64; 7]) -> i64 {
    let (st, r, _) = c
        .call(
            "POST",
            "/employees",
            Some(json!({
                "personalnr": nr, "vorname": "Test", "nachname": user, "username": user,
                "eintritt": "2026-01-01", "durchrechnung_start": "2026-01-01",
                "passwort": "geheim123", "pin": "1234", "wochenmodell": hours
            })),
        )
        .await;
    assert_eq!(st, StatusCode::OK, "{r}");
    r["employee"]["id"].as_i64().unwrap()
}

#[tokio::test]
async fn employee_requires_admin() {
    let mut c = Client::new().await;
    c.login("admin", "admin-test").await;
    create_employee(&mut c, "7", "maria", [8.0; 7]).await;
    let mut m = Client::new().await;
    // Separate App-Instanz hat eigene DB; hier nur Rollenprüfung auf derselben Instanz:
    c.call("POST", "/auth/logout", None).await;
    c.cookie = None;
    assert_eq!(c.login("maria", "geheim123").await, StatusCode::OK);
    assert_eq!(c.call("GET", "/employees", None).await.0, StatusCode::FORBIDDEN);
    assert_eq!(m.call("GET", "/employees", None).await.0, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn terminal_punch_state_machine() {
    let mut c = Client::new().await;
    c.login("admin", "admin-test").await;
    create_employee(&mut c, "7", "maria", [8.0, 8.0, 8.0, 8.0, 8.0, 0.0, 0.0]).await;
    let punch = |art: &str, pin: &str| json!({"personalnr": "7", "pin": pin, "art": art});
    assert_eq!(c.call("POST", "/terminal/punch", Some(punch("kommen", "9999"))).await.0, StatusCode::UNAUTHORIZED);
    let (st, r, _) = c.call("POST", "/terminal/punch", Some(punch("kommen", "1234"))).await;
    assert_eq!(st, StatusCode::OK, "{r}");
    assert_eq!(r["zustand"], "arbeitet");
    assert_eq!(c.call("POST", "/terminal/punch", Some(punch("kommen", "1234"))).await.0, StatusCode::CONFLICT);
    assert_eq!(c.call("POST", "/terminal/punch", Some(punch("pause_ende", "1234"))).await.0, StatusCode::CONFLICT);
    let (st, r, _) = c.call("POST", "/terminal/punch", Some(punch("pause_start", "1234"))).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(r["zustand"], "pause");
    let (_, r, _) = c.call("POST", "/terminal/punch", Some(punch("gehen", "1234"))).await;
    assert_eq!(r["zustand"], "draussen");
    assert_eq!(r["heute"].as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn absences_vacation_and_export_preview() {
    let mut c = Client::new().await;
    c.login("admin", "admin-test").await;
    let id = create_employee(&mut c, "7", "maria", [8.0, 8.0, 8.0, 8.0, 8.0, 0.0, 0.0]).await;
    // Firmennummer setzen
    let (_, mut s, _) = c.call("GET", "/settings", None).await;
    s["bmd_firmennr"] = json!("27210390");
    assert_eq!(c.call("PUT", "/settings", Some(s)).await.0, StatusCode::OK);
    // Urlaub 3.–7.8.2026 (Mo–Fr) buchen
    let (st, r, _) = c.call("POST", &format!("/employees/{id}/absences"), Some(json!({"art": "urlaub", "von": "2026-08-03", "bis": "2026-08-07"}))).await;
    assert_eq!(st, StatusCode::OK, "{r}");
    // Überschneidung wird abgelehnt
    let (st, _, _) = c.call("POST", &format!("/employees/{id}/absences"), Some(json!({"art": "krank", "von": "2026-08-05", "bis": "2026-08-05"}))).await;
    assert_eq!(st, StatusCode::CONFLICT);
    // Monat: 5 Tage Abwesenheit, Urlaubskonto 25 − 5
    let (st, m, _) = c.call("GET", &format!("/employees/{id}/month?monat=2026-08"), None).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(m["abwesenheit_min"], 2400);
    assert_eq!(m["urlaub"]["rest"], 20.0);
    // Feiertag 15.8. (Samstag) hat kein Soll; 26.10. (Montag) Feiertag → Soll 480, Feiertagsausfall 480
    let (_, o, _) = c.call("GET", &format!("/employees/{id}/month?monat=2026-10"), None).await;
    let d26 = o["days"].as_array().unwrap().iter().find(|d| d["date"] == "2026-10-26").unwrap();
    assert_eq!(d26["holiday_name"], "Nationalfeiertag");
    assert_eq!(d26["target_min"], 480);
    // Export-Vorschau: Urlaubszeile 301 mit Abrechnungsmonat 9
    let (st, p, _) = c.call("GET", "/export/preview?monat=2026-08", None).await;
    assert_eq!(st, StatusCode::OK, "{p}");
    let rows = p["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["nlz_k"], "301");
    assert_eq!(rows[0]["monat"], 9);
    assert_eq!(rows[0]["nlz_v"], "03.08.2026");
    assert_eq!(rows[0]["nlz_ver"], "");
    // Export ohne Monatsabschluss wird verweigert
    let (st, _, _) = c.call("POST", "/export/bmd", Some(json!({"monat": "2026-08"}))).await;
    assert_eq!(st, StatusCode::CONFLICT);
}

#[tokio::test]
async fn correction_request_flow() {
    let mut c = Client::new().await;
    c.login("admin", "admin-test").await;
    create_employee(&mut c, "7", "maria", [8.0, 8.0, 8.0, 8.0, 8.0, 0.0, 0.0]).await;
    let admin_cookie = c.cookie.clone();
    c.cookie = None;
    assert_eq!(c.login("maria", "geheim123").await, StatusCode::OK);
    // Antrag: Gehen für gestern nachtragen
    let yesterday = (chrono::Utc::now() - chrono::Duration::days(1)).format("%Y-%m-%d").to_string();
    let (st, r, _) = c.call("POST", "/punch-requests", Some(json!({"typ": "einfuegen", "datum": yesterday, "zeit": "17:00", "art": "gehen", "begruendung": "vergessen"}))).await;
    assert_eq!(st, StatusCode::OK, "{r}");
    let rid = r["id"].as_i64().unwrap();
    // Zukunft wird abgelehnt
    let (st, _, _) = c.call("POST", "/punch-requests", Some(json!({"typ": "einfuegen", "datum": "2099-01-01", "zeit": "08:00", "art": "kommen", "begruendung": "x"}))).await;
    assert_eq!(st, StatusCode::BAD_REQUEST);
    // Admin genehmigt → Stempelung existiert
    c.cookie = admin_cookie;
    let (st, _, _) = c.call("POST", &format!("/punch-requests/{rid}/decide"), Some(json!({"status": "genehmigt"}))).await;
    assert_eq!(st, StatusCode::OK);
    let (_, list, _) = c.call("GET", "/admin/punch-requests?status=genehmigt", None).await;
    assert_eq!(list.as_array().unwrap().len(), 1);
}
