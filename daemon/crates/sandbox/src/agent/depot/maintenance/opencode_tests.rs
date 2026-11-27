//! Tests of the real `OpenCode` recipe against a stand-in GitHub (#110).
//!
//! The script runs on the host under `/bin/sh` with a stand-in `curl` that
//! serves a vendor directory of the test, and the host's `tar`, `awk`,
//! `grep` and `sha256sum`. Nothing reaches a network, GitHub or a real
//! `OpenCode`; the "program" is a shell script of the test. Maintenance runs
//! exist only on Linux (Seatbelt refuses them), so do these tests.

#![cfg(target_os = "linux")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use super::tests::{Fixture, assert_failed_at, hex_sum, snapshot};
use super::*;

/// The platform of [`Fixture::prepare`], `LinuxX64`, as the recipe names it.
const ASSET: &str = "opencode-linux-x64-baseline.tar.gz";

/// The stand-in `curl`: serves `$VENDOR/api/` for the vendor's API on
/// `api.github.com`, `$VENDOR/download/` for its release downloads on
/// `github.com`, writes every URL it is asked for to `$VENDOR/requests`, and
/// fails like `curl -f` for anything else.
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
    'https://api.github.com/repos/anomalyco/opencode/releases/latest') file="$VENDOR/api/latest" ;;
    https://api.github.com/repos/anomalyco/opencode/releases/tags/*) file="$VENDOR/api/tags/${url##*/}" ;;
    https://github.com/anomalyco/opencode/releases/download/*) file="$VENDOR/download/${url#https://github.com/anomalyco/opencode/releases/download/}" ;;
    *) echo "curl: (6) not served: $url" >&2; exit 6 ;;
esac
[ -f "$file" ] || { echo "curl: (22) 404 $url" >&2; exit 22; }
if [ -n "$out" ]; then cat "$file" > "$out"; else cat "$file"; fi
"#;

fn write(path: &Path, bytes: &[u8], mode: u32) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
}

struct GitHub {
    dir: PathBuf,
    tools: PathBuf,
    /// How many releases were published; only for the release list.
    listed: std::cell::RefCell<Vec<String>>,
}

impl GitHub {
    fn new(fixture: &Fixture) -> Self {
        let dir = fixture.base.path().join("github");
        std::fs::create_dir_all(dir.join("api/tags")).unwrap();
        let tools = fixture.base.path().join("opencode-tools");
        std::fs::create_dir_all(&tools).unwrap();
        for tool in [
            "cat",
            "tr",
            "sed",
            "head",
            "mkdir",
            "awk",
            "grep",
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
        Self {
            dir,
            tools,
            listed: std::cell::RefCell::new(Vec::new()),
        }
    }

    /// Publishes `tag` like GitHub: the archive with `opencode` at its top,
    /// the release metadata (pretty-printed, with the archive among other
    /// assets and an uploader object between name and digest, as the API
    /// answers), and the tag at the head of the release list.
    fn publish(&self, tag: &str, immutable: bool) {
        self.release(tag, immutable, false);
    }

    /// Publishes `tag` as a prerelease: its metadata and archive exist, but
    /// `releases/latest` keeps naming the release before it, as GitHub's
    /// "latest release" never is a draft or a prerelease.
    fn prerelease(&self, tag: &str) {
        self.release(tag, true, true);
    }

    fn release(&self, tag: &str, immutable: bool, prerelease: bool) {
        let version = tag.trim_start_matches('v');
        let build = self.dir.join(format!("build-{tag}"));
        write(
            &build.join("opencode"),
            format!("#!/bin/sh\necho opencode {version}\n").as_bytes(),
            0o755,
        );
        let download = self.dir.join("download").join(tag);
        std::fs::create_dir_all(&download).unwrap();
        let made = std::process::Command::new("tar")
            .arg("-czf")
            .arg(download.join(ASSET))
            .arg("-C")
            .arg(&build)
            .arg("opencode")
            .status()
            .unwrap();
        assert!(made.success());
        let archive = std::fs::read(download.join(ASSET)).unwrap();
        let asset = |name: &str, digest: String| {
            serde_json::json!({
                "url": format!("https://api.github.com/repos/anomalyco/opencode/releases/assets/{name}"),
                "name": name,
                "label": "",
                "uploader": {"login": "release-bot", "id": 1, "type": "Bot"},
                "content_type": "application/gzip",
                "state": "uploaded",
                "digest": digest,
            })
        };
        let metadata = serde_json::to_string_pretty(&serde_json::json!({
            "tag_name": tag,
            "name": tag,
            "draft": false,
            "prerelease": prerelease,
            "immutable": immutable,
            "assets": [
                asset("opencode-linux-arm64.tar.gz", format!("sha256:{}", hex_sum(b"arm"))),
                asset(ASSET, format!("sha256:{}", hex_sum(&archive))),
                asset("opencode-darwin-arm64.zip", format!("sha256:{}", hex_sum(b"mac"))),
            ],
            "body": "notes",
        }))
        .unwrap();
        std::fs::write(self.dir.join("api/tags").join(tag), &metadata).unwrap();
        self.listed.borrow_mut().insert(0, tag.to_owned());
        if !prerelease {
            std::fs::write(self.dir.join("api/latest"), &metadata).unwrap();
        }
    }

    fn metadata(&self, tag: &str) -> PathBuf {
        self.dir.join("api/tags").join(tag)
    }

    fn requests(&self) -> String {
        std::fs::read_to_string(self.dir.join("requests")).unwrap_or_default()
    }

    /// Runs the script of `run` as the sandbox would, with this stand-in.
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

fn opencode() -> &'static KnownAgent {
    crate::agent::depot::recipe::known("opencode").unwrap()
}

fn prepared(
    fixture: &Fixture,
    task: MaintenanceTask,
    version: Option<&str>,
) -> Box<MaintenanceRun> {
    match fixture.prepare(opencode(), task, version).unwrap() {
        Maintenance::Run(run) => run,
        other => panic!("expected a run, got {other:?}"),
    }
}

/// Acceptance 2 with a stand-in GitHub: `agents install opencode --version`
/// installs the pinned release into a test depot, without reading the
/// release list; `agents update opencode` then takes the newest `OpenCode` 1
/// release of the list, never an `OpenCode` 2. The run reaches only the three
/// hosts of the recipe, and nothing of the host's home is touched (the
/// fixture's `HOME` and data directory are temporary).
///
/// Mutation proof: the `OpenCode` recipe back to `None` makes `prepare` answer
/// `AGENT_015`, and the first `unwrap` turns red.
#[test]
fn the_opencode_recipe_installs_a_pinned_release_from_github() {
    let fixture = Fixture::new();
    let github = GitHub::new(&fixture);
    github.publish("v1.19.5", true);
    github.publish("v1.19.6", true);
    github.prerelease("v2.0.0-beta.1");

    let run = prepared(&fixture, MaintenanceTask::Install, Some("1.19.5"));
    assert_eq!(
        run.hosts(),
        [
            "api.github.com",
            "github.com",
            "release-assets.githubusercontent.com"
        ]
    );
    let ok = github.execute(&run);
    let committed = run.commit_at(ok, "t").unwrap();
    assert_eq!(committed.version, "1.19.5");
    assert!(
        !github.requests().contains("releases/latest"),
        "{}",
        github.requests()
    );
    let depot = fixture.depot(opencode());
    let installed = depot.installed().unwrap();
    assert_eq!(
        installed.manifest.files.keys().collect::<Vec<_>>(),
        ["opencode"]
    );
    assert_eq!(installed.manifest.signature, "unsigned");
    assert_eq!(
        installed.manifest.source,
        "https://github.com/anomalyco/opencode/releases"
    );
    assert_eq!(
        std::fs::read(installed.dir.join("opencode")).unwrap(),
        b"#!/bin/sh\necho opencode 1.19.5\n"
    );

    let run = prepared(&fixture, MaintenanceTask::Update, None);
    let ok = github.execute(&run);
    let committed = run.commit_at(ok, "t").unwrap();
    assert_eq!(
        committed.version, "1.19.6",
        "the latest release, never a prerelease"
    );
    assert_eq!(committed.previous.as_deref(), Some("1.19.5"));
    assert!(github.requests().contains("releases/latest"));
    assert!(
        !github.requests().contains("v2.0.0"),
        "{}",
        github.requests()
    );
}

/// Once the latest release is `OpenCode` 2, a run without a version is
/// refused before it downloads anything; a pinned `OpenCode` 1 still
/// installs.
///
/// Mutation proof: without the `case` that refuses a version from 2 on, the
/// run downloads `v2.0.0`, and the first assertion turns red.
#[test]
fn an_opencode_2_as_latest_release_is_refused_without_a_version() {
    let fixture = Fixture::new();
    let github = GitHub::new(&fixture);
    github.publish("v1.19.6", true);
    github.publish("v2.0.0", true);
    let run = prepared(&fixture, MaintenanceTask::Install, None);
    assert!(!github.execute(&run), "{}", github.requests());
    assert!(
        !github.requests().contains("download"),
        "{}",
        github.requests()
    );

    let fixture = Fixture::new();
    let github = GitHub::new(&fixture);
    github.publish("v1.19.6", true);
    github.publish("v2.0.0", true);
    let run = prepared(&fixture, MaintenanceTask::Install, Some("1.19.6"));
    let ok = github.execute(&run);
    assert_eq!(run.commit_at(ok, "t").unwrap().version, "1.19.6");
}

/// A release that is not immutable never reaches `tar`, and the daemon's
/// own check refuses it even when the run is said to have succeeded.
///
/// Mutation proof: without the check of the flag in
/// `checksums::check_release_digest` the daemon takes the release over, and
/// the `unwrap_err` turns red.
#[test]
fn a_release_that_is_not_immutable_is_refused_by_run_and_daemon() {
    let fixture = Fixture::new();
    let github = GitHub::new(&fixture);
    github.publish("v1.19.6", false);
    let run = prepared(&fixture, MaintenanceTask::Install, Some("1.19.6"));
    assert!(!github.execute(&run), "the run checks the flag before tar");
    assert_eq!(
        std::fs::read_dir(run.home().join("out/program"))
            .unwrap()
            .count(),
        0
    );
    // A run that ignored the flag: archive and tree in place, as the
    // vendor's archive would leave them. The daemon reads no verdict of the
    // run and refuses the release itself.
    let archive = github.dir.join("download/v1.19.6").join(ASSET);
    std::fs::copy(&archive, run.home().join("out/checked/archive")).unwrap();
    let unpacked = std::process::Command::new("tar")
        .arg("-xzf")
        .arg(&archive)
        .arg("-C")
        .arg(run.home().join("out/program"))
        .status()
        .unwrap();
    assert!(unpacked.success());
    std::fs::write(run.home().join("out/version"), "1.19.6\n").unwrap();
    let before = snapshot(&fixture.agents());
    let found = run.commit_at(true, "t").unwrap_err();
    assert_failed_at(&found, "checksum");
    assert!(found.why.contains("not immutable"), "{}", found.why);
    assert_eq!(snapshot(&fixture.agents()), before);
}

/// An archive other than the one GitHub's digest names never reaches `tar`;
/// taken over anyway, the daemon refuses it.
#[test]
fn an_opencode_archive_that_does_not_match_the_digest_is_never_unpacked() {
    let fixture = Fixture::new();
    let github = GitHub::new(&fixture);
    github.publish("v1.19.6", true);
    let archive = github.dir.join("download/v1.19.6").join(ASSET);
    let mut bytes = std::fs::read(&archive).unwrap();
    bytes.push(0);
    std::fs::write(&archive, bytes).unwrap();

    let run = prepared(&fixture, MaintenanceTask::Install, None);
    assert!(!github.execute(&run), "the run compares before tar");
    assert_eq!(
        std::fs::read_dir(run.home().join("out/program"))
            .unwrap()
            .count(),
        0
    );
    std::fs::write(run.home().join("out/version"), "1.19.6\n").unwrap();
    let before = snapshot(&fixture.agents());
    assert_failed_at(&run.commit_at(true, "t").unwrap_err(), "checksum");
    assert_eq!(snapshot(&fixture.agents()), before);
}

/// Metadata of another release than the run installed is refused at the pin:
/// an authentic older archive cannot be labelled as a newer version.
#[test]
fn opencode_metadata_of_another_release_is_refused() {
    let fixture = Fixture::new();
    let github = GitHub::new(&fixture);
    github.publish("v1.19.6", true);
    let run = prepared(&fixture, MaintenanceTask::Install, Some("1.19.6"));
    assert!(github.execute(&run));
    let metadata = run.home().join("out/checked/metadata");
    let text = std::fs::read_to_string(&metadata)
        .unwrap()
        .replace("\"v1.19.6\"", "\"v1.19.5\"");
    std::fs::write(&metadata, text).unwrap();
    assert_failed_at(&run.commit_at(true, "t").unwrap_err(), "pin");
}

/// `OpenCode` 2 is another program (the vendor's installer refuses it), and a
/// version with a character outside a version name could climb in a URL:
/// both end the run before any download.
///
/// Mutation proof: the pattern `[0-9]*.[0-9]*.[0-9]*` instead of
/// `1.[0-9]*.[0-9]*` turns the first case red.
#[test]
fn an_opencode_version_that_is_no_opencode_1_ends_the_run() {
    for version in ["2.0.0", "1.19.6/../1.19.5", "1.19 6"] {
        let fixture = Fixture::new();
        let github = GitHub::new(&fixture);
        github.publish("v1.19.6", true);
        // The version check of the daemon comes later; the run is prepared
        // with the asked version as the person would pass it.
        let run = match fixture.prepare(opencode(), MaintenanceTask::Install, Some(version)) {
            Ok(Maintenance::Run(run)) => run,
            // The daemon may refuse the name itself before any run.
            Err(_) => continue,
            Ok(other) => panic!("expected a run, got {other:?}"),
        };
        assert!(!github.execute(&run), "{version}");
        assert_eq!(github.requests(), "", "{version}: {}", github.requests());
    }
    // Without the flag the metadata is read, never the archive behind it.
    let fixture = Fixture::new();
    let github = GitHub::new(&fixture);
    github.publish("v1.19.6", true);
    let path = github.metadata("v1.19.6");
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    value.as_object_mut().unwrap().remove("immutable");
    std::fs::write(&path, value.to_string()).unwrap();
    let run = prepared(&fixture, MaintenanceTask::Install, None);
    assert!(!github.execute(&run));
    assert!(
        !github.requests().contains("/download/"),
        "{}",
        github.requests()
    );
}
