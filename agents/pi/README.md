# The bundled pi contribution

This directory holds what the `PiAdapter`
(`daemon/crates/sandbox/src/agent/pi.rs`, templates rendered in
`pi_config.rs`) writes into the sandbox (HUM-274). The files are compiled in
with `include_str!`; nothing is read from here at run time and nothing is
downloaded.

| File | Where in the sandbox | What for |
|---|---|---|
| `models.json.tmpl` | `~/.pi/agent/models.json` | The one custom provider `humanitl-local` that points pi at `llm.endpoint` |
| `settings.json.tmpl` | `~/.pi/agent/settings.json` | Selects that provider and its first model, switches off install telemetry |

The briefing is the shared one of `agents/opencode/briefing.*.md`, written to
`~/.pi/agent/AGENTS.md`.

## How the templates are filled

Both files are parsed as JSON and rebuilt with `serde_json`; the
`{{...}}` placeholders are values that get replaced, never text that gets
substituted. A model name with a quotation mark therefore cannot add keys.

- `baseUrl` becomes the OpenAI base URL of `llm.endpoint`, the same value
  OpenCode gets, including a path prefix. pi posts to
  `<baseUrl>/chat/completions`.
- `apiKey` becomes `!cat /etc/humanitl/pi/llm-api-key` when a key is stored,
  otherwise the placeholder `humanitl-no-key`. pi runs a value starting with
  `!` through `/bin/sh -c`; the path is a constant, never a configured value.
- The first entry of `models` is the pattern of one model. It is repeated once
  per entry of `llm.models`, with `id` and `name` set to the model id; with an
  empty list the placeholder model `default` is used.
- `defaultModel` becomes the first model.

## The limits of a model

`contextWindow` 32768 and `maxTokens` 4096 are cautious defaults, because the
daemon does not know the model behind the endpoint. If you know yours, adjust
the pattern entry and rebuild the daemon.
