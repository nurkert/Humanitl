# Diagnose-Codes

Applies to: Humanitl 0.1

<!-- Erzeugt aus daemon/crates/core-types/src/diagnostics/codes.rs.
     Nicht von Hand ändern: `UPDATE_DIAG_DOCS=1 cargo test -p humanitl-core --test diag_docs` schreibt die Datei neu. -->

Jeder nicht-grüne Zustand trägt einen Code der Form `BEREICH_NNN`. Der Code steht in der
Meldung, in der Oberfläche und in `audit.jsonl`; er ist der kürzeste Weg von einer
Beobachtung zu ihrer Erklärung. Eine Nummer wird nie wiederverwendet, auch nicht nach dem
Entfernen eines Codes.

## Reservierte Bereiche

| Bereich | Präfix | Von | Bis | Wofür |
|---|---|---|---|---|
| daemon | `DAEMON` | 001 | 019 | 001-004 Start, Erreichbarkeit, Version des Daemons, 005-008 die Nutzer-Unit von `daemon install` (HUM-044), 009 Abschied (HUM-142), 010 Nutzersitzung, 011 Binaries aus dem `AppImage`, 012 `journalctl` (HUM-070), 013 der von systemd übergebene Socket (HUM-053), 014 `daemon uninstall` (HUM-077) |
| ipc | `IPC` | 001 | 009 | gRPC-Schnittstelle, Token, Aufrufe gegen den Zustand |
| config | `CONFIG` | 001 | 029 | 001-006 Datei, Schlüssel, Wertebereiche, 007-009 Profile (HUM-066), 010-012 Test-Wurzel und ihr Flag (HUM-087), 013 der Projektordner der Einrichtung (HUM-044), 017-018 Editor und Gruppenschlüssel (HUM-070), 019-020 der Zugangsschlüssel des Modells (HUM-140, Bereich dafür bis 029 erweitert), 021 die gespeicherten Modellserver (HUM-289), 022 der Server des Projekts (HUM-405), 023 ein entfallener Wert einer Einstellung (HUM-464), 024 die Projektwahl (HUM-532), 025 die Datenorte für `uninstall.sh` (HUM-550) |
| sandbox | `SANDBOX` | 001 | 059 | 001-006 Launcher und Profil, 007 Bridge-Richtung, 010-012 Start-Fehler, 019 verweigerte Versuche ungemeldet (HUM-138), 020-025 /work-Härtung (HUM-043), 030-032 Wegwerf-Sandbox (HUM-350), 033 Vorgabe fällt auf direkt zurück (HUM-357), 034-036 und 048 Wahl des Backends, Untergrenze der Isolation und Zulassung (HUM-431, Bereich dafür bis 059 erweitert), 037-044 und 049 Podman (Sprint 7), 045-047 die Kernel-Stufe (HUM-464), 050 Dienstverwalter des Nutzers (HUM-434), 051 beschreibbare Erweiterung unter der Kernel-Stufe (HUM-466), 053 Start ohne Projektordner (HUM-532), 054 Aufräumplan in Gebrauch oder veraltet (HUM-533), 055 stummes vsock-Gerät der Kernel-Stufe (HUM-486) |
| proxy | `PROXY` | 001 | 019 | Anfragen, Caps, Protokoll, 010-011 Grenzen der Verbindung (HUM-120), 012-019 Entscheidungs-Proxy hinter Humanitl (HUM-511) |
| tls | `TLS` | 001 | 009 | CA, Zertifikate, Handschlag |
| llm | `LLM` | 001 | 019 | LLM-Endpunkt und seine Antworten, 009 der Zugangsschlüssel (HUM-140), 010-011 Inferenz bei einem Anbieter außerhalb dieses Rechners (HUM-516, Bereich dafür bis 019 erweitert) |
| rules | `RULES` | 001 | 019 | 001-008 Regeldatei und Muster, 009-011 Regelspeicher (HUM-027) |
| findings | `FINDINGS` | 001 | 009 | Detektoren für Secrets und personenbezogene Daten |
| catalog | `CATALOG` | 001 | 009 | Gebündelter Domain-Katalog und Rangliste |
| terminal | `TERM` | 001 | 009 | Terminal-Anbindung des Agenten |
| recorder | `RECORDER` | 001 | 009 | Datenbank und Blob-Speicher |
| limits | `LIMIT` | 001 | 009 | Budgets und Zeitgrenzen |
| audit | `AUDIT` | 001 | 009 | Hash-Kette und Export |
| doctor | `DOCTOR` | 001 | 019 | Selbsttest der Installation |
| cli | `CLI` | 001 | 009 | Kommandozeile und ihre Vorbedingungen |
| ui | `UI` | 001 | 009 | Oberflaeche und was ihr die Arbeitsumgebung verweigert |
| agent | `AGENT` | 001 | 029 | 001-007 Agent-Adapter: Startkommando, Vorlagen, Vorprüfung vor dem Start; 008-029 Agent-Depot (ADR-0023, HUM-518 bis HUM-520) |
| edit | `EDIT` | 001 | 009 | Die bearbeitete Anfrage einer `AllowEdited`-Entscheidung (HUM-047) |
| hold | `HOLD` | 001 | 009 | Was eine Freigabe verweigert, obwohl ein Mensch sie wollte: 004 ein bestätigtes Geheimnis unter `hold.hard_block_checksum_secrets` (HUM-049); 001-003 sind nicht vergeben |
| update | `UPDATE` | 001 | 009 | Updates of Humanitl itself (HUM-382): asking GitHub for the latest release, the signed manifest, the download and the exchange of the app bundle |

## Codes

### Bereich daemon

#### DAEMON_001

Daemon nicht erreichbar

**Auslöser.** Der Client findet keinen Socket oder kein Token unter dem Laufzeitpfad, oder die Verbindung dorthin scheitert. Oder Laufzeitverzeichnis und Token gehören nicht dem eigenen Konto, sind ein Symlink oder für Gruppe und Andere offen; dann liest der Client das Token nicht (HUM-212).

**Fix.** Ohne Unit richtet `InstallService` sie ein (`humanitl daemon install`); mit der Unit aus `daemon install` startet `CopyCommand` sie (`systemctl --user start humanitld.service`), mit dem Paket (Dienst und Socket, nicht aus einem `AppImage`) seinen Socket (`systemctl --user start humanitld.socket`); liegen Paket und Unit des Nutzers zugleich da, verdeckt diese das Paket, und `InstallService` räumt es auf. `humanitld` im Vordergrund ist der Weg für die Fehlersuche und steht in der Dokumentation, nicht im Vorschlag (HUM-189). Bei offenen Rechten `CopyCommand` mit `chmod go-rwx`; bei fremdem Besitzer oder Symlink kein Fix. Nur im `/tmp`-Rückfall `OpenUrl` auf die Anleitung, wie man ein eigenes `XDG_RUNTIME_DIR` unter dem Heimatverzeichnis für die ganze Sitzung setzt (`docs/INSTALL.md`); es gilt nach einer neuen Anmeldung.

#### DAEMON_002

Proto-Version inkompatibel

**Auslöser.** Der Daemon meldet eine andere Hauptversion des Vertrags als die Kommandozeile spricht.

**Fix.** `CopyCommand`: beide Seiten auf denselben Stand bringen.

#### DAEMON_003

Socket bereits belegt

**Auslöser.** Auf dem Socket lauscht schon ein Daemon, oder eine verwaiste Datei liegt darauf.

**Fix.** Kein Fix-Knopf: Der Text nennt die laufende Instanz und den Pfad.

#### DAEMON_004

Laufzeitverzeichnis oder Socket nicht anlegbar

**Auslöser.** Das Laufzeitverzeichnis fehlt, ist nicht privat, lässt sich nicht anlegen, oder der Socket-Pfad ist länger als `sun_path` erlaubt.

**Fix.** Meist ohne Fix — der Text nennt Pfad und Grund; beim zu langen Pfad `SetEnv` für ein kürzeres `XDG_RUNTIME_DIR`. Gehört das Laufzeitverzeichnis einem anderen Konto oder ist es ein Symlink (HUM-212), ohne Fix; nur im `/tmp`-Rückfall `OpenUrl` auf die Anleitung, wie man ein eigenes `XDG_RUNTIME_DIR` unter dem Heimatverzeichnis für die ganze Sitzung setzt (`docs/INSTALL.md`); es gilt nach einer neuen Anmeldung.

#### DAEMON_005

Fremde Unit-Datei wird nicht überschrieben

**Auslöser.** Unter dem Unit-Pfad liegt eine Datei, deren erste Zeile nicht die Marke von Humanitl trägt, oder etwas anderes als eine gewöhnliche Datei; mit dem Paket verdeckt sie dessen Unit. Oder die eigene Unit änderte sich zwischen Ansage und Verschieben (dann liegt sie womöglich unter `.bak`, der Befund sagt es).

**Fix.** `CopyCommand`, das die fremde Datei unter einen freien Namen beiseitelegt (`mv -n -- … .bak`); ein `--force` gibt es mit Absicht nicht.

#### DAEMON_006

Unit-Datei nicht schreibbar

**Auslöser.** Die Unit-Datei lässt sich nicht lesen, schreiben oder beiseitelegen (Rechte, Verzeichnis, Dateisystem, kein freier Name).

**Fix.** `CopyCommand` mit dem Pfad, sonst der Verweis auf die Dokumentation.

#### DAEMON_007

humanitld liegt nicht neben humanitl

**Auslöser.** Neben der laufenden Kommandozeile liegt kein `humanitld`, das die Unit starten könnte — oder der eigene Pfad ist gar nicht zu ermitteln.

**Fix.** `CopyCommand` mit `ls -l` auf den erwarteten Pfad; im zweiten Fall kein Fix.

#### DAEMON_008

systemd hat die Unit nicht übernommen

**Auslöser.** `systemctl --user` hat die geschriebene Unit nicht übernommen oder nicht gestartet, oder die Unit wird aus einem anderen Verzeichnis geladen als dem, in das `humanitl` sie geschrieben hat.

**Fix.** `CopyCommand` mit `systemctl --user status humanitld`, dem Aufruf, der sagt, was systemd stört; wird die Unit aus einem anderen Verzeichnis geladen, `systemctl --user show -p UnitPath`.

#### DAEMON_009

Aufgaben beim Abschied abgebrochen

**Auslöser.** Beim Beenden liefen nach der Frist noch Aufgaben des Daemons; der Prozess endet trotzdem.

**Fix.** Kein Fix für Nutzer: Der Text nennt die Frist; ein Bericht mit dem Protokoll dieses Laufs hilft weiter.

#### DAEMON_010

Keine systemd-Nutzersitzung

**Auslöser.** `XDG_RUNTIME_DIR` fehlt, oder `systemctl --user` findet den Bus der Sitzung nicht, und der Befehl braucht die Nutzersitzung.

**Fix.** `CopyCommand`: `loginctl enable-linger $USER`, damit die Sitzung auch ohne Anmeldung steht.

#### DAEMON_011

AppImage-Kopie nicht ablegbar

**Auslöser.** `daemon install` aus einem `AppImage` kann Daemon, Shim, Katalog und Profil nicht nach `~/.local/lib/humanitl/<version>.<stempel>/` kopieren oder `current` nicht umhängen, oder Katalog oder Profil fehlen im Bild.

**Fix.** `CopyCommand` mit `ls -ln` auf das Verzeichnis; der Text nennt den Grund. Fehlen Katalog oder Profil im Bild, `OpenUrl` auf die Veröffentlichungen: Nur ein vollständiges `AppImage` hilft.

#### DAEMON_012

journalctl nicht gefunden

**Auslöser.** `humanitl daemon logs` findet kein `journalctl` im `PATH`.

**Fix.** `CopyCommand`: `sudo apt-get install systemd`, das Paket, das `journalctl` mitbringt.

#### DAEMON_013

Übergebener Socket unbrauchbar

**Auslöser.** systemd übergibt per Socket-Aktivierung mehr als einen Socket, eine Nummer 3, die nicht offen ist, keinen lauschenden Unix-Stream-Socket, oder einen an einem anderen Pfad als `$XDG_RUNTIME_DIR/humanitl/daemon.sock`.

**Fix.** `CopyCommand`: `systemctl --user cat humanitld.socket` zeigt, worauf `ListenStream` zeigt.

#### DAEMON_014

Deinstallation unvollständig

**Auslöser.** `humanitl daemon uninstall` konnte den Dienst nicht abmelden oder eine Datei, die `daemon install` angelegt hat, nicht entfernen.

**Fix.** `CopyCommand` mit dem genauen `systemctl`-Aufruf, der scheiterte, sonst `ls -ld` auf den Pfad, der stehen blieb.

### Bereich ipc

#### IPC_001

Ungültiges Token

**Auslöser.** Das Sitzungs-Token fehlt, ist unlesbar oder passt nicht zu dem des Daemons.

**Fix.** In der Kommandozeile `CopyCommand` mit `humanitld`, der das Token neu schreibt; im Daemon ohne Fix.

#### IPC_002

AllowEdited nur für genau einen Flow

**Auslöser.** `AllowEdited` kam mit keiner oder mehr als einer Flow-Id.

**Fix.** Kein Fix: Ein bearbeiteter Anfrage-Rumpf gehört zu genau einem Fluss.

#### IPC_003

Flow nicht mehr gehalten

**Auslöser.** Der genannte Fluss wartet nicht mehr: entschieden, abgelaufen oder nie gehalten.

**Fix.** Kein Fix: Die Warteschlange zeigt den aktuellen Stand.

#### IPC_004

Decide-Anfrage ungültig

**Auslöser.** Eine `Decide`-Anfrage ist in sich widersprüchlich, etwa ohne Entscheidung oder mit unbekanntem Fluss.

**Fix.** Kein Fix: Der Text nennt das Feld, das nicht stimmt.

#### IPC_005

Rules-Anfrage ungültig

**Auslöser.** Eine `Rules`-Anfrage verlangt etwas, das der Regelspeicher nicht tun kann, etwa eine mitgelieferte Regel zu löschen.

**Fix.** Kein Fix: Der Text nennt die Regel und den Grund.

#### IPC_006

Fähigkeit in diesem Daemon nicht verfügbar

**Auslöser.** Dieser Daemon hat die Fähigkeit nicht, nach der gefragt wurde — keine Sandbox, keine Aufzeichnung, keine Endpunkt-Probe; oder er endet gerade und startet deshalb keine Sandbox mehr.

**Fix.** Kein Fix: Es ist eine Aussage über diesen Daemon, nicht über die Anfrage. Endet er gerade, hilft ein neuer Start, sobald er wieder läuft.

#### IPC_007

Intercept-Anfrage ohne Operation

**Auslöser.** Eine `Intercept`-Anfrage nennt weder `status` noch `pause` noch `resume`; meist ein Client, der älter oder neuer ist als der Daemon.

**Fix.** `CopyCommand` mit `humanitl intercept status`, das den Stand liest; sonst ohne Fix.

### Bereich config

#### CONFIG_001

Config-Datei ungültig

**Auslöser.** Eine Konfigurations- oder Profildatei ist nicht lesbar oder kein gültiges TOML.

**Fix.** Meist ohne Fix — der Text nennt Datei und Parserfehler; beim Start `ChangeSetting` auf das Vorgabeprofil.

#### CONFIG_002

Unbekannter Schlüssel

**Auslöser.** Ein Schlüssel, ein Block oder ein Pfad steht in der Datei, den das Schema nicht kennt.

**Fix.** `ChangeSetting` auf den ähnlichsten Schlüssel, wenn es einen gibt und `humanitl config set` den Wert dort nimmt (HUM-507); sonst ohne Fix.

#### CONFIG_003

Wert außerhalb des Bereichs

**Auslöser.** Ein Wert liegt außerhalb seines Bereichs, hat den falschen Typ, ein Profil hat die falsche Form, oder ein Projekt-Profil setzt einen Schlüssel, der ihm nicht gehört.

**Fix.** `ChangeSetting` auf die Vorgabe, oder `-` für ein Feld ohne Vorgabe, wo es einen Schreibweg gibt und `humanitl config set` den Schlüssel kennt (HUM-507); beim Projekt-Profil bewusst keiner, ebenso bei einer Linker-Variablen in `sandbox.env` oder `[env]`: der Text nennt die Variable, die wegmuss.

#### CONFIG_004

Laufzeitverzeichnis ist ein Ersatz

**Auslöser.** `XDG_RUNTIME_DIR` fehlt, und der Daemon weicht auf ein geteiltes Verzeichnis aus.

**Fix.** `SetEnv` für `XDG_RUNTIME_DIR`.

#### CONFIG_005

Veralteter Schlüssel

**Auslöser.** Ein Schlüssel steht in der Datei, der veraltet ist: entweder entfallen (`alias::RETIRED`) oder unter seinem alten Namen geschrieben.

**Fix.** Beim alten Namen `ChangeSetting` auf den heutigen; beim entfallenen kein Fix — es gibt keinen Nachfolger, die Zeile wird gelöscht.

#### CONFIG_006

Alter und neuer Schlüssel gesetzt

**Auslöser.** Alter und neuer Name desselben Schlüssels stehen zugleich in der Datei.

**Fix.** Kein Fix: Der Text nennt beide; der neue gilt.

#### CONFIG_007

Projekt-Profil gehört einem anderen Konto

**Auslöser.** Das Projekt-Profil gehört einem anderen Konto als dem, das den Daemon fährt.

**Fix.** Kein Fix: Es gilt weiter nur, was ein Projekt setzen darf.

#### CONFIG_008

Eigenes Profil verdeckt ein mitgeliefertes

**Auslöser.** Ein eigenes Profil verdeckt ein mitgeliefertes gleichen Namens und weicht davon ab.

**Fix.** `CopyCommand`, um beide zu vergleichen.

#### CONFIG_009

Profilwunsch des Projekts gilt nicht

**Auslöser.** Das Projekt wünscht ein Profil, das es nicht wählen darf; ein anderes gilt.

**Fix.** `CopyCommand`, das den Wunsch des Projekts als Kommandozeilen-Schalter setzt — die Entscheidung bleibt beim Menschen.

#### CONFIG_010

Test-Wurzel nicht verwendbar

**Auslöser.** `resolver.test_ca` zeigt auf etwas, das keine brauchbare Wurzel ist.

**Fix.** `CopyCommand` zum Prüfen der Datei.

#### CONFIG_011

Test-Wurzel ohne Flag oder Flag ohne Test-Wurzel

**Auslöser.** Test-Wurzel ohne Flag oder Flag ohne Test-Wurzel: die beiden gehören zusammen.

**Fix.** Fehlt das Flag, `CopyCommand` mit dem Start samt `--allow-test-ca`; fehlt der Schlüssel, kein Fix, der Text nennt ihn mit einem Beispiel für den Pfad (HUM-507).

#### CONFIG_012

Test-Wurzel ohne absoluten Pfad

**Auslöser.** `resolver.test_ca` ist kein absoluter Pfad und hinge damit am Startverzeichnis des Daemons; schon das Laden der Konfiguration und `humanitl config set` lehnen ihn ab.

**Fix.** Kein Fix: Der Text verlangt einen absoluten Pfad und nennt ein Beispiel; den Pfad kennt nur der Mensch (HUM-507).

#### CONFIG_013

Kein Projektordner gewählt

**Auslöser.** Niemand hat einen Projektordner gewählt; der Client meldet es, nicht der Daemon.

**Fix.** Kein Fix-Knopf: Der Ordner wird im Setup gewählt.

#### CONFIG_014

Einstellung nicht über den Daemon setzbar

**Auslöser.** `SetConfig` bekommt einen Schlüssel außerhalb des Formulars oder eine Variable unter `sandbox.env`, deren Wert nicht das Zertifikat in der Sandbox ist.

**Fix.** Kein Fix: Der Text nennt, was angenommen wird; alles andere steht von Hand in `config.toml` (`humanitl config edit`).

#### CONFIG_015

config.toml nicht geschrieben

**Auslöser.** `config.toml` ließ sich nicht ändern, ohne mehr als den einen Wert zu ändern, oder nicht schreiben; sie ist unberührt.

**Fix.** Kein Fix: Der Text nennt die Zeile, die von Hand in den Block `[sandbox.env]` gehört.

#### CONFIG_016

Feste Namenszuordnungen gesetzt

**Auslöser.** `resolver.overrides` beantwortet die genannten Namen aus der Konfiguration, statt zu fragen; der Verkehr geht an die Adresse, die dort steht.

**Fix.** `ChangeSetting` auf eine leere Tabelle (`{}`), wenn die festen Adressen nicht gemeint waren.

#### CONFIG_017

Kein Editor gefunden

**Auslöser.** Weder `$VISUAL` noch `$EDITOR` ist gesetzt und weder `nano` noch `vi` liegt im `PATH`, oder der genannte Editor startet nicht.

**Fix.** `SetEnv`: `EDITOR=nano`.

#### CONFIG_018

Schlüssel ist eine Gruppe

**Auslöser.** `humanitl config set` bekommt einen Pfad wie `hold`, unter dem weitere Schlüssel stehen, statt eines einzelnen Werts.

**Fix.** `CopyCommand`: `humanitl config get <gruppe>` zeigt die Schlüssel darunter.

#### CONFIG_019

Zugangsschlüssel nicht im Schlüsselbund

**Auslöser.** Beim Hinterlegen des Zugangsschlüssels antwortet der Schlüsselbund des Systems nicht (Secret Service, unter macOS das Keychain über `/usr/bin/security`), und der Schlüssel liegt im `0600`-Rückfall unter `$XDG_DATA_HOME/humanitl/keys`.

**Fix.** `CopyCommand`: unter Linux bringt `sudo apt-get install libsecret-tools` das Programm `secret-tool`, unter macOS entsperrt `security unlock-keychain` den Schlüsselbund; danach `humanitl llm key set` erneut. Taugt der Schlüssel selbst nicht für `security`, gibt es keinen Vorschlag.

#### CONFIG_020

Zugangsschlüssel nicht gespeichert

**Auslöser.** `LlmKey(Set)` oder `LlmKey(Clear)` bekommt einen Schlüssel, der keiner ist (leer, kürzer als 16 Zeichen, länger als 4096 Bytes, mit Leerraum), eine Anfrage ohne Operation, oder weder Schlüsselbund noch Datei lassen sich schreiben oder löschen.

**Fix.** Bei einem zu kurzen Schlüssel `openssl rand -hex 24` zum Kopieren, für einen lokalen Server mit selbst gewähltem Schlüssel; sonst nennt der Text den Grund, und ein Schlüssel ohne Leerraum oder ein beschreibbares Datenverzeichnis behebt ihn.

#### CONFIG_021

Modellserver nicht geändert

**Auslöser.** `LlmServers` oder `humanitl llm server` bekommt einen ungültigen, doppelten oder unbekannten Namen, einen Endpunkt, der keiner ist, oder soll den aktiven Server ohne Nachfolger oder während einer Sitzung entfernen; `LlmKey` trifft einen Server, der sich beim Ablegen geändert hat oder dessen Verweis ein anderer trägt; `SetConfig` nennt einen Server, der nicht mehr aktiv ist; oder beim Laden gibt es keinen gültigen aktiven Server, einen geteilten Verweis oder einen Endpunkt, der nicht vom aktiven Server kommt; oder `llm.server` aus Lauf, Projekt oder Profil nennt einen Server, der nicht gespeichert ist, und die Sitzung startet nicht (HUM-400).

**Fix.** `CopyCommand`: `humanitl llm server ls` zeigt die Namen; beim Entfernen des aktiven Servers `--use NAME` mit dem Nachfolger; bei einem nicht gespeicherten `llm.server` `humanitl llm server add NAME URL`, oder den Schlüssel an der genannten Stelle entfernen.

#### CONFIG_022

Server des Projekts nicht bestätigt

**Auslöser.** `.humanitl/profile.toml` im Projektordner nennt mit `llm.server` einen gespeicherten Server, der nicht der Standard ist, und für diesen Ordner und diesen Server liegt keine Bestätigung vor. Die Sitzung startet nicht; kein Endpunkt bekommt einen Schlüssel.

**Fix.** `CopyCommand` mit `humanitl llm server confirm NAME --project ORDNER`; die App fragt beim Start einmal nach. Wer den Server nicht will, entfernt `llm.server` aus `.humanitl/profile.toml`.

#### CONFIG_023

Setting value retired

**Auslöser.** `sandbox.podman.runtime` (from `config.toml`, a profile, the environment or the start of a session) names the runtime of the kernel level that the container engine no longer offers. The kernel level is a backend of its own now; the configuration does not load until the value is gone.

**Fix.** `ChangeSetting` on `sandbox.backend` = `qemu`, the backend of the kernel level, and remove `sandbox.podman.runtime`.

#### CONFIG_024

Project choice not allowed

**Auslöser.** The project choice is the per-project layer the daemon keeps on the host, written only through `Projects.save_choice`. It may carry exactly the keys of the first-start screen (`agent.adapter`, `sandbox.mode`, `sandbox.work_mode`, `sandbox.backend`, `sandbox.min_isolation`, `hold.ask_mode`, `llm.server`, `sandbox.profile`); it may not lower `sandbox.min_isolation` below what `config.toml`, a profile or the project profile require, and it may not set a value a company policy locks. The sentence names the key; nothing was saved, or the start stopped.

**Fix.** Save the choice again without the named key (`Projects.save_choice`), or raise `sandbox.min_isolation` instead of lowering it; a locked value is decided by the policy.

#### CONFIG_025

Data location not printable

**Auslöser.** A data location comes from `XDG_CONFIG_HOME`, `XDG_DATA_HOME` or `HOME`, and one of them holds a line break or another control character. `humanitl config paths` prints one location per line, and `uninstall.sh --purge` deletes exactly the locations it reads there; such a path would split into rows that name other directories. Nothing was printed.

**Fix.** Set the variable the sentence names to a path without control characters, then run the command again. No fix action: the right path is known only to the person.

### Bereich sandbox

#### SANDBOX_001

bwrap nicht gefunden

**Auslöser.** `bwrap` liegt nicht im `PATH` des Daemons.

**Fix.** `CopyCommand` mit dem Installationsbefehl.

#### SANDBOX_002

bwrap-Version zu alt

**Auslöser.** Das gefundene `bwrap` ist älter als die Mindestversion des Starters.

**Fix.** `CopyCommand` mit dem Installationsbefehl.

#### SANDBOX_003

User-Namespaces nicht erlaubt

**Auslöser.** Unprivilegierte User-Namespaces sind auf diesem Kernel abgeschaltet.

**Fix.** `CopyCommand` mit dem `sysctl`-Aufruf.

#### SANDBOX_004

Isolation-Check fehlgeschlagen

**Auslöser.** Der Isolations-Check ist fehlgeschlagen; welche der drei Garantien, sagt sein eigener Code.

**Fix.** Kein Fix: Der Start wird abgebrochen.

#### SANDBOX_005

Projektordner nicht beschreibbar

**Auslöser.** Der Projektordner fehlt, ist kein Verzeichnis, ist kein absoluter Pfad, oder er ist bei `rw` nicht beschreibbar.

**Fix.** Beim nicht beschreibbaren Ordner `RemountReadOnly`; sonst kein Fix.

#### SANDBOX_006

Mount verboten

**Auslöser.** Ein Profil, der Proxy-Socket oder ein Agent-Adapter will einen Pfad einhängen, den die Mount-Regeln verbieten.

**Fix.** Beim Projektordner `EditSetting` auf `sandbox.work_dir` (bis HUM-507 ein `ChangeSetting` mit leerem Wert), die Oberfläche bietet die Ordnerauswahl an; sonst kein Fix, der Text nennt Pfad und Regel.

#### SANDBOX_007

Bridge-Richtung unbekannt

**Auslöser.** Ein Profil nennt eine Brücken-Richtung, die es nicht gibt.

**Fix.** Kein Fix: Erlaubt ist heute nur `in`.

#### SANDBOX_010

Argumentliste des Starters unerwartet

**Auslöser.** Die Argumentliste des Starters passt nicht zu dem, was der Launcher erwartet.

**Fix.** `ChangeSetting` auf ein Profil, dessen Argumentliste passt; im Escape-Starter ohne Fix.

#### SANDBOX_011

Platzhalter nicht anlegbar

**Auslöser.** Ein Platzhalter oder Verzeichnis für den Lauf lässt sich nicht anlegen, oder der Schnappschuss des Projekts kann nicht gelesen werden.

**Fix.** Meist ohne Fix — der Text nennt Pfad und Fehler; fehlt der Shim, `CopyCommand` zum Bauen.

#### SANDBOX_012

Kommandozeile des Starters ungültig

**Auslöser.** Die Kommandozeile des Starters ist ungültig, ein Plan wurde zweimal gestartet, oder der wartende Thread ist gescheitert.

**Fix.** Kein Fix: Der Fehler liegt im Starter oder im Plan, nicht in einer Einstellung.

#### SANDBOX_013

Isolation-Check ohne Bericht

**Auslöser.** Der Shim hat keinen vollständigen Bericht geliefert; die drei Garantien sind damit unbelegt.

**Fix.** Kein Fix: Der Start wird abgebrochen.

#### SANDBOX_014

Isolation-Check 1: Netzwerk-Interface vorhanden

**Auslöser.** Check 1: In der Sandbox steht mehr als `lo` als Netzwerkschnittstelle.

**Fix.** Kein Fix: Der Daemon beendet die Sandbox.

#### SANDBOX_015

Isolation-Check 2: mehr als eine Tür

**Auslöser.** Check 2: In der Sandbox steht mehr als der eine erlaubte Unix-Socket.

**Fix.** Kein Fix: Der Daemon beendet die Sandbox.

#### SANDBOX_016

Isolation-Check 3: seccomp unwirksam

**Auslöser.** Check 3: Der seccomp-Filter ist nicht aktiv oder lässt zu viel durch.

**Fix.** Kein Fix: Der Daemon beendet die Sandbox.

#### SANDBOX_020

Maskierter Pfad freigegeben

**Auslöser.** Ein Profil hebt eine Maske auf; der Agent darf den Pfad lesen und schreiben.

**Fix.** Kein Fix: Es ist eine Aussage über das gewählte Profil.

#### SANDBOX_021

Kernel ohne openat2

**Auslöser.** Der Kernel kennt `openat2` nicht; der Projektordner wird mit `openat` und `O_NOFOLLOW` gelesen.

**Fix.** Kein Fix: Die Prüfung läuft, nur langsamer und mit weniger Garantie.

#### SANDBOX_022

Symlink zeigt aus dem Projekt hinaus

**Auslöser.** Der Agent hat einen Symlink angelegt, der aus dem Projekt hinauszeigt.

**Fix.** Kein Fix: Der Bericht nennt Ziel und Quelle.

#### SANDBOX_023

Mögliche Geheimnisse im Projekt

**Auslöser.** Im Projekt stehen nach dem Lauf Zeichenketten, die wie Geheimnisse aussehen.

**Fix.** Kein Fix: Der Bericht nennt Datei und Fundzahl.

#### SANDBOX_024

Schnappschuss abgeschnitten

**Auslöser.** Ein Budget hat den Schnappschuss des Projekts vorzeitig beendet.

**Fix.** Kein Fix: Der Bericht sagt, welcher Teil gekürzt ist.

#### SANDBOX_025

Ohne Maske ins Projekt geschrieben

**Auslöser.** Der Agent hat unter einen maskierten Pfad geschrieben, der im Projekt nicht existierte.

**Fix.** Kein Fix: Der Bericht nennt die Dateien.

#### SANDBOX_026

Datei im Projekt, die der Rechner ausführt

**Auslöser.** Der Agent hat Dateien geschrieben, die dieser Rechner von sich aus ausführt (Hooks, Build-Dateien).

**Fix.** Kein Fix: Der Bericht nennt die erste davon.

#### SANDBOX_027

Keine Zusammenfassung zu diesem Lauf

**Auslöser.** Zu dieser Sandbox ist keine Zusammenfassung aufgezeichnet.

**Fix.** Kein Fix: Entweder lief sie hier nie, oder der Lauf hat keine hinterlassen.

#### SANDBOX_028

Geänderte Datei nicht durchsucht

**Auslöser.** Geänderte Dateien wurden nicht nach Geheimnissen durchsucht, weil ein Budget zuschlug.

**Fix.** Kein Fix: Der Bericht nennt Zahl und erste Datei.

#### SANDBOX_029

Sandbox erst nach der Frist beendet

**Auslöser.** Der Agent hat `SIGTERM` nicht beantwortet, und nach der Frist folgte `SIGKILL`; oder danach lag kein Exit-Status vor.

**Fix.** Kein Fix, wenn `SIGKILL` gewirkt hat oder nur der Status ausblieb; lebt der Prozess weiter, nennt der Text `ps -o stat= -p <pid>`.

#### SANDBOX_019

Verweigerte Verbindungsversuche werden nicht gemeldet

**Auslöser.** Der Kernel hat dem Filter des Agenten keinen Zuhörer gegeben; verweigert wird weiter mit `EPERM`, gezählt und gemeldet wird nichts.

**Fix.** Kein Fix-Knopf: Der Text nennt den errno; `EBUSY` heißt, dass Humanitl selbst in einer Umgebung mit eigenem seccomp-Zuhörer läuft.

#### SANDBOX_030

Disposable sandbox not possible

**Auslöser.** The start asked for a disposable sandbox, and one of its conditions is missing: bubblewrap 0.11 or newer, a runtime directory on tmpfs, a writable view of the project folder, or no changes of an earlier disposable run still pending. On macOS the project must be clonable: an APFS volume shared with the runtime directory and every file readable. A container-engine backend has no disposable sandbox at all: the engine resolves the project path itself, so its lower layer cannot be tied to the checked folder.

**Fix.** `CopyCommand` with the install command for a newer bubblewrap, `ChangeSetting` on `sandbox.work_mode` = `rw`, `SetEnv` for `XDG_RUNTIME_DIR` = `/run/user/<uid>`, `CopyCommand` with `humanitl sandbox changes` for pending changes, on macOS `ChangeSetting` on `sandbox.mode` = `direct`, or under a container-engine backend `ChangeSetting` on `sandbox.backend` to a backend that has one.

#### SANDBOX_031

Change of a disposable run not applied

**Auslöser.** Applying the changes of a disposable run stopped at a path: the project file changed since the start, the change is not something Humanitl writes to the host (a symbolic link out of the project, a device, FIFO or socket), a parent in the project is a symbolic link now, there are no pending changes, or the write failed.

**Fix.** `CopyCommand` with `humanitl sandbox apply --overwrite` after a look at the conflicting file, `CopyCommand` with `humanitl sandbox apply` and a smaller selection, or `CopyCommand` with `humanitl sandbox discard`.

#### SANDBOX_032

Disposable layer full

**Auslöser.** The changes of a disposable run grew over the cap of the upper layer in RAM (bytes or entries), and the daemon stopped the sandbox so the agent cannot fill the memory.

**Fix.** No fix for the run; the changes so far wait under `humanitl sandbox changes`. A task that needs more space runs without `--disposable`.

#### SANDBOX_033

Disposable sandbox not available, running direct

**Auslöser.** The default `sandbox.mode = "auto"` asks for a disposable sandbox where the system allows one, and this system does not: bubblewrap is older than 0.11, the runtime directory is not on tmpfs, the project folder is mounted read-only, on macOS the project cannot be cloned, or the backend is a container engine, which has no disposable sandbox. This run writes to the project folder directly.

**Fix.** The fix of the cause (install command for a newer bubblewrap, `SetEnv` for `XDG_RUNTIME_DIR`, `ChangeSetting` on `sandbox.work_mode` = `rw`, `ChangeSetting` on `sandbox.backend` to a backend with a disposable sandbox under a container-engine backend), or `sandbox.mode = "direct"` to choose direct mode and silence this notice.

#### SANDBOX_034

Selected sandbox backend not available

**Auslöser.** `sandbox.backend` (from `config.toml`, a profile, the environment or the start of a session) names a backend this build or this system cannot run, for example `seatbelt` on Linux. The start stops; it never takes another backend instead.

**Fix.** `ChangeSetting` on `sandbox.backend` = `auto`, the backend of this system.

#### SANDBOX_035

Sandbox backend below the isolation floor

**Auslöser.** The backend chosen by `sandbox.backend` isolates at a lower level than `sandbox.min_isolation` demands. The floor is the strictest value of all layers; a project profile may raise it but never lower it.

**Fix.** `CopyCommand` with `humanitl config get --origin sandbox.min_isolation`, which names the layer that set the floor. Lowering it there, or choosing a backend that reaches it, is the person's decision; there is no button for it.

#### SANDBOX_036

Sandbox profile requires another backend

**Auslöser.** The sandbox profile requires the backend it names under `[sandbox] backend`, and the configuration chose a different one. The start stops; the profile does not override the choice, and the choice does not override the profile.

**Fix.** `ChangeSetting` on `sandbox.backend` with the backend of the profile when that one is admitted; otherwise choose a sandbox profile without a fixed backend (`sandbox.profile`).

#### SANDBOX_048

Sandbox backend not admitted yet

**Auslöser.** `sandbox.backend` names an isolation level that is not admitted yet. A backend becomes selectable only after its mount check and the escape suite against it are green in CI; until then every start refuses it, whether it comes from `config.toml`, the environment, a profile or the start of a session.

**Fix.** `ChangeSetting` on `sandbox.backend` = `auto`, the admitted backend of this system.

#### SANDBOX_037

Podman too old or without a needed capability

**Auslöser.** `sandbox.backend = "podman"` needs Podman 4.9 or newer with the OCI runtime `crun`; the installed one is older, runs another runtime, or did not answer `podman info`. The start stops and never takes another backend.

**Fix.** `CopyCommand` with the install command for Podman and `crun` of this distribution.

#### SANDBOX_038

Podman not rootless or not local

**Auslöser.** The sandbox needs rootless Podman that runs on this machine, started by the daemon itself. The found Podman runs as root, or it talks to a remote service such as a Podman machine; neither sees the descriptors and files the daemon hands over.

**Fix.** `ChangeSetting` on `sandbox.backend` = `auto`, the way back to the backend of this system; installing does not help here. Run the daemon as the user, without a remote connection, to use Podman.

#### SANDBOX_039

No subordinate IDs for the container

**Auslöser.** `/etc/subuid` or `/etc/subgid` gives this user fewer than 65536 subordinate IDs, or `newuidmap` and `newgidmap` are missing. Rootless Podman cannot map the user into the container without them.

**Fix.** `CopyCommand` with what is missing: the package with `newuidmap` and `newgidmap` (`uidmap` on Debian), `sudo usermod --add-subuids 100000-165535 --add-subgids 100000-165535 <user>` for the ranges, or both joined with `&&`. The range may overlap one of another account; choose a free one then.

#### SANDBOX_040

Container limits cannot be enforced

**Auslöser.** The sandbox under Podman runs with a memory, CPU and process limit, and the cgroup v2 controllers for them are not delegated to this user. A sandbox without its limits does not start.

**Fix.** `CopyCommand` with the systemd drop-in that delegates `cpu`, `memory` and `pids` to user sessions, then a new login.

#### SANDBOX_041

Container of an earlier session found

**Auslöser.** A container that carries the label of this daemon outlived its session, for example after a crash of the daemon. The daemon removes such containers when it starts; the sentence says whether that worked.

**Fix.** None when it was removed; otherwise `CopyCommand` with the command that removes it by hand.

#### SANDBOX_044

SELinux would relabel the project

**Auslöser.** SELinux enforces labels on this machine. A container could read the project only after it was relabelled (`:z`), and that changes the labels of the user's own files. Until there is a design for it, the Podman backend does not start on such a machine.

**Fix.** `ChangeSetting` on `sandbox.backend` = `auto`.

#### SANDBOX_042

Mounted folder is not the checked one

**Auslöser.** Inside the sandbox, the project folder, an entry of `mounts.ro`, `mounts.extra_ro`, `mounts.extra_rw` or the agent program has another device and inode than the one the launcher opened and checked. The folder was swapped between the check and the mount, for example renamed and replaced. The agent did not start; the sentence names the path, the expected and the found pair.

**Fix.** `EditSetting` on `sandbox.work_dir`, the folder choice: check who replaced the named path since the plan, and start the session again once the folder is the one you chose. A folder that keeps changing (a mount point that is mounted while the session starts) cannot be a mount source.

#### SANDBOX_049

Mount table differs from the plan

**Auslöser.** The shim compared the mounts inside the sandbox with the table the launcher planned, entry by entry with type, source, read-only and the flags `nosuid`, `nodev` and `noexec`, and found a difference: an extra mount (for example one that a configuration file of the container engine adds), a missing one, or a second mount stacked onto a planned one. The agent did not start; the sentence names the missing and the extra entries.

**Fix.** The sentence names each extra mount point. Another backend does not help: the sandbox takes such a mount from the mount namespace it starts in. When the mount is the cover that `ProtectKernelModules` in the daemon's service unit of Humanitl 0.4.0 lays over `/usr/lib/modules` (the daemon runs as a service and sees a mode `000` directory there, HUM-529): with the package, update it and run `systemctl --user daemon-reload && systemctl --user restart humanitld.service` (`CopyCommand`); without the package (archive, AppImage), `InstallService` (`humanitl daemon install`). A mount from a configuration of the container engine (`mounts.conf`, `containers.conf`): remove it there. Otherwise `OpenUrl` to this entry; find what mounts at the named point and report the sentence.

#### SANDBOX_043

Sandbox image missing or not the pinned one

**Auslöser.** The root of the sandbox is an image of the container engine, named with its digest. Before every start the daemon asks the engine for exactly that digest; the image is missing, the local image under that name has another digest, or its environment sets a variable the shim reads. A start never pulls an image; pulling is a separate step the person asks for, and it uses the network directly, not through the proxy. The same code ends a pull that failed, or that the person cancelled (HUM-457).

**Fix.** `CopyCommand` with `humanitl sandbox image pull <reference>` when the image is missing or another digest is present; `ChangeSetting` back to the host root when the image itself cannot be used.

#### SANDBOX_050

User service manager unreachable

**Auslöser.** The daemon runs under its hardened user unit, where a rootless container engine cannot map its user namespace. It therefore starts the engine as a transient unit of the user's service manager instead of loosening its own hardening, and that manager did not answer, is older than systemd 254, or `systemd-run` or `systemctl` is missing. The start stops and never takes another backend.

**Fix.** `CopyCommand` with `systemctl --user status`, which shows whether the user's service manager runs and answers.

#### SANDBOX_045

Kernel level prerequisite missing

**Auslöser.** The kernel level starts a micro VM with a guest kernel of its own and needs on this host: `/dev/kvm` readable and writable for this user, the machine emulator with the machine type and vsock device it uses, the file system daemon with `--readonly`, `--fd` and `--cache`, the vsock device with `--uds-path` and `--groups`, bubblewrap with `--bind-fd` and `--size`, a readable kernel image with virtiofs built in (`CONFIG_VIRTIO_FS=y` in the configuration beside the image, `config-<release>` beside `vmlinuz-<release>`) and the module directory of that kernel. The sentence names the first one that is missing; nothing starts, and no other backend takes over.

**Fix.** `CopyCommand` with the package that brings the missing piece (for `/dev/kvm` the command that adds this user to the group `kvm`), or, for a configured kernel image that is not there, `ChangeSetting` that removes `sandbox.qemu.kernel` (`-`, the running kernel's); where only the person knows the right path, the sentence names the setting and there is no fix (HUM-507). When the running kernel's image or modules are gone while those of another kernel are installed, an upgrade removed its package: `CopyCommand` with `systemctl reboot` into an installed kernel, since installing the package again changes nothing. When the module directory is hidden from the daemon (its service unit of Humanitl 0.4.0 set `ProtectKernelModules`, HUM-529): with the package, update it and run `systemctl --user daemon-reload && systemctl --user restart humanitld.service` (`CopyCommand`); without the package (archive, AppImage), `InstallService` (`humanitl daemon install`).

#### SANDBOX_046

Kernel level door not set up

**Auslöser.** The only way out of the micro VM is one vsock port that the daemon forwards to the proxy socket of the session. The vsock device process of the session did not come up, or the daemon could not open its one listener behind it. The session does not start.

**Fix.** No automatic fix; the sentence names the step that failed. `humanitl doctor` checks the prerequisites of the kernel level.

#### SANDBOX_047

Host check of the VM monitor failed

**Auslöser.** After the start the daemon checks the processes of the session from the host: each sees only `lo`, the monitor holds only the descriptors of the plan, and the runtime directory of the session holds only the declared sockets. One of these did not hold; the session was stopped.

**Fix.** No automatic fix; the sentence names what was found. `ChangeSetting` on `sandbox.backend` = `auto` and report the sentence.

#### SANDBOX_051

Writable extension not possible at the kernel level

**Auslöser.** The sandbox profile lists folders under `mounts.extra_rw`, and the backend is the kernel level. Its guest gets the folders of the host through a read-only share of the root and one writable share for the project at `/work`; a writable extension would be another writable way to the host that nothing reviews. The start stops instead of mounting the folder read-only without saying so.

**Fix.** `ChangeSetting` on `sandbox.backend` = `auto` for a session that needs the folder writable; otherwise move the entry from `mounts.extra_rw` to `mounts.extra_ro` in the sandbox profile, or put the folder into the project.

#### SANDBOX_052

Kernel level limits cannot be enforced

**Auslöser.** The unit of a kernel-level session binds at most a fixed amount of memory and a fixed number of processes and threads (`MemoryMax`, `MemorySwapMax`, `TasksMax`), and the cgroup v2 controllers `memory` and `pids` are not delegated to the user's service manager, its cgroup cannot be read, or the running unit's cgroup does not carry the planned `memory.max`, `memory.swap.max` and `pids.max` (a slice in between withholds a controller). systemd would ignore the limits without saying so; a session without its limits does not start, and the agent never runs.

**Fix.** `CopyCommand` with the systemd drop-in that delegates `cpu`, `cpuset`, `io`, `memory` and `pids` to user sessions, then a new login.

#### SANDBOX_053

Start without a project folder

**Auslöser.** The start names no project folder: the request carries no `work_dir`, and `sandbox.work_dir` names none. The daemon does not take its own working directory or the home directory instead, because a launcher of the desktop starts it there and the agent would see a folder nobody chose. Nothing started, and the project register has no entry for it.

**Fix.** Choose a folder: open the start window of the app (`humanitl app`), run `humanitl go` or `humanitl run` inside the project folder, or pass `--work DIR`; `CopyCommand` with `humanitl run --work <dir>`.

#### SANDBOX_054

Cleanup plan in use or out of date

**Auslöser.** Cleanup deletes only a plan the daemon listed item by item and a person confirmed. `Projects.cleanup` computes the plan again in the deleting transaction and compares its digest, which binds the identity and version of every item (session and flow ids, blob hashes, the id of the waiting layer, project id with version, the hash of the project choice). A difference, a running session or changes that wait without the explicit choice stop it, and nothing is deleted. The project folder and the audit chain are never part of a plan.

**Fix.** Show the plan again (`humanitl projects clean` or `humanitl projects forget`) and confirm the new one; stop the running session first (`humanitl sandbox stop`), or decide the waiting changes (`humanitl sandbox changes`, `apply`, `discard`) or include them explicitly with `--include-waiting-changes`.

#### SANDBOX_055

Kernel level vsock device silent

**Auslöser.** The daemon reaches the guest of the kernel level only through the vsock device of the session. The guest had said on the door that it listens, a line that travels through the device's queues, and then a `CONNECT` of the daemon to the guest's report port got no answer at all for ten seconds; a working device answers within milliseconds, with `OK` or a refusal. Version 0.3.0 of the vsock device program behaves like this after its event thread has ended (a request that reached it while the guest's driver was still setting up its queues; fixed in the upstream repository after 0.3.0); the process lives on and answers nothing. Without the report the guarantees are unproven, and the session does not start. A guest kernel that hangs right after its ready line looks the same from the host; the sentence names the stage and the time waited.

**Fix.** No automatic fix: start the session again. If it repeats, install a vsock device program newer than 0.3.0, or point the setting of the kernel level that names it at a build from the upstream repository that carries the fix, and report the sentence.

### Bereich proxy

#### PROXY_001

Body über Cap

**Auslöser.** Reserviert für einen Rumpf über der Grenze, bis zu der Humanitl ihn hält oder aufzeichnet. Heute baut diesen Befund niemand: Die Grenzen aus `limits.*` greifen ohne eigene Meldung, gekürzt wird mit Vermerk am Fluss.

**Fix.** Kein Fix: Solange nichts den Code baut, erscheint er nirgends.

#### PROXY_002

Authority-Mismatch

**Auslöser.** Ziel des `CONNECT`, SNI und `Host` beziehungsweise `:authority` passen nicht zusammen (Domain Fronting).

**Fix.** Kein Fix: Die Anfrage wird blockiert.

#### PROXY_003

Upstream-Verbindung fehlgeschlagen

**Auslöser.** Die Verbindung zum Ziel kommt nicht zustande, oder es ist keine Adresse angeheftet. Am Fluss: das Zertifikat des Ziels stammt von einer Stelle, der dieser Rechner nicht vertraut.

**Fix.** Bei unbekannter Stelle `CopyCommand` mit `sudo update-ca-certificates` (die CA vorher als `.crt` nach `/usr/local/share/ca-certificates/`), bei `tls.trust_system_store = false` `ChangeSetting` auf `true`; sonst kein Fix, der Text nennt Ziel und Ursache.

#### PROXY_005

Ungültiger Übergang im Flow

**Auslöser.** Ein Fluss soll einen Übergang nehmen, den sein Zustand nicht erlaubt.

**Fix.** Kein Fix: Die Anfrage wird blockiert statt in einem unklaren Zustand fortgesetzt.

#### PROXY_007

HTTP/2 nicht verfügbar

**Auslöser.** Reserviert für den Fall, dass HTTP/2 nach oben verlangt, aber nicht verfügbar ist. Heute baut ihn niemand — der Proxy spricht nach oben HTTP/1.1, und `experimental.h2_upstream` hat keinen Leser im Weg dorthin (HUM-108).

**Fix.** Kein Fix: Der Code wartet auf den Weg, den er beschreiben soll.

#### PROXY_008

Private Zieladresse abgelehnt

**Auslöser.** Das Ziel löst auf eine private Adresse auf, und keine Regel erlaubt das.

**Fix.** `AddRule` mit `allow_private` für genau diesen Host.

#### PROXY_009

Anfrage ist keine Meta-Anfrage

**Auslöser.** Ein Fluss soll als vom Proxy selbst beantwortet gelten, ging aber nicht an `humanitl.internal`.

**Fix.** Kein Fix: Es ist ein Fehler im Daemon, keine Eingabe des Nutzers.

#### PROXY_010

Verbindungsgrenze erreicht

**Auslöser.** Die Zahl gleichzeitiger Verbindungen aus der Sandbox hat `limits.max_client_connections` erreicht.

**Fix.** `ChangeSetting` auf die doppelte Grenze, höchstens die Obergrenze der Prüfung; steht sie schon dort, kein Fix (HUM-507).

#### PROXY_011

Anfrage-Rumpf ist stehengeblieben

**Auslöser.** Ein Anfrage-Rumpf ist stehengeblieben und hat die Frist überschritten.

**Fix.** `ChangeSetting` auf die doppelte Frist, höchstens die Obergrenze der Prüfung; steht sie schon dort, kein Fix (HUM-507).

#### PROXY_012

Decision proxy unreachable

**Auslöser.** `egress.proxy` is set, so every allowed request of the agent leaves through that proxy, and the daemon could not open a connection to it within `limits.connect_timeout_secs`. The request did not go out directly instead; the agent got `502`.

**Fix.** `CopyCommand` with `humanitl egress check` once the proxy runs; otherwise set `egress.proxy` to the address it listens on (`humanitl config set egress.proxy URL`), or remove it with `-` to leave without it.

#### PROXY_013

Decision proxy refused the tunnel

**Auslöser.** Humanitl asked the decision proxy for a tunnel to the target with `CONNECT`, and the proxy answered with another status than `200`, or not with HTTP at all. The sentence names the status; the agent got `502`.

**Fix.** No automatic fix; the sentence names the target and the status. `humanitl egress check` repeats the question.

#### PROXY_014

Decision proxy wants credentials

**Auslöser.** The decision proxy answered `407 Proxy Authentication Required`. Either `egress.proxy_auth_ref` is empty, or the credentials it names are not the ones the proxy expects. The agent got `502`; it never sees the `407`.

**Fix.** `CopyCommand` with `humanitl egress auth set`, which stores `USER:PASSWORD` in the keyring and sets `egress.proxy_auth_ref`.

#### PROXY_015

Certificate does not match the decision proxy CA

**Auslöser.** Through the tunnel of the decision proxy, Humanitl speaks TLS with the name of the target and checks the certificate against the CA in `egress.proxy_ca` and the usual anchors. The certificate came from neither: `egress.proxy_ca` is empty or names another CA than the one the proxy signs with. That CA is trusted only on this way, never on a direct connection.

**Fix.** `EditSetting` on `egress.proxy_ca`: the app opens the field, the command line prints `humanitl config set egress.proxy_ca <value>`; the sentence names the PEM file to put there, for mitmproxy the absolute path of `~/.mitmproxy/mitmproxy-ca-cert.pem` (HUM-507).

#### PROXY_016

Local name refused before the decision proxy

**Auslöser.** Behind a decision proxy, names that are local by definition (`localhost`, `*.localhost`, `*.local`, `*.internal`, `*.home.arpa`, a name without a dot such as `metadata`) mean the machine where they are resolved or connected to: its own services, its network or the metadata service of a cloud. Humanitl refuses them before any resolution and before forwarding, with either value of `egress.proxy_resolve`, as it refuses private address literals (`PROXY_008`). The agent got `502`.

**Fix.** `AddRule` with `allow_private` for exactly this host, if the agent should reach it.

#### PROXY_017

Decision proxy settings unusable

**Auslöser.** `egress.proxy` is set, and a setting it needs cannot be used: `egress.proxy_ca` cannot be read or holds no certificate, or the keyring does not hand out the credentials `egress.proxy_auth_ref` names. The daemon read them at its start; every allowed request fails with this finding and nothing leaves directly.

**Fix.** `EditSetting` on the named key (on `egress.proxy` when there is no decision proxy to check): the app opens the field, the command line prints `humanitl egress auth set` or `humanitl config set KEY <value>` (HUM-507); then restart the daemon (`humanitl daemon restart`).

#### PROXY_018

TLS through the decision proxy failed

**Auslöser.** Through the tunnel of the decision proxy, Humanitl speaks TLS with the name of the target. The handshake failed for another reason than an unknown CA, such as a certificate that names another host, or it did not finish within `limits.header_timeout_secs`. The sentence names the proxy, the target and the reason; the agent got `502`.

**Fix.** `CopyCommand` with `humanitl egress check`, and check that the proxy intercepts this host with a certificate for its name.

#### PROXY_019

Decision proxy answered too late

**Auslöser.** The request went through the decision proxy, and the head of its answer did not arrive within `egress.proxy_response_timeout_secs`, also after a streamed upload. A person deciding at the proxy may need longer. The agent got `502`.

**Fix.** `ChangeSetting` on `egress.proxy_response_timeout_secs` with a longer wait.

### Bereich tls

#### TLS_001

Client hat Humanitl-CA abgelehnt

**Auslöser.** Ein Client in der Sandbox hat die Humanitl-CA abgelehnt (`unknown_ca` oder `bad_certificate`).

**Fix.** `SetEnv` mit der CA-Variablen, die dieses Werkzeug liest.

#### TLS_002

Client bricht den Handschlag wiederholt ab

**Auslöser.** Ein Client bricht den Handschlag zu demselben Host wiederholt ab.

**Fix.** `AddRule`: den Host blocken, damit der Agent schnell scheitert statt zu hängen.

#### TLS_003

Client ohne SNI

**Auslöser.** Ein Client hat ohne SNI verbunden; nichts bindet den Handschlag dann an den Host des Tunnels.

**Fix.** Kein Fix: Die Verbindung wird abgewiesen, weil ihre Anfragen keinem Ziel zuzuordnen wären.

#### TLS_004

CA-Verzeichnis nicht beschreibbar

**Auslöser.** Das CA-Verzeichnis ist nicht beschreibbar.

**Fix.** `CopyCommand` mit dem Pfad.

#### TLS_005

CA-Dateien unbrauchbar

**Auslöser.** Die CA-Dateien sind unbrauchbar: Zertifikat oder Schlüssel lassen sich nicht lesen.

**Fix.** Bei verdorbenem Material `CopyCommand`, das das CA-Verzeichnis löscht, damit es neu entsteht; sonst kein Fix.

### Bereich llm

#### LLM_001

LLM-Endpoint nicht erreichbar

**Auslöser.** Der Endpunkt antwortet nicht, löst nicht auf, die Frist läuft ab, oder sein Zertifikat stammt von einer Stelle, der dieser Rechner nicht vertraut.

**Fix.** Bei der Probe `CopyCommand` mit einem `curl`, das dasselbe versucht; bei unbekannter Stelle wie `PROXY_003`. Im Lauf `ChangeSetting` auf `llm.endpoint` mit der geltenden Adresse, ohne sie `EditSetting` auf das Feld (HUM-507).

#### LLM_002

LLM-Endpoint verlangt eine Anmeldung

**Auslöser.** Der Endpunkt verlangt eine Anmeldung (`401` oder `403`), ohne Schlüssel oder mit einem, den er ablehnt.

**Fix.** `EditSetting` auf `llm.api_key_ref` (bis HUM-507 ein `ChangeSetting` mit leerem Wert): in der Oberfläche ein Knopf, der die Einstellungen am Feld „Zugangsschlüssel“ öffnet; auf der Kommandozeile `humanitl llm key set`.

#### LLM_003

LLM-Endpoint antwortet nicht als bekannte API

**Auslöser.** Die Verbindung steht, aber die Modellliste des Endpunkts antwortet nicht als bekannte API.

**Fix.** `ChangeSetting` auf die Basis-Adresse der OpenAI-kompatiblen API.

#### LLM_004

Kein Modell konfiguriert

**Auslöser.** `llm.models` ist leer; der Agent bekommt einen Platzhalter statt eines Modells.

**Fix.** `ChangeSetting` auf `llm.endpoint` oder `CopyCommand` mit einem `curl` auf `/models` — erst fragen, dann eintragen.

#### LLM_005

Funde in einer durchgereichten Anfrage

**Auslöser.** Eine durchgereichte Anfrage an das Sprachmodell trägt Funde, vor denen in diesem Lauf am selben Endpunkt noch nicht oder vor mindestens 15 Minuten gewarnt wurde: Geheimnisse oder personenbezogene Daten. Funde, vor denen kürzlich gewarnt wurde, lösen keine neue Warnung aus, stehen aber weiter an jeder Anfrage in der Aufzeichnung.

**Fix.** Kein Fix: Die Anfrage ist bereits gesendet; der Befund macht es sichtbar.

#### LLM_006

LLM-Endpunkt liegt nicht in einem privaten Netz

**Auslöser.** Der Endpunkt liegt nicht in einem privaten Netz.

**Fix.** `ChangeSetting` auf `llm.endpoint`, wo die Probe ihn baut; der Verkehr dorthin geht an der Warteschlange vorbei.

#### LLM_007

LLM-Endpunkt ist keine lesbare HTTP-Adresse

**Auslöser.** `llm.endpoint` ist keine lesbare HTTP-Adresse.

**Fix.** `ChangeSetting` auf eine absolute `http`- oder `https`-Adresse.

#### LLM_008

Die Suche im Netz kann nicht stattfinden

**Auslöser.** Die Suche im Netz kann nicht stattfinden: keine Vorgaberoute, keine lesbare Routing-Tabelle, oder ein Netz jenseits des eigenen `/24`.

**Fix.** Kein Fix: Der Text nennt beide Netze und die Grenze.

#### LLM_009

Zugangsschlüssel nicht verfügbar

**Auslöser.** `llm.api_key_ref` ist gesetzt, aber der Schlüsselbund oder die Datei unter `$XDG_DATA_HOME/humanitl/keys` liefert keinen Schlüssel dazu.

**Fix.** `CopyCommand`: `humanitl llm key set` hinterlegt ihn neu; bei offenen Rechten der Datei ein `chmod 600`.

#### LLM_010

Inference goes to a provider outside this machine

**Auslöser.** The agent's model endpoint is a provider outside this machine, for example `api.anthropic.com`. Prompts, the files the agent reads and the output of its tools go to that provider as part of its requests to the model. Humanitl isolates the system and holds every other connection; the inference itself is deliberately allowed. The first start with a combination of agent and provider asks for a confirmation, kept by the daemon.

**Fix.** No fix needed; to keep inference on this machine or in your network, set `llm.endpoint` to a local server, or mark a saved server of your own network with `llm.servers.NAME.local = true`.

#### LLM_011

Cloud inference not confirmed

**Auslöser.** The agent of this start would send its inference to a provider outside this machine (`LLM_010`), this combination of agent and provider was never confirmed, and the start request neither confirms it (`Start.acknowledge_cloud_inference`) nor says that the notice was shown (`Start.cloud_notice_shown`). Nothing started.

**Fix.** `CopyCommand` with `humanitl run --yes`, which confirms and starts; in the app the start asks once. A client of its own shows the notice and sets one of the two fields.

#### LLM_012

No model server set

**Auslöser.** The agent sends its inference to the model server Humanitl sets (`agents.<id>.inference = "model-server"`), and none is set: `llm.endpoint` is empty and no saved server is chosen. The agent would start and fail at every prompt. Nothing started.

**Fix.** `ChangeSetting` on `llm.endpoint` with the address of a model server (or `humanitl llm server add NAME URL`); for an agent whose adapter knows its own providers (OpenCode) the sentence also names `agents.<id>.inference = "own"`, which sends the inference to those providers behind the cloud inference notice (`LLM_010`).

#### LLM_013

Agent has no own-provider mode

**Auslöser.** `agents.<id>.inference` is `own`, and Humanitl knows no own providers for this agent yet: it cannot name the hosts and paths a passthrough would need, so it would hold every request to the model. Nothing started.

**Fix.** `ChangeSetting` on `agents.<id>.inference` with `model-server`, and a model server in `llm.endpoint`.

### Bereich rules

#### RULES_001

Regel-Datei ungültig

**Auslöser.** Die Regeldatei ist unlesbar, kein gültiges YAML, oder eine Regel überlebt den Umlauf nicht.

**Fix.** Kein Fix: Der Text nennt die Stelle.

#### RULES_002

Host-Muster verdächtig (xn--, IP in Host-Glob)

**Auslöser.** Ein Host-Muster sieht verdächtig aus: eine IP im Glob oder ein `xn--`-Label.

**Fix.** Kein Fix: Der Text nennt die Schreibweise, die wirklich passt.

#### RULES_003

Host-Muster ungültig

**Auslöser.** Ein Host-Muster ist ungültig und passt auf nichts.

**Fix.** Kein Fix: Der Text nennt den Fehler im Muster.

#### RULES_005

Pfadmuster ungültig

**Auslöser.** Ein Pfadmuster ist ungültig, etwa ohne führenden Schrägstrich.

**Fix.** Kein Fix: Der Text nennt die Form, die gilt.

#### RULES_006

Version der Regel-Datei unbekannt

**Auslöser.** Die Regeldatei nennt eine Version, die es nicht gibt.

**Fix.** Kein Fix: Es gibt genau eine Version.

#### RULES_007

Doppelte Regel-Id

**Auslöser.** Zwei Regeln tragen dieselbe Id.

**Fix.** Kein Fix: Jede Regel braucht ihre eigene.

#### RULES_008

Regel wirkt zu breit

**Auslöser.** Eine Regel wirkt so breit, dass sie mehr erlaubt, als der Mensch vermutlich meint.

**Fix.** Kein Fix: Der Text nennt, worauf sie zusätzlich passt.

#### RULES_009

Regel-Datei nicht schreibbar

**Auslöser.** Die Regeldatei lässt sich nicht schreiben; der Satz auf der Platte bleibt unverändert.

**Fix.** Kein Fix: Der Text nennt Pfad und Grund.

#### RULES_010

Mitgelieferte Regel ist unveränderlich

**Auslöser.** Eine mitgelieferte Regel soll geändert oder gelöscht werden.

**Fix.** Kein Fix: Mitgelieferte Regeln werden deaktiviert, nicht entfernt.

#### RULES_011

Regelsatz neu geladen

**Auslöser.** Der Regelsatz wurde neu geladen; die Zahl der Regeln steht im Text.

**Fix.** Kein Fix: Es ist eine Meldung, kein Fehler.

#### RULES_012

Probelauf konnte die Aufzeichnung nicht lesen

**Auslöser.** Der Probelauf fand keine Aufzeichnung, gegen die er die Regel prüfen könnte.

**Fix.** Kein Fix: Ohne Verkehr gibt es nichts zu prüfen.

### Bereich findings

#### FINDINGS_001

Detektor-Regeln unbrauchbar

**Auslöser.** Das eingebaute Regelwerk der Detektoren ist unbrauchbar; das ist ein Fehler im Bau.

**Fix.** Kein Fix: Der Text nennt die Datei und den Parserfehler.

#### FINDINGS_002

Scan unvollständig

**Auslöser.** Der Scan war unvollständig: Der Rumpf war größer als die Vorschau-Grenze oder das Entpacken lief gegen die Verhältnis-Grenze.

**Fix.** `ChangeSetting` auf `limits.preview_cap_bytes` beziehungsweise `limits.max_decompress_ratio`.

#### FINDINGS_003

Fund in einer Notiz

**Auslöser.** In der Notiz an den Agenten steckt ein möglicher Geheimniswert; sie geht trotzdem hinaus.

**Fix.** Kein Fix: Die Entscheidung steht. Wer den Wert nicht senden will, blockt erneut mit einer anderen Notiz.

### Bereich catalog

#### CATALOG_001

Domain-Katalog nicht lesbar

**Auslöser.** Der mitgelieferte Domain-Katalog ist nicht lesbar.

**Fix.** Kein Fix: Ohne Katalog fehlen nur die Namen, nicht die Entscheidung.

#### CATALOG_002

Rangliste nicht lesbar

**Auslöser.** Die Rangliste der Domains ist nicht lesbar.

**Fix.** Kein Fix: Die Sortierung fällt auf die Reihenfolge der Funde zurück.

### Bereich terminal

#### TERM_001

Zweiter schreibender Terminal-Client abgelehnt

**Auslöser.** Ein zweiter schreibender Terminal-Client meldet sich an derselben Sitzung an.

**Fix.** `CopyCommand` mit `humanitl sandbox attach --read-only`: zusehen geht, schreiben nicht.

#### TERM_002

Terminal der Sandbox nicht erreichbar

**Auslöser.** Das Terminal der Sandbox ist nicht erreichbar; der Lauf hat keines oder es ist schon zu.

**Fix.** Kein Fix: Der Text nennt die Sitzung.

#### TERM_003

Terminal unsuitable for a full-screen view

**Auslöser.** A full-screen view of `humanitl` needs a terminal that can place the cursor and has at least the minimum size the view names. This terminal is smaller, or `TERM` is unset or `dumb`. Nothing was changed on the terminal; the sentence names the measured and the needed size and the command that does the same without a full-screen view.

**Fix.** `CopyCommand` with the command that needs no full-screen view; otherwise enlarge the window and run the command again.

### Bereich recorder

#### RECORDER_001

Aufzeichnung nicht verfügbar

**Auslöser.** Die Aufzeichnung ist nicht verfügbar: Datenbank fehlt, ist gesperrt oder unlesbar — oder dieser Daemon läuft ohne Aufzeichnung.

**Fix.** Wo es einen gibt, nennt der Fix den Pfad der Datenbank.

#### RECORDER_002

Filter ungültig

**Auslöser.** Ein Filter der Historie ist ungültig.

**Fix.** Kein Fix: Der Text nennt das Feld.

#### RECORDER_003

Aufzeichnung konnte nicht schreiben

**Auslöser.** Die Aufzeichnung konnte nicht schreiben; der Fluss bleibt unvollständig.

**Fix.** Wo der Schreiber steht, `CopyCommand` zum Neustart des Daemons; sonst nennt der Text nur den Fehler.

#### RECORDER_004

Blob-Speicher nicht benutzbar

**Auslöser.** Der Blob-Speicher ist nicht benutzbar; große Rümpfe können nicht abgelegt werden.

**Fix.** `CopyCommand` mit `ls -ld` und `df -h` auf das Verzeichnis — Rechte oder Platz.

#### RECORDER_005

Aufzeichnung läuft wieder

**Auslöser.** Der Schreib-Thread hat nach einer gemeldeten Störung wieder eine Transaktion ohne Fehler geschrieben.

**Fix.** Kein Fix: Was während der Störung geschah, fehlt in der History und wird nicht nachgetragen.

### Bereich limits

#### LIMIT_001

Rumpf über der Haltegrenze

**Auslöser.** Eine Anfrage ist größer als `limits.hold_body_cap_bytes`: angekündigt über `Content-Length` oder beim Puffern bemerkt.

**Fix.** `AddRule` mit `action: ask` und `stream: true` für Host, Methode und Pfad der Anfrage; ohne wirksame Regel oder unter `hold.hard_block_checksum_secrets` (dort greift `stream: true` nie) `ChangeSetting` auf `limits.hold_body_cap_bytes`, doppelt so hoch, höchstens so hoch, wie die Prüfung annimmt; darüber kein Fix (HUM-507).

#### LIMIT_002

Warteschlange voll

**Auslöser.** Eine neue Anfrage sollte gehalten werden, während die Warteschlange schon `limits.hold_max_flows` Flüsse oder `limits.hold_max_bytes` Bytes hält, oder ein Rumpf passt beim Puffern nicht mehr in `limits.hold_max_bytes`.

**Fix.** Kein Fix-Knopf: Die Warteschlange abarbeiten. Eine höhere Grenze hilft nur, wenn die gehaltenen Anfragen wirklich gebraucht werden.

#### LIMIT_003

Entpack-Verhältnis überschritten

**Auslöser.** Ein gepackter Rumpf hätte beim Entpacken mehr als `limits.max_decompress_ratio` mal so viele Bytes ergeben, wie er gepackt groß ist.

**Fix.** Kein Fix: Ein solches Verhältnis ist das Merkmal einer Entpack-Bombe, und eine höhere Grenze wäre der falsche Rat.

#### LIMIT_004

Antwort des Ziels verstummt

**Auslöser.** Das Ziel hat nach den Antwort-Kopfzeilen länger als `limits.body_timeout_secs` kein Stück des Rumpfs mehr geschickt, oder bei einem gestreamten Upload kein Stück mehr angenommen (`limits.body_timeout_secs`) beziehungsweise nach dem ganzen Rumpf keine Kopfzeilen geschickt (`limits.header_timeout_secs`).

**Fix.** `ChangeSetting` auf die Grenze, die ablief, mit dem doppelten Wert, höchstens der Obergrenze der Prüfung; steht sie schon dort, kein Fix (HUM-507).

#### LIMIT_005

Anfrage-Kopf zu spät

**Auslöser.** Ein Client hat eine Verbindung geöffnet und den Kopf seiner Anfrage nicht innerhalb von `limits.header_timeout_secs` geschickt.

**Fix.** Kein Fix-Knopf: `limits.header_timeout_secs` nur erhöhen, wenn ein Werkzeug auf einer langsamen Leitung wirklich so lange braucht.

#### LIMIT_006

Ereignis-Puffer übergelaufen

**Auslöser.** Ein Abonnent des Ereignisstroms hat mehr als `limits.event_buffer` Ereignisse nicht abgeholt.

**Fix.** Kein Fix: Die Oberfläche lädt über `ListFlows` nach.

### Bereich audit

#### AUDIT_001

Hash-Kette gebrochen

**Auslöser.** Die Prüfung von `audit.jsonl` findet einen Record, der nicht zu Vorgänger, Hash, MAC oder Anker passt, oder die Datei endet vor einem Anker; beim Start ist es der letzte Record, an den der Schreiber anhängen soll. Beim Export steht im Log eine Zeile, die kein Record ist, auch eine leere; der Export bricht dann ab und schreibt nichts. Eine Kette, deren Records nicht zusammenpassen, exportiert er dagegen unverändert, den Bruch meldet erst die Prüfung.

**Fix.** `CopyCommand`, der die Datei samt Zeitstempel beiseitelegt; sie bleibt als Beleg liegen, und die Kette beginnt neu. Beim Export ein `CopyCommand` mit `humanitl audit verify --file` auf das Log.

#### AUDIT_002

Unvollständige letzte Zeile beiseitegelegt

**Auslöser.** Beim Start endet `audit.jsonl` nicht mit einem Zeilenumbruch; der Rest hinter dem letzten vollständigen Record wurde in eine eigene Datei verschoben.

**Fix.** Kein Fix nötig: Der Text nennt die Datei mit dem Rest, und die Kette läuft weiter.

#### AUDIT_003

Audit-Daten nicht kanonisch

**Auslöser.** Die Daten eines Records enthielten eine Zahl, die keine Ganzzahl ist; geschrieben wurde der Record mit einem Platzhalter statt der Daten.

**Fix.** Kein Fix für Nutzer: ein Programmfehler; der Text nennt die Art des Records.

#### AUDIT_004

Audit-Log von einem anderen Daemon belegt

**Auslöser.** Beim Start hält ein anderer Prozess die exklusive Sperre auf `audit.jsonl`.

**Fix.** `CopyCommand` mit `humanitl daemon status`: den laufenden Daemon finden und beenden.

#### AUDIT_005

Audit-Schlüssel unbrauchbar

**Auslöser.** Die Schlüsseldatei ist ein Symlink oder keine reguläre Datei, trägt Rechte für Gruppe oder Andere, gehört einem anderen Nutzer, hat nicht genau 32 Bytes oder lässt sich nicht anlegen.

**Fix.** `CopyCommand`: `rm` für einen verbrannten Schlüssel, sonst `ls -ln` auf die Datei oder `mkdir`/`chmod` auf das Verzeichnis.

#### AUDIT_006

Audit-Log nicht schreibbar

**Auslöser.** Datei, Verzeichnis oder Anker-Tabelle des Audit-Logs lassen sich nicht öffnen, lesen, schreiben oder auf die Platte bringen.

**Fix.** `CopyCommand` mit `ls -ld` und `df -h` auf das Verzeichnis — Rechte oder Platz.

#### AUDIT_007

Audit-Kette hinter dem letzten Anker fortgesetzt

**Auslöser.** Beim Start endet `audit.jsonl` vor einem Anker aus `audit_anchors`; die Kette läuft hinter diesem Anker weiter, und die Prüfung meldet die Lücke weiter als Bruch.

**Fix.** Kein Fix nötig: Der Text nennt, wo das Log endete und hinter welchem Anker die Kette weiterläuft.

#### AUDIT_008

Audit-Export nicht schreibbar

**Auslöser.** Die Zieldatei von `humanitl audit export` liegt schon da, ihr Verzeichnis fehlt, oder das Schreiben scheitert.

**Fix.** `CopyCommand`, das die vorhandene Datei beiseitelegt; sonst nennt der Text den Pfad und den Grund.

#### AUDIT_009

Audit-Anfrage ungültig

**Auslöser.** Eine `Audit`-Anfrage nennt keine Operation, ein unbekanntes Exportformat, einen relativen Zielpfad, eine Host-Schwärzung oder einen unlesbaren Zeitpunkt oder Cursor.

**Fix.** `CopyCommand` mit `humanitl audit --help`; der Text nennt das Feld, das nicht stimmt.

### Bereich doctor

#### DOCTOR_001

bubblewrap fehlt oder ist zu alt

**Auslöser.** Die Prüfung dieses Rechners fand kein `bubblewrap` oder ein zu altes.

**Fix.** `CopyCommand` mit dem Installationsbefehl.

#### DOCTOR_002

Nutzer-Namensräume nicht verfügbar

**Auslöser.** Unprivilegierte Namensräume stehen auf diesem Rechner nicht zur Verfügung.

**Fix.** `CopyCommand` mit dem `sysctl`-Aufruf.

#### DOCTOR_003

Kernel ohne brauchbares seccomp

**Auslöser.** Der Kernel hat kein brauchbares seccomp; die dritte Garantie wäre nicht zu halten.

**Fix.** `OpenUrl` auf die Dokumentation zu seccomp — ohne den Kernel hilft keine Einstellung.

#### DOCTOR_004

Laufzeitverzeichnis fehlt oder ist nicht privat

**Auslöser.** Das Laufzeitverzeichnis fehlt oder ist nicht privat.

**Fix.** `SetEnv` oder der Hinweis auf die Rechte.

#### DOCTOR_005

Keine systemd-Nutzersitzung

**Auslöser.** Es gibt keine systemd-Nutzersitzung, in der der Daemon laufen könnte.

**Fix.** `CopyCommand` je nach Lage: `humanitld` von Hand, `systemctl --user --failed` oder `loginctl enable-linger`.

#### DOCTOR_006

Daemon nicht erreichbar oder anderer Vertrag

**Auslöser.** Der Daemon antwortet nicht oder spricht einen anderen Vertrag.

**Fix.** `CopyCommand` mit dem Startbefehl.

#### DOCTOR_007

Agent-Kommando nicht im PATH

**Auslöser.** Das Agent-Kommando liegt weder im `PATH` des Daemons noch in `~/.opencode/bin`, `~/.local/bin` oder `~/bin` (HUM-278).

**Fix.** `CopyCommand` mit `humanitl config set agent.command`, der Pfad ist ein Platzhalter, und der Installationsbefehl steht im Text; bei einem Adapter, dessen Programm ein Skript ist (pi, HUM-274), der Installationsbefehl des Adapters selbst, weil der Daemon ein Skript nie einbindet.

#### DOCTOR_008

Sprachmodell nicht erreichbar

**Auslöser.** Das Sprachmodell war nicht erreichbar, als jemand danach gefragt hat — oder es ist gar keines eingetragen.

**Fix.** `CopyCommand` mit dem `curl` auf den Endpunkt; ohne Eintrag `ChangeSetting` auf `llm.endpoint`.

#### DOCTOR_009

Kein Platz für das Anzeigesymbol

**Auslöser.** Die Arbeitsumgebung bietet keinen Platz für das Anzeigesymbol.

**Fix.** `CopyCommand` für das fehlende Portal oder die Erweiterung; ohne sie bleibt das Fenster der Weg.

#### DOCTOR_010

Renderer und Grafiktreiber vertragen sich nicht

**Auslöser.** Renderer und Grafiktreiber vertragen sich nicht; das Fenster bliebe schwarz.

**Fix.** `OpenUrl` auf die Dokumentation zu Impeller und dem Software-Renderer.

#### DOCTOR_011

Wenig Platz im Datenverzeichnis

**Auslöser.** Im Datenverzeichnis ist wenig Platz; Aufzeichnung und Blobs wachsen dort.

**Fix.** `ChangeSetting` auf eine kürzere `recorder.retention_days` — oder Platz schaffen.

#### DOCTOR_012

Prüfung nicht durchführbar

**Auslöser.** Eine Prüfung ließ sich auf diesem Rechner nicht durchführen; niemand hat sie bestanden oder verfehlt.

**Fix.** Der Fix nennt den Befehl, der sie ausführen würde.

#### DOCTOR_013

Sprachmodell nicht angesprochen

**Auslöser.** Das Sprachmodell wurde nicht angesprochen, weil niemand darum gebeten hat.

**Fix.** `CopyCommand` mit `humanitl doctor --probe-llm`.

#### DOCTOR_014

Probe start of the sandbox backend failed

**Auslöser.** `humanitl doctor --probe` started an empty sandbox with the shim and `true` under an admitted backend, and either the start failed or one of the three guarantees from the shim's five `CHECK` lines did not hold. A real session under this backend would be refused the same way.

**Fix.** The fix of the inner finding when it has one (an install command, a setting); otherwise `OpenUrl` on this page.

#### DOCTOR_015

Probe start of the kernel level failed

**Auslöser.** `humanitl doctor --probe` started an empty session of the kernel level with the shim as init of the guest and `true` as the command, and either the micro VM did not start, it ended without the shim's report, or one of the three guarantees did not hold, from the guest's `CHECK` lines or from the host check of the VM monitor. A real session under this level would be refused the same way.

**Fix.** The fix of the inner finding when it has one (a package, a setting of the kernel level); otherwise `OpenUrl` on this page.

### Bereich cli

#### CLI_001

Aufruf am Daemon abgelehnt

**Auslöser.** Ein Aufruf ist am Daemon gescheitert, oder eine Ausgabe ließ sich nicht schreiben.

**Fix.** Kein Fix: Der Text nennt den Aufruf und den Grund.

#### CLI_002

`--ask terminal` ist hier nicht möglich

**Auslöser.** `--ask terminal` ist für diesen Lauf nicht möglich, etwa bei einem Vollbild-Agenten.

**Fix.** Kein Fix: Der Text nennt die beiden Modi, die gehen.

#### CLI_003

Unterkommando noch nicht verfügbar

**Auslöser.** Ein Unterkommando gibt es noch nicht; das Issue dazu steht im Text.

**Fix.** `OpenUrl` auf das Issue.

#### CLI_004

Aufruf ungültig

**Auslöser.** Der Aufruf ist ungültig: unbekanntes Unterkommando, fehlendes Argument, widersprüchliche Schalter.

**Fix.** `CopyCommand` mit der Form, die gilt.

#### CLI_006

Die Laufzeit liess sich nicht starten

**Auslöser.** Die Kommandozeile konnte ihre Laufzeit nicht bauen; der Grund des Betriebssystems steht im Text.

**Fix.** Kein Fix: Der Text nennt, was das Betriebssystem gemeldet hat.

#### CLI_005

Es läuft schon eine Sitzung

**Auslöser.** In diesem Daemon läuft schon eine Sitzung.

**Fix.** `CopyCommand` `humanitl go --dir ORDNER` mit dem Ordner der laufenden Sitzung, sonst `humanitl sandbox attach`: Beide hängen sich an sie an. Beendet wird sie, wo sie gestartet wurde.

#### CLI_007

Desktop app not installed or not starting

**Auslöser.** `humanitl app` hands the folder to a desktop app that is connected to the daemon, and otherwise starts the app: `humanitl-app` next to the own program or on `PATH`, on macOS `open -a Humanitl`. No app was connected, and the app was not found there or did not start; the text names where it looked and what the system said.

**Fix.** `OpenUrl` on the installation guide; the desktop app comes with the package `humanitl` (Linux) or the `Humanitl.app` from the release (macOS).

### Bereich ui

#### UI_002

Kein Platz für das Anzeigesymbol

**Auslöser.** Die Arbeitsumgebung bietet keinen Platz für das Anzeigesymbol im Systembereich, oder macOS erlaubt der App keine Mitteilungen.

**Fix.** `OpenUrl` zur AppIndicator-Erweiterung, unter macOS `CopyCommand` zu den Mitteilungs-Einstellungen. Das Fenster bleibt der Weg zur Warteschlange.

#### UI_003

Ordnerdialog nicht verfügbar

**Auslöser.** Die Arbeitsumgebung konnte den Dialog zur Wahl des Projektordners nicht öffnen; der Grund des Portals steht im Text.

**Fix.** `CopyCommand` mit `humanitl run`, im Projektordner auszuführen.

### Bereich agent

#### AGENT_001

Agent-Kommando nicht gefunden

**Auslöser.** Das Agent-Kommando ist weder im `PATH`, in `~/.opencode/bin`, `~/.local/bin`, `~/bin` noch über `agent.command` gesetzt (HUM-278).

**Fix.** `CopyCommand` mit `humanitl config set agent.command`, der Pfad ist ein Platzhalter, und der Installationsbefehl steht im Text; bei einem Adapter, dessen Programm ein Skript ist (pi, HUM-274), der Installationsbefehl des Adapters selbst, weil der Daemon ein Skript nie einbindet.

#### AGENT_002

Agent-Kommando nicht ausführbar

**Auslöser.** `agent.command` zeigt auf etwas, das auf diesem Rechner nicht ausführbar ist.

**Fix.** `ChangeSetting` zurück auf das Standardkommando des Adapters.

#### AGENT_003

Gebündelte Agenten-Vorlage unbrauchbar

**Auslöser.** Eine mitgelieferte Vorlage des Adapters ist unbrauchbar; das ist ein Fehler im Bau.

**Fix.** Kein Fix: Der Text nennt die Datei.

#### AGENT_004

Agent-Kommando in der Sandbox nicht erreichbar

**Auslöser.** Gesucht wird zuerst in der Sandbox: ihr Suchpfad (`sandbox.env` vor `[env]` des Profils), ihre Einhängungen und die Verweise des Profils, relative Einträge gegen `[mounts].work.dst`. Erst wenn dort nichts liegt, gilt der Host, und der Befund nennt beides. Zwei Fälle: das Kommando liegt unter keiner Einhängung, oder es liegt unter einer, deren Verzeichnis der Suchpfad nicht nennt. Was sich nicht entscheiden lässt, erzeugt keinen Befund (HUM-139).

**Fix.** `CopyCommand`, das es an eine eingehängte Stelle installiert; steht die Einhängung und fehlt nur der Suchpfad, kein Fix, der Text nennt die Zeile `PATH` für `[sandbox.env]` in `config.toml`, denn `humanitl config set` schreibt keinen Eintrag dieser Tabelle (HUM-507).

#### AGENT_005

Agent startete nicht

**Auslöser.** Der Shim meldet auf dem Berichtskanal `EXEC fail`, oder das Kommando endet ohne dieses Signal mit Exit-Code `127` oder `126`, ohne ein einziges Byte geschrieben zu haben. Der Befund nennt das Kommando und den `PATH` der Sandbox; den `PATH` hält er zurück, wenn ein Mensch ihn geschrieben hat (`sandbox.env` oder ein eigenes Profil), wie die Umgebungstabelle.

**Fix.** `CopyCommand` mit `humanitl sandbox run -- /bin/sh -c 'command -v …'`: fragt dieselbe Sandbox, wo das Kommando liegt. Was eingehängt wird, entscheidet das Profil.

#### AGENT_006

Node.js missing in the sandbox

**Auslöser.** The command resolves to a script that starts Node.js through its `#!` line, and the sandbox finds no executable `node` on its `PATH` (or at the absolute path the line names). The finding names the script, the sandbox `PATH` and where a `node` lies on the host, if anywhere.

**Fix.** No fix when a mounted `node` is only missing from the path: the sentence names the `PATH` line for `[sandbox.env]` in `config.toml`, since `humanitl config set` writes no entry of that table (HUM-507); `CopyCommand` that installs the host's `node` under `/usr/local/bin` when it is not mounted, the installer of pi when the `#!` line names an absolute `node` outside the mounts, otherwise `OpenUrl` to the Node.js download page (pi needs 22.19 or newer).

#### AGENT_007

pi web extension not installed

**Auslöser.** `agent.pi.web_access` is `on`, and no directory the sandbox reaches holds the package `pi-web-access`. pi starts without `web_search` and `fetch_content`.

**Fix.** `CopyCommand` with `sudo npm install -g --ignore-scripts pi-web-access`, which puts the package under `/usr/local`, where the sandbox sees it read-only.

#### AGENT_008

Agent not installed in the depot

**Auslöser.** The source of this agent is the depot, and the depot holds no installed version of it: there is no `current` link, or the version it names has no directory or no `manifest.json`. A session never falls back to a program on the host.

**Fix.** `CopyCommand` with `humanitl agents install <id>` (HUM-519); or, without a company lock, `CopyCommand` with `humanitl agents source <id> host`.

#### AGENT_009

Agent in the depot not logged in

**Auslöser.** The depot of this agent holds an installed version and no stored login. The agent starts and asks for a login itself; a login made in a session goes back into the depot only through the guard of the write-back, and only where the adapter declares the format of its credential file. `humanitl agents login <id>` logs in once for every session.

**Fix.** `CopyCommand` with `humanitl agents login <id>` (HUM-520).

#### AGENT_010

Agent source fixed by a company policy

**Auslöser.** A company policy fixes the source of this agent to the depot. Starting it from the host and setting its source back to `host` are refused, on every way: a start, `humanitl agents source`, `humanitl config set`, the settings of the app and `ChangeSetting`. A policy may only name the depot; one that names the host is refused itself. The same code refuses a write of any other setting a company policy fixes to one value, through `SetConfig` or `humanitl config set`; such a setting is read only in every client, and the sentence names the key and the policy.

**Fix.** None. The policy decides; ask whoever manages it.

#### AGENT_013

Agent depot unusable

**Auslöser.** The daemon checks the depot each time it opens it: owner (this user or root), no write permission for other users, no symbolic link out of the depot, in a version directory only regular files, directories and relative links inside it, a readable `manifest.json`, and at most 64 MiB of configuration and credentials to copy into a session. The sentence names the first path that failed.

**Fix.** `CopyCommand` with `humanitl agents remove <id>` and a new install, or with `chmod go-w` on the named path when only its mode is wrong.

#### AGENT_015

Adapter or backend without depot support

**Auslöser.** The source of this agent is the depot, and either its adapter declares no depot recipe yet, or the chosen sandbox backend cannot bind the depot into a session. The setting is valid and kept; loading the configuration says so as a notice, and a start stops here.

**Fix.** `CopyCommand` with `humanitl agents source <id> host` when no company policy fixes the source; or `ChangeSetting` on `sandbox.backend` = `auto` when the backend is the reason.

#### AGENT_017

Command names a host program of a depot agent

**Auslöser.** The source of this agent is the depot, or a company policy fixes it there, and the command of the session names a program of the host: a path on the host, a wrapper or a symbolic link whose target an adapter recognises as its agent, or a bind of such a program. A command may add arguments and may name the depot program by its bare name (`claude --model x`); nothing else of that agent starts.

**Fix.** `ChangeSetting` on `agent.command` with the bare program name, or `CopyCommand` with `humanitl run -- <name>`; without a company policy also `CopyCommand` with `humanitl agents source <id> host`.

#### AGENT_018

Agent depot in use

**Auslöser.** The running session took this agent from the depot; its program directory and its configuration are bound into the sandbox. The depot is removed only when no session uses it.

**Fix.** `CopyCommand` with `humanitl sandbox stop`, then remove again.

#### AGENT_011

Agent install or update failed

**Auslöser.** Install and update of a depot agent run in a sandbox, and the daemon takes the result over only when every check passes: the version is the one asked for and only an update changes an installed one; where the vendor signs, the daemon itself verified its own copy of the vendor's manifest with the host's `gpg` and the key the adapter fixes, and the program's SHA-256 matches that manifest (the run only downloads, and no verdict of it counts); the program tree holds only regular files, directories and relative links inside it, no socket, FIFO, device, hard link, set-ID bit or name that is not UTF-8, within its caps. A missing `gpg` is a failure, never an unsigned install. The sentence names the step that failed; the depot is as it was before.

**Fix.** `CopyCommand` with `humanitl agents install <id>` to run again after a failed download; `humanitl agents update <id>` when a version is installed already; install `gnupg` on the host when `gpg` is missing. A failed signature, checksum or tree check has no fix to go around it.

#### AGENT_012

Credential write-back refused

**Auslöser.** Only one file goes from a session back into the depot, the credential file the adapter names, and only checked: opened without following a link, a regular file of at most 64 KiB, a JSON object with only the keys of the adapter's format, every value of its type and form, every required key present; after the first login also exactly the keys of the stored file. This file failed a check, and the stored login stays as it was. Nothing else a session writes ever reaches the depot.

**Fix.** `CopyCommand` with `humanitl agents login <id>` when the stored login no longer works; otherwise none, the session keeps its own copy until it ends.

#### AGENT_014

Agent login failed

**Auslöser.** The login of a depot agent runs in a sandbox, and only the credential file it leaves, checked by the guard of the write-back, reaches the depot. The login failed, left no such file, or the adapter declares no login run or credential format yet, so nothing would be taken back. A callback URL pasted from the browser is delivered inside the run only when it is exactly the agent's callback on its own loopback, and only on a backend whose sandbox has a loopback of its own; under Seatbelt on macOS only the device flow works.

**Fix.** `CopyCommand` with `humanitl agents login <id>` to try again; where the device flow is off at the vendor, the sentence names the setting that turns it on.

#### AGENT_016

Credential of another account or older

**Auslöser.** Two sessions can renew the same login, and some vendors swap the refresh token on each renewal. The daemon never writes an older token over a newer one: the file of this session expires earlier than the stored one, or, where the format carries no expiry, another session stored a file after this session started. Or the file names another account than the one logged in; only a login run changes the account. The stored login stays as it was.

**Fix.** `CopyCommand` with `humanitl agents login <id>` when the agent asks for a login again.

#### AGENT_019

Configuration run not taken over

**Auslöser.** The configuration of a depot agent changes only through a configuration run that the person confirms: the agent runs on a writable copy without a project, the daemon lists every changed file and highlights the keys that run commands, and only a confirmation replaces `config/` of the depot, in one step. This run, or its confirmation, did not get that far, and `config/` is as it was.

**Fix.** `CopyCommand` with `humanitl agents config <id>` to run it again.

#### AGENT_020

Credential encryption not possible

**Auslöser.** With agents.encrypt_credentials = true the credential files of the depot lie encrypted with ChaCha20-Poly1305, and the key lies only in the system keyring (Secret Service on Linux, the login Keychain on macOS), never in a file next to the data. Either the keyring did not answer, so the setting was refused, or an encrypted file did not open with the key from the keyring: another key, a damaged file or a format this daemon does not know. Nothing was changed, and no file was written in plain text instead.

**Fix.** `CopyCommand` with the package of `secret-tool` when it is missing, or `humanitl agents login <id>` when a file does not open with the stored key; otherwise none, unlock the keyring and try again.

### Bereich edit

#### EDIT_001

Ziel der bearbeiteten Anfrage weicht ab

**Auslöser.** Host, Port oder Schema der bearbeiteten Anfrage weichen nach der Normalisierung von der gehaltenen ab.

**Fix.** Kein Fix: Wer ein anderes Ziel will, stellt eine neue Anfrage; über diese hier hat niemand für dieses Ziel entschieden.

#### EDIT_002

Methode der bearbeiteten Anfrage ungültig

**Auslöser.** Die Methode ist leer, länger als 16 Zeichen oder enthält etwas anderes als Großbuchstaben.

**Fix.** Kein Fix-Knopf: Der Text nennt die abgelehnte Methode; die Oberfläche schreibt sie in Großbuchstaben.

#### EDIT_003

Pfad der bearbeiteten Anfrage ungültig

**Auslöser.** Der Pfad beginnt nicht mit `/`, oder er enthält ein Leerzeichen oder ein Steuerzeichen.

**Fix.** Kein Fix-Knopf: Der Text nennt die Stelle; Query-Werte gehören URL-kodiert.

#### EDIT_005

Bearbeiteter Body über der Grenze

**Auslöser.** Der Body der bearbeiteten Anfrage ist größer als die Grenze, gegen die der Hold gepuffert hat (`limits.hold_body_cap_bytes`).

**Fix.** `ChangeSetting` auf `limits.hold_body_cap_bytes`, oder weniger schicken.

### Bereich hold

#### HOLD_004

Bestätigtes Geheimnis gesperrt

**Auslöser.** Die Anfrage oder ihre bearbeitete Fassung trägt einen prüfsummen-bestätigten Fund der Art IBAN, Kreditkarte, API-Schlüssel oder JWT, und `hold.hard_block_checksum_secrets` ist an.

**Fix.** `ChangeSetting` auf `hold.hard_block_checksum_secrets = false`; sonst geht die Anfrage nur ohne den Wert hinaus, im Editor ersetzt oder vom Agenten neu gestellt.

### Bereich update

#### UPDATE_001

Update check failed

**Auslöser.** Asking `api.github.com` for the latest release, or fetching its manifest and signature from `github.com`, failed: no answer, an error status, a redirect outside GitHub, a body over its cap, or a release without `update-macos.json` and `update-macos.json.sig`.

**Fix.** `OpenUrl` on the releases page, where the newest version can be downloaded by hand; the check repeats on its own the next day.

#### UPDATE_002

Update manifest refused

**Auslöser.** The manifest of the latest release carries no valid Ed25519 signature of the Humanitl release key, cannot be read, names a version that is not newer than this one or differs from the release tag, names a download outside `https://github.com/nurkert/Humanitl/releases/download/`, or needs a newer macOS than this Mac runs.

**Fix.** No automatic fix: nothing is installed. `OpenUrl` on the releases page; a refused signature is worth a report, because only a release built by the project's pipeline carries one.

#### UPDATE_003

Downloaded update refused

**Auslöser.** The zip that was downloaded is larger or smaller than the manifest says, its SHA-256 differs, it does not unpack to `Humanitl/Humanitl.app`, the app fails `codesign --verify --deep --strict`, lacks the entitlement `com.apple.security.files.user-selected.read-only`, or its version is not the one the manifest names.

**Fix.** No automatic fix: the download is deleted and the installed app stays as it was. Try again later, or `OpenUrl` on the releases page for the manual download.

#### UPDATE_004

Update cannot be installed here

**Auslöser.** `Apply` was asked on Linux, where the package manager installs updates, or the daemon does not run from a `Humanitl.app`, runs from a disk image under `/Volumes` or from App Translocation, or may not write the folder that holds the app.

**Fix.** Under Linux `CopyCommand` with the upgrade command of the package manager (apt, dnf, AUR); on macOS `OpenUrl` on the releases page, or move Humanitl.app to /Applications first.

#### UPDATE_005

Update could not be installed

**Auslöser.** Unpacking the zip or moving the new app into place failed after all checks passed, for example on a full disk or a folder that changed meanwhile. The previous app was restored under its old name; if even that failed, the sentence names where it lies.

**Fix.** No automatic fix; the sentence names the step and the path. Try again, or `OpenUrl` on the releases page for the manual download.

