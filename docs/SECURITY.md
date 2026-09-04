# Humanitl — Sicherheit

> Stand: 2026-09-02 (Entwurf, HUM-007). Verbindliche Kurzfassung: `BACKLOG.md` Abschnitt 4.
> Wer wovon angegriffen wird, steht in [`THREAT-MODEL.md`](THREAT-MODEL.md).

Dieses Dokument beschreibt, warum ein Agent in Humanitl nicht unbemerkt ins Netz kommt, wo die
Grenzen dieser Aussage liegen, und wie man beides selbst nachprüft. Es ist für zwei Leser
geschrieben: für Fachleute ohne Sicherheitsspezialisierung, die wissen wollen, worauf sie sich
verlassen, und für ein Review, das jede Behauptung gegen einen Befehl oder eine Datei prüft.
Fachbegriffe werden bei der ersten Nennung in Klammern erklärt.

Humanitl ist im Aufbau. Dieses Dokument beschreibt den Zielzustand von Version 0.1. Wo ein
Prüfbefehl heute noch nicht existiert, steht die Issue-Nummer dabei, die ihn liefert
(Spezifikationen unter `backlog/`).

---

## 1. Die Garantie in drei Sätzen

Diese drei Sätze sind der Kern des Versprechens. Sie stehen wortgleich hier, in `BACKLOG.md` 4.1
als Überschriften und im Isolations-Panel der Anwendung (ARB-Schlüssel `isolationCheck1` bis
`isolationCheck3`, HUM-041), damit Dokument, Plan und Oberfläche nicht auseinanderlaufen. Deutsch
und Englisch stehen beide hier, weil das Panel beide Sprachen zeigt.

| # | Deutsch | English |
|---|---|---|
| 1 | **Kein Netzwerk-Interface. Es gibt keinen Weg nach draußen.** | **No network interface. There is nowhere for traffic to go.** |
| 2 | **Genau eine Tür: ein Socket, der zu Humanitl führt.** | **Exactly one door: a socket that leads to Humanitl.** |
| 3 | **Der Kernel öffnet keine neue Tür (seccomp).** | **The kernel opens no new door (seccomp).** |

Zusammengesetzt: Die Sandbox (der abgeschottete Bereich, in dem der Agent läuft) hat kein
Netzwerkgerät, ihr einziger Ausgang ist eine einzelne Socket-Datei zum Proxy von Humanitl, und der
Kernel selbst verweigert dem Agenten das Anlegen weiterer Verbindungswege. Jedes Byte, das die
Sandbox verlässt, ist entweder eine von einem Menschen freigegebene HTTP-Anfrage oder ein Aufruf
an den erklärten Sprachmodell-Host. Beides wird aufgezeichnet.

### Die zwei offenen Kanäle, vorweg

Zwei Wege stehen absichtlich offen. Sie zu verschweigen, würde das Versprechen wertlos machen,
deshalb stehen sie hier und nicht erst auf Seite drei:

1. **Das Projektverzeichnis.** Der Agent schreibt nach `/work`. Was er dort ablegt, liegt danach
   auf der Platte — auch etwas, das ein Mensch später selbst nach außen trägt.
2. **Die Verbindung zum Sprachmodell.** Der Agent schickt Code an das Modell, ohne dass jede
   Inferenz einzeln bestätigt wird. Wer den Modell-Host kontrolliert, sieht diesen Verkehr.

Beide sind in Abschnitt 3 ausführlich beschrieben, beide werden in der Anwendung angezeigt, keiner
wird geschlossen. Der Satz „der Sprachmodell-Host ist Teil der Vertrauensbasis" ist die Bedingung,
unter der alles Übrige gilt.

### Was der dritte Satz nicht heißt

Der dritte Satz heißt **nicht** „gar keine Sockets". Der Filter muss TCP über IPv4 und IPv6
erlauben, denn genau darüber erreicht der Agent den Proxy auf `127.0.0.1:3128`. Er verweigert
jede andere Familie und jeden anderen Typ. Dass ein erlaubtes TCP-Socket trotzdem nirgendwohin
führt, ist die Leistung des leeren Netz-Namespace aus Satz 1, nicht die des Filters: Dort
existiert nur `lo`, und die Routing-Tabelle ist leer. Die drei Sätze tragen gemeinsam; keiner
trägt allein.

---

## 2. Was das konkret heißt

Für jeden der drei Sätze: der Mechanismus, der Prüfbefehl, der automatisierte Test, und was ein
Angreifer versuchen würde.

### Satz 1 — Kein Netzwerk-Interface

**Mechanismus.** Die Sandbox wird mit `bwrap --unshare-all --cap-drop ALL` gestartet. `bubblewrap` ist ein
kleines, weit verbreitetes Programm, das ohne root-Rechte neue *Namespaces* anlegt — abgetrennte
Sichten des Kernels auf Netzwerk, Prozesse, Mounts, IPC und Hostname. Im neuen Netz-Namespace
existiert genau ein Gerät, die Loopback-Schnittstelle `lo`, die bwrap aktiviert, damit die Brücke
zum Proxy auf `127.0.0.1` lauschen kann. Es gibt keine IP-Adresse nach außen, keine Route, keinen
DNS-Server, kein ICMP, kein QUIC, keine Raw-Sockets. Zusätzlich werden alle Capabilities
abgegeben (`--cap-drop ALL`), damit auch niemand ein Gerät nachrüsten kann: `CAP_NET_ADMIN` fehlt.

**Prüfung (in der Sandbox).**

```sh
ip link                 # nur "lo"; falls iproute2 fehlt: cat /proc/net/dev
ip route                # leer
cat /proc/net/route     # nur die Kopfzeile
capsh --print           # Current: = (leer)
```

**Automatisiert.** `tests/escape/esc-1-sockets.sh` (ESC-1) und `esc-3-egress.sh` (ESC-3).
Zur Laufzeit prüft der Shim dasselbe und meldet es als `IsolationCheck::NoNetworkInterface`
(HUM-041); schlägt es fehl, startet der Agent nicht.

**Was ein Angreifer versuchen würde.** Ein `veth`-Paar oder ein TUN-Gerät anlegen — scheitert ohne
`CAP_NET_ADMIN`. Ein UDP-Paket an einen DNS-Server senden — scheitert schon am Filter, spätestens
an der fehlenden Route. Einen vom Host geerbten Dateideskriptor benutzen — der Daemon übergibt
ausschließlich die Deskriptoren aus `LaunchPlan::fds` und schließt die übrigen; eine
FD-Enumeration beim Agent-Start ist als zusätzliche Kontrolle vorgesehen (`BACKLOG.md` 10). Ein abstraktes Unix-Socket verwenden, weil es keinen Pfad im Dateisystem braucht — abstrakte
Sockets sind an das Netz-Namespace gebunden und dort leer.

### Satz 2 — Genau eine Tür

**Mechanismus.** Der Daemon legt seinen Proxy-Socket auf dem Host unter
`$XDG_RUNTIME_DIR/humanitl/proxy/proxy.sock` an, in einem eigenen Verzeichnis mit Modus 0700.
In die Sandbox wird die **Datei** gebunden, nach `/run/humanitl/proxy.sock`, nie das Verzeichnis.
Der Unterschied ist wesentlich: Ein gebundenes Verzeichnis würde jede Datei sichtbar machen, die
dort später entsteht, und der Agent könnte darin selbst Sockets anlegen. Der gRPC-Socket des
Daemons (`$XDG_RUNTIME_DIR/humanitl/daemon.sock`, Modus 0600) ist in der Sandbox nicht vorhanden;
über ihn steuert nur die Anwendung auf dem Host den Daemon.

Alles Weitere, was der Agent zum Arbeiten braucht, kommt aus einer Allowlist, deren vollständige
Fassung die `bwrap`-Argumentliste ist (`humanitl sandbox argv`; Reihenfolge verbindlich in
HUM-011). Im Auszug: schreibgeschützt gebunden werden `/usr`, `/etc/ssl`, `/etc/alternatives`,
`/etc/ld.so.cache`, das CA-Zertifikat als `/etc/humanitl/ca.crt` und als Overlay über
`/etc/ssl/certs/ca-certificates.crt`, generierte Minimalfassungen von `/etc/passwd`,
`/etc/group` und `/etc/hosts` (aus dem Speicher des Daemons, nicht vom Host) sowie der Shim unter
`/usr/local/bin/humanitl-shim`; frisch angelegt werden ein eigenes `/proc`, ein minimales `/dev`
(mit `/dev/shm` als `tmpfs`), `tmpfs` für `/tmp` und ein leeres `/home/agent`; das Projekt liegt
unter `/work`, darin `tmpfs` über `.git/hooks`, `.vscode`, `.idea` und `/dev/null` über `.envrc`
und `.git/config`; dazu die Proxy-Socket-Datei. Kein `/etc/resolv.conf`. Was **nicht** gebunden
wird, steht im Profil ebenso ausdrücklich: `$XDG_RUNTIME_DIR`, `/run`, `/tmp` des Hosts, `/home`,
`~/.ssh`, `~/.gitconfig`, `~/.netrc`, X11-, Wayland-, D-Bus- und Docker-Sockets; der
Argv-Builder lehnt ein Profil mit einem dieser Pfade ab (`SANDBOX_006`).

**Prüfung (in der Sandbox).**

```sh
find / -xdev -path /proc -prune -o -type s -print   # genau: /run/humanitl/proxy.sock
cat /proc/net/unix                                  # nur erlaubte Socket-Pfade
cat /proc/self/mountinfo                            # kein /run/user, kein X11, kein docker.sock
hostname                                            # "sandbox"
```

**Automatisiert.** `tests/escape/esc-2-mounts.sh` (ESC-2). Zur Laufzeit
`IsolationCheck::SingleSocket`, dessen Beweis aus der Sandbox stammt und nicht vom Host behauptet
wird: Der Shim durchsucht vor dem `exec` das Dateisystem der Sandbox nach Unix-Sockets und meldet
jeden gefundenen (`CHECK single_socket`), zusätzlich zur Zeile `CHECK bridge_listening`, die zeigt,
dass die eine Tür offen ist und antwortet. Beide zusammen sind der zweite Satz; bis zum Review vom
2026-09-03 trug die zweite Zeile ihn allein und behauptete damit mehr, als sie belegte. Der
Suchlauf ist eine Prüfung mit Budget (ohne `/proc`, `/sys` und `/dev` außer `/dev/shm`, ohne
Symlinks zu folgen, Tiefe 3, 2000 Einträge; griff eine Schranke, steht das in der Evidenz). Der
erschöpfende Beweis bleibt der `find`-Befehl oben und ESC-2. Ist eine der drei Prüfungen rot, wird
die Sandbox beendet, statt den Agenten laufen zu lassen.

**Was ein Angreifer versuchen würde.** Den D-Bus- oder Docker-Socket suchen — nicht vorhanden.
Über `/proc/<pid>/root` in ein anderes Mount-Namespace greifen — im eigenen PID-Namespace sind nur
Sandbox-Prozesse sichtbar. Einen zweiten Socket im selben Verzeichnis anlegen, um zwei
Sandbox-Prozesse an der Aufzeichnung vorbei zu verbinden — das Verzeichnis ist nicht gebunden, und
nach dem Filter ist `AF_UNIX` ohnehin gesperrt.

### Satz 3 — Keine neuen Türen

**Mechanismus.** Der `humanitl-shim` ist ein kleines Programm ohne Laufzeitumgebung (nur `libc`
und `seccompiler`), das in der Sandbox unter bwraps Init-Prozess startet. Sein Prozessmodell ist der
eigentliche Trick (HUM-012, „Variante B", verbindlich): Die Brücke lebt im Elternprozess, der
Agent ist ein Kind mit Filter vor `exec`. Beide tragen einen Filter; der des Elternprozesses ist
um genau eine Familie weiter, weil die Brücke `AF_UNIX` braucht.

1. Er öffnet die im Profil deklarierten Brücken selbst, ohne `socat`: Richtung „in" lauscht auf
   `127.0.0.1:3128` und reicht jede TCP-Verbindung an den gebundenen Unix-Socket weiter. Der
   Listener entsteht vor dem Fork, damit der Agent nie `ECONNREFUSED` sieht, und trägt
   `CLOEXEC`, damit der Agent ihn nicht erbt.
2. Er forkt. Der Elternprozess bleibt als Brücke stehen, wartet auf das Kind und endet mit
   dessen Exit-Code. Sobald das Kind läuft, legt auch er einen Filter an: dieselbe Politik wie
   für den Agenten, nur zusätzlich mit `AF_UNIX`, weil er für jede angenommene Verbindung den
   Proxy-Socket öffnet. Alles Weitere passiert im Kind.
3. Das Kind schließt alle geerbten Deskriptoren außer 0, 1, 2 und setzt `PR_SET_NO_NEW_PRIVS`.
   Damit kann kein späterer `exec` mehr Rechte gewinnen, etwa über ein setuid-Programm — ohne
   dieses Flag darf ein unprivilegierter Prozess gar keinen seccomp-Filter setzen.
4. Das Kind lädt den seccomp-Filter (ein kleines Kernel-Programm, das Systemaufrufe prüft, bevor
   sie ausgeführt werden) mit `TSYNC`, sodass er für alle Threads gilt.
5. Erst dann `execvp` auf den Agenten. Der Agent erbt den Filter und kann ihn nicht ablegen; jeder
   Prozess, den er startet, erbt ihn ebenfalls.

Der Elternprozess trägt eine eigene Fassung desselben Filters, weil `TSYNC` Threads erfasst,
keine Kinder: Der Agent kann seinen Filter nicht an den Elternprozess weiterreichen, und der
Elternprozess kann den engeren Filter des Agenten nicht selbst tragen, weil er `AF_UNIX`
braucht. Beide Filter sperren dieselben Systemaufrufe; unter bwraps Init-Prozess trägt damit
jeder Prozess in der Sandbox einen Filter (ESC-1 `seccomp_every_process`). Was das für den
Agenten bedeutet, steht in [`THREAT-MODEL.md`](THREAT-MODEL.md) K-04.

Der Filter erlaubt `socket()` ausschließlich für die Familien aus `allow_families` (`AF_INET`,
`AF_INET6`) mit den Typen aus `allow_types` (`SOCK_STREAM`). Beide Listen sind keine Einstellung,
sondern ein Boden in beide Richtungen: ein Profil, das `AF_UNIX` oder `SOCK_DGRAM` nennt, wird beim
Laden mit `CONFIG_003` abgelehnt, und der Launcher reicht an den Shim genau diesen Boden weiter.
Die eine vorgesehene Ausnahme ist das spätere Profil `browser` (M7), das `AF_UNIX` für die
Chromium-IPC braucht; sie heißt `SocketFloor::BrowserUnixIpc`, steht im Code des Launchers
(`SandboxProfile::parse_with_floor`) und lässt sich aus keiner Profildatei setzen, weil eine solche
Datei aus einem geklonten Repository stammen kann. `SOCK_DGRAM` bleibt auch dort gesperrt. Das Typ-Argument wird mit `0xff` maskiert, damit `SOCK_NONBLOCK`
und `SOCK_CLOEXEC` durchgehen. Alles andere — `AF_UNIX`, `AF_NETLINK`, `AF_PACKET`, `AF_VSOCK`,
`SOCK_DGRAM`, `SOCK_RAW` — bekommt `EPERM`. `socketpair()` bleibt vom seccomp-Filter unberührt und ist erlaubt: es kennt nur `AF_UNIX`,
verbindet zwei Deskriptoren desselben Prozessbaums und bietet keinen Egress (nötig für die
Kindprozess-IPC von Node und Bun, `CONVENTIONS.md` 4.11). ESC-1 und HUM-041 Check 3 prüfen, dass
`socketpair` gelingt. Zusätzlich verweigert werden `ptrace`, `io_uring_setup`,
`io_uring_enter`, `io_uring_register`,
`process_vm_readv`, `process_vm_writev`, `keyctl`, `add_key`, `request_key`, dazu die
Standard-Härtung `kexec_load`, `kexec_file_load`, `init_module`, `finit_module`,
`delete_module`, `bpf`, `perf_event_open`, `userfaultfd` (dieselben Namen, die das
Docker-Standardprofil sperrt), sowie **alle**
x32-Syscalls (Nummern mit gesetztem Bit `0x40000000`, abgefangen von einem handgeschriebenen
BPF-Präludium vor dem erzeugten Programm). Ein Architektur-Mismatch führt nicht zu `EPERM`,
sondern zu `KillProcess`.

Die verbindliche Liste steht im Quelltext, nicht hier: `daemon/bin/humanitl-shim/src/seccomp.rs`
mit einer `#[cfg(test)]`-Tabelle aller Regeln. Dieses Dokument zitiert sie; bei Abweichung gilt
die Datei.

**Prüfung (in der Sandbox).**

```sh
grep Seccomp /proc/self/status                       # "Seccomp: 2" (Filter-Modus)
grep NoNewPrivs /proc/self/status                    # "NoNewPrivs: 1"
# PID 1 ist bwraps Init-Prozess und trägt keinen Filter. Der Shim darunter trägt den weiteren
# Brücken-Filter, der Agent den engeren. Der Beweis liest deshalb den Agenten, nie /proc/1/status:
grep Seccomp /proc/$(pgrep -n -P "$(pgrep -o humanitl-shim)")/status   # "Seccomp: 2"
python3 -c 'import socket; socket.socket(socket.AF_UNIX)'                 # PermissionError
python3 -c 'import socket; socket.socket(socket.AF_INET, socket.SOCK_DGRAM)'  # PermissionError
python3 -c 'import socket; socket.socket(socket.AF_INET, socket.SOCK_STREAM)' # gelingt
python3 -c 'import socket; socket.create_connection(("10.0.0.1",80),2)'   # ENETUNREACH
```

**Automatisiert.** ESC-1 deckt genau diese Proben ab, einschließlich `io_uring_setup` und eines
x32-Syscalls. Zur Laufzeit `IsolationCheck::SeccompActive`: Der Shim meldet beim Start auf einem
eigenen Deskriptor (`HUMANITL_REPORT_FD`) Prüfzeilen aus der laufenden Sandbox, unter anderem
dass der Filter geladen ist und welche Familien er erlaubt; der Launcher liest sie und macht
daraus die drei Ergebnisse. Der Beweis stammt damit aus der Sandbox, in der der Agent wirklich
läuft, und nicht aus einer zweiten, nur zu Prüfzwecken gestarteten.

**Was ein Angreifer versuchen würde.** Ein setuid-Programm ausführen, um den Filter loszuwerden —
`NO_NEW_PRIVS` verhindert es. Einen Nachbarprozess mit `ptrace` übernehmen — `EPERM`. Über
`io_uring` Systemaufrufe an der Filterung vorbei einreihen — die drei `io_uring_*`-Aufrufe sind
gesperrt. Denselben Syscall unter der x32-Nummer aufrufen, ein klassischer Filter-Bypass — das
Präludium fängt ihn ab. Einen Thread starten, der den Filter nicht hat — `TSYNC` sorgt dafür, dass
es keinen solchen Thread gibt. Den Elternprozess des Shims kapern, der die Brücke hält — siehe
die ehrliche Einschränkung in [`THREAT-MODEL.md`](THREAT-MODEL.md) K-04; gewonnen ist damit
nichts, weil auch er einen Filter trägt und die Brücke genau ein Ziel kennt, den aufzeichnenden
Proxy.

---

## 3. Was nicht abgedeckt ist

Dies ist der wichtigste Abschnitt des Dokuments. Ein Sicherheitsversprechen, das seine Ausnahmen
verschweigt, ist keines. Alle fünf Kanäle werden in der Anwendung angezeigt, nicht versteckt.

### 3.1 Der Sprachmodell-Kanal (LLM-Passthrough)

*Warum es ihn gibt.* Der Agent muss Code an das Modell schicken, sonst arbeitet er nicht. Würde
jede Inferenz gehalten, müsste ein Mensch pro Minute mehrfach bestätigen; das hielte niemand
durch, und nach zwei Stunden klickt er alles blind weg.

*Was Humanitl tut.* Der Passthrough ist eine Regel, keine Lücke im Filter: exakt ein Host, ein
Port, ein Schema, die Methoden `POST` und `GET` (die Modellliste ist ein `GET`) und eine Liste von
Pfadpräfixen. Ein Präfix benennt dabei einen Endpunkt und nie eine ganze API-Fläche. Weder `/v1/`
noch `/api/` steht deshalb im Default: Beide decken neben der Inferenz auch Aufrufe, die den Server
verändern — unter `/api/` sind das `POST /api/pull`, `POST /api/create` und `DELETE /api/delete`,
unter `/v1/` `POST /v1/files`, `/v1/uploads`, `/v1/vector_stores`, `/v1/fine_tuning/jobs` und bei
vLLM `POST /v1/load_lora_adapter`. Der Default zählt stattdessen die dreizehn Endpunkte auf, die
Inferenz machen oder Auskunft geben: `/v1/chat/completions`, `/v1/completions`, `/v1/responses`,
`/v1/embeddings`, `/v1/models` sowie `/api/chat`, `/api/generate`, `/api/embed`, `/api/embeddings`,
`/api/tags`, `/api/show`, `/api/ps` und `/api/version`. Genauso wenig trifft ein Pfad mit einem
`..`-Segment ein Präfix, denn erst der Server löst es auf. Jede andere Anfrage an denselben Host
wird normal gehalten, also einem Menschen gezeigt. Wer eine davon ohne Rückfrage braucht, schreibt
ihren Pfad selbst in `llm.passthrough_paths`.

Der Verkehr wird vollständig aufgezeichnet und durch die
Findings-Detektoren geschickt; ein Treffer erzeugt eine Warnung (`LLM_005`, eine je Anfrage, mit
Zahl und Host und nie mit dem gefundenen Wert), hält aber nicht an. Weil das
Modell typischerweise im eigenen Netz steht, trägt die Regel `allow_private: true` — sie ist
damit die einzige Regel, die absichtlich in private Adressbereiche zeigt. Das Recht hängt an dieser
einen Regel und an dieser einen Anfrage: Die nächste Anfrage derselben Verbindung fängt wieder ohne
es an. Im Isolations-Panel erscheint der Kanal als vierte, bernsteinfarbene Zeile mit dem konkreten
Endpunkt.

*Was offen bleibt.* Steht in `llm.endpoint` ein Name statt einer Adresse, entscheidet der Resolver,
wohin die Durchreiche führt, und er entscheidet es bei jeder Anfrage neu. Wer diesen einen Namen
kontrolliert — ein DHCP-verteilter DNS im fremden WLAN, ein kompromittierter Router —, lenkt die
Durchreiche zwischen zwei Anfragen auf einen anderen Rechner, und weil die Regel `allow_private`
trägt, ist auch der Router selbst oder `169.254.169.254` ein gültiges Ziel. Die Prüfung auf private
Adressen greift hier nicht: Sie ist für diese Regel absichtlich ausgeschaltet. Die Abhilfe ist eine
Adresse statt eines Namens, oder ein fester Eintrag unter `resolver.overrides`, der den Namen an
genau eine Adresse bindet, bevor überhaupt jemand gefragt wird.

*Was der Nutzer tun sollte.* Nur eine Maschine eintragen, die er selbst kontrolliert. Kein
geteilter Ollama-Server ohne Authentifizierung im Firmennetz. Am besten die IP-Adresse eintragen
und nicht den Namen, oder den Namen unter `resolver.overrides` festnageln. Wer keine eigene
Maschine hat, schaltet den Passthrough ab und lässt auch die Inferenz halten — unbequem, aber
ehrlich. Und: den Endpunkt in Zeile 4 des Panels bei jedem Start einmal ansehen.

### 3.2 Das Projektverzeichnis `/work`

*Warum es ihn gibt.* Ein Agent, der nicht schreiben darf, ist ein Chatfenster.

*Was Humanitl tut.* Gebunden wird nur ein Unterpfad, nie `$HOME`. `sandbox.work_mode = "ro"` ist
möglich und für reine Analyse-Sitzungen der Vorschlag. Pfade, über die geschriebene Dateien später
auf dem Host **ausgeführt** würden, werden überdeckt: `.git/hooks`, `.vscode`, `.idea` mit einem
leeren `tmpfs`, `.envrc` und `.git/config` mit `/dev/null` (der Agent sieht eine leere Datei,
Schreibversuche enden in `EROFS`, weil der Bind schreibgeschützt ist). Zum Sitzungsende zeigt
Humanitl die berührten Dateien,
markiert neue Symlinks, deren Ziel außerhalb von `/work` liegt, und führt den Secret-Scan über den
Diff.

*Was der Nutzer tun sollte.* Den Diff lesen, bevor er committet — insbesondere neue Dateien und
alles unter `.github/`, `Makefile`, `package.json` (`scripts`) und Build-Dateien. Für fremde
Repositories mit `--work-mode ro` starten.

### 3.3 Die Terminal-Ausgabe des Agenten

*Warum es ihn gibt.* Der Mensch muss sehen, was der Agent tut. Diese Ausgabe stammt vom Agenten
und damit potenziell vom Angreifer.

*Was Humanitl tut.* Im Terminal-Widget sind OSC 52 (Zugriff auf die Zwischenablage), OSC 8
(Hyperlinks, mit denen sich ein fremdes Ziel hinter harmlosem Text verstecken lässt) und das
Setzen des Fenstertitels abgeschaltet. Über dem Terminal steht dauerhaft ein Hinweis, dass die
Ausgabe des Agenten nicht vertrauenswürdig ist.

*Was der Nutzer tun sollte.* Angezeigte Befehle nicht per Copy-Paste in eine Host-Shell übernehmen,
ohne sie zu lesen. Der klassische Angriff ist ein Befehl, dessen sichtbarer Teil harmlos ist und
dessen Rest hinter Leerzeichen und einem Zeilenumbruch steht.

### 3.4 Hostnamen in der Aufzeichnung

*Warum es ihn gibt.* Auch eine blockierte Anfrage wird protokolliert — sonst wüsste niemand, was
der Agent versucht hat.

*Was Humanitl tut.* Die Aufzeichnung liegt lokal unter `$XDG_DATA_HOME/humanitl`, nichts wird
hochgeladen. Der Export schreibt heute alles mit, was aufgezeichnet wurde: Host, vollständigen Pfad
samt Query, sämtliche Kopfzeilen und beide Rümpfe im Klartext. Das Export-Fenster sagt das in einem
Satz, bevor die Datei geschrieben wird. Eine optionale Host-Redaktion wird es geben; sie ist nach
dem MVP eingeplant und hat noch kein eigenes Issue — `backlog/sprint-4.md` nennt sie bisher nur im
Nebensatz zum Audit-Log („Im Export mit Host-Redaktion (nach MVP) wird auch die IP redigiert"). Wer
sie baut, legt das Issue an und streicht diesen Absatz.

*Was der Nutzer tun sollte.* Einen Export wie die Anfragen selbst behandeln: Er trägt Tokens in
Kopfzeilen und Geheimnisse in Rümpfen, solange es keine Redaktion gibt. Daran denken, dass schon die
Hostnamen Kundenprojekte verraten können.

### 3.5 Package-Caches

*Warum es ihn gibt.* Ohne Cache kostet jede Installation Netzverkehr und damit eine Freigabe.

*Was Humanitl tut.* Caches werden pro Projekt gehalten, nicht pro Vertrauensstufe geteilt. Wer
keinen einträgt, bekommt keinen.

*Was der Nutzer tun sollte.* Keinen Cache zwischen einem Kundenprojekt und einem Experiment
teilen. Schreibgeschützte Seed-Caches sind nach dem MVP vorgesehen.

---

## 4. Vertrauensbasis

Die Vertrauensbasis (TCB, „trusted computing base") ist die Menge der Bausteine, deren Versagen
die Garantien aufhebt. Sie ist absichtlich klein und besteht ausschließlich aus Dingen, die viele
andere ebenfalls benutzen.

| Baustein | Rolle | Anforderung | Warum wir ihm vertrauen |
|---|---|---|---|
| Linux-Kernel | Namespaces, seccomp-BPF, Capabilities | 5.x, praktisch getestet ab 5.10; `CONFIG_SECCOMP_FILTER`, nicht-privilegierte User-Namespaces erlaubt | Dieselben Mechanismen tragen Container, Flatpak und die Sandboxen von Claude Code und der OpenAI-Codex-CLI |
| `bubblewrap` | Aufbau der Sandbox | ≥ 0.8 | Klein, setuid-frei im rootless-Betrieb, Debian/Fedora-Paket, seit Jahren unter Beobachtung |
| seccomp-BPF-Filter | Verbot weiterer Socket-Familien und gefährlicher Syscalls | erzeugt von `seccompiler`, Quelle `daemon/bin/humanitl-shim/src/seccomp.rs` | Wenige Dutzend Regeln, im Test vollständig aufgezählt |
| `humanitl-shim` | Brücke halten, forken, im Kind Filter setzen und `exec` | Rust, ohne tokio, nur `libc` + `seccompiler` | Wenige hundert Zeilen; das Prozessmodell ist der ganze Inhalt |
| `humanitld` | Proxy, Hold-Queue, Regeln, Aufzeichnung, Sandbox-Steuerung | Rust, `#![forbid(unsafe_code)]` in allen Bibliotheks-Crates | Speichersicher, ein statisches Binary, keine 40 transitiven Laufzeit-Pakete |
| `hyper`/`rustls`/`rcgen` (über `hudsucker`) | TLS-Terminierung und HTTP-Verarbeitung | exakt gepinnt, regelmäßig aktualisiert | Rust-TLS-Stack ohne OpenSSL-Speicherfehlerklasse; Historie von Rapid Reset und CONTINUATION-Flood wird verfolgt |
| Der Sprachmodell-Host | sieht jeden Prompt | muss dem Nutzer gehören | Erklärter Seitenkanal, keine technische Absicherung (Abschnitt 3.1) |
| Keyring des Nutzers | HMAC-Schlüssel des Audit-Logs | Secret Service (`gnome-keyring`, `kwallet`) | Schutz gegen andere Nutzer, nicht gegen denselben Nutzer |

**Nicht in der Vertrauensbasis der Isolation:** die Flutter-Anwendung. Sie ist ein gRPC-Client wie
die CLI und kann die Sandbox nicht schwächen. Sie ist allerdings vertrauenswürdig für die
*Entscheidung*: Sie zeigt dem Menschen, worüber er entscheidet. Eine Anwendung, die den Body falsch
darstellt, führt zu einer falschen Freigabe. Deshalb rendert sie Bodies als Text und niemals in
einer WebView.

Toolchain-Anforderungen für Reproduzierbarkeit: Rust 1.85+ (gepinnt in
`daemon/rust-toolchain.toml`), Flutter 3.47.2 (gepinnt in `app/.fvmrc`), `bubblewrap` 0.8+.
`socat` wird **nicht** benötigt; die Brücke steckt im Shim, damit kein weiteres Programm in der
Sandbox laufen muss.

---

## 5. Der Proxy

**Eigene CA pro Installation.** Damit Humanitl den Inhalt von HTTPS-Anfragen zeigen kann, muss es
sie aufbrechen (TLS-Terminierung, „MITM"). Dafür erzeugt der Daemon beim ersten Start eine eigene
Zertifizierungsstelle. Der private Schlüssel liegt unter `$XDG_DATA_HOME/humanitl/ca/ca.key` mit
Modus 0600. In die Sandbox kommt ausschließlich das öffentliche Zertifikat, als
`/etc/humanitl/ca.crt`, und das Env-Kit setzt `SSL_CERT_FILE` darauf.

**Nie in den Host-Trust-Store.** Das Zertifikat wird nicht in den systemweiten oder den
Browser-Trust-Store eingetragen. Der Grund ist einfach: Ein dort eingetragenes CA-Zertifikat
würde bedeuten, dass jeder, der den Schlüssel erlangt, jede TLS-Verbindung des Nutzers fälschen
kann — auch die des Online-Bankings, das mit Humanitl nichts zu tun hat. Der private Schlüssel
bleibt deshalb auf dem Host (`$XDG_DATA_HOME/humanitl/ca/ca.key`, 0600, lesbar nur für den
Daemon); in die Sandbox geht nur `ca.crt`, und nur die Sandbox vertraut dieser CA. Das Vertrauen
endet damit an der Sandbox-Grenze: Wer `ca.crt` besitzt, kann nichts fälschen, und wer `ca.key`
erlangt hat, hat den Host bereits. Als Option ist eine ephemere CA pro Sitzung vorgesehen, die
beim Beenden verworfen wird.

*Prüfung.* `ls -l $XDG_DATA_HOME/humanitl/ca/` zeigt `0600` auf `ca.key`;
`trust list | grep -i humanitl` (beziehungsweise `ls /usr/local/share/ca-certificates/`) liefert
nichts.

**Certificate Pinning.** Manche Programme akzeptieren nur ein fest einprogrammiertes Zertifikat.
Für sie funktioniert die Terminierung nicht, und das ist beabsichtigt: Das Werkzeug scheitert
sichtbar mit einem TLS-Fehler, Humanitl bietet **keinen** Rückfallpfad, der die Verbindung
unentschlüsselt durchreicht. Ein solcher Rückfall wäre ein Tunnel, durch den ein Angreifer
beliebige Daten schicken könnte, und die Aufzeichnung wäre eine Lüge. Der Nutzer bekommt ein
`Diagnostic` mit dem Hostnamen und der Empfehlung, das Werkzeug host-seitig zu betreiben.

**Grenzen und Zeitlimits.** Der Anfrage-Body wird vollständig gepuffert, bevor gefragt wird — wer
nur Header zeigt und den Body streamt, zeigt genau den Teil nicht, in dem exfiltriert wird. Daraus
folgt eine Reihe harter Grenzen:

| Grenze | Default | Verhalten beim Überschreiten |
|---|---|---|
| `limits.hold_body_cap_bytes` | 32 MiB | `413`, `BlockReason::BodyCap`, außer die Regel sagt `stream: true` |
| `limits.preview_cap_bytes` | 8 MiB | Vorschau wird abgeschnitten, Entscheidung bleibt möglich |
| `limits.max_decompress_ratio` | 100 | Dekompression bricht ab (Schutz gegen Zip-Bomben) |
| `limits.hold_max_bytes` | 256 MiB | neue Anfrage `503`, `BlockReason::HoldMemory`; gehaltene werden nie verworfen |
| `limits.hold_max_flows` | 200 | neue Anfrage `503`, `BlockReason::HoldMaxFlows` |
| `hold.timeout_secs` | 300 | `504`, `BlockReason::Timeout` — Ablauf bedeutet **block**, nie `allow` |
| `limits.connect_timeout_secs`, `header_timeout_secs`, `body_timeout_secs`, `idle_timeout_secs` | siehe `humanitl config schema` | Verbindung wird abgebrochen, Flow wird `Failed` |

Die Zahlen sind die Defaults aus `BACKLOG.md` (Abschnitt 3 und 4.4) und Sprint 1; verbindlich ist
die Ausgabe von `humanitl config schema`, gegen die HUM-059 diese Tabelle vor dem Release
abgleicht. Für `limits.hold_max_flows` nennt HUM-057 abweichend 500; welcher Wert gilt, legt
`CONVENTIONS.md` 4.4 fest.

`Expect: 100-continue` beantwortet der Proxy sofort selbst; der Body landet im Hold-Puffer und
nicht beim Ziel. Vor der Entscheidung erreicht kein Byte den Upstream.

**Härtung des Dienstes.** Die systemd-Unit läuft als Benutzerdienst mit `NoNewPrivileges`,
`PrivateTmp`, `RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6` und
`SystemCallFilter=@system-service`. Für den Schutz des Heimatverzeichnisses gilt eine Einschränkung:
`ProtectHome=read-only` verträgt sich nicht mit Bind-Mounts von Projekten unterhalb von `$HOME`;
die endgültige Kombination wird in HUM-053 festgelegt und hier nachgetragen.

**Protokoll-Umfang.** In Milestone 1 spricht der Proxy HTTP/1.1 auf beiden Seiten; die
ALPN-Aushandlung bietet dem Client nur `http/1.1` an. Damit scheitert gRPC über TLS sichtbar
(`PROXY_007 h2 not available`) statt still. HTTP/2 zum Upstream kommt in M6 hinter
`experimental.h2_upstream`. Parser und Dekoder werden nächtlich gefuzzt.

---

## 6. Regeln und ihre Fallen

Regeln sind eine geordnete Liste, die erste passende gewinnt, der Default ist `ask`. Sitzungsregeln
(im Speicher, `expires: session`) werden vor dauerhaften Regeln ausgewertet, damit eine gerade
getroffene Entscheidung sofort gilt. Die Fallen sind alle bekannt und alle getestet (ESC-4):

- **Globs laufen über Labels, nicht über Zeichen.** `*` steht für genau ein Label, `**` für eines
  oder mehrere. `*.github.com` passt auf `api.github.com`, aber **nicht** auf `github.com` selbst
  und **nicht** auf `a.b.github.com`. `**.github.com` passt auf beides einschließlich der
  Apex-Domain. Ein Substring-Vergleich wäre die klassische Lücke: `evil-github.com` und
  `github.com.evil.io` dürfen niemals passen und tun es nicht.
- **Normalisierung vor dem Vergleich.** Hostnamen werden per `idna::domain_to_ascii` in A-Labels
  (Punycode) übersetzt, kleingeschrieben und um einen abschließenden Punkt gekürzt.
  `API.GITHUB.COM.` passt also auf `*.github.com`.
- **Internationalisierte Namen sehen aus wie andere.** `xn--80ak6aa92e.com` rendert in vielen
  Schriften als `аpple.com` mit kyrillischem „а". Das UI zeigt deshalb immer die A-Label-Form und
  daneben die Identität aus dem Katalog.
- **IP-Literale passen nie auf ein Host-Glob.** Für sie braucht es eine ausdrückliche Regel mit
  `host: "ip:192.168.1.50"` oder `host: "cidr:192.168.0.0/16"`.
- **Private Bereiche sind gesperrt.** Löst ein Name auf RFC-1918, `127/8`, `169.254/16`
  (einschließlich `169.254.169.254`, der Cloud-Metadaten-Adresse), `100.64/10`, `fc00::/7` oder
  `::1` auf, wird die Verbindung verweigert, außer die passende Regel trägt `allow_private: true`.
- **Authority-Konsistenz.** Nach der TLS-Terminierung wird pro Anfrage der Host-Header
  beziehungsweise `:authority` gegen das CONNECT-Ziel und die SNI geprüft. Ein Mismatch wird ohne
  Rückfrage geblockt (`BlockReason::AuthorityMismatch`, `403`). Das ist die Abwehr gegen Domain
  Fronting: einen erlaubten Host verbinden und im Header einen anderen anfragen.
- **WebSocket.** Ein Upgrade passt nur auf eine Regel, die `upgrade: websocket` trägt; sonst
  `ask`. Nach dem Upgrade werden Frames aufgezeichnet, aber nicht einzeln gehalten. Das UI sagt
  das ausdrücklich, bevor der Nutzer freigibt.
- **Weiterleitungen erben nichts.** Ein `301`/`302` erzeugt eine neue Anfrage des Clients an ein
  neues Ziel, und die läuft erneut durch die Regeln und gegebenenfalls in die Warteschlange. Eine
  Freigabe für `example.com` ist keine Freigabe für den Ort, auf den es weiterleitet.
- **Unbekannte Methoden führen zu `ask`,** nie zu `allow`.

*Prüfung.* `humanitl rules test https://api.github.com/repos/x/y` liefert die greifende Regel und
den Exit-Code (0 allow, 10 block, 11 ask). ESC-4 fährt die vollständige Tabelle ab.

---

## 7. DNS

Ein Hostname trägt bis zu 63 Byte je Label und 253 Byte insgesamt. Wer
`ZGllcyBpc3QgZWluIGdlaGVpbW5pcw.attacker.com` auflösen lässt, hat die Daten übertragen, sobald
irgendein Resolver die Anfrage stellt: Der autoritative Server der Angreiferdomain sieht sie, auch
wenn danach nie eine HTTP-Verbindung zustande kommt. Ein Proxy, der auflöst, **bevor** ein Mensch
entschieden hat, ist damit selbst der Exfiltrationskanal — und zwar einer, der jede Freigabe
umgeht.

Deshalb: Humanitl hält Anfragen auf dem Hostnamen als Zeichenkette. Aufgelöst wird erst nach
`allow`, genau einmal, über den `Resolver`-Port (hickory), nie über den System-Resolver eines
HTTP-Connectors. Die gefundene Adresse wird an die Verbindung gepinnt
(`Egress::connect(authority, Some(ip))`), damit eine zweite Auflösung kein anderes Ziel liefern
kann (DNS-Rebinding). Auch die Domain-Vorschau im UI holt nichts automatisch aus dem Netz; sie
stammt aus einem mitgelieferten Katalog, und ein Live-Abruf passiert nur auf ausdrücklichen Klick,
host-seitig, nur für die eTLD+1.

*Prüfung.* ESC-3 beobachtet host-seitig (über `resolvectl statistics` oder `tcpdump port 53`),
dass für einen gehaltenen Namen vor der Entscheidung kein Lookup stattfindet.

---

## 8. Aufzeichnung und Audit

Aufgezeichnet wird lokal: Flows, Entscheidungen und Bodies in SQLite unter
`$XDG_DATA_HOME/humanitl/humanitl.db`, große Bodies inhaltsadressiert unter `blobs/`, und
zusätzlich ein append-only-Protokoll `audit/audit.jsonl`. Jeder Eintrag trägt `seq`, `ts`,
`prev_hash` und `hash` über kanonischem JSON; über der Kette liegt ein HMAC mit einem
Installationsschlüssel aus dem Keyring des Nutzers. Eine Verankerung des Kopf-Hashes außerhalb der
Datei (Anzeige im UI, zweiter Ablageort, systemd-Journal) ist geplant (HUM-050), existiert im MVP
aber noch nicht; bis dahin ist das Abschneiden des Endes der Kette nicht erkennbar.

**Was die Kette beweist.** Dass seit dem letzten Anker kein Eintrag geändert, entfernt oder
umsortiert wurde, ohne dass `humanitl audit verify` es meldet — vorausgesetzt, der Angreifer hat
den HMAC-Schlüssel nicht.

**Was die Kette nicht beweist.**

1. Nicht, dass der Daemon ehrlich geschrieben hat. Ein manipulierter Daemon protokolliert, was er
   will; die Kette ist dann korrekt und trotzdem falsch.
2. Nicht, dass nichts fehlt, was nie geschrieben wurde.
3. Nicht, dass die letzten bis zu `anchor_every` Einträge hinter dem letzten Anker noch da sind:
   Wer die Datei am Ende kürzt, hinterlässt eine gültige, nur kürzere Kette. `verify` meldet das
   als Warnung und bleibt so lange rot, bis externes Head-Anchoring existiert.
4. Nichts gegen einen Angreifer, der als **derselbe Nutzer** läuft. Wer den Keyring des Nutzers
   und die Datei hat, hält beide Enden in der Hand und baut die Kette neu.

Die Kette schützt also gegen nachträgliches Editieren durch jemanden mit Dateizugriff, nicht gegen
den Nutzer selbst und nicht gegen einen Prozess mit dessen Rechten. Für stärkere Aussagen braucht
es einen externen Anker (Post-MVP).

Getrennt davon liegt das Pseudonymisierungs-Mapping: verschlüsselt, nur auf dem Host, nie in der
Sandbox und nie in einer Anfrage.

*Prüfung.* `humanitl audit verify` (Exit 0 = Kette intakt), `humanitl audit export --format jsonl`.
ESC-5 löscht einen Eintrag und kürzt die Datei; erwartet wird, dass die erste Manipulation als
Fehler und die zweite als Warnung erkannt wird.

---

## 9. So prüfst du es selbst

Nichts in diesem Dokument muss man glauben. Drei Wege, es nachzusehen — vom schnellsten zum
gründlichsten:

**1. Die genaue Kommandozeile ansehen.** Die vollständige Isolations-Policy ist ein einziger
`bwrap`-Aufruf. Er steht im UI unter „Sandbox → Isolation → Kommandozeile anzeigen" und auf der
Kommandozeile:

```sh
humanitl sandbox argv --profile default
```

Die Ausgabe ist eine Zeile, die sich per Copy-Paste in einer Shell ausführen lässt. Wer wissen
will, was gebunden wird, liest die `--bind`/`--ro-bind`-Argumente; wer wissen will, was nicht
gebunden wird, sucht vergeblich nach `/run/user`, `.ssh` und `docker.sock`.

**2. Die Prüfungen laufen lassen.**

```sh
humanitl sandbox check           # drei Checks, Beweise aus der laufenden Sandbox
humanitl sandbox check --json    # dasselbe maschinenlesbar
```

Die Prüfungen lesen `/sys/class/net`, `/proc/net/dev` und `/proc/net/unix` **in** der Sandbox
und die `Seccomp:`-Zeile aus einem Kindprozess, der den Filter trägt. Ein Beweis, den der Host über sich selbst behauptet, wäre keiner. Schlägt eine
Prüfung fehl, startet der Agent nicht; es gibt kein „trotzdem starten".

**3. Die Escape-Tests laufen lassen.**

```sh
./tests/escape/run.sh            # oder: make escape
xmllint --noout target/escape/escape.xml
```

| ID | Datei | Was er zeigt |
|---|---|---|
| ESC-1 | `tests/escape/esc-1-sockets.sh` | Socket-Familien und -Typen, `io_uring`, x32, Interfaces, Routing, Capabilities, `Seccomp: 2` |
| ESC-2 | `tests/escape/esc-2-mounts.sh` | Mount-Oberfläche, genau ein Socket, kein Host-`/proc`, Hostname |
| ESC-3 | `tests/escape/esc-3-egress.sh` | kein Egress ohne Proxy; über den Proxy landet alles in der Warteschlange; kein DNS vor der Entscheidung |
| ESC-4 | `tests/escape/esc-4-rules.sh` | die Regel-Tabelle einschließlich Homograph, IP-Literal, Body-Cap |
| ESC-5 | `tests/escape/esc-5-filesystem.sh` | Symlink-Markierung, maskierte Pfade, OSC 52, Audit-Manipulation |

Verfügbarkeit im Aufbau: `sandbox argv` und `sandbox check` liefert Sprint 1 beziehungsweise
Sprint 3 (HUM-010, HUM-041); das Test-Gerüst und ESC-1 bis ESC-3 liefert HUM-006, ESC-4 und ESC-5
folgen mit den zugehörigen Teilsystemen. Bis dahin sind die Prüfbefehle aus Abschnitt 2 der
direkte Weg: sie brauchen nur eine Shell in der Sandbox.

---

## 10. Bekannte Grenzen und offene Punkte

Ehrliche Liste dessen, was heute fehlt oder schwächer ist, als man annehmen könnte:

1. **Head-Anchoring des Audit-Logs.** Ohne externen Anker bleibt das Kürzen am Ende der Kette eine
   Warnung statt eines Beweises (Abschnitt 8). Externes Anchoring ist Post-MVP.
2. **HTTP/2 zum Upstream.** In M1 wird auf HTTP/1.1 gezwungen. Werkzeuge, die zwingend h2 brauchen
   (gRPC über TLS), scheitern sichtbar mit `PROXY_007`. Das ist ein dokumentierter Fehlschlag, kein
   stiller Rückfall.
3. **WebSocket-Frames werden nicht gehalten.** Nach einem freigegebenen Upgrade fließen Frames und
   werden aufgezeichnet, aber nicht einzeln bestätigt. Wer das nicht will, blockt das Upgrade.
4. **Antworten werden nicht gehalten.** Der MVP moderiert Anfragen. Antworten werden gestreamt und
   aufgezeichnet; ein bösartiger Server kann also Inhalte an den Agenten liefern, die als Prompt
   Injection wirken. Genau deshalb ist die Ausgangsrichtung streng.
5. **Listener des Proxys.** Der Session-Socket ist ein Unix-Socket. Ob `hudsucker` ihn direkt
   annimmt, klärt ein Spike zu Beginn von HUM-015; fällt er negativ aus, wird die Accept-Schleife
   auf `UnixListener` geforkt. Ein Loopback-TCP-Port auf dem Host entsteht in keinem der beiden
   Fälle (`CONVENTIONS.md` 4.10). Offen ist nur der Ausgang des Spikes, nicht die Angriffsfläche.
6. **Kein wiederholter Isolations-Check.** Die drei Prüfungen laufen beim Start. Ein regelmäßiger
   Nachlauf während der Sitzung ist Post-MVP.
7. **Vollständige Aufzeichnung ab M1+.** Der Satz „alles wird aufgezeichnet" gilt, sobald der
   Recorder existiert (HUM-026). Vorher zeigt Humanitl den Verkehr, speichert ihn aber nicht
   dauerhaft.
8. **Rücktausch von Pseudonymen** funktioniert im MVP für nicht gestreamte Text-Antworten
   (HUM-079). Bei gestreamten Antworten kann ein Pseudonym in der Anzeige stehen bleiben.
9. **Nur eine Sandbox-Technik.** bubblewrap ist die einzige Implementierung im MVP. Docker- und
   microVM-Backends sind vorgesehen, aber nicht bewertet — die Architektur hält den Platz frei
   (`SandboxBackend`), mehr nicht.
10. **`ProtectHome` in der systemd-Unit** ist noch nicht abschließend festgelegt (Abschnitt 5).
## 11. Ausdrücklich außerhalb des Geltungsbereichs

Humanitl beansprucht nicht, gegen Folgendes zu schützen. Wer das braucht, braucht andere Mittel:

- **Einen Angreifer mit root auf dem Host** oder einen Kernel-Exploit, der Namespaces oder seccomp
  aushebelt. Darunter liegt keine Verteidigung mehr.
- **Physischen Zugriff**, entsperrte Sitzungen, Cold-Boot-Angriffe, Speicherabbilder.
- **Manipulierte Humanitl-Pakete.** Integrität vor dem Start ist Sache der Paketsignatur.
- **Den Menschen, der freigibt.** Humanitl macht die Entscheidung informiert; es trifft sie nicht
  und überstimmt sie nicht.
- **Den Sprachmodell-Host.** Er sieht, was der Agent ihm zeigt (Abschnitt 3.1).
- **Was der Agent in `/work` schreibt** und ein Mensch später selbst weitergibt (Abschnitt 3.2).
- **Mikroarchitektur- und Timing-Seitenkanäle** (Spectre-Klasse, Cache-Timing).
- **Verfügbarkeit.** Ein blockierter Agent ist der gewünschte Ausgang, kein Fehler. Es gibt keinen
  Modus, in dem Humanitl im Zweifel durchlässt.
- **Andere Nutzer auf derselben Maschine mit denselben Rechten.** Datei- und Keyring-Schutz gelten
  gegenüber anderen Konten, nicht gegenüber Prozessen des eigenen Kontos.
- **Netzwerkangriffe auf den Host selbst** (Firewall, VPN, Systemhärtung sind nicht Teil dieses
  Programms).

---

## 12. Schwachstellen melden

Wer ein Sicherheitsproblem findet, meldet es bitte **nicht** als öffentliches Issue. Kontakt,
Fristen und Zusagen sind hier Vorschläge des Entwurfs; verbindlich werden sie mit HUM-059.

- **Kontakt:** `security@<projekt-domain>` — Platzhalter; die endgültige Adresse und der
  PGP-Fingerprint werden mit HUM-059 gesetzt und hier eingetragen. Bis dahin ist der Weg ein
  privater Security-Advisory im Repository (GitHub: „Security" → „Report a vulnerability").
- **Was hilft:** die betroffene Version, das Sandbox-Profil (`humanitl sandbox argv`), eine
  minimale Reproduktion und, wenn möglich, die Beobachtung als Escape-Test-Probe im Stil von
  `tests/escape/`.
- **Antwort:** Eingangsbestätigung binnen 72 Stunden, Einschätzung binnen sieben Tagen.
- **Frist:** koordinierte Offenlegung nach 90 Tagen oder nach Verfügbarkeit eines Fixes, je
  nachdem, was früher eintritt. Wer früher veröffentlichen will, sagt das bitte in der Meldung.
- **Kein Bug-Bounty.** Das ist ein Einzelprojekt unter GPL-3.0. Anerkennung in der
  Änderungshistorie gibt es gern und namentlich, Geld nicht.

Sicherheitsrelevante Änderungen an Shim, Mount-Allowlist, seccomp-Filter, Passthrough oder
Audit-Kette bekommen eine Zeile in der Änderungshistorie von
[`THREAT-MODEL.md`](THREAT-MODEL.md) Abschnitt 7 und, sobald das Projekt veröffentlicht ist, einen
Eintrag im Sicherheits-Changelog dieses Dokuments.
