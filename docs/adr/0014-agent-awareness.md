# ADR-0014 · Agent-Bewusstsein und Feedback über den einen vorhandenen Kanal
Status: Accepted
Datum: 2026-09-02

## Kontext

Ein LLM-Agent in der Humanitl-Sandbox erlebt eine Umgebung, für die er nicht
trainiert wurde. Sein `webfetch` hängt fünf Minuten und liefert dann `403`. Sein
Update-Check schlägt fehl. Ein `curl` läuft in einen Timeout. Ohne Erklärung
reagiert er so, wie Agenten auf Fehler reagieren: Er wiederholt, variiert die
URL, probiert einen anderen Weg und verbrennt dabei Zeit und Token — und der
Mensch sieht eine Warteschlange voller Wiederholungen desselben blockierten
Requests.

Umgekehrt hat der Mensch beim Blocken oft einen Grund, den der Agent gebrauchen
könnte: „nicht GitHub, nimm PyPI", „diese Domain nicht, das ist Telemetrie".
Heute verpufft dieser Grund.

Die Versuchung ist ein zweiter Kanal: ein Steuersocket, eine Datei, die der Agent
liest, ein Sidecar. Jeder davon ist eine neue Tür in die Sandbox und damit ein
Riss in der Garantie „genau eine Tür" (ADR-0002).

## Entscheidung

Alles läuft über den einen vorhandenen Kanal, den Proxy-Socket. Kein neuer Kanal,
keine neue Fähigkeit für den Agenten, keine neue Lücke in der Garantie.

**1. Briefing.** Der `AgentAdapter` legt beim Start eine globale
Instruktionsdatei **in der Sandbox** an — bei OpenCode
`~/.config/opencode/AGENTS.md`, bei Claude Code `~/.claude/CLAUDE.md` — und
**nie im Projektverzeichnis**. Inhalt ist eine gebündelte Vorlage von etwa 150
Token in der Sprache des Nutzers (`ui.language`). Kernaussagen: kein direkter
Internetzugang; jede HTTP-Anfrage geht durch einen Proxy, ein Mensch
entscheidet; Warten ist normal und kein Grund zum Abbrechen; eine Antwort `403`
mit `Blocked by Humanitl` ist endgültig und wird nicht wiederholt — stattdessen
den Nutzer informieren und eine Alternative vorschlagen; Statusabfrage unter
`http://humanitl.internal/`.

**2. Feedback beim Blocken.** Die Aktionsleiste hat ein optionales, aufklappbares
Notizfeld. Der Text landet im `403`-Body unter `note:` und im Header
`X-Humanitl-Note`. Der Agent sieht ihn im Ergebnis seines Werkzeugaufrufs und
kann darauf reagieren. Die Notiz wird vor beidem gesäubert (`sanitize_note` in
`humanitl-core`, `backlog/CONVENTIONS.md` 4.11): `CR` und `LF` werden zu
Leerzeichen, andere Steuerzeichen fallen weg, ebenso unsichtbare Zeichen
(Zero-Width-Zeichen, Bidi-Overrides, BOM, weiches Trennzeichen, Tag-Zeichen),
die im Terminal des Agenten einen anderen Text vortäuschen könnten als den, den
der Nutzer geschrieben hat. Es bleiben höchstens 500 Zeichen. Der Header-Wert
ist zusätzlich auf sichtbare ASCII-Zeichen plus `SP` und `HTAB` beschränkt
(RFC 9110 §5.5); Nicht-ASCII steht nur im Body (keine Header-Injection).

**3. Meta-Endpunkt.** Der Proxy bedient den virtuellen Host
`humanitl.internal` selbst, ohne Upstream und ohne DNS, mit kleinen
`text/plain`-Antworten:

| Pfad | Methode | Antwort |
|---|---|---|
| `/` | GET | Kurzstatus: Sitzung, Ask-Modus, Timeout, gültige Regeln (eine Zeile pro Regel) |
| `/why/<flow-id>` | GET | Begründung der Entscheidung samt Notiz |
| `/ask` | POST | Freitext bis 2 KB; erzeugt ein `AgentAsk`-Ereignis und im UI eine Karte „Der Agent bittet um …" mit Regelvorschlag |

Andere Methoden werden abgelehnt. `/ask` erzeugt **nur** eine Karte im UI,
**nie** eine Regel.

**4. Sichtbarkeit im Terminal.** Wird ein Request gehalten, schreibt der Daemon
eine Statuszeile in das Terminal des Agenten — über den PTY-Strom, nicht in
`stdin`. Der Mensch, der auf das Terminal schaut, sieht damit, was gerade hängt.

**Was das ausdrücklich nicht ist:** Der Agent bekommt keine Möglichkeit, Regeln
anzulegen, Entscheidungen zu beeinflussen oder den Nutzer zu umgehen. `/ask` ist
eine Bitte, keine Aktion.

## Begründung

Der Proxy-Socket existiert bereits, ist bereits die einzige Tür, wird bereits
vollständig aufgezeichnet und unterliegt bereits der Regelauswertung. Ihn für
Statusinformation und Feedback mitzubenutzen, kostet keine zusätzliche
Angriffsfläche: Ein Request an `humanitl.internal` ist ein Request wie jeder
andere, nur dass ihn der Proxy selbst beantwortet.

Das Briefing kostet etwa 150 Token und spart ein Vielfaches davon. Ein Agent, der
weiß, dass `403 Blocked by Humanitl` endgültig ist, wiederholt nicht. Ein Agent,
der weiß, dass Warten normal ist, bricht nicht nach dreißig Sekunden ab. Das ist
die billigste Verbesserung im ganzen Entwurf.

Dass das Briefing in ein **globales** Konfigurationsverzeichnis der Sandbox geht
und nicht ins Projektverzeichnis, ist wichtig: `/work` ist der Projektordner des
Nutzers, oft ein Git-Repository. Eine Datei, die Humanitl dort ablegt, landet in
einem Diff, vielleicht in einem Commit, und irgendwann in einem fremden
Repository. Das globale Verzeichnis liegt im tmpfs der Sandbox und verschwindet
mit ihr.

Die Notiz im `403` ist der kürzeste Weg von der Entscheidung des Menschen zum
Verhalten des Agenten. Sie nutzt einen Antwortweg, den der Agent ohnehin liest,
und braucht kein Protokoll. Header **und** Body, weil manche Werkzeuge nur eines
von beidem an das Modell weiterreichen.

`/ask` ist bewusst eine Sackgasse. Der Agent kann eine Bitte formulieren, sie
erscheint im UI als Karte mit einem Regelvorschlag, und dort endet seine
Wirkung. Ein Agent, der Regeln anlegen könnte, hätte die Kontrolle, um die es
hier geht; ein Agent, der gar nichts sagen kann, lässt den Menschen raten. Die
Karte ist der Kompromiss, der die Machtverhältnisse unverändert lässt.

Die Statuszeile im Terminal löst ein Wahrnehmungsproblem: Der Mensch sitzt vor
dem Terminal des Agenten und sieht dort Stillstand. Die Zeile sagt ihm, dass
nicht der Agent hängt, sondern eine Entscheidung aussteht. Sie geht über den
Ausgabestrom, nie über `stdin`, damit sie nicht mit der Eingabe des Agenten
verwechselt werden kann.

## Verworfene Alternativen

- **Ein zweiter Socket oder eine Steuerdatei für Agent-Kommunikation.** Der
  offensichtliche Weg, und der einzige, der die Garantie „genau eine Tür"
  bricht. Ausgeschlossen.
- **Umgebungsvariablen für den Status.** Statisch beim Start; Regeln und
  Zustand ändern sich während der Sitzung. Für ein Briefing zu wenig Platz.
- **Briefing ins Projektverzeichnis (`/work/AGENTS.md`).** Am einfachsten und in
  vielen Agenten der Standardort. Verworfen wegen der Verschmutzung des
  Nutzer-Repositories.
- **Briefing als System-Prompt über den LLM-Passthrough einschleusen.** Hätte
  bedeutet, dass Humanitl in die Prompts des Nutzers hineinschreibt. Ein
  massiver Eingriff, schwer zu debuggen, und beim Streaming kaum sauber
  umsetzbar.
- **`/ask` erzeugt automatisch eine Regel oder eine Freigabeanfrage mit
  Auto-Timeout.** Verworfen: Damit könnte der Agent den Entscheidungsfluss
  steuern, statt ihn nur zu informieren.
- **Kein Feedback beim Blocken.** Der Ist-Zustand ohne diesen ADR. Der Agent
  probiert weiter, der Mensch blockt weiter, und niemand lernt etwas.
- **Statuszeile über `stdin` in den Agenten schreiben.** Wäre für zeilenbasierte
  Agenten bequem und ist eine Injektion in den Eingabestrom eines Programms, dem
  wir nicht trauen. Ausgeschlossen.

## Konsequenzen

- `humanitl.internal` ist ein reservierter Hostname. Er wird nie aufgelöst und
  nie an einen Upstream weitergereicht; die Regelauswertung sieht ihn als
  Sonderfall.
- Der Meta-Endpunkt ist Teil der Angriffsfläche des Proxys und wird entsprechend
  behandelt: nur `GET` und `POST`, feste Pfade, `/ask` mit 2 KB Obergrenze,
  Tests für alle drei Pfade und für die Ablehnung anderer Methoden.
- Die Notiz erweitert `Decision::Block { reason, note }` und damit den
  Kerntyp, die Proto (`DecideRequest.block.note`), das Audit-Log und die
  History-Ansicht.
- Header-Injection über die Notiz ist ein Testfall, kein Restrisiko: Länge
  begrenzt, `CR`/`LF` zu Leerzeichen, Steuer- und unsichtbare Zeichen entfernt,
  Header-Wert auf sichtbares ASCII plus `SP`/`HTAB` beschränkt.
- Der `AgentAdapter` bekommt `files(&self, ctx) -> Vec<(PathBuf, Vec<u8>)>` für
  Dateien, die in der Sandbox angelegt werden. Das Briefing ist die erste
  Anwendung davon; `opencode.json` ist die zweite.
- Die Briefing-Vorlage liegt unter `agents/opencode/briefing.{en,de}.md` und ist
  damit lesbar, anpassbar und übersetzbar, statt im Code zu stehen.
- Ein Test prüft, dass nach dem Start die Datei in der Sandbox existiert **und**
  das Projektverzeichnis unverändert ist.

## Betroffene Issues

`HUM-071` (Agent-Briefing), `HUM-072` (Block mit Notiz), `HUM-073`
(Meta-Endpunkt `humanitl.internal`), `HUM-037` (AgentAdapter und
OpenCode-Profil), `HUM-042` (Terminal mit Statuszeile).
