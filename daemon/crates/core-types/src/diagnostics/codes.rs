//! Das Register aller Diagnose-Codes.
//!
//! Jeder Code steht genau einmal hier, mit Bereich, Titel und Anker in
//! `docs/DIAGNOSTICS.md`. Eine Nummer wird nie wiederverwendet: ein
//! zurückgezogener Code bleibt als `#[deprecated]` stehen, damit ein alter
//! Screenshot, ein Fehlerbericht oder eine Zeile in `audit.jsonl` weiterhin
//! eindeutig bleibt.
//!
//! Neue Codes kommen mit dem Issue, das den Fehlerpfad einführt, und zwar
//! innerhalb des reservierten Bereichs (siehe [`AREAS`]).

use super::DiagnosticCode;

/// Ein Eintrag des Registers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodeInfo {
    /// Der Code selbst.
    pub code: DiagnosticCode,
    /// Der Bereich in Kleinbuchstaben, zum Beispiel `sandbox`.
    pub area: &'static str,
    /// Der feste Teil der Meldung. Der veränderliche Teil ist `why`.
    pub title: &'static str,
    /// Anker in `docs/DIAGNOSTICS.md`, immer `#` plus Code in Kleinbuchstaben.
    pub docs_anchor: &'static str,
}

/// Ein reservierter Nummernbereich.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AreaInfo {
    /// Der Bereich in Kleinbuchstaben.
    pub area: &'static str,
    /// Das Präfix der Codes, in Großbuchstaben.
    pub prefix: &'static str,
    /// Kleinste Nummer des Bereichs.
    pub first: u16,
    /// Größte Nummer des Bereichs.
    pub last: u16,
    /// Wofür der Bereich gedacht ist.
    pub note: &'static str,
}

/// Die reservierten Bereiche (`backlog/CONVENTIONS.md` 4.6).
///
/// Ein Code außerhalb seines Bereichs ist ein Fehler; der Test
/// `codes_stay_inside_their_area` prüft das.
pub static AREAS: &[AreaInfo] = &[
    AreaInfo {
        area: "daemon",
        prefix: "DAEMON",
        first: 1,
        last: 19,
        note: "Start, Erreichbarkeit, Version des Daemons",
    },
    AreaInfo {
        area: "ipc",
        prefix: "IPC",
        first: 1,
        last: 9,
        note: "gRPC-Schnittstelle, Token, Aufrufe gegen den Zustand",
    },
    AreaInfo {
        area: "config",
        prefix: "CONFIG",
        first: 1,
        last: 9,
        note: "Konfigurationsdatei, Schlüssel, Wertebereiche",
    },
    AreaInfo {
        area: "sandbox",
        prefix: "SANDBOX",
        first: 1,
        last: 29,
        note: "001-006 Launcher und Profil, 007 Bridge-Richtung, 010-012 Start-Fehler",
    },
    AreaInfo {
        area: "proxy",
        prefix: "PROXY",
        first: 1,
        last: 9,
        note: "Anfragen, Caps, Protokoll",
    },
    AreaInfo {
        area: "tls",
        prefix: "TLS",
        first: 1,
        last: 9,
        note: "CA, Zertifikate, Handschlag",
    },
    AreaInfo {
        area: "llm",
        prefix: "LLM",
        first: 1,
        last: 9,
        note: "LLM-Endpunkt und seine Antworten",
    },
    AreaInfo {
        area: "rules",
        prefix: "RULES",
        first: 1,
        last: 9,
        note: "Regeldatei und Muster",
    },
    AreaInfo {
        area: "terminal",
        prefix: "TERM",
        first: 1,
        last: 9,
        note: "Terminal-Anbindung des Agenten",
    },
    AreaInfo {
        area: "recorder",
        prefix: "RECORDER",
        first: 1,
        last: 9,
        note: "Datenbank und Blob-Speicher",
    },
    AreaInfo {
        area: "limits",
        prefix: "LIMIT",
        first: 1,
        last: 9,
        note: "Budgets und Zeitgrenzen",
    },
    AreaInfo {
        area: "audit",
        prefix: "AUDIT",
        first: 1,
        last: 9,
        note: "Hash-Kette und Export",
    },
    AreaInfo {
        area: "doctor",
        prefix: "DOCTOR",
        first: 1,
        last: 19,
        note: "Selbsttest der Installation",
    },
    AreaInfo {
        area: "cli",
        prefix: "CLI",
        first: 1,
        last: 9,
        note: "Kommandozeile und ihre Vorbedingungen",
    },
];

macro_rules! registry {
    ($(
        $(#[$meta:meta])*
        $ident:ident => $area:literal, $title:literal, $anchor:literal;
    )*) => {
        $(
            $(#[$meta])*
            pub const $ident: DiagnosticCode = DiagnosticCode(stringify!($ident));
        )*

        /// Alle bekannten Codes, in der Reihenfolge des Registers.
        pub static CODES: &[CodeInfo] = &[
            $(
                CodeInfo {
                    code: $ident,
                    area: $area,
                    title: $title,
                    docs_anchor: $anchor,
                },
            )*
        ];
    };
}

registry! {
    /// Der Daemon läuft nicht oder der Socket antwortet nicht.
    DAEMON_001 => "daemon", "Daemon nicht erreichbar", "#daemon_001";
    /// Client und Daemon sprechen unterschiedliche Fassungen der Proto-Datei.
    DAEMON_002 => "daemon", "Proto-Version inkompatibel", "#daemon_002";
    /// Der Daemon-Socket ist bereits belegt (zweite Instanz oder verwaister Socket).
    DAEMON_003 => "daemon", "Socket bereits belegt", "#daemon_003";
    /// Laufzeitverzeichnis oder Socket-Datei konnte nicht angelegt werden.
    DAEMON_004 => "daemon", "Laufzeitverzeichnis oder Socket nicht anlegbar", "#daemon_004";

    /// Das Token aus `$XDG_RUNTIME_DIR/humanitl/token` fehlt oder passt nicht.
    IPC_001 => "ipc", "Ungültiges Token", "#ipc_001";
    /// `AllowEdited` kam für mehr als einen Flow; eine bearbeitete Anfrage gilt
    /// immer genau einem.
    IPC_002 => "ipc", "AllowEdited nur für genau einen Flow", "#ipc_002";
    /// Der Flow wartet nicht mehr; die Entscheidung kommt zu spät.
    IPC_003 => "ipc", "Flow nicht mehr gehalten", "#ipc_003";
    /// Die `Decide`-Anfrage lässt sich so nicht ausführen: keine Flow-Id, keine
    /// Entscheidung, eine unlesbare Flow-Id oder eine bearbeitete Anfrage, die
    /// sich nicht lesen lässt oder über `limits.hold_body_cap_bytes` liegt. Der
    /// Grund steht im Befund. Fehlt die Entscheidung, wird sie nie zu `Allow`
    /// ergänzt.
    IPC_004 => "ipc", "Decide-Anfrage ungültig", "#ipc_004";

    /// `config.toml` ließ sich nicht lesen.
    CONFIG_001 => "config", "Config-Datei ungültig", "#config_001";
    /// Ein Schlüssel steht nicht im Schema.
    CONFIG_002 => "config", "Unbekannter Schlüssel", "#config_002";
    /// Ein Wert liegt außerhalb des erlaubten Bereichs.
    CONFIG_003 => "config", "Wert außerhalb des Bereichs", "#config_003";
    /// `$XDG_RUNTIME_DIR` fehlt; ein Ersatzverzeichnis unter `/run/user` oder `$TMPDIR` wird genutzt (Info).
    CONFIG_004 => "config", "Laufzeitverzeichnis ist ein Ersatz", "#config_004";
    /// Ein veralteter Schlüssel (Alias) ist in Gebrauch; der kanonische Name steht im Befund (Info).
    CONFIG_005 => "config", "Veralteter Schlüssel", "#config_005";
    /// Alter und neuer Schlüssel sind gleichzeitig gesetzt; der kanonische gewinnt (Warning).
    CONFIG_006 => "config", "Alter und neuer Schlüssel gesetzt", "#config_006";

    /// `bwrap` ist nicht installiert oder liegt nicht im Pfad.
    SANDBOX_001 => "sandbox", "bwrap nicht gefunden", "#sandbox_001";
    /// Die gefundene `bwrap`-Version kann nicht alles, was das Profil verlangt.
    SANDBOX_002 => "sandbox", "bwrap-Version zu alt", "#sandbox_002";
    /// Der Kernel erlaubt keine unprivilegierten User-Namespaces.
    SANDBOX_003 => "sandbox", "User-Namespaces nicht erlaubt", "#sandbox_003";
    /// Eine der drei Garantien ließ sich in der laufenden Sandbox nicht zeigen.
    SANDBOX_004 => "sandbox", "Isolation-Check fehlgeschlagen", "#sandbox_004";
    /// Der Projektordner ist nicht beschreibbar, obwohl `work_mode = "rw"` gilt.
    SANDBOX_005 => "sandbox", "Projektordner nicht beschreibbar", "#sandbox_005";
    /// Ein Mount im Profil zeigt auf eine Host-Quelle, die nie in die Sandbox darf.
    SANDBOX_006 => "sandbox", "Mount verboten", "#sandbox_006";
    /// Eine Bridge im Profil hat eine Richtung, die es nicht gibt.
    SANDBOX_007 => "sandbox", "Bridge-Richtung unbekannt", "#sandbox_007";
    /// Die Argumentliste des Starters hat nicht die von HUM-010 erzeugte Form.
    SANDBOX_010 => "sandbox", "Argumentliste des Starters unerwartet", "#sandbox_010";
    /// Platzhalter für Socket, CA oder Shim konnten nicht angelegt werden.
    SANDBOX_011 => "sandbox", "Platzhalter nicht anlegbar", "#sandbox_011";
    /// Die Kommandozeile des Starters ist ungültig.
    SANDBOX_012 => "sandbox", "Kommandozeile des Starters ungültig", "#sandbox_012";
    /// Isolation-Check: der Shim hat keinen Prüfbericht geliefert.
    SANDBOX_013 => "sandbox", "Isolation-Check ohne Bericht", "#sandbox_013";
    /// Isolation-Check 1 fehlgeschlagen: ein Netzwerk-Interface außer `lo` existiert.
    SANDBOX_014 => "sandbox", "Isolation-Check 1: Netzwerk-Interface vorhanden", "#sandbox_014";
    /// Isolation-Check 2 fehlgeschlagen: mehr als ein Socket erreichbar.
    SANDBOX_015 => "sandbox", "Isolation-Check 2: mehr als eine Tür", "#sandbox_015";
    /// Isolation-Check 3 fehlgeschlagen: seccomp nicht aktiv oder Familien nicht gesperrt.
    SANDBOX_016 => "sandbox", "Isolation-Check 3: seccomp unwirksam", "#sandbox_016";

    /// Der Body ist größer als `limits.hold_body_cap_bytes`.
    PROXY_001 => "proxy", "Body über Cap", "#proxy_001";
    /// Der `Host`-Header widerspricht dem Ziel des TLS-Handschlags.
    PROXY_002 => "proxy", "Authority-Mismatch", "#proxy_002";
    /// Die Verbindung zum Ziel scheiterte (Auflösung, TCP, TLS, private Adresse
    /// oder Zeitüberschreitung); der Proxy antwortet dem Client mit `502` und
    /// verbucht den Flow als `Failed` (HUM-015, HUM-024).
    PROXY_003 => "proxy", "Upstream-Verbindung fehlgeschlagen", "#proxy_003";
    /// Der Zustandsautomat hat einen Übergang abgelehnt, den der Proxy versucht
    /// hat: ein Fehler im Daemon, kein Zustand des Clients. Der Flow wird
    /// fail-closed mit `Block` beendet und der Befund geht in den Ereignisstrom
    /// (HUM-016).
    PROXY_005 => "proxy", "Ungültiger Übergang im Flow", "#proxy_005";
    /// Der Client verlangt HTTP/2; in M1 bietet der Proxy nur `http/1.1` an.
    PROXY_007 => "proxy", "HTTP/2 nicht verfügbar", "#proxy_007";

    /// Der Client vertraut der mitgelieferten CA nicht.
    TLS_001 => "tls", "Client hat Humanitl-CA abgelehnt", "#tls_001";
    /// Das CA-Verzeichnis oder eine Datei darin ließ sich nicht anlegen, schreiben oder umbenennen (HUM-014).
    TLS_004 => "tls", "CA-Verzeichnis nicht beschreibbar", "#tls_004";
    /// `ca.key` oder `ca.crt` fehlt, ist unlesbar, passt nicht zusammen oder hat unsichere Rechte (HUM-014).
    TLS_005 => "tls", "CA-Dateien unbrauchbar", "#tls_005";

    /// Der LLM-Endpunkt aus `llm.endpoint` antwortet nicht.
    LLM_001 => "llm", "LLM-Endpoint nicht erreichbar", "#llm_001";
    /// Der LLM-Endpunkt antwortet, aber nicht wie eine OpenAI-kompatible API.
    LLM_002 => "llm", "LLM-Endpoint antwortet nicht als OpenAI-kompatible API", "#llm_002";

    /// `rules.yaml` ließ sich nicht lesen.
    RULES_001 => "rules", "Regel-Datei ungültig", "#rules_001";
    /// Ein Host-Muster sieht nach einem Fehler oder nach Täuschung aus.
    RULES_002 => "rules", "Host-Muster verdächtig (xn--, IP in Host-Glob)", "#rules_002";

    /// Es gibt bereits einen schreibenden Terminal-Client.
    TERM_001 => "terminal", "Zweiter schreibender Terminal-Client abgelehnt", "#term_001";

    /// Die Hash-Kette in `audit.jsonl` passt nicht mehr zusammen.
    AUDIT_001 => "audit", "Hash-Kette gebrochen", "#audit_001";

    /// Der Daemon hat geantwortet, aber den Aufruf abgelehnt: der Aufruf
    /// selbst passt nicht zum Zustand des Daemons.
    CLI_001 => "cli", "Aufruf am Daemon abgelehnt", "#cli_001";
    /// `--ask terminal` verträgt sich nicht mit einem Vollbild-TUI-Agenten.
    CLI_002 => "cli", "Vollbild-TUI-Agent nicht mit --ask terminal", "#cli_002";
    /// Das Unterkommando steht im Vertrag, aber noch nicht in diesem Binary.
    CLI_003 => "cli", "Unterkommando noch nicht verfügbar", "#cli_003";
}

/// Sucht einen Code im Register.
#[must_use]
pub fn lookup(code: DiagnosticCode) -> Option<&'static CodeInfo> {
    CODES.iter().find(|info| info.code == code)
}

/// Sucht einen Code im Register anhand seiner Textform.
#[must_use]
pub fn lookup_str(code: &str) -> Option<&'static CodeInfo> {
    CODES.iter().find(|info| info.code.as_str() == code)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::collections::BTreeSet;

    use super::{AREAS, CODES, DAEMON_001, lookup, lookup_str};

    #[test]
    fn codes_are_unique() {
        let unique: BTreeSet<&str> = CODES.iter().map(|info| info.code.as_str()).collect();
        assert_eq!(unique.len(), CODES.len(), "a code appears twice");
    }

    #[test]
    fn codes_follow_schema() {
        for info in CODES {
            let text = info.code.as_str();
            let Some((prefix, number)) = text.split_once('_') else {
                panic!("{text} has no underscore");
            };
            assert!(!prefix.is_empty(), "{text} has an empty area");
            assert!(
                prefix.chars().all(|c| c.is_ascii_uppercase()),
                "{text} must use A-Z for the area"
            );
            assert_eq!(number.len(), 3, "{text} needs three digits");
            assert!(
                number.chars().all(|c| c.is_ascii_digit()),
                "{text} needs three digits"
            );
        }
    }

    #[test]
    fn anchors_match_the_code() {
        for info in CODES {
            assert_eq!(
                info.docs_anchor,
                format!("#{}", info.code.as_str().to_lowercase()),
                "{} has a stale anchor",
                info.code
            );
        }
    }

    #[test]
    fn codes_stay_inside_their_area() {
        let prefixes: BTreeSet<&str> = AREAS.iter().map(|area| area.prefix).collect();
        assert_eq!(prefixes.len(), AREAS.len(), "an area prefix appears twice");

        for info in CODES {
            let text = info.code.as_str();
            let Some((prefix, number)) = text.split_once('_') else {
                panic!("{text} has no underscore");
            };
            let Some(area) = AREAS.iter().find(|area| area.prefix == prefix) else {
                panic!("{text} has no reserved area in AREAS");
            };
            assert_eq!(area.area, info.area, "{text} names another area");
            let Ok(number) = number.parse::<u16>() else {
                panic!("{text} has no number");
            };
            assert!(
                (area.first..=area.last).contains(&number),
                "{text} is outside {}..={}",
                area.first,
                area.last
            );
        }
    }

    #[test]
    fn lookup_finds_registered_codes() {
        let Some(info) = lookup(DAEMON_001) else {
            panic!("DAEMON_001 must be registered");
        };
        assert_eq!(info.title, "Daemon nicht erreichbar");
        assert_eq!(lookup_str("DAEMON_001"), Some(info));
        assert_eq!(lookup_str("DAEMON_999"), None);
    }
}
