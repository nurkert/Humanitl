# Installation: maintainer notes

Applies to: Humanitl 0.1

**To install Humanitl, read [the install guide](guides/install.md).** It covers
the prerequisites, the `.deb`, the `.rpm`, the AUR package, the AppImage, the
archive, the macOS disk image, enabling the user service, checking the
installation and uninstalling. How the
macOS app and its disk image are built is in `packaging/macos/` (HUM-326).

This page keeps what the guide leaves out: the APT repository, switching
between install forms, why the systemd unit is hardened the way it is, the
runtime directory without logind, building the packages and the manual test
matrix. Shipped code links to two sections of this page (`#hardening` and
`#xdg_runtime_dir-ohne-logind`); both anchors stay.

## APT repository

Every release also lands in the APT repository `apt.nurkert.de`, so later
versions arrive with `apt upgrade`:

```sh
curl -fsSL https://apt.nurkert.de/install/humanitl | sudo sh
humanitl daemon install
humanitl daemon status
```

The script places the signing key of the repository at
`/usr/share/keyrings/nurkert-archive-keyring.gpg`, the source at
`/etc/apt/sources.list.d/nurkert.list`, and installs the package. Whoever does
not trust a pipe into a root shell reads the script first, or takes the `.deb`
from the release page as the guide describes.

## The package in detail

The menu and the icon cache are updated by the triggers of `desktop-file-utils`
and `hicolor-icon-theme`. The package has two maintainer scripts, both about
the user service (HUM-284):

- `postinst` finds every user with a running systemd user manager
  (`loginctl list-users`, then `user@UID.service`; not in a chroot, not
  without systemd) and, as that user via `setpriv`, runs `systemctl --user
  daemon-reload`. If that user's `humanitld.service` is active and comes from
  the package, it runs `humanitl daemon retire-if-idle`: the daemon itself
  decides, under the lock a start takes its claim with, that no agent session
  holds it, and from then on no session can start. Only after `retired` does
  `systemctl --user restart humanitld.service` start the new version. A
  session that runs keeps its daemon; a daemon from this version on ends by
  itself once the session is over, and the next connection starts the new
  program through the socket. Until then the app shows the version notice
  with a "Restart daemon" button. The daemon of 0.0.6 does not know the
  request; for it `postinst` asks `humanitl --json daemon status` and runs
  `try-restart` only on `agent_session` `none`. That old daemon reports
  `stopped` for the whole of a start, so a start that runs or begins before
  the restart can be cut off; only this one upgrade has that window. Every call as
  the user runs under a root-side `timeout`, detached from the terminal, and
  `policy-rc.d` returning 101 forbids the restart.
- `postrm` runs the same `daemon-reload` after `remove` and `purge`.

Neither ever fails the installation. Whatever did not work is written to the
journal (`journalctl -t humanitl`) and to the output of apt, with the command
to run by hand, usually `systemctl --user restart humanitld`.

`humanitl daemon install` enables only the socket, not the service (HUM-164).
Whoever ran `humanitl daemon install` before that change enabled the service
too; a new call does not remove that link, and the service keeps starting with
the session. That does no harm. To have it start on the first client only, run
`systemctl --user disable humanitld.service` once; the socket stays enabled.

## Switching between install forms

Whoever installed from the archive or the AppImage before still has the unit
that `humanitl daemon install` wrote under `~/.config/systemd/user/`. It hides
the unit of the package. `humanitl daemon install` from the package recognises
that unit by its first line, announces it, moves it and its enablement link
aside as `humanitld.service.bak` (or `.bak.1`, `.bak.2` and so on if the name is
taken) and restarts the service, so that the unit of the package runs. Under
`--no-start` or without `systemctl` it stays where it is, and the output says
that it still hides the packaged unit.

Moving it aside uses `renameat2` with `RENAME_NOREPLACE`. If `~/.config` lies on
NFS, which refuses that call with `EINVAL`, the command stops with `DAEMON_006`
and leaves everything as it was. A file without the first line of
`daemon install` is never touched; the command stops with `DAEMON_005`. That
file belongs to somebody else, who removes it or moves the changes into
`systemctl --user edit`.

## Hardening

`packaging/systemd/humanitld.service` is hardened as far as the sandbox allows.
Every line was measured on 2026-09-18 on Debian 14 with systemd 262 and
bubblewrap 0.12.0, with `systemd-run --user` and the same properties: the escape
tests (`tests/escape/run.sh`) pass under it with 124 of 124 cases, including the
three sandbox guarantees and the agent's terminal; a writable project under
`$HOME` is writable in the sandbox; node, python3 and java run under it. This
matters because **every child of the daemon inherits the restrictions of the
unit**: the sandbox backend, the shim and the agent. What stands here applies to
the agent as well.

`systemd-analyze --user security --offline=true` on the unit with the path of
the package:

Overall exposure level: 3.7 (systemd 262, Debian 14, 2026-09-18; limit from HUM-053: 4.0)

The test `the_exposure_stays_at_or_below_the_documented_value` in
`daemon/bin/humanitl/src/cmd/unit.rs` reads the value above and measures the
unit again on every run where `systemd-analyze` can, and keeps both at 4.0 or
below.

### What is set

| Line | Reason |
|---|---|
| `Type=notify` | The daemon sends `READY=1` as soon as token and socket are in place. |
| `NoNewPrivileges=yes` | bubblewrap runs without setuid through user namespaces and needs no new privileges. |
| `PrivateTmp=yes` | The daemon's `/tmp` is its own. A project under `/tmp` is therefore invisible to it. |
| `ProtectSystem=strict` | In a user unit, everything is read-only afterwards except `$HOME`, the runtime directory and the API file systems (measured, not read). |
| `ReadWritePaths=-%h/.local/share/humanitl -%h/.config/humanitl -%t/humanitl` | The three directories of the daemon, each with `-`, because systemd does not start a unit with a missing path. |
| `PrivateIPC=yes`, `PrivateMounts=yes`, `PrivateUsers=yes` | Own IPC and mount namespaces, only the own user visible. |
| `ProtectClock=yes`, `ProtectControlGroups=yes`, `ProtectKernelModules=yes` | Clock, cgroups and kernel modules are none of the daemon's business. |
| `ProtectProc=invisible` | Processes of other users are invisible. |
| `KeyringMode=private`, `LockPersonality=yes`, `RestrictRealtime=yes` | Standard hardening without effect on the sandbox. |
| `RestrictFileSystems=~@privileged-api` | No file systems such as `bpf` or `debugfs`. |
| `CapabilityBoundingSet=CAP_SYS_ADMIN` | A user service has no capabilities; the bound acts on programs with file capabilities. It must not be empty (see below). |
| `SystemCallFilter=@system-service @mount @sandbox sethostname`, `SystemCallErrorNumber=EPERM` | `@mount`, because bubblewrap mounts and calls `pivot_root`; `@sandbox`, because the shim sets its own seccomp filter with `seccomp(2)`; `sethostname`, because bubblewrap names the UTS namespace of the sandbox `sandbox` ("Can't set hostname to sandbox" without it). The host name of the machine does not change. |
| `RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6 AF_NETLINK` | bubblewrap needs `AF_NETLINK` for `lo` in the namespace, the proxy needs IPv4 and IPv6 to reach the destination. |

### What is left out on purpose

| Line | Measured consequence |
|---|---|
| `RestrictNamespaces` | bubblewrap: "No permissions to create a new namespace". The sandbox needs user, mnt, pid, net, ipc and uts. |
| `ProtectKernelTunables=yes` | bubblewrap: "Can't mount proc on /proc: Operation not permitted". A covered `/proc/sys` forbids a new `/proc`. |
| `ProtectKernelLogs=yes` | The same error, because of the covered `/proc/kmsg`. |
| `ProtectHostname=yes` | Harmless alone; together with the other lines the same error when `/proc` is mounted. |
| `RestrictSUIDSGID=yes` | bubblewrap: "Can't open source /usr: Function not implemented". |
| `MemoryDenyWriteExecute=yes` | Inherited by the agent: node (V8) aborts at start with "Check failed: 12 == (*__errno_location ())", as does every other JIT runtime. |
| `CapabilityBoundingSet=` (empty) | bubblewrap does not start. |
| `SystemCallArchitectures=native` | The shim checks its own filter with an x32 call of `socket` and dies of `SIGSYS`; the sandbox does not start (`SANDBOX_016`, "families not reported"). |
| `PrivateDevices=yes` | Without `/dev/ptmx` the daemon opens no terminal for the agent (`TERM_002`, "cannot open a pseudo terminal"). |
| `UMask=0077` | Inherited by the agent: every file it creates in the project would get other permissions than without Humanitl. |
| `ProtectHome=read-only` | A project under `$HOME` would be read-only in the sandbox; `sandbox.work_mode = rw` is the default. |
| `ProcSubset=pid` | `humanitl doctor` reads `/proc/sys/kernel/*` and `/proc/modules`; those findings would be lost. |
| `PrivateNetwork`, `IPAddressDeny` | The proxy has to reach the destination that a person approves. |

### Projects outside `$HOME`

Under `ProtectSystem=strict`, a project outside the home directory, for example
under `/srv` or on a separate disk, is read-only for the daemon and therefore in
the sandbox. A drop-in for the unit makes it writable:

```sh
systemctl --user edit humanitld.service
# [Service]
# ReadWritePaths=/srv/demo-project
systemctl --user restart humanitld.service
```

A project under `/tmp` is invisible to the daemon because of `PrivateTmp`.

<a id="xdg_runtime_dir-ohne-logind"></a>

## XDG_RUNTIME_DIR without logind

Socket and token of the daemon live in the runtime directory
`$XDG_RUNTIME_DIR/humanitl`. If `XDG_RUNTIME_DIR` is unset and there is no
`/run/user/<uid>`, for example without logind, daemon, command line and app fall
back to `$TMPDIR/humanitl-<uid>`. Any other account can create that name first.
Humanitl refuses such a foreign directory (`DAEMON_001`, `DAEMON_004`) and does
not start while it is there ([SECURITY.md](SECURITY.md)).

The fix is a runtime directory of your own under your home directory. Daemon,
command line and app must see the same variable, so it belongs in the login and
not in front of a single command. Create the directory once:

```sh
mkdir -p "$HOME/.cache/humanitl-run"
chmod 700 "$HOME/.cache/humanitl-run"
```

Then add the following line to the file your login reads. It sets the variable
only if it is missing and `/run/user/<uid>` does not exist, so a later session
with logind keeps its own sockets for Wayland, PipeWire and D-Bus.

```sh
[ -n "$XDG_RUNTIME_DIR" ] || [ -d "/run/user/$(id -u)" ] || export XDG_RUNTIME_DIR="$HOME/.cache/humanitl-run"
```

Which file that is depends on the shell:

- `bash` reads at login only the first existing file of `~/.bash_profile`,
  `~/.bash_login` and `~/.profile`. The line belongs in exactly that file.
  Create `~/.profile` only if neither `~/.bash_profile` nor `~/.bash_login`
  exists.
- `zsh` reads `~/.zprofile`, not `~/.profile`. The line belongs in
  `~/.zprofile`; the file may be created for it.
- Display managers without a systemd session usually read `~/.profile`. Whoever
  starts the app from there adds the line there as well.

The line takes effect after the next login. From then on daemon, command line
and app find the same directory. That the clients find it without this line is
HUM-222.

## Building the packages

```sh
make package PACKAGE_VERSION=0.0.N
```

The target builds `humanitld`, `humanitl` and the static shim
(`packaging/release/build-binaries.sh`) into `app/linux/bundle-extra/`, then the
Flutter bundle (`app/linux/CMakeLists.txt` puts the three into `bin/`), then
`dist/humanitl_<version>_amd64.deb` (`packaging/deb/build-deb.sh`) and
`dist/Humanitl-<version>-x86_64.AppImage`
(`packaging/appimage/build-appimage.sh`), and checks the AppImage
(`packaging/appimage/check-appimage.sh`). It needs `dpkg-dev`, `patchelf`,
`curl` and, for the static shim, `musl-tools`. The script downloads
`appimagetool` and the AppImage runtime in pinned versions and checks their
SHA-256 sums before running them.

`Depends` of the `.deb` is derived, not written by hand: `dpkg-shlibdeps` reads
the libraries of the built programs, and `build-deb.sh` adds
`bubblewrap (>= MIN_BWRAP_VERSION)` from `daemon/crates/sandbox/src/bwrap.rs`,
`libegl1` and `libgles2`. The packages are built on Ubuntu 24.04, so the
derived library versions are those of that system.

The PNG icons under `packaging/deb/icons/` are rendered from
`packaging/deb/humanitl.svg` with Inkscape 1.x. Whoever changes the SVG renders
them again and commits the three images in the same commit:

```sh
packaging/deb/icons/render.sh
```

Origin and licence of the icon (Font Awesome Free, CC BY 4.0) are in
`THIRD_PARTY.md` and, in the package, in `/usr/share/doc/humanitl/copyright`.

The `.deb` is never installed on a workstation but checked in a throwaway
container, with `packaging/deb/check-install.sh` (installation, libraries, start
of the app under Xvfb, units, icons, `daemon install --print`, removal with
`--purge`) and `packaging/deb/lintian.sh` (no errors, no warnings; every
exception stands with its reason in `packaging/deb/lintian-overrides`).

## Manual test matrix

What no container shows, a person checks on real hardware, for every pre-release
and before 0.1.0. Result and date go into the table; an empty cell means: not
checked yet.

| Check | Debian 13 GNOME Wayland (Intel) | Debian 13 KDE Wayland (NVIDIA proprietary) | Ubuntu 24.04 GNOME X11 |
|---|---|---|---|
| `sudo apt install ./humanitl_…deb` on a fresh VM, then `humanitl daemon install` and `humanitl daemon status` with exit 0 | | | |
| App starts, setup shows the daemon green | | | |
| Tray icon (GNOME needs the extension "AppIndicator and KStatusNotifierItem Support"; the setup points it out) | | | |
| Window sharp and complete at 200 % scaling | | | |
| Notification for a held request | | | |
| Sandbox start with a writable project under `$HOME` under the unit | | | |
| Impeller renders (window not black, `humanitl doctor` without `DOCTOR_010`) | | | |
| AppImage starts without bundled GTK | | | |
| [Install guide](guides/install.md) and [first session](guides/first-session.md) followed step by step on a fresh Debian 13 VM with only the `.deb`, until the first held request, in under 15 minutes | | | |

If Impeller fails on a driver (black window, crash at start), the finding goes
into this table with driver and version. The release build has no fallback to
another renderer yet: switches of the Flutter engine such as
`--no-enable-impeller` are passed by the Linux runner to the Dart code, not to
the engine, and the engine's environment variables apply, according to its
source, only outside release builds. Neither is measured here (HUM-168).
