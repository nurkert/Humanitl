//! Alte Schlüsselnamen, die weiter funktionieren.
//!
//! `backlog/CONVENTIONS.md` 4.4 hat alle Caps und Zeitgrenzen in die Gruppe
//! `limits` gezogen. Wer eine `config.toml` aus der Zeit davor hat, soll sie
//! nicht anfassen müssen. Ein Alias ist deshalb ein vollständiger Pfad, der auf
//! einen kanonischen Pfad zeigt; das Laden ersetzt ihn, bevor irgendetwas
//! gemischt wird.
//!
//! Der Alias liegt in einer anderen Gruppe als sein Ziel (`hold.body_cap_bytes`
//! gegen `limits.hold_body_cap_bytes`). `#[serde(alias)]` kann das nicht, weil
//! es nur Namen innerhalb einer Struktur umbenennt. Die Ersetzung passiert
//! darum in [`mod@crate::load`] auf der Ebene der Pfade.
//!
//! Regel bei Streit: innerhalb einer Ebene gewinnt der kanonische Schlüssel;
//! über Ebenen hinweg gilt die Präzedenz, auch wenn die höhere Ebene den alten
//! Namen benutzt. In beiden Fällen legt das Laden eine Warnung dazu, die den
//! Gewinner nennt. Still verlieren soll keiner der beiden.
//!
//! Daneben steht [`RETIRED`]: Pfade, die es einmal gab und die **ersatzlos**
//! entfallen sind. Sie haben kein Ziel, auf das ein Alias zeigen könnte, und
//! sie sind trotzdem kein Tippfehler — die Datei des Nutzers war gestern
//! gültig. Das Laden übergeht sie mit einer Warnung, statt den Start
//! abzubrechen (`backlog/CONVENTIONS.md` 4.25). Jeder Eintrag nennt dabei die
//! Form, die der Schlüssel hatte ([`RetiredShape`]): Sie entscheidet, ob das,
//! was in einer alten Datei unter dem Pfad steht, mit ihm verworfen wird oder
//! weiterhin hart scheitert.

/// Ein alter Pfad und sein heutiger Name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Alias {
    /// Der alte Pfad, zum Beispiel `hold.body_cap_bytes`.
    pub old: &'static str,
    /// Der heutige Pfad, zum Beispiel `limits.hold_body_cap_bytes`.
    pub canonical: &'static str,
    /// Das Issue, das die Umbenennung entschieden hat.
    pub since: &'static str,
}

/// Alle Aliasse, sortiert nach dem alten Pfad.
pub static ALIASES: &[Alias] = &[
    Alias {
        old: "hold.body_cap_bytes",
        canonical: "limits.hold_body_cap_bytes",
        since: "HUM-057",
    },
    Alias {
        old: "ipc.event_buffer",
        canonical: "limits.event_buffer",
        since: "HUM-057",
    },
    Alias {
        old: "preview.cap_bytes",
        canonical: "limits.preview_cap_bytes",
        since: "HUM-057",
    },
    Alias {
        old: "preview.max_decompress_ratio",
        canonical: "limits.max_decompress_ratio",
        since: "HUM-057",
    },
    Alias {
        old: "recorder.max_body_bytes",
        canonical: "limits.recorder_max_body_bytes",
        since: "HUM-057",
    },
    Alias {
        old: "upstream.connect_timeout_secs",
        canonical: "limits.connect_timeout_secs",
        since: "HUM-057",
    },
];

/// Die Form, die ein entfallener Schlüssel hatte.
///
/// Sie entscheidet, was mit dem passiert, was in einer alten Datei **unter**
/// dem Pfad steht. Der Pfad allein reicht dafür nicht:
///
/// - Unter einem [`Scalar`](RetiredShape::Scalar) hat es nie etwas gegeben.
///   `limits.idle_timeout_secs` war eine Zahl; `[limits.idle_timeout_secs]`
///   mit Feldern darunter ist keine alte, gültige Datei, sondern eine
///   Struktur, die das Schema nie kannte. Sie scheitert weiterhin hart, wie
///   jeder unbekannte Schlüssel.
/// - Unter einer [`FreeTable`](RetiredShape::FreeTable) gehörte alles dem
///   Schlüssel. `experimental.upstream_port_map` war eine freie Tabelle, ihre
///   Schlüssel waren Portnummern des Nutzers; `upstream_port_map.extra` ist
///   ein Eintrag dieser Tabelle und kein eigener Pfad. Sie wird als ein
///   Eintrag verworfen, mit einer Warnung.
///
/// Kurz: Was unter einem entfallenen Skalar steht, hat es nie gegeben und
/// scheitert; was unter einer entfallenen Tabelle stand, gehörte ihr und wird
/// mit ihr verworfen (`backlog/CONVENTIONS.md` 4.25).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetiredShape {
    /// Ein einzelner Wert: eine Zahl, ein Text, ein Schalter.
    Scalar,
    /// Eine freie Tabelle, deren Schlüssel dem Nutzer gehörten.
    FreeTable,
}

/// Ein Pfad, den es einmal gab und der ersatzlos entfallen ist.
///
/// Kein Alias: Es gibt keinen Nachfolger, auf den das Laden ihn abbilden
/// könnte. Der Eintrag existiert, damit der Unterschied zwischen „gibt es
/// nicht" und „gab es einmal" sichtbar bleibt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Retired {
    /// Der Pfad, wie er in alten Dateien steht.
    pub path: &'static str,
    /// Das Issue, das ihn entfernt hat.
    pub since: &'static str,
    /// Ein Satz, warum er weg ist; er steht im Befund.
    pub why: &'static str,
    /// Die Form, die der Schlüssel hatte. Sie entscheidet über alles, was in
    /// einer alten Datei unter dem Pfad steht; siehe [`RetiredShape`].
    pub shape: RetiredShape,
}

impl Retired {
    /// Ob alles unter diesem Pfad zu ihm gehörte und mit ihm verworfen wird.
    ///
    /// Nur die freie Tabelle. Unter einem Skalar hat es nie eine Ebene
    /// gegeben, und eine, die trotzdem dasteht, ist kein alter Wert, sondern
    /// ein unbekannter Schlüssel.
    #[must_use]
    pub const fn swallows_what_is_below(&self) -> bool {
        matches!(self.shape, RetiredShape::FreeTable)
    }
}

/// Alle entfallenen Pfade, sortiert.
///
/// Wer einen Schlüssel entfernt, hängt hier eine Zeile an. Ohne sie wird aus
/// dem entfernten Schlüssel beim nächsten Start ein harter `CONFIG_002`, und
/// der Nutzer trägt einen Fehler, den er nicht gemacht hat.
pub static RETIRED: &[Retired] = &[
    Retired {
        path: "experimental.upstream_port_map",
        since: "HUM-088",
        why: "the proxy never redirected a port, and no shipped test needs one: they bind the real port inside their own network namespace or address the ephemeral port directly",
        shape: RetiredShape::FreeTable,
    },
    Retired {
        path: "limits.idle_timeout_secs",
        since: "HUM-101",
        why: "it described the same span as limits.header_timeout_secs, the one idle clock of the connection to the agent",
        shape: RetiredShape::Scalar,
    },
];

/// Der Eintrag zu einem entfallenen Pfad.
#[must_use]
pub fn retired(path: &str) -> Option<&'static Retired> {
    RETIRED.iter().find(|entry| entry.path == path)
}

/// Der heutige Pfad zu einem alten, falls es einen gibt.
#[must_use]
pub fn canonical(path: &str) -> Option<&'static str> {
    ALIASES
        .iter()
        .find(|alias| alias.old == path)
        .map(|alias| alias.canonical)
}

/// Der Eintrag zu einem alten Pfad.
#[must_use]
pub fn lookup(path: &str) -> Option<&'static Alias> {
    ALIASES.iter().find(|alias| alias.old == path)
}

/// Alle alten Namen eines heutigen Pfades.
#[must_use]
pub fn old_names(canonical_path: &str) -> Vec<&'static str> {
    ALIASES
        .iter()
        .filter(|alias| alias.canonical == canonical_path)
        .map(|alias| alias.old)
        .collect()
}

/// Die Gruppen, die es nur noch als Alias gibt und deshalb nicht im Schema stehen.
///
/// `hold` und `recorder` tragen zwar auch alte Pfade, sind aber weiterhin
/// Gruppen des Schemas und stehen darum nicht in dieser Liste.
#[must_use]
pub fn legacy_groups() -> Vec<&'static str> {
    let mut groups: Vec<&'static str> = ALIASES
        .iter()
        .filter_map(|alias| alias.old.split_once('.').map(|(group, _)| group))
        .filter(|group| !crate::schema::field(group).is_some_and(|field| field.group))
        .collect();
    groups.sort_unstable();
    groups.dedup();
    groups
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::{ALIASES, RETIRED, RetiredShape, canonical, legacy_groups, old_names, retired};

    #[test]
    fn aliases_are_sorted_and_unique() {
        let mut sorted: Vec<&str> = ALIASES.iter().map(|alias| alias.old).collect();
        let original = sorted.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            sorted, original,
            "ALIASES must be sorted and free of doubles"
        );
    }

    #[test]
    fn no_alias_points_at_another_alias() {
        for alias in ALIASES {
            assert_eq!(
                canonical(alias.canonical),
                None,
                "{} points at another alias",
                alias.old
            );
        }
    }

    #[test]
    fn old_names_finds_both_directions() {
        assert_eq!(
            canonical("hold.body_cap_bytes"),
            Some("limits.hold_body_cap_bytes")
        );
        assert_eq!(
            old_names("limits.hold_body_cap_bytes"),
            vec!["hold.body_cap_bytes"]
        );
        assert_eq!(canonical("hold.timeout_secs"), None);
    }

    #[test]
    fn retired_paths_are_sorted_unique_and_gone_from_the_schema() {
        let mut sorted: Vec<&str> = RETIRED.iter().map(|entry| entry.path).collect();
        let original = sorted.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            sorted, original,
            "RETIRED must be sorted and free of doubles"
        );
        for entry in RETIRED {
            assert!(
                !crate::schema::known_paths().contains(entry.path),
                "{} is listed as retired and is still in the schema",
                entry.path
            );
            assert_eq!(
                canonical(entry.path),
                None,
                "{} is retired and an alias at the same time; decide which",
                entry.path
            );
            assert!(!entry.why.is_empty() && !entry.since.is_empty());
        }
        assert!(retired("limits.idle_timeout_secs").is_some());
        assert!(retired("limits.header_timeout_secs").is_none());
        // HUM-088: der zweite entfallene Pfad, und der erste, dessen Wert eine
        // Tabelle war. Das Laden muss ihn als einen Eintrag sehen, nicht als
        // eine Ebene mit Portnummern darunter.
        assert!(retired("experimental.upstream_port_map").is_some());
        assert!(retired("experimental.upstream_port_map.443").is_none());
        assert!(retired("experimental.h2_upstream").is_none());
    }

    #[test]
    fn the_shape_decides_what_happens_below_a_retired_path() {
        // Der Pfad allein reicht nicht. `limits.idle_timeout_secs` war eine
        // Zahl: Was darunter steht, hat es nie gegeben und muss weiterhin
        // hart scheitern. `experimental.upstream_port_map` war eine freie
        // Tabelle: Was darunter stand, gehörte ihr und wird mit ihr verworfen.
        let scalar = retired("limits.idle_timeout_secs").expect("the retired idle limit");
        assert_eq!(scalar.shape, RetiredShape::Scalar);
        assert!(!scalar.swallows_what_is_below());

        let table = retired("experimental.upstream_port_map").expect("the retired port map");
        assert_eq!(table.shape, RetiredShape::FreeTable);
        assert!(table.swallows_what_is_below());
        // Genau der Pfad, nicht sein Anfang und nicht seine Nachbarn: Ein
        // Vergleich mit `starts_with` machte aus jedem Tippfehler hinter einem
        // entfallenen Schlüssel eine Warnung, und die Milde für entfallene
        // Schlüssel griffe auf Tippfehler über (`backlog/CONVENTIONS.md` 4.25).
        assert!(retired("limits.idle_timeout_secsx").is_none());
        assert!(retired("limits.idle_timeout_secs.deeper").is_none());
        assert!(retired("limits.idle_timeout_sec").is_none());
        assert!(retired("").is_none());
    }

    #[test]
    fn legacy_groups_lists_the_groups_that_only_exist_as_alias() {
        // `hold` und `recorder` haben Aliasse, sind aber echte Gruppen.
        assert_eq!(legacy_groups(), vec!["ipc", "preview", "upstream"]);
    }
}
