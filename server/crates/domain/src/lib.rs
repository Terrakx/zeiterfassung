//! Rechenkern der Zeiterfassung. Reine Funktionen ohne IO, vollständig testbar.
//!
//! Alle Zeiten sind Minuten (i32) oder lokale `NaiveDateTime` (Europe/Vienna).
//! Die Umrechnung von UTC in Lokalzeit passiert außerhalb dieses Crates.

pub mod absence;
pub mod day;
pub mod holidays;
pub mod schedule;
pub mod vacation;

pub use absence::{AbsenceKind, AbsenceUnit};
pub use day::{compute_day, DayInput, DayResult, PauseRule, Warning};
pub use holidays::{austrian_holidays, easter_sunday, Holiday};
pub use schedule::{assign_shift_dates, Punch, PunchKind, WeekModel};
pub use vacation::{vacation_days_in_range, working_days};
