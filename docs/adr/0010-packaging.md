# ADR-0010 · Delivery as `.deb` and AppImage, Flatpak later and only for the UI

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-09-02

## Context

Humanitl consists of four executable parts: the daemon `humanitld`, the
Flutter interface, the command-line tool `humanitl` and the
sandbox shim `humanitl-shim`. On top of that come profiles, the domain catalogue, the
Public Suffix List and the bundled rules.

The target audience is professionals without a security background. For them,
what `BACKLOG.md` 1.3 states as principle 9 must hold: one package installs
everything, the first start offers the background service with one click, and
after that the standard path works without a terminal.

The technical constraint that determines everything else: **The daemon starts
`bwrap`.** A sandboxed application format that does not itself allow nesting
user namespaces cannot run the daemon. This affects Flatpak in
particular.

## Decision

Delivery is first as **`.deb`** and as **AppImage**, built with
[fastforge](https://fastforge.dev/). Both artefacts contain everything: UI, daemon,
shim, CLI, profiles, catalogue and rules. The daemon binary sits next to the
Flutter bundle, not in a separate package.

The background service is a **systemd user unit**, which is offered at first start via
`FixAction::InstallService` and installed and started with one click.
No terminal required. The unit is hardened:
`NoNewPrivileges`, `ProtectHome=read-only` with targeted `ReadWritePaths`,
`RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6`, `PrivateTmp`,
`SystemCallFilter=@system-service`.

The AppImage places unit and binaries under `~/.local` and updates them on
a version change. Uninstalling removes both. `humanitl doctor`
then confirms that the installation is complete.

**Flatpak comes later and only for the UI.** The daemon then runs outside the
Flatpak, and the connection is made via `--filesystem=xdg-run/humanitl`, that is, via
the gRPC socket from ADR-0003.

## Rationale

`.deb` is the format of the primary target platform (Debian, Ubuntu and derivatives).
It integrates with package management, can declare dependencies such as `bubblewrap`
and is what the target audience expects.

AppImage covers everything beside that, without a package format matrix. It needs no
root privileges and is the shortest "try it once" for a pre-release test audience.

Together, both cover the realistic Linux desktop landscape without us
maintaining a separate recipe for every distribution.

That daemon and UI sit in **one** artefact is a decision against the
usual split into `humanitl` and `humanitl-daemon`. The reason is principle 9:
a package that installs half of the product and then says "please also install
the daemon package" is exactly the friction that is to be avoided.
In addition, daemon and proto versions must match (ADR-0003); a single
artefact makes a version mismatch impossible.

The systemd user unit instead of a system service: Humanitl needs no
root privileges, works on the user's `$XDG_RUNTIME_DIR` and `$XDG_DATA_HOME` and
starts a rootless sandbox with `bwrap`. A system service would be more
privilege without gain and would raise multi-user questions that do not
exist.

Flatpak for the UI is attractive (distribution via Flathub, automatic
updates), but fails because of the daemon: a Flatpak sandbox cannot start `bwrap` the way
we need it. The split "UI in the Flatpak, daemon outside"
is the only viable path — and it works only because the daemon
is a separate process with a clean socket interface anyway
(ADR-0003). In return it costs an additional installation step for the
daemon and therefore comes last.

## Rejected alternatives

- **Flatpak only.** The most elegant distribution and technically
  impossible for this product as long as the daemon starts `bwrap`.
- **Snap.** Similar sandbox problems, plus a store that part of the
  target audience avoids, and more difficult handling of `$XDG_RUNTIME_DIR`.
- **Only an AppImage.** Saves the `.deb`, but loses the declaration of
  system dependencies (`bubblewrap`) and the familiar installation via
  package management.
- **Separate packages for daemon and UI.** Cleaner from a packager's view, but a
  break with principle 9 and a risk of version mismatches between
  proto client and server.
- **System service instead of user unit.** More privilege, no gain, and
  root privileges during installation that a rootless setup does not
  need.
- **`cargo install` and `flutter build` as installation instructions.** Fine for
  developers, not a delivery for the target audience.
- **Distribution packages maintained by others (AUR, Fedora COPR).** Welcome,
  but no substitute for an artefact that we build and test ourselves.

## Consequences

- The release job builds both artefacts, generates checksums and attaches them to the
  tag. The demo scripts of milestones M1 to M4 must be green for this.
- The systemd unit is part of the repository (`packaging/systemd/`) and is
  tested along with it, not improvised at delivery.
- The first start needs a path without a terminal. That is the reason why
  `FixAction::InstallService` exists as an enumerable action (ADR-0012) and
  not as text instructions.
- Wayland is the normal case, X11 the fallback. Both are tested before the release on
  NVIDIA and Intel graphics, because Flutter desktop looks different
  there.
- An update must take database migrations (ADR-0008) and proto versions
  (ADR-0003) into account. The package brings both sides at the same time, which
  limits the case "UI newer than daemon" to the special case Flatpak — there
  the version check from `GetInfo` applies.
- Uninstalling must remove unit and binaries again, otherwise a
  service remains that points to a missing socket.

## Affected issues

`HUM-053` (packaging: `.deb` and AppImage via fastforge, systemd user unit,
Wayland test), `HUM-077` (one-click installation and uninstallation),
`HUM-075` (`humanitl doctor` confirms the installation), `HUM-060`
(release 0.1 with changelog and checksums).

## Addendum 2026-11-13: Podman under the hardened user unit (HUM-434)

The rest of the addendum for the Podman backend follows with HUM-441; this part covers the unit.

**Measured.** Under `humanitld.service` rootless Podman does not start as a child of the daemon.
Measurement P8 (HUM-292, 2026-11-13, Podman 5.8.6, transient user units built from the unit file,
one property left out at a time): every property with a mount namespace of its own breaks
`newuidmap` ("write to uid_map failed"): `PrivateTmp`, `ProtectSystem`, `PrivateMounts`,
`ProtectClock`, `ProtectControlGroups`, `ProtectKernelModules`, `PrivateIPC` and `PrivateUsers`
(in a user unit each implies a private user namespace). `CapabilityBoundingSet=CAP_SYS_ADMIN`
fails at exec (218). Alone they pass: `NoNewPrivileges` (only with an existing pause process),
`ProtectProc`, `KeyringMode`, `LockPersonality`, `RestrictRealtime`, `RestrictFileSystems`,
`SystemCallFilter`, `SystemCallErrorNumber`, `RestrictAddressFamilies`. A transient unit without
the mount-namespace properties runs `podman run --rootfs`.

**Decision (owner, 2026-11-13): option (b).** Loosening the unit (option a) would have removed
eight properties, most of the hardening, for every backend. Instead the unit stays exactly as it
is, and Podman starts through the user's service manager:

- When the daemon runs outside the first user namespace (`/proc/self/uid_map` is not
  `0 0 4294967295`, which is the case under the unit), every Podman call, the session as well as
  each query, runs as a transient unit via `systemd-run --user --quiet --collect --pipe --wait
  --service-type=exec --expand-environment=no`, `KillMode=mixed`, `Delegate=yes`, `TimeoutStopSec` equal to the
  kill grace (5 s), named `humanitl-<session>`; a query is `humanitl-q-<id>` with `RuntimeMaxSec`
  just above its deadline. The program runs behind `/usr/bin/env -i` with exactly the
  environment the daemon gives it. Only names with the prefix `humanitl-` are ever built, every
  argument must be UTF-8, and the program must be an absolute path without `=`. From a terminal,
  in CI and in tests Podman keeps starting as a child.
- `systemd-run` and not `StartTransientUnit` over D-Bus: no D-Bus crate is in the workspace, and
  `systemd-run` already does what is needed (stdio through `--pipe`, the exit status through
  `--wait`, collection through `--collect`). systemd 254 or newer is required for
  `--expand-environment=no`; an older or unreachable user manager is `SANDBOX_050`.
- The descriptors of the shim no longer travel through `--preserve-fds`, which only works for a
  child: the daemon binds a `SOCK_SEQPACKET` socket per session (`0600`, in the session's `0700`
  directory of its own), mounts only that directory read-only into the sandbox
  (`/run/humanitl-handover`) and names the socket and a one-time token of 128 bits on the shim's
  line. It accepts exactly one connection, closes the listening socket and removes the socket file
  right after `accept`, so no second socket is left in the sandbox (ESC-2), checks `SO_PEERCRED` against its own uid and the token, then sends every descriptor
  in one message. The shim lays them on 3, 4, 5, ... as before, so the environment memfd, the
  mount table and the lifeline are read unchanged. When the daemon dies before the handover, the
  shim finds nobody and never starts the agent; after it, the lifeline ends the sandbox as
  before. The same path also serves Podman started as a child, so one way is tested everywhere.
- Stopping a session stops its unit (`systemctl --user stop --no-block`) after the container is
  killed and removed; the end of an unplanned client (HUM-452) is the end of `systemd-run
  --wait`, which is the end of the unit's main process. A cancelled pull waits for its unit to
  stop before it says that nothing of it runs any more.
- Under the unit the engine sees the mount flags of the sources from the user manager's mount
  namespace, the daemon from its own, where the unit remounts the host `nosuid`. The expected
  mount table therefore leaves the inherited flags of bind mounts open on that path and keeps
  point, type, mode, count and device and inode strict; the direct path stays fully strict.
- bubblewrap and Seatbelt are untouched: they inherit their descriptors as before.

**Cost.** A compromised daemon can start a transient unit of the user, outside its hardening
(`docs/THREAT-MODEL.md`, trust boundaries). The user account could already do that; the hardened
daemon could not. The daemon's unit itself gains no right and no path.
