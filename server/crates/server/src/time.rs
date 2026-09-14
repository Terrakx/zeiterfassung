//! Zeitumrechnung UTC ↔ Europe/Vienna und Formatierung.

use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Europe::Vienna;

pub fn now_utc() -> DateTime<Utc> {
    Utc::now()
}

pub fn to_local(utc: DateTime<Utc>) -> NaiveDateTime {
    utc.with_timezone(&Vienna).naive_local()
}

/// Uhrzeit „HH:MM“ (Einstellungen, Gleitzeitrahmen, Stempelfenster).
pub fn parse_hm(s: &str) -> Option<chrono::NaiveTime> {
    chrono::NaiveTime::parse_from_str(s.trim(), "%H:%M").ok()
}

pub fn local_to_utc(local: NaiveDateTime) -> DateTime<Utc> {
    // Bei Zeitumstellung: erste Variante nehmen (Herbst) bzw. nächste gültige (Frühling).
    match Vienna.from_local_datetime(&local) {
        chrono::LocalResult::Single(t) => t.with_timezone(&Utc),
        chrono::LocalResult::Ambiguous(a, _) => a.with_timezone(&Utc),
        chrono::LocalResult::None => {
            let shifted = local + chrono::Duration::hours(1);
            Vienna.from_local_datetime(&shifted).unwrap().with_timezone(&Utc)
        }
    }
}

pub fn fmt_utc(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

pub fn parse_utc(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s).ok().map(|d| d.with_timezone(&Utc))
}

pub fn today_local() -> NaiveDate {
    to_local(now_utc()).date()
}

pub fn parse_date(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
}

pub fn fmt_date(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

pub fn fmt_date_de(d: NaiveDate) -> String {
    d.format("%d.%m.%Y").to_string()
}

/// Minuten → "h:mm" mit Vorzeichen, z. B. -1:30 oder 8:00.
pub fn fmt_hm(min: i32) -> String {
    let sign = if min < 0 { "-" } else { "" };
    let a = min.abs();
    format!("{sign}{}:{:02}", a / 60, a % 60)
}

/// Minuten → Industriezeit mit Komma, z. B. 7,50.
pub fn fmt_industrial(min: i32) -> String {
    format!("{:.2}", min as f64 / 60.0).replace('.', ",")
}
