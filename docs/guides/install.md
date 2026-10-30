# Installing Humanitl

Applies to: Humanitl 0.1

This guide takes a desktop Linux machine from nothing to a running Humanitl
daemon with a passing sandbox check. It takes about ten minutes. The next step
after it is [your first session](first-session.md).

Humanitl ships in five forms for x86_64. All of them contain the same four
programs: the desktop app, the command line `humanitl`, the daemon `humanitld`
and the static shim `humanitl-shim` that runs inside the sandbox.

| Form | File | Best for |
|---|---|---|
| Debian package | `humanitl_<version>_amd64.deb` | Debian and Ubuntu; updates through apt |
| RPM package | `humanitl-<version>-1.x86_64.rpm` | Fedora 40 and newer, openSUSE Tumbleweed |
| AUR package | `humanitl-bin`, built from the archive | Arch Linux and its derivatives, once it is published |
| AppImage | `Humanitl-<version>-x86_64.AppImage` | other distributions, or trying it without root |
| Archive | `humanitl-<version>-linux-x86_64.tar.gz` | servers and scripted setups without the app |

Flatpak, Snap and arm64 are not part of 0.1, and neither is a dnf repository:
the RPM is installed from the file, like the `.deb` without the APT repository.

The daemon always runs as a **systemd user service** of the person who uses it,
never as a system service and never with `sudo`. It reads one person's runtime
directory, configuration and recording, and nobody else's. That is also why no
package switches it on by itself: each person enables it once, with one command
or one click in the app.

## Prerequisites

| What | Why | How to check |
|---|---|---|
| Debian 13 or Ubuntu 24.04, or newer, on x86_64; for the RPM Fedora 40 or newer or openSUSE Tumbleweed; for the AUR package a current Arch Linux | the programs are built on Ubuntu 24.04 and need glibc 2.39 or newer, which RHEL 9 (2.34) and openSUSE Leap 15 (2.31) do not have | `cat /etc/os-release`, `ldd --version` |
| systemd with a user session | the daemon is a user service | `systemctl --user is-system-running` answers `running` |
| bubblewrap 0.8.0 or newer | the sandbox backend; older versions are refused (`SANDBOX_002`) | `bwrap --version` |
| unprivileged user namespaces | bubblewrap opens the sandbox without setuid | see [User namespaces](#user-namespaces) |
| a kernel with seccomp, 5.4 or newer | the third guarantee, no new sockets | `grep Seccomp /proc/self/status` |
| GTK 3, GLib, EGL and GLES 2 | the desktop app | the `.deb`, the `.rpm` and the AUR package pull them in |
| `libayatana-appindicator3-1` (optional) | the tray icon; without it the app runs and the window title carries the count of held requests | `humanitl doctor`, line `tray` |
| `libsecret-tools` (recommended) | keeps an access key for the model server in the system keyring; without it the key lives in a file with mode `0600` | `command -v secret-tool` |

The `.deb` declares bubblewrap, EGL and GLES 2 as dependencies, so apt installs
them; the `.rpm` and the AUR package declare the same, for dnf, zypper and pacman. GTK 3 and GLib come in through the libraries of the app. The tray library
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

The package writes nothing under `/etc`, `/var` or your home directory. Its
only maintainer scripts reload the user units of logged-in users after an
install, an upgrade or a removal, and restart a running daemon that has no
agent session (see [Update](#update) and `docs/INSTALL.md`).

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

## Install the RPM

For Fedora 40 and newer and openSUSE Tumbleweed. The RPM holds the same files at
the same paths as the `.deb` (see the table above); only the licence texts lie
under `/usr/share/licenses/humanitl/`. The release run installs, checks and
removes it in a Fedora 44 container; Tumbleweed is not tested. Older systems
such as RHEL 9 or openSUSE Leap 15 refuse it, because their glibc is older
than 2.39.

```sh
sudo dnf install ./humanitl-<version>-1.x86_64.rpm
humanitl daemon install
humanitl daemon status
```

On openSUSE the release is not signed, so zypper asks for it explicitly:

```sh
sudo zypper install --allow-unsigned-rpm ./humanitl-<version>-1.x86_64.rpm
```

Like the `.deb`, the package writes nothing under `/etc`, `/var` or your home
directory, and `humanitl daemon install` only enables the socket it ships (see
[Enable the user service](#enable-the-user-service)). Its scriptlets reload the
user units of logged-in users after an install, an upgrade or a removal, and
restart a running daemon that has no agent session.

## Install from the AUR

The AUR package `humanitl-bin` takes the archive
`humanitl-<version>-linux-x86_64.tar.gz` of a release, checks its SHA-256 and
installs it with the same layout as the `.deb`, units and menu entry included.
The release run builds it with makepkg and installs and removes it in an Arch
Linux container. The maintainer publishes it to the AUR by hand after each
release, so it can lag behind the release page for a while; as long as
<https://aur.archlinux.org/packages/humanitl-bin> does not exist, the package is
not published yet.

```sh
git clone https://aur.archlinux.org/humanitl-bin.git
cd humanitl-bin
less PKGBUILD humanitl-bin.install   # read what you are about to run as root
makepkg -si
humanitl daemon install
humanitl daemon status
```

An AUR helper such as `yay -S humanitl-bin` does the same. The package needs
`bubblewrap` 0.8.0 or newer from the Arch repositories; `libsecret` and
`libayatana-appindicator` are optional.

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

## Install on macOS

Humanitl builds for macOS 12 or newer, on Apple Silicon and Intel; the release run tests it on
macOS 15, and it was used on macOS 27. On other versions the isolation check at every start is
what tells you the sandbox holds. The release carries
`Humanitl-<version>-macos.dmg` and the same app as `Humanitl-<version>-macos.zip`.

1. Open the disk image and drag Humanitl into Applications.
2. The app is signed ad hoc, not with an Apple Developer ID, so macOS refuses the first start.
   Allow it once under System Settings › Privacy & Security › Open Anyway, or in a terminal:

   ```sh
   xattr -dr com.apple.quarantine /Applications/Humanitl.app
   ```

   Start the app from Applications, not from the disk image or Downloads: macOS runs a
   quarantined app from a temporary copy, and the service would point at that copy.
3. On the setup screen, "Set up the service" runs `humanitl daemon install`. It writes
   `~/Library/LaunchAgents/dev.humanitl.humanitld.plist`, and launchd starts the daemon now and at
   every login. Its log is `~/Library/Logs/Humanitl/humanitld.log`.

The command line lies in the app; link it into your `PATH` if you use it often:

```sh
ln -s /Applications/Humanitl.app/Contents/MacOS/bin/humanitl /usr/local/bin/humanitl
humanitl daemon status
humanitl sandbox check        # three green lines
humanitl daemon restart       # launchctl kickstart, after an update
humanitl daemon uninstall     # removes the launch agent
```

Socket and token lie in `~/Library/Application Support/Humanitl/run`. The sandbox is Seatbelt,
not bubblewrap; there is nothing to install for it. What it guarantees on macOS is in
[`SECURITY.md` section 12](../SECURITY.md#12-macos-seatbelt-instead-of-bubblewrap). The agent must
be readable from inside the sandbox, and the home directory is not: a single native binary such as
OpenCode's may lie anywhere (the start allows that one file), but an agent with support files,
pi with its Node modules for example, must be installed outside the home directory, through
`npm install -g` with the Node of Homebrew. `brew install opencode` works as well.

## Install the agent

Humanitl 0.1 ships two agent adapters, OpenCode (the default) and pi. Install
OpenCode as its project documents it, for example:

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

To use pi instead, install it and Node.js 22.19 or newer under `/usr`, where
the sandbox sees them, and select it:

```sh
sudo npm install -g --ignore-scripts @earendil-works/pi-coding-agent
humanitl config set agent.adapter pi
```

pi is a Node.js script; a pi or a Node.js under your home directory (the
`pi.dev` installer, nvm) is not visible in the sandbox. See
[Using pi](agent-profiles.md#using-pi).

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
  `apt upgrade` from the APT repository). Nothing else to type: the package
  reloads your user units and restarts your daemon with the new version. If an
  agent session is running, the daemon keeps running until it ends; a daemon of
  any release after 0.0.6 then ends by itself, and the next connection starts the new
  version. Until then the app's status bar names the other version and offers
  **Restart daemon** once no session runs; the command line equivalent is
  `systemctl --user daemon-reload && systemctl --user restart humanitld`.
- **RPM:** `sudo dnf install ./humanitl-<newer>-1.x86_64.rpm` (openSUSE:
  `sudo zypper install --allow-unsigned-rpm ./…rpm`). The scriptlet reloads and
  restarts like the `.deb` does.
- **AUR:** `git pull` in the `humanitl-bin` checkout, then `makepkg -si`, or
  the update of your AUR helper. The install script reloads and restarts like
  the `.deb` does.
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

**RPM** on Fedora:

```sh
humanitl daemon uninstall
sudo dnf remove humanitl
```

**RPM** on openSUSE:

```sh
humanitl daemon uninstall
sudo zypper remove humanitl
```

**AUR package** on Arch Linux:

```sh
humanitl daemon uninstall
sudo pacman -Rns humanitl-bin
```

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
