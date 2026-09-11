//! Gesetzliche Feiertage nach § 7 ARG (Österreich).

use chrono::{Datelike, Duration, NaiveDate};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Holiday {
    pub date: NaiveDate,
    pub name: String,
}

/// Ostersonntag nach der Gauß'schen Osterformel (gregorianisch).
pub fn easter_sunday(year: i32) -> NaiveDate {
    let a = year % 19;
    let b = year / 100;
    let c = year % 100;
    let d = b / 4;
    let e = b % 4;
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let i = c / 4;
    let k = c % 4;
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let month = (h + l - 7 * m + 114) / 31;
    let day = ((h + l - 7 * m + 114) % 31) + 1;
    NaiveDate::from_ymd_opt(year, month as u32, day as u32).expect("valid easter date")
}

/// Alle gesetzlichen Feiertage nach § 7 Abs 1 bis 3 ARG für ein Jahr, sortiert.
pub fn austrian_holidays(year: i32) -> Vec<Holiday> {
    let easter = easter_sunday(year);
    let fixed = [
        (1, 1, "Neujahr"),
        (1, 6, "Heilige Drei Könige"),
        (5, 1, "Staatsfeiertag"),
        (8, 15, "Mariä Himmelfahrt"),
        (10, 26, "Nationalfeiertag"),
        (11, 1, "Allerheiligen"),
        (12, 8, "Mariä Empfängnis"),
        (12, 25, "Christtag"),
        (12, 26, "Stefanitag"),
    ];
    let mut out: Vec<Holiday> = fixed
        .iter()
        .map(|(m, d, n)| Holiday {
            date: NaiveDate::from_ymd_opt(year, *m, *d).expect("fixed holiday"),
            name: (*n).to_string(),
        })
        .collect();
    let moving = [
        (1, "Ostermontag"),
        (39, "Christi Himmelfahrt"),
        (50, "Pfingstmontag"),
        (60, "Fronleichnam"),
    ];
    for (offset, name) in moving {
        out.push(Holiday {
            date: easter + Duration::days(offset),
            name: name.to_string(),
        });
    }
    out.sort_by_key(|h| h.date);
    debug_assert!(out.iter().all(|h| h.date.year() == year));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn easter_known_dates() {
        assert_eq!(easter_sunday(2024), NaiveDate::from_ymd_opt(2024, 3, 31).unwrap());
        assert_eq!(easter_sunday(2025), NaiveDate::from_ymd_opt(2025, 4, 20).unwrap());
        assert_eq!(easter_sunday(2026), NaiveDate::from_ymd_opt(2026, 4, 5).unwrap());
        assert_eq!(easter_sunday(2027), NaiveDate::from_ymd_opt(2027, 3, 28).unwrap());
    }

    #[test]
    fn thirteen_holidays_per_year() {
        let h = austrian_holidays(2026);
        assert_eq!(h.len(), 13);
        let names: Vec<_> = h.iter().map(|x| x.name.as_str()).collect();
        assert!(names.contains(&"Fronleichnam"));
        let fronleichnam = h.iter().find(|x| x.name == "Fronleichnam").unwrap();
        assert_eq!(fronleichnam.date, NaiveDate::from_ymd_opt(2026, 6, 4).unwrap());
    }
}
