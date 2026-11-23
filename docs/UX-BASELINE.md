# UX-Ist-Aufnahme

Applies to: Humanitl 0.1

Dieses Dokument hält fest, wie sich Humanitl am 2026-11-23 bedienen lässt, nach Release 0.6.0
und vor dem ersten Code von Sprint 9 (GitHub #78). [`UX.md`](UX.md) sagt, wie die Oberfläche
sein soll; diese Seite sagt, was da ist. Sprint 9 baut die Bedienung unter dem Leitsatz „alles
kann, nichts muss" um, und sein Plan entstand aus Befragungen, die den Ist-Stand nicht kannten.
Jedes spätere Issue des Sprints wird deshalb gegen diese Seite abgeglichen: Was hier als
vorhanden steht, wird erreichbar gemacht und nicht neu gebaut.

Die Seite beschreibt einen Stand und veraltet mit jedem Issue, das eine Oberfläche ändert.
Verbindlich bleiben die erzeugten Referenzen und der Code.

## Inhalt

1. [Woher die Angaben kommen](#1-woher-die-angaben-kommen)
2. [Inventar der Einstellungen](#2-inventar-der-einstellungen)
3. [Inventar der Befehle und Bildschirme](#3-inventar-der-befehle-und-bildschirme)
4. [Die zwölf Abläufe, wie sie heute sind](#4-die-zwölf-abläufe-wie-sie-heute-sind)
5. [Vorgaben gegen die Grenze „ab Werk abgeschirmt"](#5-vorgaben-gegen-die-grenze-ab-werk-abgeschirmt)
6. [Lückenliste je Ablauf](#6-lückenliste-je-ablauf)
7. [Was der Plan von Sprint 9 anders annimmt](#7-was-der-plan-von-sprint-9-anders-annimmt)

## 1. Woher die Angaben kommen

Nichts auf dieser Seite ist geschätzt. Die Quellen, in der Reihenfolge ihres Gewichts:

| Angabe | Quelle |
|---|---|
| Schlüssel, Typ, Vorgabe, Stufe, Projekt-Profil | `daemon/crates/config/src/model.rs`, daraus erzeugt [`CONFIG.md`](CONFIG.md) |
| Zeilen der Einstellungen | `daemon/crates/config/src/catalog.rs`, `catalog/en.toml`, `catalog/hidden.toml` |
| Schlüssel der Projektwahl | `PROJECT_CHOICE_KEYS` in `daemon/crates/config/src/project_choice.rs` |
| Schlüssel einer Sitzung | `SESSION_OVERRIDE_KEYS` und die Modulbeschreibung in `daemon/crates/ipc/src/session.rs` |
| Sperren einer Firmenvorgabe | `daemon/crates/config/src/policy.rs`, `agents.rs` |
| Felder des Erststarts | `daemon/crates/ipc/src/projects/proposal/fields.rs` |
| Befehle und Flags | `daemon/bin/humanitl/src/cli.rs`, daraus erzeugt [`reference/cli.md`](reference/cli.md) |
| Terminal-Ansichten | `daemon/bin/humanitl/src/tui/`, `prefix/`, `ask_terminal.rs` |
| Bildschirme der App | `app/lib/features/`, die Texte in `app/l10n/app_en.arb` |
| Welche RPC ein Befehl und ein Bildschirm nutzt | [`reference/parity.md`](reference/parity.md) |

Die erzeugten Tabellen stehen hier nicht noch einmal. Typ und Beschreibung eines Schlüssels
liest man in [`CONFIG.md`](CONFIG.md), den Hilfetext eines Befehls in
[`reference/cli.md`](reference/cli.md). Diese Seite ergänzt, was beiden fehlt: auf welchen
Ebenen ein Schlüssel gesetzt werden kann und wo man ihn heute sieht und ändert.

Die Namen von Knöpfen und Reitern stehen so da, wie die englische Oberfläche sie zeigt
(„Start", „Clean up", „Who uses which server"). Gestartet wurde für diese Aufnahme nichts:
kein Daemon, keine Sandbox, keine App. Die Wege sind am Code nachverfolgt, nicht geklickt; der
gemeinsame Durchgang mit dem Owner am Ende des Sprints prüft sie am laufenden Programm.

## 2. Inventar der Einstellungen

Das Schema hat 95 Blattschlüssel in 17 Gruppen. [`CONFIG.md`](CONFIG.md) nennt je Schlüssel
Typ, Vorgabe, Stufe, Projekt-Profil und Wirkung.

### 2.1 Die Ebenen, wie sie heute sind

[`CONFIG.md`](CONFIG.md) zählt neun Ebenen der Auflösung. Für die Bedienung zählen sieben
Stellen, an denen ein Mensch einen Wert setzt:

| Kürzel | Ebene | Wo der Wert liegt | Für welche Schlüssel |
|---|---|---|---|
| – | Vorgabe | im Programm | alle 95 |
| – | Nutzer | `config.toml`, dazu die Profile unter `profiles/` | alle 95 |
| PP | Projekt-Profil | `<projekt>/.humanitl/profile.toml`, Block `[config]`, also im Projektordner | die 20 Schlüssel mit Projekt `allowed` |
| PW | Projektwahl | im Projektregister des Daemons, nie im Projektordner | die 13 Schlüssel aus `PROJECT_CHOICE_KEYS` |
| Si | Sitzung | nirgends; reist mit dem Start | 12 Schlüssel, siehe unten |
| – | Umgebung und Flag | `HUMANITL_*` und `--gruppe-schlüssel` | Umgebung für alle 95, Flag für 92 |
| – | Firmenvorgabe | noch keine Datei | heute keiner |

Dazu gehören fünf Feststellungen, die in keiner erzeugten Tabelle stehen:

1. **Es gibt zwei Projekt-Ebenen.** Das Projekt-Profil liegt im Repository und gilt als vom
   Angreifer beeinflusst; `humanitl config set --project` und `humanitl llm server assign
   --project` schreiben dorthin. Die Projektwahl liegt beim Daemon; der Erststart-Bildschirm,
   „Start choice…" und `humanitl settings --project` schreiben dorthin. Beide heißen in den
   Oberflächen „project". Nur zwei Schlüssel kann man auf beiden Ebenen setzen: `llm.server`
   und `sandbox.min_isolation`.
2. **Die Sitzungsebene ist schmal und nur in der Kommandozeile vollständig.** Sechs Schlüssel
   reisen als `Start.cli_overrides` (`llm.endpoint`, `hold.timeout_secs`, `sandbox.backend`,
   `sandbox.podman.runtime`, `sandbox.min_isolation`, `agent.adapter`), sechs weitere in
   eigenen Feldern des Starts (`sandbox.work_dir`, `sandbox.work_mode`, `sandbox.mode`,
   `hold.ask_mode`, `llm.server`, `agent.command`). Jedes andere Flag an `humanitl run` oder
   `humanitl go` ändert nur, was der Befehl selbst lokal auflöst, und erreicht die Sitzung im
   Daemon nicht; [`cli.md`](cli.md) sagt das unter „The flags", der Befehl selbst sagt es
   nicht. Die App schickt beim Start nur Ordnermodus und Backend.
3. **Die Umgebung gilt dem Prozess, der sie liest.** Der Daemon läuft meist als Nutzer-Unit
   mit eigener Umgebung und liest nur seine eigene `config.toml`. Aus einem Aufruf von
   `humanitl run` oder `humanitl go` reist ein Wert deshalb nur als Wunsch dieses Laufs mit,
   und nur für die Schlüssel der Sitzung: gesetzt durch ein Flag, durch eine Variable
   `HUMANITL_*` des Aufrufs oder durch eine Datei, die `--config` nennt
   (`cmd/run/wish.rs`, `per_run`). Aus der Umgebung reisen so `sandbox.backend`,
   `sandbox.podman.runtime`, `sandbox.min_isolation`, `sandbox.work_mode`, `sandbox.mode`,
   `hold.ask_mode` und `sandbox.work_dir`. Eine Variable für jeden anderen Schlüssel ändert
   nur, was der Befehl lokal auflöst.
4. **Die Firmenvorgabe ist ein Mechanismus ohne Datei.** `PolicyLocks::of_policy` und
   `SourceLock::of_policy` liefern heute immer „keine Sperre". Auflösung, Herkunft `policy`,
   die nur lesbare Zeile in App und Terminal und die Ablehnung `AGENT_010` sind gebaut und
   durch Tests belegt; setzen kann eine Sperre heute niemand.
5. **Die Stufen wirken in keiner Oberfläche.** `basic`, `advanced` und `expert` stehen im
   Schema und im Katalog (10, 50 und 35 Schlüssel). Weder die App noch `humanitl settings`
   blendet nach ihnen ein oder aus; ein „Mehr anzeigen" gibt es nicht. Ob ein Schlüssel eine
   Zeile hat, entscheidet allein der Katalog.

Drei Schlüssel sind Tabellen und haben kein Flag: `llm.servers`, `resolver.overrides` und
`sandbox.env`.

### 2.2 Die Oberflächen, an denen Einstellungen stehen

| Oberfläche | Was sie zeigt | Wohin sie schreibt |
|---|---|---|
| App, Einstellungen (Overlay mit `Ctrl+,` in der Sitzung, Reiter „Settings" im Startfenster) | die 27 Zeilen des Katalogs in zehn Kategorien, Suche, Sperre einer Firmenvorgabe; **keine Herkunft**, kein allgemeines Zurücksetzen (nur „Use the start folder" entfernt `sandbox.work_dir`), keine Projekt-Sicht | `SetConfig` in `config.toml`; Server, Schlüssel, Agenten und Proxy über ihre eigenen RPC |
| App, Erststart-Karte (neuer Ordner oder „Start choice…") | sechs Felder mit Optionen, Vorgabe, Grund einer nicht wählbaren Option, Sperre | `Projects.save_choice`, die Projektwahl |
| App, Sandbox-Seite | Ordner, Schreibmodus, Schalter „Disposable", Backend der nächsten Sitzung; Untergrenze, Einhängungen, Umgebung und Modellserver mit Herkunft nur lesbar | Ordner in `config.toml`; Modus und Backend nur in den nächsten Start |
| App, Setup | Modellserver (nur Kontakt), Ordner, Schreibmodus, Wegwerf-Modus | Ordner in `config.toml`; Modus nur in den nächsten Start |
| `humanitl settings` | 25 der 27 Zeilen: Der Katalog behält „Scale" und „Keyboard shortcuts" der App vor (`Def::app()` in `catalog.rs`, `client = app`), und `tui/settings/model.rs` (`shown`) lässt solche Zeilen weg, Suche mit `/`, **Herkunft je Zeile**, Sperre; `x` oder `Entf` entfernt nur ein hinterlegtes Geheimnis | `SetConfig`; eigene Seiten für Server, Zuordnung und Quelle der Agenten |
| `humanitl settings --project PATH` | dieselben Zeilen mit den Werten eines Starts in diesem Ordner | nur die fünf Zeilen der Projektwahl, über `Projects.save_choice`; alles andere ist dort nur lesbar |
| Erststart-Bildschirm von `humanitl go` | dieselben sechs Felder wie die Karte der App | `Projects.save_choice` |
| `humanitl config get`, `set`, `edit` | jeden der 95 Schlüssel; `--origin` nennt die Ebene | `config.toml`, mit `--project` das Projekt-Profil |
| `humanitl config catalog [--project PATH]` | den Katalog des Daemons mit Herkunft und Sperre je Zeile | nichts |
| `humanitl projects show PATH` | jeden Schlüssel der Projektwahl mit gewähltem Wert, wirksamem Wert und Herkunft | nichts |

Von den 95 Schlüsseln haben 20 eine eigene Zeile im Katalog. 14 weitere sind über ein eigenes
Bedienelement erreichbar (die Quelle und das Zurückschreiben je Agent, die Verschlüsselung des
Depots, der Standardserver, Schreibmodus und Profil im Erststart). Die übrigen 61 haben in keiner
Ansicht ein Bedienelement: Man ändert sie mit `humanitl config set` oder `humanitl config
edit`. Sichtbar sind davon nur zwei, und nur lesbar: die wirksame Untergrenze
`sandbox.min_isolation` im Reiter „Isolation" der Sandbox-Seite und `sandbox.env` im Reiter
„Environment". Den Wert der anderen zeigt keine App-Ansicht und keine Terminal-Ansicht. Sieben Zeilen des Katalogs sind keine
Schlüssel: „Agent depot", „Scale", „Keyboard shortcuts", „Connection" des Proxys, „Version",
„Restart" und „config.toml".

`humanitl config get --origin` löst im Aufruf selbst auf, ohne den Daemon. Es kennt Vorgabe,
`config.toml`, Profile, das Projekt-Profil des aktuellen Verzeichnisses, Umgebung und Flags.
Die Projektwahl und eine Sperre kennt es nicht; deren Herkunft zeigen nur `config catalog
--project`, `settings --project` und `projects show`. Auf die Vorgabe zurück führt allgemein
nur `humanitl config set KEY -`. Die App hat dafür ein einziges Bedienelement: „Use the start
folder" in den Einstellungen der Sandbox entfernt `sandbox.work_dir`.

### 2.3 Die Schlüssel

Lesehilfe: „Stufe" ist `x-tier` des Schemas. „Weitere Ebenen" nennt, was über Vorgabe, Nutzer,
Umgebung und Flag hinausgeht: PP ist das Projekt-Profil, PW die Projektwahl, Si die Sitzung mit
dem Flag, das den Wert für einen Start setzt. „nur Datei" heißt: nur `humanitl config get`,
`set` und `edit`, Flag und Umgebung. `settings` meint `humanitl settings`.

### `agent`

Welcher Agent in der Sandbox läuft.

| Schlüssel | Vorgabe | Stufe | Weitere Ebenen | Sichtbar und änderbar |
|---|---|---|---|---|
| `agent.adapter` | `"opencode"` | advanced | PW; Si (`go AGENT`, `--agent-adapter`) | App: Einstellungen (Agent), Erststart-Karte. Terminal: `settings`, `settings --project`, Erststart-Bildschirm |
| `agent.briefing.enabled` | `true` | advanced | PP | nur Datei |
| `agent.command` | `-` | expert | Si (`-- CMD`) | nur Datei |
| `agent.pi.web_access` | `"auto"` | advanced | – | nur Datei |

### `agents`

Woher die Agenten kommen, vom Host oder aus dem Depot.

| Schlüssel | Vorgabe | Stufe | Weitere Ebenen | Sichtbar und änderbar |
|---|---|---|---|---|
| `agents.agy.credential_writeback` | `true` | expert | – | App: Einstellungen (Agents), Schalter je Agent |
| `agents.agy.source` | `-` | advanced | PW | App: Einstellungen (Agents), Erststart-Karte (Quelle). Terminal: `settings` (Seite der Agenten), Erststart-Bildschirm, `agents source`, `agents status` |
| `agents.claude.credential_writeback` | `true` | expert | – | App: Einstellungen (Agents), Schalter je Agent |
| `agents.claude.source` | `-` | advanced | PW | App: Einstellungen (Agents), Erststart-Karte (Quelle). Terminal: `settings` (Seite der Agenten), Erststart-Bildschirm, `agents source`, `agents status` |
| `agents.codex.credential_writeback` | `true` | expert | – | App: Einstellungen (Agents), Schalter je Agent |
| `agents.codex.source` | `-` | advanced | PW | App: Einstellungen (Agents), Erststart-Karte (Quelle). Terminal: `settings` (Seite der Agenten), Erststart-Bildschirm, `agents source`, `agents status` |
| `agents.encrypt_credentials` | `false` | advanced | – | App: Einstellungen (Agents), Schalter. Terminal: `agents status` zeigt den Stand |
| `agents.opencode.credential_writeback` | `true` | expert | – | App: Einstellungen (Agents), Schalter je Agent |
| `agents.opencode.source` | `-` | advanced | PW | App: Einstellungen (Agents), Erststart-Karte (Quelle). Terminal: `settings` (Seite der Agenten), Erststart-Bildschirm, `agents source`, `agents status` |
| `agents.pi.credential_writeback` | `true` | expert | – | App: Einstellungen (Agents), Schalter je Agent |
| `agents.pi.source` | `-` | advanced | PW | App: Einstellungen (Agents), Erststart-Karte (Quelle). Terminal: `settings` (Seite der Agenten), Erststart-Bildschirm, `agents source`, `agents status` |
| `agents.source` | `"host"` | advanced | – | nur Datei; `agents status` zeigt die Herkunft der Quelle |

### `audit`

Die Audit-Kette.

| Schlüssel | Vorgabe | Stufe | Weitere Ebenen | Sichtbar und änderbar |
|---|---|---|---|---|
| `audit.anchor_every` | `100` | advanced | – | nur Datei |
| `audit.fsync_every` | `50` | expert | – | nur Datei |
| `audit.max_bytes` | `0` | expert | – | nur Datei |
| `audit.retention_days` | `0` | expert | – | nur Datei; der Abschnitt „Retention“ im Audit der App erklärt die Frist und bietet einen Befehl zum Kopieren, den wirksamen Wert zeigt er nicht |

### `egress`

Der Entscheidungs-Proxy hinter Humanitl.

| Schlüssel | Vorgabe | Stufe | Weitere Ebenen | Sichtbar und änderbar |
|---|---|---|---|---|
| `egress.proxy` | `-` | advanced | – | App: Einstellungen (Decision proxy). Terminal: `settings`, `egress status` |
| `egress.proxy_auth_ref` | `-` | advanced | – | App: Einstellungen (Decision proxy). Terminal: `settings`, `egress auth set`, `egress auth remove`; der Wert selbst erscheint nie |
| `egress.proxy_ca` | `-` | advanced | – | App: Einstellungen (Decision proxy). Terminal: `settings` |
| `egress.proxy_resolve` | `"proxy"` | advanced | – | App: Einstellungen (Decision proxy). Terminal: `settings`, `egress status` |
| `egress.proxy_response_timeout_secs` | `300` | advanced | – | nur Datei |

### `experimental`

Schalter für Unfertiges.

| Schlüssel | Vorgabe | Stufe | Weitere Ebenen | Sichtbar und änderbar |
|---|---|---|---|---|
| `experimental.h2_upstream` | `false` | expert | – | nur Datei |

### `findings`

Die Erkennung von Secrets und personenbezogenen Daten.

| Schlüssel | Vorgabe | Stufe | Weitere Ebenen | Sichtbar und änderbar |
|---|---|---|---|---|
| `findings.email_allow_domains` | `[]` | advanced | – | nur Datei |
| `findings.enabled` | `true` | advanced | – | nur Datei |
| `findings.ignored_hashes` | `[]` | expert | – | nur Datei |
| `findings.user_terms` | `[]` | basic | PP | nur Datei; der Editor der App liest die Begriffe |

### `hold`

Das Anhalten von Anfragen.

| Schlüssel | Vorgabe | Stufe | Weitere Ebenen | Sichtbar und änderbar |
|---|---|---|---|---|
| `hold.ask_mode` | `"ui"` | advanced | PW; Si (`--ask`) | App: Einstellungen (Requests), Erststart-Karte. Terminal: `settings`, `settings --project`, Erststart-Bildschirm, `sessions` |
| `hold.hard_block_checksum_secrets` | `false` | advanced | – | nur Datei |
| `hold.timeout_secs` | `300` | basic | PP; Si (`--hold-timeout-secs`) | App: Einstellungen (Requests). Terminal: `settings` |

### `limits`

Fristen und Obergrenzen des Proxys.

| Schlüssel | Vorgabe | Stufe | Weitere Ebenen | Sichtbar und änderbar |
|---|---|---|---|---|
| `limits.body_timeout_secs` | `300` | expert | PP | nur Datei |
| `limits.connect_timeout_secs` | `10` | advanced | PP | nur Datei |
| `limits.event_buffer` | `1024` | expert | PP | nur Datei |
| `limits.header_timeout_secs` | `30` | expert | PP | nur Datei |
| `limits.hold_body_cap_bytes` | `33554432` | advanced | PP | nur Datei |
| `limits.hold_max_bytes` | `268435456` | advanced | PP | nur Datei |
| `limits.hold_max_flows` | `200` | advanced | PP | nur Datei |
| `limits.max_client_connections` | `256` | expert | PP | nur Datei |
| `limits.max_decompress_ratio` | `100` | expert | PP | nur Datei |
| `limits.preview_cap_bytes` | `8388608` | expert | PP | nur Datei |
| `limits.recorder_max_body_bytes` | `33554432` | expert | – | nur Datei |
| `limits.registry_max_finished_flows` | `10000` | expert | – | nur Datei |

### `llm`

Modellserver und Durchleitung.

| Schlüssel | Vorgabe | Stufe | Weitere Ebenen | Sichtbar und änderbar |
|---|---|---|---|---|
| `llm.active` | `-` | advanced | – | App: Einstellungen (Models, „Make default“), Statusleiste. Terminal: `settings` (Seite der Server), `llm server use` |
| `llm.api_key_ref` | `-` | advanced | – | App: Einstellungen (Models, Detail des Servers). Terminal: `settings`, `llm key set`, `llm key remove`, `llm key status`; der Schlüssel selbst erscheint nie |
| `llm.endpoint` | `-` | basic | Si (`--llm URL`) | App: Einstellungen (Models), Setup. Terminal: `settings`, `llm server edit --url` |
| `llm.models` | `[]` | basic | – | App: Einstellungen (Models). Terminal: `settings`, `llm server edit --model` |
| `llm.passthrough_paths` | `["/v1/","/api/"]` | advanced | – | nur Datei |
| `llm.server` | `-` | advanced | PP; PW; Si (`--llm NAME`) | App: Einstellungen (Models, „Who uses which server“), Erststart-Karte, Sandbox-Seite (nur Anzeige). Terminal: `settings` (Seite der Zuordnung), `settings --project`, Erststart-Bildschirm, `llm server assign`, `unassign`, `confirm`, `sandbox plan` |
| `llm.servers` | `{}` | advanced | – | App: Einstellungen (Models, Liste und Detail). Terminal: `settings` (Seite der Server), `llm server ls`, `add`, `edit`, `rename`, `rm` |

### `pseudonyms`

Der Rücktausch von Pseudonymen in Antworten.

| Schlüssel | Vorgabe | Stufe | Weitere Ebenen | Sichtbar und änderbar |
|---|---|---|---|---|
| `pseudonyms.max_response_bytes` | `8388608` | expert | – | nur Datei; heute ohne Wirkung (HUM-079) |
| `pseudonyms.translate_responses` | `true` | advanced | – | nur Datei; heute ohne Wirkung (HUM-079) |

### `recorder`

Die Aufzeichnung.

| Schlüssel | Vorgabe | Stufe | Weitere Ebenen | Sichtbar und änderbar |
|---|---|---|---|---|
| `recorder.inline_max_bytes` | `262144` | expert | PP | nur Datei |
| `recorder.retention_days` | `180` | advanced | – | nur Datei; der Abschnitt „Retention“ im Audit der App erklärt die Frist und bietet einen Befehl zum Kopieren, den wirksamen Wert zeigt er nicht |

### `resolver`

Die Namensauflösung nach einer Freigabe.

| Schlüssel | Vorgabe | Stufe | Weitere Ebenen | Sichtbar und änderbar |
|---|---|---|---|---|
| `resolver.cache_ttl_secs` | `300` | expert | – | nur Datei |
| `resolver.nameserver` | `-` | expert | – | nur Datei |
| `resolver.overrides` | `{}` | expert | – | nur Datei |
| `resolver.prefer` | `"ipv4"` | expert | – | nur Datei |
| `resolver.test_ca` | `-` | expert | – | nur Datei |

### `sandbox`

Sandbox, Projektordner und Sandbox-Backend.

| Schlüssel | Vorgabe | Stufe | Weitere Ebenen | Sichtbar und änderbar |
|---|---|---|---|---|
| `sandbox.backend` | `"auto"` | basic | PW; Si (`--backend`) | App: Einstellungen (Sandbox), Erststart-Karte (Feld Isolation), Sandbox-Seite (nur für die nächste Sitzung). Terminal: `settings`, `settings --project`, Erststart-Bildschirm, `sandbox backends` |
| `sandbox.env` | `{}` | advanced | – | nur Datei; App: Sandbox-Seite, Reiter „Environment“, nur lesbar |
| `sandbox.min_isolation` | `"policy"` | advanced | PP; PW; Si (`--sandbox-min-isolation`, `go --kernel`; hebt nur) | nur Datei; App: Sandbox-Seite, Reiter „Isolation“, nur lesbar |
| `sandbox.mode` | `"auto"` | basic | PW; Si (`--disposable`, `--direct`) | App: Einstellungen (Sandbox), Erststart-Karte (Feld Ordner), Sandbox-Seite (Schalter „Disposable“), Setup. Terminal: `settings`, `settings --project`, Erststart-Bildschirm, `sandbox plan` |
| `sandbox.podman.cpus` | `-` | advanced | – | nur Datei |
| `sandbox.podman.image` | `-` | advanced | – | nur Datei; `sandbox image pull` und „Pull image…“ der App ziehen das genannte Abbild |
| `sandbox.podman.memory_mib` | `-` | advanced | – | nur Datei |
| `sandbox.podman.pids_limit` | `4096` | expert | – | nur Datei |
| `sandbox.podman.program` | `-` | expert | – | nur Datei |
| `sandbox.podman.rootfs` | `"host"` | advanced | – | nur Datei |
| `sandbox.podman.runtime` | `"crun"` | advanced | Si (`--podman-runtime`) | nur Datei |
| `sandbox.podman.start_timeout_secs` | `30` | expert | – | nur Datei |
| `sandbox.profile` | `"default"` | advanced | PW | App: Erststart-Karte (Feld Profil), Sandbox-Seite (nur Anzeige). Terminal: Erststart-Bildschirm, `sandbox plan` |
| `sandbox.qemu.cpus` | `-` | advanced | – | nur Datei |
| `sandbox.qemu.kernel` | `-` | expert | – | nur Datei |
| `sandbox.qemu.memory_mib` | `-` | advanced | – | nur Datei |
| `sandbox.qemu.modules` | `-` | expert | – | nur Datei |
| `sandbox.qemu.qemu` | `-` | expert | – | nur Datei |
| `sandbox.qemu.start_timeout_secs` | `30` | expert | – | nur Datei |
| `sandbox.qemu.vhost_device_vsock` | `-` | expert | – | nur Datei |
| `sandbox.qemu.virtiofsd` | `-` | expert | – | nur Datei |
| `sandbox.work_dir` | `-` | basic | Si (`--work`, `go --dir`) | App: Einstellungen (Sandbox), Setup, Sandbox-Seite. Terminal: `settings`, `sandbox plan` |
| `sandbox.work_mode` | `"rw"` | basic | PW; Si (`--work-mode`, `go --ro`, `go --rw`) | App: Erststart-Karte (Feld Ordner), Sandbox-Seite („The agent may“), Setup. Terminal: Erststart-Bildschirm |

### `terminal`

Der Terminal-Client einer Sitzung.

| Schlüssel | Vorgabe | Stufe | Weitere Ebenen | Sichtbar und änderbar |
|---|---|---|---|---|
| `terminal.bell_on_hold` | `true` | advanced | – | nur Datei |
| `terminal.direct_keys` | `true` | advanced | – | nur Datei |
| `terminal.on_close` | `"detach"` | advanced | – | nur Datei |
| `terminal.prefix` | `"ctrl-b"` | advanced | – | nur Datei; `--prefix` an `sandbox attach` und `sandbox exec` gilt für einen Aufruf |
| `terminal.splash` | `true` | advanced | – | nur Datei |

### `tls`

Das Vertrauen beim Ziel.

| Schlüssel | Vorgabe | Stufe | Weitere Ebenen | Sichtbar und änderbar |
|---|---|---|---|---|
| `tls.trust_system_store` | `true` | advanced | – | App: Einstellungen (Network). Terminal: `settings` |

### `ui`

Die Oberfläche.

| Schlüssel | Vorgabe | Stufe | Weitere Ebenen | Sichtbar und änderbar |
|---|---|---|---|---|
| `ui.language` | `"en"` | basic | PP | App: Einstellungen (Interface), Befehlspalette. Terminal: `settings` |
| `ui.notifications` | `true` | advanced | PP | App: Einstellungen (Interface). Terminal: `settings` |
| `ui.terminal_notices` | `true` | advanced | PP | nur Datei |
| `ui.theme` | `"dark"` | advanced | PP | App: Einstellungen (Interface), Befehlspalette. Terminal: `settings` |

### `update`

Die Suche nach neuen Releases.

| Schlüssel | Vorgabe | Stufe | Weitere Ebenen | Sichtbar und änderbar |
|---|---|---|---|---|
| `update.check` | `true` | basic | – | App: Einstellungen (Daemon). Terminal: `settings` |

## 3. Inventar der Befehle und Bildschirme

### 3.1 Befehle

[`reference/cli.md`](reference/cli.md) führt 97 Einträge: 81 Befehle, die etwas tun, und 16
Gruppen. Die Tabelle nennt je Eintrag den Zweck und die Stelle, an der dieselbe Fähigkeit in der
App und in einer Terminal-Ansicht steht. Ein Strich heißt: dort gibt es sie nicht. Flags und
Hilfetexte stehen in der Referenz.

| Befehl | Zweck | In der App | In einer Terminal-Ansicht |
|---|---|---|---|
| `run` | Startet eine Sitzung im gespeicherten Projektordner (`sandbox.work_dir`), sonst im aktuellen Verzeichnis, oder zeigt, was ein Profil ergibt. | Startfenster („Start“), Sandbox-Seite („Start“) | – |
| `go` | Startet einen Agenten im aktuellen Ordner und übergibt ihm das Terminal; am Ende die Frage zu wartenden Änderungen. | Startfenster | Erststart-Bildschirm, Präfix-Ansichten |
| `sandbox` | Gruppe: Sandbox starten, planen, prüfen. | – | – |
| `sandbox run` | Startet eine Sandbox mit einem Kommando als Agent und reicht dessen Exit-Code durch. | – | – |
| `sandbox argv` | Druckt die Kommandozeile des Sandbox-Backends, ohne zu starten. | Sandbox-Seite („Show the exact command“) | – |
| `sandbox check` | Startet eine kurzlebige Sandbox und misst die drei Garantien. | Sandbox-Seite, Reiter „Isolation“, nur in einer laufenden Sitzung | – |
| `sandbox backends` | Listet die Sandbox-Backends mit Stufe, Zulassung, Wahl und dem, was fehlt. | Sandbox-Seite, Reiter „Isolation“; Einstellungen (Sandbox) | `settings`, Zeile des Backends |
| `sandbox plan` | Zeigt, was ein Start hier bekäme: Profil, Ordner, Modus, Modellserver mit Herkunft. | Sandbox-Seite (Kopf, „Mounts“, „Environment“), in Teilen | – |
| `sandbox attach` | Hängt dieses Terminal an die laufende Sitzung. | Startfenster („Attach“), Terminal der Sandbox-Seite | Präfix-Ansichten |
| `sandbox exec` | Öffnet eine weitere Shell in der laufenden Sandbox. | Terminal-Reiter „+ Shell“ | – |
| `sandbox changes` | Zeigt die Änderungen eines Wegwerf-Laufs, mit Diff. | Sandbox-Seite („Review changes“) | – |
| `sandbox apply` | Schreibt Änderungen eines Wegwerf-Laufs in den Projektordner und verwirft den Rest. | Änderungs-Ansicht („Apply selected“, „Apply all“) | – |
| `sandbox discard` | Verwirft alle Änderungen eines Wegwerf-Laufs. | Änderungs-Ansicht („Discard all“) | – |
| `sandbox image` | Gruppe: das Abbild einer Container-Sandbox. | – | – |
| `sandbox image pull` | Zieht ein Abbild mit festem Digest, direkt über das Netz. | Sandbox-Seite („Pull image…“) | – |
| `rules` | Gruppe: Regeln. | Rules | – |
| `rules list` | Listet die Regeln in der Reihenfolge der Auswertung; `--all` auch mitgelieferte und abgelaufene. | Rules | – |
| `rules add` | Legt eine Regel an. | Rules („New rule“) | Interceptor (`r`) aus einer angehaltenen Anfrage |
| `rules update` | Ändert eine Regel, auch ihre Gültigkeit (`--expires`). | Rules, Editor der Regel | – |
| `rules remove` | Entfernt eine Regel. | Rules („Delete rule“, mit „Undo“) | – |
| `rules reorder` | Verschiebt eine Regel in ihrer Gruppe. | Rules (Ziehen, `Alt+Up`, `Alt+Down`) | – |
| `rules dry-run` | Zeigt, welche der zuletzt aufgezeichneten Anfragen eine Regel getroffen hätte. | Rules, „Dry run“ im Editor der Regel | – |
| `rules disable` | Schaltet eine mitgelieferte Regel ab. | Rules („Switch off“) | – |
| `rules enable` | Schaltet sie wieder an. | Rules („Switch on“) | – |
| `rules reload` | Liest `rules.yaml` neu. | Rules („Reload“) | – |
| `rules test` | Wertet eine URL gegen den ganzen Regelsatz aus; Exit 0, 10 oder 11. | – | – |
| `flows` | Gruppe: Flows dieser und früherer Sitzungen. | History | – |
| `flows list` | Listet Flows mit Filter, auch `state:held`. | History; Warteschlange in Intercept | Interceptor (nur angehaltene) |
| `flows show` | Zeigt einen Flow oder einen seiner Bodies. | History (Detail), Intercept (Karte) | Interceptor |
| `flows decide` | Entscheidet eine wartende Anfrage, auf Wunsch mit Regel (`--remember`). | Intercept (Aktionsleiste), Benachrichtigung | Interceptor (`a`, `b`, `r`), Kasten von `--ask terminal` |
| `sessions` | Zeigt die laufende Sitzung oder die, die ein Start hier bekäme, und wer ohne Regel entscheidet. | Startfenster („Running“), Statusleiste | – |
| `sessions summary` | Zeigt, was ein Lauf im Projekt hinterlassen hat: Dateien, mögliche Secrets, Verweise nach außen. | – | – |
| `audit` | Gruppe: Audit-Kette. | Audit | – |
| `audit verify` | Prüft die Kette: jeden Record, seinen Vorgänger, die Anker. | Audit („Check now“) | – |
| `audit export` | Schreibt die Records als JSONL oder CSV in eine Datei. | Audit („Export“) | – |
| `config` | Gruppe: Konfiguration und Schema. | Einstellungen | `settings` |
| `config get` | Druckt den aufgelösten Wert eines oder aller Schlüssel; `--origin` nennt die Ebene. | – | `settings` zeigt Wert und Herkunft seiner Zeilen |
| `config set` | Schreibt einen Schlüssel in `config.toml`, mit `--project` in das Projekt-Profil. | Einstellungen, für Schlüssel mit Zeile | `settings`, für Schlüssel mit Zeile |
| `config schema` | Druckt das JSON-Schema oder die Liste der Profile. | – | – |
| `config edit` | Öffnet `config.toml` im Editor und prüft sie danach. | Einstellungen (Daemon, Zeile `config.toml`) | `settings`, Zeile `config.toml` |
| `config catalog` | Druckt den Einstellungskatalog des Daemons mit Herkunft und Sperre je Zeile. | ist die Quelle der Einstellungen | ist die Quelle von `settings` |
| `config paths` | Druckt, wo Humanitl Daten des Nutzers hält. | – | – |
| `daemon` | Gruppe: der Hintergrunddienst. | – | – |
| `daemon status` | Fragt den Daemon, wer er ist. | Statusleiste, Einstellungen (Daemon) | `settings`, Zeile der Version |
| `daemon install` | Schreibt die systemd-Unit des Nutzers und startet den Dienst. | Setup („Install and start“) | – |
| `daemon logs` | Zeigt das Journal des Daemons. | – | – |
| `daemon restart` | Startet den Dienst neu. | Statusleiste, Einstellungen (Daemon) | `settings`, Zeile des Neustarts |
| `daemon uninstall` | Stoppt den Dienst und entfernt die Unit. | – | – |
| `daemon retire-if-idle` | Bittet den Daemon, sich zu beenden, wenn keine Sitzung ihn hält. | – | – |
| `doctor` | Prüft diese Maschine, eine Zeile je Voraussetzung, mit Abhilfe; `--probe` startet eine leere Sandbox je Backend. | Setup (Schritt der Sandbox-Prüfungen), ohne `--probe` | – |
| `update` | Gruppe: neue Releases. | – | – |
| `update check` | Fragt jetzt nach einem neueren Release. | Hinweis in der Shell, Einstellungen (Daemon) | `settings`, Zeile der Version |
| `update apply` | Installiert das neuere Release. | Hinweis in der Shell („Update“) | – |
| `llm` | Gruppe: Modellserver. | Einstellungen (Models) | `settings` (Models) |
| `llm discover` | Sucht im lokalen /24 nach Modellservern. | Setup („Find one“) | – |
| `llm test` | Fragt einen Endpunkt, was er ist und welche Modelle er hat. | Setup („Contact it“), Einstellungen („Check“) | `settings`, Seite der Server (`p`) |
| `llm key` | Gruppe: der Zugangsschlüssel im Schlüsselbund. | Einstellungen (Models, Detail) | `settings` |
| `llm key status` | Sagt, ob und wo ein Schlüssel liegt. | Einstellungen (Models, Detail) | `settings` |
| `llm key set` | Legt den Schlüssel ab, gelesen von `stdin`. | Einstellungen (Models, Detail) | `settings` (`k`) |
| `llm key remove` | Entfernt Schlüssel und Verweis. | Einstellungen (Models, Detail) | `settings` |
| `llm server` | Gruppe: gespeicherte Modellserver. | Einstellungen (Models) | `settings`, Seite der Server |
| `llm server ls` | Listet die gespeicherten Server. | Einstellungen (Models), Statusleiste | `settings`, Seite der Server |
| `llm server add` | Speichert einen Server. | Einstellungen (Models, `Ctrl+N`) | `settings`, Seite der Server (`a`) |
| `llm server use` | Macht einen Server zum Standard, ab der nächsten Sitzung. | Einstellungen („Make default“), Statusleiste | `settings`, Seite der Server (`Enter`) |
| `llm server rename` | Benennt einen Server um. | Einstellungen (Models, Detail) | `settings`, Seite der Server (`n`) |
| `llm server rm` | Entfernt einen Server und seinen Schlüssel. | Einstellungen (Models, Detail) | `settings`, Seite der Server (`x`) |
| `llm server edit` | Ändert Adresse oder Modelle eines Servers. | Einstellungen (Models, Detail) | `settings`, Seite der Server (`e`, `m`) |
| `llm server assign` | Lässt ein Sitzungsprofil oder ein Projekt einen gespeicherten Server nutzen. | Einstellungen („Who uses which server“) | `settings`, Seite der Zuordnung |
| `llm server unassign` | Nimmt die Zuordnung zurück. | Einstellungen („Who uses which server“) | `settings`, Seite der Zuordnung |
| `llm server confirm` | Bestätigt den Server, den das Projekt-Profil eines Ordners wählt. | Dialog vor dem Start („Allow this project’s model server?“) | – |
| `intercept` | Gruppe: das Anhalten pausieren. | Kopfleiste (Pille) | – |
| `intercept status` | Zeigt, ob das Anhalten an oder pausiert ist. | Kopfleiste (Pille) | – |
| `intercept pause` | Pausiert das Anhalten bis zum Fortsetzen, zur nächsten Sitzung oder zum Ende des Daemons. | Kopfleiste (Pille, mit Rückfrage) | – |
| `intercept resume` | Setzt das Anhalten fort. | Kopfleiste (Pille) | – |
| `egress` | Gruppe: der Entscheidungs-Proxy. | Einstellungen (Decision proxy) | `settings` (Decision proxy) |
| `egress status` | Zeigt den Entscheidungs-Proxy, den Weg des laufenden Daemons und ob Zugangsdaten liegen. | Einstellungen (Decision proxy) | `settings` |
| `egress check` | Öffnet einen Tunnel durch den Proxy samt TLS-Handschlag, ohne Anfrage. | Einstellungen (Decision proxy, „Connection“) | `settings`, Zeile der Prüfung |
| `egress auth` | Gruppe: Zugangsdaten des Entscheidungs-Proxys. | Einstellungen (Decision proxy) | `settings` |
| `egress auth set` | Legt `USER:PASSWORD` im Schlüsselbund ab, gelesen von `stdin`. | Einstellungen (Decision proxy) | `settings` |
| `egress auth remove` | Entfernt die Zugangsdaten. | Einstellungen (Decision proxy) | `settings` |
| `agents` | Gruppe: das Agent-Depot. | Einstellungen (Agents) | `settings`, Seite der Agenten (nur Quelle) |
| `agents status` | Zeigt je Agent Quelle mit Herkunft oder Sperre, Version, Signatur, Anmeldung, Größe. | Einstellungen (Agents), Karte je Agent | `settings`, Seite der Agenten (nur Quelle) |
| `agents source` | Setzt, woher ein Agent kommt: `host` oder `depot`. | Einstellungen (Agents), Erststart-Karte | `settings`, Seite der Agenten; Erststart-Bildschirm |
| `agents remove` | Entfernt das Depot eines Agenten. | Einstellungen (Agents, „Remove“) | – |
| `agents install` | Installiert einen Agenten in einer Wartungs-Sandbox ins Depot. | Einstellungen (Agents, „Install“) | – |
| `agents update` | Aktualisiert einen Agenten im Depot oder wechselt auf eine gehaltene Version. | Einstellungen (Agents, „Update“) | – |
| `agents login` | Meldet einen Agenten des Depots in einer Wartungs-Sandbox an. | Einstellungen (Agents, „Log in“) | – |
| `agents callback` | Reicht die Callback-URL des Browsers in die laufende Anmeldung. | Einstellungen (Agents, Feld der Anmeldung) | – |
| `agents config` | Lässt den Agenten auf einer Kopie seiner Konfiguration laufen und übernimmt Änderungen erst nach Bestätigung; `--show` listet nur. | Einstellungen (Agents, „Configure“, „Show configuration“) | – |
| `projects` | Gruppe: das Projektregister. | Startfenster | – |
| `projects ls` | Listet die bekannten Projekte. | Startfenster („Running“, „Recent“) | – |
| `projects show` | Zeigt ein Projekt mit jedem Schlüssel seiner Projektwahl, dem wirksamen Wert und der Herkunft. | Erststart-Karte über „Start choice…“, ohne Herkunft | `settings --project` |
| `projects clean` | Räumt auf: Plan mit Größen, Rückfrage, dann Löschen. | Startfenster („Clean up“) | – |
| `projects forget` | Vergisst Projektwahl und Registereintrag eines Ordners. | – | – |
| `projects proposal` | Zeigt den Startvorschlag eines Ordners, den Inhalt des Erststart-Bildschirms. | Erststart-Karte | Erststart-Bildschirm |
| `app` | Öffnet das Startfenster der App mit einem vorgewählten Ordner. | Startfenster | – |
| `settings` | Jede Einstellung mit Zeile als Vollbild im Terminal; `--project` zeigt und ändert die Projektwahl. | Einstellungen | ist die Ansicht selbst |

Zehn Befehle haben kein Gegenstück in der App: `sandbox run`, `rules test`, `sessions summary`,
`config get`, `config schema`, `config paths`, `daemon logs`, `daemon uninstall`,
`daemon retire-if-idle` und `projects forget`. Dazu kommen zwei Schalter, die nur die
Kommandozeile hat: `doctor --probe` und `projects clean --older-than`. Umgekehrt gibt es eine
Fähigkeit nur in der App: eine angehaltene Anfrage vor dem Senden bearbeiten und Funde
pseudonymisieren (der Editor hinter „Edit + Allow"); der Kasten von `--ask terminal` bearbeitet
mit `e` im `$EDITOR`, aber nur für Kommandos ohne Vollbild.

[`reference/parity.md`](reference/parity.md) nennt vier RPC ohne Ort in der App (`Egress`,
`GetSessionSummary`, `Projects`, `SandboxBackends`). Für drei davon stimmt das nicht mehr: Die
Kategorie „Decision proxy" der Einstellungen ruft `Egress`, das Startfenster `Projects`, die
Sandbox-Seite und die Einstellungen `SandboxBackends`. Es fehlen die Einträge in
`app/lib/core/parity.dart`, nicht die Bildschirme. Nur `GetSessionSummary` hat wirklich keinen.

### 3.2 Bildschirme der App

Die App hat zwei Zustände. Ohne angehängte Sitzung zeigt sie das Startfenster, mit Sitzung die
Sitzungsansicht mit sechs Bereichen in der Leiste links. Ohne Daemon öffnet sie Setup.

| Bildschirm | Wie man hinkommt | Zweck |
|---|---|---|
| Startfenster, „Projects" | Start der App, `humanitl app`, Palette „Go to the start window" | „Running" mit „Attach", „Recent" mit „Start" und „Start choice…", „New project…" |
| Erststart-Karte | neuer Ordner oder „Start choice…" | Zeile der Vorgaben, „Start", „Customize…" mit sechs Feldern; speichert die Projektwahl |
| Startfenster, „Clean up" | Reiter | Projekte wählen, „Review cleanup…", Dialog „Delete these items?" mit Größen und „Stays" |
| Startfenster, „Settings" | Reiter | dieselbe Komponente wie das Einstellungs-Overlay |
| Intercept | `Ctrl+1` | Warteschlange, Karte der Anfrage mit Grund des Anhaltens und Frist, Domain-Spalte, „Allow", „Block", Notiz, „Remember" |
| Editor | `E` oder „Edit + Allow" | Anfrage vor dem Senden ändern, Funde durch Pseudonyme ersetzen |
| History | `Ctrl+2` | jede aufgezeichnete Anfrage mit Zustand und „Decided by", Filter, Detail, Export |
| Rules | `Ctrl+3` | Reiter „Saved" und „Temporary", mitgelieferte Regeln, Reihenfolge, Editor mit „Dry run" |
| Sandbox | `Ctrl+4` | Terminal des Agenten mit Reitern für weitere Shells; Kopf mit Zustand, Ordner, Backend, Modellserver, „Start", „Stop"; Reiter „Mounts", „Environment", „Isolation", „Log" |
| Änderungen des Wegwerf-Laufs | Sandbox-Seite, „Review changes (N)" | Liste mit Diff je Datei, „Apply selected", „Apply all", „Discard all" |
| Audit | `Ctrl+5` | Zustand der Kette, „Check now", Tabelle der Records mit Filter, „Export", Abschnitt „Retention" mit erklärendem Text |
| Setup | `Ctrl+6`, oder von selbst ohne Daemon und bei einem Schritt, der den Start blockiert | vier Schritte: Hintergrunddienst, Modellserver, Projektordner, Sandbox-Prüfungen |
| Einstellungen | Zahnrad, `Ctrl+,`, Abhilfe eines Befunds | zehn Kategorien: Models, Agent, Agents, Sandbox, Requests, Interface, Keyboard shortcuts, Network, Decision proxy, Daemon |
| Befehlspalette | `Ctrl+K` | Bereiche, Thema, Sprache, Einstellungen, Terminal, Skalierung, Startfenster, „About" |

Ständig sichtbar sind fünf Elemente: die Kopfleiste (Bereich, Pille „Intercept ON" oder
„PAUSED", Zahl der angehaltenen Anfragen, Zahnrad), die Statusleiste (Daemon und Version,
Sitzung, „3/3 isolation checks passed", Warteschlange, Modellserver „LLM NAME" mit Umschalter,
Update-Hinweis), das schwebende Terminal (`Ctrl+Shift+P`), das Tray-Symbol mit Zähler und die
Benachrichtigung des Systems.

Neun Dialoge kommen dazu: Anhalten pausieren, Cloud-Inferenz bestätigen, den Modellserver eines
Projekt-Profils erlauben, Agent stoppen, Abbild ziehen, Sammelentscheidung über mehrere
Anfragen, dauerhafte Regel speichern, Suche nach Modellservern, die genaue Kommandozeile der
Sandbox.

### 3.3 Ansichten im Terminal

| Ansicht | Wie man hinkommt | Zweck |
|---|---|---|
| `humanitl settings` | Befehl; `--project PATH` für die Projektwahl | Kategorien links, Zeilen rechts mit Wert und Herkunft, `/` sucht; Unterseiten für Server, Zuordnung und Quelle der Agenten |
| Erststart-Bildschirm | erstes `humanitl go` in einem Ordner, den das Register nicht kennt | Zeile der Vorgaben, „Start" mit `Enter`, „Customize…" mit `a`, `Esc` bricht ab |
| Einstellungen über dem Agenten | Präfix und `,` in `go`, `sandbox attach`, `run -- CMD` | die globalen Einstellungen, ohne Editor und Neustart |
| Interceptor | Präfix und `i` | Liste der angehaltenen Anfragen, `a` erlauben, `b` blockieren, `r` Regel in zwei Schritten |
| Hilfe-Kasten | Präfix und `?` | die Tabelle der Präfix-Tasten |
| Kasten von `--ask terminal` | `humanitl run --ask terminal -- CMD`, nur ohne Vollbild-Agent | eine angehaltene Anfrage: `a`, `s`, `b`, `r`, `e`, `v`, `n` |

Die Präfixtaste ist `Ctrl+B` (`terminal.prefix`). Dazu kommen zwei Stellen ohne eigene Ansicht:
die Ladeanimation von `humanitl go` und die Textfrage am Ende eines Wegwerf-Laufs (`[a]pply
all`, `[d]iscard all`, `[v]iew the diffs`, `[k]` für später). Regeln, Verlauf, Audit, Sandbox und
Aufräumen haben im Terminal keine Ansicht, nur Befehle.

## 4. Die zwölf Abläufe, wie sie heute sind

Gezählt wird der kürzeste Weg, der heute geht. Ein Schritt ist eine Befehlszeile, ein Klick
oder eine Taste, die etwas auslöst; das Tippen eines Werts zählt als ein Schritt, ein Akkord
wie `Ctrl+B i` ebenso. Nicht gezählt werden die Installation, ein laufender Daemon und das
Öffnen der App oder eines Terminals. Wo ein Ablauf etwas voraussetzt, steht es dabei.

### W1 Erster Start in einem Projektordner

Voraussetzung: Der Agent ist auf dem Host oder im Depot, und für einen Agenten mit lokalem
Modell ist ein Modellserver gespeichert. Der Erststart legt keinen Server an; er wählt nur
unter den gespeicherten.

| | App | Terminal |
|---|---|---|
| Weg | „New project…", Ordner im Dateidialog wählen, „Start" auf der Erststart-Karte | `humanitl go claude` im Ordner, `Enter` auf dem Erststart-Bildschirm |
| Schritte | 3; mit `humanitl app .` aus dem Ordner 2 | 2; mit `--yes` 1 |
| Zusätzlich | bei einem Agenten mit Cloud-Inferenz einmal je Agent und Anbieter der Dialog „Start with cloud inference?" | dieselbe Frage als `[y/N]`; `--yes` beantwortet sie mit |
| Ohne Klick sichtbar | „Running", „Recent"; auf der Karte der Einleitungssatz, die Zeile der Vorgaben, „Start" mit Fokus, „Customize…" | Name des Ordners, Zeile der Vorgaben (Agent, Ordnermodus, Isolation, wo gefragt wird, Modellserver), „Start" mit Fokus |

Die Vorgaben sind der Start, der ohne Bildschirm geschähe. „Start" speichert sie als
Projektwahl; der nächste Start im Ordner zeigt keinen Bildschirm mehr.

Was fehlt oder verwirrt:

- Es gibt drei Wege zu starten, mit zwei Regeln für den Ordner. `humanitl go` und das
  Startfenster nehmen den genannten Ordner und die Projektwahl. `humanitl run`, Setup („Start a
  session") und „Start" auf der Sandbox-Seite nehmen den in `sandbox.work_dir` gespeicherten
  Ordner und erst ohne ihn das aktuelle Verzeichnis.
- Setup stammt aus der Zeit vor dem Startfenster und führt weiter seinen eigenen Ordner, seinen
  eigenen Schalter für den Schreibmodus und einen eigenen Startknopf.
- Im Terminal wird mit der Vorgabe `hold.ask_mode = "ui"` in der App gefragt. Wer nur
  `humanitl go` benutzt, erfährt das aus einer Zeile beim Start; `--ask terminal` lehnt `go` mit
  `CLI_002` ab.
- Die Zeile der Vorgaben nennt Werte, keine Folgen. Dass „folder read-write" ein direktes
  Schreiben in den Projektordner ist, weil die Wegwerf-Schicht hier nicht geht, sagt erst
  `SANDBOX_033` beim Start.

### W2 Anderer Modellserver für ein Projekt oder eine Sitzung

Voraussetzung: Der Server ist gespeichert (`humanitl llm server add NAME URL`, in der App
Einstellungen, Models, `Ctrl+N`).

| | App | Terminal |
|---|---|---|
| Nur diese Sitzung | kein Weg | `humanitl run --llm NAME` oder `humanitl go claude --llm NAME`: 1 Schritt |
| Dieses Projekt | Einstellungen, Models, Tabelle „Who uses which server", Zeile des Projekts, Server wählen: 4 Schritte. Oder „Start choice…", „Customize…", Feld Modellserver, „Start": 4 Schritte | `humanitl llm server assign NAME --project` im Ordner: 1 Schritt. Oder `humanitl settings --project .`, Zeile „Who uses which server", Server wählen: 3 Schritte |
| Sehen, was gilt | Sandbox-Seite: „Model server NAME · QUELLE" | `humanitl sandbox plan`: Server und Herkunft (dieser Lauf, das Projekt, ein Profil, `llm.active`) |

Die Fähigkeit ist vollständig da: der Schlüssel `llm.server`, die Wahl je Lauf, je Projekt und
je Sitzungsprofil, die Anzeige der Herkunft. `--llm` mit einem Wert ohne `://` ist ein Name,
mit `://` eine Adresse (`llm.endpoint`).

Was fehlt oder verwirrt:

- Das sichtbarste Bedienelement tut das, was der Ablauf vermeiden will: Der Umschalter „LLM
  NAME" in der Statusleiste und „Make default" ändern den Standard für alle Projekte.
- Die Wahl je Projekt hat zwei Speicherorte. `llm server assign --project` und die Tabelle der
  Einstellungen schreiben `llm.server` in das Projekt-Profil im Projektordner; eine solche Wahl
  braucht einmal eine Bestätigung je Ordner (`CONFIG_022`, `llm server confirm`), die die
  Zuordnung über den Daemon gleich mit ausstellt. Der Erststart und `settings --project`
  schreiben denselben Schlüssel in die Projektwahl beim Daemon. Keine Oberfläche sagt, welcher
  der beiden Orte gemeint ist.
- Die Tabelle der Einstellungen kennt als Projekt nur den Ordner aus `sandbox.work_dir`, nicht
  die Projekte des Registers.
- Die App hat keine Wahl nur für eine Sitzung.
- Ein Wechsel gilt ab der nächsten Sitzung; eine laufende behält ihren Server.

### W3 Agent wählen und konfigurieren

| | App | Terminal |
|---|---|---|
| Agent für einen Start | kein Weg ohne Speichern | `humanitl go claude`: 1 Schritt, gilt nur für diesen Start |
| Agent für ein Projekt | „Start choice…", „Customize…", Feld Agent, „Start": 4 Schritte | Erststart mit `a`, oder `humanitl settings --project .`, Zeile „Coding agent": 3 Schritte |
| Agent für alle Projekte | Einstellungen, Agent, „Coding agent": 3 Schritte | `humanitl config set agent.adapter claude`: 1 Schritt |
| Quelle, Installation, Anmeldung | Einstellungen, Agents: je Agent eine Karte mit Quelle, Version, Signatur, Anmeldung, „Install", „Update", „Log in", „Configure", „Remove" | `humanitl agents status`, `source`, `install`, `update`, `login`, `remove`; in `settings` nur die Quelle |
| Konfiguration ansehen | Karte des Agenten, „Show configuration": 3 Schritte | `humanitl agents config claude --show`: 1 Schritt |
| Konfiguration ändern | „Configure": der Agent läuft auf einer Kopie, danach Liste der geänderten Dateien, „Accept" oder „Discard" | `humanitl agents config claude`, dann bestätigen: 2 Schritte |

Was das Tool selbst verwaltet: Es legt dem Agenten in der Sandbox eine eigene Instruktionsdatei
und einen Skill `humanitl` hin (`agent.briefing.enabled`, Vorgabe an), wählt den Adapter
(`agent.adapter`), kann dessen Kommandozeile ersetzen (`agent.command`) und reicht Argumente für
einen Start durch (`humanitl go claude -- --resume`). Für Agenten im Depot hält es Programm,
Konfiguration und Anmeldung. Die Konfiguration eines Agenten auf dem Host liest es nie.

Was fehlt oder verwirrt:

- Die Instruktionsdatei hat keine Oberfläche: Ihr Schalter steht nur in `config.toml`, ihr Text
  ist nirgends einzusehen.
- Optionen je Agent gibt es als Einstellung nur eine (`agent.pi.web_access`), und sie steht nur
  in der Datei. `agent.command` ebenso.
- Erweiterungen (Skills, Plugins, MCP-Server) zeigt nichts als solche. `agents config --show`
  listet die Dateien der Konfiguration eines Depot-Agenten und markiert Schlüssel, die Kommandos
  ausführen (bei Claude Code unter anderem `mcpServers` und `hooks`); Name, Art und Netzbedarf
  einer Erweiterung nennt es nicht.
- „Agent" und „Agents" sind zwei Kategorien der Einstellungen: die eine wählt den Adapter, die
  andere das Depot.
- Was im Depot je Agent schon geht, ist ungleich; die Tabelle steht in
  [`guides/agent-depot.md`](guides/agent-depot.md).

### W4 Von einer angehaltenen Anfrage erfahren und sie entscheiden

| | App | Terminal |
|---|---|---|
| Erfahren | Warteschlange in Intercept, Zähler in der Kopfleiste, Tray-Symbol mit Zahl, Streifen über dem Terminal („Waiting for you: …"); ist das Fenster nicht vorn, eine Benachrichtigung mit Host, Methode, Pfad, Restzeit und den Knöpfen „Allow", „Block", „Show" (bei einer Anfrage mit offenem Fund ohne „Allow") | ein Klingeln des Terminals je neuer Anfrage (`terminal.bell_on_hold`), höchstens einmal je Sekunde; kein Text im Bild des Agenten |
| Entscheiden | Anfrage in der Warteschlange, `Enter` oder `A` erlaubt, `B` blockiert: 1 Schritt. Aus der Benachrichtigung: 1 Klick | `Ctrl+B i`, Pfeiltaste, `a` oder `b`: 3 Schritte. Ohne angehängtes Terminal: `humanitl flows list state:held`, dann `humanitl flows decide ID allow`: 2 Schritte |
| Mit Regel | `R` öffnet „Remember": Dauer (Once, Session, 1 h, Forever) und Ziel (URL, Host, Domain, Host + method), dann „Allow": 3 bis 4 Schritte; „Forever" fragt nach; danach „Undo" | im Interceptor `r`, Ziel, Dauer, `Enter`; oder `flows decide ID allow --remember HOST --remember-expires session`: 1 Schritt mehr |
| Ohne Klick sichtbar | Warteschlange nach Ziel gruppiert, Karte mit „Held: no rule matches · default: ask" oder der Regel, die fragt, Frist als Ring, Query, Header, Body, Funde | im Interceptor: Methode, Host, Größe, Funde, Wartezeit, Frist |

Schon da sind außerdem: Sammelentscheidung je Gruppe („Send N", „Block N") mit Rückfrage,
Notiz an den Agenten beim Blockieren, die Pause über Funden („Send anyway", „Pseudonymize",
„Block"), die Regel aus einer abgelaufenen Anfrage („Save as rule").

Was fehlt oder verwirrt:

- Wer nur im Terminal sitzt, erfährt von einer Anfrage durch ein Klingeln und von ihrem Ablauf
  gar nicht. Zahl, Restzeit und Ziel stehen erst nach `Ctrl+B i` da.
- Läuft die Frist ab, wird blockiert; die App zeigt das (Banner, Zähler, Tray-Symbol), das
  Terminal der Sitzung nicht.
- In der App erlaubt `Enter` die gewählte Anfrage. Der Interceptor wählt nie von selbst und
  entscheidet nur nach einer ausdrücklichen Taste.
- Die Dauer einer Regel heißt an vier Stellen verschieden: „Once, Session, 1 h, Forever" in
  „Remember", „this session, until, always" im Editor der Regel, `never`, `session` oder ein
  Zeitpunkt in der Kommandozeile, „session" und „permanent" im Kasten von `--ask terminal`.
- Die Benachrichtigung zeigt ab Werk Inhalt und Knöpfe (`ui.notifications = true`).

### W5 Regeln verwalten

| | App | Terminal |
|---|---|---|
| Sehen | Rules (`Ctrl+3`): 1 Schritt. Die Reihenfolge steht als Text da: temporäre Regeln, dann gespeicherte, dann mitgelieferte, „Otherwise · ask" | `humanitl rules list`: 1 Schritt; mitgelieferte und abgelaufene erst mit `--all` |
| Ordnen | Regel ziehen oder `Alt+Up`, `Alt+Down`: 2 Schritte; nicht bei gesetztem Filter | `humanitl rules reorder ID POSITION`: 1 Schritt, innerhalb der eigenen Gruppe |
| Befristen | Regel öffnen, „Lifetime" auf „until" mit Zeitpunkt, „Save": 4 Schritte; „Make permanent" für eine temporäre | `humanitl rules update ID --expires WANN`: 1 Schritt |
| Testen, eine Regel | „Dry run" im Editor der Regel zählt beim Tippen, wie viele der zuletzt aufgezeichneten Anfragen sie getroffen hätte | `humanitl rules dry-run --action allow --host …`: 1 Schritt |
| Testen, eine URL | kein Weg | `humanitl rules test URL`: 1 Schritt; Exit 0 erlaubt, 10 blockiert, 11 nachfragen |

Schon da sind außerdem: mitgelieferte Regeln abschalten und mit einer eigenen überstimmen,
Löschen mit „Undo", Neuladen von `rules.yaml`, der Vermerk „from REQUEST" an einer Regel aus
einer Entscheidung, die Anzeige „expired" und „expires in …".

Was fehlt oder verwirrt:

- Das Terminal hat keine Ansicht der Regeln, nur Befehle mit einer UUID als Kennung.
- Die Frage „was geschieht mit dieser URL" beantwortet nur `rules test`; die App hat sie nicht.
  „Dry run" prüft eine einzelne Regel und sagt selbst, dass eine Regel darüber gewinnen kann.
- Treffer je Regel gibt es nicht. Der Vertrag hat das Feld `hit_count`, der Daemon füllt es
  immer mit 0, und nur `rules list --json` gibt es aus.
- Überdeckte oder tote Regeln erkennt nichts.
- Die Reihenfolge gilt je Gruppe; eine eigene Regel steht immer vor den mitgelieferten und nie
  vor der Durchleitung zum Modellserver. Das steht in der App als Text, in `rules list` als
  Spalte `ORIGIN`.

### W6 Änderungen des Agenten prüfen, übernehmen oder verwerfen

Der Ablauf gibt es nur nach einem Wegwerf-Lauf (`sandbox.mode`, Vorgabe `auto`: Wegwerf-Schicht,
wo das System sie kann). Nach einem direkten Lauf steht alles schon im Projektordner.

| | App | Terminal |
|---|---|---|
| Weg | Sandbox (`Ctrl+4`), „Review changes (N)", „Apply all", bestätigen | am Ende von `humanitl go` die Frage, `a`; später `humanitl sandbox changes`, `humanitl sandbox apply`, bestätigen |
| Schritte | 4 | 1 am Ende des Laufs, sonst 3 |
| Ohne Klick sichtbar | im Startfenster die Marke „Changes waiting" am Projekt; auf der Sandbox-Seite der Knopf mit der Zahl | die Liste der Änderungen vor der Frage; `humanitl projects ls` nennt wartende Änderungen |

Schon da sind: Diff je Textdatei, Größe und Prüfsumme sonst, einzelne Dateien wählen, die
Marken „conflict", „never applied", „mode bits dropped" und „hard link split", `--overwrite` und
die Rückfrage je überschriebener Datei, die Sperre eines neuen Starts, solange Änderungen warten.

Was fehlt oder verwirrt:

- Im Startfenster ist das Projekt nur markiert; einen Knopf zur Prüfung gibt es dort nicht.
- „Apply selected" verwirft alles, was nicht gewählt ist. Der Dialog sagt es, der Knopf nicht.
- Nach Übernehmen oder Verwerfen gibt es kein Zurück und kein Archiv.
- Eine Zusammenfassung und Marken für riskante Dateien (Hooks, CI, `.env`, ausführbar) gibt es
  nicht. `humanitl sessions summary ID` nennt nach einem Lauf geänderte Dateien, mögliche Secrets
  und Verweise nach außen, hat aber keinen Ort in der App.
- Nur `humanitl go` fragt am Ende. `humanitl run --disposable` fragt nicht und lässt die Änderungen
  warten, `humanitl sandbox run --disposable` verwirft sie.

### W7 Isolationsstufe und Reichweite sehen und ändern

| | App | Terminal |
|---|---|---|
| Stufe sehen | Sandbox, Kopf: „BACKEND · STUFE"; Reiter „Isolation": die drei Garantien gemessen, die Backends mit Stufe, „Minimum isolation": 2 Schritte | `humanitl sandbox backends`: 1 Schritt |
| Reichweite sehen | Sandbox, „Mounts" (Satz „The agent sees only /work = …", Tabelle mit Herkunft), „Environment" (Namen, Werte zurückgehalten), „Show the exact command" | `humanitl sandbox plan` und `humanitl sandbox argv`: 2 Schritte |
| Stufe für eine Sitzung | Sandbox, „Backend for the next session": 2 Schritte | `humanitl go claude --kernel`, `--backend NAME` oder `--sandbox-min-isolation STUFE`: 1 Schritt |
| Stufe für ein Projekt | „Start choice…", „Customize…", Feld Isolation: 4 Schritte | Erststart mit `a`, oder `humanitl settings --project .`, Zeile „Sandbox backend": 3 Schritte |
| Stufe für alle Projekte | Einstellungen, Sandbox, „Sandbox backend": 3 Schritte | `humanitl config set sandbox.backend qemu`: 1 Schritt |
| Ordner | Erststart-Karte, Feld Ordner; Sandbox-Seite: „Disposable", „The agent may" | `--disposable`, `--rw`, `--ro` an `go`; `--work-mode` an `run` |

Ein gewähltes Backend fällt nie auf ein anderes zurück (`SANDBOX_034`), und ein Backend unter
der Untergrenze startet nicht (`SANDBOX_035`). Die Untergrenze selbst hebt jede Ebene nur; über
alle Ebenen gilt der strengste Wert.

Was fehlt oder verwirrt:

- Eine Ansicht „was erreicht der Agent" gibt es nicht an einem Ort. Pfade stehen in „Mounts",
  Umgebung in „Environment", das Netz in den Regeln, die Stufe im Kopf; im Terminal verteilt
  auf `sandbox plan`, `sandbox argv`, `sandbox backends` und `rules list`.
- Die Sandbox-Seite gibt es nur in der Sitzungsansicht. Vor dem ersten Start zeigt die App die
  Reichweite nicht.
- `sandbox.min_isolation` hat in keiner Ansicht ein Bedienelement. Das Feld „Isolation" des
  Erststarts schreibt `sandbox.backend`, nicht die Untergrenze, obwohl diese zur Projektwahl
  gehört. Setzen lässt sie sich nur mit `config set`, dem Flag und `go --kernel`.
- Die Stufen haben drei Sätze von Namen: `policy`, `namespaces`, `kernel` in der Konfiguration;
  „shared kernel" und „kernel level" im Erststart; „kernel policy over the real paths", „shared
  kernel, own namespaces" und „own kernel in a micro VM" auf der Sandbox-Seite.
- Der Ordnermodus `auto` weicht auf direktes Schreiben aus, wo die Wegwerf-Schicht nicht geht.
  Er meldet es (`SANDBOX_033`), aber er startet.

### W8 Eine Sitzung laufen lassen und zurückkommen

| | App | Terminal |
|---|---|---|
| Verlassen | Fenster schließen, oder Palette „Go to the start window": 1 Schritt | `Ctrl+B d`, oder das Terminal schließen: 1 Schritt |
| Zurückkommen | Startfenster, „Running", „Attach": 1 Schritt | `humanitl go` im selben Ordner oder `humanitl sandbox attach` von überall: 1 Schritt |
| Ohne Klick sichtbar | „Running" mit „No terminal attached"; Tray-Symbol mit Zahl; nach der Rückkehr das Banner „The agent has been waiting …" mit „Jump to oldest" | `humanitl sessions`: Zustand und `no terminal attached` |

Die Sitzung läuft im Daemon weiter. Angehaltene Anfragen warten bis zur Frist und werden dann
blockiert, nie erlaubt.

Was fehlt oder verwirrt:

- Nur `humanitl go` hängt beim Schließen ab (`terminal.on_close = "detach"`). `humanitl run`
  stoppt seine Sitzung beim Schließen immer, die App lässt sie immer laufen. Kein Hinweis sagt
  beim ersten Schließen, welches von beiden gerade geschah.
- Im Terminal gibt es nach dem Anhängen keine Zeile darüber, was seit dem Abhängen geschah.
- Ein Daemon trägt eine Sitzung. Ein Start in einem zweiten Projekt ist `CLI_005`.
- `humanitl sessions` nennt einen Zustand der einen Sitzung; eine Liste mit mehreren Zuständen
  gibt es nicht.

### W9 Einen Wert für alle Projekte oder ein Projekt setzen und seine Herkunft sehen

| | App | Terminal |
|---|---|---|
| Alle Projekte | Einstellungen, Kategorie, Zeile: 3 Schritte; für 32 der 95 Schlüssel | `humanitl config set KEY VALUE`: 1 Schritt, für jeden Schlüssel; oder `humanitl settings` |
| Ein Projekt | „Start choice…", „Customize…", Feld, „Start": 4 Schritte; für die Felder des Erststarts | `humanitl settings --project .`: 3 Schritte, fünf Zeilen. `humanitl config set --project KEY VALUE`: 1 Schritt, 20 Schlüssel, schreibt in den Projektordner |
| Herkunft sehen | kein Weg für Einstellungen | `humanitl config get KEY --origin`: 1 Schritt; `humanitl config catalog --project .`; die Spalte in `humanitl settings`; `humanitl projects show .` |
| Zurücksetzen | kein allgemeiner Weg; nur „Use the start folder" für `sandbox.work_dir` | `humanitl config set KEY -`: 1 Schritt |

Was fehlt oder verwirrt:

- Die App zeigt keine Herkunft und hat kein allgemeines Zurücksetzen. Der Katalog liefert die
  Herkunft je Zeile; gezeichnet wird nur die Sperre einer Firmenvorgabe. Die Texte „Source" und „Reset to default" stehen im Glossar und in
  den ARB-Dateien und werden nirgends benutzt.
- Die App kann keinen Wert für ein Projekt setzen außer über den Erststart. Die Einstellungen
  sind dort immer global.
- „Projekt" meint zwei Speicherorte mit verschiedenen Schlüsseln (Abschnitt 2.1). `config set
  --project` und `settings --project` schreiben nicht an dieselbe Stelle.
- `config get --origin` kennt Projektwahl und Sperre nicht und sagt das nicht.
- 61 Schlüssel haben kein Bedienelement, 59 davon sind in keiner Ansicht zu sehen. Die Stufen `basic`, `advanced`, `expert` ordnen
  nichts.
- `config set` nennt in seiner Rückmeldung die Ebene (`global` oder das Projekt-Profil); eine Übersicht, was vom
  Standard abweicht, und eine Erklärung je Schlüssel im Terminal gibt es nicht.
- Sechs Schlüssel der Terminal-Ansichten (`terminal.*`, `ui.terminal_notices`) lassen sich in
  `humanitl settings` nicht ändern.

### W10 Aufräumen

| | App | Terminal |
|---|---|---|
| Weg | Startfenster, „Clean up", Projekte wählen, „Review cleanup…", „Delete N items, SIZE" | `humanitl projects clean .`, dann `y` |
| Schritte | 4 | 2 |
| Ohne Klick sichtbar | die Projekte zur Wahl, „Sessions without a project", der Schalter für wartende Änderungen | der Plan: jeder Posten mit Größe, und was bleibt |

Der Daemon löscht nur genau den Plan, der gezeigt wurde. Der Projektordner und die Audit-Kette
stehen nie in einem Plan. Fristen laufen von selbst: Aufzeichnungen nach 180 Tagen
(`recorder.retention_days`), die Audit-Kette nie (`audit.retention_days = 0`).

Was fehlt oder verwirrt:

- Wie viel Platz was belegt, sieht man erst im Plan, nicht vorher und nicht je Kategorie.
- `--older-than` und `humanitl projects forget` gibt es nur in der Kommandozeile.
- Drei Wörter stehen nebeneinander: „clean" (Aufzeichnungen, auf Wunsch Wahl und Eintrag),
  „forget" (Wahl und Eintrag, Aufzeichnungen bleiben), „discard" (wartende Änderungen). Das Depot
  eines Agenten entfernt ein vierter Ort (`agents remove`, Einstellungen, Agents).
- Die Fristen stehen nur in der Datei. Der Abschnitt „Retention" im Audit der App erklärt sie
  mit festem Text und einem Befehl zum Kopieren; die wirksamen Werte zeigt er nicht.

### W11 Sicherheit nachweisen

| | App | Terminal |
|---|---|---|
| Selbsttest | in einer laufenden Sitzung Sandbox, „Isolation": 2 Schritte; die Statusleiste zeigt „3/3 isolation checks passed" ohne Klick | `humanitl sandbox check`: 1 Schritt, Exit 3 bei einem Fehlschlag; `humanitl doctor --probe` prüft jedes zugelassene Backend |
| Kette prüfen | Audit (`Ctrl+5`), „Check now": 2 Schritte | `humanitl audit verify`: 1 Schritt |
| Export | „Export", Format, Datei: 3 Schritte | `humanitl audit export --format jsonl --out DATEI`: 1 Schritt |
| Zusammen | 7 Schritte, und nur mit laufender Sitzung | 3 Befehle |

Schon da sind: die Messung der drei Garantien in der laufenden Sandbox mit Beleg je Zeile, die
gezählten Versuche an der einen Tür vorbei, die Warnungen der Prüfung (ohne Schlüssel, Records
hinter dem letzten Anker), Export als JSONL und CSV mit Zeitraum, der Export des Verlaufs.

Was fehlt oder verwirrt:

- Ohne Sitzung kann die App die Garantien nicht messen; `sandbox check` und `doctor --probe` hat
  sie nicht.
- Ein Befehl, der Selbsttest und Kettenprüfung bündelt, fehlt, ebenso ein Bericht, den ein
  Mensch ohne das Werkzeug lesen kann.
- Was die Garantien nicht abdecken, steht in [`SECURITY.md`](SECURITY.md) und in keiner
  Oberfläche.

### W12 Herausfinden, warum etwas scheiterte, und es beheben

| | App | Terminal |
|---|---|---|
| Ein Befehl oder Start scheitert | Befund an der Stelle, mit Code, Grund und Knopf für die Abhilfe („Copy command", „Open settings", „Choose folder"); Setup zeigt die Zeilen von `doctor` | Block auf `stderr` mit Code, Titel, `why`, `fix` und Link nach [`DIAGNOSTICS.md`](DIAGNOSTICS.md); `--json` gibt ihn als Objekt |
| Eine Anfrage des Agenten scheitert | History (`Ctrl+2`), Chip „Blocked", Zeile wählen: Spalte „Decided by" (manual, rule, timeout, passthrough): 3 Schritte | `humanitl flows list state:blocked`, `humanitl flows show ID`: 2 Schritte; `humanitl rules test URL` sagt, was der Regelsatz täte |
| Die Maschine | Setup (`Ctrl+6`), „Check again" | `humanitl doctor`: 1 Schritt, elf Zeilen mit Abhilfe; `humanitl daemon logs` |
| Die Sandbox | Sandbox, „Log" und „Attempts past the one door" | `humanitl sandbox backends` nennt, was einem Backend fehlt |

Jeder Fehlerpfad liefert ein Diagnostic mit `why` und, wo es eine gibt, `fix`; das prüft
`scripts/ci/lint-no-string-errors.sh`. Der Agent selbst bekommt `403` mit einer Notiz oder `504`
nach abgelaufener Frist.

Was fehlt oder verwirrt:

- Den Weg vom Fehler des Agenten zur Ursache geht der Mensch selbst: Verlauf filtern, Zeile
  finden, Regel nachschlagen. Eine Frage „warum" an einer Stelle gibt es nicht.
- Eine abgelaufene Frist ist im Terminal der Sitzung unsichtbar.
- Das Journal des Daemons hat keinen Ort in der App.
- Nicht jeder Befund trägt eine Abhilfe als Knopf; welche Codes ohne `fix` sind, ist nicht
  gezählt worden und gehört in das Issue, das die Abhilfen prüft.

## 5. Vorgaben gegen die Grenze „ab Werk abgeschirmt"

Die Grenze des Sprints: Die Vorgabe ist die dichteste Einrichtung, und alles, was dem Agenten
mehr erlaubt oder mehr nach außen trägt, ist ab Werk aus. Die folgenden Vorgaben sind heute
nicht der dichteste Wert. Geprüft sind alle 95 Schlüssel und die Vorgaben, die kein Schlüssel
sind.

| Schlüssel | Vorgabe heute | Dichtester Wert | Anmerkung |
|---|---|---|---|
| `sandbox.work_mode` | `"rw"` | `"ro"` | Mit `sandbox.mode = "auto"` schreibt der Agent in eine Schicht, wo das System sie kann; sonst direkt in den Ordner. |
| `sandbox.mode` | `"auto"` | `"disposable"` | `auto` weicht mit `SANDBOX_033` auf `direct` aus. `disposable` lehnt den Start ab, wo die Schicht nicht geht (`SANDBOX_030`). |
| `sandbox.min_isolation` | `"policy"` | `"kernel"` | Die Vorgabe ist die schwächste der drei Stufen, nicht `namespaces`. Unter Linux startet trotzdem nichts unter `namespaces`, weil kein Backend dort schwächer ist. |
| `sandbox.backend` | `"auto"` | `"qemu"` | `auto` ist bubblewrap unter Linux und Seatbelt unter macOS, nie die Kernel-Stufe, auch wo sie verfügbar ist. |
| `hold.hard_block_checksum_secrets` | `false` | `true` | Aus heißt: Eine Regel oder eine Freigabe kann ein prüfsummen-sicheres Secret unverändert senden. |
| `tls.trust_system_store` | `true` | `false` | An heißt: Jede auf dem System installierte CA gilt beim Ziel. |
| `update.check` | `true` | `false` | Einmal am Tag eine Anfrage an GitHub. Sie sendet nichts über den Nutzer, ist aber Verkehr des Daemons ohne Zutun. |
| `ui.notifications` | `true` | `false`, oder nur ein Zähler | Die Benachrichtigung zeigt Host, Methode und Pfad und trägt „Allow" und „Block"; bei einer Anfrage mit offenem Fund fehlt „Allow" (`tray/attention_text.dart`). |
| `agents.source` | `"host"` | `"depot"` | `host` nimmt das Programm des Agenten vom Rechner, nicht seine Konfiguration: Die Sandbox bekommt ein frisches Home. `depot` hält Programm, Konfiguration und Anmeldung beim Daemon. |
| `agents.encrypt_credentials` | `false` | `true` | Anmeldungen im Depot liegen unverschlüsselt, mit Dateirechten geschützt. |
| `agents.<id>.credential_writeback` (fünf Schlüssel) | `true` | `false` | Eine Sitzung darf die geprüfte Zugangsdatei ins Depot zurückschreiben. |
| `agent.pi.web_access` | `"auto"` | `"off"` | Lädt eine Erweiterung, sobald sie installiert ist. |
| `egress.proxy_resolve` | `"proxy"` | `"humanitl"` | Wirkt nur mit `egress.proxy`. Mit `proxy` prüft Humanitl private Adressen hinter einem öffentlichen Namen nicht selbst. |
| `llm.passthrough_paths` | `["/v1/","/api/"]` | die Inferenz-Endpunkte allein | Der Adapter ersetzt die beiden Präfixe durch die Endpunkte, die Inferenz machen; wirksam ist die Vorgabe also enger als ihr Text. |

Vorgaben ohne eigenen Schlüssel:

| Verhalten | Heute | Dichter wäre |
|---|---|---|
| Agent mit Cloud-Inferenz (Claude Code, Codex, Antigravity CLI) | startet nach einer einmaligen Bestätigung je Agent und Anbieter; `--yes` bestätigt mit. Danach eine Zeile bei jedem Start. | ab Werk gesperrt, mit eigenem Schalter und Herkunft |
| Modellserver außerhalb des eigenen Netzes | `--llm URL` setzt für einen Lauf eine Durchleitung, die nicht angehalten wird und eigene Block-Regeln überholt; `LLM_006` warnt, die Sitzung startet. | Ablehnung ohne ausdrückliche Freigabe |
| Durchleitung zum Modellserver | wird aufgezeichnet, nie angehalten | bleibt; sie ist erklärt und sichtbar |
| Wartungslauf des Depots (`agents install`, `update`, `login`) | die Hosts des Herstellers gehen ohne Anhalten durch; nur auf ausdrücklichen Befehl | bleibt |
| Abbild ziehen (`sandbox image pull`) | direkt über das Netz, nicht durch den Proxy; fragt vorher | bleibt |
| Abhängen beim Schließen (`terminal.on_close = "detach"`) | die Sitzung läuft unbeaufsichtigt weiter; abgelaufene Fristen blockieren | kein Dichtheitsthema, aber eines der Aufsicht |

Schon dicht sind: Die mitgelieferten Regeln in `rules/default.yaml` enthalten kein `allow`, es
gibt also keine Start-Allowlist. Eine Anfrage ohne Regel wird angehalten, eine abgelaufene Frist
blockiert. `findings.enabled` ist an. Die Pause des Anhaltens ist aus, fragt vor dem Einschalten
und lässt sich nicht dauerhaft setzen. Die Untergrenze der Isolation lässt sich von keiner Ebene
senken.

Für keine dieser Lockerungen gilt heute alles, was die Grenze verlangt (sichtbarer Schalter,
sichtbare Herkunft, Audit-Eintrag): Fünf der 18 Schlüssel der oberen Tabelle stehen nur in der
Datei, und die Herkunft zeigt die App nirgends.

## 6. Lückenliste je Ablauf

Je Ablauf, was die späteren Issues von Sprint 9 hinzufügen, ändern oder nur erreichbar machen
müssen. „Erreichbar machen" heißt: Die Fähigkeit ist im Daemon und in der Kommandozeile da.

| Ablauf | Hinzufügen | Ändern | Nur erreichbar machen |
|---|---|---|---|
| W1 | Folgen in der Zeile der Vorgaben; Modellserver im Erststart anlegen | ein Startweg und eine Ordner-Regel statt drei und zwei; Setup auf die Prüfung der Maschine zurückführen | nichts; der Erststart auf einem Bildschirm mit Modellserver und Stufe steht |
| W2 | Wahl „nur diese Sitzung" in der App und im laufenden Terminal | Umschalter der Statusleiste nicht mehr auf den globalen Standard; einen Speicherort je Projekt benennen | `llm.server` je Lauf, Projekt und Profil; Herkunft des Servers aus `sandbox plan` |
| W3 | Ansicht der Instruktionsdatei; Liste der Erweiterungen mit Art und Netzbedarf für Depot-Agenten | „Agent" und „Agents" zusammenführen; Seite der Agenten in `humanitl settings` über die Quelle hinaus | Version, Quelle, Anmeldung, Update und `agents config --show`, in der App schon vollständig |
| W4 | Zahl, Ziel und Restzeit im Terminal ohne Ansichtswechsel; sichtbarer Ablauf einer Frist im Terminal | `Enter` in der Warteschlange; Inhalt und Knöpfe der Benachrichtigung ab Werk aus; eine Benennung der Dauer | Bündelung je Ziel, Dauer der Regel, Sammelentscheidung, „Undo", Notiz |
| W5 | Zählen der Treffer im Daemon; Erkennen überdeckter Regeln; Ansicht der Regeln im Terminal | Kennung einer Regel, die sich tippen lässt | `rules test` in die App; `dry-run` in „Remember" |
| W6 | Zusammenfassung, Marken für riskante Dateien, Archiv mit Zurück | Wortlaut von „Apply selected"; Frage am Ende auch bei `humanitl run` | Prüfung aus dem Startfenster; `sessions summary` in die App |
| W7 | eine Ansicht der Reichweite; Bedienelement für die Untergrenze | die Namen der Stufen nach Abschnitt 7, das Backend als zweite Angabe; `auto` beim Ordnermodus | Liste der Backends, Plan und Kommandozeile vor dem ersten Start |
| W8 | Zeile „seit dem Abhängen"; Hinweis beim ersten Schließen | gleiches Verhalten beim Schließen für `run` und `go` | Abhängen, Anhängen und „Running" stehen |
| W9 | Herkunft und ein allgemeines Zurücksetzen in der App; Projekt-Sicht der Einstellungen in der App; Übersicht der Abweichungen | die zwei Projekt-Ebenen in der Bedienung zu einer zusammenführen (Abschnitt 7); Stufen wirksam machen oder streichen; `config get --origin` um Projektwahl und Sperre ergänzen | Herkunft je Zeile aus dem Katalog; `config set --project` |
| W10 | Größen vor dem Plan und je Kategorie | Wörter für Aufräumen, Vergessen, Verwerfen | `--older-than` und `forget` in die App |
| W11 | ein gebündelter Befehl; ein lesbarer Bericht; „was wir nicht schützen" in der Oberfläche | nichts | `sandbox check` und `doctor --probe` in die App, ohne Sitzung |
| W12 | die Kette vom Fehler des Agenten zur Ursache; Journal in der App | Ablauf einer Frist als Ereignis im Terminal | `rules test`, „Decided by", `flows show` als Bausteine |

Über die Abläufe hinweg, in der Reihenfolge ihres Gewichts:

1. **Die App zeigt keine Herkunft, kein allgemeines Zurücksetzen und keine Projekt-Sicht.** Die Daten
   liefert der Katalog schon; `humanitl settings` zeichnet sie.
2. **Zwei Projekt-Ebenen unter einem Namen**, und `llm.server` liegt je nach Befehl in der
   einen oder der anderen.
3. **Das Terminal der Sitzung zeigt keinen Zustand.** Angehaltene Anfragen klingeln, abgelaufene
   Fristen sind unsichtbar, und die App ist ab Werk der Ort der Entscheidung.
4. **61 von 95 Schlüsseln lassen sich nur in der Datei ändern**, darunter die Untergrenze der Isolation, der
   harte Block für Secrets und alle Schlüssel der Terminal-Ansichten. Die Stufen ordnen nichts.
5. **Mehrere Startwege mit verschiedenen Regeln**: `go` und das Startfenster gegen `run`, Setup
   und die Sandbox-Seite.
6. **Die Firmenvorgabe hat keine Datei.** Jede Zusage des Sprints, die „Firma" oder „Vorgabe"
   nennt, braucht zuerst die Stelle, an der eine Sperre entsteht.

## 7. Was der Plan von Sprint 9 anders annimmt

Der Abgleich mit dem Tracking-Issue des Sprints. Die Nummern sind die seiner Liste.

**Verlangt, aber schon vorhanden:**

| Punkt | Was schon da ist |
|---|---|
| 1.7, Start scheitert laut unter der Mindeststufe | `SANDBOX_035`, und ein gewähltes Backend fällt nie zurück (`SANDBOX_034`). Offen ist nur die Darstellung. |
| 1.8, jede Diagnose mit Abhilfe | das Feld `fix` am Diagnostic, der Block der Kommandozeile, der Knopf in der App. Offen ist die Zählung der Codes ohne `fix`. |
| 2.2 und 2.3, Suche in den Einstellungen | in der App und in `humanitl settings`. |
| 2.3, Herkunft in der Einstellungs-TUI | je Zeile vorhanden. |
| 2.4, `config set` nennt seine Ebene | die Rückmeldung nennt `global`; `--project` schreibt in das Projekt-Profil. |
| 2.5, Sitzungsebene | in der Kommandozeile für Modellserver, Stufe (hebt nur), Ordnermodus und Agent vorhanden; in der App für Ordnermodus und Backend. Offen sind die Auswahl in den Oberflächen und das Netz. |
| 2.6, der aktive Server als sichtbare Regel | die Durchleitung steht als eigene Regel in der Liste und in jeder Aufzeichnung. |
| 2.8, Agentenseite in der App | Kategorie „Agents" mit Version, Quelle, Signatur, Anmeldung, Update, Konfiguration. Offen sind die Instruktionsdatei, Optionen je Agent und die Seite im Terminal. |
| 2.11, Erststart auf einem Bildschirm | in App und Terminal vorhanden, mit Modellserver und Isolation als Feldern. |
| 3.1, Bündelung je Host und wählbare Dauer | Gruppen in der Warteschlange, Sammelentscheidung, Dauer „Once, Session, 1 h, Forever", Restzeit. |
| 3.3, Vorschau und Rückgängig | „Dry run" im Editor der Regel, „Undo" nach dem Speichern und dem Löschen. Offen ist die Vorschau in „Remember". |
| 3.4, Reihenfolge und Verschieben | der Text der Auswertungsfolge, Ziehen und `Alt+Up`, die Anzeige abgelaufener Regeln. |
| 3.5, Aktionsknöpfe der Benachrichtigung | vorhanden und ab Werk an; der Punkt ist eine Änderung der Vorgabe, kein Bau. |
| 4.2, `attach` ohne Argument | `humanitl sandbox attach` nimmt schon keines; ein Daemon hat eine Sitzung. |
| 4.5, Größen beim Aufräumen | je Posten im Plan vorhanden. |
| 4.6, Exit-Codes und `--json` | dokumentiert in der Hilfe von `humanitl` und in [`cli.md`](cli.md). Offen sind `--non-interactive` und die Vervollständigung. |

**Angenommen, aber nicht vorhanden oder anders:**

| Annahme | Ist-Stand |
|---|---|
| Die Vorgabe von `sandbox.min_isolation` ist `namespaces`. | Sie ist `policy`. `namespaces` ist unter Linux die Stufe des Backends, nicht die Untergrenze. |
| Die App zeigt Herkunft und Sperren aus dem Katalog. | Sie zeigt die Sperren. Die Herkunft kommt an und wird nicht gezeichnet. |
| Es gibt eine Ebene „Firma" (Einstellungsmodell, Cloud-Sperre 2.7). | Es gibt den Mechanismus der Sperre, aber keine Vorgabedatei; heute entsteht nie eine Sperre. |
| In diesem Sprint gibt es keine Projektdatei im Repository. | Es gibt sie seit HUM-066: `<projekt>/.humanitl/profile.toml` mit `[config]` und `[rules]`. Entschieden am 2026-11-23, siehe unten: Sie bleibt, eine neue kommt nicht dazu. |
| Die Ebenen heißen Standard, Meins, Projekt, Sitzung, Firma. | Zwischen „Meins" und „Projekt" liegen heute Profile (`profiles/*.toml`, `--profile`), und „Projekt" sind technisch zwei Ebenen; für die Bedienung werden sie eine, siehe unten. |
| Die Isolationsstufen heißen Prozess-Sandbox, Container, Micro-VM. | Die drei Stufen des Codes sind `policy` (Seatbelt), `namespaces` (bubblewrap und Podman) und `kernel` (qemu). Prozess-Sandbox und Container sind dieselbe Stufe. Entschieden am 2026-11-23, siehe unten: Die Namen folgen den Stufen des Codes. |
| Treffer und letzter Treffer je Regel lassen sich anzeigen (3.4). | `hit_count` steht im Vertrag und ist immer 0; gezählt wird nicht. |
| Einfach- und Expertenansicht bauen auf den Stufen auf (2.1). | Die Stufen stehen im Schema, aber kein Client liest sie; ihre Verteilung (10, 50, 35) ist nie an einer Oberfläche geprüft worden. |
| Die Präfixtaste wechselt von `Ctrl+B` (1.9). | Stimmt, aber die Taste hat schon einmal gewechselt: Der alte Name `terminal.detach_prefix` ist ein Alias, und [`guides/first-session.md`](guides/first-session.md) nennt noch `Ctrl+A d`. |
| Die Inhaltsprüfung ist eine Einstellung, die ab Werk an ist. | Der Proxy öffnet TLS immer; einen Schalter dafür gibt es nicht. Abschaltbar ist nur die Erkennung von Funden (`findings.enabled`). |

**Entscheidungen des Owners nach dieser Aufnahme (2026-11-23):**

Zwei der Abweichungen oben sind entschieden. Die Issues des Sprints richten sich danach, nicht
nach dem ursprünglichen Wortlaut des Plans.

1. **Isolationsstufen.** Gezeigt werden die drei wirklichen Stufen mit technischen Namen:

   | Stufe im Code | Name in der Oberfläche |
   |---|---|
   | `policy` | Kernel-Richtlinie (macOS, Seatbelt) |
   | `namespaces` | Namensräume (geteilter Kernel; bubblewrap, Podman) |
   | `kernel` | Eigener Kernel (Micro-VM, QEMU) |

   Das Sandbox-Backend steht als zweite Angabe neben der Stufe und ist nicht ihr Name. Die
   drei Sätze von Namen aus W7 werden damit einer.
2. **Projekt-Ebenen.** In der Bedienung gibt es eine Ebene, „dieses Projekt". Technisch
   bleiben beide: die Projektwahl beim Daemon und das Projekt-Profil im Repository. Wohin
   welcher Befehl schreibt, legt das ADR zum Einstellungsmodell fest (#80); der Vorschlag ist,
   dass jeder Befehl und jede Ansicht in die Projektwahl des Daemons schreibt. Ein Wert aus
   der Datei im Repository trägt die Marke „aus dem Repository". Eine neue Projektdatei
   entsteht nicht. Offen für #80 bleiben damit die beiden Befehle, die heute in das
   Projekt-Profil schreiben (`config set --project`, `llm server assign --project`), und die
   Bestätigung je Ordner, die eine Wahl von `llm.server` aus dem Repository braucht.

[`guides/first-session.md`](guides/first-session.md) beschreibt noch den Weg vor dem
Startfenster (die App öffnet Setup, der Start geschieht dort). Die Seite gehört in die
Dokumentations-Runde am Ende des Sprints.
