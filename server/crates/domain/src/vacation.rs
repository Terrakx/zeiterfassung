//! Urlaubsberechnung in Arbeitstagen.

use chrono::{Duration, NaiveDate};

use crate::schedule::WeekModel;

/// Arbeitstage laut Wochenmodell im Zeitraum (inklusive), Feiertage ausgenommen.
pub fn working_days(
    model: &WeekModel,
    from: NaiveDate,
    to: NaiveDate,
    holidays: &[NaiveDate],
) -> Vec<NaiveDate> {
    let mut out = Vec::new();
    let mut d = from;
    while d <= to {
        if model.is_working_day(d) && !holidays.contains(&d) {
            out.push(d);
        }
        d += Duration::days(1);
    }
    out
}

/// Urlaubstage, die ein Antrag von `from` bis `to` verbraucht.
pub fn vacation_days_in_range(
    model: &WeekModel,
    from: NaiveDate,
    to: NaiveDate,
    holidays: &[NaiveDate],
) -> f64 {
    working_days(model, from, to, holidays).len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vacation_skips_weekend_and_holiday() {
        let m = WeekModel::from_hours([8.0, 8.0, 8.0, 8.0, 8.0, 0.0, 0.0]);
        // Mo 26.10.2026 (Nationalfeiertag) bis Fr 30.10.2026
        let from = NaiveDate::from_ymd_opt(2026, 10, 24).unwrap(); // Samstag
        let to = NaiveDate::from_ymd_opt(2026, 10, 30).unwrap();
        let hol = [NaiveDate::from_ymd_opt(2026, 10, 26).unwrap()];
        assert_eq!(vacation_days_in_range(&m, from, to, &hol), 4.0);
    }
}
