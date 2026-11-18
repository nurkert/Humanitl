-- Das Projektregister (HUM-532, ADR-0024 Punkt 1).
--
-- Ein Projekt hat eine stabile Kennung `id`; Projektwahl, Aufräumen und
-- Clients beziehen sich auf sie, nicht auf den Pfad. Der Pfad ist kanonisch
-- (`realpath`), `dev` und `ino` stammen vom ersten Start und erkennen den
-- Ordner wieder. Liegt am selben Pfad später ein anderer Ordner, bekommt die
-- alte Zeile `replaced_at` und bleibt als Verlauf, und eine neue Zeile mit
-- neuer `id` wird die aktuelle. Genau eine aktuelle Zeile je Pfad hält der
-- Teilindex `projects_current_path`; mehrere Zeilen mit demselben Pfad sind
-- erlaubt, solange höchstens eine davon `replaced_at IS NULL` hat.
--
-- `choice` ist die Projektwahl als JSON-Objekt aus Schlüssel und Text. Die
-- Struktur gehört `humanitl-config` (`ProjectChoice`); diese Crate speichert
-- den Text, wie sie eine Zusammenfassung speichert (V5). `version` steigt bei
-- jeder Änderung der Zeile; der Digest des Aufräumens bindet sie (HUM-533).
--
-- Zeiten in Unix-Millisekunden UTC wie überall in dieser Datenbank.
CREATE TABLE projects (
  id              TEXT    PRIMARY KEY,
  path            TEXT    NOT NULL,
  dev             INTEGER,
  ino             INTEGER,
  display_name    TEXT    NOT NULL,
  first_seen      INTEGER NOT NULL,
  last_started    INTEGER,
  last_agent      TEXT,
  runs            INTEGER NOT NULL DEFAULT 0,
  choice          TEXT,
  choice_saved_at INTEGER,
  replaced_at     INTEGER,
  version         INTEGER NOT NULL DEFAULT 1
);

CREATE UNIQUE INDEX projects_current_path ON projects(path) WHERE replaced_at IS NULL;

-- `Projects.list` liest die aktuellen Einträge, der jüngste Start zuerst.
CREATE INDEX projects_recent ON projects(COALESCE(last_started, first_seen) DESC, id DESC)
  WHERE replaced_at IS NULL;

-- Jede Sitzung gehört zu einer Projekt-`id`; alles, was je Projekt auswählt,
-- wählt über diese Spalte, nie über `work_dir`.
ALTER TABLE sessions ADD COLUMN project_id TEXT REFERENCES projects(id);
CREATE INDEX sessions_project ON sessions(project_id);

-- Nachfüllen aus den Sitzungen von gestern: je Pfad ein Projekt mit erstem
-- und letztem Start, dem Agenten des letzten und der Zahl der Sitzungen.
-- `dev` und `ino` sind unbekannt und kommen mit dem nächsten Start. Ein leerer
-- oder relativer Pfad nennt kein Projekt und bleibt außen vor. Die Kennung ist
-- eine zufällige UUID (Version 4); SQLite kennt keine zeitgeordnete.
--
-- Nur Sitzungen, in denen eine Sandbox lief, nennen ein Projekt: Bis HUM-532
-- schrieb der Daemon beim eigenen Start sein Arbeitsverzeichnis nach
-- `work_dir`, auch wenn nie eine Sandbox startete. Belegt ist ein Lauf durch
-- eine Zusammenfassung (`session_summaries`, je Sandbox-Lauf, HUM-043) oder
-- durch Verkehr des Agenten (`flows`, nur aus einer Sandbox über den Proxy).
-- Eine Sitzung ohne beides bleibt ohne Projekt.
INSERT INTO projects (id, path, display_name, first_seen, last_started, last_agent, runs)
SELECT
  lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4'
    || substr(lower(hex(randomblob(2))), 2) || '-'
    || substr('89ab', 1 + (abs(random()) % 4), 1) || substr(lower(hex(randomblob(2))), 2)
    || '-' || lower(hex(randomblob(6))),
  s.work_dir,
  replace(s.work_dir, rtrim(s.work_dir, replace(s.work_dir, '/', '')), ''),
  MIN(s.started_at),
  MAX(s.started_at),
  (SELECT agent FROM sessions latest WHERE latest.work_dir = s.work_dir
     AND (EXISTS (SELECT 1 FROM session_summaries m WHERE m.session_id = latest.id)
          OR EXISTS (SELECT 1 FROM flows f WHERE f.session_id = latest.id))
     ORDER BY latest.started_at DESC, latest.id DESC LIMIT 1),
  COUNT(*)
FROM sessions s
WHERE s.work_dir LIKE '/%'
  AND (EXISTS (SELECT 1 FROM session_summaries m WHERE m.session_id = s.id)
       OR EXISTS (SELECT 1 FROM flows f WHERE f.session_id = s.id))
GROUP BY s.work_dir;

-- Eine Sitzung gehört zu dem Projekt, das zu ihrem Start am Pfad aktuell war,
-- wenn das eindeutig ist: genau ein Eintrag je Pfad, und kein Ersatz seit
-- ihrem Start. Sonst bleibt `project_id` leer, und die Sitzung gilt als nicht
-- zugeordnet.
UPDATE sessions SET project_id = (
  SELECT p.id FROM projects p
  WHERE p.path = sessions.work_dir AND p.replaced_at IS NULL
)
WHERE project_id IS NULL
  AND (EXISTS (SELECT 1 FROM session_summaries m WHERE m.session_id = sessions.id)
       OR EXISTS (SELECT 1 FROM flows f WHERE f.session_id = sessions.id))
  AND (SELECT COUNT(*) FROM projects p WHERE p.path = sessions.work_dir) = 1
  AND NOT EXISTS (
    SELECT 1 FROM projects p
    WHERE p.path = sessions.work_dir
      AND p.replaced_at IS NOT NULL AND p.replaced_at > sessions.started_at
  );
