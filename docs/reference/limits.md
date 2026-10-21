# Resource limits

Applies to: Humanitl 0.1

Every hard limit of the daemon lives in the group `limits` of `config.toml`
(`backlog/CONVENTIONS.md` 4.4, HUM-057). Each has a default that fits a coding
agent on a laptop, and exceeding any of them ends in a fixed state with a
diagnostic, never in a silent abort.

Names, defaults and tiers are defined by the output of `humanitl config schema`
(and the generated [`CONFIG.md`](../CONFIG.md)); this page explains what each
limit does. Values are set with `humanitl config set limits.<key> <value>` or in
the settings dialog (tiers `advanced` and `expert`).

## The eleven limits

| Key | Default | Tier | When exceeded | Diagnostic |
|---|---|---|---|---|
| `limits.hold_body_cap_bytes` (alias `hold.body_cap_bytes`) | 32 MiB | advanced | The request is blocked with `413` (`BlockReason::BodyCap`), even if a rule allows the host. Exception: an `allow` or `ask` rule with `stream: true` lets it out streamed and unscanned, but only while `hold.hard_block_checksum_secrets` is off (see below). | `LIMIT_001` (`Warning` at the block with a rule suggestion, `Info` when streaming) |
| `limits.preview_cap_bytes` (alias `preview.cap_bytes`) | 8 MiB | expert | The secret scan and the preview see only the beginning; the decision stays possible, the findings count as incomplete. | `FINDINGS_002` |
| `limits.event_buffer` (alias `ipc.event_buffer`) | 1024 | expert | A subscriber of the event stream that misses more events gets `Lagged` and reloads through `ListFlows`. The proxy never waits for it. | `LIMIT_006` (log only, `Info`) |
| `limits.max_decompress_ratio` (alias `preview.max_decompress_ratio`) | 100 | expert | Decoding a body for scan and preview stops as soon as more would come out than ratio times the packed size; checked while decoding, not afterwards. | `LIMIT_003` (no fix), `FINDINGS_002` when the preview cap hit instead of the ratio |
| `limits.hold_max_flows` | 200 | advanced | A new request that would have to be held is blocked with `503` (`BlockReason::HoldMaxFlows`); no held request is pushed out. From 80 % the status bar shows the load amber, at 100 % red. | `LIMIT_002` |
| `limits.hold_max_bytes` | 256 MiB | advanced | Sum of all bodies in progress: reserved while buffering, released after forwarding, block, timeout or abort. A request that no longer fits gets `503` (`BlockReason::HoldMemory`), also in the middle of buffering. A held head under `stream: true` counts only the prefix already read. | `LIMIT_002` |
| `limits.connect_timeout_secs` (alias `upstream.connect_timeout_secs`) | 10 s | advanced | Connecting to the destination stops; the flow becomes `Failed`, the client gets `502`. | `PROXY_003` |
| `limits.header_timeout_secs` | 30 s | expert | A client that does not send its request head in this time is disconnected; no flow is created. Also covers the gap before the next request on a keep-alive connection and the TLS handshake after `CONNECT`, never while a request is held. For a streamed upload, also the time from the end of the body to the destination's response headers. | `LIMIT_005` (log only), `LIMIT_004` (upload without answer) |
| `limits.body_timeout_secs` | 300 s | expert | Silence **between two chunks** of a body, never the total duration. Request body: `408` with `Connection: close`. Streamed response body: the stream is cut and recorded as truncated. Streamed upload the destination no longer reads: it fails. | `PROXY_011` (request), `LIMIT_004` (response, upload) |
| `limits.recorder_max_body_bytes` (alias `recorder.max_body_bytes`) | 32 MiB | expert | What lies beyond is not stored by the recording; the row carries `truncated` and the full size. | none, `truncated` mark on the body |
| `limits.max_client_connections` | 256 | expert | The accept loop answers `503` with `Connection: close` instead of accepting and leaving the connection lying. | `PROXY_010` (summarised once per minute) |

The specification of HUM-057 named three keys differently
(`limits.hold_memory_cap_bytes`, `limits.upstream_connect_timeout_secs`,
`limits.client_header_timeout_secs`) and two more
(`limits.client_body_timeout_secs`, `limits.response_idle_timeout_secs`) that
HUM-120 merged into `limits.body_timeout_secs`. The names in this table apply
(`backlog/CONVENTIONS.md` 4.4).

## `stream: true`

The request body is fully buffered before any decision, because otherwise a
human does not see exactly the part where data leaves. A request above
`limits.hold_body_cap_bytes` does not fit into that buffer and is blocked. A
rule with `stream: true` is the only exception, and the human writes it
themselves:

```yaml
- action: ask          # or allow
  match:
    host: uploads.example.com
    method: [POST]
    path_prefixes: ["/v1/files"]
  stream: true
```

With `ask` the proxy holds the request with its head, and only an approval lets
the body flow. With `allow` it goes at once. In both cases there is no secret
scan and no preview; the body is recorded up to
`limits.recorder_max_body_bytes`. `LIMIT_001` at the block carries the
suggestion of such a rule, always with `ask`. With
`hold.hard_block_checksum_secrets` on, `stream: true` never applies: an unscanned
stream cannot keep the hard block. The request is then blocked as without a
rule, and `LIMIT_001` suggests the cap instead of a rule. The security statement
is in [`SECURITY.md`](../SECURITY.md#6-the-proxy); the rule field in
[`rules.md`](rules.md).

## Proven by

- `daemon/crates/proxy/tests/limits.rs`: block at the cap with `LIMIT_001`,
  streaming under `stream: true`, a silent response with `LIMIT_004`, the head
  deadline with `LIMIT_005`.
- `daemon/crates/proxy/tests/hold.rs`: `hold_rejects_over_memory`,
  `hold_rejects_over_count`.
- `daemon/crates/proxy/tests/timeouts.rs`: the spans of HUM-120.
- `daemon/crates/findings/tests/regressions/main.rs`: decompression bombs,
  `LIMIT_003` versus `FINDINGS_002`.
- `daemon/crates/ipc/src/server.rs`: `subscriber_lag_gets_lagged_event`.
- `daemon/crates/proxy/tests/backpressure.rs`: 1000 flows in 10 s against a
  slow subscriber, in CI with measurements in the job log.
