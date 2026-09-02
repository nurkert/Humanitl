# CLAUDE.md — Arbeitsregeln für dieses Repository

Diese Datei gilt für jede KI-Session in diesem Repository. Sie ergänzt
`CONTRIBUTING.md` (Toolchain, Commit-Format, Definition of Done) und
`backlog/CONVENTIONS.md` (kanonische Namen). Bei Widerspruch gewinnt diese Datei
für den Arbeitsablauf, `CONVENTIONS.md` für Namen und Typen.

## Woran wir arbeiten

Der Plan steht in `BACKLOG.md`, die Issue-Spezifikationen in
`backlog/sprint-N.md`, das Architektur-Leitbild in `docs/ARCHITECTURE.md`.
Bearbeitungsreihenfolge ist die Tabellenreihenfolge im Sprint-File, nicht die
Issue-Nummer. Vor dem ersten Issue eines Sprints: `BACKLOG.md` Abschnitte 2 bis
6 und `backlog/CONVENTIONS.md` lesen, dort gilt Abschnitt 4 vor Abschnitt 3.

## Ablauf pro Issue

1. Spezifikation des Issues vollständig lesen, dann den Code, der schon da ist.
2. Umsetzen inklusive der im Issue genannten Tests und der Fallstricke.
3. `make check` grün. Lokal fehlen rustfmt und clippy; CI erzwingt sie mit
   `STRICT=1`, also Code so schreiben, als liefen sie.
4. **Vier-Augen-Prinzip vor jedem Commit.** Den Diff des Issues von beiden
   externen Reviewern read-only prüfen lassen, parallel:
   - Antigravity über den Skill `antigravity:review` (Diff der Arbeitskopie)
   - Codex über den Skill `codex:rescue` mit einem Review-Auftrag auf den Diff
   Jeden Befund einzeln bewerten: zutreffend, teilweise, nicht zutreffend, mit
   einem Satz Begründung. Zutreffende Befunde vor dem Commit beheben, dann
   `make check` erneut. Nicht übernommene Befunde kurz im Commit-Body nennen,
   damit die Entscheidung nachvollziehbar bleibt.
5. Commit auf eigenem Branch `hum-xxx-kurztitel`, Merge mit `--no-ff` nach
   `main`, Push. `tools/commit-issue.sh` macht Branch, Commit und Merge in
   einem Schritt. Ein Issue, ein Commit. Nie mehrere Issues in einem Commit.
6. Nach dem Push: kurze Zusammenfassung an den Nutzer, was gebaut wurde, was
   die Reviewer fanden, was davon übernommen wurde.

## Regeln für Subagenten

- Subagenten arbeiten nur an den Dateien ihres Issues und führen keine
  git-Befehle aus, die den Zustand ändern. Committet wird zentral.
- Sie editieren nie `daemon/Cargo.toml`; gemeinsame Abhängigkeiten stehen dort
  zentral, Member-Crates referenzieren sie mit `dep.workspace = true`.
- Parallel laufende Agenten bekommen disjunkte Pfade. Wo das nicht geht,
  laufen sie nacheinander.

## Was der Compiler nicht prüft und wir deshalb selbst prüfen

- Abhängigkeitsrichtung nur nach innen: `tools/check-deps.sh`, Teil von
  `make check`.
- Die drei Sandbox-Garantien: `tests/escape/`, rot bis Sprint 1 fertig ist,
  danach Pflicht.
- Jeder Fehlerpfad liefert ein `Diagnostic` mit `why` und wenn möglich `fix`,
  nie einen nackten String.
- Jede Fähigkeit ist zuerst ein RPC; UI und CLI sind dünne Clients ohne
  Fachlogik (ADR-018).

## Sprache

Prosa in Dokumenten und Kommentaren Deutsch, Bezeichner und Code Englisch,
Commit-Texte Englisch. UI-Strings nur über ARB (`en` Quelle, `de` Übersetzung).

## Was wir nicht tun

- Keine Abstraktion über Fremdbibliotheken. Keine neuen Ports ohne ADR mit
  konkretem zweitem Adapter. Keine Mikroservices. Keine eigene Kryptographie.
- Nichts, was die Sicherheitsaussage in `README.md` schwächt, ohne dass
  `docs/SECURITY.md` und `docs/THREAT-MODEL.md` im selben Commit angepasst werden.
