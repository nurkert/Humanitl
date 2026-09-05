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
        last: 19,
        note: "001-006 Datei, Schlüssel, Wertebereiche, 007-009 Profile (HUM-066)",
    },
    AreaInfo {
        area: "sandbox",
        prefix: "SANDBOX",
        first: 1,
        last: 29,
        note: "001-006 Launcher und Profil, 007 Bridge-Richtung, 010-012 Start-Fehler, \
               020-025 /work-Härtung (HUM-043)",
    },
    AreaInfo {
        area: "proxy",
        prefix: "PROXY",
        first: 1,
        last: 19,
        note: "Anfragen, Caps, Protokoll, 010-011 Grenzen der Verbindung (HUM-120)",
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
        last: 19,
        note: "001-008 Regeldatei und Muster, 009-011 Regelspeicher (HUM-027)",
    },
    AreaInfo {
        area: "findings",
        prefix: "FINDINGS",
        first: 1,
        last: 9,
        note: "Detektoren für Secrets und personenbezogene Daten",
    },
    AreaInfo {
        area: "catalog",
        prefix: "CATALOG",
        first: 1,
        last: 9,
        note: "Gebündelter Domain-Katalog und Rangliste",
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
    AreaInfo {
        area: "ui",
        prefix: "UI",
        first: 1,
        last: 9,
        note: "Oberflaeche und was ihr die Arbeitsumgebung verweigert",
    },
    AreaInfo {
        area: "agent",
        prefix: "AGENT",
        first: 1,
        last: 9,
        note: "Agent-Adapter: Startkommando, Vorlagen, Vorprüfung vor dem Start",
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
    /// Die `Rules`-Anfrage lässt sich so nicht ausführen: keine Operation, eine
    /// fehlende oder unlesbare Regel, eine unbekannte Regel-Id, oder der Daemon
    /// läuft ohne Regelspeicher. Eine abgelehnte Anfrage ändert nichts
    /// (HUM-027).
    IPC_005 => "ipc", "Rules-Anfrage ungültig", "#ipc_005";
    /// Den RPC gibt es, aber dieser Daemon hat nicht, was er dafür braucht:
    /// keine Endpunkt-Probe etwa, weil sich der Verbindungsstapel aus der
    /// Konfiguration nicht bauen ließ. Der Unterschied zu `UNIMPLEMENTED` ist
    /// für den Client wichtig: Das eine ändert sich mit einem Update, das
    /// andere mit dem Start (HUM-039).
    IPC_006 => "ipc", "Fähigkeit in diesem Daemon nicht verfügbar", "#ipc_006";

    /// `config.toml` ließ sich nicht lesen.
    CONFIG_001 => "config", "Config-Datei ungültig", "#config_001";
    /// Ein Schlüssel steht nicht im Schema.
    CONFIG_002 => "config", "Unbekannter Schlüssel", "#config_002";
    /// Ein Wert liegt außerhalb des erlaubten Bereichs.
    CONFIG_003 => "config", "Wert außerhalb des Bereichs", "#config_003";
    /// `$XDG_RUNTIME_DIR` fehlt; ein Ersatzverzeichnis unter `/run/user` oder `$TMPDIR` wird genutzt (Info).
    CONFIG_004 => "config", "Laufzeitverzeichnis ist ein Ersatz", "#config_004";
    /// Ein veralteter Schlüssel ist in Gebrauch: als Alias, dann steht der
    /// kanonische Name im Befund (Info), oder ersatzlos entfallen, dann nennt
    /// der Befund das Issue und der Wert wird übergangen (Warning, HUM-101).
    CONFIG_005 => "config", "Veralteter Schlüssel", "#config_005";
    /// Alter und neuer Schlüssel sind gleichzeitig gesetzt; der kanonische gewinnt (Warning).
    CONFIG_006 => "config", "Alter und neuer Schlüssel gesetzt", "#config_006";
    /// Das Projekt-Profil liegt in einer Datei, die einem anderen Konto gehört (Warning, HUM-066).
    CONFIG_007 => "config", "Projekt-Profil gehört einem anderen Konto", "#config_007";
    /// Ein eigenes Profil verdeckt ein mitgeliefertes mit demselben Namen; die Datei gewinnt (Info, HUM-066).
    CONFIG_008 => "config", "Eigenes Profil verdeckt ein mitgeliefertes", "#config_008";
    /// Das Projekt-Profil nennt ein Profil, das nicht gilt: ein Projekt darf nur ein
    /// mitgeliefertes wählen, und die Kommandozeile geht vor (Warning, HUM-066).
    CONFIG_009 => "config", "Profilwunsch des Projekts gilt nicht", "#config_009";

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
    /// Eine Pflicht-Maske oder eine andere Maske unter `/work` wurde per
    /// `mounts.unmask` freigegeben; der Agent kann die Datei lesen und
    /// beschreiben (Warning, HUM-043).
    SANDBOX_020 => "sandbox", "Maskierter Pfad freigegeben", "#sandbox_020";
    /// Der Kernel kennt `openat2` nicht; der Lauf über das Projektverzeichnis
    /// nimmt den Weg über `openat` je Bestandteil (Info, HUM-043).
    SANDBOX_021 => "sandbox", "Kernel ohne openat2", "#sandbox_021";
    /// Der Agent hat einen Symlink angelegt, dessen Ziel außerhalb von `/work`
    /// liegt (Warning, HUM-043).
    SANDBOX_022 => "sandbox", "Symlink zeigt aus dem Projekt hinaus", "#sandbox_022";
    /// Im Diff des Sandbox-Laufs stecken mögliche Geheimnisse (Warning,
    /// HUM-043).
    SANDBOX_023 => "sandbox", "Mögliche Geheimnisse im Projekt", "#sandbox_023";
    /// Ein Budget hat gegriffen: Der Schnappschuss des Projektverzeichnisses
    /// ist unvollständig (Info, HUM-043).
    SANDBOX_024 => "sandbox", "Schnappschuss abgeschnitten", "#sandbox_024";
    /// Der Agent hat unter einem Pfad geschrieben, den das Profil überdeckt,
    /// den es aber im Projekt nicht gab: Ohne vorhandenen Mountpoint hängt
    /// `bwrap` kein `tmpfs` und keine Maske darüber (Warning, HUM-043).
    SANDBOX_025 => "sandbox", "Ohne Maske ins Projekt geschrieben", "#sandbox_025";
    /// Der Lauf hat eine Datei hinterlassen, die dieser Rechner von selbst
    /// ausführt: ein Git-Hook, ein `Makefile`, ein `package.json` mit
    /// `postinstall`, eine Workflow-Datei. Nicht geblockt, sondern gelistet
    /// (Warning, HUM-043).
    SANDBOX_026 => "sandbox", "Datei im Projekt, die der Rechner ausführt", "#sandbox_026";
    /// Zu dieser Sandbox-Kennung liegt keine Zusammenfassung vor: Der Lauf ist
    /// älter als die Aufzeichnung, endete ohne Zusammenfassung, oder die
    /// Kennung gehört zu keinem Lauf dieses Rechners (Error, HUM-043).
    SANDBOX_027 => "sandbox", "Keine Zusammenfassung zu diesem Lauf", "#sandbox_027";
    /// Der Lauf hat Dateien geändert, in die der Fundscan nicht gesehen hat:
    /// zu groß, nicht lesbar, oder das Byte-Budget war aufgebraucht. In ihnen
    /// wurde nichts gefunden, weil in ihnen nichts gesucht wurde — wie groß
    /// eine Datei ist und welche Rechte sie trägt, bestimmt der Agent
    /// (Warning, HUM-043).
    SANDBOX_028 => "sandbox", "Geänderte Datei nicht durchsucht", "#sandbox_028";

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
    /// Die aufgelöste Zieladresse liegt in einem privaten Netz (RFC 1918,
    /// Loopback, Link-Local, CGNAT oder `fc00::/7`), und keine Regel hat dieses
    /// Ziel geöffnet. Die Verbindung kommt auch dann nicht zustande, wenn ein
    /// Mensch die Anfrage gerade freigegeben hat: `allow_private` hängt an einer
    /// Regel, nicht an einer Entscheidung (ADR-006).
    ///
    /// Der Befund nennt die Adresse und schlägt eine Regel mit `action: ask` und
    /// `allow_private: true` vor. Damit wird genau dieses Ziel geöffnet, und die
    /// Anfrage wird trotzdem jedes Mal einem Menschen gezeigt; ein `allow` wäre
    /// mehr Öffnung als die Freigabe, die gerade gescheitert ist (HUM-102).
    ///
    /// Der Vorschlag fehlt, wo sich keine Regel bauen ließe, die wirkt: bei Port
    /// `0`, den `parse_rules` ablehnt, und bei einer Methode, gegen die
    /// überhaupt keine Regel matcht. Dann steht der Grund im `why` statt eines
    /// Knopfes. Ebenso sagt das `why`, wie weit die Regel reicht, wenn der Pfad
    /// kein Präfix hergibt, und dass sie vor die Regel gehört, die gerade
    /// entschieden hat.
    ///
    /// Die Adresse steht im Befund und in `resolved_ip`, nie im Rumpf der
    /// Antwort an den Client und nie in einer Kopfzeile: Die Sandbox hat keinen
    /// Resolver, und die Zuordnung von Name zu privater Adresse wäre für den
    /// Agenten neue Information über das lokale Netz.
    PROXY_008 => "proxy", "Private Zieladresse abgelehnt", "#proxy_008";

    /// Der Client vertraut der mitgelieferten CA nicht.
    TLS_001 => "tls", "Client hat Humanitl-CA abgelehnt", "#tls_001";
    /// Ein Client in der Sandbox bricht den TLS-Handschlag zu demselben Host
    /// wiederholt ab (dreimal in zehn Sekunden), ohne einen Alert zu schicken,
    /// der die CA nennt. Das deutet auf Certificate Pinning oder auf ein
    /// Werkzeug, das die CA-Umgebungsvariablen nicht liest (HUM-045).
    TLS_002 => "tls", "Client bricht den Handschlag wiederholt ab", "#tls_002";
    /// Der Client hat im `ClientHello` keinen Namen genannt (keine SNI),
    /// obwohl der Tunnel zu einem DNS-Namen führt. Der Handschlag kommt
    /// zustande, aber keine Anfrage darin lässt sich dem Tunnelziel zuordnen;
    /// alle werden abgelehnt (HUM-045, HUM-023).
    ///
    /// **Bereich `Session`, nicht `Flow`.** Der Katalog in `backlog/sprint-3.md`
    /// HUM-068 führt `TLS_001..003` gemeinsam unter `Flow`. Für `TLS_001` und
    /// `TLS_002` stimmt das: Sie hängen am Flow des gescheiterten `CONNECT`.
    /// `TLS_003` entsteht dagegen, wenn der Handschlag gerade *gelungen* ist
    /// und noch keine Anfrage darin steht; welcher Flow daraus wird, ist offen,
    /// und es werden meist mehrere. Der Proxy schickt ihn deshalb mit
    /// `flow_id = None` in den Ereignisstrom. Wer `DiagnosticScope` baut
    /// (HUM-068), trägt hier `Session` ein und nicht `Flow`.
    TLS_003 => "tls", "Client ohne SNI", "#tls_003";
    /// Das CA-Verzeichnis oder eine Datei darin ließ sich nicht anlegen, schreiben oder umbenennen (HUM-014).
    TLS_004 => "tls", "CA-Verzeichnis nicht beschreibbar", "#tls_004";
    /// `ca.key` oder `ca.crt` fehlt, ist unlesbar, passt nicht zusammen oder hat unsichere Rechte (HUM-014).
    TLS_005 => "tls", "CA-Dateien unbrauchbar", "#tls_005";

    /// Der LLM-Endpunkt aus `llm.endpoint` antwortet nicht: die TCP-Verbindung
    /// kam nicht zustande, der Name löste nicht auf, oder die Frist der Probe
    /// lief ab. Blockierend, weil der Agent ohne Modell nichts tun kann
    /// (HUM-039).
    LLM_001 => "llm", "LLM-Endpoint nicht erreichbar", "#llm_001";
    /// Der LLM-Endpunkt antwortet, verlangt aber eine Anmeldung (`401` oder
    /// `403`). Humanitl schickt im MVP keine Zugangsdaten an das Modell
    /// (HUM-039).
    ///
    /// **Der Titel hat sich mit HUM-039 geschärft.** Er lautete
    /// „LLM-Endpoint antwortet nicht als OpenAI-kompatible API"; diese
    /// Bedeutung trägt jetzt `LLM_003`, und die Aufteilung folgt der Tabelle in
    /// `backlog/sprint-3.md` unter HUM-039. Die Nummer wird damit nicht
    /// wiederverwendet: bis HUM-039 hat sie kein Codepfad je ausgegeben, es
    /// gibt also keine ältere Meldung, keinen Screenshot und keine Zeile in
    /// `audit.jsonl`, die etwas anderes bedeuten könnte.
    LLM_002 => "llm", "LLM-Endpoint verlangt eine Anmeldung", "#llm_002";
    /// Die Verbindung steht, aber weder `/api/tags` (Ollama) noch `/v1/models`
    /// (OpenAI-kompatibel) hat geantwortet. Meist zeigt die Adresse auf eine
    /// Oberfläche statt auf die Wurzel der API (HUM-039).
    LLM_003 => "llm", "LLM-Endpoint antwortet nicht als bekannte API", "#llm_003";

    /// `rules.yaml` ließ sich nicht lesen: die Datei fehlt, ist nicht lesbar,
    /// ist kein gültiges YAML oder passt nicht zum Schema. Der Befund nennt
    /// `Zeile:Spalte` und den Feldpfad.
    RULES_001 => "rules", "Regel-Datei ungültig", "#rules_001";
    /// Ein Host-Muster sieht nach einem Fehler oder nach Täuschung aus: ein
    /// Punycode-Literal (`xn--`), das ein anderer Name sein könnte als der
    /// gemeinte, oder eine IP-Adresse an der Stelle eines Host-Globs. Das ist
    /// eine Warnung; die Regel bleibt gültig (HUM-022).
    RULES_002 => "rules", "Host-Muster verdächtig (xn--, IP in Host-Glob)", "#rules_002";
    // RULES_004 bleibt frei: `backlog/sprint-2.md` führt darunter die Warnung
    // vor einem Punycode-Literal, die dieses Register seit HUM-063 unter
    // RULES_002 kennt. Eine Nummer zweimal zu vergeben wäre schlimmer als eine
    // Lücke, und eine Nummer wird nie wiederverwendet.
    /// Ein Host-Muster ließ sich nicht lesen: ein Stern steht nicht als ganzes
    /// Label (`*foo.com`), ein Label ist leer (`foo..com`) oder das Muster ist
    /// kein Host, kein Glob und keine Adresse (HUM-022).
    RULES_003 => "rules", "Host-Muster ungültig", "#rules_003";
    /// Ein Pfadmuster ließ sich nicht übersetzen: der reguläre Ausdruck hinter
    /// `~` ist ungültig oder überschreitet die Größengrenze, oder der Glob ist
    /// kein gültiges Muster (HUM-022).
    RULES_005 => "rules", "Pfadmuster ungültig", "#rules_005";
    /// `version` fehlt in `rules.yaml` oder ist nicht `1`.
    RULES_006 => "rules", "Version der Regel-Datei unbekannt", "#rules_006";
    /// Zwei Regeln tragen dieselbe `id`.
    RULES_007 => "rules", "Doppelte Regel-Id", "#rules_007";
    /// Eine Regel erlaubt mehr, als sie vermutlich soll: `host: "**"` ohne
    /// weitere Einschränkung zusammen mit `action: allow` hebt die Moderation
    /// für jeden DNS-Host auf. Das ist eine Warnung, keine Ablehnung.
    RULES_008 => "rules", "Regel wirkt zu breit", "#rules_008";
    /// `rules.yaml` ließ sich nicht schreiben: das Verzeichnis fehlt, die
    /// Rechte reichen nicht, die Platte ist voll. Die Datei bleibt dabei
    /// unangetastet, weil der Regelsatz erst in eine Nebendatei geht und dann
    /// umbenannt wird; die Änderung gilt deshalb auch im Speicher nicht
    /// (HUM-027).
    RULES_009 => "rules", "Regel-Datei nicht schreibbar", "#rules_009";
    /// Eine mitgelieferte Regel (`bundled: true`) soll gelöscht oder geändert
    /// werden. Mitgelieferte Regeln gehören nicht dem Nutzer; wer sie
    /// aufheben will, legt davor eine eigene Regel mit demselben Muster an
    /// (HUM-027).
    RULES_010 => "rules", "Mitgelieferte Regel ist unveränderlich", "#rules_010";
    /// Der Regelsatz wurde aus `rules.yaml` neu geladen. Der Befund nennt,
    /// was sich dabei geändert hat; er ist eine Information, kein Fehler
    /// (HUM-027).
    RULES_011 => "rules", "Regelsatz neu geladen", "#rules_011";
    /// Der Probelauf konnte die aufgezeichneten Flows nicht lesen. Die
    /// Antwort trägt dann keine Treffer und keine geprüfte Zeile; ohne diesen
    /// Befund läse der Regel-Bildschirm eine gezählte Null, hinter der man
    /// Grün vermuten könnte (`backlog/CONVENTIONS.md` 4.13, HUM-033).
    RULES_012 => "rules", "Probelauf konnte die Aufzeichnung nicht lesen", "#rules_012";

    /// Das eingebaute Regel-Set der Secret-Detektoren ließ sich nicht lesen
    /// oder eines seiner Muster nicht übersetzen. Das ist ein Fehler im
    /// Daemon, kein Zustand der Anfrage: ohne Regel-Set gibt es keine Suche
    /// nach Secrets, und die Suche wird nicht stillschweigend übersprungen.
    FINDINGS_001 => "findings", "Detektor-Regeln unbrauchbar", "#findings_001";
    /// Die Anfrage wurde nur teilweise durchsucht: der Body liegt über
    /// `limits.preview_cap_bytes`, das Entpacken lief über
    /// `limits.max_decompress_ratio`, oder der Body trägt eine Kodierung, für
    /// die es keinen Entpacker gibt, beziehungsweise einen beschädigten Strom.
    /// Angezeigte Funde sind dann unvollständig.
    FINDINGS_002 => "findings", "Scan unvollständig", "#findings_002";

    /// `catalog/domains.yaml` fehlt oder lässt sich nicht als Katalog lesen:
    /// unbekannte `version`, ungültiges YAML, doppelte `id`, ein Host-Muster,
    /// das kein Name und kein Label-Glob ist. Der Daemon läuft dann mit einem
    /// leeren Katalog weiter; jede Domain steht als unbekannt da, und keine
    /// wird als bekannt ausgegeben (HUM-031).
    CATALOG_001 => "catalog", "Domain-Katalog nicht lesbar", "#catalog_001";
    /// `catalog/tranco-top100k.csv.gz` fehlt oder lässt sich nicht lesen:
    /// beschädigter Gzip-Strom, eine Zeile ohne `rang,domain`, oder die Datei
    /// überschreitet die Grenzen für entpackte Größe und Zeilenzahl. Der
    /// Daemon läuft dann ohne Ränge weiter; das Panel zeigt „unranked" statt
    /// einer geratenen Zahl (HUM-031).
    CATALOG_002 => "catalog", "Rangliste nicht lesbar", "#catalog_002";

    /// Es gibt bereits einen schreibenden Terminal-Client.
    TERM_001 => "terminal", "Zweiter schreibender Terminal-Client abgelehnt", "#term_001";
    /// Das Terminal der Sandbox nimmt weder Eingabe noch Geometrie an: Die
    /// Sitzung läuft ohne Pseudoterminal, oder der Agent hat sich beendet und
    /// der Kernel meldet `EIO`. Wer zusieht, verliert dabei nichts; wer
    /// schreibt, erfährt, dass es niemanden mehr gibt, der liest (HUM-042).
    TERM_002 => "terminal", "Terminal der Sandbox nicht erreichbar", "#term_002";

    /// Die Aufzeichnung ließ sich nicht öffnen: das Datenverzeichnis oder der
    /// Blob-Speicher ist nicht anlegbar, die Datenbankdatei nicht lesbar oder
    /// beschreibbar, oder eine Migration schlug fehl. Ohne Aufzeichnung gilt
    /// die Zusage „alles wird aufgezeichnet" nicht mehr (HUM-026).
    RECORDER_001 => "recorder", "Aufzeichnung nicht verfügbar", "#recorder_001";
    /// Ein Filterausdruck für `ListFlows` ließ sich nicht lesen: unbekannter
    /// Schlüssel, fehlender Wert, unbrauchbare Zahl oder Zeitangabe. Der Befund
    /// nennt den beanstandeten Term und die gültigen Schlüssel (HUM-026).
    RECORDER_002 => "recorder", "Filter ungültig", "#recorder_002";
    /// Ein Schreibvorgang der Aufzeichnung schlug fehl. Der Schreib-Thread
    /// lebt weiter, der betroffene Datensatz fehlt aber; der Befund geht in
    /// den Ereignisstrom, damit die Lücke sichtbar ist (HUM-026).
    RECORDER_003 => "recorder", "Aufzeichnung konnte nicht schreiben", "#recorder_003";
    /// Ein Body ließ sich nicht in den Blob-Speicher schreiben oder von dort
    /// lesen: fehlende Datei, falsche Rechte, volle Platte (HUM-026).
    RECORDER_004 => "recorder", "Blob-Speicher nicht benutzbar", "#recorder_004";

    /// Die Hash-Kette in `audit.jsonl` passt nicht mehr zusammen.
    AUDIT_001 => "audit", "Hash-Kette gebrochen", "#audit_001";

    /// Der Daemon hat geantwortet, aber den Aufruf abgelehnt: der Aufruf
    /// selbst passt nicht zum Zustand des Daemons.
    CLI_001 => "cli", "Aufruf am Daemon abgelehnt", "#cli_001";
    /// `--ask terminal` steht für diesen Lauf nicht zur Verfügung.
    ///
    /// Zwei Gründe, und der zweite bleibt: Solange die Kommandozeile kein PTY
    /// anhängt (HUM-042), gibt es überhaupt kein Terminal, in dem die Frage
    /// stehen könnte; danach bleibt sie für Vollbild-TUI-Agenten wie `OpenCode`
    /// verwehrt, weil die Frage dort nicht zu sehen wäre
    /// (`backlog/CONVENTIONS.md` 4.10). Der Befund schlägt in beiden Fällen
    /// `--ask ui` und `--ask none` vor.
    CLI_002 => "cli", "`--ask terminal` ist hier nicht möglich", "#cli_002";
    /// Das Unterkommando steht im Vertrag, aber noch nicht in diesem Binary.
    CLI_003 => "cli", "Unterkommando noch nicht verfügbar", "#cli_003";
    /// Die Kommandozeile ließ sich nicht lesen: unbekanntes Unterkommando, fehlendes oder unlesbares Argument (HUM-064).
    CLI_004 => "cli", "Aufruf ungültig", "#cli_004";
    /// Die Arbeitsumgebung bietet keinen Platz fuer ein Anzeigesymbol
    /// (GNOME ohne die AppIndicator-Erweiterung). Die Anwendung laeuft
    /// weiter, der Zaehler steht im Fenstertitel; der Fix verweist auf die
    /// Erweiterung (HUM-034).
    UI_002 => "ui", "Kein Platz für das Anzeigesymbol", "#ui_002";

    // HUM-037: der Agent-Adapter und seine Vorprüfung. Neue Einträge stehen am
    // Ende des Registers, nicht bei ihrem Bereich; die Reihenfolge im Quelltext
    // sagt nichts aus, `docs/DIAGNOSTICS.md` gruppiert nach Bereich.
    /// Das Kommando des Agenten ist auf diesem Rechner nicht zu finden: weder
    /// im `$PATH` des Hosts noch als `agent.command`. Ohne Kommando gibt es
    /// nichts zu starten, deshalb ist der Befund blockierend (HUM-037).
    AGENT_001 => "agent", "Agent-Kommando nicht gefunden", "#agent_001";
    /// `agent.command` zeigt auf eine Datei, die es nicht gibt oder die nicht
    /// ausführbar ist. Die Sandbox startet trotzdem, weil der Pfad in der
    /// Sandbox ein anderer sein kann als auf dem Host; scheitert das `exec`,
    /// meldet der Shim es mit seinem eigenen Exit-Code (HUM-037).
    AGENT_002 => "agent", "Agent-Kommando nicht ausführbar", "#agent_002";
    /// Eine mitgelieferte Vorlage des Adapters (`opencode.json.tmpl`,
    /// `models.json`) ließ sich nicht als JSON lesen oder hat nicht die Form,
    /// die der Adapter erwartet. Das ist ein Fehler im Build, keine
    /// Nutzereingabe: die Dateien liegen unter `agents/` und werden
    /// einkompiliert (HUM-037).
    AGENT_003 => "agent", "Gebündelte Agenten-Vorlage unbrauchbar", "#agent_003";
    /// Es ist kein Modell konfiguriert (`llm.models` ist leer). Der Adapter
    /// trägt ein Platzhalter-Modell in die Konfiguration des Agenten ein,
    /// damit er überhaupt startet; ob der LLM-Server dieses Modell kennt, weiß
    /// Humanitl nicht (HUM-037, HUM-039).
    LLM_004 => "llm", "Kein Modell konfiguriert", "#llm_004";
    /// Das Kommando des Agenten liegt auf dem Host, aber an einer Stelle, die
    /// die Sandbox nicht einhängt. In der Sandbox scheitert dann das `exec`,
    /// und zwar erst nach dem Start. Der Befund nennt den gefundenen Pfad und
    /// einen Ort, an dem er erreichbar wäre (HUM-037).
    AGENT_004 => "agent", "Agent-Kommando in der Sandbox nicht erreichbar", "#agent_004";

    // HUM-039: die Durchreiche zum Sprachmodell und die Probe ihres Endpunkts.
    /// Eine durchgereichte Anfrage an das Sprachmodell trägt Funde: mögliche
    /// Geheimnisse oder personenbezogene Daten. Sie wird trotzdem gesendet,
    /// weil der LLM-Endpunkt die erklärte Vertrauensgrenze ist (BACKLOG.md
    /// 4.2); der Befund ist die Warnung, die davon übrig bleibt, und steht als
    /// bernsteinfarbene Zeile am Fluss. Er nennt die Zahl der Funde und den
    /// Host, nie den gefundenen Wert (HUM-039).
    LLM_005 => "llm", "Funde in einer durchgereichten Anfrage", "#llm_005";
    /// Der Endpunkt aus `llm.endpoint` liegt nicht in einem privaten Netz.
    ///
    /// Als privat zählen zwei Wege, und beide genügen für sich: die aufgelöste
    /// Adresse in RFC 1918, Loopback, Link-Local oder CGNAT, oder der Name
    /// `localhost` beziehungsweise ein Name unter `.local`, `.lan`,
    /// `.home.arpa` oder `.internal`. Die Spezifikation von HUM-039 nennt nur
    /// die ersten drei Suffixe; `localhost` und `.internal` kamen bei der
    /// Umsetzung dazu, weil beide dasselbe meinen und ein Mensch sie tippt
    /// (`backlog/CONVENTIONS.md` 4.21).
    ///
    /// Der Befund ist erlaubt und nur ein Hinweis, aber ein wichtiger: Er
    /// heißt, dass der Verkehr an der Warteschlange vorbei den Rechner und das
    /// eigene Netz verlässt (HUM-039).
    LLM_006 => "llm", "LLM-Endpunkt liegt nicht in einem privaten Netz", "#llm_006";
    /// Die Adresse in `llm.endpoint` lässt sich gar nicht als HTTP-Adresse
    /// lesen: ein anderes Schema als `http` oder `https`, kein Host, oder
    /// überhaupt keine URL. Es wurde nichts gemessen und nichts verbunden —
    /// deshalb nicht `LLM_001` und nicht `LLM_003`, die beide eine Beobachtung
    /// am Endpunkt behaupten würden (HUM-039).
    LLM_007 => "llm", "LLM-Endpunkt ist keine lesbare HTTP-Adresse", "#llm_007";

    // HUM-067: `humanitl run`. Neue Einträge stehen am Ende dieser Gruppe.
    /// Der Daemon führt genau eine Sitzung, und sie läuft schon.
    ///
    /// Ein zweites `humanitl run` würde die erste nicht ersetzen und auch
    /// nicht daneben laufen; es bekäme eine Sandbox, die einem anderen
    /// Projektverzeichnis gehört. Der Befund nennt die Kennung der laufenden
    /// Sitzung. Ein Befehl zum Anhängen steht nicht darin: `attach` gibt es
    /// nicht, und die Oberfläche sieht die Sitzung ohnehin. Auch ein Vorschlag
    /// zum Beheben steht nicht darin, weil es keinen Befehl gibt, der eine
    /// fremde Sitzung beendet — wer sie gestartet hat, beendet sie dort
    /// (HUM-067).
    CLI_005 => "cli", "Es läuft schon eine Sitzung", "#cli_005";

    // HUM-103: der Meta-Fluss in der Historie.
    /// Etwas wollte einen Fluss als vom Proxy selbst beantwortet abschließen,
    /// dessen Anfrage nicht an den reservierten Namen `humanitl.internal` ging
    /// (`Flow::answer`).
    ///
    /// Der Weg von `Received` unmittelbar nach `Recorded` ist der einzige, der
    /// über keine Entscheidung führt, und er gehört allein dem Meta-Endpunkt:
    /// Über eine Meta-Anfrage entscheidet niemand, weil sie nirgendwo hingeht.
    /// Stünde er jeder Anfrage offen, wäre er ein Weg am Menschen vorbei. Der
    /// Befund ist deshalb ein Fehler im Daemon und keine Eingabe eines Nutzers;
    /// er trägt kein `fix`, weil nichts einzustellen ist (HUM-103).
    PROXY_009 => "proxy", "Anfrage ist keine Meta-Anfrage", "#proxy_009";

    // HUM-120: die drei unbewachten Spannen der Verbindung. Neue Einträge des
    // Bereichs `proxy` stehen am Ende dieser Gruppe.
    /// Der Accept-Loop hat eine Verbindung abgelehnt, weil
    /// `limits.max_client_connections` erreicht war. Der Client bekommt `503`
    /// und die Verbindung wird geschlossen; angenommen und liegen gelassen wird
    /// sie nicht (HUM-120).
    ///
    /// Der Befund gehört zur Sitzung und zu keinem Fluss: Es wurde keine
    /// Anfrage gelesen, es gibt also nichts, was jemand entscheiden könnte. Er
    /// nennt die Zahl der Ablehnungen seit der letzten Meldung, denn die
    /// einzelne Ablehnung sagt wenig und ihre Häufung alles.
    ///
    /// **Er wird bewusst zusammengefasst gemeldet.** Ein Befund je abgelehnter
    /// Verbindung wäre selbst der Angriff: Der Ereignisstrom der Oberfläche hat
    /// `limits.event_buffer` Plätze, und wer ihn überläuft, nimmt dem Menschen
    /// die Sicht auf die Flüsse, die es wirklich gibt. Die Grenze schützt den
    /// Host, und ihre Meldung darf ihn nicht an anderer Stelle wieder öffnen.
    PROXY_010 => "proxy", "Verbindungsgrenze erreicht", "#proxy_010";
    /// Der Client hat den Kopf seiner Anfrage vollständig geschickt und im
    /// Rumpf länger als `limits.body_timeout_secs` geschwiegen. Der Proxy
    /// antwortet mit `408` und schließt die Verbindung (HUM-120).
    ///
    /// Gemessen wird die Stille **zwischen zwei Stücken**, nicht die
    /// Gesamtdauer: Ein großer Upload darf so lange dauern, wie er dauert.
    ///
    /// Auch dieser Befund hängt an keinem Fluss. Der Fluss entsteht erst, wenn
    /// der Rumpf vollständig gepuffert ist — vorher gibt es keine Anfrage, die
    /// ein Mensch sehen könnte, und genau das war die Lücke: Die Verbindung
    /// blieb stehen, ohne dass irgendetwas davon sichtbar wurde. Er wird wie
    /// [`PROXY_010`] zusammengefasst gemeldet, aus demselben Grund.
    PROXY_011 => "proxy", "Anfrage-Rumpf ist stehengeblieben", "#proxy_011";
    // HUM-075: `humanitl doctor`. Ein Code je Prüfung, damit eine Zeile der
    // Ausgabe und ihr Befund nicht auseinanderlaufen können, dazu zwei für die
    // beiden Arten, nicht gemessen zu haben.
    /// `bubblewrap` fehlt oder ist älter als die Untergrenze des Launchers.
    ///
    /// Beide Fälle stehen unter demselben Code, weil derselbe Befehl beide
    /// behebt; welcher von beiden vorliegt, sagt das `why` und der Beleg der
    /// Zeile. Ohne `bwrap` gibt es keine Sandbox, deshalb blockierend
    /// (HUM-075).
    DOCTOR_001 => "doctor", "bubblewrap fehlt oder ist zu alt", "#doctor_001";
    /// Ein unprivilegierter Nutzer-Namensraum ließ sich nicht aufmachen.
    ///
    /// Gemessen wird mit `bwrap` und nicht mit `unshare`: Auf Ubuntu ab 23.10
    /// schränkt `AppArmor` unprivilegierte Namensräume ein, das ausgelieferte
    /// `bwrap` trägt dafür aber ein Profil. Der Befund nennt, was
    /// `/proc/sys/kernel/apparmor_restrict_unprivileged_userns` und
    /// `/proc/sys/kernel/unprivileged_userns_clone` dazu sagen, und
    /// unterscheidet dabei eine Datei, die es nicht gibt, von einer mit
    /// unbrauchbarem Wert (HUM-075).
    DOCTOR_002 => "doctor", "Nutzer-Namensräume nicht verfügbar", "#doctor_002";
    /// Der Kernel taugt nicht für den seccomp-Filter des Shims.
    ///
    /// Blockierend, wenn `/proc/self/status` gar kein Feld `Seccomp` führt:
    /// Dann ist der Kernel ohne `CONFIG_SECCOMP` gebaut, und der Shim kann
    /// seinen Filter nicht laden. Eine Warnung, wenn der Kernel älter ist als
    /// die Fassung, gegen die der Filter gemessen wurde (HUM-075).
    DOCTOR_003 => "doctor", "Kernel ohne brauchbares seccomp", "#doctor_003";
    /// `$XDG_RUNTIME_DIR` fehlt, gehört einem anderen oder steht offen.
    ///
    /// Dort liegen der Socket des Daemons und das Sitzungs-Token. Ein
    /// Verzeichnis, in das Gruppe oder Welt hineindarf, wäre der bequemste Weg
    /// an jeder Entscheidung vorbei; eines, das einem anderen Nutzer gehört,
    /// ebenso (HUM-075).
    DOCTOR_004 => "doctor", "Laufzeitverzeichnis fehlt oder ist nicht privat", "#doctor_004";
    /// Es gibt keine brauchbare systemd-Nutzersitzung.
    ///
    /// Nur eine Warnung: Ohne systemd startet man `humanitld` von Hand, und
    /// alles Weitere läuft genauso. Was fehlt, ist der Start beim Anmelden
    /// (HUM-075).
    DOCTOR_005 => "doctor", "Keine systemd-Nutzersitzung", "#doctor_005";
    /// Der Daemon antwortet nicht oder spricht einen anderen Vertrag.
    ///
    /// Der Befund des Verbindungsversuchs steht im `why`, mit seinem eigenen
    /// Code davor; der Doctor führt je Zeile genau einen Code, damit Zeile und
    /// Befund zusammenbleiben. Eine andere Major-Version ist blockierend, eine
    /// fehlende Verbindung nur eine Warnung: Der Doctor selbst läuft auch ohne
    /// Daemon, und genau dafür ist er da (HUM-075).
    DOCTOR_006 => "doctor", "Daemon nicht erreichbar oder anderer Vertrag", "#doctor_006";
    /// Das Kommando des Agenten liegt nicht im `PATH` des Hosts.
    ///
    /// Eine Warnung und kein Fehler: Der Pfad in der Sandbox kann ein anderer
    /// sein als der auf dem Host, und ob das `exec` gelingt, entscheidet der
    /// Shim. Ohne das Kommando läuft der Agent aber nicht (HUM-075, HUM-037).
    DOCTOR_007 => "doctor", "Agent-Kommando nicht im PATH", "#doctor_007";
    /// Der Endpunkt des Sprachmodells fehlt oder hat nicht geantwortet.
    ///
    /// Steht nur nach einer Messung. Der Befund der Endpunkt-Probe steht mit
    /// seinem Code im `why`. Wurde gar nicht gemessen, ist es `DOCTOR_013`
    /// (HUM-075, HUM-039).
    DOCTOR_008 => "doctor", "Sprachmodell nicht erreichbar", "#doctor_008";
    /// Die Arbeitsumgebung hat keinen Platz für das Anzeigesymbol.
    ///
    /// Entweder fehlt `libayatana-appindicator3`, oder die Sitzung ist GNOME,
    /// das seit 3.26 keinen eigenen Bereich für Anzeigesymbole mehr hat und
    /// die AppIndicator-Erweiterung braucht. Die Anwendung läuft weiter; der
    /// Zähler der wartenden Anfragen steht dann im Fenstertitel (HUM-075,
    /// HUM-034).
    DOCTOR_009 => "doctor", "Kein Platz für das Anzeigesymbol", "#doctor_009";
    /// Renderer und Grafiktreiber vertragen sich voraussichtlich nicht.
    ///
    /// Der bekannte Fall ist Impeller auf einem geladenen NVIDIA-Modul unter
    /// Wayland: Die Oberfläche startet und bleibt schwarz. Der Befund nennt
    /// den Schalter, mit dem sie ohne Impeller startet (HUM-075).
    DOCTOR_010 => "doctor", "Renderer und Grafiktreiber vertragen sich nicht", "#doctor_010";
    /// Im Datenverzeichnis ist wenig Platz.
    ///
    /// Die Aufzeichnung legt dort Flows und Bodies ab. Der Vorschlag ist eine
    /// kürzere Aufbewahrung, nicht das Abschalten der Aufzeichnung: Ohne sie
    /// gilt die Zusage „alles wird aufgezeichnet" nicht mehr (HUM-075,
    /// HUM-026).
    DOCTOR_011 => "doctor", "Wenig Platz im Datenverzeichnis", "#doctor_011";
    /// Eine Prüfung ließ sich auf diesem Rechner nicht durchführen.
    ///
    /// Der einzige ehrliche Ausgang, wenn die Quelle fehlt oder nicht lesbar
    /// ist: Eine Prüfung, die nicht nachsehen konnte, ist nicht grün. Das `why`
    /// nennt die Prüfung und den Grund, der `fix` den Befehl, den der Doctor
    /// versucht hat und den ein Mensch von Hand nachfahren kann (HUM-075).
    DOCTOR_012 => "doctor", "Prüfung nicht durchführbar", "#doctor_012";
    /// Der Endpunkt des Sprachmodells wurde nicht angesprochen.
    ///
    /// Nicht `DOCTOR_012`: Dort konnte der Doctor nicht nachsehen, hier wollte
    /// er nicht. Die Erreichbarkeit des Sprachmodells ist die einzige Prüfung,
    /// hinter der eine Verbindung stünde, und sie läuft nie als Nebenwirkung
    /// eines anderen Befehls oder beim Öffnen eines Bildschirms. Der `fix`
    /// nennt den Befehl, der sie auslöst (HUM-075, HUM-039).
    DOCTOR_013 => "doctor", "Sprachmodell nicht angesprochen", "#doctor_013";
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
