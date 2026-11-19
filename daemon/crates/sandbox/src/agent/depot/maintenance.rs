//! Install and update of a depot agent as a maintenance run (ADR-0023
//! point 5, HUM-519).
//!
//! An install never runs on the host. The daemon prepares a run
//! ([`prepare`]) whose parts a sandbox session carries like any other:
//!
//! - a work directory of its own as `HOME` ([`MaintenanceRun::home`], bound
//!   writable at [`MAINTENANCE_HOME_DST`]); no project, no data of the host;
//! - one command, `/bin/sh -c <script>` ([`MaintenanceRun::command`]): the
//!   adapter's [`InstallRecipe::script`], which only downloads; the run
//!   reports no verdict, and the daemon reads none from it;
//! - the variables of the recipe's contract ([`MaintenanceRun::env`]);
//! - `allow` rules for exactly the recipe's hosts, valid for this run's
//!   session and no other ([`MaintenanceRun::session_rules`]); every other
//!   request of the run is held like in any session, and nothing touches the
//!   stored rules.
//!
//! After the run the daemon takes the result over ([`MaintenanceRun::commit`]):
//! the tree checks of [`super::adopt`] on a copy of its own, the vendor's
//! signature checked by the daemon with the host's `gpg` on that copy
//! ([`super::verify`]), the checksum against the verified manifest, the
//! manifest of the depot, `program/<version>/`, then `current`. Every
//! failure is `AGENT_011` with its step, and leaves the depot as it was.
//!
//! A prepared run holds two locks until it is committed or dropped: the
//! maintenance lock of the agent (one run or switch at a time) and a shared
//! reservation of the depot, so `agents remove` cannot pull the depot from
//! under an update.
//!
//! **The version is pinned.** `install` refuses a depot that holds a version
//! already; only `update` changes it. An update to a version the depot still
//! holds switches back without a run ([`Maintenance::Switch`]).

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::os::fd::AsFd as _;
use std::path::{Path, PathBuf};

use humanitl_config::Paths;
use humanitl_core::diagnostics::codes::{AGENT_008, AGENT_015, AGENT_018};
use humanitl_core::ids::{RuleId, SessionId};
use humanitl_core::rule::{Action, Expiry, HostPattern, Matcher, Rule};
use humanitl_core::{Diagnostic, FixAction, HostName, Method, Scheme, Severity};

use super::adopt::{self, Staging, Step, failed};
use super::recipe::{HostPlatform, InstallRecipe, KnownAgent};
use super::store::{Depot, MANIFEST_FILE, Manifest, Reservation, version_name_ok};
use super::verify;

/// Where the run's work directory lies in the sandbox: its `HOME`. Under
/// `/run/humanitl`, which never comes from the host.
pub const MAINTENANCE_HOME_DST: &str = "/run/humanitl/agent-home";

/// The result directory, relative to `HOME`.
pub const OUT_DIR: &str = "out";

/// The variables of the recipe's contract.
pub const ENV_VERSION: &str = "HUMANITL_DEPOT_VERSION";
/// The vendor's name of the platform.
pub const ENV_PLATFORM: &str = "HUMANITL_DEPOT_PLATFORM";
/// The result directory in the sandbox.
pub const ENV_OUT: &str = "HUMANITL_DEPOT_OUT";

/// What a maintenance run does (ADR-0023 point 5). HUM-520 adds the login
/// and the configuration run; a caller that maps a task to words of its own
/// (`as_str`, the protocol's operations) matches every variant, so a new
/// one shows up there at compile time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaintenanceTask {
    /// The first install; refused when the depot holds a version.
    Install,
    /// A new version, or back to one the depot holds.
    Update,
}

impl MaintenanceTask {
    /// The word of the CLI: `install` or `update`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Install => "install",
            Self::Update => "update",
        }
    }
}

/// The session kind of a maintenance run: which agent, which task. A
/// session of this kind has no `/work`, its `HOME` is the run's work
/// directory, its network goes only through the proxy, and it carries no
/// briefing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DepotMaintenance {
    /// The agent's identifier.
    pub agent: String,
    /// The task.
    pub task: MaintenanceTask,
}

/// What [`prepare`] decided.
#[derive(Debug)]
pub enum Maintenance {
    /// A run in a sandbox.
    Run(Box<MaintenanceRun>),
    /// The depot holds the asked version already: `current` switches to it
    /// without a run ([`switch`]).
    Switch {
        /// The version.
        version: String,
    },
    /// The asked version is the current one; nothing to do.
    Current {
        /// The version.
        version: String,
    },
}

/// A prepared run: its work directory, command, environment and rules. The
/// work directory goes with this value.
#[derive(Debug)]
pub struct MaintenanceRun {
    agent: &'static KnownAgent,
    install: InstallRecipe,
    kind: DepotMaintenance,
    requested: Option<String>,
    session: SessionId,
    platform: &'static str,
    depot: Depot,
    work: PathBuf,
    /// The host's `gpg`, which verifies a signed recipe's download.
    gpg: Option<PathBuf>,
    /// Held from preparing to the end: one run or switch per agent at a
    /// time.
    _lock: Reservation,
    /// A shared reservation of the depot from preparing to the takeover:
    /// a removal waits for it.
    hold: Option<Reservation>,
}

/// What a takeover changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Committed {
    /// The version `current` names now.
    pub version: String,
    /// The version it named before, if any.
    pub previous: Option<String>,
    /// The versions removed because no session used them.
    pub pruned: Vec<String>,
    /// The manifest written.
    pub manifest: Manifest,
}

/// What a caller asks.
#[derive(Debug, Clone, Copy)]
pub struct MaintenanceRequest<'a> {
    /// The daemon's paths.
    pub paths: &'a Paths,
    /// The agent's identifier.
    pub agent: &'a str,
    /// Install or update.
    pub task: MaintenanceTask,
    /// The version asked for; `None` for the vendor's current one.
    pub version: Option<&'a str>,
    /// The session the run will be.
    pub session: SessionId,
}

/// Prepares an install or update of `request.agent`.
///
/// # Errors
///
/// `CONFIG`-free findings only: `AGENT_015` for an agent without an install
/// recipe or a platform the vendor does not build for, `AGENT_011` at the
/// step `pin` for an install over an installed version or a version name
/// that is none, `AGENT_008` for an update without an install, `AGENT_013`
/// for a depot that fails its checks, `AGENT_011` at the step `run` while
/// another run of the agent is prepared.
pub fn prepare(request: &MaintenanceRequest<'_>) -> Result<Maintenance, Diagnostic> {
    prepare_at(
        request,
        Depot::of(request.paths, request.agent),
        host_platform(),
    )
}

/// [`prepare`] for the agent `entry` names, its install recipe included;
/// `request.agent` is ignored. The daemon takes its agents from
/// [`super::KNOWN`]; its end-to-end tests give a stand-in recipe this way
/// (HUM-563).
///
/// # Errors
///
/// As [`prepare`].
pub fn prepare_known(
    request: &MaintenanceRequest<'_>,
    entry: &'static KnownAgent,
) -> Result<Maintenance, Diagnostic> {
    prepare_for(
        request,
        entry,
        Depot::of(request.paths, entry.id),
        host_platform(),
    )
}

/// [`prepare`] with an explicit depot and platform; for tests.
pub(crate) fn prepare_at(
    request: &MaintenanceRequest<'_>,
    depot: Depot,
    platform: Option<HostPlatform>,
) -> Result<Maintenance, Diagnostic> {
    let agent = super::recipe::known(request.agent).ok_or_else(|| {
        Diagnostic::builder(AGENT_015, Severity::Error)
            .why(format!("{:?} is no agent of the depot", request.agent))
            .build()
    })?;
    prepare_for(request, agent, depot, platform)
}

pub(crate) fn prepare_for(
    request: &MaintenanceRequest<'_>,
    agent: &'static KnownAgent,
    depot: Depot,
    platform: Option<HostPlatform>,
) -> Result<Maintenance, Diagnostic> {
    let install = agent
        .recipe
        .and_then(|recipe| recipe.install)
        .ok_or_else(|| no_install_recipe(agent))?;
    if let Some(version) = request.version
        && !version_name_ok(version)
    {
        return Err(failed(
            agent.id,
            Step::Pin,
            &format!("{version:?} is no version name"),
        ));
    }
    let platform_name = platform
        .and_then(|platform| {
            install
                .platforms
                .iter()
                .find(|(known, _)| *known == platform)
                .map(|(_, name)| *name)
        })
        .ok_or_else(|| {
            Diagnostic::builder(AGENT_015, Severity::Error)
                .why(format!(
                    "the vendor of {} builds for no platform the daemon runs on ({:?})",
                    agent.display_name, platform
                ))
                .build()
        })?;
    if let Some(decided) = pinned(request, agent, &depot)? {
        return Ok(decided);
    }
    let lock = maintenance_lock(&depot)?;
    let hold = depot.reserve_for_session()?;
    let work = work_dir(request.paths, agent.id, request.session);
    let _ = std::fs::remove_dir_all(&work);
    for dir in [
        work.parent().map(Path::to_path_buf),
        Some(work.clone()),
        Some(work.join(OUT_DIR)),
    ]
    .into_iter()
    .flatten()
    {
        adopt::make_dir(&dir).map_err(|err| {
            failed(
                agent.id,
                Step::Run,
                &format!("{} cannot be created ({err})", dir.display()),
            )
        })?;
    }
    Ok(Maintenance::Run(Box::new(MaintenanceRun {
        agent,
        install,
        kind: DepotMaintenance {
            agent: agent.id.to_owned(),
            task: request.task,
        },
        requested: request.version.map(ToOwned::to_owned),
        session: request.session,
        platform: platform_name,
        depot,
        work,
        gpg: super::verify::host_gpg(),
        _lock: lock,
        hold: Some(hold),
    })))
}

/// The pin of the version (ADR-0023 point 5): an install over an installed
/// version is refused, an update needs an install; an update to the current
/// version or to one the depot holds needs no run. `None`: a run follows.
fn pinned(
    request: &MaintenanceRequest<'_>,
    agent: &KnownAgent,
    depot: &Depot,
) -> Result<Option<Maintenance>, Diagnostic> {
    let installed = match depot.installed() {
        Ok(installed) => Some(installed),
        Err(found) if found.code == AGENT_008 => None,
        Err(found) => return Err(found),
    };
    match (request.task, &installed) {
        (MaintenanceTask::Install, Some(installed)) => {
            let mut found = failed(
                agent.id,
                Step::Pin,
                &format!(
                    "the depot holds version {} already, and only an update changes it",
                    installed.version
                ),
            );
            found.fix = Some(FixAction::CopyCommand(format!(
                "humanitl agents update {}",
                agent.id
            )));
            return Err(found);
        }
        (MaintenanceTask::Update, None) => {
            return Err(Diagnostic::builder(AGENT_008, Severity::Error)
                .why(format!(
                    "the depot holds no version of {} to update",
                    agent.display_name
                ))
                .fix(FixAction::CopyCommand(format!(
                    "humanitl agents install {}",
                    agent.id
                )))
                .build());
        }
        (MaintenanceTask::Update, Some(installed)) => {
            if let Some(version) = request.version {
                if version == installed.version {
                    return Ok(Some(Maintenance::Current {
                        version: version.to_owned(),
                    }));
                }
                if std::fs::symlink_metadata(depot.program_dir().join(version))
                    .is_ok_and(|meta| meta.is_dir())
                {
                    return Ok(Some(Maintenance::Switch {
                        version: version.to_owned(),
                    }));
                }
            }
        }
        (MaintenanceTask::Install, None) => {}
    }
    Ok(None)
}

/// Switches `current` to a version the depot holds, after checking it like
/// a start would (an update back, `agents update <id> --version <old>`).
///
/// # Errors
///
/// `AGENT_013` for a depot or version that fails its checks, `AGENT_011`
/// when the switch fails.
pub fn switch(paths: &Paths, agent: &str, version: &str) -> Result<Committed, Diagnostic> {
    switch_in(&Depot::of(paths, agent), version)
}

pub(crate) fn switch_in(depot: &Depot, version: &str) -> Result<Committed, Diagnostic> {
    // Serialised with every run of the agent: a takeover and its pruning
    // never see `current` change under them.
    let _lock = maintenance_lock(depot)?;
    let _hold = depot.reserve_for_session()?;
    adopt::recover(depot);
    let previous = depot.installed()?.version;
    let manifest = super::store::read_manifest(depot, version)?;
    switch_or_back(depot, version, Some(&previous)).map_err(|failed| failed.found)?;
    Ok(Committed {
        version: version.to_owned(),
        previous: Some(previous),
        pruned: Vec::new(),
        manifest,
    })
}

impl MaintenanceRun {
    /// The session kind.
    #[must_use]
    pub fn kind(&self) -> &DepotMaintenance {
        &self.kind
    }

    /// The agent this run installs or updates.
    #[must_use]
    pub fn agent(&self) -> &'static KnownAgent {
        self.agent
    }

    /// The hosts the run may reach without a hold, from the recipe.
    #[must_use]
    pub fn hosts(&self) -> Vec<&'static str> {
        self.install.hosts.iter().map(|host| host.host).collect()
    }

    /// The work directory on the host, bound writable at
    /// [`MAINTENANCE_HOME_DST`] as the run's `HOME`.
    #[must_use]
    pub fn home(&self) -> &Path {
        &self.work
    }

    /// The command of the run.
    #[must_use]
    pub fn command(&self) -> Vec<OsString> {
        vec![
            OsString::from("/bin/sh"),
            OsString::from("-c"),
            OsString::from(self.script()),
        ]
    }

    /// The variables of the run, for a `HOME` at [`MAINTENANCE_HOME_DST`].
    #[must_use]
    pub fn env(&self) -> Vec<(String, String)> {
        self.env_for(Path::new(MAINTENANCE_HOME_DST))
    }

    /// The variables of the run for a `HOME` at `home`.
    pub(crate) fn env_for(&self, home: &Path) -> Vec<(String, String)> {
        vec![
            ("HOME".to_owned(), home.display().to_string()),
            (
                ENV_VERSION.to_owned(),
                self.requested.clone().unwrap_or_default(),
            ),
            (ENV_PLATFORM.to_owned(), self.platform.to_owned()),
            (ENV_OUT.to_owned(), home.join(OUT_DIR).display().to_string()),
        ]
    }

    /// The script: the recipe in a subshell under `set -eu`. It only
    /// downloads; whatever else it writes, the daemon reads no verdict from
    /// it.
    #[must_use]
    pub fn script(&self) -> String {
        format!(
            "set -eu\numask 077\nout=\"${ENV_OUT}\"\nmkdir -p \"$out\"\n(\n{}\n)\n",
            self.install.script.trim_end()
        )
    }

    /// The same run with another `gpg` on the host; for tests.
    #[cfg(test)]
    pub(crate) fn with_gpg(mut self, gpg: Option<PathBuf>) -> Self {
        self.gpg = gpg;
        self
    }

    /// The `allow` rules of this run: one per recipe host, `GET` and `HEAD`
    /// over HTTPS on port 443, valid only for this session
    /// ([`Expiry::Session`]). The caller adds them to the session's rules,
    /// never to the stored ones.
    #[must_use]
    pub fn session_rules(&self) -> Vec<Rule> {
        session_rules(self.agent, &self.install, self.session)
    }

    /// Takes the result of a finished run over into the depot.
    ///
    /// `succeeded` is whether the run's command ended with status 0; a run
    /// that did not is refused before anything is read.
    ///
    /// # Errors
    ///
    /// `AGENT_011` with the step that failed; the depot is then unchanged.
    pub fn commit(self, succeeded: bool) -> Result<Committed, Diagnostic> {
        self.commit_at(succeeded, &now())
    }

    pub(crate) fn commit_at(mut self, succeeded: bool, at: &str) -> Result<Committed, Diagnostic> {
        let agent = self.agent.id;
        if !succeeded {
            return Err(failed(
                agent,
                Step::Run,
                "the run ended with an error (a download that failed, or an installer that \
                 wants sudo or writes into /usr, which the sandbox does not allow); its output \
                 in the terminal names the cause",
            ));
        }
        let result = self.read_result()?;
        let depot = self.depot.clone();
        // The signature first: the version of a signed install is the one
        // the verified manifest names, and every decision below uses it.
        let verified = self.verify_download(&depot, &result)?;
        adopt::recover(&depot);
        let previous = match depot.installed() {
            Ok(installed) => Some(installed.version),
            Err(found) if found.code == AGENT_008 => None,
            Err(found) => return Err(found),
        };
        let version = result.version.clone();
        match (self.kind.task, &previous) {
            (MaintenanceTask::Install, Some(_)) => {
                return Err(failed(
                    agent,
                    Step::Pin,
                    "another run installed a version meanwhile, and only an update changes it",
                ));
            }
            (MaintenanceTask::Update, None) => {
                return Err(failed(
                    agent,
                    Step::Pin,
                    "the depot holds no installed version any more, so there is nothing to update",
                ));
            }
            _ => {}
        }
        if previous.as_deref() == Some(version.as_str()) {
            let manifest = super::store::read_manifest(&depot, &version)?;
            return Ok(Committed {
                version,
                previous,
                pruned: Vec::new(),
                manifest,
            });
        }
        let manifest = if std::fs::symlink_metadata(depot.program_dir().join(&version)).is_ok() {
            // Back to a version the depot still holds, checked at its own
            // install: the download is discarded.
            let kept = super::store::read_manifest(&depot, &version)?;
            switch_or_back(&depot, &version, previous.as_deref()).map_err(|failed| failed.found)?;
            kept
        } else {
            let (staging, manifest) = self.stage(&depot, result, verified.as_ref(), at)?;
            publish(&depot, staging, &version, &mut self.hold)?;
            manifest
        };
        // The takeover is done; the pruning needs the depot for itself.
        drop(self.hold.take());
        let pruned = prune_unused(&depot, &[version.as_str()], previous.as_deref());
        Ok(Committed {
            version,
            previous,
            pruned,
            manifest,
        })
    }

    /// Reads what the run left: the version, the program tree, and for a
    /// signed recipe the files to verify. No verdict of the run is read.
    fn read_result(&self) -> Result<RunResult, Diagnostic> {
        let agent = self.agent.id;
        let home = adopt::open_dir(&self.work).map_err(|err| {
            failed(
                agent,
                Step::Result,
                &format!("the work directory cannot be opened ({err})"),
            )
        })?;
        let out =
            adopt::open_dir_at(home.as_fd(), std::ffi::OsStr::new(OUT_DIR)).map_err(|err| {
                failed(
                    agent,
                    Step::Result,
                    &format!("{OUT_DIR}/ cannot be opened ({err})"),
                )
            })?;
        let read = |rel: &str| {
            adopt::read_small(out.as_fd(), Path::new(rel), adopt::SMALL_FILE_CAP)
                .map_err(|what| failed(agent, Step::Result, &what))
        };
        let version = read("version")?
            .ok_or_else(|| failed(agent, Step::Result, "the run named no version"))?;
        let version = String::from_utf8_lossy(&version).trim().to_owned();
        if !version_name_ok(&version) {
            return Err(failed(
                agent,
                Step::Result,
                &format!("the run named {version:?}, which is no version name"),
            ));
        }
        if let Some(asked) = &self.requested
            && *asked != version
        {
            return Err(failed(
                agent,
                Step::Pin,
                &format!("version {asked} was asked for, and the run installed {version}"),
            ));
        }
        let signed = if self.install.signature.is_some() {
            let mut files = Vec::new();
            for name in [verify::MANIFEST, verify::SIGNATURE, verify::KEY] {
                let bytes = read(&format!("signed/{name}"))?.ok_or_else(|| {
                    failed(
                        agent,
                        Step::Signature,
                        &format!("the run left no signed/{name} of the vendor"),
                    )
                })?;
                files.push((name, bytes));
            }
            Some(files)
        } else {
            None
        };
        let program =
            adopt::open_dir_at(out.as_fd(), std::ffi::OsStr::new("program")).map_err(|err| {
                failed(
                    agent,
                    Step::Result,
                    &format!("program/ cannot be opened ({err})"),
                )
            })?;
        Ok(RunResult {
            version,
            signed,
            program,
        })
    }

    /// Copies the program tree into a staging directory with its checks,
    /// verifies the vendor's signature on a copy of the daemon's own, compares
    /// the staged program's SHA-256 with the verified manifest, and writes the
    /// depot's manifest into the staging directory.
    fn stage(
        &self,
        depot: &Depot,
        result: RunResult,
        verified: Option<&Verified>,
        at: &str,
    ) -> Result<(Staging, Manifest), Diagnostic> {
        let agent = self.agent.id;
        let staging = Staging::new(depot).map_err(|err| private(agent, &err))?;
        let mut files = BTreeMap::new();
        adopt::take_tree(agent, result.program.as_fd(), staging.path(), &mut files)?;
        let Some(sum) = files.get(self.install.program).cloned() else {
            return Err(failed(
                agent,
                Step::Result,
                &format!(
                    "the program tree holds no regular file {}",
                    self.install.program
                ),
            ));
        };
        if let Some(verified) = verified
            && sum != verified.checksum
        {
            return Err(failed(
                agent,
                Step::Checksum,
                &format!(
                    "the SHA-256 of {} is {sum}, and the vendor's signed manifest names {}",
                    self.install.program, verified.checksum
                ),
            ));
        }
        let signed_by = verified.map(|verified| verified.signed_by.clone());
        let manifest = Manifest {
            version: result.version,
            source: self.install.source.to_owned(),
            files,
            signature: if signed_by.is_some() {
                "verified".to_owned()
            } else {
                "unsigned".to_owned()
            },
            signed_by: signed_by.unwrap_or_default(),
            installed_at: at.to_owned(),
        };
        write_manifest(&staging.path().join(MANIFEST_FILE), &manifest)
            .and_then(|()| {
                adopt::sync_dir(staging.path())
                    .map_err(|err| format!("the staging directory cannot be synced ({err})"))
            })
            .map_err(|what| failed(agent, Step::Commit, &what))?;
        Ok((staging, manifest))
    }

    /// Verifies a signed recipe's download on the daemon's own copy
    /// ([`verify::verify`]) and reads version and checksum from the verified
    /// manifest. The version must be the one the run named (and so the one
    /// asked for): a run cannot label an authentic older release as a newer
    /// one. `None` for an unsigned recipe.
    fn verify_download(
        &self,
        depot: &Depot,
        result: &RunResult,
    ) -> Result<Option<Verified>, Diagnostic> {
        let agent = self.agent.id;
        let (Some(signature), Some(signed)) = (self.install.signature, &result.signed) else {
            return Ok(None);
        };
        let verify_dir = Staging::for_verify(depot).map_err(|err| private(agent, &err))?;
        for (name, bytes) in signed {
            write_private(&verify_dir.path().join(name), bytes)
                .map_err(|what| failed(agent, Step::Signature, &what))?;
        }
        verify::verify(
            self.gpg.as_deref(),
            verify_dir.path(),
            signature.fingerprint,
        )
        .map_err(|why| failed(agent, Step::Signature, &why))?;
        // Version and checksum come from the bytes gpg verified, the
        // daemon's copy, never from the run's directory again.
        let manifest = signed
            .iter()
            .find(|(name, _)| *name == verify::MANIFEST)
            .map(|(_, bytes)| bytes.as_slice())
            .unwrap_or_default();
        let version = adopt::vendor_version(manifest, signature.version_pointer)
            .map_err(|why| failed(agent, Step::Signature, &why))?;
        if version != result.version {
            return Err(failed(
                agent,
                Step::Signature,
                &format!(
                    "the run named version {}, and the vendor's signed manifest names {version}",
                    result.version
                ),
            ));
        }
        let checksum = adopt::vendor_checksum(manifest, signature.checksum_pointer, self.platform)
            .map_err(|why| failed(agent, Step::Checksum, &why))?;
        Ok(Some(Verified {
            checksum,
            signed_by: signature.fingerprint.to_owned(),
        }))
    }
}

/// What the daemon verified of a signed download.
#[derive(Debug)]
struct Verified {
    /// The program's SHA-256 in the verified manifest.
    checksum: String,
    /// The fixed key that signed it.
    signed_by: String,
}

/// `AGENT_011` at the step `commit` for a private directory that cannot be
/// made.
fn private(agent: &str, err: &std::io::Error) -> Diagnostic {
    failed(
        agent,
        Step::Commit,
        &format!("a private directory of the takeover cannot be created ({err})"),
    )
}

/// What a failed switch left.
#[derive(Debug)]
struct SwitchFailed {
    found: Diagnostic,
    /// `current` may name the new version: setting it back failed too.
    uncertain: bool,
}

/// Switches `current` to `version`; when that fails, sets it back to
/// `previous`, since the `rename` may have happened before the failure.
/// When setting it back fails too, the finding says the depot may be
/// switched ([`adopt::uncertain`]).
fn switch_or_back(
    depot: &Depot,
    version: &str,
    previous: Option<&str>,
) -> Result<(), SwitchFailed> {
    adopt::switch_current(depot, version).map_err(|what| back(depot, version, previous, &what))
}

/// Sets `current` back after `what` failed; the finding to report.
fn back(depot: &Depot, version: &str, previous: Option<&str>, what: &str) -> SwitchFailed {
    match adopt::roll_back(depot, previous) {
        Ok(()) => SwitchFailed {
            found: failed(depot.agent(), Step::Commit, what),
            uncertain: false,
        },
        Err(undo) => SwitchFailed {
            found: adopt::uncertain(depot.agent(), version, what, &undo),
            uncertain: true,
        },
    }
}

/// Makes a staged version the current one: the depot's directories (only
/// now, so a refused first install leaves no depot), the journal, the
/// `rename` into `program/<version>/`, the switch of `current`, the end of
/// the journal. A failure undoes what this call did: a depot it created
/// goes, a version it placed goes ([`adopt::recover`]).
fn publish(
    depot: &Depot,
    staging: Staging,
    version: &str,
    hold: &mut Option<Reservation>,
) -> Result<(), Diagnostic> {
    let created = !depot.exists();
    let previous = depot.current_version();
    let mut placed = false;
    let done = (|| {
        ensure_layout(depot).map_err(|what| failed(depot.agent(), Step::Commit, &what))?;
        adopt::begin_journal(depot, version)?;
        staging.place(depot, version)?;
        placed = true;
        Ok(())
    })()
    .map_err(|found| SwitchFailed {
        found,
        uncertain: false,
    })
    .and_then(|()| {
        switch_or_back(depot, version, previous.as_deref())?;
        // `current` names the new version now; a journal that cannot end
        // sets it back, so the answer "unchanged" stays true.
        adopt::end_journal(depot).map_err(|what| back(depot, version, previous.as_deref(), &what))
    });
    match done {
        Ok(()) => Ok(()),
        // The depot may be switched: nothing is removed under it.
        Err(SwitchFailed {
            found,
            uncertain: true,
        }) => Err(found),
        Err(SwitchFailed { found, .. }) => {
            // `current` named the new version for a moment, and a session may
            // have started on it: what this call placed goes only under the
            // depot's exclusive lock, which the run's own shared hold would
            // block. A version a session uses stays for a later pruning.
            drop(hold.take());
            if created {
                adopt::remove_unused_depot(depot);
            } else {
                adopt::recover(depot);
                // The journal may be gone already (its end failed after the
                // removal); the version this call placed goes all the same.
                if placed {
                    adopt::remove_unused_version(depot, version);
                }
            }
            Err(found)
        }
    }
}

/// A file of the daemon, `0600`, written once.
fn write_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt as _;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .and_then(|mut file| file.write_all(bytes))
        .map_err(|err| format!("{} cannot be written ({err})", path.display()))
}

/// What a run left, as the daemon read it.
#[derive(Debug)]
struct RunResult {
    version: String,
    /// For a signed recipe: manifest, signature and key, by their names in
    /// [`verify`].
    signed: Option<Vec<(&'static str, Vec<u8>)>>,
    /// The program tree, opened without following a link.
    program: std::os::fd::OwnedFd,
}

impl Drop for MaintenanceRun {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.work);
    }
}

/// The session rules of a run of `agent`.
pub(crate) fn session_rules(
    agent: &KnownAgent,
    install: &InstallRecipe,
    session: SessionId,
) -> Vec<Rule> {
    install
        .hosts
        .iter()
        .filter_map(|host| {
            let name = HostName::parse(host.host).ok()?;
            Some(
                Rule::new(
                    RuleId::new(),
                    Action::Allow,
                    Matcher::host(HostPattern::Exact(name))
                        .with_methods(vec![Method::GET, Method::HEAD])
                        .with_scheme(Scheme::Https)
                        .with_port(443),
                )
                .with_expiry(Expiry::Session(session))
                .with_note(format!(
                    "Install and update of {} in the depot, this run only. Source: {}",
                    agent.display_name, host.evidence
                )),
            )
        })
        .collect()
}

/// The work directory of a run, under the daemon's data directory and
/// beside the depots, never inside one.
fn work_dir(paths: &Paths, agent: &str, session: SessionId) -> PathBuf {
    let data = std::fs::canonicalize(paths.data_dir()).unwrap_or_else(|_| paths.data_dir());
    data.join("agents")
        .join(".maintenance")
        .join(format!("{agent}-{session}"))
}

/// One run per agent at a time: an exclusive lock on
/// `agents/<id>.maintenance.lock`.
fn maintenance_lock(depot: &Depot) -> Result<Reservation, Diagnostic> {
    depot.lock_named("maintenance").map_err(|found| {
        if found.code == AGENT_018 {
            failed(
                depot.agent(),
                Step::Run,
                "another install, update or switch of this agent runs",
            )
        } else {
            found
        }
    })
}

/// Creates the depot's directories, `0700`, where they are missing.
/// Each directory it creates is synced, and so is its parent, which holds
/// the new entry.
fn ensure_layout(depot: &Depot) -> Result<(), String> {
    let parent = depot.root().parent().unwrap_or(depot.root()).to_path_buf();
    for dir in [
        parent,
        depot.root().to_path_buf(),
        depot.program_dir(),
        depot.config_dir(),
        depot.credentials_dir(),
    ] {
        let new = std::fs::symlink_metadata(&dir).is_err();
        adopt::make_dir(&dir)
            .map_err(|err| format!("{} cannot be created ({err})", dir.display()))?;
        if new {
            for synced in [Some(dir.as_path()), dir.parent()].into_iter().flatten() {
                adopt::sync_dir(synced)
                    .map_err(|err| format!("{} cannot be synced ({err})", synced.display()))?;
            }
        }
    }
    Ok(())
}

fn write_manifest(path: &Path, manifest: &Manifest) -> Result<(), String> {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt as _;
    let text = serde_json::to_string_pretty(manifest)
        .map_err(|err| format!("the manifest cannot be written ({err})"))?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|err| format!("the manifest cannot be written ({err})"))?;
    file.write_all(text.as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|err| format!("the manifest cannot be written ({err})"))
}

/// Removes old versions when no session holds the depot: every version but
/// the current and the previous one, which stays for a way back. A session
/// that runs keeps its version: then nothing is removed now. What is kept is
/// decided under the exclusive lock, with `current` as it lies on disk then.
pub(crate) fn prune_unused(depot: &Depot, keep: &[&str], previous: Option<&str>) -> Vec<String> {
    let Ok(_exclusive) = depot.reserve(true) else {
        return Vec::new();
    };
    let Some(current) = depot.current_version() else {
        return Vec::new();
    };
    let mut kept: Vec<&str> = keep.to_vec();
    kept.push(current.as_str());
    kept.extend(previous);
    adopt::prune(depot, &kept)
}

fn no_install_recipe(agent: &KnownAgent) -> Diagnostic {
    Diagnostic::builder(AGENT_015, Severity::Error)
        .why(format!(
            "the adapter of {} brings no install recipe for the depot yet; nothing is installed",
            agent.display_name
        ))
        .build()
}

/// The platform the daemon runs on, in the daemon's names; `None` for one
/// no recipe knows.
#[must_use]
pub fn host_platform() -> Option<HostPlatform> {
    let musl = |arch: &str| Path::new(&format!("/lib/ld-musl-{arch}.so.1")).exists();
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") if musl("x86_64") => Some(HostPlatform::LinuxX64Musl),
        ("linux", "x86_64") => Some(HostPlatform::LinuxX64),
        ("linux", "aarch64") if musl("aarch64") => Some(HostPlatform::LinuxArm64Musl),
        ("linux", "aarch64") => Some(HostPlatform::LinuxArm64),
        ("macos", "x86_64") => Some(HostPlatform::MacX64),
        ("macos", "aarch64") => Some(HostPlatform::MacArm64),
        _ => None,
    }
}

/// Now, RFC 3339 in UTC, whole seconds.
fn now() -> String {
    humanitl_core::time::rfc3339_secs(humanitl_core::time::from_system_time(
        std::time::SystemTime::now(),
    ))
}

#[cfg(test)]
mod tests;
