---
source: adr/0012-diagnostics-as-type.md
source-sha256: f81d44cab9456cb4e244d1113d60f8540c48426592438f1555ea1948f9cdfba9
lang: de
---
# ADR-0012 · Geführte Zustände als Typ: `Diagnostic` statt Fehler-String

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-09-02

## Kontext

Humanitl steht auf einem Stapel aus Kernel-Fähigkeiten, Systemdiensten und
Fremdprogrammen: unprivilegierte User-Namespaces, seccomp, bubblewrap in einer
bestimmten Version, eine systemd-User-Session, ein `$XDG_RUNTIME_DIR`, ein
LLM-Server irgendwo im LAN, OpenCode im `PATH`, ein Projektordner mit den
richtigen Rechten, ein Zertifikat, das der Agent akzeptieren muss. Jedes dieser
Teile kann fehlen oder falsch eingerichtet sein.

Die Zielgruppe kann diese Fehler nicht diagnostizieren. Ein „Permission denied"
im Log ist für sie das Ende, nicht der Anfang. Prinzip 7 nennt die Anforderung:
Jeder Zustand, der nicht grün ist, trägt einen Grund in verständlicher Sprache
und eine Aktion, die ihn behebt. Kein Fehler ohne „warum" und „was jetzt".

Als Konvention lässt sich das nicht durchhalten. Wenn ein Fehlerpfad einen
`String` zurückgeben *darf*, wird er es tun, meist unter Zeitdruck, meist an der
Stelle, an der es am meisten wehtut. Oberfläche und CLI müssten dann jede für
sich raten, wie sie einen Fehler darstellen, und die Abhilfe bliebe Prosa.

## Entscheidung

Fehlerpfade geben einen Wert zurück, keinen Text:

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

Der Typ lebt in `humanitl-core`. `FixAction` ist eine **aufzählbare Aktion**,
keine Textanweisung: Oberfläche und CLI stellen dieselbe Aktion jeweils auf ihre
Weise dar (als Knopf beziehungsweise als kopierbaren Befehl), und ein Klick führt
sie aus.

**Codes werden registriert, nicht erfunden.** Alle Codes sind Konstanten mit
einem Doku-Kommentar in
`daemon/crates/core-types/src/diagnostics/codes.rs`; die Tabelle `AREAS` in
derselben Datei ist die verbindliche Liste der reservierten Bereiche. Stand
heute: `DAEMON_001..019`, `IPC_001..009`, `CONFIG_001..009`,
`SANDBOX_001..029`, `PROXY_001..009`, `TLS_001..009`, `LLM_001..009`,
`RULES_001..009`, `TERM_001..009`, `RECORDER_001..009`, `LIMIT_001..009`,
`AUDIT_001..009`, `DOCTOR_001..019`, `CLI_001..009`
(`backlog/CONVENTIONS.md` 4.6 nennt die ursprüngliche Teilmenge). Ein Code wird
**nie wiederverwendet**; entfernte Codes bleiben als `#[deprecated]` stehen.
Ein CI-Test prüft, dass jeder im Code benutzte Diagnose-Code im Register steht
und in seinem Bereich liegt.

**Darstellung.** Die Oberfläche zeigt Diagnostics immer am Ort des Problems, auf
der Karte, auf der Zeile, am Eintrag der Checkliste, **nie als Modal**. Die CLI
zeigt sie als Block mit `why:` und `fix:`.

**Definition of Done.** Jedes Issue, das einen neuen Fehlerpfad einführt,
liefert dafür eine `Diagnostic` mit `why` und, wo möglich, `fix`. Kein
`Err(String)` in öffentlichen Daemon-Pfaden; der CI-Lint prüft das.

## Begründung

Ein Typ erzwingt Vollständigkeit. Wer eine `Diagnostic` baut, muss `code`,
`severity`, `title` und `why` angeben; er kann nicht versehentlich nur die
technische Ursache zurücklassen. Bei einem `String` ist das Gegenteil der
Normalfall.

`FixAction` als Enum statt als Text ist die Stelle, an der aus einer
Fehlermeldung Hilfe wird. „Setze `SSL_CERT_FILE` auf `/etc/humanitl/ca.crt`" ist
eine Anweisung, die der Nutzer ausführen muss. `FixAction::SetEnv { … }` ist
etwas, das die Oberfläche als Knopf zeigt und die CLI als kopierbaren Befehl
ausgibt, und beide meinen nachweislich dasselbe, weil es derselbe Wert ist.
Prinzip 9 verlangt genau das: Was fehlt, wird angeboten, nie bloß gemeldet.

Der stabile Code macht Fehler adressierbar. Ein Nutzer kann `SANDBOX_007` in eine
Suche tippen, ein Fehlerbericht wird eindeutig, und die Dokumentation kann direkt
darauf verweisen. Dass Codes nie wiederverwendet werden, hält alte Fehlerberichte
und alte Dokumentation gültig. Das Register mit CI-Prüfung verhindert den
naheliegenden Verfall: zwei Stellen, die denselben Code für verschiedene Dinge
benutzen.

Diagnostics inline statt im Modal ist keine Geschmacksfrage. Ein Modal reißt den
Nutzer aus dem Zusammenhang, muss weggeklickt werden und ist danach weg, genau
dann, wenn der Nutzer die Information braucht. Am Ort des Problems bleibt sie, so
lange das Problem besteht. Das passt zur Grundhaltung der Oberfläche (ADR-0009):
Entscheidungen inline, nie modal.

`severity` trennt „das ist ein Hinweis" von „hier geht nichts weiter".
`Blocking` ist der Zustand, in dem der Sandbox-Bildschirm den Start sperrt und nie
„trotzdem starten" anbietet; eine Sicherheitsprüfung, die sich wegklicken lässt,
ist keine.

## Verworfene Alternativen

- **`anyhow` mit Kontextketten.** In `main` sinnvoll und dort erlaubt. Für
  Bibliotheks-Crates verworfen: Die Kette ist für Entwickler geschrieben, nicht
  für Nutzer, und sie trägt keine Abhilfe. `thiserror` pro Crate plus
  `Diagnostic` an der Oberfläche ist die Aufteilung.
- **`Err(String)` mit der Konvention „bitte freundlich formulieren".** Genau das
  Modell, dessen Scheitern man in vielen Programmen sehen kann.
- **Fehlercodes als Zahlen (`E1042`).** Kompakt und unlesbar. `SANDBOX_007` sagt
  schon beim Lesen, wo man suchen muss.
- **Abhilfe als Freitext.** Halbherzig: Die Oberfläche könnte sie zeigen, aber
  nicht ausführen. Der Unterschied zwischen „hier steht, was du tun sollst" und
  „hier ist der Knopf" ist für die Zielgruppe der ganze Unterschied.
- **Übersetzung des Fehlertexts im Daemon.** Verworfen: Der Daemon liefert
  `code` plus strukturierte Felder, die Übersetzung geschieht in der Oberfläche
  über ARB-Schlüssel. Sonst hätte der Daemon eine Sprache und die CLI eine
  zweite.
- **Diagnostics als modaler Dialog.** Siehe oben; widerspricht der grundlegenden
  Haltung der Interaktion.
- **Kein Register, Codes nach Bedarf vergeben.** Ginge schneller und brächte
  binnen eines Jahres Duplikate und Lücken hervor.

## Folgen

- `Diagnostic.docs` und `FixAction::OpenUrl` tragen `String`, nicht `url::Url`, damit das Crate `url` aus `humanitl-core` draußen bleibt (CONVENTIONS 4.11).

- `Diagnostic`, `Severity`, `FixAction` und `DiagnosticCode` sind Kerntypen und
  haben eine Proto-Abbildung, damit sie über den Ereignisstrom und in
  RPC-Antworten transportiert werden können.
- `FlowEvent` hat eine Variante `Diagnostic`; ein Problem lässt sich daher an
  einen Flow hängen, nicht nur an ein RPC-Ergebnis.
- Es gibt einen Darstellungsvertrag für Oberfläche und CLI: dieselben Felder,
  dieselbe Reihenfolge, dieselbe Bedeutung. Snapshot-Tests decken alle benutzten
  Codes ab (HUM-068).
- `humanitl doctor` ist im Kern eine Liste von Prüfungen, von denen jede eine
  `Diagnostic` liefert. Deshalb kann er `--json` ausgeben und im Bildschirm
  „Einrichten" dieselbe Information zeigen wie im Terminal.
- Neue Fehlerpfade kosten etwas mehr Aufwand als ein `bail!`. Das ist gewollt:
  Der Aufwand fällt einmal beim Schreiben an und erspart jedem Nutzer eine
  Sackgasse.
- `FixAction::AddRule` nimmt eine `Box<Rule>` (`backlog/CONVENTIONS.md` 4.1),
  damit `Diagnostic` klein bleibt und kein eigener Typ für einen Regelentwurf
  nötig ist.

## Betroffene Issues

`HUM-063` (Diagnostic-Typ, Proto-Abbildung, Lint gegen `Err(String)`),
`HUM-068` (geführte Diagnostics im Sandbox-Bildschirm, Snapshot-Tests für alle
Codes), `HUM-045` (Erkennung von TLS-Fehlern mit `FixAction::SetEnv`), `HUM-075`
(`humanitl doctor`), `HUM-044` (Einrichtungsablauf mit Checkliste), `HUM-077`
(`FixAction::InstallService`).
