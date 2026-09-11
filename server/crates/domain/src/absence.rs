//! Abwesenheits- und Nichtleistungsarten mit BMD-Zuordnung.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AbsenceKind {
    Urlaub,
    Krank,
    Arbeitsunfall,
    Freizeitunfall,
    Pflegeurlaub,
    Zeitausgleich,
    Absonderung,
    Sonderurlaub,
    PersFeiertag,
    Arzt,
    Unbezahlt,
    Dienstreise,
}

impl AbsenceKind {
    pub const ALL: [AbsenceKind; 12] = [
        AbsenceKind::Urlaub,
        AbsenceKind::Krank,
        AbsenceKind::Arbeitsunfall,
        AbsenceKind::Freizeitunfall,
        AbsenceKind::Pflegeurlaub,
        AbsenceKind::Zeitausgleich,
        AbsenceKind::Absonderung,
        AbsenceKind::Sonderurlaub,
        AbsenceKind::PersFeiertag,
        AbsenceKind::Arzt,
        AbsenceKind::Unbezahlt,
        AbsenceKind::Dienstreise,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            AbsenceKind::Urlaub => "urlaub",
            AbsenceKind::Krank => "krank",
            AbsenceKind::Arbeitsunfall => "arbeitsunfall",
            AbsenceKind::Freizeitunfall => "freizeitunfall",
            AbsenceKind::Pflegeurlaub => "pflegeurlaub",
            AbsenceKind::Zeitausgleich => "zeitausgleich",
            AbsenceKind::Absonderung => "absonderung",
            AbsenceKind::Sonderurlaub => "sonderurlaub",
            AbsenceKind::PersFeiertag => "pers_feiertag",
            AbsenceKind::Arzt => "arzt",
            AbsenceKind::Unbezahlt => "unbezahlt",
            AbsenceKind::Dienstreise => "dienstreise",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|k| k.as_str() == s)
    }

    pub fn label(self) -> &'static str {
        match self {
            AbsenceKind::Urlaub => "Urlaub",
            AbsenceKind::Krank => "Krankenstand",
            AbsenceKind::Arbeitsunfall => "Arbeitsunfall",
            AbsenceKind::Freizeitunfall => "Freizeitunfall",
            AbsenceKind::Pflegeurlaub => "Pflegeurlaub",
            AbsenceKind::Zeitausgleich => "Zeitausgleich",
            AbsenceKind::Absonderung => "Behördliche Absonderung",
            AbsenceKind::Sonderurlaub => "Sonderurlaub",
            AbsenceKind::PersFeiertag => "Persönlicher Feiertag",
            AbsenceKind::Arzt => "Arztbesuch",
            AbsenceKind::Unbezahlt => "Unbezahlter Urlaub",
            AbsenceKind::Dienstreise => "Dienstreise",
        }
    }

    /// BMD-NLZ-Typ (Zehner/Einer-Stelle von NLZ_K). `None` = kein Export.
    pub fn bmd_nlz_type(self) -> Option<u8> {
        match self {
            AbsenceKind::Urlaub => Some(1),
            AbsenceKind::Krank => Some(2),
            AbsenceKind::Arbeitsunfall => Some(3),
            AbsenceKind::Arzt => Some(4),
            AbsenceKind::Pflegeurlaub => Some(5),
            AbsenceKind::Absonderung => Some(6),
            AbsenceKind::Freizeitunfall => Some(14),
            AbsenceKind::PersFeiertag => Some(15),
            AbsenceKind::Zeitausgleich => None, // läuft über den Gutstundentopf
            AbsenceKind::Sonderurlaub => None,
            AbsenceKind::Unbezahlt => None,
            AbsenceKind::Dienstreise => None,
        }
    }

    /// Zählt als Sollzeit-Erfüllung (bezahlte Nichtleistungszeit).
    pub fn counts_as_worked(self) -> bool {
        !matches!(self, AbsenceKind::Unbezahlt)
    }

    /// Verbraucht Urlaubstage.
    pub fn consumes_vacation(self) -> bool {
        matches!(self, AbsenceKind::Urlaub | AbsenceKind::PersFeiertag)
    }

    /// Verbraucht Gutstunden.
    pub fn consumes_credit_hours(self) -> bool {
        matches!(self, AbsenceKind::Zeitausgleich)
    }

    /// Darf ein Mitarbeiter diese Art selbst beantragen?
    pub fn employee_may_request(self) -> bool {
        matches!(
            self,
            AbsenceKind::Urlaub
                | AbsenceKind::Zeitausgleich
                | AbsenceKind::Pflegeurlaub
                | AbsenceKind::PersFeiertag
                | AbsenceKind::Sonderurlaub
                | AbsenceKind::Arzt
                | AbsenceKind::Dienstreise
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AbsenceUnit {
    Tag,
    HalberTag,
    Stunden,
}

impl AbsenceUnit {
    pub fn as_str(self) -> &'static str {
        match self {
            AbsenceUnit::Tag => "tag",
            AbsenceUnit::HalberTag => "halber_tag",
            AbsenceUnit::Stunden => "stunden",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "tag" => AbsenceUnit::Tag,
            "halber_tag" => AbsenceUnit::HalberTag,
            "stunden" => AbsenceUnit::Stunden,
            _ => return None,
        })
    }
}
