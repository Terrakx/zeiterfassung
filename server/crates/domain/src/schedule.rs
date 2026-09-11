//! Wochenmodell und Stempelungen.

use chrono::{Datelike, NaiveDate, NaiveDateTime, Weekday};
use serde::{Deserialize, Serialize};

/// Sollminuten je Wochentag, Index 0 = Montag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeekModel {
    pub minutes: [i32; 7],
}

impl WeekModel {
    pub fn from_hours(hours: [f64; 7]) -> Self {
        let mut minutes = [0; 7];
        for (i, h) in hours.iter().enumerate() {
            minutes[i] = (h * 60.0).round() as i32;
        }
        Self { minutes }
    }

    pub fn target_for(&self, date: NaiveDate) -> i32 {
        self.minutes[weekday_index(date.weekday())]
    }

    pub fn weekly_minutes(&self) -> i32 {
        self.minutes.iter().sum()
    }

    pub fn is_working_day(&self, date: NaiveDate) -> bool {
        self.target_for(date) > 0
    }

    /// Anzahl der Arbeitstage pro Woche laut Modell.
    pub fn working_days_per_week(&self) -> u32 {
        self.minutes.iter().filter(|m| **m > 0).count() as u32
    }
}

pub fn weekday_index(w: Weekday) -> usize {
    w.num_days_from_monday() as usize
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PunchKind {
    ClockIn,
    ClockOut,
    BreakStart,
    BreakEnd,
}

impl PunchKind {
    pub fn as_str(self) -> &'static str {
        match self {
            PunchKind::ClockIn => "kommen",
            PunchKind::ClockOut => "gehen",
            PunchKind::BreakStart => "pause_start",
            PunchKind::BreakEnd => "pause_ende",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "kommen" => PunchKind::ClockIn,
            "gehen" => PunchKind::ClockOut,
            "pause_start" => PunchKind::BreakStart,
            "pause_ende" => PunchKind::BreakEnd,
            _ => return None,
        })
    }
}

/// Eine Stempelung in Lokalzeit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Punch {
    pub at: NaiveDateTime,
    pub kind: PunchKind,
}

/// Ordnet Stempelungen einem Schichttag zu: „Kommen“ eröffnet die Schicht an seinem Datum,
/// die folgenden Stempelungen gehören bis zum „Gehen“ zu diesem Tag, auch nach Mitternacht.
/// Liegen mehr als `max_gap_hours` zwischen zwei Stempelungen, gilt die Schicht als abgebrochen
/// (vergessenes Gehen) und die nächste Stempelung zählt zu ihrem eigenen Datum.
pub fn assign_shift_dates(punches: &[Punch], max_gap_hours: i64) -> Vec<NaiveDate> {
    let mut out = Vec::with_capacity(punches.len());
    let mut current: Option<(NaiveDate, NaiveDateTime)> = None;
    for p in punches {
        let date = match (p.kind, current) {
            (PunchKind::ClockIn, _) => {
                current = Some((p.at.date(), p.at));
                p.at.date()
            }
            (_, Some((d, last))) if (p.at - last).num_hours() < max_gap_hours => {
                current = Some((d, p.at));
                d
            }
            _ => {
                current = None;
                p.at.date()
            }
        };
        if p.kind == PunchKind::ClockOut {
            current = None;
        }
        out.push(date);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(d: u32, h: u32, m: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 9, d).unwrap().and_hms_opt(h, m, 0).unwrap()
    }

    #[test]
    fn night_shift_belongs_to_start_day() {
        let punches = [
            Punch { at: at(1, 22, 0), kind: PunchKind::ClockIn },
            Punch { at: at(2, 2, 0), kind: PunchKind::BreakStart },
            Punch { at: at(2, 2, 30), kind: PunchKind::BreakEnd },
            Punch { at: at(2, 6, 0), kind: PunchKind::ClockOut },
            Punch { at: at(2, 22, 0), kind: PunchKind::ClockIn },
            Punch { at: at(3, 6, 0), kind: PunchKind::ClockOut },
        ];
        let d = assign_shift_dates(&punches, 16);
        let day = |n| NaiveDate::from_ymd_opt(2026, 9, n).unwrap();
        assert_eq!(d, vec![day(1), day(1), day(1), day(1), day(2), day(2)]);
    }

    #[test]
    fn forgotten_clock_out_does_not_swallow_next_day() {
        let punches = [
            Punch { at: at(1, 8, 0), kind: PunchKind::ClockIn },
            Punch { at: at(2, 17, 0), kind: PunchKind::ClockOut }, // 33 h später: eigener Tag
        ];
        let d = assign_shift_dates(&punches, 16);
        assert_eq!(d[1], NaiveDate::from_ymd_opt(2026, 9, 2).unwrap());
    }

    #[test]
    fn week_model_basics() {
        let m = WeekModel::from_hours([8.0, 8.0, 8.0, 8.0, 8.0, 0.0, 0.0]);
        assert_eq!(m.weekly_minutes(), 2400);
        assert_eq!(m.working_days_per_week(), 5);
        let sat = NaiveDate::from_ymd_opt(2026, 9, 12).unwrap();
        assert!(!m.is_working_day(sat));
        let mon = NaiveDate::from_ymd_opt(2026, 9, 14).unwrap();
        assert_eq!(m.target_for(mon), 480);
    }
}
