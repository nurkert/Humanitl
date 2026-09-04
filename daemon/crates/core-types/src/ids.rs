//! Typisierte Identifikatoren.
//!
//! Alle Ids sind Newtypes über `uuid::Uuid` in Version 7. Version 7 trägt die
//! Erzeugungszeit in den führenden Bytes, damit `Ord` über die Bytes zugleich
//! Zeitordnung ist: `ListFlows(since)` und `ORDER BY id` bleiben ohne
//! zusätzliche Spalte korrekt.
//!
//! Die Typen sind bewusst nicht ineinander konvertierbar. Eine `RuleId` an
//! einer Stelle, die eine `FlowId` erwartet, ist ein Compile-Fehler.

use core::fmt;
use core::str::FromStr;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Ein Text ließ sich nicht als Id lesen.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid {kind} {input:?}: {reason}")]
pub struct IdParseError {
    /// Name des Zieltyps, zum Beispiel `FlowId`.
    pub kind: &'static str,
    /// Der abgelehnte Text.
    pub input: String,
    /// Warum der Text abgelehnt wurde.
    pub reason: &'static str,
}

macro_rules! typed_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            /// Erzeugt eine neue, zeitgeordnete Id (UUID v7).
            #[must_use]
            pub fn new() -> Self {
                Self(Uuid::now_v7())
            }

            /// Die Nil-Id (nur Nullen). Platzhalter für „noch unbekannt",
            /// etwa in Ereignissen ohne Flow-Bezug.
            #[must_use]
            pub const fn nil() -> Self {
                Self(Uuid::nil())
            }

            /// Übernimmt eine bereits vorhandene UUID, ohne die Version zu prüfen.
            ///
            /// Gedacht für Daten, die aus Datenbank, Konfiguration oder gRPC kommen
            /// und dort schon als UUID vorliegen.
            #[must_use]
            pub const fn from_uuid(uuid: Uuid) -> Self {
                Self(uuid)
            }

            /// Die zugrunde liegende UUID.
            #[must_use]
            pub const fn as_uuid(&self) -> Uuid {
                self.0
            }

            /// Liest eine Id aus ihrer Textform (mit oder ohne Bindestriche).
            ///
            /// # Errors
            ///
            /// [`IdParseError`], wenn der Text keine UUID ist. Die Version wird
            /// nicht geprüft: fremde Bestände dürfen v4-Ids enthalten.
            pub fn parse(s: &str) -> Result<Self, IdParseError> {
                Uuid::parse_str(s).map(Self).map_err(|_| IdParseError {
                    kind: stringify!($name),
                    input: s.to_owned(),
                    reason: "not a uuid",
                })
            }
        }

        impl Default for $name {
            /// Erzeugt eine neue Id, siehe [`Self::new`].
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0.hyphenated(), f)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}({})", stringify!($name), self.0.hyphenated())
            }
        }

        impl FromStr for $name {
            type Err = IdParseError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::parse(s)
            }
        }
    };
}

typed_id! {
    /// Id einer einzelnen HTTP-Anfrage durch den Proxy.
    FlowId
}

typed_id! {
    /// Id einer Regel aus `rules.yaml` oder aus einer Nutzerentscheidung.
    RuleId
}

typed_id! {
    /// Id einer Sitzung: ein Lauf von `humanitl run` bis zum Ende des Agenten.
    SessionId
}

typed_id! {
    /// Id einer gestarteten Sandbox.
    SandboxId
}

typed_id! {
    /// Id einer Bitte des Agenten über `POST http://humanitl.internal/ask`
    /// (HUM-073, ADR-014).
    ///
    /// Eine Bitte ist kein Flow: Sie hält nichts an, sie entscheidet nichts,
    /// und der Zustandsautomat kennt sie nicht. Sie braucht trotzdem eine
    /// Kennung, damit die Oberfläche zwei gleichlautende Bitten
    /// auseinanderhalten und eine Karte wiederfinden kann.
    AskId
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::{FlowId, RuleId};

    #[test]
    fn new_ids_are_time_ordered() {
        let first = FlowId::new();
        let second = FlowId::new();
        assert!(second > first, "{second:?} must sort after {first:?}");
    }

    #[test]
    fn display_is_hyphenated_and_round_trips() {
        let id = RuleId::new();
        let text = id.to_string();
        assert_eq!(text.len(), 36);
        assert_eq!(text.matches('-').count(), 4);
        assert_eq!(RuleId::parse(&text), Ok(id));
        assert_eq!(text.parse::<RuleId>(), Ok(id));
    }

    #[test]
    fn parse_rejects_garbage() {
        let err = FlowId::parse("not-a-uuid").expect_err("must fail");
        assert_eq!(err.kind, "FlowId");
        assert_eq!(err.input, "not-a-uuid");
        assert!(err.to_string().contains("not a uuid"));
    }

    #[test]
    fn nil_is_stable() {
        assert_eq!(
            FlowId::nil().to_string(),
            "00000000-0000-0000-0000-000000000000"
        );
    }

    #[test]
    fn new_ids_are_version_7() {
        assert_eq!(FlowId::new().as_uuid().get_version_num(), 7);
    }
}
