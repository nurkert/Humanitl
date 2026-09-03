-- Der Filter `host:github.com` trifft den Host selbst und jede Subdomain. Als
-- `host = ?1 OR host LIKE '%.' || ?1` geschrieben ist das ein Suffix-Vergleich,
-- und ein Suffix kann kein Index beantworten: SQLite läuft über die ganze
-- Tabelle, egal wie wenige Zeilen am Ende übrig bleiben. Bei langer History ist
-- das der teuerste Filter, den die Oberfläche anbieten kann.
--
-- `host_rev` hält denselben Host mit umgekehrter Label-Reihenfolge und einem
-- abschließenden Punkt: aus `api.github.com` wird `com.github.api.`, aus
-- `github.com` wird `com.github.`. Aus dem Suffix wird damit ein Präfix, und aus
-- dem Präfix ein Bereich: alles zwischen `com.github.` und `com.github/` (dem
-- nächsten Zeichen nach dem Punkt). `evil-github.com` liegt als
-- `com.evil-github.` davor, `github.com.evil.io` als `io.evil.com.github.`
-- weit dahinter; beide fallen aus dem Bereich, wie es die Regel-Semantik aus
-- ADR-007 verlangt.
ALTER TABLE flows ADD COLUMN host_rev TEXT NOT NULL DEFAULT '';
CREATE INDEX flows_host_rev ON flows(host_rev, ts DESC, id DESC);

-- `flows_ts` aus V1 sortiert `ts` absteigend, `id` aber aufsteigend, weil ein
-- fehlendes `DESC` in SQLite `ASC` bedeutet. Die Liste sortiert beide
-- absteigend, also passt die Reihenfolge des Index nicht zur Reihenfolge der
-- Abfrage, und SQLite legt für die zweite Spalte einen temporären B-Baum an
-- (`USE TEMP B-TREE FOR LAST TERM OF ORDER BY`). Mit `id DESC` entfällt der.
DROP INDEX flows_ts;
CREATE INDEX flows_ts ON flows(ts DESC, id DESC);

-- Die Liste kann nach Host, Dauer und Größe sortieren. Ohne Index bedeutet das
-- einen Lauf über die ganze Tabelle und eine vollständige Sortierung, auch für
-- die ersten zweihundert Zeilen. Die drei Indizes stehen aufsteigend; weil
-- `list_flows` alle drei Spalten in derselben Richtung sortiert, bedient ein
-- aufsteigender Index beide Richtungen, vorwärts wie rückwärts gelesen. Die
-- Ausdrücke sind wortgleich die aus `SortKey::expr`, sonst erkennt SQLite sie
-- nicht wieder.
CREATE INDEX flows_sort_host     ON flows(host, ts, id);
CREATE INDEX flows_sort_duration ON flows(COALESCE(duration_ms, -1), ts, id);
CREATE INDEX flows_sort_size     ON flows(request_size + COALESCE(response_size, 0), ts, id);
