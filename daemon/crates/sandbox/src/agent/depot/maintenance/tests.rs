//! Tests of install and update (HUM-519).
//!
//! A stand-in adapter installs from a fixture tree instead of the network:
//! its script copies what a download would have left. The script runs under
//! `/bin/sh` with a `PATH` of a few tools. The daemon's check of the
//! signature runs a stand-in `gpg` that prints fixed status lines, or the
//! host's real `gpg` with a key a test makes. Nothing here reaches a network
//! or a real vendor.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use humanitl_config::{Env, Paths};
use humanitl_core::diagnostics::codes::{AGENT_008, AGENT_011, AGENT_018};
use humanitl_core::ids::SessionId;
use humanitl_core::{Diagnostic, HostName, Method, Scheme};
use sha2::{Digest as _, Sha256};

use super::*;
use crate::agent::depot::InstallHost;
use crate::agent::depot::recipe::{DepotRecipe, LoginFlow, SignedManifest};
use crate::agent::depot::store::CURRENT_LINK;
use crate::agent::depot::store::tests::uid;

const FPR: &str = "0123456789ABCDEF0123456789ABCDEF01234567";

const STAND_IN_SCRIPT: &str = r#"cp -R "$FIXTURE/program" "$out/"
if [ -d "$FIXTURE/signed" ]; then cp -R "$FIXTURE/signed" "$out/"; fi
cp "$FIXTURE/version" "$out/version"
"#;

const PLATFORMS: &[(HostPlatform, &str)] = &[(HostPlatform::LinuxX64, "linux-x64")];

static SIGNED: KnownAgent = KnownAgent {
    id: "standin",
    display_name: "Stand-in",
    program: "standin",
    host_installs: &["/usr/bin/standin"],
    recipe: Some(DepotRecipe {
        program: "standin",
        config_env: "STANDIN_CONFIG_DIR",
        update_off: &[],
        credential_file: None,
        install: Some(InstallRecipe {
            script: STAND_IN_SCRIPT,
            hosts: &[InstallHost {
                host: "downloads.standin.test",
                evidence: "https://standin.test/docs",
            }],
            program: "standin",
            platforms: PLATFORMS,
            signature: Some(SignedManifest {
                fingerprint: FPR,
                checksum_pointer: "/platforms/{platform}/checksum",
                version_pointer: "/version",
                evidence: "https://standin.test/docs",
            }),
            source: "https://downloads.standin.test/releases",
        }),
        login: LoginFlow::ApiKeyOnly,
    }),
};

static UNSIGNED: KnownAgent = KnownAgent {
    id: "plain",
    display_name: "Plain",
    program: "standin",
    host_installs: &["/usr/bin/standin"],
    recipe: Some(DepotRecipe {
        program: "standin",
        config_env: "STANDIN_CONFIG_DIR",
        update_off: &[],
        credential_file: None,
        install: Some(InstallRecipe {
            script: STAND_IN_SCRIPT,
            hosts: &[InstallHost {
                host: "downloads.standin.test",
                evidence: "https://standin.test/docs",
            }],
            program: "standin",
            platforms: PLATFORMS,
            signature: None,
            source: "https://downloads.standin.test/releases",
        }),
        login: LoginFlow::ApiKeyOnly,
    }),
};

/// What the stand-in `gpg` of the host answers to `--verify`; an import
/// always names the key `import`.
#[derive(Clone, Copy)]
struct FakeGpg {
    import: &'static str,
    signer: &'static str,
    exit: i32,
    bad: bool,
}

const GOOD: FakeGpg = FakeGpg {
    import: FPR,
    signer: FPR,
    exit: 0,
    bad: false,
};

struct Fixture {
    base: tempfile::TempDir,
    paths: Paths,
    session: SessionId,
}

impl Fixture {
    fn new() -> Self {
        let base = tempfile::tempdir().unwrap();
        let paths = Paths::new(Env::from_pairs([
            ("HOME", base.path().join("home").to_str().unwrap()),
            ("XDG_DATA_HOME", base.path().join("data").to_str().unwrap()),
            ("XDG_RUNTIME_DIR", base.path().join("run").to_str().unwrap()),
        ]));
        Self {
            base,
            paths,
            session: SessionId::new(),
        }
    }

    fn agents(&self) -> PathBuf {
        self.paths.data_dir().join("agents")
    }

    fn depot(&self, agent: &KnownAgent) -> Depot {
        Depot::at(self.agents().join(agent.id), agent.id, uid())
    }

    fn prepare(
        &self,
        agent: &'static KnownAgent,
        task: MaintenanceTask,
        version: Option<&str>,
    ) -> Result<Maintenance, Diagnostic> {
        prepare_for(
            &MaintenanceRequest {
                paths: &self.paths,
                agent: agent.id,
                task,
                version,
                session: self.session,
            },
            agent,
            self.depot(agent),
            Some(HostPlatform::LinuxX64),
        )
    }

    /// A prepared run whose host `gpg` is the stand-in with a good answer.
    fn run(
        &self,
        agent: &'static KnownAgent,
        task: MaintenanceTask,
        version: Option<&str>,
    ) -> MaintenanceRun {
        match self.prepare(agent, task, version).unwrap() {
            Maintenance::Run(run) => run.with_gpg(Some(self.fake_gpg(GOOD))),
            other => panic!("expected a run, got {other:?}"),
        }
    }

    /// A download of `version` as the vendor would serve it; `sum` overrides
    /// the checksum in the vendor's manifest.
    fn release(&self, version: &str, sum: Option<&str>) -> PathBuf {
        let dir = self.base.path().join(format!("release-{version}"));
        std::fs::create_dir_all(dir.join("program/lib")).unwrap();
        std::fs::create_dir_all(dir.join("signed")).unwrap();
        let program = format!("#!/bin/sh\necho standin {version}\n");
        std::fs::write(dir.join("program/standin"), &program).unwrap();
        std::fs::set_permissions(
            dir.join("program/standin"),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
        std::fs::write(dir.join("program/lib/data.txt"), b"data").unwrap();
        std::os::unix::fs::symlink("../standin", dir.join("program/lib/alias")).unwrap();
        let real = hex_sum(program.as_bytes());
        let manifest = format!(
            r#"{{"version":"{version}","platforms":{{"linux-x64":{{"checksum":"{}"}}}}}}"#,
            sum.unwrap_or(&real)
        );
        std::fs::write(dir.join("signed/manifest"), manifest).unwrap();
        std::fs::write(dir.join("signed/signature"), b"sig").unwrap();
        std::fs::write(dir.join("signed/key.asc"), b"key").unwrap();
        std::fs::write(dir.join("version"), format!("{version}\n")).unwrap();
        dir
    }

    /// A `PATH` with the tools of the stand-in script.
    fn tools(&self) -> PathBuf {
        let dir = self.base.path().join("tools");
        if dir.exists() {
            return dir;
        }
        std::fs::create_dir_all(&dir).unwrap();
        for tool in ["cp", "mkdir", "chmod", "cat"] {
            let found = ["/usr/bin", "/bin"]
                .iter()
                .map(|bin| Path::new(bin).join(tool))
                .find(|path| path.exists())
                .unwrap_or_else(|| panic!("{tool} not found"));
            std::os::unix::fs::symlink(found, dir.join(tool)).unwrap();
        }
        dir
    }

    /// The stand-in for the host's `gpg`: its answers are written into it,
    /// since the daemon runs `gpg` with an empty environment.
    fn fake_gpg(&self, fake: FakeGpg) -> PathBuf {
        let path = self
            .base
            .path()
            .join(format!("gpg-{}", uuid::Uuid::now_v7().simple()));
        let verdict = if fake.bad {
            "[GNUPG:] BADSIG 0123 Stand-in".to_owned()
        } else {
            format!(
                "[GNUPG:] GOODSIG 0123 Stand-in\n[GNUPG:] VALIDSIG {0} 2026-11-19 0 0 4 0 1 10 00 {0}",
                fake.signer
            )
        };
        std::fs::write(
            &path,
            format!(
                "#!/bin/sh\nfor a in \"$@\"; do\n  if [ \"$a\" = --import ]; then\n    \
                 printf '%s\\n' '[GNUPG:] IMPORT_OK 1 {}'\n    exit 0\n  fi\ndone\n\
                 printf '%s\\n' '{}'\nexit {}\n",
                fake.import,
                verdict.replace('\n', "' '"),
                fake.exit
            ),
        )
        .unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    /// Runs the script of `run` on the host as the sandbox would, with the
    /// fixture as the vendor. Returns whether it ended with status 0.
    fn execute(&self, run: &MaintenanceRun, release: &Path) -> bool {
        std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg(run.script())
            .env_clear()
            .envs(run.env_for(run.home()))
            .env("PATH", self.tools())
            .env("FIXTURE", release)
            .status()
            .unwrap()
            .success()
    }

    /// Install `version` of the signed stand-in with a good signature.
    fn install(&self, task: MaintenanceTask, version: &str) -> Committed {
        self.install_of(&SIGNED, task, version)
    }

    /// Install `version` of `agent`.
    fn install_of(
        &self,
        agent: &'static KnownAgent,
        task: MaintenanceTask,
        version: &str,
    ) -> Committed {
        let run = self.run(agent, task, Some(version));
        let release = self.release(version, None);
        let ok = self.execute(&run, &release);
        run.commit_at(ok, "2026-11-19T00:00:00Z").unwrap()
    }
}

fn hex_sum(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    Sha256::digest(bytes)
        .iter()
        .fold(String::new(), |mut out, byte| {
            let _ = write!(out, "{byte:02x}");
            out
        })
}

/// Every entry under `root` with its kind, mode and content, links not
/// followed; the work directories of runs (`.maintenance`) left out.
fn snapshot(root: &Path) -> BTreeMap<PathBuf, String> {
    let mut out = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let rel = path.strip_prefix(root).unwrap().to_path_buf();
            if rel.starts_with(".maintenance") {
                continue;
            }
            let meta = std::fs::symlink_metadata(&path).unwrap();
            let mode = meta.permissions().mode() & 0o7777;
            if meta.file_type().is_symlink() {
                out.insert(
                    rel,
                    format!("link {}", std::fs::read_link(&path).unwrap().display()),
                );
            } else if meta.is_dir() {
                out.insert(rel, format!("dir {mode:o}"));
                stack.push(path);
            } else {
                out.insert(
                    rel,
                    format!("{mode:o} {}", hex_sum(&std::fs::read(&path).unwrap())),
                );
            }
        }
    }
    out
}

fn assert_failed_at(found: &Diagnostic, step: &str) {
    assert_eq!(found.code, AGENT_011, "{}", found.why);
    assert!(
        found.why.contains(&format!("at the step {step}:")),
        "{}",
        found.why
    );
}

#[test]
fn an_install_lands_in_its_version_directory_with_its_manifest_and_current() {
    let fixture = Fixture::new();
    let committed = fixture.install(MaintenanceTask::Install, "1.0.0");
    let depot = fixture.depot(&SIGNED);
    let installed = depot.installed().unwrap();
    assert_eq!(installed.version, "1.0.0");
    assert_eq!(installed.dir, depot.program_dir().join("1.0.0"));
    assert_eq!(
        std::fs::read_link(depot.root().join(CURRENT_LINK)).unwrap(),
        Path::new("program/1.0.0")
    );
    let manifest = installed.manifest;
    assert_eq!(manifest, committed.manifest);
    assert_eq!(manifest.signature, "verified");
    assert_eq!(manifest.signed_by, FPR);
    assert_eq!(manifest.installed_at, "2026-11-19T00:00:00Z");
    assert_eq!(manifest.source, "https://downloads.standin.test/releases");
    // Every file with its SHA-256, computed from what lies in the depot;
    // links are kept as links and carry no sum.
    let program = std::fs::read(installed.dir.join("standin")).unwrap();
    assert_eq!(
        manifest.files.keys().collect::<Vec<_>>(),
        ["lib/data.txt", "standin"]
    );
    assert_eq!(manifest.files["standin"], hex_sum(&program));
    assert_eq!(manifest.files["lib/data.txt"], hex_sum(b"data"));
    assert_eq!(
        std::fs::read_link(installed.dir.join("lib/alias")).unwrap(),
        Path::new("../standin")
    );
    // The depot's modes: the program executable for its owner alone.
    let mode = std::fs::metadata(installed.dir.join("standin"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o700);
    // The version directory passes the check before every bind.
    crate::agent::depot::tree::check_version_dir(
        "standin",
        &installed.dir,
        &installed.dir,
        &crate::agent::locate::Trust::new(uid(), None),
    )
    .unwrap();
    // No journal, no private directory of the takeover, no work directory.
    assert!(!depot.root().join(adopt::JOURNAL).exists());
    let leftovers: Vec<_> = std::fs::read_dir(fixture.agents())
        .unwrap()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with(".staging-") || name.starts_with(".verify-"))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
    assert!(
        !fixture
            .agents()
            .join(".maintenance")
            .join(format!("standin-{}", fixture.session))
            .exists()
    );
}

#[test]
fn an_unsigned_recipe_records_unsigned() {
    let fixture = Fixture::new();
    let run = fixture.run(&UNSIGNED, MaintenanceTask::Install, None);
    let release = fixture.release("2.0.0", None);
    let ok = fixture.execute(&run, &release);
    let committed = run.commit_at(ok, "t").unwrap();
    assert_eq!(committed.version, "2.0.0");
    assert_eq!(committed.manifest.signature, "unsigned");
    assert!(committed.manifest.signed_by.is_empty());
}

/// A takeover that fails leaves everything under `agents/` byte for byte as
/// it was (modes and lock files included), with an installed version beside
/// it.
fn refused_leaves_the_depot(step: &str, run_and_commit: impl FnOnce(&Fixture) -> Diagnostic) {
    refused_leaves_the_depot_of(&SIGNED, step, run_and_commit);
}

fn refused_leaves_the_depot_of(
    agent: &'static KnownAgent,
    step: &str,
    run_and_commit: impl FnOnce(&Fixture) -> Diagnostic,
) {
    let fixture = Fixture::new();
    fixture.install_of(agent, MaintenanceTask::Install, "1.0.0");
    // `run_and_commit` compares `agents/` before and after the takeover
    // itself, once its run is prepared and has created its lock files.
    let found = run_and_commit(&fixture);
    assert_failed_at(&found, step);
    assert_eq!(fixture.depot(agent).installed().unwrap().version, "1.0.0");
}

/// The update's run of `fixture`, with the snapshot of `agents/` taken once
/// it is prepared; `check` gets both and compares after the takeover.
fn update_unchanged(
    fixture: &Fixture,
    agent: &'static KnownAgent,
    gpg: Option<PathBuf>,
    plant: impl FnOnce(&MaintenanceRun, &Path),
) -> Diagnostic {
    let run = fixture
        .run(agent, MaintenanceTask::Update, Some("2.0.0"))
        .with_gpg(gpg);
    let release = fixture.release("2.0.0", None);
    assert!(fixture.execute(&run, &release));
    plant(&run, &release);
    let before = snapshot(&fixture.agents());
    let found = run.commit_at(true, "t").unwrap_err();
    assert_eq!(snapshot(&fixture.agents()), before, "{}", found.why);
    found
}

/// Mutation proof (HUM-519): a host without `gpg` judged as "not checked"
/// turns this red. The run forges every verdict it could: a result `0`,
/// status lines of a good signature by the fixed key, and a manifest whose
/// checksum fits an altered program. None of it counts.
#[test]
fn a_forged_verdict_of_the_run_passes_nothing() {
    refused_leaves_the_depot("signature", |fixture| {
        let found = update_unchanged(fixture, &SIGNED, None, |run, _| {
            let out = run.home().join(OUT_DIR);
            std::fs::create_dir_all(out.join("gpg")).unwrap();
            std::fs::write(out.join("gpg/result"), b"0\n").unwrap();
            std::fs::write(
                out.join("gpg/status"),
                format!(
                    "[GNUPG:] GOODSIG 0123 x\n[GNUPG:] VALIDSIG {FPR} 2026-11-19 0 0 4 0 1 10 00 {FPR}\n"
                ),
            )
            .unwrap();
            let altered = b"#!/bin/sh\necho altered\n";
            std::fs::write(out.join("program/standin"), altered).unwrap();
            std::fs::write(
                out.join("signed/manifest"),
                format!(
                    r#"{{"platforms":{{"linux-x64":{{"checksum":"{}"}}}}}}"#,
                    hex_sum(altered)
                ),
            )
            .unwrap();
        });
        assert!(
            found.why.contains("gpg is not installed on the host"),
            "{}",
            found.why
        );
        found
    });
}

#[test]
fn gpg_ending_with_an_error_fails() {
    refused_leaves_the_depot("signature", |fixture| {
        let gpg = fixture.fake_gpg(FakeGpg { exit: 2, ..GOOD });
        update_unchanged(fixture, &SIGNED, Some(gpg), |_, _| {})
    });
}

#[test]
fn another_fingerprint_fails() {
    refused_leaves_the_depot("signature", |fixture| {
        let other = "FEDCBA9876543210FEDCBA9876543210FEDCBA98";
        let gpg = fixture.fake_gpg(FakeGpg {
            signer: other,
            ..GOOD
        });
        update_unchanged(fixture, &SIGNED, Some(gpg), |_, _| {})
    });
}

#[test]
fn a_key_file_with_another_key_fails() {
    refused_leaves_the_depot("signature", |fixture| {
        let other = "FEDCBA9876543210FEDCBA9876543210FEDCBA98";
        let gpg = fixture.fake_gpg(FakeGpg {
            import: other,
            ..GOOD
        });
        update_unchanged(fixture, &SIGNED, Some(gpg), |_, _| {})
    });
}

#[test]
fn a_bad_signature_fails() {
    refused_leaves_the_depot("signature", |fixture| {
        let gpg = fixture.fake_gpg(FakeGpg {
            bad: true,
            exit: 1,
            ..GOOD
        });
        update_unchanged(fixture, &SIGNED, Some(gpg), |_, _| {})
    });
}

#[test]
fn a_wrong_checksum_fails() {
    refused_leaves_the_depot("checksum", |fixture| {
        let gpg = fixture.fake_gpg(GOOD);
        update_unchanged(fixture, &SIGNED, Some(gpg), |run, _| {
            std::fs::write(
                run.home().join(OUT_DIR).join("signed/manifest"),
                format!(
                    r#"{{"version":"2.0.0","platforms":{{"linux-x64":{{"checksum":"{}"}}}}}}"#,
                    "0".repeat(64)
                ),
            )
            .unwrap();
        })
    });
}

#[test]
fn a_failed_run_takes_nothing_over() {
    refused_leaves_the_depot("run", |fixture| {
        let run = fixture.run(&SIGNED, MaintenanceTask::Update, Some("2.0.0"));
        // No release at the fixture's place: the copy fails like a download.
        let ok = fixture.execute(&run, &fixture.base.path().join("nothing"));
        assert!(!ok);
        let before = snapshot(&fixture.agents());
        let found = run.commit_at(ok, "t").unwrap_err();
        assert_eq!(snapshot(&fixture.agents()), before);
        assert!(found.why.contains("sudo"), "{}", found.why);
        found
    });
}

/// A run whose result holds something the tree must not: written straight
/// into the result directory, as a compromised installer could.
fn tree_refused(plant: impl FnOnce(&Path)) -> Diagnostic {
    let mut found = None;
    refused_leaves_the_depot_of(&UNSIGNED, "tree", |fixture| {
        let refused = update_unchanged(fixture, &UNSIGNED, None, |run, _| {
            plant(&run.home().join(OUT_DIR).join("program"));
        });
        found = Some(refused.clone());
        refused
    });
    found.unwrap()
}

/// Mutation proof (HUM-519): without the check for sockets this turns red.
#[test]
fn a_socket_in_the_tree_is_refused() {
    let found = tree_refused(|program| {
        // Bound at a short path (`SUN_LEN`), then moved into the tree; the
        // same file system as the fixture, both under the temporary directory.
        let short = std::env::temp_dir().join(format!(
            "h519-{}-{}.sock",
            std::process::id(),
            uuid::Uuid::now_v7().simple()
        ));
        let _ = std::fs::remove_file(&short);
        let _socket = std::os::unix::net::UnixListener::bind(&short).unwrap();
        std::fs::rename(&short, program.join("sock")).unwrap();
    });
    assert!(
        found.why.contains("socket, FIFO or device"),
        "{}",
        found.why
    );
}

#[test]
fn a_fifo_in_the_tree_is_refused() {
    let found = tree_refused(|program| {
        let made = std::process::Command::new("mkfifo")
            .arg(program.join("fifo"))
            .status()
            .unwrap();
        assert!(made.success());
    });
    assert!(
        found.why.contains("socket, FIFO or device"),
        "{}",
        found.why
    );
}

#[test]
fn a_set_user_id_file_is_refused() {
    let found = tree_refused(|program| {
        std::fs::set_permissions(
            program.join("lib/data.txt"),
            std::fs::Permissions::from_mode(0o4755),
        )
        .unwrap();
    });
    assert!(found.why.contains("set-user-ID"), "{}", found.why);
}

#[test]
fn a_link_to_etc_is_refused() {
    let found = tree_refused(|program| {
        std::os::unix::fs::symlink("/etc", program.join("etc")).unwrap();
    });
    assert!(
        found.why.contains("out of the program tree"),
        "{}",
        found.why
    );
    let found = tree_refused(|program| {
        std::os::unix::fs::symlink("../../..", program.join("lib/up")).unwrap();
    });
    assert!(
        found.why.contains("out of the program tree"),
        "{}",
        found.why
    );
}

/// Chains of links that each look inside and together lead out (HUM-519,
/// review). Mutation proof: without the rule on `..` after a name in
/// `link_stays_inside` both turn red.
#[test]
fn a_chain_of_links_out_of_the_tree_is_refused() {
    let found = tree_refused(|program| {
        std::os::unix::fs::symlink("..", program.join("lib/up")).unwrap();
        std::os::unix::fs::symlink("lib/up/..", program.join("escape")).unwrap();
    });
    assert!(found.why.contains("escape"), "{}", found.why);
    let found = tree_refused(|program| {
        std::fs::create_dir(program.join("d")).unwrap();
        std::os::unix::fs::symlink("..", program.join("d/a")).unwrap();
        std::os::unix::fs::symlink("d/a/../..", program.join("l")).unwrap();
    });
    assert!(found.why.contains("program/l"), "{}", found.why);
}

#[test]
fn a_hard_link_is_refused() {
    let found = tree_refused(|program| {
        std::fs::hard_link(program.join("standin"), program.join("lib/second")).unwrap();
    });
    assert!(found.why.contains("hard links"), "{}", found.why);
}

#[test]
fn a_manifest_of_its_own_is_refused() {
    let found = tree_refused(|program| {
        std::fs::write(program.join("manifest.json"), b"{}").unwrap();
    });
    assert!(
        found.why.contains("only the daemon writes"),
        "{}",
        found.why
    );
}

/// Two names that are not UTF-8 would share one key of the manifest.
#[cfg(target_os = "linux")]
#[test]
fn a_name_that_is_not_utf8_is_refused() {
    use std::os::unix::ffi::OsStrExt as _;
    let found = tree_refused(|program| {
        let name = std::ffi::OsStr::from_bytes(b"bad\xff");
        std::fs::write(program.join(name), b"x").unwrap();
    });
    assert!(found.why.contains("not UTF-8"), "{}", found.why);
}

#[test]
fn a_tree_deeper_than_the_cap_is_refused() {
    let found = tree_refused(|program| {
        let mut dir = program.to_path_buf();
        for _ in 0..=adopt::MAX_DEPTH {
            dir.push("d");
        }
        std::fs::create_dir_all(&dir).unwrap();
    });
    assert!(found.why.contains("deeper than"), "{}", found.why);
}

/// The result directory itself a link to a host directory: never followed.
#[test]
fn a_result_that_is_a_link_is_never_followed() {
    let fixture = Fixture::new();
    let run = fixture.run(&UNSIGNED, MaintenanceTask::Install, None);
    let host = fixture.base.path().join("host-data");
    std::fs::create_dir_all(host.join("program")).unwrap();
    std::fs::write(host.join("version"), b"9.9.9\n").unwrap();
    std::fs::write(host.join("program/standin"), b"host").unwrap();
    let out = run.home().join(OUT_DIR);
    std::fs::remove_dir(&out).unwrap();
    std::os::unix::fs::symlink(&host, &out).unwrap();
    let found = run.commit_at(true, "t").unwrap_err();
    assert_failed_at(&found, "result");
    assert!(!fixture.depot(&UNSIGNED).exists());
}

/// A first install that is refused leaves no depot behind: the depot's
/// directories come only right before the version is placed. Mutation
/// proof (HUM-519, review): the layout created before the checks turns
/// this red.
#[test]
fn a_refused_first_install_leaves_no_depot() {
    let fixture = Fixture::new();
    let run = fixture.run(&UNSIGNED, MaintenanceTask::Install, Some("1.0.0"));
    let release = fixture.release("1.0.0", None);
    assert!(fixture.execute(&run, &release));
    std::fs::hard_link(
        run.home().join(OUT_DIR).join("program/standin"),
        run.home().join(OUT_DIR).join("program/second"),
    )
    .unwrap();
    let before = snapshot(&fixture.agents());
    assert_failed_at(&run.commit_at(true, "t").unwrap_err(), "tree");
    assert_eq!(snapshot(&fixture.agents()), before);
    assert!(!fixture.depot(&UNSIGNED).exists());
}

/// A takeover that died between placing the version and switching
/// `current` is undone before the next one: the placed version goes, the
/// current one stays. Mutation proof (HUM-519, review): without the
/// recovery the placed version stays and this turns red. The version goes
/// only under the depot's exclusive lock: while a session holds the depot
/// it stays for a later pruning. Mutation proof (HUM-519, review delta 2):
/// a removal without that lock turns this red too.
#[test]
fn a_crash_between_place_and_switch_is_undone() {
    let fixture = Fixture::new();
    fixture.install(MaintenanceTask::Install, "1.0.0");
    let depot = fixture.depot(&SIGNED);
    // What a crash after the `rename` into `program/` leaves.
    let crash = || {
        let copy = std::process::Command::new("cp")
            .arg("-R")
            .arg(depot.program_dir().join("1.0.0"))
            .arg(depot.program_dir().join("2.0.0"))
            .status()
            .unwrap();
        assert!(copy.success());
        std::fs::write(depot.root().join(adopt::JOURNAL), b"2.0.0").unwrap();
    };
    crash();
    let session = depot.reserve_for_session().unwrap();
    adopt::recover(&depot);
    assert!(
        depot.program_dir().join("2.0.0").exists(),
        "a session holds the depot"
    );
    assert!(!depot.root().join(adopt::JOURNAL).exists());
    drop(session);
    std::fs::remove_dir_all(depot.program_dir().join("2.0.0")).unwrap();

    crash();
    let committed = fixture.install(MaintenanceTask::Update, "3.0.0");
    assert_eq!(committed.previous.as_deref(), Some("1.0.0"));
    assert!(!depot.program_dir().join("2.0.0").exists());
    assert!(!depot.root().join(adopt::JOURNAL).exists());
    // A journal whose version became current only goes.
    std::fs::write(depot.root().join(adopt::JOURNAL), b"3.0.0").unwrap();
    adopt::recover(&depot);
    assert!(depot.program_dir().join("3.0.0").exists());
    assert!(!depot.root().join(adopt::JOURNAL).exists());
}

/// A switch whose sync fails while a session holds the depot: `current`
/// goes back, and the placed version stays, since the session may have
/// started on it. Mutation proof (HUM-519, review delta 2): a removal
/// without the exclusive lock turns this red.
#[test]
fn a_set_back_keeps_a_version_a_session_may_use() {
    let fixture = Fixture::new();
    fixture.install(MaintenanceTask::Install, "1.0.0");
    let depot = fixture.depot(&SIGNED);
    let session = depot.reserve_for_session().unwrap();
    let found = update_with_failing_root_syncs(&fixture, &[1]);
    assert_failed_at(&found, "commit");
    assert_eq!(depot.current_version().as_deref(), Some("1.0.0"));
    assert!(depot.program_dir().join("2.0.0").exists());
    drop(session);
    // The next takeover's pruning removes it.
    let committed = fixture.install(MaintenanceTask::Update, "3.0.0");
    assert!(
        committed.pruned.contains(&"2.0.0".to_owned()),
        "{committed:?}"
    );
}

/// The first use creates `agents/` and syncs the data directory above it,
/// so a crash cannot lose the entry of a depot made right after. Mutation
/// proof (HUM-519, review delta 2): without the sync in `create_synced`
/// this turns red.
#[test]
fn the_first_agents_directory_is_synced_into_its_parent() {
    let fixture = Fixture::new();
    let _ = adopt::sync_probe::take();
    let run = fixture.run(&SIGNED, MaintenanceTask::Install, None);
    let synced = adopt::sync_probe::take();
    let data = std::fs::canonicalize(fixture.paths.data_dir()).unwrap();
    assert!(
        synced
            .iter()
            .any(|path| std::fs::canonicalize(path).ok().as_ref() == Some(&data)),
        "{} not synced: {synced:?}",
        data.display()
    );
    drop(run);
}

/// A prepared update holds the depot: `agents remove` waits for it.
/// Mutation proof (HUM-519, review): without the run's reservation the
/// removal succeeds and this turns red.
#[test]
fn a_prepared_run_keeps_the_depot_from_being_removed() {
    let fixture = Fixture::new();
    fixture.install(MaintenanceTask::Install, "1.0.0");
    let run = fixture.run(&SIGNED, MaintenanceTask::Update, Some("2.0.0"));
    let found = fixture.depot(&SIGNED).remove().unwrap_err();
    assert_eq!(found.code, AGENT_018);
    assert!(fixture.depot(&SIGNED).exists());
    drop(run);
    fixture.depot(&SIGNED).remove().unwrap();
}

/// A switch back waits for a run of the same agent. Mutation proof
/// (HUM-519, review): without the maintenance lock in `switch_in` this
/// turns red.
#[test]
fn a_switch_waits_for_a_run() {
    let fixture = Fixture::new();
    fixture.install(MaintenanceTask::Install, "1.0.0");
    fixture.install(MaintenanceTask::Update, "2.0.0");
    let depot = fixture.depot(&SIGNED);
    let run = fixture.run(&SIGNED, MaintenanceTask::Update, Some("3.0.0"));
    assert_failed_at(&switch_in(&depot, "1.0.0").unwrap_err(), "run");
    assert_eq!(depot.installed().unwrap().version, "2.0.0");
    drop(run);
    switch_in(&depot, "1.0.0").unwrap();
}

/// Pruning keeps what `current` names on disk at that moment, whatever the
/// caller believed. Mutation proof (HUM-519, review): without the re-read of
/// `current` under the lock this turns red.
#[test]
fn pruning_keeps_the_version_current_names_now() {
    let fixture = Fixture::new();
    fixture.install(MaintenanceTask::Install, "1.0.0");
    fixture.install(MaintenanceTask::Update, "2.0.0");
    let depot = fixture.depot(&SIGNED);
    switch_in(&depot, "1.0.0").unwrap();
    let pruned = prune_unused(&depot, &["2.0.0"], None);
    assert!(pruned.is_empty(), "{pruned:?}");
    assert_eq!(depot.installed().unwrap().version, "1.0.0");
}

/// The host's real `gpg` on a key and a signature the test makes: a good
/// manifest passes, an altered one with the same signature does not. Runs
/// only where `gpg` exists and can make a key (an agent may be refused in a
/// confined test environment); it says so when it does not run.
#[test]
fn the_hosts_gpg_verifies_the_daemons_copy() {
    let Some(gpg) = crate::agent::depot::verify::host_gpg() else {
        eprintln!("skipped: no gpg on this host");
        return;
    };
    let fixture = Fixture::new();
    let Some(signer) = Signer::new(&gpg, fixture.base.path()) else {
        eprintln!("skipped: gpg cannot make a key here");
        return;
    };
    let fpr: &'static str = Box::leak(signer.fingerprint.clone().into_boxed_str());
    let agent: &'static KnownAgent = Box::leak(Box::new(signed_by(fpr)));

    let release = fixture.release("1.0.0", None);
    signer.sign(&release);
    let run = fixture
        .run(agent, MaintenanceTask::Install, Some("1.0.0"))
        .with_gpg(Some(gpg.clone()));
    assert!(fixture.execute(&run, &release));
    let committed = match run.commit_at(true, "t") {
        Ok(committed) => committed,
        // Linux CI has a `gpg` that works in a fresh home: there the test
        // stays hard. Elsewhere a `gpg` that cannot import into a fresh home
        // is a property of that host; the daemon then fails closed with
        // `AGENT_011` at the step signature, which is what the found says.
        Err(found) if !cfg!(target_os = "linux") && found.why.contains("could not import") => {
            assert_failed_at(&found, "signature");
            eprintln!(
                "skipped: this host's gpg cannot import in a fresh home: {}",
                found.why
            );
            return;
        }
        Err(found) => panic!("{}", found.why),
    };
    assert_eq!(committed.manifest.signature, "verified");
    assert_eq!(committed.manifest.signed_by, fpr);

    // The run alters program and manifest and keeps the signature.
    let release = fixture.release("2.0.0", None);
    signer.sign(&release);
    let run = fixture
        .run(agent, MaintenanceTask::Update, Some("2.0.0"))
        .with_gpg(Some(gpg));
    assert!(fixture.execute(&run, &release));
    let out = run.home().join(OUT_DIR);
    let altered = b"#!/bin/sh\necho altered\n";
    std::fs::write(out.join("program/standin"), altered).unwrap();
    std::fs::write(
        out.join("signed/manifest"),
        format!(
            r#"{{"version":"2.0.0","platforms":{{"linux-x64":{{"checksum":"{}"}}}}}}"#,
            hex_sum(altered)
        ),
    )
    .unwrap();
    assert_failed_at(&run.commit_at(true, "t").unwrap_err(), "signature");
    assert_eq!(fixture.depot(agent).installed().unwrap().version, "1.0.0");
}

/// The signed stand-in with another fixed key.
fn signed_by(fingerprint: &'static str) -> KnownAgent {
    let mut agent = SIGNED;
    agent.id = "signed";
    if let Some(recipe) = agent.recipe.as_mut()
        && let Some(install) = recipe.install.as_mut()
        && let Some(signature) = install.signature.as_mut()
    {
        signature.fingerprint = fingerprint;
    }
    agent
}

/// A key of the test's own in a home of its own; its agent is ended when
/// the value goes.
struct Signer {
    gpg: PathBuf,
    home: PathBuf,
    fingerprint: String,
}

impl Signer {
    fn new(gpg: &Path, base: &Path) -> Option<Self> {
        let home = base.join("signer");
        std::fs::create_dir_all(&home).ok()?;
        std::fs::set_permissions(&home, std::fs::Permissions::from_mode(0o700)).ok()?;
        let mut signer = Self {
            gpg: gpg.to_path_buf(),
            home,
            fingerprint: String::new(),
        };
        let made = signer
            .gpg(&[
                "--passphrase",
                "",
                "--quick-gen-key",
                "Stand-in <standin@example.invalid>",
                "ed25519",
                "sign",
                "never",
            ])
            .status()
            .ok()?;
        if !made.success() {
            return None;
        }
        let listed = signer
            .gpg(&["--with-colons", "--list-keys"])
            .output()
            .ok()?;
        let fingerprint = String::from_utf8_lossy(&listed.stdout)
            .lines()
            .find_map(|line| line.strip_prefix("fpr:"))
            .and_then(|rest| rest.split(':').find(|field| field.len() == 40))?
            .to_owned();
        signer.fingerprint = fingerprint;
        Some(signer)
    }

    fn gpg(&self, args: &[&str]) -> std::process::Command {
        let mut command = std::process::Command::new(&self.gpg);
        command
            .env_clear()
            .env("GNUPGHOME", &self.home)
            .env("PATH", "/usr/bin:/bin")
            .args(["--batch", "--no-tty", "--quiet"])
            .args(args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null());
        command
    }

    /// Signs the release's manifest and puts the public key beside it.
    fn sign(&self, release: &Path) {
        let signed = release.join("signed");
        let _ = std::fs::remove_file(signed.join("signature"));
        let status = self
            .gpg(&["--detach-sign", "-o"])
            .arg(signed.join("signature"))
            .arg(signed.join("manifest"))
            .status()
            .unwrap();
        assert!(status.success());
        let key = self.gpg(&["--armor", "--export"]).output().unwrap();
        std::fs::write(signed.join("key.asc"), key.stdout).unwrap();
    }
}

impl Drop for Signer {
    fn drop(&mut self) {
        let gpgconf = self.gpg.with_file_name("gpgconf");
        let _ = std::process::Command::new(gpgconf)
            .env("GNUPGHOME", &self.home)
            .args(["--kill", "all"])
            .status();
    }
}

#[test]
fn the_version_is_pinned() {
    let fixture = Fixture::new();
    // Update without an install.
    let found = fixture
        .prepare(&SIGNED, MaintenanceTask::Update, None)
        .unwrap_err();
    assert_eq!(found.code, AGENT_008);
    fixture.install(MaintenanceTask::Install, "1.0.0");
    // A second install is refused; only update changes the version.
    let found = fixture
        .prepare(&SIGNED, MaintenanceTask::Install, Some("2.0.0"))
        .unwrap_err();
    assert_failed_at(&found, "pin");
    // A run that installs another version than asked for.
    let run = fixture.run(&SIGNED, MaintenanceTask::Update, Some("2.0.0"));
    let release = fixture.release("3.0.0", None);
    let ok = fixture.execute(&run, &release);
    assert_failed_at(&run.commit_at(ok, "t").unwrap_err(), "pin");
    assert_eq!(fixture.depot(&SIGNED).installed().unwrap().version, "1.0.0");
    // A version name that is none.
    let found = fixture
        .prepare(&SIGNED, MaintenanceTask::Update, Some("../x"))
        .unwrap_err();
    assert_failed_at(&found, "pin");
    // The current version asked for: nothing to do.
    assert!(matches!(
        fixture
            .prepare(&SIGNED, MaintenanceTask::Update, Some("1.0.0"))
            .unwrap(),
        Maintenance::Current { .. }
    ));
}

#[test]
fn one_run_per_agent_at_a_time() {
    let fixture = Fixture::new();
    let _first = fixture.run(&SIGNED, MaintenanceTask::Install, None);
    let other = Fixture {
        base: tempfile::tempdir().unwrap(),
        paths: fixture.paths.clone(),
        session: SessionId::new(),
    };
    let found = other
        .prepare(&SIGNED, MaintenanceTask::Install, None)
        .unwrap_err();
    assert_failed_at(&found, "run");
}

/// An update while a session runs: the session keeps its version directory,
/// a new session gets the new one; old versions go only when no session
/// holds the depot, and the previous one stays for a way back.
#[test]
fn an_update_never_changes_a_running_session() {
    let fixture = Fixture::new();
    fixture.install(MaintenanceTask::Install, "1.0.0");
    let depot = fixture.depot(&SIGNED);
    let session = depot.reserve_for_session().unwrap();
    let running = depot.installed().unwrap();
    let before = snapshot(&running.dir);

    let committed = fixture.install(MaintenanceTask::Update, "2.0.0");
    assert_eq!(committed.previous.as_deref(), Some("1.0.0"));
    assert!(committed.pruned.is_empty());
    assert_eq!(
        snapshot(&running.dir),
        before,
        "the running session's version"
    );
    let fresh = depot.installed().unwrap();
    assert_eq!(fresh.version, "2.0.0");
    assert_ne!(fresh.dir, running.dir);

    let committed = fixture.install(MaintenanceTask::Update, "3.0.0");
    assert!(
        committed.pruned.is_empty(),
        "a session still holds the depot"
    );
    assert!(running.dir.exists());
    drop(session);

    let committed = fixture.install(MaintenanceTask::Update, "4.0.0");
    assert_eq!(committed.pruned, ["1.0.0", "2.0.0"]);
    assert!(
        depot.program_dir().join("3.0.0").exists(),
        "the way back stays"
    );

    // Back to the previous version: a switch, no run.
    let Maintenance::Switch { version } = fixture
        .prepare(&SIGNED, MaintenanceTask::Update, Some("3.0.0"))
        .unwrap()
    else {
        panic!("expected a switch");
    };
    let switched = switch_in(&depot, &version).unwrap();
    assert_eq!(switched.previous.as_deref(), Some("4.0.0"));
    assert_eq!(depot.installed().unwrap().version, "3.0.0");
}

/// The rules of a run allow exactly the recipe's hosts, in this session
/// only. Mutation proof (HUM-519): rules written for every session
/// (`Expiry::Never`, as stored rules would be) turn the follow-up session
/// red.
#[test]
fn the_rules_allow_only_the_recipes_hosts_and_only_in_this_session() {
    let fixture = Fixture::new();
    let claude = crate::agent::depot::recipe::known("claude").unwrap();
    let run = match prepare_for(
        &MaintenanceRequest {
            paths: &fixture.paths,
            agent: "claude",
            task: MaintenanceTask::Install,
            version: None,
            session: fixture.session,
        },
        claude,
        fixture.depot(claude),
        Some(HostPlatform::LinuxX64),
    )
    .unwrap()
    {
        Maintenance::Run(run) => *run,
        other => panic!("{other:?}"),
    };
    let rules = humanitl_rules::RuleSet::from_rules(run.session_rules());
    let now = humanitl_core::time::from_system_time(std::time::SystemTime::now());
    let verdict = |host: &str, session: SessionId| {
        let host = HostName::parse(host).unwrap();
        rules.evaluate(
            &humanitl_rules::RequestKey::new(
                &host,
                &Method::GET,
                "/claude-code-releases/latest",
                Scheme::Https,
                443,
            ),
            now,
            session,
        )
    };
    assert!(matches!(
        verdict("downloads.claude.ai", fixture.session),
        humanitl_rules::Verdict::Matched {
            action: humanitl_core::rule::Action::Allow,
            ..
        }
    ));
    // A host outside the list is held in the run.
    assert_eq!(
        verdict("storage.googleapis.com", fixture.session),
        humanitl_rules::Verdict::Default
    );
    assert_eq!(
        verdict("claude.ai", fixture.session),
        humanitl_rules::Verdict::Default
    );
    // The follow-up session holds even the recipe's host.
    assert_eq!(
        verdict("downloads.claude.ai", SessionId::new()),
        humanitl_rules::Verdict::Default
    );
    // The script asks for no host the rules do not allow.
    for url in run
        .script()
        .split_whitespace()
        .filter(|word| word.contains("https://"))
    {
        assert!(url.contains("https://downloads.claude.ai/"), "{url}");
    }
    assert!(
        run.script()
            .contains(claude.recipe.unwrap().install.unwrap().script.trim_end())
    );
}

#[test]
fn the_platform_must_be_one_the_vendor_builds_for() {
    let fixture = Fixture::new();
    let found = prepare_for(
        &MaintenanceRequest {
            paths: &fixture.paths,
            agent: SIGNED.id,
            task: MaintenanceTask::Install,
            version: None,
            session: fixture.session,
        },
        &SIGNED,
        fixture.depot(&SIGNED),
        Some(HostPlatform::MacArm64),
    )
    .unwrap_err();
    assert_eq!(found.code, humanitl_core::diagnostics::codes::AGENT_015);
}

#[test]
fn an_agent_without_an_install_recipe_is_agent_015() {
    let fixture = Fixture::new();
    let found = prepare_at(
        &MaintenanceRequest {
            paths: &fixture.paths,
            agent: "codex",
            task: MaintenanceTask::Install,
            version: None,
            session: fixture.session,
        },
        fixture.depot(crate::agent::depot::recipe::known("codex").unwrap()),
        Some(HostPlatform::LinuxX64),
    )
    .unwrap_err();
    assert_eq!(found.code, humanitl_core::diagnostics::codes::AGENT_015);
}

#[test]
fn the_run_gets_its_home_and_the_contract() {
    let fixture = Fixture::new();
    let run = fixture.run(&SIGNED, MaintenanceTask::Install, Some("1.2.3"));
    let env: BTreeMap<String, String> = run.env().into_iter().collect();
    assert_eq!(env["HOME"], MAINTENANCE_HOME_DST);
    assert_eq!(env[ENV_VERSION], "1.2.3");
    assert_eq!(env[ENV_PLATFORM], "linux-x64");
    assert_eq!(env[ENV_OUT], format!("{MAINTENANCE_HOME_DST}/out"));
    assert_eq!(run.command()[..2], ["/bin/sh", "-c"]);
    assert_eq!(
        run.kind(),
        &DepotMaintenance {
            agent: "standin".to_owned(),
            task: MaintenanceTask::Install,
        }
    );
    // The work directory lies beside the depots, never inside one.
    assert!(!run.home().starts_with(fixture.depot(&SIGNED).root()));
    assert!(
        run.home()
            // The daemon resolves its data directory (macOS: `/var` is a
            // link to `/private/var`); so does the comparison.
            .starts_with(
                std::fs::canonicalize(fixture.paths.data_dir())
                    .unwrap()
                    .join("agents/.maintenance")
            )
    );
}

/// A run that labels an authentic older release as the version asked for:
/// the signed manifest names 1.0.0, the run's `version` file 2.0.0, and the
/// checksum fits. Mutation proof (HUM-519, review): without the comparison
/// with the verified manifest's version this turns red.
#[test]
fn the_version_comes_from_the_signed_manifest() {
    refused_leaves_the_depot("signature", |fixture| {
        let gpg = fixture.fake_gpg(GOOD);
        let found = update_unchanged(fixture, &SIGNED, Some(gpg), |run, _| {
            let out = run.home().join(OUT_DIR);
            let program = std::fs::read(out.join("program/standin")).unwrap();
            std::fs::write(
                out.join("signed/manifest"),
                format!(
                    r#"{{"version":"1.0.0","platforms":{{"linux-x64":{{"checksum":"{}"}}}}}}"#,
                    hex_sum(&program)
                ),
            )
            .unwrap();
        });
        assert!(
            found.why.contains("signed manifest names 1.0.0"),
            "{}",
            found.why
        );
        found
    });
}

/// Every directory of the staged tree is synced, leaf up, and so is every
/// directory a first install creates, with its parent. Mutation proof
/// (HUM-519, review): without the sync at the end of `copy_dir` this turns
/// red.
#[test]
fn the_directories_are_synced() {
    let fixture = Fixture::new();
    let _ = adopt::sync_probe::take();
    fixture.install(MaintenanceTask::Install, "1.0.0");
    let synced = adopt::sync_probe::take();
    // A synced staging directory is gone after its `rename` and cannot be
    // resolved any more; it is matched against both spellings of `agents/`
    // (macOS: `/var` is a link to `/private/var`).
    let raw = fixture.agents();
    let agents = std::fs::canonicalize(&raw).unwrap_or_else(|_| raw.clone());
    let staged: Vec<_> = synced
        .iter()
        .filter_map(|path| {
            let rest = path
                .strip_prefix(&agents)
                .or_else(|_| path.strip_prefix(&raw))
                .ok()?;
            let mut parts = rest.components();
            let first = parts.next()?.as_os_str().to_string_lossy().into_owned();
            first
                .starts_with(".staging-")
                .then(|| parts.as_path().to_path_buf())
        })
        .collect();
    let lib = staged.iter().position(|path| path == Path::new("lib"));
    let root = staged.iter().position(|path| path.as_os_str().is_empty());
    assert!(lib.is_some() && root.is_some(), "{synced:?}");
    assert!(lib < root, "leaf up: {synced:?}");
    let depot = fixture.depot(&SIGNED);
    for dir in [
        fixture.agents(),
        depot.root().to_path_buf(),
        depot.program_dir(),
        depot.config_dir(),
        depot.credentials_dir(),
    ] {
        let dir = std::fs::canonicalize(&dir).unwrap();
        assert!(
            synced
                .iter()
                .any(|path| std::fs::canonicalize(path).ok().as_ref() == Some(&dir)),
            "{} not synced: {synced:?}",
            dir.display()
        );
    }
}

/// The update of `fixture` to 2.0.0 with the given syncs of the depot's
/// root failing (0 the journal, 1 the switch, 2 the end of the journal).
fn update_with_failing_root_syncs(fixture: &Fixture, calls: &[usize]) -> Diagnostic {
    let run = fixture.run(&SIGNED, MaintenanceTask::Update, Some("2.0.0"));
    let release = fixture.release("2.0.0", None);
    assert!(fixture.execute(&run, &release));
    adopt::sync_probe::fail(fixture.depot(&SIGNED).root(), calls);
    let found = run.commit_at(true, "t").unwrap_err();
    let _ = adopt::sync_probe::take();
    found
}

/// The `rename` of `current` succeeds and the `fsync` after it fails: the
/// answer is "unchanged", so `current` is set back and the placed version
/// goes. Mutation proof (HUM-519, review): without setting it back
/// `current` names 2.0.0 and this turns red.
#[test]
fn a_switch_whose_sync_fails_is_set_back() {
    for calls in [&[1][..], &[2][..]] {
        let fixture = Fixture::new();
        fixture.install(MaintenanceTask::Install, "1.0.0");
        let found = update_with_failing_root_syncs(&fixture, calls);
        assert_failed_at(&found, "commit");
        assert!(
            found.why.contains("nothing was taken over"),
            "{}",
            found.why
        );
        let depot = fixture.depot(&SIGNED);
        assert_eq!(
            depot.current_version().as_deref(),
            Some("1.0.0"),
            "{calls:?}"
        );
        assert!(!depot.program_dir().join("2.0.0").exists(), "{calls:?}");
        assert!(!depot.root().join(adopt::JOURNAL).exists(), "{calls:?}");
    }
}

/// Setting `current` back fails as well: the finding says the depot may be
/// switched, and nothing is removed under it.
#[test]
fn a_failed_setting_back_is_reported_as_uncertain() {
    let fixture = Fixture::new();
    fixture.install(MaintenanceTask::Install, "1.0.0");
    let found = update_with_failing_root_syncs(&fixture, &[1, 2]);
    assert_eq!(found.code, AGENT_011);
    assert!(
        found.why.contains("may name version 2.0.0"),
        "{}",
        found.why
    );
    assert!(
        !found.why.contains("nothing was taken over"),
        "{}",
        found.why
    );
    assert!(fixture.depot(&SIGNED).program_dir().join("2.0.0").exists());
}
