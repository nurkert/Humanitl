-- Die Zusammenfassung eines Sandbox-Laufs (HUM-043).
--
-- Sie gehört zum Lauf, nicht zur Sitzung des Daemons: `humanitld` hat genau
-- eine Sitzung je Prozess (`start_session` beim Start, `end_session` beim
-- Herunterfahren), und die Sandbox startet und stoppt darin beliebig oft. Ein
-- Primärschlüssel allein auf `session_id` ließe deshalb nur eine
-- Zusammenfassung je Daemon-Prozess zu. Die Kennung des Laufs ist die
-- `SandboxId`, die `SandboxHandle` ohnehin trägt; ein eigener Id-Typ dafür wäre
-- ein zweiter Name für dieselbe Sache.
--
-- Der Primärschlüssel ist `sandbox_id` allein, nicht das Paar mit der Sitzung.
-- Eine `SandboxId` ist eine UUIDv7 und damit für sich eindeutig, und der Weg
-- des Nutzers führt über sie: `humanitl sessions summary <id>` kennt genau
-- diese eine Kennung. Ein zusammengesetzter Schlüssel zwänge die
-- Kommandozeile, zusätzlich nach der Sitzung zu fragen, die niemand zur Hand
-- hat. `session_id` bleibt als Spalte mit Fremdschlüssel und eigenem Index:
-- Danach wird gruppiert, nicht gesucht.
--
-- `json` hält die Zusammenfassung als Ganzes: Pfade, Änderungen, Funde,
-- Symlinks. Die Struktur gehört `humanitl-sandbox`
-- (`humanitl_sandbox::summary::SessionSummary`), und diese Crate darf sie nicht
-- kennen (`tools/deps-allow.toml`: nur `humanitl-core`); sie speichert deshalb
-- den Text, so wie sie eine Regel als YAML speichert (V2). Als BLOB und nicht
-- als TEXT, damit `SQLite` den Inhalt nie umkodiert.
--
-- Ein Fremdschlüssel auf `sessions(id)`: Eine Zusammenfassung ohne ihre Sitzung
-- wäre eine Zeile, die die History nicht anzeigen kann.
CREATE TABLE session_summaries (
  sandbox_id TEXT    PRIMARY KEY,
  session_id TEXT    NOT NULL REFERENCES sessions(id),
  created    INTEGER NOT NULL,           -- Unix-Millisekunden UTC
  json       BLOB    NOT NULL
);

-- Die History zeigt die Läufe einer Sitzung, die jüngsten zuerst.
CREATE INDEX session_summaries_session ON session_summaries(session_id, created DESC);
