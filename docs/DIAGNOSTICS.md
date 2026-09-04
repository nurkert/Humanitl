# Diagnose-Codes

<!-- Erzeugt aus daemon/crates/core-types/src/diagnostics/codes.rs.
     Nicht von Hand ändern: `UPDATE_DIAG_DOCS=1 cargo test -p humanitl-core --test diag_docs` schreibt die Datei neu. -->

Jeder nicht-grüne Zustand trägt einen Code der Form `BEREICH_NNN`. Der Code steht in der
Meldung, in der Oberfläche und in `audit.jsonl`; er ist der kürzeste Weg von einer
Beobachtung zu ihrer Erklärung. Eine Nummer wird nie wiederverwendet, auch nicht nach dem
Entfernen eines Codes.

## Reservierte Bereiche

| Bereich | Präfix | Von | Bis | Wofür |
|---|---|---|---|---|
| daemon | `DAEMON` | 001 | 019 | Start, Erreichbarkeit, Version des Daemons |
| ipc | `IPC` | 001 | 009 | gRPC-Schnittstelle, Token, Aufrufe gegen den Zustand |
| config | `CONFIG` | 001 | 019 | 001-006 Datei, Schlüssel, Wertebereiche, 007-009 Profile (HUM-066) |
| sandbox | `SANDBOX` | 001 | 029 | 001-006 Launcher und Profil, 007 Bridge-Richtung, 010-012 Start-Fehler |
| proxy | `PROXY` | 001 | 009 | Anfragen, Caps, Protokoll |
| tls | `TLS` | 001 | 009 | CA, Zertifikate, Handschlag |
| llm | `LLM` | 001 | 009 | LLM-Endpunkt und seine Antworten |
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
| agent | `AGENT` | 001 | 009 | Agent-Adapter: Startkommando, Vorlagen, Vorprüfung vor dem Start |

## Codes

### Bereich daemon

#### DAEMON_001

Daemon nicht erreichbar

#### DAEMON_002

Proto-Version inkompatibel

#### DAEMON_003

Socket bereits belegt

#### DAEMON_004

Laufzeitverzeichnis oder Socket nicht anlegbar

### Bereich ipc

#### IPC_001

Ungültiges Token

#### IPC_002

AllowEdited nur für genau einen Flow

#### IPC_003

Flow nicht mehr gehalten

#### IPC_004

Decide-Anfrage ungültig

#### IPC_005

Rules-Anfrage ungültig

#### IPC_006

Fähigkeit in diesem Daemon nicht verfügbar

### Bereich config

#### CONFIG_001

Config-Datei ungültig

#### CONFIG_002

Unbekannter Schlüssel

#### CONFIG_003

Wert außerhalb des Bereichs

#### CONFIG_004

Laufzeitverzeichnis ist ein Ersatz

#### CONFIG_005

Veralteter Schlüssel

#### CONFIG_006

Alter und neuer Schlüssel gesetzt

#### CONFIG_007

Projekt-Profil gehört einem anderen Konto

#### CONFIG_008

Eigenes Profil verdeckt ein mitgeliefertes

#### CONFIG_009

Profilwunsch des Projekts gilt nicht

### Bereich sandbox

#### SANDBOX_001

bwrap nicht gefunden

#### SANDBOX_002

bwrap-Version zu alt

#### SANDBOX_003

User-Namespaces nicht erlaubt

#### SANDBOX_004

Isolation-Check fehlgeschlagen

#### SANDBOX_005

Projektordner nicht beschreibbar

#### SANDBOX_006

Mount verboten

#### SANDBOX_007

Bridge-Richtung unbekannt

#### SANDBOX_010

Argumentliste des Starters unerwartet

#### SANDBOX_011

Platzhalter nicht anlegbar

#### SANDBOX_012

Kommandozeile des Starters ungültig

#### SANDBOX_013

Isolation-Check ohne Bericht

#### SANDBOX_014

Isolation-Check 1: Netzwerk-Interface vorhanden

#### SANDBOX_015

Isolation-Check 2: mehr als eine Tür

#### SANDBOX_016

Isolation-Check 3: seccomp unwirksam

### Bereich proxy

#### PROXY_001

Body über Cap

#### PROXY_002

Authority-Mismatch

#### PROXY_003

Upstream-Verbindung fehlgeschlagen

#### PROXY_005

Ungültiger Übergang im Flow

#### PROXY_007

HTTP/2 nicht verfügbar

### Bereich tls

#### TLS_001

Client hat Humanitl-CA abgelehnt

#### TLS_002

Client bricht den Handschlag wiederholt ab

#### TLS_003

Client ohne SNI

#### TLS_004

CA-Verzeichnis nicht beschreibbar

#### TLS_005

CA-Dateien unbrauchbar

### Bereich llm

#### LLM_001

LLM-Endpoint nicht erreichbar

#### LLM_002

LLM-Endpoint verlangt eine Anmeldung

#### LLM_003

LLM-Endpoint antwortet nicht als bekannte API

#### LLM_004

Kein Modell konfiguriert

#### LLM_005

Funde in einer durchgereichten Anfrage

#### LLM_006

LLM-Endpunkt liegt nicht in einem privaten Netz

#### LLM_007

LLM-Endpunkt ist keine lesbare HTTP-Adresse

### Bereich rules

#### RULES_001

Regel-Datei ungültig

#### RULES_002

Host-Muster verdächtig (xn--, IP in Host-Glob)

#### RULES_003

Host-Muster ungültig

#### RULES_005

Pfadmuster ungültig

#### RULES_006

Version der Regel-Datei unbekannt

#### RULES_007

Doppelte Regel-Id

#### RULES_008

Regel wirkt zu breit

#### RULES_009

Regel-Datei nicht schreibbar

#### RULES_010

Mitgelieferte Regel ist unveränderlich

#### RULES_011

Regelsatz neu geladen

#### RULES_012

Probelauf konnte die Aufzeichnung nicht lesen

### Bereich findings

#### FINDINGS_001

Detektor-Regeln unbrauchbar

#### FINDINGS_002

Scan unvollständig

### Bereich catalog

#### CATALOG_001

Domain-Katalog nicht lesbar

#### CATALOG_002

Rangliste nicht lesbar

### Bereich terminal

#### TERM_001

Zweiter schreibender Terminal-Client abgelehnt

### Bereich recorder

#### RECORDER_001

Aufzeichnung nicht verfügbar

#### RECORDER_002

Filter ungültig

#### RECORDER_003

Aufzeichnung konnte nicht schreiben

#### RECORDER_004

Blob-Speicher nicht benutzbar

### Bereich audit

#### AUDIT_001

Hash-Kette gebrochen

### Bereich cli

#### CLI_001

Aufruf am Daemon abgelehnt

#### CLI_002

Vollbild-TUI-Agent nicht mit --ask terminal

#### CLI_003

Unterkommando noch nicht verfügbar

#### CLI_004

Aufruf ungültig

### Bereich ui

#### UI_002

Kein Platz für das Anzeigesymbol

### Bereich agent

#### AGENT_001

Agent-Kommando nicht gefunden

#### AGENT_002

Agent-Kommando nicht ausführbar

#### AGENT_003

Gebündelte Agenten-Vorlage unbrauchbar

#### AGENT_004

Agent-Kommando in der Sandbox nicht erreichbar

