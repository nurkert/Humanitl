# ADR-0008 · Speicherung: SQLite, Blob-Store, Audit-Kette, getrenntes Pseudonym-Mapping
Status: Accepted
Datum: 2026-09-02

## Kontext

Humanitl zeichnet auf, was durch den Proxy geht: Metadaten jedes Flows, Header,
Bodies von Request und Response, Findings, Entscheidungen, Regeln. Diese Daten
sind gleichzeitig das Nachweismittel für die DSGVO-Pflichten der Zielgruppe
(„was ist an wen gegangen?") und die sensibelste Datensammlung auf dem Rechner:
Sie enthält per Konstruktion alles, was der Agent je verschickt hat, inklusive
Kundendaten.

Daraus folgen drei getrennte Anforderungen. Erstens eine abfragbare Datenbank
für Metadaten (History-Screen mit Filter, Sortierung und Cursor, serverseitig).
Zweitens ein Ablageort für Bodies, der bei einem 40-MB-Upload nicht die
Datenbankdatei aufbläht. Drittens ein Nachweis darüber, dass die Aufzeichnung
nachträglich nicht verändert wurde — und dabei eine ehrliche Aussage darüber,
was dieser Nachweis wert ist.

Randbedingungen: Ein Desktop-Programm ohne Server. Kein Datenbankdienst, den der
Nutzer installieren muss. Ein Nutzer, ein Prozess, gleichzeitige Schreibvorgänge
aus dem Proxy und Lesevorgänge aus dem UI.

## Entscheidung

**Metadaten in SQLite** im WAL-Modus, mit versionierten Migrationen
(`refinery`, eingebettet aus `daemon/crates/recorder/migrations/`), unter
`$XDG_DATA_HOME/humanitl/humanitl.db`. Ein eigener Writer-Thread schreibt,
Lesezugriffe laufen auf einer getrennten Nur-Lese-Verbindung; Filter,
Sortierung und Cursor-Paging der History werden serverseitig in SQL
ausgeführt. Gespeichert werden Sitzungen, Flows, Messages (Header und Body je
Richtung), Findings und ein Schnappschuss gelöschter Regeln, damit die History
eine Entscheidung auch dann noch erklären kann, wenn die Regel in `rules.yaml`
nicht mehr steht — `rules.yaml` bleibt die Quelle der Wahrheit für Regeln.

Das verbindliche Schema steht nicht in dieser Datei, sondern in den
Migrationen: `V1__init.sql` und `V2__rules_snapshot.sql` in HUM-026
(`backlog/sprint-2.md`) sowie die Pseudonym-Migration in HUM-048
(`backlog/sprint-4.md`). Die Kurzfassung in `BACKLOG.md` 3.4 ist eine frühe
Skizze; wo sie von den Migrationen abweicht, gelten die Migrationen.

**Bodies** bis 256 KB (`recorder.inline_max_bytes`) liegen inline als BLOB in
`messages`. Größere Bodies gehen content-addressed in den Blob-Store unter
`$XDG_DATA_HOME/humanitl/blobs/<hex[0..2]>/<sha256-hex>`; in der Datenbank steht
nur die Referenz.

**Audit-Log** als append-only JSONL unter
`$XDG_DATA_HOME/humanitl/audit/audit.jsonl`, mit einer Hash-Kette. Jeder Eintrag
trägt `seq`, `ts`, `prev_hash` und `hash` über die kanonische JSON-Form des
Eintrags, dazu einen HMAC mit einem Installationsschlüssel aus dem System-Keyring.
Der Head-Hash wird periodisch verankert: Anzeige im UI, Ablage an einem zweiten
Speicherort und zusätzlich per `logger` ins systemd-Journal.

**Was die Kette beweist, wird ausgeschrieben, nicht angedeutet.** Sie schützt
gegen nachträgliches Editieren durch Dritte mit Dateizugriff. Sie schützt
**nicht** gegen einen Angreifer, der als derselbe Nutzer läuft und sowohl den
Schlüssel als auch die Datei hat — der kann die Kette neu schreiben. Der Schutz
gegen diesen Fall wäre ein externer Anker (Zeitstempeldienst, fremder Rechner)
und ist ausdrücklich Post-MVP.

**Pseudonymisierungs-Mapping getrennt.** Die Zuordnung von Pseudonym zu
Originalwert liegt in einer eigenen Tabelle, verschlüsselt mit einem
Keyring-Schlüssel, ausschließlich host-seitig. Sie geht nie in die Sandbox und
nie in eine Anfrage. Secrets werden nur als Hash plus Anzeigepräfix gespeichert,
nie im Klartext. Ein Export ist verschlüsselt.

## Begründung

SQLite ist für diesen Fall die naheliegende und richtige Wahl: eingebettet, kein
Dienst, eine Datei, transaktional, und im WAL-Modus laufen der schreibende
Recorder und das lesende UI nebeneinander, ohne sich zu blockieren. Migrationen
sind ab dem ersten Tag da, weil ein Werkzeug, das Nachweise speichert, seine
alten Daten beim Update nicht verlieren darf.

Die Grenze bei 256 KB trennt zwei Zugriffsmuster. Kleine Bodies (JSON-Payloads,
LLM-Prompts) will man zusammen mit den Metadaten in einer Abfrage haben; große
Bodies will man weder in der Datenbankdatei noch beim Auflisten der History im
Speicher. Content-Addressing über SHA-256 dedupliziert nebenbei: Ein Agent, der
dieselbe Datei zehnmal hochlädt, belegt den Platz einmal. Der zweistufige Pfad
(`<hex[0..2]>/<sha256>`) hält die Verzeichnisse klein genug für jedes
Dateisystem.

JSONL für das Audit-Log statt einer weiteren Tabelle, weil append-only genau die
Eigenschaft ist, die eine Hash-Kette braucht, und weil eine Textdatei ohne
Humanitl lesbar und mit Standardwerkzeugen prüfbar bleibt. Die kanonische
JSON-Form ist Bedingung dafür, dass der Hash reproduzierbar ist. Der HMAC hindert
jemanden ohne Schlüssel daran, eine plausible Kette neu zu berechnen; die
Verankerung des Head-Hash an mehreren Orten macht ein Kürzen des Endes
(Truncation) auffällig.

Die Ehrlichkeit über die Grenzen der Kette ist Teil der Entscheidung, nicht ein
Zusatz. Ein Sicherheitsversprechen, das mehr behauptet als es hält, ist
schlimmer als keines, weil der Nutzer sein Verhalten danach richtet. Prinzip 3
(„ehrlich über Grenzen") gilt hier wörtlich.

Das Pseudonym-Mapping ist der einzige Datenbestand, der die Wiederherstellung
der Originaldaten aus pseudonymisierten Aufzeichnungen erlaubt. Er wird deshalb
verschlüsselt, getrennt gehalten und niemals in der Sandbox sichtbar — sonst
wäre die Pseudonymisierung ein Ritual statt einer Maßnahme.

## Verworfene Alternativen

- **PostgreSQL.** Bräuchte einen Dienst, den der Nutzer installiert, konfiguriert
  und aktualisiert. Für eine Einzelplatzanwendung eine unbegründete Hürde.
  Vorgemerkt als möglicher zweiter `FlowStore`-Adapter für einen Team-Modus.
- **Alles in Dateien (ein Verzeichnis pro Flow).** Einfach zu schreiben, aber
  jede Filterung im History-Screen wäre ein Scan über alle Flows. Serverseitiges
  Filtern, Sortieren und Cursor-Paging ist eine Anforderung, kein Extra.
- **Bodies immer inline in der Datenbank.** Eine Datenbankdatei, die mit
  40-MB-Uploads wächst, wird beim `VACUUM` und beim Backup zum Problem, und ein
  `SELECT *` im UI zieht Hunderte Megabyte.
- **Bodies immer im Blob-Store.** Zwei IO-Operationen für jedes kleine
  JSON-Objekt und eine Menge winziger Dateien. Die Schwelle bei 256 KB nimmt von
  beiden Wegen den nützlichen Teil.
- **Audit-Log in derselben SQLite-Datenbank.** Bequem, aber eine Datenbank ist
  editierbar; append-only ist bei ihr eine Konvention, keine Eigenschaft. Und ein
  Prüfer soll die Kette ohne Humanitl nachrechnen können.
- **Signatur statt HMAC (asymmetrisch).** Wäre stärker, wenn der private
  Schlüssel woanders läge — genau das ist im MVP nicht der Fall. Solange der
  Schlüssel auf derselben Maschine liegt, kauft die Asymmetrie nichts, außer
  Komplexität. Bei einem externen Anker ändert sich das.
- **Blockchain-Anker oder externer Zeitstempeldienst im MVP.** Der einzige echte
  Schutz gegen den Angreifer mit Schlüssel und Datei, aber er setzt eine
  Netzwerkverbindung und einen Dienst voraus. Post-MVP, und bis dahin klar
  benannt als das, was fehlt.
- **Pseudonym-Mapping unverschlüsselt neben den Flows.** Hätte die
  Pseudonymisierung entwertet: Wer Zugriff auf die Aufzeichnung hat, hätte auch
  die Auflösung.

## Konsequenzen

- Der Port `FlowStore` (Adapter: SQLite) und der Port `AuditSink` (Adapter:
  JSONL-Hash-Kette) sind getrennt. Ein späterer Remote-Anker ist ein zweiter
  `AuditSink`, kein Umbau (ADR-0015).
- Migrationen sind ab HUM-026 Pflicht, auch für die erste Version des Schemas.
- `recorder.retention_days` steuert die Löschung; die Löschung wird dokumentiert,
  weil sie eine Lücke in der Kette erzeugt, die die Prüfung kennen muss.
- Der Blob-Store braucht eine Aufräumroutine, die verwaiste Blobs entfernt,
  sobald die zugehörigen Flows gelöscht sind.
- `humanitl audit verify` prüft die Kette und ist auch ohne UI verfügbar; die
  Tamper-Tests aus `BACKLOG.md` 4.5 (Eintrag löschen, Ende kürzen) sind
  Bestandteil der Abnahme. Der Truncation-Fall bleibt rot, bis das
  Head-Anchoring existiert — auch das wird so dokumentiert.
- M1 zeichnet noch nicht dauerhaft auf. Die Aussage „alles wird aufgezeichnet"
  gilt erst ab HUM-026 (`backlog/CONVENTIONS.md` 4.10) und wird bis dahin nicht
  behauptet.

## Betroffene Issues

`HUM-026` (Recorder: SQLite, WAL, Migrationen, Blob-Store, `ListFlows`),
`HUM-050` (Audit-Hash-Kette und `verify`), `HUM-051` (Audit-Screen, Export,
Retention), `HUM-048` (Pseudonym-Mapping, verschlüsselt, Keyring),
`HUM-032` (History-Screen und Export).
