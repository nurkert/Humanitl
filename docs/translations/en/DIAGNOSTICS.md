---
source: DIAGNOSTICS.md
source-sha256: b27ed5a8cac42eab9cdb8fc2776eebb492581e41b676e5cad82a3a1b6216e699
lang: en
---
# Diagnostic codes

Applies to: Humanitl 0.1

<!-- Erzeugt aus daemon/crates/core-types/src/diagnostics/codes.rs.
     Nicht von Hand ändern: `UPDATE_DIAG_DOCS=1 cargo test -p humanitl-core --test diag_docs` schreibt die Datei neu. -->

Every state that is not green carries a code of the form `BEREICH_NNN`. The code appears in
the message, in the interface and in `audit.jsonl`; it is the shortest way from an
observation to its explanation. A number is never reused, not even after a code has been
removed.

## Reserved ranges

| Area | Prefix | From | To | For |
|---|---|---|---|---|
| daemon | `DAEMON` | 001 | 019 | 001-004 start, reachability and version of the daemon, 005-008 the user unit of `daemon install` (HUM-044), 009 shutdown (HUM-142), 010 user session, 011 binaries from the `AppImage`, 012 `journalctl` (HUM-070), 013 the socket handed over by systemd (HUM-053), 014 `daemon uninstall` (HUM-077) |
| ipc | `IPC` | 001 | 009 | gRPC interface, token, calls against the state |
| config | `CONFIG` | 001 | 029 | 001-006 file, keys, value ranges, 007-009 profiles (HUM-066), 010-012 test root and its flag (HUM-087), 013 the project folder of the setup (HUM-044), 017-018 editor and group keys (HUM-070), 019-020 the access key of the model (HUM-140, range extended to 029 for it), 021 the saved model servers (HUM-289) |
| sandbox | `SANDBOX` | 001 | 032 | 001-006 launcher and profile, 007 bridge direction, 010-012 start errors, 019 refused attempts not reported (HUM-138), 020-025 /work hardening (HUM-043), 030-032 disposable sandbox (HUM-350) |
| proxy | `PROXY` | 001 | 019 | requests, caps, protocol, 010-011 limits of the connection (HUM-120) |
| tls | `TLS` | 001 | 009 | CA, certificates, handshake |
| llm | `LLM` | 001 | 009 | LLM endpoint and its answers, 009 the access key (HUM-140) |
| rules | `RULES` | 001 | 019 | 001-008 rules file and patterns, 009-011 rule store (HUM-027) |
| findings | `FINDINGS` | 001 | 009 | detectors for secrets and personal data |
| catalog | `CATALOG` | 001 | 009 | bundled domain catalogue and ranking |
| terminal | `TERM` | 001 | 009 | terminal connection of the agent |
| recorder | `RECORDER` | 001 | 009 | database and blob store |
| limits | `LIMIT` | 001 | 009 | budgets and time limits |
| audit | `AUDIT` | 001 | 009 | hash chain and export |
| doctor | `DOCTOR` | 001 | 019 | self-test of the installation |
| cli | `CLI` | 001 | 009 | command line and its preconditions |
| ui | `UI` | 001 | 009 | interface and what the desktop environment refuses it |
| agent | `AGENT` | 001 | 009 | agent adapter: start command, templates, check before the start |
| edit | `EDIT` | 001 | 009 | the edited request of an `AllowEdited` decision (HUM-047) |
| hold | `HOLD` | 001 | 009 | what refuses a release even though a human wanted it: 004 a confirmed secret under `hold.hard_block_checksum_secrets` (HUM-049); 001-003 are not assigned |

## Codes

### Area daemon

#### DAEMON_001

Background service not reachable

**Trigger.** The client finds no socket or no token under the runtime path, or the connection to it fails. Or the runtime directory and the token do not belong to your own account, are a symlink or are open to group and others; then the client does not read the token (HUM-212).

**Fix.** Without a unit, `InstallService` sets it up (`humanitl daemon install`); with the unit from `daemon install`, `CopyCommand` starts it (`systemctl --user start humanitld.service`), with the package (service and socket, not from an `AppImage`) its socket (`systemctl --user start humanitld.socket`); if the package and the user's unit are both present, the latter hides the package, and `InstallService` cleans it up. `humanitld` in the foreground is the way to debug and is in the documentation, not in the suggestion (HUM-189). With open permissions, `CopyCommand` with `chmod go-rwx`; with a foreign owner or a symlink, no fix. Only in the `/tmp` fallback, `OpenUrl` to the guide on how to set your own `XDG_RUNTIME_DIR` under the home directory for the whole session (`docs/INSTALL.md`); it applies after a new login.

#### DAEMON_002

Incompatible protocol version

**Trigger.** The daemon reports a different major version of the contract than the command line speaks.

**Fix.** `CopyCommand`: bring both sides to the same version.

#### DAEMON_003

Socket already in use

**Trigger.** A daemon is already listening on the socket, or an orphaned file lies on it.

**Fix.** No fix button: the text names the running instance and the path.

#### DAEMON_004

Runtime directory or socket cannot be created

**Trigger.** The runtime directory is missing, is not private, cannot be created, or the socket path is longer than `sun_path` allows.

**Fix.** Mostly without a fix — the text names the path and the reason; for a path that is too long, `SetEnv` for a shorter `XDG_RUNTIME_DIR`. If the runtime directory belongs to another account or is a symlink (HUM-212), no fix; only in the `/tmp` fallback, `OpenUrl` to the guide on how to set your own `XDG_RUNTIME_DIR` under the home directory for the whole session (`docs/INSTALL.md`); it applies after a new login.

#### DAEMON_005

A foreign unit file is not overwritten

**Trigger.** Under the unit path lies a file whose first line does not carry the Humanitl mark, or something other than a regular file; with the package, it hides the package's unit. Or your own unit changed between the announcement and the move (then it may lie under `.bak`, the finding says so).

**Fix.** `CopyCommand` that moves the foreign file aside under a free name (`mv -n -- … .bak`); there is deliberately no `--force`.

#### DAEMON_006

Unit file cannot be written

**Trigger.** The unit file cannot be read, written or moved aside (permissions, directory, file system, no free name).

**Fix.** `CopyCommand` with the path, otherwise the reference to the documentation.

#### DAEMON_007

humanitld is not next to humanitl

**Trigger.** There is no `humanitld` next to the running command line that the unit could start — or its own path cannot be determined at all.

**Fix.** `CopyCommand` with `ls -l` on the expected path; in the second case no fix.

#### DAEMON_008

systemd did not take the unit

**Trigger.** `systemctl --user` did not take over or did not start the written unit, or the unit is loaded from a different directory than the one `humanitl` wrote it to.

**Fix.** `CopyCommand` with `systemctl --user status humanitld`, the call that says what bothers systemd; if the unit is loaded from a different directory, `systemctl --user show -p UnitPath`.

#### DAEMON_009

Tasks cancelled on shutdown

**Trigger.** On shutdown, tasks of the daemon were still running after the deadline; the process ends anyway.

**Fix.** No fix for users: the text names the deadline; a report with the log of this run helps further.

#### DAEMON_010

No systemd user session

**Trigger.** `XDG_RUNTIME_DIR` is missing, or `systemctl --user` does not find the session's bus, and the command needs the user session.

**Fix.** `CopyCommand`: `loginctl enable-linger $USER`, so that the session stays up even without a login.

#### DAEMON_011

Programs from the AppImage cannot be stored

**Trigger.** `daemon install` from an `AppImage` cannot copy the daemon, shim, catalogue and profile to `~/.local/lib/humanitl/<version>.<stempel>/` or cannot repoint `current`, or the catalogue or profile is missing in the image.

**Fix.** `CopyCommand` with `ls -ln` on the directory; the text names the reason. If the catalogue or profile is missing in the image, `OpenUrl` to the releases: only a complete `AppImage` helps.

#### DAEMON_012

journalctl not found

**Trigger.** `humanitl daemon logs` finds no `journalctl` on the `PATH`.

**Fix.** `CopyCommand`: `sudo apt-get install systemd`, the package that brings `journalctl`.

#### DAEMON_013

Handed-over socket unusable

**Trigger.** Through socket activation, systemd hands over more than one socket, a number 3 that is not open, no listening Unix stream socket, or one at a different path than `$XDG_RUNTIME_DIR/humanitl/daemon.sock`.

**Fix.** `CopyCommand`: `systemctl --user cat humanitld.socket` shows what `ListenStream` points to.

#### DAEMON_014

Uninstall incomplete

**Trigger.** `humanitl daemon uninstall` could not deregister the service or could not remove a file that `daemon install` created.

**Fix.** `CopyCommand` with the exact `systemctl` call that failed, otherwise `ls -ld` on the path that remained.

### Area ipc

#### IPC_001

Invalid token

**Trigger.** The session token is missing, unreadable or does not match the daemon's.

**Fix.** On the command line, `CopyCommand` with `humanitld`, which writes the token anew; in the daemon no fix.

#### IPC_002

Sending an edited version needs exactly one request

**Trigger.** `AllowEdited` came with no flow id or with more than one.

**Fix.** No fix: an edited request body belongs to exactly one flow.

#### IPC_003

Request no longer held

**Trigger.** The named flow is no longer waiting: decided, timed out or never held.

**Fix.** No fix: the queue shows the current state.

#### IPC_004

Invalid decision

**Trigger.** A `Decide` request contradicts itself, for example without a decision or with an unknown flow.

**Fix.** No fix: the text names the field that is wrong.

#### IPC_005

Invalid rules request

**Trigger.** A `Rules` request asks for something the rule store cannot do, for example deleting a bundled rule.

**Fix.** No fix: the text names the rule and the reason.

#### IPC_006

Not available in this daemon

**Trigger.** This daemon does not have the capability that was asked for — no sandbox, no recording, no endpoint probe; or it is shutting down and therefore starts no more sandboxes.

**Fix.** No fix: it is a statement about this daemon, not about the request. If it is shutting down, a new start helps as soon as it runs again.

### Area config

#### CONFIG_001

Invalid configuration file

**Trigger.** A configuration or profile file is not readable or is not valid TOML.

**Fix.** Mostly without a fix — the text names the file and the parser error; at the start, `ChangeSetting` to the default profile.

#### CONFIG_002

Unknown key

**Trigger.** A key, a block or a path is in the file that the schema does not know.

**Fix.** `ChangeSetting` to the most similar key, if there is one; otherwise no fix.

#### CONFIG_003

Value out of range

**Trigger.** A value lies outside its range, has the wrong type, a profile has the wrong shape, or a project profile sets a key that does not belong to it.

**Fix.** `ChangeSetting` where there is a way to write it; for the project profile deliberately none.

#### CONFIG_004

Runtime directory is a fallback

**Trigger.** `XDG_RUNTIME_DIR` is missing, and the daemon falls back to a shared directory.

**Fix.** `SetEnv` for `XDG_RUNTIME_DIR`.

#### CONFIG_005

Outdated key

**Trigger.** A key is in the file that is outdated: either dropped (`alias::RETIRED`) or written under its old name.

**Fix.** For the old name, `ChangeSetting` to today's; for the dropped one no fix — there is no successor, the line is deleted.

#### CONFIG_006

Old and new key both set

**Trigger.** The old and the new name of the same key are both in the file.

**Fix.** No fix: the text names both; the new one applies.

#### CONFIG_007

Project profile belongs to another account

**Trigger.** The project profile belongs to a different account than the one running the daemon.

**Fix.** No fix: only what a project may set still applies.

#### CONFIG_008

Your profile hides a bundled one

**Trigger.** A profile of your own hides a bundled one of the same name and differs from it.

**Fix.** `CopyCommand` to compare the two.

#### CONFIG_009

The project's profile choice does not apply

**Trigger.** The project asks for a profile it may not choose; a different one applies.

**Fix.** `CopyCommand` that sets the project's wish as a command-line switch — the decision stays with the human.

#### CONFIG_010

Test root not usable

**Trigger.** `resolver.test_ca` points to something that is not a usable root.

**Fix.** `CopyCommand` to check the file.

#### CONFIG_011

Test root and its flag do not match

**Trigger.** Test root without flag or flag without test root: the two belong together.

**Fix.** The fix names the missing half.

#### CONFIG_012

Test root without an absolute path

**Trigger.** `resolver.test_ca` is not an absolute path and would therefore depend on the daemon's start directory.

**Fix.** `ChangeSetting` to an absolute path.

#### CONFIG_013

No project folder chosen

**Trigger.** Nobody has chosen a project folder; the client reports it, not the daemon.

**Fix.** No fix button: the folder is chosen in the setup.

#### CONFIG_014

Setting cannot be changed through the daemon

**Trigger.** `SetConfig` receives a key outside the form or a variable under `sandbox.env` whose value is not the certificate in the sandbox.

**Fix.** No fix: the text names what is accepted; everything else goes into `config.toml` by hand (`humanitl config edit`).

#### CONFIG_015

config.toml not written

**Trigger.** `config.toml` could not be changed without changing more than the one value, or could not be written; it is untouched.

**Fix.** No fix: the text names the line that belongs in the block `[sandbox.env]` by hand.

#### CONFIG_016

Fixed name mappings set

**Trigger.** `resolver.overrides` answers the named names from the configuration instead of asking; the traffic goes to the address listed there.

**Fix.** `ChangeSetting` to an empty table if the fixed addresses were not intended.

#### CONFIG_017

No editor found

**Trigger.** Neither `$VISUAL` nor `$EDITOR` is set and neither `nano` nor `vi` is on the `PATH`, or the named editor does not start.

**Fix.** `SetEnv`: `EDITOR=nano`.

#### CONFIG_018

Key is a group

**Trigger.** `humanitl config set` receives a path like `hold`, under which further keys stand, instead of a single value.

**Fix.** `CopyCommand`: `humanitl config get <gruppe>` shows the keys below it.

#### CONFIG_019

Access key not in the system keyring

**Trigger.** When the access key is stored, the Secret Service does not answer, and the key lies in the `0600` fallback under `$XDG_DATA_HOME/humanitl/keys`.

**Fix.** `CopyCommand`: `sudo apt-get install libsecret-tools` brings `secret-tool`; then `humanitl llm key set` again.

#### CONFIG_020

Access key not stored

**Trigger.** `LlmKey(Set)` or `LlmKey(Clear)` receives a key that is not one (empty, shorter than 16 characters, longer than 4096 bytes, with whitespace), a request without an operation, or neither the keyring nor the file can be written or deleted.

**Fix.** For a key that is too short, `openssl rand -hex 24` to copy, for a local server with a key of your own choosing; otherwise the text names the reason, and a key without whitespace or a writable data directory fixes it.

#### CONFIG_021

Model servers not changed

**Trigger.** `LlmServers` or `humanitl llm server` receives an invalid, duplicate or unknown name, an endpoint that is not one, or is to remove the active server without a successor or during a session; `LlmKey` hits a server that changed while the key was being stored or whose reference another one carries; `SetConfig` names a server that is no longer active; or on loading there is no valid active server, a shared reference or an endpoint that does not come from the active server.

**Fix.** `CopyCommand`: `humanitl llm server ls` shows the names; when removing the active server, `--use NAME` with the successor.

### Area sandbox

#### SANDBOX_001

Sandbox tool not found

**Trigger.** `bwrap` is not on the daemon's `PATH`.

**Fix.** `CopyCommand` with the install command.

#### SANDBOX_002

Sandbox tool too old

**Trigger.** The `bwrap` found is older than the starter's minimum version.

**Fix.** `CopyCommand` with the install command.

#### SANDBOX_003

User namespaces not allowed

**Trigger.** Unprivileged user namespaces are switched off on this kernel.

**Fix.** `CopyCommand` with the `sysctl` call.

#### SANDBOX_004

Isolation check failed

**Trigger.** The isolation check failed; which of the three guarantees is said by its own code.

**Fix.** No fix: the start is aborted.

#### SANDBOX_005

Project folder not writable

**Trigger.** The project folder is missing, is not a directory, is not an absolute path, or is not writable with `rw`.

**Fix.** For the folder that is not writable, `RemountReadOnly`; otherwise no fix.

#### SANDBOX_006

Mount not allowed

**Trigger.** A profile, the proxy socket or an agent adapter wants to mount a path that the mount rules forbid.

**Fix.** For the project folder, `ChangeSetting` to `sandbox.work_dir`, the interface offers the folder picker; otherwise no fix, the text names the path and the rule.

#### SANDBOX_007

Unknown bridge direction

**Trigger.** A profile names a bridge direction that does not exist.

**Fix.** No fix: today only `in` is allowed.

#### SANDBOX_010

Unexpected launcher arguments

**Trigger.** The starter's argument list does not match what the launcher expects.

**Fix.** `ChangeSetting` to a profile whose argument list matches; in the escape starter without a fix.

#### SANDBOX_011

Placeholder cannot be created

**Trigger.** A placeholder or directory for the run cannot be created, or the snapshot of the project cannot be read.

**Fix.** Mostly without a fix — the text names the path and the error; if the shim is missing, `CopyCommand` to build it.

#### SANDBOX_012

Invalid launcher command line

**Trigger.** The starter's command line is invalid, a plan was started twice, or the waiting thread failed.

**Fix.** No fix: the error lies in the starter or in the plan, not in a setting.

#### SANDBOX_013

Isolation check without a report

**Trigger.** The shim did not deliver a complete report; the three guarantees are therefore unproven.

**Fix.** No fix: the start is aborted.

#### SANDBOX_014

Isolation check 1: a network interface exists

**Trigger.** Check 1: in the sandbox there is more than `lo` as a network interface.

**Fix.** No fix: the daemon ends the sandbox.

#### SANDBOX_015

Isolation check 2: more than one socket

**Trigger.** Check 2: in the sandbox there is more than the one allowed Unix socket.

**Fix.** No fix: the daemon ends the sandbox.

#### SANDBOX_016

Isolation check 3: seccomp not in effect

**Trigger.** Check 3: the seccomp filter is not active or lets too much through.

**Fix.** No fix: the daemon ends the sandbox.

#### SANDBOX_020

Masked path released

**Trigger.** A profile lifts a mask; the agent may read and write the path.

**Fix.** No fix: it is a statement about the chosen profile.

#### SANDBOX_021

Kernel without openat2

**Trigger.** The kernel does not know `openat2`; the project folder is read with `openat` and `O_NOFOLLOW`.

**Fix.** No fix: the check runs, only more slowly and with less of a guarantee.

#### SANDBOX_022

Symlink points out of the project

**Trigger.** The agent created a symlink that points out of the project.

**Fix.** No fix: the report names the target and the source.

#### SANDBOX_023

Possible secrets in the project

**Trigger.** After the run, the project contains strings that look like secrets.

**Fix.** No fix: the report names the file and the number of findings.

#### SANDBOX_024

Snapshot cut short

**Trigger.** A budget ended the snapshot of the project early.

**Fix.** No fix: the report says which part is cut short.

#### SANDBOX_025

Written into the project without a mask

**Trigger.** The agent wrote under a masked path that did not exist in the project.

**Fix.** No fix: the report names the files.

#### SANDBOX_026

A file in the project that this computer runs

**Trigger.** The agent wrote files that this computer runs on its own (hooks, build files).

**Fix.** No fix: the report names the first of them.

#### SANDBOX_027

No summary for this run

**Trigger.** No summary is recorded for this sandbox.

**Fix.** No fix: either it never ran here, or the run left none.

#### SANDBOX_028

Changed file not scanned

**Trigger.** Changed files were not scanned for secrets because a budget kicked in.

**Fix.** No fix: the report names the number and the first file.

#### SANDBOX_029

Sandbox ended only after the deadline

**Trigger.** The agent did not answer `SIGTERM`, and after the deadline `SIGKILL` followed; or after that there was no exit status.

**Fix.** No fix if `SIGKILL` took effect or only the status was missing; if the process lives on, the text names `ps -o stat= -p <pid>`.

#### SANDBOX_019

Refused connection attempts are not reported

**Trigger.** The kernel did not give the agent's filter a listener; refusing continues with `EPERM`, but nothing is counted or reported.

**Fix.** No fix button: the text names the errno; `EBUSY` means that Humanitl itself runs in an environment with its own seccomp listener.

#### SANDBOX_030

Disposable sandbox not possible

**Trigger.** The start asked for a disposable sandbox, and one of its conditions is missing: bubblewrap 0.11 or newer, a runtime directory on tmpfs, a writable view of the project folder, or no changes of an earlier disposable run still pending.

**Fix.** `CopyCommand` with the install command for a newer bubblewrap, `ChangeSetting` on `sandbox.work_mode` = `rw`, `SetEnv` for `XDG_RUNTIME_DIR` = `/run/user/<uid>`, or `CopyCommand` with `humanitl sandbox changes` for pending changes.

#### SANDBOX_031

Change of a disposable run not applied

**Trigger.** Applying the changes of a disposable run stopped at a path: the project file changed since the start, the change is not something Humanitl writes to the host (a symbolic link out of the project, a device, FIFO or socket), a parent in the project is a symbolic link now, there are no pending changes, or the write failed.

**Fix.** `CopyCommand` with `humanitl sandbox apply --overwrite` after a look at the conflicting file, `CopyCommand` with `humanitl sandbox apply` and a smaller selection, or `CopyCommand` with `humanitl sandbox discard`.

#### SANDBOX_032

Disposable layer full

**Trigger.** The changes of a disposable run grew over the cap of the upper layer in RAM (bytes or entries), and the daemon stopped the sandbox so the agent cannot fill the memory.

**Fix.** No fix for the run; the changes so far wait under `humanitl sandbox changes`. A task that needs more space runs without `--disposable`.

### Area proxy

#### PROXY_001

Body over the limit

**Trigger.** Reserved for a body over the limit up to which Humanitl holds or records it. Today nothing builds this finding: the limits from `limits.*` take effect without a message of their own, and truncation is noted on the flow.

**Fix.** No fix: as long as nothing builds the code, it appears nowhere.

#### PROXY_002

Target mismatch

**Trigger.** The target of the `CONNECT`, SNI and `Host` or `:authority` do not match (domain fronting).

**Fix.** No fix: the request is blocked.

#### PROXY_003

Upstream connection failed

**Trigger.** The connection to the upstream does not come about, or no address is pinned. On the flow: the upstream's certificate comes from an authority this computer does not trust.

**Fix.** For an unknown authority, `CopyCommand` with `sudo update-ca-certificates` (with the CA placed beforehand as `.crt` in `/usr/local/share/ca-certificates/`), with `tls.trust_system_store = false`, `ChangeSetting` to `true`; otherwise no fix, the text names the upstream and the cause.

#### PROXY_005

Invalid state change of a request

**Trigger.** A flow is to take a transition that its state does not allow.

**Fix.** No fix: the request is blocked instead of continuing in an unclear state.

#### PROXY_007

HTTP/2 not available

**Trigger.** Reserved for the case that HTTP/2 is requested upstream but is not available. Today nothing builds it — the proxy speaks HTTP/1.1 upstream, and `experimental.h2_upstream` has no reader on the way there (HUM-108).

**Fix.** No fix: the code is waiting for the path it is meant to describe.

#### PROXY_008

Private target address refused

**Trigger.** The upstream resolves to a private address, and no rule allows that.

**Fix.** `AddRule` with `allow_private` for exactly this host.

#### PROXY_009

Not a meta request

**Trigger.** A flow is to count as answered by the proxy itself, but did not go to `humanitl.internal`.

**Fix.** No fix: it is an error in the daemon, not an input of the user.

#### PROXY_010

Connection limit reached

**Trigger.** The number of simultaneous connections from the sandbox has reached `limits.max_client_connections`.

**Fix.** `ChangeSetting` to a higher limit.

#### PROXY_011

Request body stalled

**Trigger.** A request body stalled and exceeded the deadline.

**Fix.** `ChangeSetting` to a longer deadline.

### Area tls

#### TLS_001

Client refused the Humanitl CA

**Trigger.** A client in the sandbox refused the Humanitl CA (`unknown_ca` or `bad_certificate`).

**Fix.** `SetEnv` with the CA variable that this tool reads.

#### TLS_002

Client keeps aborting the handshake

**Trigger.** A client keeps aborting the handshake to the same host.

**Fix.** `AddRule`: block the host so that the agent fails quickly instead of hanging.

#### TLS_003

Client without SNI

**Trigger.** A client connected without SNI; nothing then binds the handshake to the host of the tunnel.

**Fix.** No fix: the connection is turned away because its requests could not be assigned to any upstream.

#### TLS_004

CA directory not writable

**Trigger.** The CA directory is not writable.

**Fix.** `CopyCommand` with the path.

#### TLS_005

CA files unusable

**Trigger.** The CA files are unusable: the certificate or the key cannot be read.

**Fix.** For corrupted material, `CopyCommand` that deletes the CA directory so that it is created anew; otherwise no fix.

### Area llm

#### LLM_001

LLM endpoint not reachable

**Trigger.** The endpoint does not answer, does not resolve, the deadline runs out, or its certificate comes from an authority this computer does not trust.

**Fix.** For the probe, `CopyCommand` with a `curl` that tries the same; for an unknown authority as with `PROXY_003`. During the run, `ChangeSetting` to `llm.endpoint`.

#### LLM_002

LLM endpoint asks for a login

**Trigger.** The endpoint asks for a login (`401` or `403`), without a key or with one it refuses.

**Fix.** `ChangeSetting` to `llm.api_key_ref`: in the interface a button that opens the settings at the field “Access key”; on the command line `humanitl llm key set`.

#### LLM_003

LLM endpoint is not a known API

**Trigger.** The connection is up, but the endpoint's model list does not answer as a known API.

**Fix.** `ChangeSetting` to the base address of the OpenAI-compatible API.

#### LLM_004

No model configured

**Trigger.** `llm.models` is empty; the agent gets a placeholder instead of a model.

**Fix.** `ChangeSetting` to `llm.endpoint` or `CopyCommand` with a `curl` on `/models` — ask first, then enter it.

#### LLM_005

Findings in a passthrough request

**Trigger.** A passthrough request to the language model carries findings that have not yet been warned about in this run at the same endpoint, or were last warned about at least 15 minutes ago: secrets or personal data. Findings that were warned about recently do not trigger a new warning, but they still appear on every request in the recording.

**Fix.** No fix: the request has already been sent; the finding makes it visible.

#### LLM_006

LLM endpoint is not on a private network

**Trigger.** The endpoint is not on a private network.

**Fix.** `ChangeSetting` to `llm.endpoint`, where the probe builds it; the traffic to it goes past the queue.

#### LLM_007

LLM endpoint is not a readable HTTP address

**Trigger.** `llm.endpoint` is not a readable HTTP address.

**Fix.** `ChangeSetting` to an absolute `http` or `https` address.

#### LLM_008

The network search cannot run

**Trigger.** The network search cannot run: no default route, no readable routing table, or a network beyond your own `/24`.

**Fix.** No fix: the text names both networks and the limit.

#### LLM_009

Access key not available

**Trigger.** `llm.api_key_ref` is set, but the keyring or the file under `$XDG_DATA_HOME/humanitl/keys` yields no key for it.

**Fix.** `CopyCommand`: `humanitl llm key set` stores it anew; if the file's permissions are open, a `chmod 600`.

### Area rules

#### RULES_001

Invalid rules file

**Trigger.** The rules file is unreadable, not valid YAML, or a rule does not survive the round trip.

**Fix.** No fix: the text names the location.

#### RULES_002

Suspicious host pattern (xn--, IP in host glob)

**Trigger.** A host pattern looks suspicious: an IP in the glob or an `xn--` label.

**Fix.** No fix: the text names the spelling that really matches.

#### RULES_003

Invalid host pattern

**Trigger.** A host pattern is invalid and matches nothing.

**Fix.** No fix: the text names the error in the pattern.

#### RULES_005

Invalid path pattern

**Trigger.** A path pattern is invalid, for example without a leading slash.

**Fix.** No fix: the text names the form that applies.

#### RULES_006

Unknown rules file version

**Trigger.** The rules file names a version that does not exist.

**Fix.** No fix: there is exactly one version.

#### RULES_007

Duplicate rule id

**Trigger.** Two rules carry the same id.

**Fix.** No fix: every rule needs its own.

#### RULES_008

Rule is too broad

**Trigger.** A rule is so broad that it allows more than the human probably means.

**Fix.** No fix: the text names what else it matches.

#### RULES_009

Rules file not writable

**Trigger.** The rules file cannot be written; the rule set on disk stays unchanged.

**Fix.** No fix: the text names the path and the reason.

#### RULES_010

Bundled rule cannot be changed

**Trigger.** A bundled rule is to be changed or deleted.

**Fix.** No fix: bundled rules are deactivated, not removed.

#### RULES_011

Rules reloaded

**Trigger.** The rule set was reloaded; the number of rules is in the text.

**Fix.** No fix: it is a notice, not an error.

#### RULES_012

Dry run could not read the recording

**Trigger.** The dry run found no recording against which it could check the rule.

**Fix.** No fix: without traffic there is nothing to check.

### Area findings

#### FINDINGS_001

Detector rules unusable

**Trigger.** The detectors' built-in rule set is unusable; that is a build error.

**Fix.** No fix: the text names the file and the parser error.

#### FINDINGS_002

Scan incomplete

**Trigger.** The scan was incomplete: the body was larger than the preview limit, or decompression ran into the ratio limit.

**Fix.** `ChangeSetting` to `limits.preview_cap_bytes` or `limits.max_decompress_ratio` respectively.

#### FINDINGS_003

Finding in a note

**Trigger.** The note to the agent contains a possible secret value; it goes out anyway.

**Fix.** No fix: the decision stands. Whoever does not want to send the value blocks again with a different note.

### Area catalog

#### CATALOG_001

Domain catalogue unreadable

**Trigger.** The bundled domain catalogue is not readable.

**Fix.** No fix: without the catalogue only the names are missing, not the decision.

#### CATALOG_002

Ranking unreadable

**Trigger.** The ranking of the domains is not readable.

**Fix.** No fix: the sorting falls back to the order of the findings.

### Area terminal

#### TERM_001

A second writing terminal client was refused

**Trigger.** A second writing terminal client connects to the same session.

**Fix.** `CopyCommand` with `humanitl sandbox attach --read-only`: watching works, writing does not.

#### TERM_002

Sandbox terminal not reachable

**Trigger.** The sandbox terminal is not reachable; the run has none or it is already closed.

**Fix.** No fix: the text names the session.

### Area recorder

#### RECORDER_001

Recording not available

**Trigger.** The recording is not available: the database is missing, locked or unreadable — or this daemon runs without recording.

**Fix.** Where there is one, the fix names the path of the database.

#### RECORDER_002

Invalid filter

**Trigger.** A filter of the history is invalid.

**Fix.** No fix: the text names the field.

#### RECORDER_003

Recording could not write

**Trigger.** The recording could not write; the flow stays incomplete.

**Fix.** Where the writer stands, `CopyCommand` to restart the daemon; otherwise the text names only the error.

#### RECORDER_004

Blob store not usable

**Trigger.** The blob store is not usable; large bodies cannot be stored.

**Fix.** `CopyCommand` with `ls -ld` and `df -h` on the directory — permissions or space.

#### RECORDER_005

Recording works again

**Trigger.** After a reported disruption, the writer thread has again written a transaction without an error.

**Fix.** No fix: what happened during the disruption is missing from the history and is not added later.

### Area limits

#### LIMIT_001

Request body over the hold limit

**Trigger.** A request is larger than `limits.hold_body_cap_bytes`: announced via `Content-Length` or noticed while buffering.

**Fix.** `AddRule` with `action: ask` and `stream: true` for the host, method and path of the request; without an effective rule or under `hold.hard_block_checksum_secrets` (where `stream: true` never takes effect), `ChangeSetting` to `limits.hold_body_cap_bytes`.

#### LIMIT_002

Queue full

**Trigger.** A new request was to be held while the queue already holds `limits.hold_max_flows` flows or `limits.hold_max_bytes` bytes, or a body no longer fits into `limits.hold_max_bytes` while buffering.

**Fix.** No fix button: work through the queue. A higher limit only helps if the held requests are really needed.

#### LIMIT_003

Decompression ratio exceeded

**Trigger.** A compressed body would have produced more than `limits.max_decompress_ratio` times as many bytes on decompression as its compressed size.

**Fix.** No fix: such a ratio is the hallmark of a decompression bomb, and a higher limit would be the wrong advice.

#### LIMIT_004

Response of the target went silent

**Trigger.** After the response headers, the upstream sent no further piece of the body for longer than `limits.body_timeout_secs`, or with a streamed upload accepted no further piece (`limits.body_timeout_secs`) or sent no headers after the whole body (`limits.header_timeout_secs`).

**Fix.** `ChangeSetting` to the limit that ran out, with double the value.

#### LIMIT_005

Request head too late

**Trigger.** A client opened a connection and did not send the head of its request within `limits.header_timeout_secs`.

**Fix.** No fix button: raise `limits.header_timeout_secs` only if a tool on a slow line really needs that long.

#### LIMIT_006

Event buffer overflowed

**Trigger.** A subscriber of the event stream has not collected more than `limits.event_buffer` events.

**Fix.** No fix: the interface reloads via `ListFlows`.

### Area audit

#### AUDIT_001

Audit chain broken

**Trigger.** The check of `audit.jsonl` finds a record that does not match its predecessor, hash, MAC or anchor, or the file ends before an anchor; at the start it is the last record the writer is to append to. On export there is a line in the log that is not a record, an empty one included; the export then aborts and writes nothing. A chain whose records do not fit together, on the other hand, it exports unchanged; only the check reports the break.

**Fix.** `CopyCommand` that moves the file aside together with a timestamp; it stays as evidence, and the chain starts anew. On export, a `CopyCommand` with `humanitl audit verify --file` on the log.

#### AUDIT_002

Incomplete last line set aside

**Trigger.** At the start, `audit.jsonl` does not end with a line break; the rest after the last complete record was moved into a file of its own.

**Fix.** No fix needed: the text names the file with the rest, and the chain continues.

#### AUDIT_003

Audit data not canonical

**Trigger.** The data of a record contained a number that is not an integer; the record was written with a placeholder instead of the data.

**Fix.** No fix for users: a program error; the text names the kind of record.

#### AUDIT_004

Audit log held by another daemon

**Trigger.** At the start, another process holds the exclusive lock on `audit.jsonl`.

**Fix.** `CopyCommand` with `humanitl daemon status`: find the running daemon and end it.

#### AUDIT_005

Audit key unusable

**Trigger.** The key file is a symlink or not a regular file, carries permissions for group or others, belongs to another user, does not have exactly 32 bytes or cannot be created.

**Fix.** `CopyCommand`: `rm` for a burnt key, otherwise `ls -ln` on the file or `mkdir`/`chmod` on the directory.

#### AUDIT_006

Audit log not writable

**Trigger.** The file, directory or anchor table of the audit log cannot be opened, read, written or flushed to disk.

**Fix.** `CopyCommand` with `ls -ld` and `df -h` on the directory — permissions or space.

#### AUDIT_007

Audit chain continued past the last anchor

**Trigger.** At the start, `audit.jsonl` ends before an anchor from `audit_anchors`; the chain continues after this anchor, and the check keeps reporting the gap as a break.

**Fix.** No fix needed: the text names where the log ended and after which anchor the chain continues.

#### AUDIT_008

Audit export not writable

**Trigger.** The target file of `humanitl audit export` already exists, its directory is missing, or writing fails.

**Fix.** `CopyCommand` that moves the existing file aside; otherwise the text names the path and the reason.

#### AUDIT_009

Invalid audit request

**Trigger.** An `Audit` request names no operation, an unknown export format, a relative target path, a host redaction or an unreadable point in time or cursor.

**Fix.** `CopyCommand` with `humanitl audit --help`; the text names the field that is wrong.

### Area doctor

#### DOCTOR_001

Sandbox tool missing or too old

**Trigger.** The check of this computer found no `bubblewrap` or one that is too old.

**Fix.** `CopyCommand` with the install command.

#### DOCTOR_002

User namespaces not available

**Trigger.** Unprivileged namespaces are not available on this computer.

**Fix.** `CopyCommand` with the `sysctl` call.

#### DOCTOR_003

Kernel without usable seccomp

**Trigger.** The kernel has no usable seccomp; the third guarantee could not be kept.

**Fix.** `OpenUrl` to the documentation on seccomp — without the kernel, no setting helps.

#### DOCTOR_004

Runtime directory missing or not private

**Trigger.** The runtime directory is missing or not private.

**Fix.** `SetEnv` or the hint about the permissions.

#### DOCTOR_005

No systemd user session

**Trigger.** There is no systemd user session in which the daemon could run.

**Fix.** `CopyCommand` depending on the situation: `humanitld` by hand, `systemctl --user --failed` or `loginctl enable-linger`.

#### DOCTOR_006

Daemon not reachable or on another protocol

**Trigger.** The daemon does not answer or speaks a different contract.

**Fix.** `CopyCommand` with the start command.

#### DOCTOR_007

Agent command not on PATH

**Trigger.** The agent command is neither on the daemon's `PATH` nor in `~/.opencode/bin`, `~/.local/bin` or `~/bin` (HUM-278).

**Fix.** `CopyCommand` with `humanitl config set agent.command`, the path is a placeholder, and the install command is in the text; for an adapter whose program is a script (pi, HUM-274), the install command of the adapter itself, because the daemon never mounts a script.

#### DOCTOR_008

Language model not reachable

**Trigger.** The language model was not reachable when someone asked for it — or none is entered at all.

**Fix.** `CopyCommand` with the `curl` on the endpoint; without an entry, `ChangeSetting` to `llm.endpoint`.

#### DOCTOR_009

No room for the tray icon

**Trigger.** The desktop environment offers no room for the tray icon.

**Fix.** `CopyCommand` for the missing portal or the extension; without them the window remains the way.

#### DOCTOR_010

Renderer and graphics driver do not agree

**Trigger.** The renderer and the graphics driver do not agree; the window would stay black.

**Fix.** `OpenUrl` to the documentation on Impeller and the software renderer.

#### DOCTOR_011

Little space in the data directory

**Trigger.** There is little space in the data directory; the recording and the blobs grow there.

**Fix.** `ChangeSetting` to a shorter `recorder.retention_days` — or free up space.

#### DOCTOR_012

Check could not run

**Trigger.** A check could not run on this computer; nobody passed or failed it.

**Fix.** The fix names the command that would run it.

#### DOCTOR_013

Language model not contacted

**Trigger.** The language model was not contacted because nobody asked for it.

**Fix.** `CopyCommand` with `humanitl doctor --probe-llm`.

### Area cli

#### CLI_001

Call refused by the daemon

**Trigger.** A call failed at the daemon, or an output could not be written.

**Fix.** No fix: the text names the call and the reason.

#### CLI_002

`--ask terminal` is not possible here

**Trigger.** `--ask terminal` is not possible for this run, for example with a full-screen agent.

**Fix.** No fix: the text names the two modes that work.

#### CLI_003

Subcommand not available yet

**Trigger.** A subcommand does not exist yet; the issue for it is in the text.

**Fix.** `OpenUrl` to the issue.

#### CLI_004

Invalid call

**Trigger.** The call is invalid: unknown subcommand, missing argument, contradictory switches.

**Fix.** `CopyCommand` with the form that applies.

#### CLI_006

The runtime could not start

**Trigger.** The command line could not build its runtime; the operating system's reason is in the text.

**Fix.** No fix: the text names what the operating system reported.

#### CLI_005

A session is already running

**Trigger.** A session is already running in this daemon.

**Fix.** No fix: whoever started it ends it there.

### Area ui

#### UI_002

No room for the tray icon

**Trigger.** The desktop environment offers no room for the tray icon in the system area.

**Fix.** No fix: the window remains the way to the queue.

#### UI_003

The folder dialog did not open

**Trigger.** The desktop environment could not open the dialog for choosing the project folder; the portal's reason is in the text.

**Fix.** `CopyCommand` with `humanitl run`, to be run in the project folder.

### Area agent

#### AGENT_001

Agent command not found

**Trigger.** The agent command is neither on the `PATH`, in `~/.opencode/bin`, `~/.local/bin`, `~/bin` nor set via `agent.command` (HUM-278).

**Fix.** `CopyCommand` with `humanitl config set agent.command`, the path is a placeholder, and the install command is in the text; for an adapter whose program is a script (pi, HUM-274), the install command of the adapter itself, because the daemon never mounts a script.

#### AGENT_002

Agent command not executable

**Trigger.** `agent.command` points to something that is not executable on this computer.

**Fix.** `ChangeSetting` back to the adapter's default command.

#### AGENT_003

Bundled agent template unusable

**Trigger.** A bundled template of the adapter is unusable; that is a build error.

**Fix.** No fix: the text names the file.

#### AGENT_004

Agent command not reachable in the sandbox

**Trigger.** The search starts in the sandbox: its search path (`sandbox.env` before the profile's `[env]`), its mounts and the profile's references, relative entries against `[mounts].work.dst`. Only if nothing lies there does the host count, and the finding names both. Two cases: the command lies under no mount, or it lies under one whose directory the search path does not name. What cannot be decided produces no finding (HUM-139).

**Fix.** `CopyCommand` that installs it at a mounted location, or `ChangeSetting` to `sandbox.env.PATH` if the mount is in place and only the search path is missing.

#### AGENT_005

Agent did not start

**Trigger.** The shim reports `EXEC fail` on the report channel, or the command ends without this signal with exit code `127` or `126` without having written a single byte. The finding names the command and the sandbox's `PATH`; it withholds the `PATH` if a human wrote it (`sandbox.env` or a profile of your own), like the environment table.

**Fix.** `CopyCommand` with `humanitl sandbox run -- /bin/sh -c 'command -v …'`: asks the same sandbox where the command lies. What is mounted is decided by the profile.

#### AGENT_006

Node.js missing in the sandbox

**Trigger.** The command resolves to a script that starts Node.js through its `#!` line, and the sandbox finds no executable `node` on its `PATH` (or at the absolute path the line names). The finding names the script, the sandbox `PATH` and where a `node` lies on the host, if anywhere.

**Fix.** `ChangeSetting` on `sandbox.env.PATH` when a mounted `node` is only missing from the path, `CopyCommand` that installs the host's `node` under `/usr/local/bin` when it is not mounted, the installer of pi when the `#!` line names an absolute `node` outside the mounts, otherwise `OpenUrl` to the Node.js download page (pi needs 22.19 or newer).

#### AGENT_007

pi web extension not installed

**Trigger.** `agent.pi.web_access` is `on`, and no directory the sandbox reaches holds the package `pi-web-access`. pi starts without `web_search` and `fetch_content`.

**Fix.** `CopyCommand` with `sudo npm install -g --ignore-scripts pi-web-access`, which puts the package under `/usr/local`, where the sandbox sees it read-only.

### Area edit

#### EDIT_001

Target of the edited request differs

**Trigger.** The host, port or scheme of the edited request differ from the held one after normalisation.

**Fix.** No fix: whoever wants a different upstream makes a new request; nobody decided about this one for that upstream.

#### EDIT_002

Invalid method in the edited request

**Trigger.** The method is empty, longer than 16 characters or contains something other than capital letters.

**Fix.** No fix button: the text names the refused method; the interface writes it in capital letters.

#### EDIT_003

Invalid path in the edited request

**Trigger.** The path does not start with `/`, or it contains a space or a control character.

**Fix.** No fix button: the text names the location; query values belong URL-encoded.

#### EDIT_005

Edited body over the limit

**Trigger.** The body of the edited request is larger than the limit against which the hold buffered (`limits.hold_body_cap_bytes`).

**Fix.** `ChangeSetting` to `limits.hold_body_cap_bytes`, or send less.

### Area hold

#### HOLD_004

Confirmed secret locked

**Trigger.** The request or its edited version carries a checksum-confirmed finding of the kind IBAN, credit card, API key or JWT, and `hold.hard_block_checksum_secrets` is on.

**Fix.** `ChangeSetting` to `hold.hard_block_checksum_secrets = false`; otherwise the request goes out only without the value, replaced in the editor or made anew by the agent.
