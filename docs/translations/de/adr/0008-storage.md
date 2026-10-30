---
source: adr/0008-storage.md
source-sha256: cdbf31d083a195cec1ae237cca37f488d5cff889fe2709ba40ba398b4538bdd6
lang: de
---
# ADR-0008 · Speicherung: SQLite, Blob-Speicher, Audit-Kette, getrennte Pseudonym-Zuordnung

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-09-02

## Kontext

Humanitl zeichnet auf, was durch den Proxy geht: Metadaten jedes Flows, Header, Bodies von
Anfrage und Antwort, Funde, Entscheidungen, Regeln. Diese Daten sind zugleich das
Beweismittel für die DSGVO-Pflichten der Zielgruppe („was ging an wen?") und die
sensibelste Datensammlung auf dem Rechner: Sie enthält konstruktionsbedingt alles, was der
Agent je gesendet hat, auch Kundendaten.

Daraus folgen drei getrennte Anforderungen. Erstens eine abfragbare Datenbank für
Metadaten (Verlaufsbildschirm mit Filter, Sortierung und Cursor, serverseitig). Zweitens
ein Ablageort für Bodies, der die Datenbankdatei nicht mit einem Upload von 40 MB
aufbläht. Drittens ein Nachweis, dass die Aufzeichnung nachträglich nicht verändert wurde,
und damit eine ehrliche Aussage darüber, was dieser Nachweis wert ist.

Randbedingungen: ein Desktop-Programm ohne Server. Kein Datenbankdienst, den der Nutzer
installieren muss. Ein Nutzer, ein Prozess, gleichzeitiges Schreiben aus dem Proxy und
Lesen aus der Oberfläche.

## Entscheidung

**Metadaten in SQLite** im WAL-Modus, mit versionierten Migrationen (`refinery`,
eingebettet aus `daemon/crates/recorder/migrations/`), unter
`$XDG_DATA_HOME/humanitl/humanitl.db`. Ein eigener Schreib-Thread schreibt, Lesezugriffe
laufen über eine getrennte Nur-Lese-Verbindung; Filtern, Sortieren und Blättern per Cursor
im Verlauf werden serverseitig in SQL ausgeführt. Gespeichert werden Sessions, Flows,
Nachrichten (Header und Body je Richtung), Funde und ein Schnappschuss gelöschter Regeln,
damit der Verlauf eine Entscheidung auch dann noch erklären kann, wenn die Regel nicht
mehr in `rules.yaml` steht; `rules.yaml` bleibt die maßgebliche Quelle für Regeln.

Das verbindliche Schema steht nicht in dieser Datei, sondern in den Migrationen:
`V1__init.sql` und `V2__rules_snapshot.sql` in HUM-026 (`backlog/sprint-2.md`) sowie die
Pseudonym-Migration in HUM-048 (`backlog/sprint-4.md`). Die Kurzfassung in `BACKLOG.md`
3.4 ist eine frühe Skizze; wo sie von den Migrationen abweicht, gelten die Migrationen.

**Bodies** bis 256 KB (`recorder.inline_max_bytes`) liegen eingebettet als BLOB in
`messages`. Größere Bodies gehen inhaltsadressiert in den Blob-Speicher unter
`$XDG_DATA_HOME/humanitl/blobs/<hex[0..2]>/<sha256-hex>`; die Datenbank hält nur den
Verweis.

**Audit-Log** als reines Anhänge-JSONL unter `$XDG_DATA_HOME/humanitl/audit/audit.jsonl`,
mit einer Hash-Kette. Jeder Eintrag trägt `seq`, `ts`, `prev_hash` und `hash` über die
kanonische JSON-Form des Eintrags, dazu ein HMAC mit einem Installationsschlüssel aus dem
Schlüsselbund des Systems. Der Kopf-Hash wird regelmäßig verankert: Anzeige in der
Oberfläche, Ablage an einem zweiten Ort und zusätzlich über `logger` im systemd-Journal.

**Was die Kette beweist, wird ausgesprochen, nicht angedeutet.** Sie schützt vor
nachträglichem Bearbeiten durch Dritte mit Dateizugriff. Sie schützt **nicht** vor einem
Angreifer, der als derselbe Nutzer läuft und sowohl den Schlüssel als auch die Datei hat;
er kann die Kette neu schreiben. Der Schutz vor diesem Fall wäre ein externer Anker
(Zeitstempeldienst, fremder Rechner) und kommt ausdrücklich erst nach dem MVP.

**Pseudonymisierungs-Zuordnung getrennt.** Die Zuordnung vom Pseudonym zum ursprünglichen
Wert lebt in einer eigenen Tabelle, verschlüsselt mit einem Schlüssel aus dem
Schlüsselbund, ausschließlich auf der Seite des Rechners. Sie gelangt nie in die Sandbox
und nie in eine Anfrage. Secrets werden nur als Hash plus ein Präfix für die Anzeige
gespeichert, nie im Klartext. Ein Export ist verschlüsselt.

## Begründung

SQLite ist für diesen Fall die naheliegende und richtige Wahl: eingebettet, kein Dienst,
eine Datei, transaktional, und im WAL-Modus laufen der schreibende Recorder und die lesende
Oberfläche nebeneinander, ohne sich gegenseitig zu blockieren. Migrationen gibt es vom
ersten Tag an, weil ein Werkzeug, das Beweismittel speichert, bei einem Update seine alten
Daten nicht verlieren darf.

Die Grenze bei 256 KB trennt zwei Zugriffsmuster. Kleine Bodies (JSON-Nutzlasten,
LLM-Prompts) will man zusammen mit den Metadaten in einer Abfrage; große Bodies will man
beim Auflisten des Verlaufs weder in der Datenbankdatei noch im Speicher haben. Die
Inhaltsadressierung über SHA-256 dedupliziert nebenbei: Ein Agent, der dieselbe Datei
zehnmal hochlädt, belegt den Platz einmal. Der zweistufige Pfad (`<hex[0..2]>/<sha256>`)
hält die Verzeichnisse für jedes Dateisystem klein genug.

JSONL für das Audit-Log statt einer weiteren Tabelle, weil reines Anhängen genau die
Eigenschaft ist, die eine Hash-Kette braucht, und weil eine Textdatei ohne Humanitl lesbar
und mit Standardwerkzeugen prüfbar bleibt. Die kanonische JSON-Form ist die Voraussetzung
dafür, dass der Hash reproduzierbar ist. Das HMAC verhindert, dass jemand ohne den
Schlüssel eine plausible Kette neu berechnet; das Verankern des Kopf-Hashs an mehreren
Orten macht das Abschneiden des Endes (Truncation) auffällig.

Die Ehrlichkeit über die Grenzen der Kette ist Teil der Entscheidung, kein Zusatz. Ein
Sicherheitsversprechen, das mehr behauptet, als es hält, ist schlimmer als keines, weil
der Nutzer sein Verhalten danach ausrichtet. Grundsatz 3 („ehrlich über Grenzen") gilt hier
wörtlich.

Die Pseudonym-Zuordnung ist der einzige Datenbestand, mit dem sich aus pseudonymisierten
Aufzeichnungen die ursprünglichen Daten wiederherstellen lassen. Sie ist deshalb
verschlüsselt, getrennt gehalten und in der Sandbox nie sichtbar; sonst wäre die
Pseudonymisierung ein Ritual statt einer Maßnahme.

## Verworfene Alternativen

- **PostgreSQL.** Bräuchte einen Dienst, den der Nutzer installiert, konfiguriert und
  aktualisiert. Für eine Einzelplatzanwendung eine nicht zu rechtfertigende Hürde.
  Vorgemerkt als möglicher zweiter `FlowStore`-Adapter für einen Team-Modus.
- **Alles in Dateien (ein Verzeichnis je Flow).** Einfach zu schreiben, aber jeder Filter
  im Verlaufsbildschirm wäre ein Durchlauf über alle Flows. Serverseitiges Filtern,
  Sortieren und Blättern per Cursor ist eine Anforderung, keine Zugabe.
- **Bodies immer eingebettet in der Datenbank.** Eine Datenbankdatei, die mit Uploads von
  40 MB wächst, wird zum Problem für `VACUUM` und für Backups, und ein `SELECT *` in der
  Oberfläche zieht Hunderte Megabyte.
- **Bodies immer im Blob-Speicher.** Zwei IO-Vorgänge für jedes kleine JSON-Objekt und
  eine Menge winziger Dateien. Die Schwelle bei 256 KB nimmt von beiden Wegen den
  nützlichen Teil.
- **Audit-Log in derselben SQLite-Datenbank.** Bequem, aber eine Datenbank ist
  bearbeitbar; reines Anhängen ist für sie eine Konvention, keine Eigenschaft. Und ein
  Prüfer soll die Kette ohne Humanitl nachrechnen können.
- **Signatur statt HMAC (asymmetrisch).** Wäre stärker, wenn der private Schlüssel
  woanders läge, und genau das ist im MVP nicht der Fall. Solange der Schlüssel auf
  demselben Rechner liegt, kauft die Asymmetrie nichts außer Komplexität. Mit einem
  externen Anker ändert sich das.
- **Blockchain-Anker oder externer Zeitstempeldienst im MVP.** Der einzige echte Schutz
  vor dem Angreifer mit Schlüssel und Datei, setzt aber eine Netzwerkverbindung und einen
  Dienst voraus. Nach dem MVP, und bis dahin klar als das benannt, was fehlt.
- **Pseudonym-Zuordnung unverschlüsselt neben den Flows.** Hätte die Pseudonymisierung
  entwertet: Wer Zugriff auf die Aufzeichnung hat, hätte auch die Auflösung.

## Folgen

- Der Port `FlowStore` (Adapter: SQLite) und der Port `AuditSink` (Adapter:
  JSONL-Hash-Kette) sind getrennt. Ein späterer entfernter Anker ist ein zweiter
  `AuditSink`, kein Umbau (ADR-0015).
- Migrationen sind ab HUM-026 Pflicht, auch für die erste Fassung des Schemas.
- `recorder.retention_days` steuert das Löschen; das Löschen wird dokumentiert, weil es
  eine Lücke in der Kette erzeugt, von der die Prüfung wissen muss.
- Der Blob-Speicher braucht eine Aufräumroutine, die verwaiste Blobs entfernt, sobald die
  zugehörigen Flows gelöscht sind.
- `humanitl audit verify` prüft die Kette und steht auch ohne die Oberfläche zur
  Verfügung; die Manipulationstests aus `BACKLOG.md` 4.5 (einen Eintrag löschen, das Ende
  abschneiden) gehören zur Abnahme. Der Fall des abgeschnittenen Endes bleibt rot, bis es
  die Verankerung des Kopfs gibt; auch das ist als solches dokumentiert.
- M1 zeichnet noch nicht dauerhaft auf. Die Aussage „alles wird aufgezeichnet" gilt erst
  ab HUM-026 (`backlog/CONVENTIONS.md` 4.10) und wird bis dahin nicht behauptet.

## Betroffene Issues

`HUM-026` (Recorder: SQLite, WAL, Migrationen, Blob-Speicher, `ListFlows`), `HUM-050`
(Audit-Hash-Kette und `verify`), `HUM-051` (Audit-Bildschirm, Export, Aufbewahrung),
`HUM-048` (Pseudonym-Zuordnung, verschlüsselt, Schlüsselbund), `HUM-032`
(Verlaufsbildschirm und Export).
