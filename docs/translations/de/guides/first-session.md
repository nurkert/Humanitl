---
source: guides/first-session.md
source-sha256: 5b5bb1f9b878e4ed5690f248d44f469b975d64c17b47650eeb4537df889fb0ec
lang: de
---
# Deine erste Session

Applies to: Humanitl 0.1

Diese Anleitung führt durch eine erste Session in der Desktop-App, vom Bildschirm
„Einrichten“ bis zu einer verifizierten Audit-Kette. Sie setzt eine fertige
[Installation](install.md) voraus: Der Daemon antwortet auf `humanitl daemon status`,
`humanitl sandbox check` zeigt drei `✓`, und OpenCode ist dort installiert, wo die
Sandbox es findet. Außerdem brauchst du in deinem Netz einen Modellserver mit einer
OpenAI-kompatiblen API, etwa Ollama, llama.cpp oder vLLM.

Das Beispielprojekt ist ein Ordner `~/demo-project`, der Modellserver ist
`http://192.168.1.50:11434`, und der Agent wird `registry.npmjs.org` und `example.com`
erreichen wollen. Jeder Schritt hat am Ende eine Entsprechung auf der Kommandozeile, in
[Ohne die App](#without-the-app).

Die Bildschirmfotos stammen aus den Golden-Tests der App, deshalb ist jeder Text darin
als grauer Block gezeichnet. Sie zeigen die Anordnung, nicht die Wörter; die Wörter
stehen zitiert im Text.

## 1. Den Bildschirm „Einrichten“ öffnen

Starte die App aus dem Menü („Humanitl“) oder mit `humanitl-app`. Beim ersten Start
öffnet sie den Bildschirm „Einrichten“; später ist er der letzte Eintrag der
Seitenleiste, „Einrichten“ (`Ctrl+6`).

![Der Bildschirm „Einrichten“ mit seinen vier Zeilen und dem Knopf „Agent starten“](../img/setup-checklist.png)

„Vier Dinge müssen stimmen, bevor ein Agent läuft.“ Jede Zeile ist die Antwort des
Daemons über diesen Rechner:

| Zeile | Wird grün, wenn |
|---|---|
| Hintergrunddienst | der Daemon antwortet. Ohne ihn bietet die Zeile „Installieren und starten“ an. |
| Dein Modellserver | der Endpunkt angesprochen wurde und geantwortet hat. |
| Projektordner | ein Ordner gewählt ist und der Daemon zustimmt, ihn einzuhängen. |
| Sandbox-Prüfung | jede Vorbedingung der Sandbox erfüllt ist: bubblewrap, User-Namespaces, seccomp, Laufzeitverzeichnis. |

Eine Zeile zeigt „in Ordnung“, „wissenswert“, „nicht gemessen“ oder „sperrt den Start“.
Eine Zeile, die nicht „in Ordnung“ ist, trägt eine Karte mit dem Grund und, wo es eine
gibt, einen Knopf für die Abhilfe, etwa „Befehl kopieren“. Der Knopf „Session starten“
unten bleibt aus, bis alle vier Zeilen grün sind, und die Zeile darunter nennt die
Zeile, die im Weg steht. Die vierte Zeile hat „Erneut prüfen“ und listet die Zeilen des
Doctors über diesen Rechner auf; es sind dieselben Zeilen, die `humanitl doctor`
ausgibt.

## 2. Den Modellserver testen

Ist schon ein Modellserver in der Konfiguration gespeichert, spricht „Einrichten“ ihn
beim Öffnen einmal von selbst an, mit dem hinterlegten Zugangsschlüssel, und erneut bei
„Erneut prüfen“ oder wenn sich die gespeicherte Adresse ändert. Sonst spricht es nichts
an, und gar nichts, solange keine Adresse gespeichert ist. Andernfalls tippst du die
Adresse in „Adresse des Modellservers“ und drückst „Ansprechen“ (oder `Enter`). Tippen
allein spricht nichts an; der Modellserver ist die eine Adresse, die Verkehr erreicht,
ohne durch die Warteschlange zu gehen, deshalb wird er nur angesprochen, wenn du darum
bittest. Die Zeile sagt dann zum Beispiel
„http://192.168.1.50:11434 hat mit 3 Modellen in 40 ms geantwortet“ und listet die
Modelle auf.

Du kennst die Adresse nicht? „Suchen“ durchsucht das lokale `/24` nach Modellservern auf
den Ports 11434, 1234, 8000 und 8080, schickt Verbindungsversuche und sonst nichts, und
bietet neben jedem Server, der geantwortet hat, „Übernehmen“ an. Ollama lauscht nur auf
localhost, solange `OLLAMA_HOST=0.0.0.0` nicht gesetzt ist.

Das Ansprechen speichert den Server nicht. Öffne die Einstellungen mit `Ctrl+,`
(„Einstellungen“), Gruppe „Modell“:

1. „Endpunkt“: dieselbe Adresse. Sie wird in `config.toml` geschrieben, wenn das Feld den
   Fokus verliert oder bei `Enter`; „Prüfen“ spricht sie erneut an.
2. „Modell“: das Modell, das der Agent nutzen soll, zum Beispiel `qwen2.5-coder:14b`.
   Nach einer Prüfung trägt ein Klick auf ein gelistetes Modell es ein.
3. „Zugangsschlüssel“: nur, wenn der Server einen verlangt. Er kommt in den
   Schlüsselbund des Systems und nie in `config.toml`.

Wie das Einstellungsblatt sagt, werden Änderungen in `config.toml` geschrieben und
wirken beim nächsten Start des Daemons. Starte ihn einmal neu, während keine Session
läuft:

```sh
systemctl --user restart humanitld.service
```

Die App verbindet sich von selbst neu. Zurück auf dem Bildschirm „Einrichten“ sprichst du
den Server noch einmal an, damit seine Zeile grün wird.

## 3. Das Projekt wählen

Drücke in der Zeile „Projektordner“ auf „Projektordner wählen“ und wähle
`~/demo-project`. Das Wählen startet nichts: Die App speichert den Ordner als
`sandbox.work_dir` in deiner `config.toml` und fragt den Daemon, was ein Start damit
einhängen würde, und die Zeile wird grün, wenn der Daemon zustimmt. Du wählst ihn
einmal: `humanitl run` und `humanitl sandbox run` nutzen denselben Ordner aus jedem
Verzeichnis, und `--work DIR` wählt für einen einzelnen Aufruf einen anderen.
„Der Agent darf“ schaltet zwischen „lesen und schreiben“ (die Vorgabe) und „nur lesen“
um.

Der Ordner ist das Einzige von dir, was der Agent sieht, als `/work`. Der Daemon weist
Ordner zurück, die er nicht einhängt, etwa dein Heimatverzeichnis selbst oder einen Pfad
außerhalb davon (`SANDBOX_006`), und speichert sie nicht; die Karte unter der Zeile
bietet dann „Anderen Ordner wählen“ an. Einige Dateien im Projekt sind absichtlich vor
dem Agenten verborgen, etwa `.env` und `.git/config`; der Bildschirm „Sandbox“ listet sie
unter „Einhängungen“ auf.

## 4. Starten

Mit vier grünen Zeilen drückst du „Agent starten“. Der Daemon baut die Sandbox, startet
OpenCode darin, misst die drei Isolationsgarantien, und die App wechselt zu „Anhalten“
(`Ctrl+1`). Scheitert der Start, bleibt die App auf dem Bildschirm „Einrichten“, und die
Zeile trägt den Befund, damit du siehst, warum.

Die Kopfzeile zeigt jetzt „Anhalten AN“, die Zahl der angehaltenen Anfragen und in der
Statusleiste „3/3 Isolationsprüfungen bestanden“.

## 5. Die Isolationsprüfung lesen

Öffne „Sandbox“ (`Ctrl+4`). Die obere, größere Hälfte ist das Terminal des Agenten; hier
sprichst du mit OpenCode. Die untere Hälfte hat vier Reiter: „Einhängungen“, „Umgebung“,
„Isolation“ und „Protokoll“. Öffne „Isolation“.

![Der Bereich „Isolation“ mit drei belegten Garantien](../img/isolation-panel.png)

„Jede Zeile darunter wurde in der laufenden Sandbox gemessen, nicht auf diesem Rechner,
und trägt den Beleg dazu.“

| Zeile | Was sie belegt |
|---|---|
| „Kein Netzwerk-Interface. Es gibt keinen Weg nach draußen.“ | Die Sandbox hat nur ein Loopback-Gerät; es gibt keinen Weg hinaus. |
| „Genau eine Tür: ein Socket, der zu Humanitl führt.“ | Der einzige Socket, den der Agent erreichen kann, ist der Proxy-Socket. |
| „Der Kernel öffnet keine neue Tür (seccomp).“ | Der seccomp-Filter lässt den Agenten nur TCP-Stream-Sockets öffnen, die ohne Netzwerk-Interface nichts erreichen außer der Proxy-Bridge auf Loopback. |

Jede Zeile muss „belegt“ sagen. „gilt nicht“ stoppt die Session; „nicht gemessen“ ist
auch kein Bestehen. Darunter nennt der Bereich die eine erklärte Ausnahme, den
Modellserver: „Ausnahme: LLM unter http://192.168.1.50:11434, Passthrough, geloggt, nie
angehalten.“ „Versuche an der einen Tür vorbei“ zählt, was der Agent außerhalb von HTTP
versucht und der Filter verweigert hat. „Den genauen Sandbox-Befehl anzeigen“ öffnet die
vollständige bubblewrap-Kommandozeile, aus der jede Tabelle auf diesem Bildschirm
gelesen wird.

Was die drei Garantien bedeuten und was sie nicht abdecken, steht in
[SECURITY.md](../SECURITY.md) und [THREAT-MODEL.md](../THREAT-MODEL.md).

## 6. Über die erste angehaltene Anfrage entscheiden

Gib OpenCode in seinem Terminal eine Aufgabe, zum Beispiel „Fetch https://example.com and
summarise it“. OpenCode fragt nicht in seiner eigenen Oberfläche, bevor es eine Seite
abruft oder einen Befehl ausführt; dafür ist Humanitl die eine Stelle, die fragt
(HUM-308). Die Anfrage verlässt den Agenten, erreicht Humanitl und wird angehalten.
Sehr oft kommt die erste angehaltene Anfrage sogar früher: OpenCode installiert beim
ersten Gebrauch sein Provider-Paket von `registry.npmjs.org`, und eine mitgelieferte
Regel hält diese Anfrage an, damit du sie siehst.

Eine angehaltene Anfrage erscheint in „Anhalten“, und eine Desktop-Benachrichtigung
erscheint, wenn das Fenster nicht im Vordergrund ist.

![Der Bildschirm „Anhalten“ mit drei angehaltenen Anfragen](../img/intercept-held.png)

Die linke Spalte ist die Warteschlange, die mittlere die Karte der Anfrage, die rechte
der Domain-Bereich. Die Karte sagt, warum die Anfrage angehalten wurde: „Angehalten:
keine Regel · Vorgabe: fragen“ oder „Angehalten: Regel … sagt fragen“ für eine
mitgelieferte Nachfrage-Regel. Darüber ein Countdown: Läuft er ab, wird die Anfrage
blockiert, und der Agent bekommt `504` (`hold.timeout_secs`, als Vorgabe 300 Sekunden).
Beim ersten Mal erklärt ein Hinweis den Bildschirm: „Diese Anfrage wartet auf dich“.

| Aktion | Taste | Wirkung |
|---|---|---|
| „Senden“ | `Enter` oder `A` | sendet die Anfrage unverändert; die Antwort wird aufgezeichnet, nicht angehalten |
| „Blockieren“ | `B` | der Agent bekommt `403` und darf es erneut versuchen |
| Notiz | `N` | eine Notiz für den Agenten; „Mit Notiz blockieren“ schickt sie im Rumpf der Blockade mit |
| „Bearbeiten + Senden“ | `E` | öffnet den Editor (Schritt 8) |
| nächste und vorige Anfrage | `J` und `K` | |

Sende die Anfrage. Die Karte sagt „Gesendet an example.com · …“, und der Agent bekommt
seine Antwort.

## 7. Eine Regel anlegen

Eine Entscheidung, die du wieder treffen wirst, gehört in eine Regel. Es gibt zwei Wege.

**Aus der Entscheidung.** Der Pfeil neben „Senden“ (oder `R`) öffnet „Merken“: „Wie
lange die Regel gilt“ („Einmal“, „Session“, „1 Std.“, „Immer“) und „Wofür die Regel
gilt“ („URL“, „Host“, „Domain“, „Host + Methode“). Wähle „Session“ und „Host + Methode“,
bevor du ein `GET` an `registry.npmjs.org` sendest, und jedes weitere `GET` dorthin ist
ohne Nachfrage erlaubt, bis die Session endet. „Immer“ fragt erst nach („Eine dauerhafte
Regel für registry.npmjs.org speichern?“) und schreibt die Regel in `rules.yaml`. Direkt
nach dem Speichern entfernt „Rückgängig“ sie wieder.

**Im Bildschirm „Regeln“.** Öffne „Regeln“ (`Ctrl+3`) und drücke „Neue Regel“
(`Ctrl+B`).

![Der Bildschirm „Regeln“ mit gespeicherten Regeln und der mitgelieferten Gruppe](../img/rules-list.png)

Fülle „Aktion“ (`allow`), „Host“ (`registry.npmjs.org`), „Methoden“ (`GET`, `HEAD`),
„Gültigkeit“ („immer“) und eine „Notiz“ aus. `*` in einem Host steht für ein Label, `**`
für eines oder mehrere: `**.example.com` passt auf `example.com` und jede Subdomain.
Während du tippst, zeigt „Probelauf“, auf wie viele der zuletzt aufgezeichneten Anfragen
die Regel gepasst hätte. „Speichern“ fügt sie hinzu.

Die Liste hat zwei Reiter, „Gespeichert“ und „Temporär“ (Regeln für diese Session). Die
Reihenfolge, in der Regeln ausgewertet werden, steht auf dem Bildschirm: zuerst temporäre
Regeln, dann gespeicherte, dann die mitgelieferten, „Sonst · Nachfragen“. Die
mitgelieferten Regeln kommen mit Humanitl und blockieren, was OpenCode von sich aus
ansprechen würde; du kannst eine abschalten oder eine eigene Regel darüber setzen, aber
nicht löschen. Die vollständige Regelsprache steht in der
[Regel-Referenz](../reference/rules.md).

## 8. Eine Anfrage bearbeiten, bevor sie hinausgeht

Manchmal ist eine Anfrage im Grunde richtig, trägt aber etwas, das sie nicht tragen
soll, etwa ein Token, eine E-Mail-Adresse oder einen Dateipfad. Wähle sie in der
Warteschlange aus und drücke `E` („Bearbeiten + Senden“).

Der Editor zeigt zwei Hälften, „So gesendet“ und „Geht hinaus“, mit den Reitern „Rumpf“,
„Kopfzeilen“ und „Query“. Funde sind hervorgehoben; „Nächstes“ springt zum nächsten,
und „Alle durch Pseudonyme ersetzen“ ersetzt jeden Fund durch einen stabilen Platzhalter,
aufgelistet unter „Zuordnung“. Methode, Pfad und Kopfzeilen lassen sich ändern, das Ziel
nicht („Die Zieladresse lässt sich nicht ändern“), und Kopfzeilen, die Humanitl setzt,
sind gesperrt. „Editierte Version senden“ sendet die rechte Hälfte; „Verwerfen“ geht
zurück zur Karte. Der Verlauf behält beide Fassungen.

## 9. Den Verlauf ansehen

Öffne „Verlauf“ (`Ctrl+2`). Jede Anfrage dieser und früherer Sessions steht dort mit
ihrem Zustand, Methode, Host, Pfad, Status, Größe, Dauer, Funden und „Entschieden von“
(„manuell“, „Regel …“, „Zeit abgelaufen“, „durchgereicht“).

![Die Tabelle des Verlaufs](../img/history-table.png)

Der Filter nimmt Begriffe wie `host:registry.npmjs.org decision:block
since:1h`; die Chips „Angehalten“, „Blockiert“, „Mit Funden“ und „Editiert“ setzen
häufige davon. Verkehr zum Modellserver ist als Vorgabe ausgeblendet („LLM-Verkehr
ausgeblendet“). Wähle eine Zeile aus, um die Anfrage, die Antwort und, bei einer
bearbeiteten Anfrage, die bearbeitete Fassung zu sehen. „Exportieren“ schreibt die
Auswahl oder die gefilterten Anfragen in eine Datei; die Datei enthält Hosts,
vollständige Pfade, alle Kopfzeilen und beide Rümpfe im Klartext.

## 10. Die Audit-Kette verifizieren

Öffne „Audit“ (`Ctrl+5`) und drücke „Jetzt prüfen“. Der Daemon prüft jeden Eintrag der
Audit-Kette gegen seinen Vorgänger, seinen MAC und die Anker, und der Status wechselt zu
„Verifiziert“, mit der Zahl der Einträge und Anker und der Zeit des letzten Ankers.
„Kette gebrochen ab Sequenz …“ nennt den ersten Eintrag, der nicht passt, und warum.

Die Tabelle darunter listet die Einträge auf, etwa Entscheidungen und Regeländerungen,
mit einem Filter nach Art, Session und Zeit. „Exportieren“ schreibt sie als JSONL
(vollständig) oder CSV (Übersicht). Was die Kette belegt und was nicht, steht in
[SECURITY.md](../SECURITY.md).

Wenn du fertig bist, beendet „Stoppen“ auf dem Bildschirm „Sandbox“ die Session; Dateien
in `/work` bleiben.

## Ohne die App

Jeder Schritt hat einen Befehl. App und Kommandozeile sprechen mit demselben Daemon und
sehen dieselbe Warteschlange, dieselben Regeln und denselben Verlauf.

| Schritt | Befehl |
|---|---|
| Den Rechner prüfen | `humanitl doctor` |
| Einen Modellserver finden | `humanitl llm discover` |
| Ihn testen | `humanitl llm test http://192.168.1.50:11434` |
| Ihn speichern | `humanitl config set llm.endpoint http://192.168.1.50:11434` |
| Das Modell wählen | `humanitl config set llm.models '["qwen2.5-coder:14b"]'` |
| Einen Zugangsschlüssel hinterlegen | `humanitl llm key set` (liest den Schlüssel von stdin) |
| Im Projekt starten | `cd ~/demo-project && humanitl run` |
| Isolationsprüfung | `humanitl run` gibt die drei Garantien aus, bevor der Agent startet; `humanitl sandbox check` misst sie für sich |
| In den laufenden Agenten tippen | `humanitl sandbox attach` (lösen mit `Ctrl+A d`) |
| Angehaltene Anfragen auflisten | `humanitl flows list state:held` |
| Eine davon ansehen | `humanitl flows show <id>` |
| Sie senden und eine Regel merken | `humanitl flows decide <id> allow --remember registry.npmjs.org --remember-method GET --remember-expires session` |
| Sie mit einer Notiz blockieren | `humanitl flows decide <id> block --note "not needed"` |
| Eine Regel hinzufügen | `humanitl rules add --action allow --host registry.npmjs.org --method GET --method HEAD --expires never --note "npm packages"` |
| Eine Regel erst ausprobieren | `humanitl rules dry-run --action allow --host registry.npmjs.org` |
| Fragen, was die Regeln zu einer URL sagen | `humanitl rules test https://registry.npmjs.org/left-pad` (Exit 0 erlauben, 10 blockieren, 11 nachfragen) |
| Verlauf | `humanitl flows list host:registry.npmjs.org` |
| Verlauf mit dem Verkehr zum Modell | `humanitl flows list --passthrough` |
| Die Audit-Kette verifizieren | `humanitl audit verify` |

Eine Anfrage zu bearbeiten, bevor sie hinausgeht, geht nur in der App.

`humanitl run` hält Anfragen als Vorgabe für die App an (`--ask ui`). OpenCode ist ein
Vollbildprogramm, deshalb wird `--ask terminal` dafür zurückgewiesen (`CLI_002`); entscheide
aus einem zweiten Terminal mit `humanitl flows decide`, oder nutze
`humanitl run --profile llm-only`, das nur das Modell durchlässt und alles andere ohne
Nachfrage blockiert. Alle Befehle stehen in der [CLI-Referenz](../reference/cli.md),
jeder Code eines Befunds in [DIAGNOSTICS.md](../DIAGNOSTICS.md) und die Grenzen für
Anfragen und Rümpfe unter [Grenzen](../reference/limits.md).
