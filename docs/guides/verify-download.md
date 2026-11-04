# Verifying a download

Applies to: Humanitl 0.1

Every release on the [releases page](https://github.com/nurkert/Humanitl/releases) carries a file
`SHA256SUMS` with one SHA-256 checksum for each package, archive and disk image. This page shows
how to check a download against it, and what that check does and does not prove.

## What a release contains

`SHA256SUMS` covers exactly these six files:

| File | For |
|---|---|
| `humanitl_<version>_amd64.deb` | Debian 13, Ubuntu 24.04 or newer |
| `humanitl-<version>-1.x86_64.rpm` | Fedora 40 or newer, openSUSE Tumbleweed |
| `Humanitl-<version>-x86_64.AppImage` | an x86_64 Linux desktop with glibc 2.39 or newer, no installation |
| `humanitl-<version>-linux-x86_64.tar.gz` | the archive, same requirement; the AUR package `humanitl-bin` installs it |
| `Humanitl-<version>-macos.dmg` | macOS, Apple Silicon and Intel |
| `Humanitl-<version>-macos.zip` | the same app as a zip |

The Linux programs are built on Ubuntu 24.04 and need glibc 2.39 or newer: Debian 13, Ubuntu
24.04, Fedora 40 or newer. Debian 12 and Ubuntu 22.04 are too old.

The release also carries files that `SHA256SUMS` does **not** cover: `update-*.json` and
`update-*.json.sig`. They are the manifests for updates from inside the app, each with an Ed25519
signature. You do not check them by hand; the daemon checks the signature against a key built into
it before it believes anything in the manifest (see below).

## Check the checksum

Download `SHA256SUMS` from the same release into the folder that holds your download, then run in
that folder:

```sh
sha256sum -c SHA256SUMS --ignore-missing               # Linux
shasum -a 256 -c SHA256SUMS --ignore-missing           # macOS
```

A good download prints its name followed by `OK`, for example
`humanitl_0.1.0_amd64.deb: OK`, and the command exits with 0. `--ignore-missing` skips the files of
the release you did not download.

Anything else means: do not install it.

- `FAILED` and `computed checksum did NOT match`: the file differs from the one the release was
  built with. Download it again.
- `no file was verified`: none of the names in `SHA256SUMS` is in the folder. Browsers sometimes
  rename a second download (`humanitl_0.1.0_amd64(1).deb`); give the file its original name and run
  the command again.

## What the checksum proves

`SHA256SUMS` comes from the same release as the files. It shows that your download is complete and
unchanged on the way from GitHub to your machine. It does **not** show that the release itself is
genuine: whoever could replace a file on the releases page could replace `SHA256SUMS` as well.

Releases before 1.0.0 are **not signed**. Signed releases come with 1.0.0. Until then, three
paths give more than the checksum:

- **Updates from inside the app** check an Ed25519 signature over the update manifest against a
  public key built into the daemon, then the size and SHA-256 of the download; see
  [Updates from inside the app](../SECURITY.md#updates-from-inside-the-app).
- **The APT repository** is signed with its own key, and `apt` checks that signature; see
  [APT repository](../INSTALL.md#apt-repository).
- **Building from source** at the tag of the release, with the steps in
  [`CONTRIBUTING.md`](https://github.com/nurkert/Humanitl/blob/main/CONTRIBUTING.md).

The macOS app is signed ad hoc, not with an Apple Developer ID. The ad-hoc signature shows that
the bundle is internally consistent; it says nothing about who built it.
