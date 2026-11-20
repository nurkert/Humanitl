//! Tests of an install from a package of several files, checked against the
//! vendor's list of checksums (HUM-566).
//!
//! A stand-in recipe copies a fixture tree as the run's result; the real
//! Codex script runs on the host under `/bin/sh` with a stand-in `curl` that
//! serves a vendor directory of the test and the host's `tar`, `awk` and
//! `sha256sum`. Nothing reaches a network or a real vendor.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use super::tests::{Fixture, assert_failed_at, hex_sum, snapshot};
use super::*;
use crate::agent::depot::InstallHost;
use crate::agent::depot::recipe::{ChecksumList, DepotRecipe, LoginFlow};
use crate::agent::depot::store::CURRENT_LINK;

const PACKAGE_SCRIPT: &str = r#"cp -R "$FIXTURE/program" "$out/"
cp -R "$FIXTURE/checked" "$out/"
cp "$FIXTURE/version" "$out/version"
"#;

static PACKAGE: KnownAgent = KnownAgent {
    id: "pkg",
    display_name: "Package",
    program: "pkgtool",
    host_installs: &["/usr/bin/pkgtool"],
    recipe: Some(DepotRecipe {
        program: "pkgtool",
        config_env: Some("PKG_CONFIG_DIR"),
        update_off: &[],
        credential_file: None,
        install: Some(InstallRecipe {
            script: PACKAGE_SCRIPT,
            hosts: &[InstallHost {
                host: "downloads.standin.test",
                evidence: "https://standin.test/docs",
            }],
            program: "bin/pkgtool",
            expected: &["helper/rg"],
            platforms: &[(HostPlatform::LinuxX64, "x86_64")],
            signature: None,
            checksums: Some(ChecksumList {
                version_pointer: "/tag_name",
                version_prefix: "v",
                list: "SUMS",
                archive: "pkg-{platform}.tar.gz",
                evidence: "https://standin.test/docs",
            }),
            source: "https://downloads.standin.test/releases",
        }),
        install_refused: None,
        login: LoginFlow::ApiKeyOnly,
        credential_format: None,
        login_run: None,
        command_keys: &[],
    }),
};

fn write(path: &Path, bytes: &[u8], mode: u32) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
}

/// A release of the stand-in package as the run would leave it: the tree,
/// and metadata, list and archive whose chain holds.
fn package(fixture: &Fixture, version: &str) -> PathBuf {
    let dir = fixture.base.path().join(format!("package-{version}"));
    write(
        &dir.join("program/bin/pkgtool"),
        format!("#!/bin/sh\necho pkgtool {version}\n").as_bytes(),
        0o755,
    );
    write(
        &dir.join("program/helper/rg"),
        b"#!/bin/sh\necho rg\n",
        0o755,
    );
    let archive = format!("archive of {version}").into_bytes();
    write(&dir.join("checked/archive"), &archive, 0o644);
    let list = format!(
        "{}  pkg-other.tar.gz\n{}  pkg-x86_64.tar.gz\n",
        hex_sum(b"other"),
        hex_sum(&archive)
    );
    write(&dir.join("checked/list"), list.as_bytes(), 0o644);
    let metadata = serde_json::json!({
        "tag_name": format!("v{version}"),
        "assets": [{"name": "SUMS", "digest": format!("sha256:{}", hex_sum(list.as_bytes()))}],
    });
    write(
        &dir.join("checked/metadata"),
        metadata.to_string().as_bytes(),
        0o644,
    );
    write(
        &dir.join("version"),
        format!("{version}\n").as_bytes(),
        0o644,
    );
    dir
}

/// Runs the stand-in recipe on `release` and commits; the state of the
/// depots before the takeover is returned beside the result.
fn install(
    fixture: &Fixture,
    release: &Path,
) -> (
    std::collections::BTreeMap<PathBuf, String>,
    Result<Committed, Diagnostic>,
) {
    let run = fixture.run(&PACKAGE, MaintenanceTask::Install, None);
    let ok = fixture.execute(&run, release);
    let before = snapshot(&fixture.agents());
    (before, run.commit_at(ok, "2026-11-20T00:00:00Z"))
}

/// Point 1: the whole tree lands in its version directory, every file with
/// its SHA-256, the daemon's link `pkgtool -> bin/pkgtool` at the top, and
/// the directory passes the check before every bind.
///
/// Mutation proof: without the link (`name_program` returns early) the
/// assert on the link turns red.
#[test]
fn a_package_lands_whole_with_the_daemons_link_to_its_program() {
    let fixture = Fixture::new();
    let release = package(&fixture, "1.0.0");
    let (_, committed) = install(&fixture, &release);
    let committed = committed.unwrap();
    assert_eq!(committed.version, "1.0.0");
    assert_eq!(committed.manifest.signature, "unsigned");
    let depot = fixture.depot(&PACKAGE);
    let installed = depot.installed().unwrap();
    assert_eq!(
        installed.manifest.files.keys().collect::<Vec<_>>(),
        ["bin/pkgtool", "helper/rg"]
    );
    assert_eq!(
        std::fs::read_link(installed.dir.join("pkgtool")).unwrap(),
        Path::new("bin/pkgtool")
    );
    assert_eq!(
        std::fs::read(installed.dir.join("pkgtool")).unwrap(),
        b"#!/bin/sh\necho pkgtool 1.0.0\n"
    );
    assert_eq!(
        std::fs::read_link(depot.root().join(CURRENT_LINK)).unwrap(),
        Path::new("program/1.0.0")
    );
    // Neither the archive nor the list enters the depot.
    assert!(!installed.dir.join("checked").exists());
    crate::agent::depot::tree::check_version_dir(
        "pkg",
        &installed.dir,
        &installed.dir,
        &crate::agent::locate::Trust::new(crate::agent::depot::store::tests::uid(), None),
    )
    .unwrap();
}

/// Mutation proof: without the call of `check_list` in `commit_at` this
/// turns red; the run's own comparison is no verdict.
#[test]
fn an_archive_the_list_does_not_name_is_refused_and_the_depot_stays() {
    let fixture = Fixture::new();
    let release = package(&fixture, "1.0.0");
    write(&release.join("checked/archive"), b"planted archive", 0o644);
    let (before, found) = install(&fixture, &release);
    assert_failed_at(&found.unwrap_err(), "checksum");
    assert_eq!(snapshot(&fixture.agents()), before);
    assert!(!fixture.depot(&PACKAGE).exists());
}

#[test]
fn a_list_other_than_the_metadata_names_is_refused() {
    let fixture = Fixture::new();
    let release = package(&fixture, "1.0.0");
    write(&release.join("checked/archive"), b"planted archive", 0o644);
    write(
        &release.join("checked/list"),
        format!("{}  pkg-x86_64.tar.gz\n", hex_sum(b"planted archive")).as_bytes(),
        0o644,
    );
    let (before, found) = install(&fixture, &release);
    let found = found.unwrap_err();
    assert_failed_at(&found, "checksum");
    assert!(found.why.contains("SUMS"), "{}", found.why);
    assert_eq!(snapshot(&fixture.agents()), before);
}

/// The metadata of another release: an authentic older package under a
/// newer version is refused at the pin.
#[test]
fn metadata_of_another_release_is_refused_at_the_pin() {
    let fixture = Fixture::new();
    let release = package(&fixture, "1.0.0");
    write(&release.join("version"), b"1.0.1\n", 0o644);
    let (before, found) = install(&fixture, &release);
    assert_failed_at(&found.unwrap_err(), "pin");
    assert_eq!(snapshot(&fixture.agents()), before);
}

#[test]
fn a_run_without_its_archive_is_refused() {
    let fixture = Fixture::new();
    let release = package(&fixture, "1.0.0");
    std::fs::remove_file(release.join("checked/archive")).unwrap();
    let (before, found) = install(&fixture, &release);
    let found = found.unwrap_err();
    assert_failed_at(&found, "checksum");
    assert!(found.why.contains("checked/archive"), "{}", found.why);
    assert_eq!(snapshot(&fixture.agents()), before);
}

/// Mutation proof: without the loop over `expected` in `stage` this turns
/// red.
#[test]
fn a_package_without_an_expected_helper_is_refused() {
    let fixture = Fixture::new();
    let release = package(&fixture, "1.0.0");
    std::fs::remove_file(release.join("program/helper/rg")).unwrap();
    let (before, found) = install(&fixture, &release);
    let found = found.unwrap_err();
    assert_failed_at(&found, "tree");
    assert!(found.why.contains("helper/rg"), "{}", found.why);
    assert_eq!(snapshot(&fixture.agents()), before);
}

/// Only the daemon names the program: a tree whose top already holds an
/// entry of the program's name (a link to another file of the tree) is
/// refused.
///
/// Mutation proof: without the check of an existing entry the link cannot
/// be made and the step is `commit`, not `tree`: red.
#[test]
fn a_tree_that_names_the_program_itself_is_refused() {
    let fixture = Fixture::new();
    let release = package(&fixture, "1.0.0");
    std::os::unix::fs::symlink("helper/rg", release.join("program/pkgtool")).unwrap();
    let (before, found) = install(&fixture, &release);
    let found = found.unwrap_err();
    assert_failed_at(&found, "tree");
    assert!(found.why.contains("only the daemon"), "{}", found.why);
    assert_eq!(snapshot(&fixture.agents()), before);
}

/// A file of the program's name at the top, regular or a directory, is
/// refused like a link: only the daemon names the program.
#[test]
fn a_file_or_directory_named_like_the_program_is_refused() {
    for make in [
        (|top: &Path| write(&top.join("pkgtool"), b"planted", 0o755)) as fn(&Path),
        |top: &Path| std::fs::create_dir(top.join("pkgtool")).unwrap(),
    ] {
        let fixture = Fixture::new();
        let release = package(&fixture, "1.0.0");
        make(&release.join("program"));
        let (before, found) = install(&fixture, &release);
        let found = found.unwrap_err();
        assert_failed_at(&found, "tree");
        assert!(found.why.contains("only the daemon"), "{}", found.why);
        assert_eq!(snapshot(&fixture.agents()), before);
    }
}

/// An expected helper that is a link (inside the tree, so the tree itself
/// passes) is no regular file and is refused.
#[test]
fn an_expected_helper_that_is_a_link_is_refused() {
    let fixture = Fixture::new();
    let release = package(&fixture, "1.0.0");
    std::fs::remove_file(release.join("program/helper/rg")).unwrap();
    std::os::unix::fs::symlink("../bin/pkgtool", release.join("program/helper/rg")).unwrap();
    let (before, found) = install(&fixture, &release);
    let found = found.unwrap_err();
    assert_failed_at(&found, "tree");
    assert!(found.why.contains("helper/rg"), "{}", found.why);
    assert_eq!(snapshot(&fixture.agents()), before);
}

/// The real Codex recipe runs `sha256sum`, `tar` and the other tools of a
/// Linux host; maintenance runs exist only there (Seatbelt refuses them).
#[cfg(target_os = "linux")]
mod codex_script {
    use super::*;

    // --- The real Codex recipe against a stand-in vendor -----------------------

    const TARGET: &str = "x86_64-unknown-linux-musl";

    /// The stand-in `curl`: serves `$VENDOR` for `https://releases.openai.com/codex/`,
    /// writes every URL it is asked for to `$VENDOR/requests`, and fails like
    /// `curl -f` for anything else.
    const FAKE_CURL: &str = r#"#!/bin/sh
out=
url=
while [ $# -gt 0 ]; do
    case $1 in
        -o) out=$2; shift 2 ;;
        -*) shift ;;
        *) url=$1; shift ;;
    esac
done
echo "$url" >> "$VENDOR/requests"
case $url in
    https://releases.openai.com/codex/*) file="$VENDOR/${url#https://releases.openai.com/codex/}" ;;
    *) echo "curl: (6) not served: $url" >&2; exit 6 ;;
esac
[ -f "$file" ] || { echo "curl: (22) 404 $url" >&2; exit 22; }
if [ -n "$out" ]; then cat "$file" > "$out"; else cat "$file"; fi
"#;

    struct Vendor {
        dir: PathBuf,
        tools: PathBuf,
    }

    impl Vendor {
        fn new(fixture: &Fixture) -> Self {
            let dir = fixture.base.path().join("vendor");
            std::fs::create_dir_all(&dir).unwrap();
            let tools = fixture.base.path().join("codex-tools");
            std::fs::create_dir_all(&tools).unwrap();
            for tool in [
                "cat",
                "tr",
                "sed",
                "head",
                "mkdir",
                "awk",
                "sha256sum",
                "cut",
                "tar",
                "gzip",
            ] {
                let found = ["/usr/bin", "/bin"]
                    .iter()
                    .map(|bin| Path::new(bin).join(tool))
                    .find(|path| path.exists())
                    .unwrap_or_else(|| panic!("{tool} not found"));
                std::os::unix::fs::symlink(found, tools.join(tool)).unwrap();
            }
            write(&tools.join("curl"), FAKE_CURL.as_bytes(), 0o755);
            Self { dir, tools }
        }

        /// Publishes `version` like the vendor: the package as a real `tar.gz`,
        /// the list, the metadata (pretty-printed, as the vendor serves it), and
        /// as `channels/latest` when `latest`.
        fn publish(&self, version: &str, latest: bool) {
            let package = self.dir.join(format!("build-{version}"));
            write(
                &package.join("bin/codex"),
                format!("#!/bin/sh\necho codex {version}\n").as_bytes(),
                0o755,
            );
            write(
                &package.join("bin/codex-code-mode-host"),
                b"#!/bin/sh\n",
                0o755,
            );
            write(&package.join("codex-path/rg"), b"#!/bin/sh\n", 0o755);
            write(
                &package.join("codex-package.json"),
                br#"{"entrypoint":"bin/codex"}"#,
                0o644,
            );
            let release = self.dir.join("releases").join(version);
            std::fs::create_dir_all(&release).unwrap();
            let asset = format!("codex-package-{TARGET}.tar.gz");
            let made = std::process::Command::new("tar")
                .arg("-czf")
                .arg(release.join(&asset))
                .arg("-C")
                .arg(&package)
                .args(["bin", "codex-path", "codex-package.json"])
                .status()
                .unwrap();
            assert!(made.success());
            let archive = std::fs::read(release.join(&asset)).unwrap();
            let list = format!(
                "{}  codex-package-aarch64-apple-darwin.tar.gz\n{}  {asset}\n",
                hex_sum(b"mac"),
                hex_sum(&archive)
            );
            std::fs::write(release.join("codex-package_SHA256SUMS"), &list).unwrap();
            let metadata = serde_json::to_string_pretty(&serde_json::json!({
                "assets": [
                    {"name": "codex-package_SHA256SUMS",
                     "digest": format!("sha256:{}", hex_sum(list.as_bytes()))},
                    {"name": asset, "digest": format!("sha256:{}", hex_sum(&archive))},
                ],
                "tag_name": format!("rust-v{version}"),
            }))
            .unwrap();
            std::fs::write(release.join("release.json"), &metadata).unwrap();
            if latest {
                write(
                    &self.dir.join("channels/latest"),
                    metadata.as_bytes(),
                    0o644,
                );
            }
        }

        /// Rewrites the list of `version` and the digest the metadata names for
        /// it, as a vendor would publish another list.
        fn rewrite_list(&self, version: &str, change: impl FnOnce(String) -> String) {
            let release = self.dir.join("releases").join(version);
            let path = release.join("codex-package_SHA256SUMS");
            let list = change(std::fs::read_to_string(&path).unwrap());
            std::fs::write(&path, &list).unwrap();
            let mut metadata: serde_json::Value =
                serde_json::from_slice(&std::fs::read(release.join("release.json")).unwrap())
                    .unwrap();
            metadata["assets"][0]["digest"] =
                serde_json::Value::from(format!("sha256:{}", hex_sum(list.as_bytes())));
            let text = serde_json::to_string_pretty(&metadata).unwrap();
            std::fs::write(release.join("release.json"), &text).unwrap();
            if self.dir.join("channels/latest").exists() {
                std::fs::write(self.dir.join("channels/latest"), &text).unwrap();
            }
        }

        fn requests(&self) -> String {
            std::fs::read_to_string(self.dir.join("requests")).unwrap_or_default()
        }

        /// Runs the script of `run` as the sandbox would, with this vendor.
        fn execute(&self, run: &MaintenanceRun) -> bool {
            std::process::Command::new("/bin/sh")
                .arg("-c")
                .arg(run.script())
                .env_clear()
                .envs(run.env_for(run.home()))
                .env("PATH", &self.tools)
                .env("VENDOR", &self.dir)
                .status()
                .unwrap()
                .success()
        }
    }

    fn codex() -> &'static KnownAgent {
        crate::agent::depot::recipe::known("codex").unwrap()
    }

    /// Acceptance 1 with a stand-in vendor: `agents install codex --version`
    /// installs the whole package into the depot with the link `codex ->
    /// bin/codex`, without reading `channels/latest`; `agents update codex`
    /// then reads the current version from `channels/latest`. The run reaches
    /// only `releases.openai.com`.
    #[test]
    fn the_codex_recipe_installs_the_package_from_the_vendors_release() {
        let fixture = Fixture::new();
        let vendor = Vendor::new(&fixture);
        vendor.publish("0.170.0", false);
        vendor.publish("0.171.0", true);

        let run = match fixture
            .prepare(codex(), MaintenanceTask::Install, Some("0.170.0"))
            .unwrap()
        {
            Maintenance::Run(run) => run,
            other => panic!("expected a run, got {other:?}"),
        };
        assert_eq!(run.hosts(), ["releases.openai.com"]);
        let ok = vendor.execute(&run);
        let committed = run.commit_at(ok, "t").unwrap();
        assert_eq!(committed.version, "0.170.0");
        assert!(!vendor.requests().contains("channels/latest"));
        let depot = fixture.depot(codex());
        let installed = depot.installed().unwrap();
        assert_eq!(
            installed.manifest.files.keys().collect::<Vec<_>>(),
            [
                "bin/codex",
                "bin/codex-code-mode-host",
                "codex-package.json",
                "codex-path/rg"
            ]
        );
        assert_eq!(
            std::fs::read_link(installed.dir.join("codex")).unwrap(),
            Path::new("bin/codex")
        );
        assert_eq!(installed.manifest.signature, "unsigned");

        let run = match fixture
            .prepare(codex(), MaintenanceTask::Update, None)
            .unwrap()
        {
            Maintenance::Run(run) => run,
            other => panic!("expected a run, got {other:?}"),
        };
        let ok = vendor.execute(&run);
        let committed = run.commit_at(ok, "t").unwrap();
        assert_eq!(committed.version, "0.171.0");
        assert_eq!(committed.previous.as_deref(), Some("0.170.0"));
        assert!(vendor.requests().contains("channels/latest"));
        assert_eq!(
            std::fs::read(depot.installed().unwrap().dir.join("codex")).unwrap(),
            b"#!/bin/sh\necho codex 0.171.0\n"
        );
    }

    /// An archive that is not the listed one never reaches `tar`: the run ends
    /// with an error and leaves no tree; taken over anyway, the daemon's own
    /// check refuses it.
    #[test]
    fn a_codex_archive_that_does_not_match_its_list_is_never_unpacked() {
        let fixture = Fixture::new();
        let vendor = Vendor::new(&fixture);
        vendor.publish("0.171.0", true);
        let asset = vendor
            .dir
            .join("releases/0.171.0")
            .join(format!("codex-package-{TARGET}.tar.gz"));
        let mut bytes = std::fs::read(&asset).unwrap();
        bytes.push(0);
        std::fs::write(&asset, bytes).unwrap();

        let run = match fixture
            .prepare(codex(), MaintenanceTask::Install, None)
            .unwrap()
        {
            Maintenance::Run(run) => run,
            other => panic!("expected a run, got {other:?}"),
        };
        assert!(!vendor.execute(&run), "the run compares before tar");
        let unpacked = std::fs::read_dir(run.home().join("out/program"))
            .unwrap()
            .count();
        assert_eq!(unpacked, 0, "tar saw the archive");
        // The daemon reads no verdict of the run: told it succeeded, it checks
        // the chain itself and refuses.
        std::fs::write(run.home().join("out/version"), "0.171.0\n").unwrap();
        let before = snapshot(&fixture.agents());
        assert_failed_at(&run.commit_at(true, "t").unwrap_err(), "checksum");
        assert_eq!(snapshot(&fixture.agents()), before);
    }

    fn prepared(fixture: &Fixture, version: Option<&str>) -> Box<MaintenanceRun> {
        match fixture
            .prepare(codex(), MaintenanceTask::Install, version)
            .unwrap()
        {
            Maintenance::Run(run) => run,
            other => panic!("expected a run, got {other:?}"),
        }
    }

    /// A list that names the archive twice: the run refuses it before `tar`,
    /// as the daemon does on its own copy, even when both lines agree.
    ///
    /// Mutation proof: the script printing the first line (`n >= 1`) turns this
    /// red.
    #[test]
    fn a_codex_list_with_the_archive_twice_is_refused_by_run_and_daemon() {
        let fixture = Fixture::new();
        let vendor = Vendor::new(&fixture);
        vendor.publish("0.171.0", true);
        vendor.rewrite_list("0.171.0", |list| {
            let line = list.lines().last().unwrap().to_owned();
            format!("{list}{line}\n")
        });
        let run = prepared(&fixture, Some("0.171.0"));
        assert!(!vendor.execute(&run), "the run takes one of two lines");
        assert_eq!(
            std::fs::read_dir(run.home().join("out/program"))
                .unwrap()
                .count(),
            0
        );
        std::fs::write(run.home().join("out/version"), "0.171.0\n").unwrap();
        let before = snapshot(&fixture.agents());
        let found = run.commit_at(true, "t").unwrap_err();
        assert_failed_at(&found, "checksum");
        assert!(found.why.contains("more than once"), "{}", found.why);
        assert_eq!(snapshot(&fixture.agents()), before);
    }

    /// `sha256sum -b` writes `*` before the name: run and daemon read the line
    /// alike, and the install lands.
    ///
    /// Mutation proof: without `sub(/^\*/, ...)` in the script this turns red.
    #[test]
    fn a_codex_list_in_binary_mode_is_read_by_run_and_daemon_alike() {
        let fixture = Fixture::new();
        let vendor = Vendor::new(&fixture);
        vendor.publish("0.171.0", true);
        vendor.rewrite_list("0.171.0", |list| {
            list.replace("  codex-package-", " *codex-package-")
        });
        let run = prepared(&fixture, Some("0.171.0"));
        assert!(vendor.execute(&run));
        assert_eq!(run.commit_at(true, "t").unwrap().version, "0.171.0");
    }

    /// A current version with a character outside a version name (a space, a
    /// slash that would climb in the URL) ends the run before any download.
    ///
    /// Mutation proof: without the branch for such characters both turn red.
    #[test]
    fn a_codex_current_version_that_is_no_version_name_ends_the_run() {
        // The second climbs back to the same release: without the check of
        // the characters the run would download and unpack it.
        for tag in ["rust-v0.171.0 x", "rust-v0.171.0/../0.171.0"] {
            let fixture = Fixture::new();
            let vendor = Vendor::new(&fixture);
            vendor.publish("0.171.0", true);
            let latest = vendor.dir.join("channels/latest");
            let text = std::fs::read_to_string(&latest)
                .unwrap()
                .replace("rust-v0.171.0", tag);
            std::fs::write(&latest, text).unwrap();
            let run = prepared(&fixture, None);
            assert!(!vendor.execute(&run), "{tag}");
            assert!(
                !vendor.requests().contains("releases/"),
                "{tag}: {}",
                vendor.requests()
            );
            assert_failed_at(&run.commit_at(false, "t").unwrap_err(), "run");
        }
    }
}
