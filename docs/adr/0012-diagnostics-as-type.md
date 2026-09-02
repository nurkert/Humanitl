# ADR-0012 · Geführte Zustände als Typ: `Diagnostic` statt Fehlerstring
Status: Accepted
Datum: 2026-09-02

## Kontext

Humanitl steht auf einem Stapel aus Kernel-Fähigkeiten, Systemdiensten und
fremden Programmen: unprivilegierte User-Namespaces, seccomp, bubblewrap in
einer bestimmten Version, eine systemd user session, ein `$XDG_RUNTIME_DIR`,
ein LLM-Server irgendwo im LAN, OpenCode im `PATH`, ein Projektordner mit den
richtigen Rechten, ein Zertifikat, das der Agent akzeptieren muss. Jedes dieser
Teile kann fehlen oder falsch stehen.

Die Zielgruppe kann diese Fehler nicht diagnostizieren. Ein „Permission denied"
im Log ist für sie das Ende, nicht der Anfang. Prinzip 7 formuliert die
Anforderung: Jeder Zustand, der nicht grün ist, trägt einen Grund in Klartext und
eine Aktion, die ihn behebt. Kein Fehler ohne „Warum" und „Was jetzt".

Das lässt sich nicht als Konvention durchhalten. Wenn ein Fehlerpfad einen
`String` zurückgeben *darf*, wird er es tun — meist unter Zeitdruck, meist an der
Stelle, an der es am meisten wehtut. UI und CLI müssten dann jeder für sich
raten, wie ein Fehler darzustellen ist, und der Fix bliebe Prosa.

## Entscheidung

Fehlerpfade liefern einen Wert, keinen Text:

```rust
pub struct Diagnostic {
    pub code: DiagnosticCode,
    pub severity: Severity,      // Info | Warning | Error | Blocking
    pub title: String,
    pub why: String,
    pub fix: Option<FixAction>,
    pub docs: Option<String>,
}

pub enum FixAction {
    SetEnv { key: String, value: String },
    AddRule(Box<Rule>),
    InstallService,
    ChangeSetting { key: String, value: String },
    CopyCommand(String),
    OpenUrl(String),
    RemountReadOnly(PathBuf),
}

pub struct DiagnosticCode(pub &'static str);   // Schema: BEREICH_NNN
```

Der Typ liegt in `humanitl-core`. `FixAction` ist eine **aufzählbare Aktion**,
keine Textanweisung: UI und CLI rendern dieselbe Aktion je auf ihre Art (Knopf
beziehungsweise kopierbarer Befehl), und ein Klick führt sie aus.

**Codes sind registriert, nicht erfunden.** Alle Codes stehen als Konstanten mit
Dokumentationskommentar in
`daemon/crates/core-types/src/diagnostics/codes.rs`; die Tabelle `AREAS` in
derselben Datei ist die verbindliche Liste der reservierten Bereiche. Stand
heute: `DAEMON_001..019`, `IPC_001..009`, `CONFIG_001..009`,
`SANDBOX_001..029`, `PROXY_001..009`, `TLS_001..009`, `LLM_001..009`,
`RULES_001..009`, `TERM_001..009`, `RECORDER_001..009`, `LIMIT_001..009`,
`AUDIT_001..009`, `DOCTOR_001..019`, `CLI_001..009`
(`backlog/CONVENTIONS.md` 4.6 nennt die ursprüngliche Teilmenge). Ein Code wird
**nie wiederverwendet**; entfernte Codes bleiben als `#[deprecated]` stehen.
Ein CI-Test prüft, dass jeder im Code verwendete Diagnostic-Code im Register
steht und innerhalb seines Bereichs liegt.

**Darstellung.** Das UI zeigt Diagnostics immer am Ort des Problems — an der
Karte, an der Zeile, am Checklisten-Eintrag —, **nie als Modal**. Die CLI zeigt
sie als Block mit `why:` und `fix:`.

**Definition of Done.** Jedes Issue, das einen neuen Fehlerpfad einführt, liefert
dafür ein `Diagnostic` mit `why` und, wo möglich, `fix`. Kein `Err(String)` in
öffentlichen Daemon-Pfaden; der CI-Lint prüft das.

## Begründung

Ein Typ zwingt zur Vollständigkeit. Wer `Diagnostic` konstruiert, muss `code`,
`severity`, `title` und `why` angeben; er kann nicht versehentlich nur die
technische Ursache hinterlassen. Bei einem `String` ist das Gegenteil der
Normalfall.

`FixAction` als Enum statt als Text ist der Punkt, an dem aus einer
Fehlermeldung Hilfe wird. „Setze `SSL_CERT_FILE` auf `/etc/humanitl/ca.crt`" ist
eine Anweisung, die der Nutzer ausführen muss. `FixAction::SetEnv { … }` ist
etwas, das das UI als Knopf zeigt und die CLI als kopierbaren Befehl ausgibt —
und beide meinen nachweislich dasselbe, weil es derselbe Wert ist. Prinzip 9
verlangt genau das: Fehlendes wird angeboten, nie nur gemeldet.

Der stabile Code macht Fehler adressierbar. Ein Nutzer kann `SANDBOX_007` in
eine Suche eingeben, ein Bugreport wird eindeutig, und die Dokumentation kann
gezielt darauf verweisen. Dass Codes nie wiederverwendet werden, hält alte
Bugreports und alte Dokumentation gültig. Das Register mit CI-Prüfung verhindert
den offensichtlichen Verfall: zwei Stellen, die denselben Code für
Verschiedenes benutzen.

Diagnostics inline statt modal ist keine Geschmacksfrage. Ein Modal reißt aus
dem Kontext, muss weggeklickt werden und ist danach weg — genau dann, wenn der
Nutzer die Information braucht. Am Ort des Problems bleibt sie stehen, solange
das Problem besteht. Das passt zur Grundhaltung der Oberfläche (ADR-0009):
Entscheidungen inline, nie modal.

`severity` trennt „das ist ein Hinweis" von „hier geht nichts weiter".
`Blocking` ist der Zustand, in dem der Sandbox-Screen den Start deaktiviert und
nie „trotzdem starten" anbietet — eine Sicherheitsprüfung, die man wegklicken
kann, ist keine.

## Verworfene Alternativen

- **`anyhow` mit Kontextketten.** In `main` sinnvoll und dort auch erlaubt. Für
  Bibliotheks-Crates verworfen: Die Kette ist für Entwickler geschrieben, nicht
  für Nutzer, und sie trägt keinen Fix. `thiserror` pro Crate plus
  `Diagnostic` an der Oberfläche ist die Aufteilung.
- **`Err(String)` mit Konvention „bitte freundlich formulieren".** Genau das
  Modell, dessen Scheitern man in vielen Programmen sehen kann.
- **Fehlercodes als Zahlen (`E1042`).** Kompakt und unlesbar. `SANDBOX_007`
  sagt schon beim Lesen, wo man suchen muss.
- **Fix als Freitext.** Halber Weg: Das UI könnte ihn anzeigen, aber nicht
  ausführen. Der Unterschied zwischen „hier steht, was du tun sollst" und „hier
  ist der Knopf" ist für die Zielgruppe der ganze Unterschied.
- **Übersetzung des Fehlertextes im Daemon.** Verworfen: Der Daemon liefert
  `code` plus strukturierte Felder, die Übersetzung geschieht in der Oberfläche
  über ARB-Schlüssel. Sonst hätte der Daemon eine Sprache und die CLI eine
  zweite.
- **Diagnostics als Modal-Dialog.** Siehe oben; widerspricht der
  Interaktionsgrundhaltung.
- **Kein Register, Codes ad hoc vergeben.** Wäre schneller und würde innerhalb
  eines Jahres Duplikate und Lücken erzeugen.

## Konsequenzen

- `Diagnostic.docs` und `FixAction::OpenUrl` tragen `String`, nicht `url::Url`, damit die `url`-Crate aus `humanitl-core` bleibt (CONVENTIONS 4.11).

- `Diagnostic`, `Severity`, `FixAction` und `DiagnosticCode` sind Kerntypen und
  haben eine Proto-Abbildung, damit sie über den Ereignisstrom und in
  RPC-Antworten transportiert werden können.
- `FlowEvent` hat eine Variante `Diagnostic`; ein Problem kann also an einem Flow
  hängen, nicht nur an einem RPC-Ergebnis.
- Es gibt einen Renderer-Vertrag für UI und CLI: gleiche Felder, gleiche
  Reihenfolge, gleiche Bedeutung. Snapshot-Tests decken alle verwendeten Codes
  ab (HUM-068).
- `humanitl doctor` ist im Wesentlichen eine Liste von Prüfungen, die jeweils ein
  `Diagnostic` liefern. Deshalb kann er `--json` ausgeben und im Setup-Bildschirm
  dieselbe Information zeigen wie im Terminal.
- Neue Fehlerpfade kosten etwas mehr Aufwand als ein `bail!`. Das ist
  beabsichtigt: Der Aufwand fällt einmal beim Schreiben an und spart jedem Nutzer
  eine Sackgasse.
- `FixAction::AddRule` nimmt eine `Box<Rule>` (`backlog/CONVENTIONS.md` 4.1),
  damit `Diagnostic` klein bleibt und kein separater Regel-Stub-Typ nötig ist.

## Betroffene Issues

`HUM-063` (Diagnostic-Typ, Proto-Abbildung, Lint gegen `Err(String)`),
`HUM-068` (geführte Diagnostics im Sandbox-Screen, Snapshot-Tests für alle
Codes), `HUM-045` (TLS-Fehler-Erkennung mit `FixAction::SetEnv`), `HUM-075`
(`humanitl doctor`), `HUM-044` (Setup-Flow mit Checkliste), `HUM-077`
(`FixAction::InstallService`).
