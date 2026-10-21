-- Die Befunde eines Flusses (HUM-256).
--
-- Seit HUM-057 hängt jeder Befund, den der Proxy einem Fluss zuordnet
-- (`LIMIT_001` mit dem Vorschlag einer Regel `stream: true`, `FINDINGS_002`,
-- `LIMIT_004`, `PROXY_008` ...), am Datensatz der Registry. Die lebt nur im
-- Prozess; nach einem Neustart des Daemons stand ein am Cap geblockter Fluss
-- ohne Grund und ohne den Weg hinaus in der Historie. Hier steht er dauerhaft.
--
-- Eine Zeile je Befund, `idx` in der Reihenfolge des Eintreffens, 0-basiert,
-- höchstens `humanitl_core::MAX_FLOW_DIAGNOSTICS` je Fluss wie in der
-- Registry. Befunde ohne Fluss (sitzungsweite) stehen nicht hier.
--
-- `code`, `severity` und `title` sind die Textformen aus dem Register und aus
-- `Severity::as_str`; ein Archiv liest auch einen Code, den eine spätere
-- Fassung nicht mehr kennt. `why` ist durch die Masken gegangen (HUM-140),
-- bevor es hier ankommt.
--
-- Der Vorschlag (`fix`) steht in drei Spalten: `fix_kind` ist
-- `FixAction::as_str` (`add_rule`, `change_setting`, `copy_command` ...),
-- `fix_key` und `fix_value` tragen die Werte der Variante. Eine Regel
-- (`add_rule`) steht in `fix_value` in der Form von `rules.yaml`, geschrieben
-- von `humanitl_rules::serialize_rules` und gelesen von
-- `parse_rules_for_session` (mit der Sitzung des Flusses), also
-- von derselben Serialisierung, die die Datei des Nutzers schreibt; ein
-- eigenes Format daneben liefe von ihr weg.
--
-- Ohne `ON DELETE CASCADE`, wie `messages` und `findings`: Der Aufräumlauf
-- löscht die Zeilen ausdrücklich vor ihrem Fluss, und der Fremdschlüssel
-- (`foreign_keys=ON`) lässt einen Fluss gar nicht erst fallen, solange ein
-- Befund an ihm hängt. Ein verwaister Befund kann so nicht entstehen. Der
-- Primärschlüssel trägt die Suche nach `flow_id`.
CREATE TABLE diagnostics (
  flow_id   TEXT NOT NULL REFERENCES flows(id),
  idx       INTEGER NOT NULL,
  code      TEXT NOT NULL,
  severity  TEXT NOT NULL,
  title     TEXT NOT NULL,
  why       TEXT NOT NULL,
  docs      TEXT,
  fix_kind  TEXT,
  fix_key   TEXT,
  fix_value TEXT,
  PRIMARY KEY (flow_id, idx)
);
