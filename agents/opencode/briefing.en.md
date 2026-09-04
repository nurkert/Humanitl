<!-- Die englische Fassung der Einweisung (HUM-071, ADR-0014). Sie wird beim
     Start zu <config>/opencode/AGENTS.md in der Sandbox, wobei <config> das
     XDG_CONFIG_HOME ist, das der Agent wirklich sieht. Jede Aussage darin ist
     am Verhalten geprüft; wer den Text ändert, prüft sie erneut und zählt die
     Token mit tools/briefing-tokens.py. HTML-Kommentare entfernt der Renderer,
     auch mehrzeilige; ein Kommentar endet für ihn am ersten Ende-Zeichen, es
     darf deshalb keines im Text eines Kommentars stehen. Die beiden Blöcke am
     Ende der Datei gehören zu den Ask-Modi und treten an die Stelle des
     Platzhalters im ersten Absatz. -->

# Humanitl sandbox

No network interface: HTTP(S) leaves only through the proxy in the proxy environment. {ask_mode}

`Blocked by Humanitl.` in a body comes from the proxy. `403`: decided against — do not repeat it; read any `note:` line and tell the user what was refused, why, and what else could work.

The proxy answers `http://humanitl.internal/`: `GET /` lists the rules, `POST /ask` with one line asks the user (`202`). Only the user can add a rule.

One rule allows model calls to {llm_host}, nothing else on that host.

<!-- ask_mode: ui -->
Rules decide most; the rest waits up to {timeout}s for a person, then `504`. Waiting is normal: do not abort or retry.
<!-- ask_mode: none -->
Rules decide everything here; nobody is asked, and what no rule allows fails at once with `504`. Tell the user which rule is missing.
