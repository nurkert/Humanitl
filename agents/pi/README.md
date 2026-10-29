# The bundled pi contribution

This directory holds what the `PiAdapter`
(`daemon/crates/sandbox/src/agent/pi.rs`, templates rendered in
`pi_config.rs`) writes into the sandbox (HUM-274). The files are compiled in
with `include_str!`; nothing is read from here at run time and nothing is
downloaded.

| File | Where in the sandbox | What for |
|---|---|---|
| `models.json.tmpl` | `~/.pi/agent/models.json` | The one custom provider `humanitl-local` that points pi at `llm.endpoint` |
| `settings.json.tmpl` | `~/.pi/agent/settings.json` | Selects that provider and its first model, switches off install telemetry; lists `pi-web-access` under `packages` when it is loaded |
| `web-search.json.tmpl` | `~/.pi/agent/web-search.json` | Only with `pi-web-access`: lets the proxy resolve host names (`ssrf.trustEnvProxy`) |

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

## No permission prompts to switch off

pi 1.4.0 does not ask before a tool call; its own `docs/security.md` says so
("it does not ask for approval before every tool call"), and its tools carry no
approval step. Inside the sandbox Humanitl is the one place that asks
(HUM-308), so `settings.json` has nothing to switch off. It also leaves
`defaultProjectTrust` unset: the trust prompt for a project's own extensions
and settings stays with the human.

The `bash` tool has no default timeout; a `curl` in it waits as long as the
proxy holds the request, unless the model passes its own timeout. pi has no web tool of its own.

## Web search and fetch (HUM-309)

The npm package `pi-web-access` adds `web_search`, `fetch_content`,
`source_check` and `get_search_content`. Install it globally, next to pi:

```sh
sudo npm install -g --ignore-scripts pi-web-access
```

`pi install npm:pi-web-access` writes to `~/.pi/agent` on the host, which the
sandbox does not see. The adapter looks in the `node_modules` directory that
holds pi, then in `/usr/local/lib/node_modules` and `/usr/lib/node_modules`,
and takes a directory only when its `package.json` names `pi-web-access`. It
then adds the directory as a local package to `settings.json`
(`"packages": ["/usr/local/lib/node_modules/pi-web-access"]`); pi loads a
local package in place and installs nothing, also with `PI_OFFLINE=1`.

`agent.pi.web_access` decides: `auto` (default) loads the package when it is
there, `on` also warns with `AGENT_007` when it is not, `off` never loads it.

`web-search.json` sets `ssrf.trustEnvProxy` to `true`. The extension checks
every target with a DNS lookup of its own before it fetches; the sandbox has
no resolver, and without the switch every fetch ends with
`Failed to resolve <host>`. With it the proxy resolves the name, as for every
other client. Literal private addresses and `localhost` stay refused by the
extension. The file is read-only, so the extension's commands that save a
provider choice fail in the sandbox.

Measured with pi 1.4.0, `pi-web-access` 0.46.0 and a logging proxy in
`HTTPS_PROXY`, `NO_PROXY` empty as in the sandbox: pi loaded the package from
the path, offered the four tools, `fetch_content` arrived at the proxy as
`CONNECT example.invalid:80` and returned the page, keyless `web_search` as
`CONNECT mcp.exa.ai:443`. The requests go through pi's own `EnvHttpProxyAgent`
dispatcher, which pi installs as the global `fetch`.

Keyless search uses `mcp.exa.ai`, which the bundled rule `…0006` blocks for
every session; a rule of the user for that host wins. API keys for search
providers are not handed into the sandbox.
