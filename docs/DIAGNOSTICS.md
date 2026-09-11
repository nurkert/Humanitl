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
| daemon | `DAEMON` | 001 | 019 | 001-004 Start, Erreichbarkeit, Version des Daemons, 005-008 die Nutzer-Unit von `daemon install` (HUM-044) |
| ipc | `IPC` | 001 | 009 | gRPC-Schnittstelle, Token, Aufrufe gegen den Zustand |
| config | `CONFIG` | 001 | 019 | 001-006 Datei, Schlüssel, Wertebereiche, 007-009 Profile (HUM-066), 010-012 Test-Wurzel und ihr Flag (HUM-087), 013 der Projektordner der Einrichtung (HUM-044) |
| sandbox | `SANDBOX` | 001 | 029 | 001-006 Launcher und Profil, 007 Bridge-Richtung, 010-012 Start-Fehler, 020-025 /work-Härtung (HUM-043) |
| proxy | `PROXY` | 001 | 019 | Anfragen, Caps, Protokoll, 010-011 Grenzen der Verbindung (HUM-120) |
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

**Auslöser.** Der Client findet keinen Socket oder kein Token unter dem Laufzeitpfad, oder die Verbindung dorthin scheitert.

**Fix.** `InstallService` richtet die Nutzer-Unit ein; sonst nennt der Befund `humanitld`.

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

**Fix.** Meist ohne Fix — der Text nennt Pfad und Grund; beim zu langen Pfad `SetEnv` für ein kürzeres `XDG_RUNTIME_DIR`.

#### DAEMON_005

Fremde Unit-Datei wird nicht überschrieben

**Auslöser.** Unter dem Unit-Pfad liegt eine Datei, deren erste Zeile nicht die Marke von Humanitl trägt.

**Fix.** `CopyCommand`, das die fremde Datei beiseitelegt (`mv … .bak`); ein `--force` gibt es mit Absicht nicht.

#### DAEMON_006

Unit-Datei nicht schreibbar

**Auslöser.** Die Unit-Datei lässt sich nicht schreiben (Rechte, Verzeichnis, Dateisystem).

**Fix.** `CopyCommand` mit dem Pfad, sonst der Verweis auf die Dokumentation.

#### DAEMON_007

humanitld liegt nicht neben humanitl

**Auslöser.** Neben der laufenden Kommandozeile liegt kein `humanitld`, das die Unit starten könnte — oder der eigene Pfad ist gar nicht zu ermitteln.

**Fix.** `CopyCommand` mit `ls -l` auf den erwarteten Pfad; im zweiten Fall kein Fix.

#### DAEMON_008

systemd hat die Unit nicht übernommen

**Auslöser.** `systemctl --user` hat die geschriebene Unit nicht übernommen oder nicht gestartet.

**Fix.** `CopyCommand` mit `systemctl --user status humanitld`, dem Aufruf, der sagt, was systemd stört.

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

**Auslöser.** Dieser Daemon hat die Fähigkeit nicht, nach der gefragt wurde — keine Sandbox, keine Aufzeichnung, keine Endpunkt-Probe.

**Fix.** Kein Fix: Es ist eine Aussage über diesen Daemon, nicht über die Anfrage.

### Bereich config

#### CONFIG_001

Config-Datei ungültig

**Auslöser.** Eine Konfigurations- oder Profildatei ist nicht lesbar oder kein gültiges TOML.

**Fix.** Meist ohne Fix — der Text nennt Datei und Parserfehler; beim Start `ChangeSetting` auf das Vorgabeprofil.

#### CONFIG_002

Unbekannter Schlüssel

**Auslöser.** Ein Schlüssel, ein Block oder ein Pfad steht in der Datei, den das Schema nicht kennt.

**Fix.** `ChangeSetting` auf den ähnlichsten Schlüssel, wenn es einen gibt; sonst ohne Fix.

#### CONFIG_003

Wert außerhalb des Bereichs

**Auslöser.** Ein Wert liegt außerhalb seines Bereichs, hat den falschen Typ, ein Profil hat die falsche Form, oder ein Projekt-Profil setzt einen Schlüssel, der ihm nicht gehört.

**Fix.** `ChangeSetting`, wo es einen Schreibweg gibt; beim Projekt-Profil bewusst keiner.

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

**Fix.** Der Fix nennt die fehlende Hälfte.

#### CONFIG_012

Test-Wurzel ohne absoluten Pfad

**Auslöser.** `resolver.test_ca` ist kein absoluter Pfad und hinge damit am Startverzeichnis des Daemons.

**Fix.** `ChangeSetting` auf einen absoluten Pfad.

#### CONFIG_013

Kein Projektordner gewählt

**Auslöser.** Niemand hat einen Projektordner gewählt; der Client meldet es, nicht der Daemon.

**Fix.** Kein Fix-Knopf: Der Ordner wird im Setup gewählt.

#### CONFIG_014

Einstellung nicht über den Daemon setzbar

**Auslöser.** `SetConfig` nimmt bis HUM-069 nur eine Variable unter `sandbox.env` an, deren Wert das Zertifikat in der Sandbox ist.

**Fix.** Kein Fix: Der Text nennt, was angenommen wird; alles andere steht von Hand in `config.toml`.

#### CONFIG_015

config.toml nicht geschrieben

**Auslöser.** `config.toml` ließ sich nicht ändern, ohne mehr als den einen Wert zu ändern, oder nicht schreiben; sie ist unberührt.

**Fix.** Kein Fix: Der Text nennt die Zeile, die von Hand in den Block `[sandbox.env]` gehört.

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

**Fix.** Kein Fix: Der Text nennt Pfad und Regel.

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

**Auslöser.** Die Verbindung zum Ziel kommt nicht zustande, oder es ist keine Adresse angeheftet.

**Fix.** Kein Fix: Der Text nennt Ziel und Ursache.

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

**Fix.** `ChangeSetting` auf eine höhere Grenze.

#### PROXY_011

Anfrage-Rumpf ist stehengeblieben

**Auslöser.** Ein Anfrage-Rumpf ist stehengeblieben und hat die Frist überschritten.

**Fix.** `ChangeSetting` auf eine längere Frist.

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

**Auslöser.** Der Endpunkt antwortet nicht, löst nicht auf, oder die Frist läuft ab.

**Fix.** `CopyCommand` mit einem `curl`, das dasselbe versucht.

#### LLM_002

LLM-Endpoint verlangt eine Anmeldung

**Auslöser.** Der Endpunkt verlangt eine Anmeldung (`401` oder `403`).

**Fix.** Kein Fix: Humanitl schickt im MVP keine Zugangsdaten.

#### LLM_003

LLM-Endpoint antwortet nicht als bekannte API

**Auslöser.** Die Verbindung steht, aber weder `/api/tags` noch `/v1/models` antwortet als bekannte API.

**Fix.** `ChangeSetting` auf die Adresse der API-Wurzel.

#### LLM_004

Kein Modell konfiguriert

**Auslöser.** `llm.models` ist leer; der Agent bekommt einen Platzhalter statt eines Modells.

**Fix.** `ChangeSetting` auf `llm.endpoint` oder `CopyCommand` mit einem `curl` auf `/models` — erst fragen, dann eintragen.

#### LLM_005

Funde in einer durchgereichten Anfrage

**Auslöser.** Eine durchgereichte Anfrage an das Sprachmodell trägt Funde: Geheimnisse oder personenbezogene Daten.

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

### Bereich audit

#### AUDIT_001

Hash-Kette gebrochen

**Auslöser.** Reserviert für eine gebrochene Hash-Kette im Audit-Log. Heute baut ihn niemand: Die Crate `humanitl-audit` ist eine leere Hülle, und es gibt keine Datei, deren Kette prüfbar wäre (HUM-029).

**Fix.** Kein Fix: Der Code wartet auf das Audit-Log.

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

**Auslöser.** Das Agent-Kommando liegt nicht im `PATH` des Daemons.

**Fix.** `CopyCommand` mit dem Installationsbefehl des Agenten.

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

**Fix.** Kein Fix: Wer sie gestartet hat, beendet sie dort.

### Bereich ui

#### UI_002

Kein Platz für das Anzeigesymbol

**Auslöser.** Die Arbeitsumgebung bietet keinen Platz für das Anzeigesymbol im Systembereich.

**Fix.** Kein Fix: Das Fenster bleibt der Weg zur Warteschlange.

### Bereich agent

#### AGENT_001

Agent-Kommando nicht gefunden

**Auslöser.** Das Agent-Kommando ist weder im `PATH` noch über `agent.command` gesetzt.

**Fix.** `CopyCommand` mit dem Installationsbefehl.

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

**Auslöser.** Das Kommando liegt auf dem Rechner, aber nicht in dem, was die Sandbox einhängt.

**Fix.** `CopyCommand`, das es an eine eingehängte Stelle installiert, sonst der Verweis auf die Dokumentation.

