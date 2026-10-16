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
3. Gezielte Prüfungen grün, kein volles `make check`: die Tests der berührten
   Crates und Pakete (`cargo test -p <crate>`, `flutter test <datei>`),
   `cargo fmt --all -- --check`, `cargo clippy -p <crate> --all-targets -- -D warnings`
   für die berührten Crates, `make flutter-analyze` bei Änderungen an der App
   (enthält `dart format`), dazu die Mutationsbeweise des Issues. Den vollen Lauf
   macht einmal die CI auf dem Issue-Branch (Schritt 5). Wer schon vorher alles
   sehen will, darf `make check` oder `tools/verify-commit.sh` fahren; Pflicht
   ist es nicht mehr (HUM-237). Lokal fehlen rustfmt und clippy womöglich; CI
   erzwingt sie mit `STRICT=1`, also Code so schreiben, als liefen sie.
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
   > acceptance criterion against the code, run the tests of the affected
   > crates and packages, then report confirmed defects only, ranked
   > blocking/major/minor, with file, line and concrete fix. Severity
   > threshold: blocking or major only when an acceptance criterion or the
   > security statement in `README.md` is violated, and only with a concrete
   > failure scenario (input, steps, wrong result). Test strictness, wording,
   > naming and comments are minor at most. Ideas beyond the scope of the
   > issue are not findings; list them separately as candidates for a new
   > issue. Focus on: <die zwei bis drei heikelsten Punkte des Issues>.
   > Do not modify files.

   Jeden Befund einzeln bewerten: zutreffend, teilweise, nicht zutreffend, mit
   einem Satz Begründung. Dabei gilt dieselbe Schwelle wie im Auftrag:
   - **blocking/major** nur, wenn ein Akzeptanzkriterium oder die
     Sicherheitsaussage verletzt ist und der Befund ein konkretes
     Fehlerszenario nennt. Fehlt das Szenario, ist der Befund höchstens minor,
     egal wie ihn der Reviewer eingestuft hat.
   - **minor**: Strenge eines Tests, Wortlaut, Namen, Kommentare.
   - **außerhalb des Umfangs**: Ideen, die über das Issue hinausgehen, werden
     kein Befund, sondern ein eigenes Issue im Sprint-File.

   Zutreffende blocking/major-Befunde vor dem Commit beheben, dann die
   gezielten Prüfungen aus Schritt 3 erneut. Zutreffende minor-Befunde, auch
   die aus dem Delta einer zweiten Runde, behebt der Autor ohne neue
   Review-Runde. Nicht übernommene Befunde kurz im Commit-Body nennen, damit
   die Entscheidung nachvollziehbar bleibt.

   Fällt einer der externen Reviewer aus (Usage-Limit, Auth, Netz), ersetzt
   ein eigener, frischer Review-Subagent mit demselben Auftrag und `AGENTS.md`
   als Briefing diesen Reviewer, damit kein Commit blockiert. Nur dann, und nur
   bis der externe Reviewer wieder antwortet; die Ersatzquelle steht im
   Commit-Body.
5. Commit auf eigenem Branch `hum-xxx-kurztitel`, Push des Branches, CI auf
   genau diesem Commit, und erst bei grüner CI Merge mit `--no-ff` nach
   `main` und Push. `tools/commit-issue.sh --ci` macht alles in einem Schritt;
   es braucht die Dateipfade als Argumente und den Commit-Body auf stdin:

   ```sh
   tools/commit-issue.sh --ci HUM-022 rules-engine "feat(rules): label glob matching" \
     daemon/crates/rules backlog/CONVENTIONS.md <<'EOF'
   Was gebaut wurde, in zwei bis vier Sätzen.

   Review: Codex 2 Befunde (1 übernommen, 1 verworfen: ...), Antigravity 1 Befund (übernommen).
   EOF
   ```

   Was das Skript tut, der Reihe nach:
   1. `origin/main` holen und den Commit aus genau den genannten Pfaden auf
      dessen Spitze bauen, in einem eigenen Index. Arbeitsbaum, Index und
      ausgecheckter Branch des Aufrufers bleiben dabei unberührt. Hat sich
      einer der Pfade auf `main` geändert, seit der Arbeitsbaum abzweigte,
      bricht es ab, statt die neuere Fassung zurückzudrehen.
   2. Den Branch pushen.
   3. `tools/ci-gate.sh` wartet über die öffentliche Actions-API auf den Lauf
      dieses Commits und nennt je Job das Ergebnis, bei Rot den Schritt, der
      fiel. Die CI dauert 10 bis 20 Minuten; das Warten läuft am besten im
      Hintergrund.
   4. Nur bei Grün: Merge-Commit mit `--no-ff` auf `main` pushen, Branch auf
      `origin` löschen, lokales `main` nachziehen.

   Hat sich `main` während des CI-Laufs bewegt, setzt das Skript den einen
   Commit auf das neue `main` um, pusht und wartet erneut. Gemergt wird nur,
   wenn `main` noch der Elternteil des getesteten Commits ist; dann trägt der
   Merge-Commit Byte für Byte den Baum, den die CI geprüft hat. Bei roter CI
   bleibt der Branch auf `origin` und `main` unverändert. Dann nachbessern,
   den Commit auf dem Branch ergänzen (`--amend`), den Arbeitsbaum wieder auf
   einen anderen Branch stellen (ein ausgecheckter Issue-Branch wird
   verweigert) und mit `tools/commit-issue.sh --land hum-xxx-kurztitel`
   erneut pushen, prüfen und mergen. Das lokale `main` bleibt stehen, wenn
   ein vom Merge berührter Pfad lokale Änderungen hat (auch Rechte, gestaged
   oder nicht) oder `main` in einem anderen Arbeitsbaum ausgecheckt ist; das
   Skript sagt es, und der Abgleich ist dann
   `git merge --ff-only origin/main` von Hand.

   **Geprüft wird der Commit-Zustand, nicht der Arbeitsbaum.** Der
   Arbeitsbaum enthält beim Entwickeln fast immer mehr als der Commit, etwa
   Register-Einträge oder Profile, die zu einem anderen Issue gehören;
   `make check` ist dann grün, während derselbe Stand auf `main` nicht baut.
   Genau so ist die Pipeline am 2026-09-03 dreimal rot geworden. Die CI
   checkt den sauberen Commit aus und fährt alle Prüfungen, die früher
   `tools/verify-commit.sh` lokal fuhr (Abgleich in `backlog/sprint-4.md`,
   HUM-237). `tools/verify-commit.sh` bleibt als freiwilliges Werkzeug, etwa
   wenn die CI nicht erreichbar ist; dann gilt der alte Weg ohne `--ci`
   (lokaler Merge), aber erst nach grünem `tools/verify-commit.sh` folgt
   `git push origin main`.

   Ein Issue, ein Implementierungs-Commit plus ein Merge-Commit. Nie mehrere
   Issues in einem Commit.
6. Nach dem Push: kurze Zusammenfassung an den Nutzer, was gebaut wurde, was
   die Reviewer fanden, was davon übernommen wurde.

## Regeln für Subagenten

**Gemeinsam genutzte Dateien.** Wo mehrere Agenten dieselbe Datei berühren
müssen (`app/lib/core/ipc/fake_daemon_client.dart`, `app/l10n/*.arb`,
`daemon/crates/core-types/src/diagnostics/codes.rs`), gilt: nur den eigenen
Abschnitt ändern, neue Einträge ans Ende anhängen, die Datei unmittelbar vor
jedem Schreiben neu einlesen, und niemals die Datei als Ganzes neu schreiben
oder ihr Ende abschneiden. Am 2026-09-04 hat ein Agent so 540 Zeilen eines
anderen gelöscht; 351 Zeilen kamen aus einer Zwischenkopie zurück, der Rest
musste nachgebaut werden. Wer eine strukturelle Änderung an einer solchen
Datei braucht, meldet das und bekommt sie, wenn sonst niemand darin arbeitet.

- Subagenten arbeiten nur an den Dateien ihres Issues und führen keine
  git-Befehle aus, die den Zustand ändern. Committet wird zentral.
- Subagenten und Reviewer fahren kein volles `make check` und kein
  `tools/verify-commit.sh`, sondern die gezielten Prüfungen aus Schritt 3:
  Tests der eigenen Crates und Pakete, `cargo fmt --check`, `cargo clippy`
  auf den berührten Crates, `make flutter-analyze` bei App-Änderungen und
  ihre Mutationsbeweise. Den vollen Lauf macht einmal die CI auf dem
  Issue-Branch. Mit `HUMANITL_SCCACHE=1` teilen sich die Arbeitsbäume die
  übersetzten Abhängigkeiten (Rezept in `CONTRIBUTING.md`).
- Sie editieren nie `daemon/Cargo.toml`; gemeinsame Abhängigkeiten stehen dort
  zentral, Member-Crates referenzieren sie mit `dep.workspace = true`.
- Parallel laufende Agenten bekommen disjunkte Pfade. Wo das nicht geht,
  laufen sie nacheinander.
- Parallel laufende Reviewer bauen in eigene Zielverzeichnisse
  (`CARGO_TARGET_DIR=daemon/target/review-<name>`), damit sie sich nicht das
  Cargo-Lock streitig machen.
- Das Scratchpad-Verzeichnis der Sitzung gehoert nicht einem Agenten allein.
  Am 2026-09-05 hat ein Agent dort den Pruefbaum eines anderen vorgefunden und
  sein eigenes Skript ueberschrieben bekommen. Wer darin einen Baum oder eine
  Datei anlegt, nimmt einen Namen, der die Issue-Nummer traegt, und prueft eine
  Kopie danach mit `cmp` nach.
- Wer die Dateien eines Agenten aus dessen Arbeitsbaum uebernimmt, vergleicht
  vorher, welche davon auf `main` seit dem Abzweig des Arbeitsbaums geaendert
  wurden (`git diff --name-only <basis>..HEAD`). Fuer diese Dateien wird
  zusammengefuehrt statt kopiert. Am 2026-09-05 haette ein reines Kopieren von
  `daemon/crates/core-types/src/diagnostics/codes.rs` den Eintrag `PROXY_008`
  eines schon gemergten Issues geloescht; der Commit haette gebaut und alle
  Tests bestanden.

## Was der Compiler nicht prüft und wir deshalb selbst prüfen

- Abhängigkeitsrichtung nur nach innen: `tools/check-deps.sh`, Teil von
  `make check`.
- Adapter-Namen bleiben im Adapter: `tools/check_coupling.py`, Teil von
  `make deps-lint` (HUM-145). Gezählt wird je Datei, wo `bwrap` außerhalb von
  Sandbox-Crate und Shim steht, wo Engine-Typen (`hyper`, `rustls`, `rcgen`)
  und wo `humanitl_proxy` außerhalb des Proxy-Crates benutzt werden. Der Stand
  vom 2026-09-11 steht in `tools/coupling-baseline.toml`; eine Zahl darf
  sinken, nie steigen. Neuer Code spricht die Sandbox über `SandboxBackend`
  und neutrale Begriffe an („Sandbox-Backend", nicht „bwrap"), damit ein
  zweites Backend wie microsandbox oder Docker später nicht an noch mehr
  Stellen ansetzen muss als heute. Sinkt eine Zahl, zieht
  `python3 tools/check_coupling.py --update` die Grundlinie im selben Commit
  nach.
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
