---
source: adr/0014-agent-awareness.md
source-sha256: b90dd2ec45aad1417a68a9edd18425eace80f9f88389ed8460b1ebd1ba502dc8
lang: de
---
# ADR-0014 · Der Agent weiß Bescheid und bekommt Rückmeldung über den einen vorhandenen Kanal

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-09-02

## Kontext

Ein LLM-Agent in der Sandbox von Humanitl erlebt eine Umgebung, auf die er nicht trainiert
wurde. Sein `webfetch` hängt fünf Minuten und liefert dann `403`. Seine Update-Prüfung
scheitert. Ein `curl` läuft in eine Zeitüberschreitung. Ohne Erklärung reagiert er, wie Agenten
auf Fehler reagieren: Er versucht es erneut, variiert die URL, probiert einen anderen Weg und
verbrennt dabei Zeit und Tokens. Der Mensch sieht eine Warteschlange voller Wiederholungen
derselben blockierten Anfrage.

Umgekehrt hat der Mensch beim Blockieren oft einen Grund, den der Agent nutzen könnte:
„nicht GitHub, nimm PyPI", „nicht diese Domain, das ist Telemetrie". Heute geht dieser Grund
verloren.

Die Versuchung ist ein zweiter Kanal: ein Steuer-Socket, eine Datei, die der Agent liest, ein
Sidecar. Jeder davon ist eine neue Tür in die Sandbox und damit ein Riss in der Garantie
„genau eine Tür" (ADR-0002).

## Entscheidung

Alles läuft über den einen vorhandenen Kanal, den Proxy-Socket. Kein neuer Kanal, keine neue
Fähigkeit für den Agenten, keine neue Lücke in der Garantie.

**1. Briefing.** Beim Start legt der `AgentAdapter` eine globale Anweisungsdatei **in der
Sandbox** ab, für OpenCode `~/.config/opencode/AGENTS.md`, für Claude Code
`~/.claude/CLAUDE.md`, und **nie im Projektverzeichnis**. Ihr Inhalt ist eine mitgelieferte
Vorlage von etwa 150 Tokens in der Sprache des Nutzers (`ui.language`). Kernaussagen: kein
direkter Internetzugang; jede HTTP-Anfrage geht über einen Proxy, ein Mensch entscheidet;
Warten ist normal und kein Grund zum Abbrechen; eine Antwort `403` mit `Blocked by Humanitl`
ist endgültig und wird nicht wiederholt, stattdessen den Nutzer informieren und eine
Alternative vorschlagen; Statusabfrage unter `http://humanitl.internal/`.

**2. Rückmeldung beim Blockieren.** Die Aktionsleiste hat ein optionales, aufklappbares
Notizfeld. Der Text landet im `403`-Body unter `note:` und im Header `X-Humanitl-Note`. Der
Agent sieht ihn im Ergebnis seines Tool-Aufrufs und kann darauf reagieren. Die Notiz wird vor
beidem bereinigt (`sanitize_note` in `humanitl-core`, `backlog/CONVENTIONS.md` 4.11): `CR`
und `LF` werden zu Leerzeichen, andere Steuerzeichen fallen weg, ebenso unsichtbare Zeichen
(Nullbreitenzeichen, Bidi-Overrides, BOM, weiches Trennzeichen, Tag-Zeichen), die im Terminal
des Agenten einen anderen Text vortäuschen könnten als den, den der Nutzer geschrieben hat.
Höchstens 500 Zeichen bleiben. Der Header-Wert ist zusätzlich auf sichtbare ASCII-Zeichen
plus `SP` und `HTAB` beschränkt (RFC 9110 §5.5); Nicht-ASCII steht nur im Body (keine
Header-Injection).

**3. Meta-Endpunkt.** Der Proxy bedient den virtuellen Host `humanitl.internal` selbst, ohne
Upstream und ohne DNS, mit kleinen `text/plain`-Antworten:

| Pfad | Methode | Antwort |
|---|---|---|
| `/` | GET | Kurzstatus: Session, Nachfrage-Modus, Timeout, gültige Regeln (eine Zeile je Regel) |
| `/why/<flow-id>` | GET | Grund der Entscheidung samt Notiz |
| `/ask` | POST | Freitext bis 2 KB; erzeugt ein `AgentAsk`-Ereignis und in der Oberfläche eine Karte „Der Agent bittet um …" mit einem Regelvorschlag |

Andere Methoden werden abgewiesen. `/ask` erzeugt **nur** eine Karte in der Oberfläche,
**nie** eine Regel.

**4. Sichtbarkeit im Terminal.** Wenn eine Anfrage angehalten wird, schreibt der Daemon eine
Statuszeile in das Terminal des Agenten, über den PTY-Stream, nicht in `stdin`. Der Mensch,
der auf das Terminal schaut, sieht dadurch, was gerade aussteht.

**Was das ausdrücklich nicht ist:** Der Agent bekommt keine Möglichkeit, Regeln anzulegen,
Entscheidungen zu beeinflussen oder den Nutzer zu umgehen. `/ask` ist eine Bitte, keine
Handlung.

## Begründung

Der Proxy-Socket existiert schon, ist schon die einzige Tür, wird schon vollständig
aufgezeichnet und unterliegt schon der Regelauswertung. Ihn auch für Statusinformationen und
Rückmeldung zu nutzen, kostet keine zusätzliche Angriffsfläche: Eine Anfrage an
`humanitl.internal` ist eine Anfrage wie jede andere, nur dass der Proxy sie selbst
beantwortet.

Das Briefing kostet etwa 150 Tokens und spart ein Vielfaches davon. Ein Agent, der weiß, dass
`403 Blocked by Humanitl` endgültig ist, versucht es nicht erneut. Ein Agent, der weiß, dass
Warten normal ist, bricht nicht nach dreißig Sekunden ab. Das ist die billigste Verbesserung im
ganzen Entwurf.

Dass das Briefing in ein **globales** Konfigurationsverzeichnis der Sandbox kommt und nicht ins
Projektverzeichnis, ist wichtig: `/work` ist der Projektordner des Nutzers, oft ein
Git-Repository. Eine Datei, die Humanitl dort ablegt, landet in einem Diff, vielleicht in einem
Commit und irgendwann im Repository eines anderen. Das globale Verzeichnis liegt im tmpfs der
Sandbox und verschwindet mit ihr.

Die Notiz im `403` ist der kürzeste Weg von der Entscheidung des Menschen zum Verhalten des
Agenten. Sie nutzt einen Antwortweg, den der Agent ohnehin liest, und braucht kein Protokoll.
Header **und** Body, weil manche Werkzeuge nur eins von beiden an das Modell weitergeben.

`/ask` ist mit Absicht eine Sackgasse. Der Agent kann eine Bitte formulieren, sie erscheint in
der Oberfläche als Karte mit einem Regelvorschlag, und dort endet ihre Wirkung. Ein Agent, der
Regeln anlegen könnte, hätte genau die Kontrolle, um die es hier geht; ein Agent, der gar
nichts sagen kann, lässt den Menschen raten. Die Karte ist der Kompromiss, der das
Machtgefüge unverändert lässt.

Die Statuszeile im Terminal löst ein Wahrnehmungsproblem: Der Mensch sitzt vor dem Terminal
des Agenten und sieht dort Stillstand. Die Zeile sagt ihm, dass nicht der Agent hängt, sondern
eine Entscheidung aussteht. Sie geht über den Ausgabestrom, nie über `stdin`, damit sie nicht
für eine Eingabe des Agenten gehalten werden kann.

## Verworfene Alternativen

- **Ein zweiter Socket oder eine Steuerdatei für die Kommunikation mit dem Agenten.** Der
  naheliegende Weg und der einzige, der die Garantie „genau eine Tür" bricht. Ausgeschlossen.
- **Umgebungsvariablen für den Status.** Beim Start statisch; Regeln und Zustand ändern sich
  während der Session. Zu wenig Platz für ein Briefing.
- **Briefing im Projektverzeichnis (`/work/AGENTS.md`).** Am einfachsten und bei vielen
  Agenten der Standardort. Verworfen, weil es das Repository des Nutzers verschmutzt.
- **Das Briefing als System-Prompt über die LLM-Durchleitung einschleusen.** Hätte bedeutet,
  dass Humanitl in die Prompts des Nutzers schreibt. Ein massiver Eingriff, schwer zu debuggen
  und mit Streaming kaum sauber umsetzbar.
- **`/ask` legt automatisch eine Regel oder eine Freigabeanfrage mit automatischem Timeout
  an.** Verworfen: Damit könnte der Agent den Entscheidungsablauf steuern, statt ihn nur zu
  informieren.
- **Keine Rückmeldung beim Blockieren.** Der heutige Zustand ohne diese ADR. Der Agent
  versucht es weiter, der Mensch blockiert weiter, und niemand lernt etwas.
- **Die Statuszeile über `stdin` in den Agenten schreiben.** Wäre für zeilenbasierte Agenten
  bequem und ist eine Injection in den Eingabestrom eines Programms, dem wir nicht vertrauen.
  Ausgeschlossen.

## Konsequenzen

- `humanitl.internal` ist ein reservierter Hostname. Er wird nie aufgelöst und nie an einen
  Upstream weitergegeben; die Regelauswertung behandelt ihn als Sonderfall.
- Der Meta-Endpunkt gehört zur Angriffsfläche des Proxys und wird entsprechend behandelt: nur
  `GET` und `POST`, feste Pfade, `/ask` mit einer Obergrenze von 2 KB, Tests für alle drei
  Pfade und für das Abweisen anderer Methoden.
- Die Notiz erweitert `Decision::Block { reason, note }` und damit den Kerntyp, das Proto
  (`DecideRequest.block.note`), das Audit-Log und die Verlaufsansicht.
- Header-Injection über die Notiz ist ein Testfall, kein Restrisiko: Länge begrenzt,
  `CR`/`LF` zu Leerzeichen, Steuer- und unsichtbare Zeichen entfernt, Header-Wert auf
  sichtbares ASCII plus `SP`/`HTAB` beschränkt.
- Der `AgentAdapter` bekommt `files(&self, ctx) -> Vec<(PathBuf, Vec<u8>)>` für Dateien, die
  in der Sandbox angelegt werden. Das Briefing ist die erste Nutzung davon, `opencode.json`
  die zweite.
- Die Briefing-Vorlage liegt unter `agents/opencode/briefing.{en,de}.md` und ist damit lesbar,
  anpassbar und übersetzbar, statt im Code zu stehen.
- Ein Test prüft, dass nach dem Start die Datei in der Sandbox existiert **und** das
  Projektverzeichnis unverändert ist.

## Betroffene Issues

`HUM-071` (Agenten-Briefing), `HUM-072` (Blockieren mit Notiz), `HUM-073` (Meta-Endpunkt
`humanitl.internal`), `HUM-037` (AgentAdapter und OpenCode-Profil), `HUM-042` (Terminal mit
Statuszeile).
