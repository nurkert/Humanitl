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
   - Antigravity über den Claude-Code-Skill `antigravity:ask` bzw.
     `antigravity:review`; darunter läuft die CLI `agy -p "<Auftrag>"`.
   - Codex über den Claude-Code-Skill `codex:rescue` mit Review-Auftrag;
     darunter läuft `codex exec "<Auftrag>"`.
   Die Skills sind Plugins der Claude-Code-Umgebung, nicht Dateien im Repo.
   Fehlen sie, gehen die CLI-Aufrufe direkt.
   Beide lesen `AGENTS.md` (Antigravity zusätzlich `GEMINI.md`, das darauf
   verweist). Der Auftrag an sie nennt immer konkret, was zu prüfen ist, nach
   dieser Vorlage:

   > Read-only review of the working-tree diff for issue HUM-xxx "<Titel>".
   > Specification: `backlog/sprint-N.md` heading `## HUM-xxx`. Check every
   > acceptance criterion against the code, run build and tests, then report
   > confirmed defects only, ranked blocking/major/minor, with file, line and
   > concrete fix. Focus on: <die zwei bis drei heikelsten Punkte des Issues>.
   > Do not modify files.

   Jeden Befund einzeln bewerten: zutreffend, teilweise, nicht zutreffend, mit
   einem Satz Begründung. Zutreffende Befunde vor dem Commit beheben, dann
   `make check` erneut. Nicht übernommene Befunde kurz im Commit-Body nennen,
   damit die Entscheidung nachvollziehbar bleibt.

   Fällt einer der externen Reviewer aus (Usage-Limit, Auth, Netz), ersetzt
   ein eigener, frischer Review-Subagent mit demselben Auftrag und `AGENTS.md`
   als Briefing diesen Reviewer, damit kein Commit blockiert. Nur dann, und nur
   bis der externe Reviewer wieder antwortet; die Ersatzquelle steht im
   Commit-Body.
5. Commit auf eigenem Branch `hum-xxx-kurztitel`, Merge mit `--no-ff` nach
   `main`, Push. `tools/commit-issue.sh` macht Branch, Commit und Merge in
   einem Schritt; es braucht die Dateipfade als Argumente und den Commit-Body
   auf stdin:

   ```sh
   tools/commit-issue.sh HUM-022 rules-engine "feat(rules): label glob matching" \
     daemon/crates/rules backlog/CONVENTIONS.md <<'EOF'
   Was gebaut wurde, in zwei bis vier Sätzen.

   Review: Codex 2 Befunde (1 übernommen, 1 verworfen: ...), Antigravity 1 Befund (übernommen).
   EOF
   git push origin main
   ```

   **Vor jedem Push den Commit-Zustand prüfen, nicht den Arbeitsbaum.**
   `tools/verify-commit.sh` checkt den Commit in einen eigenen leeren Baum aus
   und fährt dort dieselben Schritte wie die CI. Der Arbeitsbaum enthält beim
   Entwickeln fast immer mehr als der Commit, etwa Register-Einträge oder
   Profile, die zu einem anderen Issue gehören; `make check` ist dann grün,
   während derselbe Stand auf `main` nicht baut. Genau so ist die Pipeline am
   2026-09-03 dreimal rot geworden.

   Ein Issue, ein Implementierungs-Commit plus ein Merge-Commit. Nie mehrere
   Issues in einem Commit.
6. Nach dem Push: kurze Zusammenfassung an den Nutzer, was gebaut wurde, was
   die Reviewer fanden, was davon übernommen wurde.

## Regeln für Subagenten

- Subagenten arbeiten nur an den Dateien ihres Issues und führen keine
  git-Befehle aus, die den Zustand ändern. Committet wird zentral.
- Sie editieren nie `daemon/Cargo.toml`; gemeinsame Abhängigkeiten stehen dort
  zentral, Member-Crates referenzieren sie mit `dep.workspace = true`.
- Parallel laufende Agenten bekommen disjunkte Pfade. Wo das nicht geht,
  laufen sie nacheinander.
- Parallel laufende Reviewer bauen in eigene Zielverzeichnisse
  (`CARGO_TARGET_DIR=daemon/target/review-<name>`), damit sie sich nicht das
  Cargo-Lock streitig machen.

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

## Ausgabestil der KI-Agenten

Jeder Claude-Agent in diesem Repository, auch Subagenten und Reviewer-Ersatz,
antwortet und berichtet im Stil `/caveman:caveman` Stufe `full`: keine
Artikel, keine Füllwörter, keine Höflichkeitsfloskeln, keine Erzählung von
Tool-Aufrufen, Fragmente erlaubt, kurze Synonyme. Zahlen, Einheiten,
Negationen (nicht/nie/nur/außer), Fachbegriffe, Bezeichner, Befehle und
Fehlertexte bleiben exakt. Keine erfundenen Abkürzungen, keine Pfeile.

Das gilt nur für Chat- und Berichtstext. Alles, was im Repository bleibt
(Code, Kommentare, Doc-Kommentare, Commit-Texte, Dokumente, Fixtures, ARB)
wird in normaler Prosa geschrieben. Sicherheitswarnungen, unumkehrbare
Aktionen und mehrschrittige Anweisungen, bei denen Weglassungen die
Reihenfolge verwischen, werden ausformuliert.
