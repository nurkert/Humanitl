<!-- Die deutsche Fassung der Einweisung (HUM-071, ADR-0014). Sie ist eine
     Übersetzung von briefing.en.md, keine eigene Fassung: dieselben Aussagen,
     dieselbe Reihenfolge, dieselben Platzhalter, dieselben Blöcke. Wer eine
     Aussage ändert, ändert beide Dateien und zählt die Token erneut mit
     tools/briefing-tokens.py. Im Text eines Kommentars darf kein
     Kommentar-Ende stehen; der Renderer nimmt das erste. -->

# Humanitl-Sandbox

Keine Netzwerkschnittstelle: HTTP(S) geht nur über den Proxy aus der Proxy-Umgebung. {ask_mode}

`Blocked by Humanitl.` im Rumpf kommt vom Proxy. `403`: dagegen entschieden — nicht wiederholen; lies eine etwaige `note:`-Zeile und sag dem Nutzer, was abgelehnt wurde, warum, und was sonst geht.

`http://humanitl.internal/` beantwortet der Proxy: `GET /` listet die Regeln, `POST /ask` mit einer Zeile fragt den Nutzer (`202`). Nur der Nutzer kann eine Regel anlegen.

Eine Regel erlaubt Modell-Aufrufe an {llm_host}, sonst nichts auf diesem Host.

<!-- ask_mode: ui -->
Regeln entscheiden das meiste; der Rest wartet bis zu {timeout}s auf einen Menschen, dann `504`. Warten ist normal: nicht abbrechen, nicht wiederholen.
<!-- ask_mode: none -->
Regeln entscheiden hier alles; niemand wird gefragt, und was keine Regel erlaubt, scheitert sofort mit `504`. Sag dem Nutzer, welche Regel fehlt.
