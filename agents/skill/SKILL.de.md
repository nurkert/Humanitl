<!-- Die deutsche Fassung des Skills `humanitl` (HUM-364). Sie ist eine
     Übersetzung von SKILL.en.md, keine eigene Fassung: dieselben Aussagen,
     dieselbe Reihenfolge, dieselben Platzhalter, dieselben Blöcke. Der Name
     im Kopf bleibt `humanitl`, die Beschreibung ist übersetzt; nach ihr
     entscheidet das Modell, ob es den Skill lädt. Kein Wort über einen
     Menschen, eine Freigabe oder ein Prüfen durch jemanden. Wer eine Aussage
     ändert, ändert beide Dateien. Im Text eines Kommentars darf kein
     Kommentar-Ende stehen, und im Text keine geschweifte Klammer außer in den
     Platzhaltern. -->
---
name: humanitl
description: "Wie der Netzzugang in dieser Humanitl-Sandbox funktioniert. Lade ihn, sobald eine Netzwerkanfrage langsam ist, hängt, scheitert, abgelehnt oder blockiert wird (403, 502, 503, 504, Blocked by Humanitl.), und vor netzlastiger Arbeit wie Downloads, Paketinstallationen, API-Aufrufen oder dem Abruf von Webseiten."
---

# Humanitl: Netzzugang in dieser Sandbox

Jede HTTP(S)-Anfrage verlässt diese Sandbox über den Humanitl-Proxy der Umgebung (`HTTPS_PROXY`, `HTTP_PROXY`); einen anderen Weg hinaus gibt es nicht. Der Proxy wendet die Regeln dieser Umgebung auf jede Anfrage an:

- Erlaubt: sie geht sofort hinaus.
- Nicht erlaubt: sofort `403`.
- Weder noch: siehe den nächsten Abschnitt.

Aufrufe an dein Modell unter {llm_host} erlaubt eine mitgelieferte Regel, auf den Pfaden, für die sie eingestellt ist, und sie warten hier nie; ist das Modell langsam, ist der Modellserver langsam, nicht Humanitl.

{ask_mode}

## Eine Antwort von Humanitl lesen

Ein Rumpf, der mit `Blocked by Humanitl.` beginnt, kommt vom Proxy, nicht vom Server. Seine Zeilen sind `reason:`, `flow:`, `host:` und manchmal `note:`, die die Antwort erklärt.

- `403`: das Ziel oder die Anfrage ist in dieser Umgebung nicht erlaubt, oder die Anfrage trug ein Geheimnis oder ging an eine private Adresse. Nicht noch einmal senden.
- `504` mit `reason: timeout`: keine Erlaubnis wurde rechtzeitig erteilt; siehe den Abschnitt oben.
- `502` mit `reason: upstream_dns`, `upstream_connect`, `upstream_tls` oder `upstream_timeout`: Humanitl hat die Anfrage durchgelassen, und das Ziel ist gescheitert. Das ist ein echter Fehler dieses Hosts.
- `502` mit `reason: upstream_private_address:` und einer Adresse: der Name löst auf eine private Adresse auf, und die ist hier nicht erlaubt. Nicht noch einmal senden.
- `503` mit `reason: hold_max_flows` oder `hold_memory`: zu viele Anfragen oder zu viele Daten warten gleichzeitig. Warte die offenen ab und sende weniger auf einmal.
- Jeder andere Status oder Grund in einem solchen Rumpf (etwa `408`, `413` oder `502` mit `reason: no_route`): die Anfrage nicht unverändert noch einmal senden; lies die `note:`-Zeile, wenn es eine gibt.

## Humanitl fragen

`http://humanitl.internal/` beantwortet der Proxy selbst:

- `curl -s http://humanitl.internal/` zeigt diese Sitzung (`timeout=`, `llm=`) und, was erlaubt ist, als Regeln; der erste Treffer gewinnt.
- `curl -s http://humanitl.internal/why/FLOW`, mit dem Wert aus `flow:` einer Antwort für FLOW, nennt `decision=`, `reason=` und `note=` dieser Anfrage, wenn sie bekannt ist.
- `curl -s --data 'eine Zeile: welches Ziel du brauchst und warum' http://humanitl.internal/ask` beantragt Zugang zu einem Ziel und antwortet sofort `202`; Zugang kann später folgen. Er gewährt von sich aus nichts. Höchstens zehn je Minute; darüber `429` mit einem Header `retry-after`.

## Wenn etwas abgelehnt wird

Sag dem Nutzer in ein oder zwei Sätzen, welcher Host nicht erreichbar war, die `note:`, wenn es eine gibt, und was die Aufgabe sonst erledigen könnte: ein Host, der schon erlaubt ist, eine Datei, die der Nutzer bereitstellen kann, oder der Zugang, der nötig wäre. Eine Anfrage über ein anderes Tool, einen anderen Host oder einen Spiegel ist eine neue Anfrage, und dieselben Regeln gelten für sie.

<!-- ask_mode: ui -->
## Hohe Latenz ist hier normal

Netzwerkanfragen haben hier hohe und schwankende Latenz. Eine Anfrage, die noch keine Regel erlaubt oder ablehnt, kann bis zu {timeout}s warten; ist bis dahin keine Erlaubnis erteilt, endet sie mit `504`. Eine erlaubte Anfrage dauert so lange, wie der Server braucht; eine abgelehnte endet sofort mit `403`. Solange eine Anfrage wartet, kommt keine Antwort. Das ist in dieser Umgebung normal, kein Ausfall.

- Weiter warten. Großzügige Fristen über {timeout}s setzen: `curl --max-time {max_time}`, und ebenso für die Frist des Tool-Aufrufs, der ihn ausführt. `curl` ohne `--max-time` wartet von selbst.
- Ein Tool, das früher aufgibt, etwa ein Abruf-Tool mit fester kurzer Grenze, schließt die Verbindung und beendet die Anfrage; eine neue Anfrage beginnt das Warten von vorn. Für eine Anfrage, die lange dauern kann, nimm `curl`.
- Sende jede Anfrage einmal. Nicht in einer Schleife wiederholen, nicht dieselbe Anfrage zweimal parallel starten und nicht zu einem anderen Tool oder Spiegel wechseln, um der Latenz auszuweichen: jedes davon ist eine Anfrage mehr, die ebenso lange wartet.
- Schließe nicht auf ein ausgefallenes Netz, weil eine Anfrage lange dauert. Braucht die Aufgabe ein Ziel, das mit `504` endete, noch, beantrage einmal Zugang mit `POST /ask` und sende die Anfrage später noch einmal.
<!-- ask_mode: none -->
## Anfragen warten hier nicht

Nur was die Regeln dieser Umgebung erlauben, geht durch, sofort; nichts wartet: eine abgelehnte Anfrage bekommt sofort `403`, jede andere `504` (`reason: timeout`). Ein `504` heißt: nicht erlaubt, kein Ausfall und kein langsames Netz; dieselbe Anfrage gleich noch einmal bekommt dieselbe Antwort. Braucht die Aufgabe ein Ziel, das mit `504` endete, noch, beantrage einmal Zugang mit `POST /ask` und sende die Anfrage später noch einmal. Sag dem Nutzer, welcher Host und Pfad nicht erreichbar waren. Eine langsame Antwort auf eine erlaubte Anfrage kommt vom Server, nicht von Humanitl.
