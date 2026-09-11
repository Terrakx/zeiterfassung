//! Tagesberechnung: Stempelpaare → Ist, Pausen, Warnungen.

use chrono::{NaiveDate, NaiveDateTime, NaiveTime, Weekday, Datelike};
use serde::{Deserialize, Serialize};

use crate::absence::AbsenceKind;
use crate::schedule::{Punch, PunchKind};

/// Regel für die automatische Pausenanrechnung nach § 11 AZG.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PauseRule {
    /// Ab dieser Arbeitszeit (Minuten) muss eine Pause vorliegen. Gesetz: 360.
    pub threshold_min: i32,
    /// Mindestpause in Minuten. Gesetz: 30.
    pub required_min: i32,
    /// Fehlende Pause automatisch abziehen (nur zulässig bei betrieblich festgelegter Pausenlage).
    pub auto_deduct: bool,
}

impl Default for PauseRule {
    fn default() -> Self {
        Self { threshold_min: 360, required_min: 30, auto_deduct: false }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum Warning {
    /// Kommen ohne Gehen.
    OpenShift,
    /// Stempelung passt nicht zum Zustand (z. B. zweimal Kommen).
    InvalidSequence { at: NaiveDateTime, kind: PunchKind },
    /// Pause fehlt oder ist zu kurz, nicht automatisch abgezogen.
    MissingBreak { worked_min: i32, break_min: i32 },
    /// Pause wurde automatisch abgezogen.
    AutoBreakDeducted { minutes: i32 },
    /// Tagesarbeitszeit über 10 Stunden.
    Over10h { worked_min: i32 },
    /// Tagesarbeitszeit über 12 Stunden (§ 9 AZG).
    Over12h { worked_min: i32 },
    /// Ruhezeit zum Vortag unter 11 Stunden (§ 12 AZG).
    RestTimeShort { rest_min: i32 },
    /// Arbeit an einem Feiertag.
    WorkOnHoliday,
    /// Arbeit an einem Sonntag.
    WorkOnSunday,
    /// Abwesenheit und Stempelung am selben Tag.
    AbsenceAndPunches,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbsenceMinutes {
    pub kind: AbsenceKind,
    pub minutes: i32,
}

#[derive(Debug, Clone)]
pub struct DayInput<'a> {
    pub date: NaiveDate,
    /// Sollminuten laut Wochenmodell (0 an freien Tagen). Feiertage setzen das Soll nicht auf 0,
    /// der Ausfall wird als bezahlte Feiertagszeit gerechnet.
    pub target_min: i32,
    pub is_holiday: bool,
    /// Stempelungen dieses Tages in Lokalzeit, aufsteigend sortiert.
    pub punches: &'a [Punch],
    /// Genehmigte Abwesenheiten mit bereits berechneten Minuten.
    pub absences: &'a [AbsenceMinutes],
    pub pause_rule: PauseRule,
    /// Ende der letzten Arbeitszeit des Vortages (Lokalzeit) für die Ruhezeitprüfung.
    pub previous_day_end: Option<NaiveDateTime>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DayResult {
    pub date: NaiveDate,
    pub target_min: i32,
    /// Tatsächlich gearbeitete Minuten ohne Pausen.
    pub worked_min: i32,
    pub break_min: i32,
    /// Bezahlte Nichtleistungszeit (Urlaub, Krank, Feiertag ...).
    pub paid_absence_min: i32,
    /// Feiertagsausfall in Minuten (Soll am Feiertag ohne Arbeit).
    pub holiday_min: i32,
    /// worked + paid_absence + holiday − target
    pub diff_min: i32,
    #[serde(serialize_with = "ser_hm", deserialize_with = "de_hm")]
    pub first_in: Option<NaiveTime>,
    #[serde(serialize_with = "ser_hm", deserialize_with = "de_hm")]
    pub last_out: Option<NaiveTime>,
    pub open_shift: bool,
    pub warnings: Vec<Warning>,
}

#[derive(Clone, Copy, PartialEq)]
enum State {
    Outside,
    Working,
    OnBreak,
}

pub fn compute_day(input: &DayInput) -> DayResult {
    let mut warnings = Vec::new();
    let mut worked = 0i32;
    let mut breaks = 0i32;
    let mut state = State::Outside;
    let mut seg_start: Option<NaiveDateTime> = None;
    let mut first_in = None;
    let mut last_out = None;

    for p in input.punches {
        match (state, p.kind) {
            (State::Outside, PunchKind::ClockIn) => {
                state = State::Working;
                seg_start = Some(p.at);
                if first_in.is_none() {
                    first_in = Some(p.at);
                }
            }
            (State::Working, PunchKind::ClockOut) => {
                worked += minutes_between(seg_start.take(), p.at);
                state = State::Outside;
                last_out = Some(p.at);
            }
            (State::Working, PunchKind::BreakStart) => {
                worked += minutes_between(seg_start.take(), p.at);
                seg_start = Some(p.at);
                state = State::OnBreak;
            }
            (State::OnBreak, PunchKind::BreakEnd) => {
                breaks += minutes_between(seg_start.take(), p.at);
                seg_start = Some(p.at);
                state = State::Working;
            }
            (State::OnBreak, PunchKind::ClockOut) => {
                // Gehen während der Pause: Pause endet mit dem Gehen.
                breaks += minutes_between(seg_start.take(), p.at);
                state = State::Outside;
                last_out = Some(p.at);
            }
            (_, kind) => {
                warnings.push(Warning::InvalidSequence { at: p.at, kind });
            }
        }
    }

    let open_shift = state != State::Outside;
    if open_shift {
        warnings.push(Warning::OpenShift);
    }

    // Pausenregel
    let rule = input.pause_rule;
    if worked > rule.threshold_min && breaks < rule.required_min {
        let missing = rule.required_min - breaks;
        if rule.auto_deduct {
            worked -= missing;
            breaks += missing;
            warnings.push(Warning::AutoBreakDeducted { minutes: missing });
        } else {
            warnings.push(Warning::MissingBreak { worked_min: worked, break_min: breaks });
        }
    }

    if worked > 720 {
        warnings.push(Warning::Over12h { worked_min: worked });
    } else if worked > 600 {
        warnings.push(Warning::Over10h { worked_min: worked });
    }

    if let (Some(prev_end), Some(fi)) = (input.previous_day_end, first_in) {
        let rest = (fi - prev_end).num_minutes() as i32;
        if rest < 660 {
            warnings.push(Warning::RestTimeShort { rest_min: rest });
        }
    }

    if worked > 0 {
        if input.is_holiday {
            warnings.push(Warning::WorkOnHoliday);
        } else if input.date.weekday() == Weekday::Sun {
            warnings.push(Warning::WorkOnSunday);
        }
    }

    let paid_absence: i32 = input
        .absences
        .iter()
        .filter(|a| a.kind.counts_as_worked())
        .map(|a| a.minutes)
        .sum();

    if paid_absence > 0 && worked > 0 && paid_absence >= input.target_min {
        warnings.push(Warning::AbsenceAndPunches);
    }

    // Feiertag: die ausgefallene Sollzeit gilt als bezahlt (§ 9 Abs 1 ARG). Arbeit am Feiertag
    // gebührt zusätzlich (§ 9 Abs 5 ARG) und erscheint daher als Plus.
    let holiday_min = if input.is_holiday {
        (input.target_min - paid_absence).max(0)
    } else {
        0
    };

    let diff = worked + paid_absence + holiday_min - input.target_min;

    DayResult {
        date: input.date,
        target_min: input.target_min,
        worked_min: worked,
        break_min: breaks,
        paid_absence_min: paid_absence,
        holiday_min,
        diff_min: diff,
        first_in: first_in.map(|d| d.time()),
        last_out: last_out.map(|d| d.time()),
        open_shift,
        warnings,
    }
}

fn ser_hm<S: serde::Serializer>(t: &Option<NaiveTime>, s: S) -> Result<S::Ok, S::Error> {
    match t {
        Some(t) => s.serialize_some(&t.format("%H:%M").to_string()),
        None => s.serialize_none(),
    }
}

fn de_hm<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<NaiveTime>, D::Error> {
    let s: Option<String> = serde::Deserialize::deserialize(d)?;
    Ok(s.and_then(|s| NaiveTime::parse_from_str(&s, "%H:%M").ok()))
}

fn minutes_between(start: Option<NaiveDateTime>, end: NaiveDateTime) -> i32 {
    match start {
        Some(s) => (end - s).num_minutes().max(0) as i32,
        None => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dt(h: u32, m: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 9, 14)
            .unwrap()
            .and_hms_opt(h, m, 0)
            .unwrap()
    }

    fn p(h: u32, m: u32, kind: PunchKind) -> Punch {
        Punch { at: dt(h, m), kind }
    }

    fn base<'a>(punches: &'a [Punch], absences: &'a [AbsenceMinutes]) -> DayInput<'a> {
        DayInput {
            date: NaiveDate::from_ymd_opt(2026, 9, 14).unwrap(),
            target_min: 480,
            is_holiday: false,
            punches,
            absences,
            pause_rule: PauseRule::default(),
            previous_day_end: None,
        }
    }

    #[test]
    fn normal_day_with_break() {
        let punches = [
            p(8, 0, PunchKind::ClockIn),
            p(12, 0, PunchKind::BreakStart),
            p(12, 30, PunchKind::BreakEnd),
            p(17, 0, PunchKind::ClockOut),
        ];
        let r = compute_day(&base(&punches, &[]));
        assert_eq!(r.worked_min, 510);
        assert_eq!(r.break_min, 30);
        assert_eq!(r.diff_min, 30);
        assert!(r.warnings.is_empty(), "{:?}", r.warnings);
        assert_eq!(r.first_in, Some(NaiveTime::from_hms_opt(8, 0, 0).unwrap()));
    }

    #[test]
    fn missing_break_warns_without_auto_deduct() {
        let punches = [p(8, 0, PunchKind::ClockIn), p(16, 0, PunchKind::ClockOut)];
        let r = compute_day(&base(&punches, &[]));
        assert_eq!(r.worked_min, 480);
        assert!(matches!(r.warnings[0], Warning::MissingBreak { .. }));
    }

    #[test]
    fn missing_break_auto_deducts() {
        let punches = [p(8, 0, PunchKind::ClockIn), p(16, 0, PunchKind::ClockOut)];
        let mut input = base(&punches, &[]);
        input.pause_rule.auto_deduct = true;
        let r = compute_day(&input);
        assert_eq!(r.worked_min, 450);
        assert_eq!(r.break_min, 30);
        assert_eq!(r.warnings, vec![Warning::AutoBreakDeducted { minutes: 30 }]);
    }

    #[test]
    fn short_day_needs_no_break() {
        let punches = [p(8, 0, PunchKind::ClockIn), p(13, 0, PunchKind::ClockOut)];
        let r = compute_day(&base(&punches, &[]));
        assert_eq!(r.worked_min, 300);
        assert!(r.warnings.is_empty());
    }

    #[test]
    fn open_shift_is_flagged() {
        let punches = [p(8, 0, PunchKind::ClockIn)];
        let r = compute_day(&base(&punches, &[]));
        assert!(r.open_shift);
        assert_eq!(r.worked_min, 0);
        assert!(r.warnings.contains(&Warning::OpenShift));
    }

    #[test]
    fn invalid_sequence_ignored() {
        let punches = [
            p(8, 0, PunchKind::ClockIn),
            p(9, 0, PunchKind::ClockIn),
            p(16, 30, PunchKind::ClockOut),
        ];
        let r = compute_day(&base(&punches, &[]));
        assert_eq!(r.worked_min, 510);
        assert!(matches!(r.warnings[0], Warning::InvalidSequence { .. }));
    }

    #[test]
    fn full_day_vacation() {
        let abs = [AbsenceMinutes { kind: AbsenceKind::Urlaub, minutes: 480 }];
        let r = compute_day(&base(&[], &abs));
        assert_eq!(r.paid_absence_min, 480);
        assert_eq!(r.diff_min, 0);
    }

    #[test]
    fn holiday_without_work_is_paid() {
        let mut input = base(&[], &[]);
        input.is_holiday = true;
        let r = compute_day(&input);
        assert_eq!(r.holiday_min, 480);
        assert_eq!(r.diff_min, 0);
    }

    #[test]
    fn holiday_with_work_is_plus_and_warns() {
        let punches = [p(8, 0, PunchKind::ClockIn), p(12, 0, PunchKind::ClockOut)];
        let mut input = base(&punches, &[]);
        input.is_holiday = true;
        let r = compute_day(&input);
        // Feiertagsentgelt (8h) + Arbeitsentgelt (4h) → diff = +4h (§ 9 Abs 5 ARG).
        assert_eq!(r.holiday_min, 480);
        assert_eq!(r.diff_min, 240);
        assert!(r.warnings.contains(&Warning::WorkOnHoliday));
    }

    #[test]
    fn rest_time_short() {
        let punches = [p(6, 0, PunchKind::ClockIn), p(12, 0, PunchKind::ClockOut)];
        let mut input = base(&punches, &[]);
        input.previous_day_end = Some(
            NaiveDate::from_ymd_opt(2026, 9, 13)
                .unwrap()
                .and_hms_opt(22, 0, 0)
                .unwrap(),
        );
        let r = compute_day(&input);
        assert!(r.warnings.contains(&Warning::RestTimeShort { rest_min: 480 }));
    }

    #[test]
    fn over_ten_hours() {
        let punches = [
            p(7, 0, PunchKind::ClockIn),
            p(12, 0, PunchKind::BreakStart),
            p(12, 30, PunchKind::BreakEnd),
            p(18, 0, PunchKind::ClockOut),
        ];
        let r = compute_day(&base(&punches, &[]));
        assert_eq!(r.worked_min, 630);
        assert!(r.warnings.contains(&Warning::Over10h { worked_min: 630 }));
    }
}
