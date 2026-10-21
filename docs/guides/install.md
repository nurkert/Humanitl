# Installing Humanitl

Applies to: Humanitl 0.1

This guide takes a desktop Linux machine from nothing to a running Humanitl
daemon with a passing sandbox check. It takes about ten minutes. The next step
after it is [your first session](first-session.md).

Humanitl ships in three forms for x86_64. All three contain the same four
programs: the desktop app, the command line `humanitl`, the daemon `humanitld`
and the static shim `humanitl-shim` that runs inside the sandbox.

| Form | File | Best for |
|---|---|---|
| Debian package | `humanitl_<version>_amd64.deb` | Debian and Ubuntu; updates through apt |
| AppImage | `Humanitl-<version>-x86_64.AppImage` | other distributions, or trying it without root |
| Archive | `humanitl-<version>-linux-x86_64.tar.gz` | servers and scripted setups without the app |

Flatpak, Snap, RPM and arm64 are not part of 0.1.

The daemon always runs as a **systemd user service** of the person who uses it,
never as a system service and never with `sudo`. It reads one person's runtime
directory, configuration and recording, and nobody else's. That is also why no
package switches it on by itself: each person enables it once, with one command
or one click in the app.

## Prerequisites

| What | Why | How to check |
|---|---|---|
| Debian 13 or Ubuntu 24.04, or newer, on x86_64 | the targets the packages are built and tested for | `cat /etc/os-release` |
| systemd with a user session | the daemon is a user service | `systemctl --user is-system-running` answers `running` |
| bubblewrap 0.8.0 or newer | the sandbox backend; older versions are refused (`SANDBOX_002`) | `bwrap --version` |
| unprivileged user namespaces | bubblewrap opens the sandbox without setuid | see [User namespaces](#user-namespaces) |
| a kernel with seccomp, 5.4 or newer | the third guarantee, no new sockets | `grep Seccomp /proc/self/status` |
| GTK 3, GLib, EGL and GLES 2 | the desktop app | the `.deb` pulls them in |
| `libayatana-appindicator3-1` (optional) | the tray icon; without it the app runs and the window title carries the count of held requests | `humanitl doctor`, line `tray` |
| `libsecret-tools` (recommended) | keeps an access key for the model server in the system keyring; without it the key lives in a file with mode `0600` | `command -v secret-tool` |

The `.deb` declares bubblewrap, EGL and GLES 2 as dependencies, so apt installs
them. GTK 3 and GLib come in through the libraries of the app. The tray library
and `libsecret-tools` are optional; the package recommends `libsecret-tools`.

On GNOME the tray icon also needs the shell extension "AppIndicator and
KStatusNotifierItem Support" (Debian: `gnome-shell-extension-appindicator`).
The setup screen of the app points this out when it is missing.

You do not have to check all of this by hand. After installing, `humanitl doctor`
prints one line per precondition, and every line that is not `ok` carries the
command that fixes it.

### User namespaces

Humanitl starts the sandbox with bubblewrap as an ordinary user. That needs
unprivileged user namespaces. Debian and Ubuntu allow them by default, with two
exceptions worth knowing:

- **A Debian kernel switch.** Debian kernels carry the switch
  `kernel.unprivileged_userns_clone`. When it is `0`, nobody can open a user
  namespace. Check and fix:

  ```sh
  sysctl kernel.unprivileged_userns_clone
  sudo sysctl -w kernel.unprivileged_userns_clone=1
  ```

  The general limit `user.max_user_namespaces` must not be `0` either
  (`sysctl user.max_user_namespaces`).

- **The AppArmor restriction of Ubuntu 23.10 and later.** Ubuntu restricts
  unprivileged user namespaces through AppArmor
  (`kernel.apparmor_restrict_unprivileged_userns = 1`). The `bwrap` from the
  Ubuntu archive carries an AppArmor profile and is allowed even so; a
  bubblewrap from somewhere else is not. If the doctor line `userns` fails and
  names this switch, either use the distribution's `bwrap` or, knowing that it
  lifts the restriction for every program, turn it off:

  ```sh
  sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0
  ```

  `sysctl -w` lasts until the next boot. To keep a setting, put it in a file
  under `/etc/sysctl.d/`.

`humanitl doctor` measures this with bubblewrap itself (`bwrap --unshare-user`),
not with `unshare`, because that is the path the sandbox takes. Its `userns`
line names the switch that is in the way and the command for it (`DOCTOR_002`).

## Install the Debian package

Download `humanitl_<version>_amd64.deb` from the release page of the project
and install it with apt, which resolves the dependencies:

```sh
sudo apt install ./humanitl_<version>_amd64.deb
humanitl daemon install
humanitl daemon status
```

An APT repository that carries every release is described in the
[maintainer notes](../INSTALL.md#apt-repository).

What the package puts where:

| Path | Content |
|---|---|
| `/usr/lib/humanitl/` | the app bundle, and under `bin/` the programs `humanitld`, `humanitl` and `humanitl-shim` |
| `/usr/bin/humanitl` | link to the command line |
| `/usr/bin/humanitl-app` | link to the desktop app |
| `/usr/lib/systemd/user/humanitld.service`, `humanitld.socket` | the user units |
| `/usr/share/humanitl/catalog/` | the domain catalog |
| `/usr/share/humanitl/profiles/sandbox/default.toml` | the sandbox profile |
| `/usr/share/applications/`, `/usr/share/icons/hicolor/` | menu entry and icons |

The package writes nothing under `/etc`, `/var` or your home directory and has
no maintainer scripts.

### Enable the user service

`humanitl daemon install` recognises the units of the package and **writes
nothing**. It only runs what the package cannot do as root:

```sh
systemctl --user daemon-reload
systemctl --user enable --now humanitld.socket
```

Only the socket is enabled, not the service. systemd starts the daemon on the
first connection, from the app or the command line, and the socket path
`$XDG_RUNTIME_DIR/humanitl/daemon.sock` stays in place across restarts of the
service. A client that finds no token of the daemon yet waits up to ten seconds
for the service to come up, then ends with `DAEMON_001` and points to
`humanitl daemon logs`. To start the daemon with every login instead, also run
`systemctl --user enable humanitld.service`.

To change a setting of the unit, do not copy it. Create a drop-in with
`systemctl --user edit humanitld.service`; a copy under
`~/.config/systemd/user/` would hide the packaged unit from every update. Why
the unit is hardened the way it is, and how to give the daemon a writable
project outside your home directory, is in the
[maintainer notes](../INSTALL.md#hardening).

Instead of the command, the app offers the same step: the first row of its
setup screen, "Background service", shows the button "Install and start" when
no daemon answers. It runs the `humanitl` next to the app, never one from
`PATH`, and turns green once the service answers.

## Install the AppImage

```sh
chmod +x Humanitl-<version>-x86_64.AppImage
./Humanitl-<version>-x86_64.AppImage                       # the app
./Humanitl-<version>-x86_64.AppImage --cli daemon install  # the service
./Humanitl-<version>-x86_64.AppImage --cli daemon status
```

`--cli` passes everything after it to the command line inside the image.
`daemon install` sees that it runs from an AppImage (`$APPIMAGE`) and copies the
daemon, the shim, the domain catalog and the sandbox profile to
`~/.local/lib/humanitl/<version>.<stamp>/`, because the mount point of the
image disappears with the process. It then writes
`~/.config/systemd/user/humanitld.service`, which starts
`~/.local/lib/humanitl/current/bin/humanitld`, and enables it. The "Install and
start" button of the setup screen does the same.

A newer AppImage refreshes the service on its own: every start of the image
runs `daemon install --refresh` first, which copies the new version, switches
`current` and restarts the service, but only if the service was set up before.
The first setup is never done by simply starting the app.

The image contains only the app bundle and the programs. GTK, GLib, Mesa and the
Wayland libraries come from your system. An AppImage needs `libfuse2` to mount
itself (Ubuntu 24.04: `libfuse2t64`). Without FUSE, run it with
`--appimage-extract-and-run`.

## Install the archive

```sh
tar -xzf humanitl-<version>-linux-x86_64.tar.gz
humanitl-<version>-linux-x86_64/bin/humanitl daemon install
humanitl-<version>-linux-x86_64/bin/humanitl daemon status
```

`daemon install` writes `~/.config/systemd/user/humanitld.service` with the
path of the daemon in the unpacked directory and enables it. There is no socket
unit on this path; the daemon binds its socket itself. Keep the directory where
it is, or run `daemon install` again after moving it.

## Install the agent

Humanitl 0.1 ships one agent adapter, OpenCode. Install OpenCode as its project
documents it, for example:

```sh
curl -fsSL https://opencode.ai/install | bash
```

The sandbox mounts `/usr` read-only and searches
`PATH=/usr/local/bin:/usr/bin:/bin`. A binary under your home directory, such as
`~/.opencode/bin/opencode`, is not visible inside it. Put a copy where the
sandbox finds it:

```sh
sudo install -m 0755 ~/.opencode/bin/opencode /usr/local/bin/opencode
```

If you skip this, the start of a session stops with `AGENT_004` and names
exactly this command. The [agent profiles guide](agent-profiles.md) explains
what Humanitl configures for OpenCode and how to run another program as the
agent.

## Check the installation

Three commands, in this order:

```sh
humanitl daemon status
humanitl doctor
humanitl sandbox check
```

`humanitl daemon status` asks the daemon who it is and exits with 0:

```
FIELD         VALUE
socket        /run/user/1000/humanitl/daemon.sock
unit          active
daemon        0.1.0
proto         1.0
session       7b1c…
capabilities  hold, rules, sandbox
```

Exit code 2 with `DAEMON_001` means nothing answers on the socket; see
[Troubleshooting](#troubleshooting).

`humanitl doctor` prints eleven lines (`bwrap`, `userns`, `seccomp`,
`runtime_dir`, `systemd_user`, `daemon`, `agent`, `llm`, `tray`, `renderer`,
`disk_space`). The first four are the preconditions of the sandbox; nothing
starts while one of them fails. The `llm` line says "not contacted" until you
set a model server: the doctor opens no network connection unless you ask for
it with `--probe-llm`.

`humanitl sandbox check` starts a short-lived sandbox, measures the three
isolation guarantees inside it and stops it again:

```
   CHECK                 EVIDENCE
✓  no_network_interface  …
✓  single_socket         …
✓  seccomp_active        …
```

All three must be `✓`. A failed guarantee ends the command with exit code 3 and
the finding of the first check that failed. What each guarantee means is in
[SECURITY.md](../SECURITY.md).

## Troubleshooting

**`DAEMON_001`, nothing answers.** The finding names a command that sets up or
starts the service: `humanitl daemon install` if there is no unit yet,
`systemctl --user start humanitld.socket` with the package, and
`systemctl --user start humanitld.service` with the unit that `daemon install`
wrote. `humanitl daemon logs` shows the journal of the service
(`journalctl --user -u humanitld`). For debugging only, `humanitld` also runs in
the foreground of a terminal and ends with it.

**No `XDG_RUNTIME_DIR`.** Without logind there may be no
`/run/user/<uid>`. Daemon, command line and app then fall back to a directory
under `$TMPDIR` that another account could create first, and Humanitl refuses to
use one it does not own (`DAEMON_001`, `DAEMON_004`). The fix is a runtime
directory of your own under your home directory, set for the whole login:
[XDG_RUNTIME_DIR without logind](../INSTALL.md#xdg_runtime_dir-without-logind).

**`DAEMON_005` on `daemon install`.** There is a file
`~/.config/systemd/user/humanitld.service` that `daemon install` did not write.
Humanitl never overwrites it. Move it away, or turn your changes into a drop-in
with `systemctl --user edit humanitld.service`.

**Black window on start.** A loaded NVIDIA module under Wayland is a known
cause; `humanitl doctor` reports it as `DOCTOR_010`. The release build has no
switch to another renderer yet.

Every code is explained in [DIAGNOSTICS.md](../DIAGNOSTICS.md).

## Update

- **Package:** install the newer `.deb` with `sudo apt install ./…deb` (or
  `apt upgrade` from the APT repository). The socket unit keeps working; the
  next connection starts the new daemon. Run `systemctl --user restart
  humanitld.service` if one is still running.
- **AppImage:** start the newer image. It refreshes the service by itself.
- **Archive:** unpack the new version and run its `bin/humanitl daemon install`.

## Uninstall

First remove the access key of the model server from the keyring, if you stored
one, while the command line is still there:

```sh
humanitl llm key remove
```

Then remove the service and the programs.

**Package:**

```sh
humanitl daemon uninstall
sudo apt remove --purge humanitl
```

`humanitl daemon uninstall` disables the socket and the service (the same as
`systemctl --user disable --now humanitld.socket humanitld.service`). The units
belong to the package, which removes them.

**AppImage:**

```sh
./Humanitl-<version>-x86_64.AppImage --cli daemon uninstall --purge-binaries
rm Humanitl-<version>-x86_64.AppImage
```

This disables the service and removes the unit, its links, a leftover socket
and everything `daemon install` placed under `~/.local/lib/humanitl/`.

**Archive:**

```sh
humanitl-<version>-linux-x86_64/bin/humanitl daemon uninstall
rm -r humanitl-<version>-linux-x86_64
```

### Your data

None of the commands above touches your data. Configuration, rules, recording,
the audit log and the private key of the local CA belong to you, not to the
package, and stay where they are:

| Path | Content |
|---|---|
| `~/.config/humanitl/config.toml` | your configuration ([CONFIG.md](../CONFIG.md)) |
| `~/.config/humanitl/rules.yaml` | your saved rules ([rules reference](../reference/rules.md)) |
| `~/.config/humanitl/profiles/` | your own session profiles, and under `sandbox/` your own sandbox profiles |
| `~/.local/share/humanitl/humanitl.db` | the recording: every request and response |
| `~/.local/share/humanitl/blobs/` | request and response bodies of the recording |
| `~/.local/share/humanitl/audit/audit.jsonl` | the audit chain |
| `~/.local/share/humanitl/ca/ca.key`, `ca.crt` | the local CA that the proxy uses inside the sandbox; it is never added to the host's trust store |
| `~/.local/share/humanitl/keys/` | an access key stored as a file, when the keyring was not available |
| `$XDG_RUNTIME_DIR/humanitl/` | socket and token of the daemon; gone after logout |

The table assumes the default `XDG_CONFIG_HOME=~/.config` and
`XDG_DATA_HOME=~/.local/share`. The recording and the audit log may hold
request bodies, so delete them on purpose, not by accident. To remove
everything:

```sh
rm -r ~/.config/humanitl ~/.local/share/humanitl
```

This cannot be undone. Export the audit chain first if you need it later
(`humanitl audit export --format jsonl --out audit-export.jsonl`).
