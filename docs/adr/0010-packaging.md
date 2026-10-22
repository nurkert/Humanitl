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
