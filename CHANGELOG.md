# Changelog

All notable changes to Humanitl are listed here. The format follows
[Keep a Changelog 1.1.0](https://keepachangelog.com/en/1.1.0/), and versions
follow [Semantic Versioning 2.0.0](https://semver.org/spec/v2.0.0.html). Before
1.0.0, a minor version may change the configuration, the protocol or the rule
format; a patch version never does.

Releases before 1.0.0 are not signed. Each release carries `SHA256SUMS`, which
detects a damaged download but not a tampered release; see
[`docs/guides/verify-download.md`](docs/guides/verify-download.md). Signed
releases come with 1.0.0.

The release notes on GitHub for a version start with its section from this
file (`packaging/release/changelog-section.sh`); the pre-releases 0.0.x have no
section here and list their commits instead.

## [Unreleased]

## [0.1.0] - unreleased

The first release. Compared with the 0.0.x pre-releases, from the first public one, 0.0.1, to the
last, 0.0.20:

### Added

- macOS (Apple Silicon and Intel): a Seatbelt sandbox, the daemon as a launchd
  agent, a menu bar item and notifications for held requests, and a disk image
  and a zip in every release.
- The disposable sandbox: the agent works on an overlay of the project folder;
  the app and the CLI show the diff, apply selected changes or discard them. It
  is the default where the system supports it (`sandbox.mode = "auto"`).
- pi as a second agent next to OpenCode, with web search and fetch through the
  proxy; both agents learn that a held request is slow on purpose and do not
  give up while it waits.
- Several saved model servers: one default plus assignments per project or
  session profile, `humanitl run --llm NAME`, and a one-time confirmation for a
  project that picks a server other than the default.
- One project folder, chosen once, shared by the app and the CLI.
- Updates from inside the app on Linux and macOS, from a daily check of the
  GitHub releases (`update.check`, on by default), with an Ed25519-signed
  manifest; APT and dnf installations update through their package manager.
- Packages: an AppImage, an RPM for Fedora and openSUSE, the AUR package
  `humanitl-bin`, and a `.deb` with a socket-activated, hardened user unit
  that replaces a running daemon on update.
- Intercept: pause before sending while findings are open, pause the whole
  intercept from the header, and an "Edited" marker on changed requests in the
  queue and the history.
- Audit: a screen with chain status, record table, export and retention,
  `audit.max_bytes`, and a trail of findings that were sent on purpose.
- Sandbox terminal: several shells as tabs, a second shell in a running
  sandbox, `humanitl sandbox attach` that detaches with Ctrl+A d, copy and
  paste, zoom, full window, and a floating or docked terminal.
- The daemon can trust the system certificate store towards destinations.
- Resource limits and backpressure in the proxy.
- Settings as a searchable overlay; German and English; the whole app scales
  with Ctrl+Plus, Ctrl+Minus and Ctrl+0.
- `humanitl config`, `humanitl audit` and `humanitl daemon` commands;
  `humanitl daemon install` sets up, refreshes and removes the service in one
  step.

### Changed

- New visual design: neutral greys, a yellow accent, orchid for held requests,
  one layout grid, a single title bar, and a short onboarding instead of the
  setup screen.
- Saved settings take effect without restarting the daemon.
- With the hard block on, a request carrying a checksum-confirmed secret is
  held and leaves only edited, after a second scan finds no such secret in the
  edited version; `Allow` on the unchanged request is refused with `HOLD_004`.
- Release notes are in English and releases are regular GitHub releases, no
  longer marked as pre-releases.

### Fixed

- OpenCode reaches a model on loopback through the reserved name
  `llm.humanitl.internal`, and pi 1.x reaches a local model through a CONNECT
  tunnel that carries cleartext HTTP.
- Model endpoints behind a path prefix work; pasted endpoints and keys are
  cleaned up.
- Ctrl+C no longer quits OpenCode in the terminal; a lone modifier key no longer
  types a stray character.
- `humanitl daemon install` checks the systemd unit directory and sets aside a
  unit of its own that would hide the package's unit.
- The app shows "reconnecting" during a short daemon restart instead of an
  error, and a flow held before the first resync still reaches the queue.

### Security

- PID 1 in the sandbox is the filtered, non-dumpable shim, and the shim closes
  descriptors the launcher leaked before it forks.
- The project folder is checked at every start and bound by descriptor, so it
  cannot be swapped between check and bind.
- Dot segments in a path can neither widen an `allow` rule nor slip past a
  `block` rule.
- Findings scan every gzip member and flag deflate bodies whose zlib and raw
  readings differ.
- A project profile can neither switch off the hard block nor shrink the
  recorded body; a rule suggestion that carries a masked secret is withheld.
- On macOS, the agent cannot replace its adapter files or reconfigure OpenCode
  from the project, and the session directory with its access key ends with
  the sandbox.
- The `/tmp` fallback for the runtime directory is checked for owner and mode.
- Dependency updates for security advisories in pulldown-cmark, tokio, rustls,
  rustls-webpki, webpki-roots, ctutils, rusqlite (bundled SQLite 3.54.1) and
  rustix.
