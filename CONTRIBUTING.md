# Mitarbeit

## Werkzeuge

| Werkzeug | Version | Hinweis |
|---|---|---|
| Rust | 1.88+ (festgelegt auf 1.95.0 in `daemon/rust-toolchain.toml`) | braucht `rustfmt` und `clippy` |
| Flutter | 3.47.2 (festgelegt in `app/.fvmrc`) | Dart 3.13+ |
| bubblewrap | 0.8+ | Laufzeitabhängigkeit der Sandbox |
| socat | nicht nötig | der Shim bringt seine eigene Brücke mit |

Eine ohne `rustup` installierte Toolchain hat weder `rustfmt` noch `clippy`.
Liegt eine rustup-Toolchain unter `~/.rustup/toolchains/` (auch ohne `rustup`
selbst im PATH), stellt das Makefile deren `bin`-Verzeichnis für die Ziele fmt
und clippy nach vorn. Sonst überspringt `make check` diese beiden Schritte und
sagt das. Die CI installiert beide und läuft mit `STRICT=1`; nichts wird
unformatiert oder mit clippy-Warnungen gemergt.

`make proto` erzeugt die Dart-Seite des Vertrags nur, wenn `protoc` und
`protoc-gen-dart` (`dart pub global activate protoc_plugin`, genaue Version in
`scripts/gen-proto.sh`) im PATH stehen; ohne sie findet die Flutter-Prüfung den
erzeugten Code nicht. `~/.pub-cache/bin` gehört in den PATH.

Optional: `cargo install cargo-deny` für `make rust-deny`.

## Wenn die CI rot ist

Der Job `rust-test` leitet seine Ausgabe in eine Datei. Ein roter Lauf nennt
deshalb die gefallenen Tests als Annotationen am Lauf, eine Zeile je Test,
höchstens zehn Zeilen, und ab dem elften Test sammelt die zehnte Zeile den
Rest. Die ganze Ausgabe liegt als Artefakt `rust-test-log` bei. Beides ist ohne
Schreibrechte am Repository sichtbar, das Job-Log selbst nicht.

`scripts/ci/test-report.sh <log>` ist dieselbe Auswertung, und
`scripts/ci/test-report.sh --self-test` (Teil von `make check`) hält sie
ehrlich.

`tools/ci-gate.sh <sha>` fasst einen Lauf von außen zusammen: je Job das
Ergebnis, bei Rot der Schritt, der fiel (HUM-237).

## Platte

Ein frischer Bauordner des Daemons wiegt nach
`cargo build --workspace --all-targets` rund 6 GiB, und er wächst mit jeder
Änderung einer Abhängigkeit oder eines Feature-Satzes: Cargo schreibt einen
neuen Satz Artefakte und entfernt nie einen alten. Am 2026-09-06 stand der
Ordner dieses Repositorys nach etwa einer Woche Arbeit bei 103 GiB. Das
Dev-Profil trägt deshalb `debug = "line-tables-only"` (`daemon/Cargo.toml`,
dort gemessen); das halbiert jeden Satz und behält Datei und Zeile in jedem
Backtrace.

Ist der Ordner größer geworden, als der Rechner hergeben kann, wird er
zurückgesetzt:

```sh
cargo clean --manifest-path daemon/Cargo.toml
```

Die Flutter-Seite ist dagegen klein (`app/build`, einige hundert MiB);
`flutter clean` setzt sie zurück.

## Arbeit an einem Issue

Ein Issue, ein Branch, benannt `hum-042-kurztitel`. Die Spezifikation jedes
Issues steht in `backlog/sprint-N.md`; vor dem ersten `BACKLOG.md` Abschnitte 2
bis 6 und `backlog/CONVENTIONS.md` lesen.

Vor dem Commit genügen die gezielten Prüfungen des Issues: Tests der berührten
Crates und Pakete, `cargo fmt`, `cargo clippy` auf den berührten Crates,
`make flutter-analyze` bei App-Änderungen und die Mutationsbeweise. Den vollen
Lauf macht die CI auf dem Issue-Branch (nächster Abschnitt). `make check` bleibt
der volle lokale Lauf für alle, die ihn wollen.

Drei Ziele gehören nicht zu `make check`:

- `make flutter-test-dbus`: Die D-Bus-Protokolltests
  (`app/test/features/tray/dbus_live_test.dart`) brauchen einen Session-Bus,
  den die CI nicht hat, und nur auf dem privaten Bus von `dbus-run-session`
  sind die Namen `org.kde.StatusNotifierWatcher` und
  `org.freedesktop.Notifications` frei, sodass der Test sie selbst halten und
  lesen kann, was seine Adapter auf die Leitung legen. Nach Änderungen am Tray
  oder am Benachrichtigungsadapter fahren. Auf dem Bus eines echten Desktops
  überspringen sich dieselben Tests mit Grund, statt sich beim Panel des
  Menschen davor einzutragen.
- `make flutter-test-integration`: die echte Anwendung auf einem Bildschirm
  (`Xvfb :99` genügt) gegen einen echten Daemon, eine Datei nach der anderen;
  mehrere Dateien in einem Aufruf starten die App zweimal auf demselben Gerät,
  und der zweite Start scheitert. Nach Änderungen an der Shell, der
  Warteschlange oder allem fahren, was die App zeigt, während ein Daemon
  antwortet.
- `make flutter-test-daemon`: der Sandbox-Bildschirm gegen einen echten Daemon
  (`app/test/features/sandbox/daemon_live_test.dart`) startet einen echten
  `humanitld` in eigener XDG-Wurzel und eine echte Sandbox mit `bwrap`, und
  beides gehört nicht in eine Prüfung, die auf jedem Rechner in Sekunden grün
  sein muss. Nach Änderungen am Sandbox-Bildschirm, seinen Providern oder der
  RPC `Sandbox` fahren. Ohne `HUMANITL_DAEMON_TESTS=1` meldet die Datei einen
  einzigen übersprungenen Test mit Grund, ein normaler `flutter test`-Lauf
  bleibt unberührt.

### Vom Arbeitsbaum nach `main` (HUM-237)

Das Tor nach `main` ist die CI auf dem Issue-Branch, nicht mehr ein lokaler
Volllauf. `.github/workflows/ci.yml` läuft auch für Pushes auf `hum-*`, und
`tools/commit-issue.sh --ci` führt den ganzen Weg:

1. Commit aus genau den genannten Pfaden auf der Spitze von `origin/main`,
   gebaut in einem eigenen Index, ohne Arbeitsbaum und Index des Aufrufers
   anzufassen.
2. Push des Branches `hum-NNN-kurztitel`.
3. `tools/ci-gate.sh <sha>` wartet über die öffentliche Actions-API auf den
   Push-Lauf genau dieses Commits (ohne `gh`; ein Token in
   `HUMANITL_CI_TOKEN` oder `GITHUB_TOKEN` hebt nur die Ratengrenze) und nennt
   je Job das Ergebnis und bei Rot den Schritt, der fiel. Exit 0 grün, 1 rot,
   2 Aufruf oder API kaputt, 3 kein Urteil (Frist abgelaufen oder Lauf
   abgebrochen).
4. Nur bei Grün: Merge mit `--no-ff` nach `main`, Push, Branch auf `origin`
   löschen.

Hat sich `main` während des Laufs bewegt, setzt das Skript den einen Commit
auf das neue `main` um und lässt die CI erneut laufen. Gemergt wird nur, wenn
`main` der Elternteil des getesteten Commits ist; ein `--no-ff`-Merge auf
dieser Grundlage hat genau den Baum des Branches, also landet auf `main`, was
die CI geprüft hat. Ein Merge auf ein bewegtes `main` ohne neuen Lauf wäre
schneller, prüfte aber einen Stand, den es so nie gab. Ein Rebase des
Arbeitsbaums vor dem Push wäre die andere Möglichkeit; sie scheitert an jedem
Arbeitsbaum, der mehr enthält als das Issue, und das ist der Normalfall.

Nach roter CI bleibt der Branch liegen: nachbessern, Commit ergänzen, zurück
auf einen anderen Branch wechseln (der Issue-Branch darf nirgends ausgecheckt
sein), dann `tools/commit-issue.sh --land hum-NNN-kurztitel`.

**Probelauf des Releases vor dem Merge (HUM-363).** Berührt der Commit einen
Pfad aus `tools/release-dry-paths.txt` (Paketierung, `release.yml`, die
Runner unter `app/linux` und `app/macos`, das Workspace-Manifest und weitere,
die Datei begründet die Auswahl), pusht das Skript denselben Commit zusätzlich
als `release-dry/hum-NNN`. Das löst den Probelauf von
`.github/workflows/release.yml` aus, der RPM, AppImage, AUR und das
macOS-Bündel baut und prüft, was `ci.yml` nicht abdeckt. Gemergt wird erst,
wenn `ci.yml` grün ist und im Probelauf jeder Job grün ist außer dem
übersprungenen `release`. Bei Rot bleiben beide Branches liegen, der Weg ist
derselbe wie oben; die Meldung bei roter CI nennt auch den Probelauf-Branch.
Gelesen wird die Liste aus dem Commit und seinem Elternteil zusammen, nie aus
dem Arbeitsbaum: Ein Commit, der die Liste kürzt oder löscht, wird noch nach
der alten geprüft, eine Änderung der Liste selbst löst den Probelauf immer
aus, und fehlt die Liste in einem der beiden Stände, läuft er ebenfalls. Der
macOS-Teil läuft so nur, wo er etwas Neues prüfen kann.

Das lokale `main` zieht das Skript nur nach, wenn dabei nichts verloren gehen
kann. Hat einer der Pfade, die der Merge verändert, lokale Änderungen (auch
Rechte, gestaged oder nicht), oder ist `main` in einem anderen Arbeitsbaum
ausgecheckt, bleibt es stehen, und das Skript sagt es; der Merge liegt dann
schon auf `origin`, der Abgleich ist `git merge --ff-only origin/main`.

Der Grund für das Ganze bleibt derselbe wie bei `tools/verify-commit.sh`: Der
Arbeitsbaum enthält fast immer mehr als der Commit, und `make check` im
Arbeitsbaum ist grün, während der Commit allein nicht baut. Die CI checkt den
sauberen Commit aus. `tools/verify-commit.sh` ist damit freiwillig; es fährt
dieselben Prüfungen lokal, etwa wenn die CI nicht erreichbar ist. Sein
Zielverzeichnis (`~/.cache/humanitl/verify-target`) baut wie die CI nicht
inkrementell und räumt alte `incremental`-Verzeichnisse bei jedem Lauf weg
(HUM-147).

### Gemeinsamer Build-Cache (sccache)

Jeder Arbeitsbaum eines Agenten hat ein eigenes Zielverzeichnis und übersetzt
die Abhängigkeiten des Workspace neu. `sccache` teilt die übersetzten
Abhängigkeiten über alle Arbeitsbäume. Es ist freiwillig, und weder die Shell
noch `~/.cargo/config.toml` werden dafür geändert.

Installation ohne root, nach `~/.cargo/bin`, gedrosselt wie jeder schwere
Lauf auf diesem Rechner:

```sh
flock --close -w 3600 /tmp/humanitl-verify.lock \
  nice -n 19 taskset -c 0-5 cargo install sccache --locked -j 2
```

Benutzung, je Aufruf:

```sh
make rust-test HUMANITL_SCCACHE=1          # Makefile-Ziele
RUSTC_WRAPPER=sccache SCCACHE_CACHE_SIZE=5G \
  cargo test -p humanitl-rules              # direkter cargo-Aufruf
sccache --show-stats                        # Trefferquote
```

`HUMANITL_SCCACHE=1` setzt im Makefile `RUSTC_WRAPPER`, `SCCACHE_CACHE_SIZE=5G`
und `SCCACHE_DIR=~/.cache/sccache`, wenn sie nicht schon gesetzt sind, und
bricht mit einer Meldung ab, wenn `sccache` fehlt. Die Obergrenze von 5 GB
gilt für den ganzen Cache; sccache wirft das am längsten Ungenutzte hinaus.
Inkrementell übersetzte Crates (die eigenen im Dev-Profil) cacht sccache
nicht; der Gewinn liegt bei den Abhängigkeiten, und die machen den größten
Teil der Bauzeit aus.

In der CI übernimmt `Swatinem/rust-cache` diese Rolle
(`.github/actions/setup-rust/action.yml`); nur Läufe auf `main` schreiben den
Cache, Issue-Branches lesen ihn.

## Sprint-Tor (Sprint gate)

Jeder Meilenstein endet mit einem Demo-Skript, und die Skripte der schon
erreichten Meilensteine bleiben grün (`BACKLOG.md` Abschnitt 8). Ab dem Ende
von Sprint 2 wird nichts gemergt, solange nicht beide in der CI grün sind:

| Meilenstein | Skript | CI-Job |
|---|---|---|
| M1, die versiegelte Box | `tests/e2e/m1_sealed_box.sh` | `e2e` |
| M2, die erste Entscheidung | `tests/e2e/m2_first_decision/run.sh` | `e2e-xvfb` |

`make e2e` fährt beide in dieser Reihenfolge; `E2E_ONLY=m1` oder `E2E_ONLY=m2`
wählt eines. Beide Skripte geben je geprüfter Zusicherung eine Zeile aus, ob
sie hielt oder nicht. Das M2-Skript zählt sie außerdem und scheitert, wenn
weniger liefen als erwartet; ein Lauf, der einen Zweig übersprang, kann also
keinen Erfolg melden. Das M1-Skript trägt denselben Zähler, prüft ihn aber noch
nicht.

**Das M2-Tor fährt die ganze Schleife.** HUM-036 verlangt echten Daemon, echte
Sandbox **und den echten Bildschirm unter xvfb**, am Ende eine gültige
HAR-Datei, und seit HUM-097 passiert genau das: Der Bildschirmtreiber
(`app/integration_test/m2_first_decision_test.dart`) startet vor dem Agenten
und trifft die Entscheidungen der Abschnitte 2 bis 4, während die Anfragen
gehalten werden (die Sammelfreigabe mit Sitzungsregel, die Sperre, die einzelne
Freigabe), filtert dann den Verlauf und schreibt die HAR-Datei, die Schritt 10
zurückliest. Ein grünes `e2e-xvfb` heißt „M2 hält".

Zwei Wege standen früher auf der Liste dessen, wofür ein grüner Lauf nicht
bürgte, und stehen dort nicht mehr. HUM-087 gab dem Daemon `--allow-test-ca`;
jede der siebzehn Anfragen geht jetzt über `https://`, also laufen das Ausgeben
von Blattzertifikaten aus Humanitls eigener CA, der Handshake mit dem Agenten
und die TLS-Sitzung nach oben für jeden freigegebenen und gesperrten Fluss, und
beide Funde entstehen in Bodies, die der Proxy selbst entschlüsselt hat.
HUM-097 brachte die Bildschirmhälfte.

`M2_UI=0` schaltet den Bildschirm ab, für einen Rechner ohne `flutter` oder
`xvfb-run`. Ein solcher Lauf sagt nichts über den Bildschirm und nichts über
das HAR-Format, und er sagt das in seiner eigenen Ausgabe; die CI nimmt diesen
Zweig nie.

## Commit-Nachrichten

Englisch, mit einem Präfix `feat`, `fix`, `test`, `docs`, `chore` oder
`refactor` und dem Bereich in Klammern:

```
feat(rules): label glob matching
fix(proxy): answer 100-continue before buffering
```

## Definition of Done

Aus `backlog/CONVENTIONS.md` Abschnitt 3.12, kurz:

- Akzeptanzkriterien des Issues abgehakt, seine Tests vorhanden und grün.
- Neue Fehlerpfade liefern ein `Diagnostic` mit `why` und wo möglich `fix`.
- Neue Einstellungen tragen Tier, Beschreibung und Vorgabe im Schema.
- Neue sichtbare Texte stehen in `app/l10n/app_en.arb` und `app_de.arb`.
- Kein `unwrap()` oder `expect()` außerhalb von Tests und `main`, kein
  `Err(String)`.
- Jeder öffentliche Typ und jede öffentliche Funktion hat einen
  Doc-Kommentar.
- Die CI ist auf dem Issue-Branch grün, bevor der Merge nach `main` geht
  (`tools/commit-issue.sh --ci`, HUM-237). Sie prüft den sauberen Commit,
  nicht den Arbeitsbaum, und ersetzt damit den Pflichtlauf von
  `tools/verify-commit.sh`.

## Architekturregeln

Abhängigkeiten zeigen nur nach innen; `make deps-lint` erzwingt das. Die
Kern-Crates tragen kein IO, kein async und kein Protobuf. Jede Fähigkeit ist
zuerst eine RPC: Desktop-Anwendung und Kommandozeile sind dünne Clients
desselben Dienstes. Einzelheiten in `docs/ARCHITECTURE.md`.

## Vorabversionen 0.0.x

Bis zum ersten richtigen Release 0.1.0 gibt es Vorabversionen mit der Nummer
0.0.N. Sie entstehen aus einem Tag und dem Workflow
`.github/workflows/release.yml`. Das Release enthält die Pakete
`humanitl_0.0.N_amd64.deb` und `humanitl-0.0.N-1.x86_64.rpm`, das AppImage
`Humanitl-0.0.N-x86_64.AppImage`, das Archiv
`humanitl-0.0.N-linux-x86_64.tar.gz`, für macOS `Humanitl-0.0.N-macos.dmg`
und `Humanitl-0.0.N-macos.zip` sowie `SHA256SUMS`. Das AUR-Paket
`humanitl-bin` ist kein Asset; es wird aus dem Archiv gerendert und vom
Maintainer selbst hochgeladen (`packaging/aur/README.md`). Seit HUM-273 erscheint es auf GitHub als reguläres Release
(„Latest“), nicht mehr als Pre-Release; es ist weiterhin nicht signiert und
nicht für den produktiven Einsatz gedacht. Signaturen kommen mit 1.0.0.

So wird eine Vorabversion geschnitten, und nur so (HUM-363):

```sh
tools/release.sh 0.0.N
```

Das Skript setzt den Tag erst, wenn derselbe Commit den ganzen Release-Bau
schon bestanden hat:

1. `0.0.N` ist genau die nächste Nummer, eins mehr als der höchste
   `v0.0.*`-Tag auf `origin`; eine Lücke oder eine schon vergebene Nummer
   lehnt es ab.
2. Ziel ist die Spitze von `origin/main` (ein zweites Argument darf nur genau
   diese Spitze nennen), und ihr `ci.yml`-Lauf auf `main` ist grün
   (`tools/ci-gate.sh`).
3. Es pusht den Commit als Branch `release-dry/0.0.N`. Das startet den
   Probelauf von `release.yml` mit genau dieser Version: alle Bau- und
   Prüfjobs, `guard` so streng wie beim Tag, ohne den Job `release`. Das
   Skript wartet, bis jeder Job grün ist.
4. Erst dann setzt es den annotierten Tag `v0.0.N` („Humanitl 0.0.N") auf
   denselben Commit, pusht ihn und löscht den Probelauf-Branch. Ist `main`
   während des Probelaufs weitergezogen, sitzt der Tag trotzdem auf dem
   geprüften Commit und nicht auf der neuen Spitze; das Skript sagt es. Genau
   dieser Commit ist vollständig geprüft und liegt auf `main`, eine Lücke
   entsteht nicht. Ist er nicht mehr Vorfahre von `origin/main` (`main` wurde
   umgeschrieben), setzt es keinen Tag.
5. Es wartet auf den Lauf des Tags und nennt die Adresse des Releases.

Ist der Probelauf rot, endet das Skript mit dem roten Job und Schritt, und es
gibt keinen Tag. Der Fehler wird auf `main` behoben, dann derselbe Aufruf mit
derselben Nummer. War es ein Ausreißer, lässt sich der Probelauf in der
Actions-Oberfläche neu starten („Re-run failed jobs"); der erneute Aufruf
wertet den neuen Versuch aus. Scheitert der Lauf des Tags trotz grünem
Probelauf, sagt das Skript es und verweist auf genau diesen Lauf: dort „Re-run
failed jobs". Der Tag bleibt, wo er ist.

Einen Tag von Hand zu setzen (`git tag` und `git push origin v0.0.N`) ist
nicht mehr vorgesehen: So entstand v0.0.14, dessen Lauf nach dem Tag in
`check-rpm` scheiterte und das deshalb kein Release hat. Der Job `guard` prüft
beim Tag weiterhin selbst, dass der Tag die Form `v0.0.N` hat, der Commit auf
`main` liegt und sein CI-Lauf mit `success` endete (läuft CI noch, wartet er
bis zu 45 Minuten).

Ein Tag wird nie verschoben oder neu gesetzt. Ist ein Tag-Lauf trotz allem
endgültig gescheitert, wird der Fehler auf `main` behoben und die nächste
Nummer geschnitten; seine Nummer wird nicht wieder vergeben.

Die Versionsnummer steht im Repository weiter als `0.0.0`. Der Workflow setzt
sie nur in seinem eigenen Auscheckstand in `daemon/Cargo.toml` und
`daemon/Cargo.lock` (`packaging/release/stamp-version.sh`) und übergibt sie
der App mit `flutter build linux --build-name`. Dass `humanitl --version`,
`humanitld --version` und die App dieselbe Nummer tragen, prüft der Lauf und
scheitert sonst.

**Probelauf.** Er baut alles, prüft die Pakete in Containern und lädt die
Dateien als Workflow-Artefakt `humanitl-dry-run-<commit>` hoch (ein Tag-Lauf
nennt sein Artefakt `humanitl-release`), legt aber kein Release an. Ausgelöst
wird er durch einen Push auf `release-dry/0.0.N` (`tools/release.sh`), auf
`release-dry/hum-NNN` (`tools/commit-issue.sh`, Version `0.0.NNN`, siehe oben)
oder unter „Actions", Workflow „release", „Run workflow" mit einer Version.
Außer bei `release-dry/0.0.N` darf er auch einen Commit bauen, der nicht auf
`main` liegt; dass der Commit nicht auf `main` liegt oder CI nicht grün ist,
steht dann als Warnung im Lauf statt als Fehler. Die Schritte lassen sich lokal einzeln nachfahren, die Skripte liegen
unter `packaging/release/` und `packaging/deb/`. Das Paket wird dabei nie auf
dem eigenen Rechner installiert, sondern nur in einem Wegwerf-Container, so wie
es `packaging/deb/check-install.sh` verlangt.

**0.1.0 ist etwas anderes.** Der erste richtige Release folgt HUM-060 in
`backlog/sprint-5.md`: eine `VERSION`-Datei als einzige Quelle aller
Versionsstellen, `CHANGELOG.md`, minisign-Signaturen, AppImage und die
Abnahme-Checkliste. Dieser Workflow reagiert nur auf `v0.0.*`; ein Tag
`v0.1.0` löst hier nichts aus, bis HUM-060 den Workflow erweitert.
