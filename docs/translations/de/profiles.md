---
source: profiles.md
source-sha256: e635f50148d854cfd89833466bce17a2beed6e315186140e93f4b431b4749be3
lang: de
---
# Profile

Applies to: Humanitl 0.1

Session-Profile (`profiles/*.toml`, `~/.config/humanitl/profiles/*.toml` und
`<project>/.humanitl/profile.toml`) beschreibt die
[Anleitung zu Agentenprofilen](guides/agent-profiles.md#session-profiles). Diese Seite
bleibt bestehen, weil Code-Kommentare und die erzeugte Konfigurationsreferenz hierher
verweisen.

| Thema | Abschnitt |
|---|---|
| Format und die vier Schlüssel der obersten Ebene | [Format](guides/agent-profiles.md#format) |
| Die mitgelieferten Profile `default` und `llm-only` | [Mitgelieferte Profile](guides/agent-profiles.md#bundled-profiles) |
| Die sieben Schichten und `Origin` | [Reihenfolge der Schichten](guides/agent-profiles.md#order-of-layers) |
| Die Grenzen eines Projekt-Profils | [Was ein Projekt-Profil nicht darf](guides/agent-profiles.md#what-a-project-profile-may-not-do) |
| Regeln aus einem Profil und ihr Rang | [Regeln aus einem Profil](guides/agent-profiles.md#rules-from-a-profile) |
| `--profile` auf der Kommandozeile | [Auf der Kommandozeile](guides/agent-profiles.md#on-the-command-line) |
| Ein eigenes Profil | [Ein eigenes Profil schreiben](guides/agent-profiles.md#writing-your-own-profile) |

Jeder Konfigurationsschlüssel steht in [CONFIG.md](CONFIG.md).
