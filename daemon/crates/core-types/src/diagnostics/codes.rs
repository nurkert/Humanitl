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
    /// Was den Befund auslöst, in einem Satz.
    ///
    /// Aus der Stelle gelesen, die ihn baut, nicht aus der Spezifikation: Der
    /// Satz sagt, wann dieser Code entsteht, damit ein Mensch von der Meldung
    /// zur Ursache kommt (HUM-068).
    pub trigger: &'static str,
    /// Was dagegen hilft, in einem Satz.
    ///
    /// Nennt die `FixAction`, die an dieser Stelle dranhängt, oder sagt
    /// ausdrücklich, dass es keine gibt und warum.
    pub fix_hint: &'static str,
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
        note: "001-004 Start, Erreichbarkeit, Version des Daemons, \
               005-008 die Nutzer-Unit von `daemon install` (HUM-044)",
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
        note: "001-006 Datei, Schlüssel, Wertebereiche, 007-009 Profile (HUM-066), \
               010-012 Test-Wurzel und ihr Flag (HUM-087), 013 der Projektordner \
               der Einrichtung (HUM-044)",
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
        $ident:ident => $area:literal, $title:literal, $anchor:literal,
            $trigger:literal, $fix_hint:literal;
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
                    trigger: $trigger,
                    fix_hint: $fix_hint,
                },
            )*
        ];
    };
}

registry! {
    /// Der Daemon läuft nicht oder der Socket antwortet nicht.
    DAEMON_001 => "daemon", "Daemon nicht erreichbar", "#daemon_001",
        "Der Client findet keinen Socket oder kein Token unter dem Laufzeitpfad, oder die Verbindung dorthin scheitert.",
        "`InstallService` richtet die Nutzer-Unit ein; sonst nennt der Befund `humanitld`.";
    /// Client und Daemon sprechen unterschiedliche Fassungen der Proto-Datei.
    DAEMON_002 => "daemon", "Proto-Version inkompatibel", "#daemon_002",
        "Der Daemon meldet eine andere Hauptversion des Vertrags als die Kommandozeile spricht.",
        "`CopyCommand`: beide Seiten auf denselben Stand bringen.";
    /// Der Daemon-Socket ist bereits belegt (zweite Instanz oder verwaister Socket).
    DAEMON_003 => "daemon", "Socket bereits belegt", "#daemon_003",
        "Auf dem Socket lauscht schon ein Daemon, oder eine verwaiste Datei liegt darauf.",
        "Kein Fix-Knopf: Der Text nennt die laufende Instanz und den Pfad.";
    /// Laufzeitverzeichnis oder Socket-Datei konnte nicht angelegt werden.
    DAEMON_004 => "daemon", "Laufzeitverzeichnis oder Socket nicht anlegbar", "#daemon_004",
        "Das Laufzeitverzeichnis fehlt, ist nicht privat, lässt sich nicht anlegen, oder der Socket-Pfad ist länger als `sun_path` erlaubt.",
        "Meist ohne Fix — der Text nennt Pfad und Grund; beim zu langen Pfad `SetEnv` für ein kürzeres `XDG_RUNTIME_DIR`.";
    // HUM-044: `humanitl daemon install`. Der Befehl schreibt eine Datei auf
    // den Rechner des Menschen; jeder Weg, auf dem das schiefgehen kann, hat
    // hier seinen eigenen Code, damit die Zeile in der Oberfläche sagt, was
    // wirklich passiert ist.
    /// Unter `~/.config/systemd/user/humanitld.service` liegt eine Unit, die
    /// Humanitl nicht geschrieben hat.
    ///
    /// Erkannt an der Marke in der ersten Zeile. Der Befehl weigert sich dann
    /// und überschreibt nichts: Die Datei bestimmt, was beim Anmelden startet,
    /// und wer sie von Hand geschrieben hat, hat einen Grund dafür gehabt. Der
    /// Vorschlag ist, sie beiseitezulegen (HUM-044).
    DAEMON_005 => "daemon", "Fremde Unit-Datei wird nicht überschrieben", "#daemon_005",
        "Unter dem Unit-Pfad liegt eine Datei, deren erste Zeile nicht die Marke von Humanitl trägt.",
        "`CopyCommand`, das die fremde Datei beiseitelegt (`mv … .bak`); ein `--force` gibt es mit Absicht nicht.";
    /// Die Unit-Datei ließ sich nicht schreiben.
    ///
    /// Das Verzeichnis war nicht anlegbar, die Datei nicht schreibbar oder das
    /// Umbenennen scheiterte. Geschrieben wird über eine Nachbardatei und
    /// `rename`, also bleibt im Fehlerfall entweder die alte Fassung stehen
    /// oder gar keine — nie eine halbe (HUM-044).
    DAEMON_006 => "daemon", "Unit-Datei nicht schreibbar", "#daemon_006",
        "Die Unit-Datei lässt sich nicht schreiben (Rechte, Verzeichnis, Dateisystem).",
        "`CopyCommand` mit dem Pfad, sonst der Verweis auf die Dokumentation.";
    /// Neben der laufenden Kommandozeile liegt kein `humanitld`.
    ///
    /// `ExecStart` entsteht aus `std::env::current_exe()` und dem Nachbarn
    /// dieses Pfades, nie aus `PATH` und nie aus einem Konfigurationswert: Was
    /// beim Anmelden startet, soll dieselbe Fassung sein wie das Programm, das
    /// die Unit geschrieben hat (HUM-044).
    DAEMON_007 => "daemon", "humanitld liegt nicht neben humanitl", "#daemon_007",
        "Neben der laufenden Kommandozeile liegt kein `humanitld`, das die Unit starten könnte — oder der eigene Pfad ist gar nicht zu ermitteln.",
        "`CopyCommand` mit `ls -l` auf den erwarteten Pfad; im zweiten Fall kein Fix.";
    /// systemd hat die geschriebene Unit nicht übernommen.
    ///
    /// `systemctl --user daemon-reload` oder `enable --now` ist mit einem
    /// Fehler zurückgekommen. Was dieser Aufruf geschrieben hat, wird dabei
    /// zurückgenommen: Eine Unit, die systemd nicht annimmt, soll nicht
    /// liegenbleiben und beim nächsten Anmelden von selbst auftauchen
    /// (HUM-044).
    DAEMON_008 => "daemon", "systemd hat die Unit nicht übernommen", "#daemon_008",
        "`systemctl --user` hat die geschriebene Unit nicht übernommen oder nicht gestartet.",
        "`CopyCommand` mit `systemctl --user status humanitld`, dem Aufruf, der sagt, was systemd stört.";

    /// Das Token aus `$XDG_RUNTIME_DIR/humanitl/token` fehlt oder passt nicht.
    IPC_001 => "ipc", "Ungültiges Token", "#ipc_001",
        "Das Sitzungs-Token fehlt, ist unlesbar oder passt nicht zu dem des Daemons.",
        "In der Kommandozeile `CopyCommand` mit `humanitld`, der das Token neu schreibt; im Daemon ohne Fix.";
    /// `AllowEdited` kam für mehr als einen Flow; eine bearbeitete Anfrage gilt
    /// immer genau einem.
    IPC_002 => "ipc", "AllowEdited nur für genau einen Flow", "#ipc_002",
        "`AllowEdited` kam mit keiner oder mehr als einer Flow-Id.",
        "Kein Fix: Ein bearbeiteter Anfrage-Rumpf gehört zu genau einem Fluss.";
    /// Der Flow wartet nicht mehr; die Entscheidung kommt zu spät.
    IPC_003 => "ipc", "Flow nicht mehr gehalten", "#ipc_003",
        "Der genannte Fluss wartet nicht mehr: entschieden, abgelaufen oder nie gehalten.",
        "Kein Fix: Die Warteschlange zeigt den aktuellen Stand.";
    /// Die `Decide`-Anfrage lässt sich so nicht ausführen: keine Flow-Id, keine
    /// Entscheidung, eine unlesbare Flow-Id oder eine bearbeitete Anfrage, die
    /// sich nicht lesen lässt oder über `limits.hold_body_cap_bytes` liegt. Der
    /// Grund steht im Befund. Fehlt die Entscheidung, wird sie nie zu `Allow`
    /// ergänzt.
    IPC_004 => "ipc", "Decide-Anfrage ungültig", "#ipc_004",
        "Eine `Decide`-Anfrage ist in sich widersprüchlich, etwa ohne Entscheidung oder mit unbekanntem Fluss.",
        "Kein Fix: Der Text nennt das Feld, das nicht stimmt.";
    /// Die `Rules`-Anfrage lässt sich so nicht ausführen: keine Operation, eine
    /// fehlende oder unlesbare Regel, eine unbekannte Regel-Id, oder der Daemon
    /// läuft ohne Regelspeicher. Eine abgelehnte Anfrage ändert nichts
    /// (HUM-027).
    IPC_005 => "ipc", "Rules-Anfrage ungültig", "#ipc_005",
        "Eine `Rules`-Anfrage verlangt etwas, das der Regelspeicher nicht tun kann, etwa eine mitgelieferte Regel zu löschen.",
        "Kein Fix: Der Text nennt die Regel und den Grund.";
    /// Den RPC gibt es, aber dieser Daemon hat nicht, was er dafür braucht:
    /// keine Endpunkt-Probe etwa, weil sich der Verbindungsstapel aus der
    /// Konfiguration nicht bauen ließ. Der Unterschied zu `UNIMPLEMENTED` ist
    /// für den Client wichtig: Das eine ändert sich mit einem Update, das
    /// andere mit dem Start (HUM-039).
    IPC_006 => "ipc", "Fähigkeit in diesem Daemon nicht verfügbar", "#ipc_006",
        "Dieser Daemon hat die Fähigkeit nicht, nach der gefragt wurde — keine Sandbox, keine Aufzeichnung, keine Endpunkt-Probe.",
        "Kein Fix: Es ist eine Aussage über diesen Daemon, nicht über die Anfrage.";

    /// `config.toml` ließ sich nicht lesen.
    CONFIG_001 => "config", "Config-Datei ungültig", "#config_001",
        "Eine Konfigurations- oder Profildatei ist nicht lesbar oder kein gültiges TOML.",
        "Meist ohne Fix — der Text nennt Datei und Parserfehler; beim Start `ChangeSetting` auf das Vorgabeprofil.";
    /// Ein Schlüssel steht nicht im Schema.
    CONFIG_002 => "config", "Unbekannter Schlüssel", "#config_002",
        "Ein Schlüssel, ein Block oder ein Pfad steht in der Datei, den das Schema nicht kennt.",
        "`ChangeSetting` auf den ähnlichsten Schlüssel, wenn es einen gibt; sonst ohne Fix.";
    /// Ein Wert liegt außerhalb des erlaubten Bereichs.
    CONFIG_003 => "config", "Wert außerhalb des Bereichs", "#config_003",
        "Ein Wert liegt außerhalb seines Bereichs, hat den falschen Typ, ein Profil hat die falsche Form, oder ein Projekt-Profil setzt einen Schlüssel, der ihm nicht gehört.",
        "`ChangeSetting`, wo es einen Schreibweg gibt; beim Projekt-Profil bewusst keiner.";
    /// `$XDG_RUNTIME_DIR` fehlt; ein Ersatzverzeichnis unter `/run/user` oder `$TMPDIR` wird genutzt (Info).
    CONFIG_004 => "config", "Laufzeitverzeichnis ist ein Ersatz", "#config_004",
        "`XDG_RUNTIME_DIR` fehlt, und der Daemon weicht auf ein geteiltes Verzeichnis aus.",
        "`SetEnv` für `XDG_RUNTIME_DIR`.";
    /// Ein veralteter Schlüssel ist in Gebrauch: als Alias, dann steht der
    /// kanonische Name im Befund (Info), oder ersatzlos entfallen, dann nennt
    /// der Befund das Issue und der Wert wird übergangen (Warning, HUM-101).
    CONFIG_005 => "config", "Veralteter Schlüssel", "#config_005",
        "Ein Schlüssel steht in der Datei, der veraltet ist: entweder entfallen (`alias::RETIRED`) oder unter seinem alten Namen geschrieben.",
        "Beim alten Namen `ChangeSetting` auf den heutigen; beim entfallenen kein Fix — es gibt keinen Nachfolger, die Zeile wird gelöscht.";
    /// Alter und neuer Schlüssel sind gleichzeitig gesetzt; der kanonische gewinnt (Warning).
    CONFIG_006 => "config", "Alter und neuer Schlüssel gesetzt", "#config_006",
        "Alter und neuer Name desselben Schlüssels stehen zugleich in der Datei.",
        "Kein Fix: Der Text nennt beide; der neue gilt.";
    /// Das Projekt-Profil liegt in einer Datei, die einem anderen Konto gehört (Warning, HUM-066).
    CONFIG_007 => "config", "Projekt-Profil gehört einem anderen Konto", "#config_007",
        "Das Projekt-Profil gehört einem anderen Konto als dem, das den Daemon fährt.",
        "Kein Fix: Es gilt weiter nur, was ein Projekt setzen darf.";
    /// Ein eigenes Profil verdeckt ein mitgeliefertes mit demselben Namen; die Datei gewinnt (Info, HUM-066).
    CONFIG_008 => "config", "Eigenes Profil verdeckt ein mitgeliefertes", "#config_008",
        "Ein eigenes Profil verdeckt ein mitgeliefertes gleichen Namens und weicht davon ab.",
        "`CopyCommand`, um beide zu vergleichen.";
    /// Das Projekt-Profil nennt ein Profil, das nicht gilt: ein Projekt darf nur ein
    /// mitgeliefertes wählen, und die Kommandozeile geht vor (Warning, HUM-066).
    CONFIG_009 => "config", "Profilwunsch des Projekts gilt nicht", "#config_009",
        "Das Projekt wünscht ein Profil, das es nicht wählen darf; ein anderes gilt.",
        "`CopyCommand`, das den Wunsch des Projekts als Kommandozeilen-Schalter setzt — die Entscheidung bleibt beim Menschen.";
    /// `resolver.test_ca` zeigt auf eine Datei, aus der sich kein Zertifikat lesen
    /// lässt: sie fehlt, ist unlesbar oder enthält keinen PEM-Block. Mit
    /// `--allow-test-ca` startet der Daemon dann nicht, statt still nichts zu
    /// vertrauen (Error, HUM-087).
    CONFIG_010 => "config", "Test-Wurzel nicht verwendbar", "#config_010",
        "`resolver.test_ca` zeigt auf etwas, das keine brauchbare Wurzel ist.",
        "`CopyCommand` zum Prüfen der Datei.";
    /// Flag und Schlüssel passen nicht zusammen: `resolver.test_ca` ist gesetzt,
    /// aber der Daemon läuft ohne `--allow-test-ca` (die Wurzel gilt nicht), oder
    /// das Flag steht ohne den Schlüssel (es bewirkt nichts). Beide Hälften
    /// gehören zusammen (Warning, HUM-087).
    CONFIG_011 => "config", "Test-Wurzel ohne Flag oder Flag ohne Test-Wurzel", "#config_011",
        "Test-Wurzel ohne Flag oder Flag ohne Test-Wurzel: die beiden gehören zusammen.",
        "Der Fix nennt die fehlende Hälfte.";
    /// `resolver.test_ca` trägt keinen absoluten Pfad. Ein relativer würde gegen
    /// das Arbeitsverzeichnis des Starts aufgelöst; damit entschiede das
    /// Verzeichnis mit, welcher Wurzel der Daemon vertraut. Abgelehnt wird vor
    /// dem Lesen (Error, HUM-087).
    CONFIG_012 => "config", "Test-Wurzel ohne absoluten Pfad", "#config_012",
        "`resolver.test_ca` ist kein absoluter Pfad und hinge damit am Startverzeichnis des Daemons.",
        "`ChangeSetting` auf einen absoluten Pfad.";
    /// Es wurde kein Projektordner gewählt. Der Agent arbeitet in genau einem
    /// Ordner, und solange keiner benannt ist, gibt es nichts zu starten — kein
    /// Fehler des Nutzers, sondern ein offener Schritt der Einrichtung. Der
    /// Befund trägt deshalb keinen `fix`: Die Zeile im Setup-Bildschirm öffnet
    /// den Ordner-Knopf, und ein zweiter Knopf daneben führte nur woanders hin
    /// (HUM-044).
    ///
    /// **Kein Weg im Daemon erhebt diesen Code, und das ist Absicht.** Er wird
    /// von der Anwendung gebaut (`ClientDiagnostics.noProjectFolder`), wie
    /// `DAEMON_001`: `Sandbox(Status)` antwortet mit leerem `work_dir_host` und
    /// meldet dazu nichts, weil ein offener Schritt der Einrichtung kein Fehler
    /// des Daemons ist. Er steht trotzdem hier, damit Anwendung und
    /// `docs/DIAGNOSTICS.md` denselben Titel und denselben Anker nennen — wer
    /// den erhebenden Pfad im Daemon sucht, sucht vergeblich.
    CONFIG_013 => "config", "Kein Projektordner gewählt", "#config_013",
        "Niemand hat einen Projektordner gewählt; der Client meldet es, nicht der Daemon.",
        "Kein Fix-Knopf: Der Ordner wird im Setup gewählt.";

    /// `bwrap` ist nicht installiert oder liegt nicht im Pfad.
    SANDBOX_001 => "sandbox", "bwrap nicht gefunden", "#sandbox_001",
        "`bwrap` liegt nicht im `PATH` des Daemons.",
        "`CopyCommand` mit dem Installationsbefehl.";
    /// Die gefundene `bwrap`-Version kann nicht alles, was das Profil verlangt.
    SANDBOX_002 => "sandbox", "bwrap-Version zu alt", "#sandbox_002",
        "Das gefundene `bwrap` ist älter als die Mindestversion des Starters.",
        "`CopyCommand` mit dem Installationsbefehl.";
    /// Der Kernel erlaubt keine unprivilegierten User-Namespaces.
    SANDBOX_003 => "sandbox", "User-Namespaces nicht erlaubt", "#sandbox_003",
        "Unprivilegierte User-Namespaces sind auf diesem Kernel abgeschaltet.",
        "`CopyCommand` mit dem `sysctl`-Aufruf.";
    /// Eine der drei Garantien ließ sich in der laufenden Sandbox nicht zeigen.
    SANDBOX_004 => "sandbox", "Isolation-Check fehlgeschlagen", "#sandbox_004",
        "Der Isolations-Check ist fehlgeschlagen; welche der drei Garantien, sagt sein eigener Code.",
        "Kein Fix: Der Start wird abgebrochen.";
    /// Der Projektordner ist nicht beschreibbar, obwohl `work_mode = "rw"` gilt.
    SANDBOX_005 => "sandbox", "Projektordner nicht beschreibbar", "#sandbox_005",
        "Der Projektordner fehlt, ist kein Verzeichnis, ist kein absoluter Pfad, oder er ist bei `rw` nicht beschreibbar.",
        "Beim nicht beschreibbaren Ordner `RemountReadOnly`; sonst kein Fix.";
    /// Ein Mount im Profil zeigt auf eine Host-Quelle, die nie in die Sandbox darf.
    SANDBOX_006 => "sandbox", "Mount verboten", "#sandbox_006",
        "Ein Profil, der Proxy-Socket oder ein Agent-Adapter will einen Pfad einhängen, den die Mount-Regeln verbieten.",
        "Kein Fix: Der Text nennt Pfad und Regel.";
    /// Eine Bridge im Profil hat eine Richtung, die es nicht gibt.
    SANDBOX_007 => "sandbox", "Bridge-Richtung unbekannt", "#sandbox_007",
        "Ein Profil nennt eine Brücken-Richtung, die es nicht gibt.",
        "Kein Fix: Erlaubt ist heute nur `in`.";
    /// Die Argumentliste des Starters hat nicht die von HUM-010 erzeugte Form.
    SANDBOX_010 => "sandbox", "Argumentliste des Starters unerwartet", "#sandbox_010",
        "Die Argumentliste des Starters passt nicht zu dem, was der Launcher erwartet.",
        "`ChangeSetting` auf ein Profil, dessen Argumentliste passt; im Escape-Starter ohne Fix.";
    /// Platzhalter für Socket, CA oder Shim konnten nicht angelegt werden.
    SANDBOX_011 => "sandbox", "Platzhalter nicht anlegbar", "#sandbox_011",
        "Ein Platzhalter oder Verzeichnis für den Lauf lässt sich nicht anlegen, oder der Schnappschuss des Projekts kann nicht gelesen werden.",
        "Meist ohne Fix — der Text nennt Pfad und Fehler; fehlt der Shim, `CopyCommand` zum Bauen.";
    /// Die Kommandozeile des Starters ist ungültig.
    SANDBOX_012 => "sandbox", "Kommandozeile des Starters ungültig", "#sandbox_012",
        "Die Kommandozeile des Starters ist ungültig, ein Plan wurde zweimal gestartet, oder der wartende Thread ist gescheitert.",
        "Kein Fix: Der Fehler liegt im Starter oder im Plan, nicht in einer Einstellung.";
    /// Isolation-Check: der Shim hat keinen Prüfbericht geliefert.
    SANDBOX_013 => "sandbox", "Isolation-Check ohne Bericht", "#sandbox_013",
        "Der Shim hat keinen vollständigen Bericht geliefert; die drei Garantien sind damit unbelegt.",
        "Kein Fix: Der Start wird abgebrochen.";
    /// Isolation-Check 1 fehlgeschlagen: ein Netzwerk-Interface außer `lo` existiert.
    SANDBOX_014 => "sandbox", "Isolation-Check 1: Netzwerk-Interface vorhanden", "#sandbox_014",
        "Check 1: In der Sandbox steht mehr als `lo` als Netzwerkschnittstelle.",
        "Kein Fix: Der Daemon beendet die Sandbox.";
    /// Isolation-Check 2 fehlgeschlagen: mehr als ein Socket erreichbar.
    SANDBOX_015 => "sandbox", "Isolation-Check 2: mehr als eine Tür", "#sandbox_015",
        "Check 2: In der Sandbox steht mehr als der eine erlaubte Unix-Socket.",
        "Kein Fix: Der Daemon beendet die Sandbox.";
    /// Isolation-Check 3 fehlgeschlagen: seccomp nicht aktiv oder Familien nicht gesperrt.
    SANDBOX_016 => "sandbox", "Isolation-Check 3: seccomp unwirksam", "#sandbox_016",
        "Check 3: Der seccomp-Filter ist nicht aktiv oder lässt zu viel durch.",
        "Kein Fix: Der Daemon beendet die Sandbox.";
    /// Eine Pflicht-Maske oder eine andere Maske unter `/work` wurde per
    /// `mounts.unmask` freigegeben; der Agent kann die Datei lesen und
    /// beschreiben (Warning, HUM-043).
    SANDBOX_020 => "sandbox", "Maskierter Pfad freigegeben", "#sandbox_020",
        "Ein Profil hebt eine Maske auf; der Agent darf den Pfad lesen und schreiben.",
        "Kein Fix: Es ist eine Aussage über das gewählte Profil.";
    /// Der Kernel kennt `openat2` nicht; der Lauf über das Projektverzeichnis
    /// nimmt den Weg über `openat` je Bestandteil (Info, HUM-043).
    SANDBOX_021 => "sandbox", "Kernel ohne openat2", "#sandbox_021",
        "Der Kernel kennt `openat2` nicht; der Projektordner wird mit `openat` und `O_NOFOLLOW` gelesen.",
        "Kein Fix: Die Prüfung läuft, nur langsamer und mit weniger Garantie.";
    /// Der Agent hat einen Symlink angelegt, dessen Ziel außerhalb von `/work`
    /// liegt (Warning, HUM-043).
    SANDBOX_022 => "sandbox", "Symlink zeigt aus dem Projekt hinaus", "#sandbox_022",
        "Der Agent hat einen Symlink angelegt, der aus dem Projekt hinauszeigt.",
        "Kein Fix: Der Bericht nennt Ziel und Quelle.";
    /// Im Diff des Sandbox-Laufs stecken mögliche Geheimnisse (Warning,
    /// HUM-043).
    SANDBOX_023 => "sandbox", "Mögliche Geheimnisse im Projekt", "#sandbox_023",
        "Im Projekt stehen nach dem Lauf Zeichenketten, die wie Geheimnisse aussehen.",
        "Kein Fix: Der Bericht nennt Datei und Fundzahl.";
    /// Ein Budget hat gegriffen: Der Schnappschuss des Projektverzeichnisses
    /// ist unvollständig (Info, HUM-043).
    SANDBOX_024 => "sandbox", "Schnappschuss abgeschnitten", "#sandbox_024",
        "Ein Budget hat den Schnappschuss des Projekts vorzeitig beendet.",
        "Kein Fix: Der Bericht sagt, welcher Teil gekürzt ist.";
    /// Der Agent hat unter einem Pfad geschrieben, den das Profil überdeckt,
    /// den es aber im Projekt nicht gab: Ohne vorhandenen Mountpoint hängt
    /// `bwrap` kein `tmpfs` und keine Maske darüber (Warning, HUM-043).
    SANDBOX_025 => "sandbox", "Ohne Maske ins Projekt geschrieben", "#sandbox_025",
        "Der Agent hat unter einen maskierten Pfad geschrieben, der im Projekt nicht existierte.",
        "Kein Fix: Der Bericht nennt die Dateien.";
    /// Der Lauf hat eine Datei hinterlassen, die dieser Rechner von selbst
    /// ausführt: ein Git-Hook, ein `Makefile`, ein `package.json` mit
    /// `postinstall`, eine Workflow-Datei. Nicht geblockt, sondern gelistet
    /// (Warning, HUM-043).
    SANDBOX_026 => "sandbox", "Datei im Projekt, die der Rechner ausführt", "#sandbox_026",
        "Der Agent hat Dateien geschrieben, die dieser Rechner von sich aus ausführt (Hooks, Build-Dateien).",
        "Kein Fix: Der Bericht nennt die erste davon.";
    /// Zu dieser Sandbox-Kennung liegt keine Zusammenfassung vor: Der Lauf ist
    /// älter als die Aufzeichnung, endete ohne Zusammenfassung, oder die
    /// Kennung gehört zu keinem Lauf dieses Rechners (Error, HUM-043).
    SANDBOX_027 => "sandbox", "Keine Zusammenfassung zu diesem Lauf", "#sandbox_027",
        "Zu dieser Sandbox ist keine Zusammenfassung aufgezeichnet.",
        "Kein Fix: Entweder lief sie hier nie, oder der Lauf hat keine hinterlassen.";
    /// Der Lauf hat Dateien geändert, in die der Fundscan nicht gesehen hat:
    /// zu groß, nicht lesbar, oder das Byte-Budget war aufgebraucht. In ihnen
    /// wurde nichts gefunden, weil in ihnen nichts gesucht wurde — wie groß
    /// eine Datei ist und welche Rechte sie trägt, bestimmt der Agent
    /// (Warning, HUM-043).
    SANDBOX_028 => "sandbox", "Geänderte Datei nicht durchsucht", "#sandbox_028",
        "Geänderte Dateien wurden nicht nach Geheimnissen durchsucht, weil ein Budget zuschlug.",
        "Kein Fix: Der Bericht nennt Zahl und erste Datei.";

    /// Der Body ist größer als `limits.hold_body_cap_bytes`.
    PROXY_001 => "proxy", "Body über Cap", "#proxy_001",
        "Reserviert für einen Rumpf über der Grenze, bis zu der Humanitl ihn hält oder aufzeichnet. Heute baut diesen Befund niemand: Die Grenzen aus `limits.*` greifen ohne eigene Meldung, gekürzt wird mit Vermerk am Fluss.",
        "Kein Fix: Solange nichts den Code baut, erscheint er nirgends.";
    /// Der `Host`-Header widerspricht dem Ziel des TLS-Handschlags.
    PROXY_002 => "proxy", "Authority-Mismatch", "#proxy_002",
        "Ziel des `CONNECT`, SNI und `Host` beziehungsweise `:authority` passen nicht zusammen (Domain Fronting).",
        "Kein Fix: Die Anfrage wird blockiert.";
    /// Die Verbindung zum Ziel scheiterte (Auflösung, TCP, TLS, private Adresse
    /// oder Zeitüberschreitung); der Proxy antwortet dem Client mit `502` und
    /// verbucht den Flow als `Failed` (HUM-015, HUM-024).
    PROXY_003 => "proxy", "Upstream-Verbindung fehlgeschlagen", "#proxy_003",
        "Die Verbindung zum Ziel kommt nicht zustande, oder es ist keine Adresse angeheftet.",
        "Kein Fix: Der Text nennt Ziel und Ursache.";
    /// Der Zustandsautomat hat einen Übergang abgelehnt, den der Proxy versucht
    /// hat: ein Fehler im Daemon, kein Zustand des Clients. Der Flow wird
    /// fail-closed mit `Block` beendet und der Befund geht in den Ereignisstrom
    /// (HUM-016).
    PROXY_005 => "proxy", "Ungültiger Übergang im Flow", "#proxy_005",
        "Ein Fluss soll einen Übergang nehmen, den sein Zustand nicht erlaubt.",
        "Kein Fix: Die Anfrage wird blockiert statt in einem unklaren Zustand fortgesetzt.";
    /// Der Client verlangt HTTP/2; in M1 bietet der Proxy nur `http/1.1` an.
    PROXY_007 => "proxy", "HTTP/2 nicht verfügbar", "#proxy_007",
        "Reserviert für den Fall, dass HTTP/2 nach oben verlangt, aber nicht verfügbar ist. Heute baut ihn niemand — der Proxy spricht nach oben HTTP/1.1, und `experimental.h2_upstream` hat keinen Leser im Weg dorthin (HUM-108).",
        "Kein Fix: Der Code wartet auf den Weg, den er beschreiben soll.";
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
    PROXY_008 => "proxy", "Private Zieladresse abgelehnt", "#proxy_008",
        "Das Ziel löst auf eine private Adresse auf, und keine Regel erlaubt das.",
        "`AddRule` mit `allow_private` für genau diesen Host.";

    /// Der Client vertraut der mitgelieferten CA nicht.
    TLS_001 => "tls", "Client hat Humanitl-CA abgelehnt", "#tls_001",
        "Ein Client in der Sandbox hat die Humanitl-CA abgelehnt (`unknown_ca` oder `bad_certificate`).",
        "`SetEnv` mit der CA-Variablen, die dieses Werkzeug liest.";
    /// Ein Client in der Sandbox bricht den TLS-Handschlag zu demselben Host
    /// wiederholt ab (dreimal in zehn Sekunden), ohne einen Alert zu schicken,
    /// der die CA nennt. Das deutet auf Certificate Pinning oder auf ein
    /// Werkzeug, das die CA-Umgebungsvariablen nicht liest (HUM-045).
    TLS_002 => "tls", "Client bricht den Handschlag wiederholt ab", "#tls_002",
        "Ein Client bricht den Handschlag zu demselben Host wiederholt ab.",
        "`AddRule`: den Host blocken, damit der Agent schnell scheitert statt zu hängen.";
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
    TLS_003 => "tls", "Client ohne SNI", "#tls_003",
        "Ein Client hat ohne SNI verbunden; nichts bindet den Handschlag dann an den Host des Tunnels.",
        "Kein Fix: Die Verbindung wird abgewiesen, weil ihre Anfragen keinem Ziel zuzuordnen wären.";
    /// Das CA-Verzeichnis oder eine Datei darin ließ sich nicht anlegen, schreiben oder umbenennen (HUM-014).
    TLS_004 => "tls", "CA-Verzeichnis nicht beschreibbar", "#tls_004",
        "Das CA-Verzeichnis ist nicht beschreibbar.",
        "`CopyCommand` mit dem Pfad.";
    /// `ca.key` oder `ca.crt` fehlt, ist unlesbar, passt nicht zusammen oder hat unsichere Rechte (HUM-014).
    TLS_005 => "tls", "CA-Dateien unbrauchbar", "#tls_005",
        "Die CA-Dateien sind unbrauchbar: Zertifikat oder Schlüssel lassen sich nicht lesen.",
        "Bei verdorbenem Material `CopyCommand`, das das CA-Verzeichnis löscht, damit es neu entsteht; sonst kein Fix.";

    /// Der LLM-Endpunkt aus `llm.endpoint` antwortet nicht: die TCP-Verbindung
    /// kam nicht zustande, der Name löste nicht auf, oder die Frist der Probe
    /// lief ab. Blockierend, weil der Agent ohne Modell nichts tun kann
    /// (HUM-039).
    LLM_001 => "llm", "LLM-Endpoint nicht erreichbar", "#llm_001",
        "Der Endpunkt antwortet nicht, löst nicht auf, oder die Frist läuft ab.",
        "`CopyCommand` mit einem `curl`, das dasselbe versucht.";
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
    LLM_002 => "llm", "LLM-Endpoint verlangt eine Anmeldung", "#llm_002",
        "Der Endpunkt verlangt eine Anmeldung (`401` oder `403`).",
        "Kein Fix: Humanitl schickt im MVP keine Zugangsdaten.";
    /// Die Verbindung steht, aber weder `/api/tags` (Ollama) noch `/v1/models`
    /// (OpenAI-kompatibel) hat geantwortet. Meist zeigt die Adresse auf eine
    /// Oberfläche statt auf die Wurzel der API (HUM-039).
    LLM_003 => "llm", "LLM-Endpoint antwortet nicht als bekannte API", "#llm_003",
        "Die Verbindung steht, aber weder `/api/tags` noch `/v1/models` antwortet als bekannte API.",
        "`ChangeSetting` auf die Adresse der API-Wurzel.";

    /// `rules.yaml` ließ sich nicht lesen: die Datei fehlt, ist nicht lesbar,
    /// ist kein gültiges YAML oder passt nicht zum Schema. Der Befund nennt
    /// `Zeile:Spalte` und den Feldpfad.
    RULES_001 => "rules", "Regel-Datei ungültig", "#rules_001",
        "Die Regeldatei ist unlesbar, kein gültiges YAML, oder eine Regel überlebt den Umlauf nicht.",
        "Kein Fix: Der Text nennt die Stelle.";
    /// Ein Host-Muster sieht nach einem Fehler oder nach Täuschung aus: ein
    /// Punycode-Literal (`xn--`), das ein anderer Name sein könnte als der
    /// gemeinte, oder eine IP-Adresse an der Stelle eines Host-Globs. Das ist
    /// eine Warnung; die Regel bleibt gültig (HUM-022).
    RULES_002 => "rules", "Host-Muster verdächtig (xn--, IP in Host-Glob)", "#rules_002",
        "Ein Host-Muster sieht verdächtig aus: eine IP im Glob oder ein `xn--`-Label.",
        "Kein Fix: Der Text nennt die Schreibweise, die wirklich passt.";
    // RULES_004 bleibt frei: `backlog/sprint-2.md` führt darunter die Warnung
    // vor einem Punycode-Literal, die dieses Register seit HUM-063 unter
    // RULES_002 kennt. Eine Nummer zweimal zu vergeben wäre schlimmer als eine
    // Lücke, und eine Nummer wird nie wiederverwendet.
    /// Ein Host-Muster ließ sich nicht lesen: ein Stern steht nicht als ganzes
    /// Label (`*foo.com`), ein Label ist leer (`foo..com`) oder das Muster ist
    /// kein Host, kein Glob und keine Adresse (HUM-022).
    RULES_003 => "rules", "Host-Muster ungültig", "#rules_003",
        "Ein Host-Muster ist ungültig und passt auf nichts.",
        "Kein Fix: Der Text nennt den Fehler im Muster.";
    /// Ein Pfadmuster ließ sich nicht übersetzen: der reguläre Ausdruck hinter
    /// `~` ist ungültig oder überschreitet die Größengrenze, oder der Glob ist
    /// kein gültiges Muster (HUM-022).
    RULES_005 => "rules", "Pfadmuster ungültig", "#rules_005",
        "Ein Pfadmuster ist ungültig, etwa ohne führenden Schrägstrich.",
        "Kein Fix: Der Text nennt die Form, die gilt.";
    /// `version` fehlt in `rules.yaml` oder ist nicht `1`.
    RULES_006 => "rules", "Version der Regel-Datei unbekannt", "#rules_006",
        "Die Regeldatei nennt eine Version, die es nicht gibt.",
        "Kein Fix: Es gibt genau eine Version.";
    /// Zwei Regeln tragen dieselbe `id`.
    RULES_007 => "rules", "Doppelte Regel-Id", "#rules_007",
        "Zwei Regeln tragen dieselbe Id.",
        "Kein Fix: Jede Regel braucht ihre eigene.";
    /// Eine Regel erlaubt mehr, als sie vermutlich soll: `host: "**"` ohne
    /// weitere Einschränkung zusammen mit `action: allow` hebt die Moderation
    /// für jeden DNS-Host auf. Das ist eine Warnung, keine Ablehnung.
    RULES_008 => "rules", "Regel wirkt zu breit", "#rules_008",
        "Eine Regel wirkt so breit, dass sie mehr erlaubt, als der Mensch vermutlich meint.",
        "Kein Fix: Der Text nennt, worauf sie zusätzlich passt.";
    /// `rules.yaml` ließ sich nicht schreiben: das Verzeichnis fehlt, die
    /// Rechte reichen nicht, die Platte ist voll. Die Datei bleibt dabei
    /// unangetastet, weil der Regelsatz erst in eine Nebendatei geht und dann
    /// umbenannt wird; die Änderung gilt deshalb auch im Speicher nicht
    /// (HUM-027).
    RULES_009 => "rules", "Regel-Datei nicht schreibbar", "#rules_009",
        "Die Regeldatei lässt sich nicht schreiben; der Satz auf der Platte bleibt unverändert.",
        "Kein Fix: Der Text nennt Pfad und Grund.";
    /// Eine mitgelieferte Regel (`bundled: true`) soll gelöscht oder geändert
    /// werden. Mitgelieferte Regeln gehören nicht dem Nutzer; wer sie
    /// aufheben will, legt davor eine eigene Regel mit demselben Muster an
    /// (HUM-027).
    RULES_010 => "rules", "Mitgelieferte Regel ist unveränderlich", "#rules_010",
        "Eine mitgelieferte Regel soll geändert oder gelöscht werden.",
        "Kein Fix: Mitgelieferte Regeln werden deaktiviert, nicht entfernt.";
    /// Der Regelsatz wurde aus `rules.yaml` neu geladen. Der Befund nennt,
    /// was sich dabei geändert hat; er ist eine Information, kein Fehler
    /// (HUM-027).
    RULES_011 => "rules", "Regelsatz neu geladen", "#rules_011",
        "Der Regelsatz wurde neu geladen; die Zahl der Regeln steht im Text.",
        "Kein Fix: Es ist eine Meldung, kein Fehler.";
    /// Der Probelauf konnte die aufgezeichneten Flows nicht lesen. Die
    /// Antwort trägt dann keine Treffer und keine geprüfte Zeile; ohne diesen
    /// Befund läse der Regel-Bildschirm eine gezählte Null, hinter der man
    /// Grün vermuten könnte (`backlog/CONVENTIONS.md` 4.13, HUM-033).
    RULES_012 => "rules", "Probelauf konnte die Aufzeichnung nicht lesen", "#rules_012",
        "Der Probelauf fand keine Aufzeichnung, gegen die er die Regel prüfen könnte.",
        "Kein Fix: Ohne Verkehr gibt es nichts zu prüfen.";

    /// Das eingebaute Regel-Set der Secret-Detektoren ließ sich nicht lesen
    /// oder eines seiner Muster nicht übersetzen. Das ist ein Fehler im
    /// Daemon, kein Zustand der Anfrage: ohne Regel-Set gibt es keine Suche
    /// nach Secrets, und die Suche wird nicht stillschweigend übersprungen.
    FINDINGS_001 => "findings", "Detektor-Regeln unbrauchbar", "#findings_001",
        "Das eingebaute Regelwerk der Detektoren ist unbrauchbar; das ist ein Fehler im Bau.",
        "Kein Fix: Der Text nennt die Datei und den Parserfehler.";
    /// Die Anfrage wurde nur teilweise durchsucht: der Body liegt über
    /// `limits.preview_cap_bytes`, das Entpacken lief über
    /// `limits.max_decompress_ratio`, oder der Body trägt eine Kodierung, für
    /// die es keinen Entpacker gibt, beziehungsweise einen beschädigten Strom.
    /// Angezeigte Funde sind dann unvollständig.
    FINDINGS_002 => "findings", "Scan unvollständig", "#findings_002",
        "Der Scan war unvollständig: Der Rumpf war größer als die Vorschau-Grenze oder das Entpacken lief gegen die Verhältnis-Grenze.",
        "`ChangeSetting` auf `limits.preview_cap_bytes` beziehungsweise `limits.max_decompress_ratio`.";
    /// In der Notiz, die ein Mensch beim Blocken an den Agenten gerichtet hat,
    /// steckt ein Fund der Detektoren: ein Schlüssel, ein Token, ein Begriff
    /// aus `findings.user_terms`. Die Notiz geht im Klartext in die
    /// 403-Antwort, der Agent liest sie also; der Befund sagt, dass sie
    /// mitgeht, und nennt Art und Anfang des Funds, nie seinen Wert
    /// (HUM-025-Regel für Funde, HUM-117).
    ///
    /// Eine Warnung, keine Sperre: Die Notiz ist der Wille des Menschen, und
    /// wer ein Geheimnis darin abschicken will, darf das. Die Entscheidung
    /// fällt unverändert.
    FINDINGS_003 => "findings", "Fund in einer Notiz", "#findings_003",
        "In der Notiz an den Agenten steckt ein möglicher Geheimniswert; sie geht trotzdem hinaus.",
        "Kein Fix: Die Entscheidung steht. Wer den Wert nicht senden will, blockt erneut mit einer anderen Notiz.";

    /// `catalog/domains.yaml` fehlt oder lässt sich nicht als Katalog lesen:
    /// unbekannte `version`, ungültiges YAML, doppelte `id`, ein Host-Muster,
    /// das kein Name und kein Label-Glob ist. Der Daemon läuft dann mit einem
    /// leeren Katalog weiter; jede Domain steht als unbekannt da, und keine
    /// wird als bekannt ausgegeben (HUM-031).
    CATALOG_001 => "catalog", "Domain-Katalog nicht lesbar", "#catalog_001",
        "Der mitgelieferte Domain-Katalog ist nicht lesbar.",
        "Kein Fix: Ohne Katalog fehlen nur die Namen, nicht die Entscheidung.";
    /// `catalog/tranco-top100k.csv.gz` fehlt oder lässt sich nicht lesen:
    /// beschädigter Gzip-Strom, eine Zeile ohne `rang,domain`, oder die Datei
    /// überschreitet die Grenzen für entpackte Größe und Zeilenzahl. Der
    /// Daemon läuft dann ohne Ränge weiter; das Panel zeigt „unranked" statt
    /// einer geratenen Zahl (HUM-031).
    CATALOG_002 => "catalog", "Rangliste nicht lesbar", "#catalog_002",
        "Die Rangliste der Domains ist nicht lesbar.",
        "Kein Fix: Die Sortierung fällt auf die Reihenfolge der Funde zurück.";

    /// Es gibt bereits einen schreibenden Terminal-Client.
    TERM_001 => "terminal", "Zweiter schreibender Terminal-Client abgelehnt", "#term_001",
        "Ein zweiter schreibender Terminal-Client meldet sich an derselben Sitzung an.",
        "`CopyCommand` mit `humanitl sandbox attach --read-only`: zusehen geht, schreiben nicht.";
    /// Das Terminal der Sandbox nimmt weder Eingabe noch Geometrie an: Die
    /// Sitzung läuft ohne Pseudoterminal, oder der Agent hat sich beendet und
    /// der Kernel meldet `EIO`. Wer zusieht, verliert dabei nichts; wer
    /// schreibt, erfährt, dass es niemanden mehr gibt, der liest (HUM-042).
    TERM_002 => "terminal", "Terminal der Sandbox nicht erreichbar", "#term_002",
        "Das Terminal der Sandbox ist nicht erreichbar; der Lauf hat keines oder es ist schon zu.",
        "Kein Fix: Der Text nennt die Sitzung.";

    /// Die Aufzeichnung ließ sich nicht öffnen: das Datenverzeichnis oder der
    /// Blob-Speicher ist nicht anlegbar, die Datenbankdatei nicht lesbar oder
    /// beschreibbar, oder eine Migration schlug fehl. Ohne Aufzeichnung gilt
    /// die Zusage „alles wird aufgezeichnet" nicht mehr (HUM-026).
    RECORDER_001 => "recorder", "Aufzeichnung nicht verfügbar", "#recorder_001",
        "Die Aufzeichnung ist nicht verfügbar: Datenbank fehlt, ist gesperrt oder unlesbar — oder dieser Daemon läuft ohne Aufzeichnung.",
        "Wo es einen gibt, nennt der Fix den Pfad der Datenbank.";
    /// Ein Filterausdruck für `ListFlows` ließ sich nicht lesen: unbekannter
    /// Schlüssel, fehlender Wert, unbrauchbare Zahl oder Zeitangabe. Der Befund
    /// nennt den beanstandeten Term und die gültigen Schlüssel (HUM-026).
    RECORDER_002 => "recorder", "Filter ungültig", "#recorder_002",
        "Ein Filter der Historie ist ungültig.",
        "Kein Fix: Der Text nennt das Feld.";
    /// Ein Schreibvorgang der Aufzeichnung schlug fehl. Der Schreib-Thread
    /// lebt weiter, der betroffene Datensatz fehlt aber; der Befund geht in
    /// den Ereignisstrom, damit die Lücke sichtbar ist (HUM-026).
    RECORDER_003 => "recorder", "Aufzeichnung konnte nicht schreiben", "#recorder_003",
        "Die Aufzeichnung konnte nicht schreiben; der Fluss bleibt unvollständig.",
        "Wo der Schreiber steht, `CopyCommand` zum Neustart des Daemons; sonst nennt der Text nur den Fehler.";
    /// Ein Body ließ sich nicht in den Blob-Speicher schreiben oder von dort
    /// lesen: fehlende Datei, falsche Rechte, volle Platte (HUM-026).
    RECORDER_004 => "recorder", "Blob-Speicher nicht benutzbar", "#recorder_004",
        "Der Blob-Speicher ist nicht benutzbar; große Rümpfe können nicht abgelegt werden.",
        "`CopyCommand` mit `ls -ld` und `df -h` auf das Verzeichnis — Rechte oder Platz.";

    /// Die Hash-Kette in `audit.jsonl` passt nicht mehr zusammen.
    ///
    /// Zwei Stellen bauen ihn (HUM-050): die Prüfung der Kette, die die erste
    /// fehlerhafte Position und den Grund nennt, und der Schreiber beim Start,
    /// wenn der letzte vollständige Record nicht zu Schlüssel oder Ankern
    /// passt. Im zweiten Fall startet der Daemon nicht: Eine Kette auf einem
    /// gebrochenen Ende fortzusetzen hieße, jeden neuen Record auf etwas zu
    /// bauen, das `verify` ohnehin verwirft.
    AUDIT_001 => "audit", "Hash-Kette gebrochen", "#audit_001",
        "Die Prüfung von `audit.jsonl` findet einen Record, der nicht zu Vorgänger, Hash, MAC oder Anker passt, oder die Datei endet vor einem Anker; beim Start ist es der letzte Record, an den der Schreiber anhängen soll.",
        "`CopyCommand`, der die Datei samt Zeitstempel beiseitelegt; sie bleibt als Beleg liegen, und die Kette beginnt neu.";

    /// Der Daemon hat geantwortet, aber den Aufruf abgelehnt: der Aufruf
    /// selbst passt nicht zum Zustand des Daemons.
    CLI_001 => "cli", "Aufruf am Daemon abgelehnt", "#cli_001",
        "Ein Aufruf ist am Daemon gescheitert, oder eine Ausgabe ließ sich nicht schreiben.",
        "Kein Fix: Der Text nennt den Aufruf und den Grund.";
    /// `--ask terminal` steht für diesen Lauf nicht zur Verfügung.
    ///
    /// Zwei Gründe, und der zweite bleibt: Solange die Kommandozeile kein PTY
    /// anhängt (HUM-042), gibt es überhaupt kein Terminal, in dem die Frage
    /// stehen könnte; danach bleibt sie für Vollbild-TUI-Agenten wie `OpenCode`
    /// verwehrt, weil die Frage dort nicht zu sehen wäre
    /// (`backlog/CONVENTIONS.md` 4.10). Der Befund schlägt in beiden Fällen
    /// `--ask ui` und `--ask none` vor.
    CLI_002 => "cli", "`--ask terminal` ist hier nicht möglich", "#cli_002",
        "`--ask terminal` ist für diesen Lauf nicht möglich, etwa bei einem Vollbild-Agenten.",
        "Kein Fix: Der Text nennt die beiden Modi, die gehen.";
    /// Das Unterkommando steht im Vertrag, aber noch nicht in diesem Binary.
    CLI_003 => "cli", "Unterkommando noch nicht verfügbar", "#cli_003",
        "Ein Unterkommando gibt es noch nicht; das Issue dazu steht im Text.",
        "`OpenUrl` auf das Issue.";
    /// Die Kommandozeile ließ sich nicht lesen: unbekanntes Unterkommando, fehlendes oder unlesbares Argument (HUM-064).
    CLI_004 => "cli", "Aufruf ungültig", "#cli_004",
        "Der Aufruf ist ungültig: unbekanntes Unterkommando, fehlendes Argument, widersprüchliche Schalter.",
        "`CopyCommand` mit der Form, die gilt.";
    /// Die Kommandozeile konnte nicht einmal anfangen: Die Laufzeit, die jeder
    /// Aufruf braucht, liess sich auf diesem Rechner nicht bauen (keine
    /// Threads, kein `epoll`). Kein Fehler des Aufrufers und keiner des
    /// Daemons, deshalb ein eigener Code statt `CLI_004` oder `CLI_001`.
    CLI_006 => "cli", "Die Laufzeit liess sich nicht starten", "#cli_006",
        "Die Kommandozeile konnte ihre Laufzeit nicht bauen; der Grund des Betriebssystems steht im Text.",
        "Kein Fix: Der Text nennt, was das Betriebssystem gemeldet hat.";
    /// Die Arbeitsumgebung bietet keinen Platz fuer ein Anzeigesymbol
    /// (GNOME ohne die AppIndicator-Erweiterung). Die Anwendung laeuft
    /// weiter, der Zaehler steht im Fenstertitel; der Fix verweist auf die
    /// Erweiterung (HUM-034).
    UI_002 => "ui", "Kein Platz für das Anzeigesymbol", "#ui_002",
        "Die Arbeitsumgebung bietet keinen Platz für das Anzeigesymbol im Systembereich.",
        "Kein Fix: Das Fenster bleibt der Weg zur Warteschlange.";

    // HUM-037: der Agent-Adapter und seine Vorprüfung. Neue Einträge stehen am
    // Ende des Registers, nicht bei ihrem Bereich; die Reihenfolge im Quelltext
    // sagt nichts aus, `docs/DIAGNOSTICS.md` gruppiert nach Bereich.
    /// Das Kommando des Agenten ist auf diesem Rechner nicht zu finden: weder
    /// im `$PATH` des Hosts noch als `agent.command`. Ohne Kommando gibt es
    /// nichts zu starten, deshalb ist der Befund blockierend (HUM-037).
    AGENT_001 => "agent", "Agent-Kommando nicht gefunden", "#agent_001",
        "Das Agent-Kommando ist weder im `PATH` noch über `agent.command` gesetzt.",
        "`CopyCommand` mit dem Installationsbefehl.";
    /// `agent.command` zeigt auf eine Datei, die es nicht gibt oder die nicht
    /// ausführbar ist. Die Sandbox startet trotzdem, weil der Pfad in der
    /// Sandbox ein anderer sein kann als auf dem Host; scheitert das `exec`,
    /// meldet der Shim es mit seinem eigenen Exit-Code (HUM-037).
    AGENT_002 => "agent", "Agent-Kommando nicht ausführbar", "#agent_002",
        "`agent.command` zeigt auf etwas, das auf diesem Rechner nicht ausführbar ist.",
        "`ChangeSetting` zurück auf das Standardkommando des Adapters.";
    /// Eine mitgelieferte Vorlage des Adapters (`opencode.json.tmpl`,
    /// `models.json`) ließ sich nicht als JSON lesen oder hat nicht die Form,
    /// die der Adapter erwartet. Das ist ein Fehler im Build, keine
    /// Nutzereingabe: die Dateien liegen unter `agents/` und werden
    /// einkompiliert (HUM-037).
    AGENT_003 => "agent", "Gebündelte Agenten-Vorlage unbrauchbar", "#agent_003",
        "Eine mitgelieferte Vorlage des Adapters ist unbrauchbar; das ist ein Fehler im Bau.",
        "Kein Fix: Der Text nennt die Datei.";
    /// Es ist kein Modell konfiguriert (`llm.models` ist leer). Der Adapter
    /// trägt ein Platzhalter-Modell in die Konfiguration des Agenten ein,
    /// damit er überhaupt startet; ob der LLM-Server dieses Modell kennt, weiß
    /// Humanitl nicht (HUM-037, HUM-039).
    LLM_004 => "llm", "Kein Modell konfiguriert", "#llm_004",
        "`llm.models` ist leer; der Agent bekommt einen Platzhalter statt eines Modells.",
        "`ChangeSetting` auf `llm.endpoint` oder `CopyCommand` mit einem `curl` auf `/models` — erst fragen, dann eintragen.";
    /// Das Kommando des Agenten liegt auf dem Host, aber an einer Stelle, die
    /// die Sandbox nicht einhängt. In der Sandbox scheitert dann das `exec`,
    /// und zwar erst nach dem Start. Der Befund nennt den gefundenen Pfad und
    /// einen Ort, an dem er erreichbar wäre (HUM-037).
    AGENT_004 => "agent", "Agent-Kommando in der Sandbox nicht erreichbar", "#agent_004",
        "Das Kommando liegt auf dem Rechner, aber nicht in dem, was die Sandbox einhängt.",
        "`CopyCommand`, das es an eine eingehängte Stelle installiert, sonst der Verweis auf die Dokumentation.";

    // HUM-039: die Durchreiche zum Sprachmodell und die Probe ihres Endpunkts.
    /// Eine durchgereichte Anfrage an das Sprachmodell trägt Funde: mögliche
    /// Geheimnisse oder personenbezogene Daten. Sie wird trotzdem gesendet,
    /// weil der LLM-Endpunkt die erklärte Vertrauensgrenze ist (BACKLOG.md
    /// 4.2); der Befund ist die Warnung, die davon übrig bleibt, und steht als
    /// bernsteinfarbene Zeile am Fluss. Er nennt die Zahl der Funde und den
    /// Host, nie den gefundenen Wert (HUM-039).
    LLM_005 => "llm", "Funde in einer durchgereichten Anfrage", "#llm_005",
        "Eine durchgereichte Anfrage an das Sprachmodell trägt Funde: Geheimnisse oder personenbezogene Daten.",
        "Kein Fix: Die Anfrage ist bereits gesendet; der Befund macht es sichtbar.";
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
    LLM_006 => "llm", "LLM-Endpunkt liegt nicht in einem privaten Netz", "#llm_006",
        "Der Endpunkt liegt nicht in einem privaten Netz.",
        "`ChangeSetting` auf `llm.endpoint`, wo die Probe ihn baut; der Verkehr dorthin geht an der Warteschlange vorbei.";
    /// Die Adresse in `llm.endpoint` lässt sich gar nicht als HTTP-Adresse
    /// lesen: ein anderes Schema als `http` oder `https`, kein Host, oder
    /// überhaupt keine URL. Es wurde nichts gemessen und nichts verbunden —
    /// deshalb nicht `LLM_001` und nicht `LLM_003`, die beide eine Beobachtung
    /// am Endpunkt behaupten würden (HUM-039).
    LLM_007 => "llm", "LLM-Endpunkt ist keine lesbare HTTP-Adresse", "#llm_007",
        "`llm.endpoint` ist keine lesbare HTTP-Adresse.",
        "`ChangeSetting` auf eine absolute `http`- oder `https`-Adresse.";
    /// Die Suche nach LLM-Servern im eigenen Netz kann nicht stattfinden.
    ///
    /// Drei Fälle, und keiner davon ist ein Fehler des gesuchten Servers: Es
    /// gibt keine Vorgaberoute, also kein eigenes Netz; die Routing-Tabelle
    /// ist nicht lesbar; oder das genannte Netz ist weiter als ein `/24`. Der
    /// letzte Fall ist eine Weigerung und keine Panne — der Text über dem
    /// Knopf verspricht, dass die Suche im eigenen `/24` bleibt, und ein `/16`
    /// wären 65 534 Verbindungsversuche in ein Netz, das dem Menschen
    /// vielleicht nicht gehört (HUM-076).
    LLM_008 => "llm", "Die Suche im Netz kann nicht stattfinden", "#llm_008",
        "Die Suche im Netz kann nicht stattfinden: keine Vorgaberoute, keine lesbare Routing-Tabelle, oder ein Netz jenseits des eigenen `/24`.",
        "Kein Fix: Der Text nennt beide Netze und die Grenze.";

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
    CLI_005 => "cli", "Es läuft schon eine Sitzung", "#cli_005",
        "In diesem Daemon läuft schon eine Sitzung.",
        "Kein Fix: Wer sie gestartet hat, beendet sie dort.";

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
    PROXY_009 => "proxy", "Anfrage ist keine Meta-Anfrage", "#proxy_009",
        "Ein Fluss soll als vom Proxy selbst beantwortet gelten, ging aber nicht an `humanitl.internal`.",
        "Kein Fix: Es ist ein Fehler im Daemon, keine Eingabe des Nutzers.";

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
    PROXY_010 => "proxy", "Verbindungsgrenze erreicht", "#proxy_010",
        "Die Zahl gleichzeitiger Verbindungen aus der Sandbox hat `limits.max_client_connections` erreicht.",
        "`ChangeSetting` auf eine höhere Grenze.";
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
    PROXY_011 => "proxy", "Anfrage-Rumpf ist stehengeblieben", "#proxy_011",
        "Ein Anfrage-Rumpf ist stehengeblieben und hat die Frist überschritten.",
        "`ChangeSetting` auf eine längere Frist.";
    // HUM-075: `humanitl doctor`. Ein Code je Prüfung, damit eine Zeile der
    // Ausgabe und ihr Befund nicht auseinanderlaufen können, dazu zwei für die
    // beiden Arten, nicht gemessen zu haben.
    /// `bubblewrap` fehlt oder ist älter als die Untergrenze des Launchers.
    ///
    /// Beide Fälle stehen unter demselben Code, weil derselbe Befehl beide
    /// behebt; welcher von beiden vorliegt, sagt das `why` und der Beleg der
    /// Zeile. Ohne `bwrap` gibt es keine Sandbox, deshalb blockierend
    /// (HUM-075).
    DOCTOR_001 => "doctor", "bubblewrap fehlt oder ist zu alt", "#doctor_001",
        "Die Prüfung dieses Rechners fand kein `bubblewrap` oder ein zu altes.",
        "`CopyCommand` mit dem Installationsbefehl.";
    /// Ein unprivilegierter Nutzer-Namensraum ließ sich nicht aufmachen.
    ///
    /// Gemessen wird mit `bwrap` und nicht mit `unshare`: Auf Ubuntu ab 23.10
    /// schränkt `AppArmor` unprivilegierte Namensräume ein, das ausgelieferte
    /// `bwrap` trägt dafür aber ein Profil. Der Befund nennt, was
    /// `/proc/sys/kernel/apparmor_restrict_unprivileged_userns` und
    /// `/proc/sys/kernel/unprivileged_userns_clone` dazu sagen, und
    /// unterscheidet dabei eine Datei, die es nicht gibt, von einer mit
    /// unbrauchbarem Wert (HUM-075).
    DOCTOR_002 => "doctor", "Nutzer-Namensräume nicht verfügbar", "#doctor_002",
        "Unprivilegierte Namensräume stehen auf diesem Rechner nicht zur Verfügung.",
        "`CopyCommand` mit dem `sysctl`-Aufruf.";
    /// Der Kernel taugt nicht für den seccomp-Filter des Shims.
    ///
    /// Blockierend, wenn `/proc/self/status` gar kein Feld `Seccomp` führt:
    /// Dann ist der Kernel ohne `CONFIG_SECCOMP` gebaut, und der Shim kann
    /// seinen Filter nicht laden. Eine Warnung, wenn der Kernel älter ist als
    /// die Fassung, gegen die der Filter gemessen wurde (HUM-075).
    DOCTOR_003 => "doctor", "Kernel ohne brauchbares seccomp", "#doctor_003",
        "Der Kernel hat kein brauchbares seccomp; die dritte Garantie wäre nicht zu halten.",
        "`OpenUrl` auf die Dokumentation zu seccomp — ohne den Kernel hilft keine Einstellung.";
    /// `$XDG_RUNTIME_DIR` fehlt, gehört einem anderen oder steht offen.
    ///
    /// Dort liegen der Socket des Daemons und das Sitzungs-Token. Ein
    /// Verzeichnis, in das Gruppe oder Welt hineindarf, wäre der bequemste Weg
    /// an jeder Entscheidung vorbei; eines, das einem anderen Nutzer gehört,
    /// ebenso (HUM-075).
    DOCTOR_004 => "doctor", "Laufzeitverzeichnis fehlt oder ist nicht privat", "#doctor_004",
        "Das Laufzeitverzeichnis fehlt oder ist nicht privat.",
        "`SetEnv` oder der Hinweis auf die Rechte.";
    /// Es gibt keine brauchbare systemd-Nutzersitzung.
    ///
    /// Nur eine Warnung: Ohne systemd startet man `humanitld` von Hand, und
    /// alles Weitere läuft genauso. Was fehlt, ist der Start beim Anmelden
    /// (HUM-075).
    DOCTOR_005 => "doctor", "Keine systemd-Nutzersitzung", "#doctor_005",
        "Es gibt keine systemd-Nutzersitzung, in der der Daemon laufen könnte.",
        "`CopyCommand` je nach Lage: `humanitld` von Hand, `systemctl --user --failed` oder `loginctl enable-linger`.";
    /// Der Daemon antwortet nicht oder spricht einen anderen Vertrag.
    ///
    /// Der Befund des Verbindungsversuchs steht im `why`, mit seinem eigenen
    /// Code davor; der Doctor führt je Zeile genau einen Code, damit Zeile und
    /// Befund zusammenbleiben. Eine andere Major-Version ist blockierend, eine
    /// fehlende Verbindung nur eine Warnung: Der Doctor selbst läuft auch ohne
    /// Daemon, und genau dafür ist er da (HUM-075).
    DOCTOR_006 => "doctor", "Daemon nicht erreichbar oder anderer Vertrag", "#doctor_006",
        "Der Daemon antwortet nicht oder spricht einen anderen Vertrag.",
        "`CopyCommand` mit dem Startbefehl.";
    /// Das Kommando des Agenten liegt nicht im `PATH` des Hosts.
    ///
    /// Eine Warnung und kein Fehler: Der Pfad in der Sandbox kann ein anderer
    /// sein als der auf dem Host, und ob das `exec` gelingt, entscheidet der
    /// Shim. Ohne das Kommando läuft der Agent aber nicht (HUM-075, HUM-037).
    DOCTOR_007 => "doctor", "Agent-Kommando nicht im PATH", "#doctor_007",
        "Das Agent-Kommando liegt nicht im `PATH` des Daemons.",
        "`CopyCommand` mit dem Installationsbefehl des Agenten.";
    /// Der Endpunkt des Sprachmodells fehlt oder hat nicht geantwortet.
    ///
    /// Steht nur nach einer Messung. Der Befund der Endpunkt-Probe steht mit
    /// seinem Code im `why`. Wurde gar nicht gemessen, ist es `DOCTOR_013`
    /// (HUM-075, HUM-039).
    DOCTOR_008 => "doctor", "Sprachmodell nicht erreichbar", "#doctor_008",
        "Das Sprachmodell war nicht erreichbar, als jemand danach gefragt hat — oder es ist gar keines eingetragen.",
        "`CopyCommand` mit dem `curl` auf den Endpunkt; ohne Eintrag `ChangeSetting` auf `llm.endpoint`.";
    /// Die Arbeitsumgebung hat keinen Platz für das Anzeigesymbol.
    ///
    /// Entweder fehlt `libayatana-appindicator3`, oder die Sitzung ist GNOME,
    /// das seit 3.26 keinen eigenen Bereich für Anzeigesymbole mehr hat und
    /// die AppIndicator-Erweiterung braucht. Die Anwendung läuft weiter; der
    /// Zähler der wartenden Anfragen steht dann im Fenstertitel (HUM-075,
    /// HUM-034).
    DOCTOR_009 => "doctor", "Kein Platz für das Anzeigesymbol", "#doctor_009",
        "Die Arbeitsumgebung bietet keinen Platz für das Anzeigesymbol.",
        "`CopyCommand` für das fehlende Portal oder die Erweiterung; ohne sie bleibt das Fenster der Weg.";
    /// Renderer und Grafiktreiber vertragen sich voraussichtlich nicht.
    ///
    /// Der bekannte Fall ist Impeller auf einem geladenen NVIDIA-Modul unter
    /// Wayland: Die Oberfläche startet und bleibt schwarz. Der Befund nennt
    /// den Schalter, mit dem sie ohne Impeller startet (HUM-075).
    DOCTOR_010 => "doctor", "Renderer und Grafiktreiber vertragen sich nicht", "#doctor_010",
        "Renderer und Grafiktreiber vertragen sich nicht; das Fenster bliebe schwarz.",
        "`OpenUrl` auf die Dokumentation zu Impeller und dem Software-Renderer.";
    /// Im Datenverzeichnis ist wenig Platz.
    ///
    /// Die Aufzeichnung legt dort Flows und Bodies ab. Der Vorschlag ist eine
    /// kürzere Aufbewahrung, nicht das Abschalten der Aufzeichnung: Ohne sie
    /// gilt die Zusage „alles wird aufgezeichnet" nicht mehr (HUM-075,
    /// HUM-026).
    DOCTOR_011 => "doctor", "Wenig Platz im Datenverzeichnis", "#doctor_011",
        "Im Datenverzeichnis ist wenig Platz; Aufzeichnung und Blobs wachsen dort.",
        "`ChangeSetting` auf eine kürzere `recorder.retention_days` — oder Platz schaffen.";
    /// Eine Prüfung ließ sich auf diesem Rechner nicht durchführen.
    ///
    /// Der einzige ehrliche Ausgang, wenn die Quelle fehlt oder nicht lesbar
    /// ist: Eine Prüfung, die nicht nachsehen konnte, ist nicht grün. Das `why`
    /// nennt die Prüfung und den Grund, der `fix` den Befehl, den der Doctor
    /// versucht hat und den ein Mensch von Hand nachfahren kann (HUM-075).
    DOCTOR_012 => "doctor", "Prüfung nicht durchführbar", "#doctor_012",
        "Eine Prüfung ließ sich auf diesem Rechner nicht durchführen; niemand hat sie bestanden oder verfehlt.",
        "Der Fix nennt den Befehl, der sie ausführen würde.";
    /// Der Endpunkt des Sprachmodells wurde nicht angesprochen.
    ///
    /// Nicht `DOCTOR_012`: Dort konnte der Doctor nicht nachsehen, hier wollte
    /// er nicht. Die Erreichbarkeit des Sprachmodells ist die einzige Prüfung,
    /// hinter der eine Verbindung stünde, und sie läuft nie als Nebenwirkung
    /// eines anderen Befehls oder beim Öffnen eines Bildschirms. Der `fix`
    /// nennt den Befehl, der sie auslöst (HUM-075, HUM-039).
    DOCTOR_013 => "doctor", "Sprachmodell nicht angesprochen", "#doctor_013",
        "Das Sprachmodell wurde nicht angesprochen, weil niemand darum gebeten hat.",
        "`CopyCommand` mit `humanitl doctor --probe-llm`.";
    /// `SetConfig` nimmt diesen Schlüssel oder diesen Wert nicht an.
    ///
    /// Der Schreibweg ist mit Absicht schmal (HUM-151): Er richtet eine
    /// Variable unter `sandbox.env` auf das Zertifikat, das Humanitl in der
    /// Sandbox einhängt, und tut sonst nichts. Alles Weitere kommt mit dem
    /// Einstellungen-Bildschirm (HUM-069).
    CONFIG_014 => "config", "Einstellung nicht über den Daemon setzbar", "#config_014",
        "`SetConfig` nimmt bis HUM-069 nur eine Variable unter `sandbox.env` an, deren Wert das Zertifikat in der Sandbox ist.",
        "Kein Fix: Der Text nennt, was angenommen wird; alles andere steht von Hand in `config.toml`.";
    /// `config.toml` wurde nicht geschrieben.
    ///
    /// Entweder steht `sandbox` oder `sandbox.env` dort in einer Form, die sich
    /// nicht ändern lässt, ohne mehr als den einen Wert zu ändern, oder die
    /// Datei ließ sich nicht anlegen oder ersetzen. In beiden Fällen ist die
    /// Datei unberührt (HUM-151).
    CONFIG_015 => "config", "config.toml nicht geschrieben", "#config_015",
        "`config.toml` ließ sich nicht ändern, ohne mehr als den einen Wert zu ändern, oder nicht schreiben; sie ist unberührt.",
        "Kein Fix: Der Text nennt die Zeile, die von Hand in den Block `[sandbox.env]` gehört.";
    /// Die letzte Zeile von `audit.jsonl` war unvollständig und liegt jetzt
    /// daneben.
    ///
    /// Ein hart beendeter Daemon kann mitten in einer Zeile aufhören. Der
    /// Schreiber legt den Rest beim nächsten Start als
    /// `audit.jsonl.corrupt-<ts>` ab und setzt die Kette am letzten
    /// vollständigen Record fort. Es entsteht keine Lücke; der verlorene
    /// Record fehlt, und das ist die dokumentierte Grenze „nie geschriebene
    /// Ereignisse" (HUM-050).
    AUDIT_002 => "audit", "Unvollständige letzte Zeile beiseitegelegt", "#audit_002",
        "Beim Start endet `audit.jsonl` nicht mit einem Zeilenumbruch; der Rest hinter dem letzten vollständigen Record wurde in eine eigene Datei verschoben.",
        "Kein Fix nötig: Der Text nennt die Datei mit dem Rest, und die Kette läuft weiter.";
    /// Ein Record trug Daten, die sich nicht kanonisch schreiben lassen.
    ///
    /// Ein Programmfehler: Jede Art von Record trägt nur Ganzzahlen, Strings,
    /// Booleans und Listen davon. Im Debug-Build hält der Schreiber an, im
    /// Release-Build schreibt er den Record mit `data: {"error":"non_canonical"}`,
    /// damit die Kette keine Lücke bekommt (HUM-050).
    AUDIT_003 => "audit", "Audit-Daten nicht kanonisch", "#audit_003",
        "Die Daten eines Records enthielten eine Zahl, die keine Ganzzahl ist; geschrieben wurde der Record mit einem Platzhalter statt der Daten.",
        "Kein Fix für Nutzer: ein Programmfehler; der Text nennt die Art des Records.";
    /// Ein anderer Prozess hält die Sperre auf `audit.jsonl`.
    ///
    /// Zwei Daemons dürfen nie in dieselbe Kette schreiben: Beide hielten
    /// `seq` und Hash des Endes im Speicher, und die Zeilen des einen hingen
    /// an einem Ende, das der andere schon überschrieben hat (HUM-050).
    AUDIT_004 => "audit", "Audit-Log von einem anderen Daemon belegt", "#audit_004",
        "Beim Start hält ein anderer Prozess die exklusive Sperre auf `audit.jsonl`.",
        "`CopyCommand` mit `humanitl daemon status`: den laufenden Daemon finden und beenden.";
    /// Der HMAC-Schlüssel des Audit-Logs ist nicht benutzbar.
    ///
    /// Bis HUM-048 den Keyring bringt, liegt er als Datei im Datenverzeichnis
    /// und wird geprüft wie der Schlüssel der CA: reguläre Datei, `0600`, dem
    /// Nutzer gehörend, genau 32 Bytes (HUM-050).
    AUDIT_005 => "audit", "Audit-Schlüssel unbrauchbar", "#audit_005",
        "Die Schlüsseldatei ist ein Symlink oder keine reguläre Datei, trägt Rechte für Gruppe oder Andere, gehört einem anderen Nutzer, hat nicht genau 32 Bytes oder lässt sich nicht anlegen.",
        "`CopyCommand`: `rm` für einen verbrannten Schlüssel, sonst `ls -ln` auf die Datei oder `mkdir`/`chmod` auf das Verzeichnis.";
    /// `audit.jsonl` oder die Anker-Tabelle lässt sich nicht öffnen, lesen,
    /// schreiben oder synchronisieren (HUM-050).
    AUDIT_006 => "audit", "Audit-Log nicht schreibbar", "#audit_006",
        "Datei, Verzeichnis oder Anker-Tabelle des Audit-Logs lassen sich nicht öffnen, lesen, schreiben oder auf die Platte bringen.",
        "`CopyCommand` mit `ls -ld` und `df -h` auf das Verzeichnis — Rechte oder Platz.";
    /// Das Audit-Log endete beim Start vor einem Anker; die Kette läuft hinter
    /// dem letzten Anker weiter.
    ///
    /// Wer `audit.jsonl` löscht, kürzt oder nach einem `AUDIT_001`
    /// beiseitelegt, lässt die Anker in `audit_anchors` zurück. Der Daemon
    /// startet trotzdem: Er schreibt `audit.resumed` unter der Nummer hinter dem
    /// letzten Anker, mit dessen Hash als Vorgänger. Die Lücke bleibt für
    /// `verify` ein Bruch, und die Anker bleiben als Beleg liegen (HUM-050).
    AUDIT_007 => "audit", "Audit-Kette hinter dem letzten Anker fortgesetzt", "#audit_007",
        "Beim Start endet `audit.jsonl` vor einem Anker aus `audit_anchors`; die Kette läuft hinter diesem Anker weiter, und die Prüfung meldet die Lücke weiter als Bruch.",
        "Kein Fix nötig: Der Text nennt, wo das Log endete und hinter welchem Anker die Kette weiterläuft.";
    /// Die Tabelle `resolver.overrides` ist nicht leer.
    ///
    /// Für die Namen darin antwortet die Konfiguration und nicht der
    /// Namensdienst, und der Verkehr geht an die Adresse, die dort steht. Das
    /// ist ein Testhebel; der Start sagt es, damit er nicht unbemerkt in einem
    /// Alltagslauf steht, so wie `CONFIG_011` es für `resolver.test_ca` tut
    /// (HUM-024).
    CONFIG_016 => "config", "Feste Namenszuordnungen gesetzt", "#config_016",
        "`resolver.overrides` beantwortet die genannten Namen aus der Konfiguration, statt zu fragen; der Verkehr geht an die Adresse, die dort steht.",
        "`ChangeSetting` auf eine leere Tabelle, wenn die festen Adressen nicht gemeint waren.";
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
