<!-- Die englische Fassung der Einweisung (HUM-071, ADR-0014). Sie wird beim
     Start zu <config>/opencode/AGENTS.md in der Sandbox, wobei <config> das
     XDG_CONFIG_HOME ist, das der Agent wirklich sieht. Jede Aussage darin ist
     am Verhalten geprüft; wer den Text ändert, prüft sie erneut und zählt die
     Token mit tools/briefing-tokens.py. HTML-Kommentare entfernt der Renderer,
     auch mehrzeilige; ein Kommentar endet für ihn am ersten Ende-Zeichen, es
     darf deshalb keines im Text eines Kommentars stehen. Die beiden Blöcke am
     Ende der Datei gehören zu den Ask-Modi und treten an die Stelle des
     Platzhalters im ersten Absatz. Seit HUM-364 erfährt der Agent nicht, dass
     ein Mensch entscheidet: lange Wartezeit ist eine Eigenschaft dieser
     Umgebung (Entscheidung des Owners vom 2026-10-31). Kein Wort über eine
     Person, eine Freigabe oder ein Prüfen durch jemanden. -->

# Humanitl sandbox

You have internet access: HTTP(S) with `curl`, `wget` etc. goes through the proxy in the environment, nothing else. {ask_mode}

`Blocked by Humanitl.` comes from the proxy. `403`: not permitted here — do not repeat it; read any `note:` line and tell the user what was not reachable, why, and what else could work.

`http://humanitl.internal/`: `GET /` lists what is permitted, `POST /ask` with one line requests access (`202`; access may follow later). It grants nothing by itself. Details: skill `humanitl`.

One rule permits model calls to {llm_host}, on its paths only; they never wait here, so delays come from the model server.

<!-- ask_mode: ui -->
Requests here have high, variable latency; one that no rule permits or refuses yet can wait up to {timeout}s, then `504`: no permission granted in time. Normal here, not an outage: keep waiting, tool timeouts above {timeout}s, no retry loops or parallel duplicates, no switching tools or mirrors, never assume the network is down. A tool that gives up sooner withdraws the request; `curl` waits.
<!-- ask_mode: none -->
Only what the rules permit goes through, at once; nothing waits: refused requests get `403`, any other `504`: not permitted, not an outage. Tell the user what was not reachable and which rule is missing.
