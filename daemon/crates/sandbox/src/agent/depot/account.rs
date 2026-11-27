//! Login and configuration of a depot agent as maintenance runs (ADR-0023
//! points 4 and 7, HUM-520).
//!
//! Both run the depot program in a sandbox without a project, like an
//! install ([`super::maintenance`]): a work directory of their own as `HOME`,
//! the program directory read-only, and a writable copy of the depot's
//! configuration and credential file as the agent's configuration directory
//! ([`super::Scratch`]). What happens to that copy afterwards differs:
//!
//! - **Login** ([`MaintenanceTask::Login`]): the adapter's login command
//!   ([`LoginRecipe`]), its login hosts as `allow` rules of this run alone.
//!   After the run only the credential file goes back, through the guard
//!   ([`super::credential::take_back`] with [`Origin::Login`]), and, where the
//!   adapter names them, single fields of another file
//!   ([`super::credential::take_named_fields`]). Every other change of the
//!   run is discarded with the copy.
//! - **Configuration** ([`MaintenanceTask::Config`]): the agent itself, so
//!   the person can change its settings. Afterwards the daemon lays the
//!   changed copy aside (`agents/.maintenance/<id>.config-pending/`) and
//!   lists what changed, with the keys that run commands highlighted
//!   ([`super::DepotRecipe::command_keys`]); `config/` changes only when the
//!   person confirms ([`accept_config`]). The credential file never goes
//!   this way; a renewal in the run goes through the guard like in any
//!   session.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::os::fd::AsFd as _;
use std::path::{Path, PathBuf};

use humanitl_config::Paths;
use humanitl_core::diagnostics::codes::{AGENT_008, AGENT_011, AGENT_014, AGENT_015, AGENT_019};
use humanitl_core::ids::{RuleId, SessionId};
use humanitl_core::rule::{Action, Expiry, HostPattern, Matcher, Rule};
use humanitl_core::{Diagnostic, FixAction, HostName, Scheme, Severity};
use serde_json::Value;

use super::credential::{self, Origin, Refused, TakeBack, Taken};
use super::maintenance::{self, MaintenanceTask};
use super::recipe::{DepotRecipe, KnownAgent, LoginFlow, LoginRecipe};
use super::scratch::{self, SCRATCH_CAP_BYTES, Scratch};
use super::seal::CredentialKeys;
use super::store::{Depot, Installed, Reservation};
use super::{DEPOT_CONFIG_DST, DepotMount};

/// The most entries the configuration copy may hold to be laid aside.
const CONFIG_ENTRIES_CAP: usize = 10_000;
/// The most changed key paths listed per file.
const KEYS_LISTED: usize = 50;
/// The directory of a pending configuration, beside the depots.
const PENDING_SUFFIX: &str = "config-pending";

/// Where a configuration run's copy is gathered before it becomes the
/// pending proposal with one `rename` (HUM-521): a restart in the middle of
/// the copy leaves only this, never a partial proposal.
const STAGING_SUFFIX: &str = "config-staging";

/// A prepared login or configuration run.
#[derive(Debug)]
pub struct AgentRun {
    agent: &'static KnownAgent,
    recipe: DepotRecipe,
    task: MaintenanceTask,
    session: SessionId,
    depot: Depot,
    installed: Installed,
    paths: Paths,
    work: PathBuf,
    copy: Scratch,
    /// The key of a sealed depot (HUM-523), for the copy and the guard.
    keys: Option<CredentialKeys>,
    /// One run or switch per agent at a time.
    _lock: Reservation,
}

/// What a finished login took over.
#[derive(Debug)]
pub struct LoginDone {
    /// The guard's answer: [`Taken::Stored`], or [`Taken::Nothing`] when the
    /// stored login is unchanged.
    pub taken: Taken,
    /// The named fields taken over.
    pub named: Vec<String>,
}

/// A failed login.
#[derive(Debug)]
pub struct LoginFailed {
    /// `AGENT_014` with the reason.
    pub found: Diagnostic,
    /// The guard's refusal, for the audit log, when it was the reason.
    pub refused: Option<Refused>,
    /// A credential file stored before a later step failed, for the audit
    /// log: every change of the stored file gets its record.
    pub stored: Option<Taken>,
    /// The earlier credential file put back after that failure (a login is
    /// all or nothing), for the audit log; `new` is empty when there was
    /// none before and the new file was removed.
    pub restored: Option<Taken>,
}

/// How one file of the configuration changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// New in the run.
    Added,
    /// Other content.
    Changed,
    /// Gone in the run.
    Removed,
}

impl Change {
    /// The word of the protocol.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Changed => "changed",
            Self::Removed => "removed",
        }
    }
}

/// One changed file of a configuration run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigChange {
    /// The path relative to the configuration directory.
    pub path: String,
    /// How it changed.
    pub change: Change,
    /// The changed key paths of a JSON file, dotted (`projects.a.mcpServers`):
    /// every one that runs a command first, never cut, then at most 50 others.
    pub keys: Vec<String>,
    /// Those of them that run commands, after the adapter's list: a path that
    /// ends at such a key, whatever changed below it. Never cut.
    pub command_keys: Vec<String>,
    /// How many other changed keys were left out of `keys`.
    pub omitted: usize,
}

/// What a configuration run laid aside for confirmation.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ConfigProposal {
    /// Every changed file.
    pub changes: Vec<ConfigChange>,
    /// What the guard said about the credential file of the run, if it
    /// changed.
    pub credential: Option<Result<Taken, Refused>>,
}

/// Prepares a login or configuration run of `agent`.
///
/// # Errors
///
/// `AGENT_015` for an agent without a depot recipe or a configuration run
/// of an agent whose configuration no variable moves, `AGENT_014` for a login
/// the adapter cannot do yet (no login run, no credential format, an API key
/// only), `AGENT_008` without an installed version, `AGENT_013` for a depot
/// that fails its checks, `AGENT_011` while another run of the agent is
/// prepared.
pub fn prepare(
    paths: &Paths,
    agent: &'static KnownAgent,
    task: MaintenanceTask,
    session: SessionId,
    keys: Option<CredentialKeys>,
) -> Result<AgentRun, Diagnostic> {
    prepare_in(
        paths,
        agent,
        task,
        session,
        Depot::of(paths, agent.id),
        keys,
    )
}

pub(crate) fn prepare_in(
    paths: &Paths,
    agent: &'static KnownAgent,
    task: MaintenanceTask,
    session: SessionId,
    depot: Depot,
    keys: Option<CredentialKeys>,
) -> Result<AgentRun, Diagnostic> {
    let recipe = agent.recipe.ok_or_else(|| {
        Diagnostic::builder(AGENT_015, Severity::Error)
            .why(format!(
                "the adapter of {} brings no recipe for the depot yet",
                agent.display_name
            ))
            .build()
    })?;
    if !task.runs_agent() {
        return Err(Diagnostic::builder(AGENT_015, Severity::Error)
            .why(format!("a {} run is no run of the agent", task.as_str()))
            .build());
    }
    if task == MaintenanceTask::Login {
        login_possible(agent, &recipe)?;
    }
    if task == MaintenanceTask::Config && recipe.config_env.is_none() {
        // Nothing points the agent at the copy (`agy`, HUM-517): what the
        // run wrote would never be read by a session.
        return Err(Diagnostic::builder(AGENT_015, Severity::Error)
            .why(format!(
                "{} keeps its configuration in the session's own home; no variable points it \
                 at a copy from the depot, so a configuration run would change nothing a \
                 session reads",
                agent.display_name
            ))
            .fix(FixAction::CopyCommand(format!(
                "humanitl agents status {}",
                agent.id
            )))
            .build());
    }
    if task == MaintenanceTask::Config
        && let Some(reason) = recipe.config_refused
    {
        return Err(Diagnostic::builder(AGENT_015, Severity::Error)
            .why(format!(
                "{} has no configuration run: {reason}",
                agent.display_name
            ))
            .fix(FixAction::CopyCommand(format!(
                "humanitl agents status {}",
                agent.id
            )))
            .build());
    }
    let installed = depot.installed().map_err(|found| {
        if found.code == AGENT_008 {
            Diagnostic::builder(AGENT_008, Severity::Error)
                .why(format!(
                    "the depot holds no version of {} to {} with",
                    agent.display_name,
                    if task == MaintenanceTask::Login {
                        "log in"
                    } else {
                        "configure"
                    }
                ))
                .fix(FixAction::CopyCommand(format!(
                    "humanitl agents install {}",
                    agent.id
                )))
                .build()
        } else {
            found
        }
    })?;
    let lock = maintenance::maintenance_lock(&depot)?;
    // Ein Rest aus einem abgebrochenen Konfigurationslauf (HUM-521): unter
    // der Wartungssperre läuft keine Kopie dorthin, er kann weg.
    let _ = std::fs::remove_dir_all(staging_dir(paths, agent.id));
    let reservation = depot.reserve_for_session()?;
    let work = maintenance::work_dir(paths, agent.id, session);
    let _ = std::fs::remove_dir_all(&work);
    for dir in [work.parent().map(Path::to_path_buf), Some(work.clone())]
        .into_iter()
        .flatten()
    {
        super::adopt::make_dir(&dir).map_err(|err| {
            Diagnostic::builder(AGENT_011, Severity::Error)
                .why(format!(
                    "the work directory {} of the run cannot be created ({err})",
                    dir.display()
                ))
                .build()
        })?;
    }
    let copy = scratch::create(paths, session, &depot, Some(reservation), keys.as_ref())?;
    Ok(AgentRun {
        agent,
        recipe,
        task,
        session,
        depot,
        installed,
        paths: paths.clone(),
        work,
        copy,
        keys,
        _lock: lock,
    })
}

/// Whether the adapter of `agent` declares everything a login run needs; the
/// status offers `login` only then (HUM-521).
#[must_use]
pub fn login_declared(agent: &KnownAgent) -> bool {
    agent
        .recipe
        .as_ref()
        .is_some_and(|recipe| login_possible(agent, recipe).is_ok())
}

/// Whether the adapter can log in at all, `AGENT_014` otherwise.
fn login_possible(agent: &KnownAgent, recipe: &DepotRecipe) -> Result<(), Diagnostic> {
    let why = if recipe.login == LoginFlow::ApiKeyOnly {
        Some(format!(
            "{} takes an API key from Humanitl's keyring in the depot; there is no login to \
             store, and the host's keyring and session bus never reach a sandbox",
            agent.display_name
        ))
    } else if recipe.credential_file.is_none() || recipe.credential_format.is_none() {
        Some(format!(
            "the adapter of {} declares no format of its credential file yet, so the guard \
             would take nothing back; nothing is guessed",
            agent.display_name
        ))
    } else if recipe.login_run.is_none() {
        Some(format!(
            "the adapter of {} declares no login run yet",
            agent.display_name
        ))
    } else {
        None
    };
    // An agent that only takes a key gets the command that stores one;
    // every other refusal points at the status.
    let fix = if recipe.login == LoginFlow::ApiKeyOnly {
        "humanitl llm key set".to_owned()
    } else {
        format!("humanitl agents status {}", agent.id)
    };
    match why {
        None => Ok(()),
        Some(why) => Err(Diagnostic::builder(AGENT_014, Severity::Error)
            .why(why)
            .fix(FixAction::CopyCommand(fix))
            .build()),
    }
}

impl AgentRun {
    /// The agent.
    #[must_use]
    pub fn agent(&self) -> &'static KnownAgent {
        self.agent
    }

    /// Login or configuration.
    #[must_use]
    pub fn task(&self) -> MaintenanceTask {
        self.task
    }

    /// The work directory on the host, the run's `HOME` at
    /// [`maintenance::MAINTENANCE_HOME_DST`].
    #[must_use]
    pub fn home(&self) -> &Path {
        &self.work
    }

    /// The login recipe; `None` for a configuration run.
    #[must_use]
    pub fn login(&self) -> Option<&LoginRecipe> {
        match self.task {
            MaintenanceTask::Login => self.recipe.login_run.as_ref(),
            _ => None,
        }
    }

    /// The mount of the depot program and the configuration copy.
    #[must_use]
    pub fn mount(&self) -> DepotMount {
        let mut env: Vec<(String, String)> = self
            .recipe
            .config_env
            .map(|key| (key.to_owned(), DEPOT_CONFIG_DST.to_owned()))
            .into_iter()
            .collect();
        env.extend(
            self.recipe
                .update_off
                .iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned())),
        );
        DepotMount {
            agent: self.agent.id.to_owned(),
            version: self.installed.version.clone(),
            program_dir: self.installed.dir.clone(),
            program: self.recipe.program.to_owned(),
            scratch: Some(self.copy.path().to_path_buf()),
            env,
        }
    }

    /// The command: the depot program, with the login words for a login.
    #[must_use]
    pub fn command(&self) -> Vec<OsString> {
        let mut command = vec![self.mount().program_dst().into_os_string()];
        if let Some(login) = self.login() {
            command.extend(login.args.iter().map(OsString::from));
        }
        command
    }

    /// The variables of the run besides the mount's: `HOME`.
    #[must_use]
    pub fn env(&self) -> Vec<(String, String)> {
        vec![(
            "HOME".to_owned(),
            maintenance::MAINTENANCE_HOME_DST.to_owned(),
        )]
    }

    /// The `allow` rules of a login: one per login host, HTTPS on port 443,
    /// every method (a login posts), valid only for this session. None for a
    /// configuration run: the agent's traffic is held like in any session.
    #[must_use]
    pub fn session_rules(&self) -> Vec<Rule> {
        let Some(login) = self.login() else {
            return Vec::new();
        };
        login
            .hosts
            .iter()
            .filter_map(|host| {
                let name = HostName::parse(host.host).ok()?;
                Some(
                    Rule::new(
                        RuleId::new(),
                        Action::Allow,
                        Matcher::host(HostPattern::Exact(name))
                            .with_scheme(Scheme::Https)
                            .with_port(443),
                    )
                    .with_expiry(Expiry::Session(self.session))
                    .with_note(format!(
                        "Login of {} into the depot, this run only. Source: {}",
                        self.agent.display_name, host.evidence
                    )),
                )
            })
            .collect()
    }

    /// Takes the login over: the credential file through the guard, then the
    /// named fields. Every other change of the run goes with the copy.
    ///
    /// `succeeded` is whether the login command ended with status 0 and the
    /// guarantees held; `own_loopback` whether the backend gives the sandbox
    /// a loopback of its own (for the reason of a failure).
    ///
    /// # Errors
    ///
    /// `AGENT_014` for a failed login, no credential file, or a file the
    /// guard refused (then with [`LoginFailed::refused`]).
    pub fn commit_login(
        self,
        succeeded: bool,
        own_loopback: bool,
    ) -> Result<LoginDone, LoginFailed> {
        let agent = self.agent;
        let failed = |why: String| LoginFailed {
            found: login_failed(agent, &self.recipe, &why, own_loopback),
            refused: None,
            stored: None,
            restored: None,
        };
        if !succeeded {
            return Err(failed(
                "the login ended with an error; its output in the terminal names the cause"
                    .to_owned(),
            ));
        }
        let (Some(file), Some(format)) =
            (self.recipe.credential_file, self.recipe.credential_format)
        else {
            return Err(failed(
                "the adapter declares no credential format".to_owned(),
            ));
        };
        // Everything is checked before anything is written: named fields
        // out of their form leave the stored login as it was.
        let named = self.recipe.login_run.and_then(|login| login.named);
        let update = match &named {
            Some(named) => credential::prepare_named_fields(self.copy.path(), &self.depot, named)
                .map_err(|found| failed(found.why))?,
            None => None,
        };
        let request = TakeBack {
            depot: &self.depot,
            file,
            format: &format,
            session: self.session,
            base: self.copy.base(file),
            origin: Origin::Login,
            keys: self.keys.as_ref(),
        };
        // One lock across storing, writing the named fields and, should the
        // latter fail, putting the earlier login back: all or nothing.
        let held = credential::lock(&self.depot).map_err(|found| failed(found.why))?;
        let before = credential::snapshot(&self.depot, file, self.keys.as_ref(), &held)
            .map_err(|found| failed(found.why))?;
        let taken = match credential::take_back_held(self.copy.path(), &request, &held) {
            Ok(Taken::Nothing) if !self.depot.logged_in() => {
                return Err(failed(format!(
                    "the login left no credential file {file} in its configuration directory"
                )));
            }
            Ok(taken) => taken,
            Err(refused) => {
                return Err(LoginFailed {
                    found: login_failed(
                        agent,
                        &self.recipe,
                        &format!(
                            "the guard refused its credential file: {}",
                            refused.found.why
                        ),
                        own_loopback,
                    ),
                    refused: Some(refused),
                    stored: None,
                    restored: None,
                });
            }
        };
        let named = match (named, update) {
            (Some(named), Some(update)) => {
                match credential::write_named_fields(&self.depot, &named, &update, &held) {
                    Ok(keys) => keys,
                    Err(found) => {
                        return Err(self.undo(&taken, &before, &held, &found.why, own_loopback));
                    }
                }
            }
            _ => Vec::new(),
        };
        Ok(LoginDone { taken, named })
    }

    /// After the named fields could not be written: the credential file this
    /// login stored is replaced by the earlier one again, so no later session
    /// gets the new login without its account fields.
    fn undo(
        &self,
        taken: &Taken,
        before: &credential::Snapshot,
        held: &credential::CredentialLock,
        why: &str,
        own_loopback: bool,
    ) -> LoginFailed {
        let Taken::Stored { new, .. } = taken else {
            return LoginFailed {
                found: login_failed(self.agent, &self.recipe, why, own_loopback),
                refused: None,
                stored: None,
                restored: None,
            };
        };
        let file = self.recipe.credential_file.unwrap_or_default();
        match credential::restore(&self.depot, file, before, held) {
            Ok(()) => LoginFailed {
                found: login_failed(
                    self.agent,
                    &self.recipe,
                    &format!("{why}; the earlier login was put back"),
                    own_loopback,
                ),
                refused: None,
                stored: Some(taken.clone()),
                restored: Some(Taken::Stored {
                    old: Some(new.clone()),
                    new: before.sha256().unwrap_or_default(),
                    account: None,
                    warning: None,
                }),
            },
            Err(what) => LoginFailed {
                found: login_failed(
                    self.agent,
                    &self.recipe,
                    &format!(
                        "{why}; putting the earlier login back failed too ({}), so the new \
                         credential file stays without its account fields",
                        what.why
                    ),
                    own_loopback,
                ),
                refused: None,
                stored: Some(taken.clone()),
                restored: None,
            },
        }
    }

    /// Lays the run's configuration aside for confirmation and lists what
    /// changed against `config/` of the depot. The credential file is not
    /// part of it; a renewal of it goes through the guard.
    ///
    /// # Errors
    ///
    /// `AGENT_019` for a run that failed, a copy with something else than
    /// regular files and directories, above its caps, or one that cannot be
    /// laid aside.
    pub fn finish_config(self, succeeded: bool) -> Result<ConfigProposal, Diagnostic> {
        let agent = self.agent.id;
        if !succeeded {
            return Err(config_failed(
                agent,
                "the run ended with an error; nothing of it is kept",
            ));
        }
        let credential = match (self.recipe.credential_file, self.recipe.credential_format) {
            (Some(file), Some(format)) => {
                let request = TakeBack {
                    depot: &self.depot,
                    file,
                    format: &format,
                    session: self.session,
                    base: self.copy.base(file),
                    origin: Origin::Session,
                    keys: self.keys.as_ref(),
                };
                match credential::take_back(self.copy.path(), &request) {
                    Ok(Taken::Nothing) => None,
                    other => Some(other),
                }
            }
            _ => None,
        };
        let staged = self.stage_config()?;
        let changes = diff(&self.depot.config_dir(), &staged, self.recipe.command_keys);
        if changes.is_empty() {
            let _ = std::fs::remove_dir_all(&staged);
            let _ = std::fs::remove_dir_all(pending_dir(&self.paths, agent));
        } else {
            publish_config(&self.paths, agent, &staged)?;
        }
        Ok(ConfigProposal {
            changes,
            credential,
        })
    }

    /// Copies the run's copy into the staging place, never into the pending
    /// proposal itself; [`publish_config`] makes it the proposal afterwards.
    /// Returns the staging directory.
    pub(crate) fn stage_config(&self) -> Result<PathBuf, Diagnostic> {
        let agent = self.agent.id;
        let staged = staging_dir(&self.paths, agent);
        let _ = std::fs::remove_dir_all(&staged);
        let mut budget = (SCRATCH_CAP_BYTES, CONFIG_ENTRIES_CAP);
        let skip = self.recipe.credential_file;
        copy_config(agent, self.copy.path(), &staged, skip, &mut budget).inspect_err(|_| {
            let _ = std::fs::remove_dir_all(&staged);
        })?;
        Ok(staged)
    }
}

/// Makes the complete copy in `staged` the pending proposal of `agent`: the
/// tree is synced, an older proposal goes, and one `rename` puts the new
/// one in place, so `pending` never sees a partial proposal (HUM-521).
fn publish_config(paths: &Paths, agent: &str, staged: &Path) -> Result<(), Diagnostic> {
    let pending = pending_dir(paths, agent);
    let failed = |what: &str, err: std::io::Error| {
        let _ = std::fs::remove_dir_all(staged);
        config_failed(agent, &format!("the proposal {what} ({err})"))
    };
    super::adopt::sync_dir(staged).map_err(|err| failed("cannot be synced", err))?;
    let _ = std::fs::remove_dir_all(&pending);
    std::fs::rename(staged, &pending).map_err(|err| failed("cannot be put in place", err))?;
    if let Some(parent) = pending.parent() {
        super::adopt::sync_dir(parent).map_err(|err| failed("cannot be synced", err))?;
    }
    Ok(())
}

impl Drop for AgentRun {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.work);
    }
}

/// `AGENT_014` for a failed login. Where the backend shares the host's
/// loopback and the adapter's callback cannot be delivered, the device flow
/// is the only way, and the sentence names where the vendor turns it on.
#[must_use]
pub fn login_failed(
    agent: &KnownAgent,
    recipe: &DepotRecipe,
    why: &str,
    own_loopback: bool,
) -> Diagnostic {
    let device_only = !own_loopback
        && recipe
            .login_run
            .is_some_and(|login| login.callback.is_some());
    let setting = recipe
        .login_run
        .and_then(|login| login.device_flow_setting)
        .filter(|_| device_only);
    let device = setting.map_or_else(String::new, |setting| {
        format!(
            "; on this backend the sandbox shares the host's loopback, so only the device flow \
             works, and it may be off at the vendor: turn on \"{setting}\""
        )
    });
    let text = format!(
        "the login of {} was not stored: {why}{device}",
        agent.display_name
    );
    Diagnostic::builder(AGENT_014, Severity::Error)
        .why(text)
        .fix(FixAction::CopyCommand(format!(
            "humanitl agents login {}",
            agent.id
        )))
        .build()
}

/// `agents/.maintenance/<id>.config-staging/`, where a configuration run's
/// copy is gathered (HUM-521).
#[must_use]
pub fn staging_dir(paths: &Paths, agent: &str) -> PathBuf {
    let data = std::fs::canonicalize(paths.data_dir()).unwrap_or_else(|_| paths.data_dir());
    data.join("agents")
        .join(".maintenance")
        .join(format!("{agent}.{STAGING_SUFFIX}"))
}

/// `agents/.maintenance/<id>.config-pending/`.
#[must_use]
pub fn pending_dir(paths: &Paths, agent: &str) -> PathBuf {
    let data = std::fs::canonicalize(paths.data_dir()).unwrap_or_else(|_| paths.data_dir());
    data.join("agents")
        .join(".maintenance")
        .join(format!("{agent}.{PENDING_SUFFIX}"))
}

/// The changes a pending configuration holds against `config/`; `None`
/// without one.
#[must_use]
pub fn pending(paths: &Paths, agent: &KnownAgent) -> Option<Vec<ConfigChange>> {
    let dir = pending_dir(paths, agent.id);
    std::fs::symlink_metadata(&dir).ok()?.is_dir().then(|| {
        let keys = agent.recipe.map_or(&[][..], |recipe| recipe.command_keys);
        diff(&Depot::of(paths, agent.id).config_dir(), &dir, keys)
    })
}

/// Takes a pending configuration over, after the person confirmed it:
/// `config/` is replaced with one `rename`, under the agent's maintenance
/// lock.
///
/// # Errors
///
/// `AGENT_019` without a pending configuration, while a run holds the lock,
/// or when the swap fails; `config/` is then as it was.
pub fn accept_config(paths: &Paths, agent: &str) -> Result<Vec<ConfigChange>, Diagnostic> {
    let depot = Depot::of(paths, agent);
    let _lock = maintenance::maintenance_lock(&depot)?;
    let _hold = depot.reserve_for_session()?;
    let pending = pending_dir(paths, agent);
    if !std::fs::symlink_metadata(&pending).is_ok_and(|meta| meta.is_dir()) {
        return Err(config_failed(
            agent,
            "no configuration run is waiting for a confirmation",
        ));
    }
    depot.check()?;
    let keys = super::recipe::known(agent)
        .and_then(|known| known.recipe)
        .map_or(&[][..], |recipe| recipe.command_keys);
    let changes = diff(&depot.config_dir(), &pending, keys);
    let config = depot.config_dir();
    let old = depot.root().join(".config-old");
    let _ = std::fs::remove_dir_all(&old);
    let had = std::fs::symlink_metadata(&config).is_ok();
    if had {
        std::fs::rename(&config, &old)
            .map_err(|err| config_failed(agent, &format!("config/ cannot be moved ({err})")))?;
    }
    if let Err(err) = std::fs::rename(&pending, &config) {
        if had {
            let _ = std::fs::rename(&old, &config);
        }
        return Err(config_failed(
            agent,
            &format!("the new configuration cannot be put in place ({err})"),
        ));
    }
    let _ = std::fs::remove_dir_all(&old);
    for dir in [config.as_path(), depot.root()] {
        super::adopt::sync_dir(dir).map_err(|err| {
            config_failed(
                agent,
                &format!("{} cannot be synced ({err})", dir.display()),
            )
        })?;
    }
    Ok(changes)
}

/// Discards a pending configuration; `config/` stays.
pub fn discard_config(paths: &Paths, agent: &str) {
    let _ = std::fs::remove_dir_all(pending_dir(paths, agent));
}

/// The files of `config/` as they are now, for `agents config <id> --show`:
/// every file as "added" against nothing, with the keys that run commands.
#[must_use]
pub fn show_config(paths: &Paths, agent: &KnownAgent) -> Vec<ConfigChange> {
    let keys = agent.recipe.map_or(&[][..], |recipe| recipe.command_keys);
    let empty = paths
        .data_dir()
        .join("agents")
        .join(".maintenance")
        .join(".none");
    diff(&empty, &Depot::of(paths, agent.id).config_dir(), keys)
}

/// Copies the run's copy into `to`: only regular files and directories, the
/// credential file at the top left out, within the caps. The walk goes by
/// descriptors and never follows a link (`O_NOFOLLOW` on every directory and
/// file it opens), so a process of the run that outlives it cannot swap an
/// entry for a link to a file of the host between a look and the read.
fn copy_config(
    agent: &str,
    from: &Path,
    to: &Path,
    skip: Option<&str>,
    budget: &mut (u64, usize),
) -> Result<(), Diagnostic> {
    let root = super::adopt::open_dir(from).map_err(|err| {
        config_failed(
            agent,
            &format!("{} cannot be opened ({err})", from.display()),
        )
    })?;
    copy_config_at(agent, root.as_fd(), to, Path::new(""), skip, budget)
}

fn copy_config_at(
    agent: &str,
    dir: std::os::fd::BorrowedFd<'_>,
    to: &Path,
    rel: &Path,
    skip: Option<&str>,
    budget: &mut (u64, usize),
) -> Result<(), Diagnostic> {
    use rustix::fs::{AtFlags, FileType, Mode, OFlags};
    use std::io::Read as _;

    let refuse =
        |child: &Path, what: &str| config_failed(agent, &format!("{} {what}", child.display()));
    super::adopt::make_dir(to).map_err(|err| refuse(rel, &format!("cannot be created ({err})")))?;
    let (names, _) = crate::worktree::dir_names(dir, CONFIG_ENTRIES_CAP + 1)
        .map_err(|err| refuse(rel, &format!("cannot be listed ({err})")))?;
    for name in names {
        // The credential file, at the top of the copy or one directory
        // deeper (OpenCode's `opencode/auth.json`, #110), never goes this way.
        if skip.is_some_and(|skip| rel.join(&name) == Path::new(skip)) {
            continue;
        }
        budget.1 = budget.1.checked_sub(1).ok_or_else(|| {
            config_failed(
                agent,
                &format!("the copy holds more than {CONFIG_ENTRIES_CAP} entries"),
            )
        })?;
        let child = rel.join(&name);
        let target = to.join(&name);
        let stat = rustix::fs::statat(dir, name.as_os_str(), AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|err| refuse(&child, &format!("cannot be read ({err})")))?;
        match FileType::from_raw_mode(stat.st_mode) {
            FileType::Directory => {
                let sub = super::adopt::open_dir_at(dir, &name)
                    .map_err(|err| refuse(&child, &format!("cannot be opened ({err})")))?;
                copy_config_at(agent, sub.as_fd(), &target, &child, skip, budget)?;
            }
            FileType::RegularFile => {
                let fd = rustix::fs::openat(
                    dir,
                    name.as_os_str(),
                    OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                    Mode::empty(),
                )
                .map_err(|err| refuse(&child, &format!("cannot be opened ({err})")))?;
                let opened = rustix::fs::fstat(&fd)
                    .map_err(|err| refuse(&child, &format!("cannot be read ({err})")))?;
                if FileType::from_raw_mode(opened.st_mode) != FileType::RegularFile {
                    return Err(refuse(&child, "changed while it was read"));
                }
                let size = u64::try_from(opened.st_size).unwrap_or(u64::MAX);
                budget.0 = budget.0.checked_sub(size).ok_or_else(|| {
                    config_failed(
                        agent,
                        &format!(
                            "the copy is larger than {} MiB",
                            SCRATCH_CAP_BYTES / (1024 * 1024)
                        ),
                    )
                })?;
                let mut bytes = Vec::new();
                std::fs::File::from(fd)
                    .take(size)
                    .read_to_end(&mut bytes)
                    .map_err(|err| refuse(&child, &format!("cannot be read ({err})")))?;
                write_private(&target, &bytes)
                    .map_err(|err| refuse(&child, &format!("cannot be written ({err})")))?;
            }
            _ => {
                return Err(refuse(
                    &child,
                    "is neither a regular file nor a directory; a configuration holds only those",
                ));
            }
        }
    }
    Ok(())
}

fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt as _;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// The regular files under `dir`, by their relative path, with their bytes.
fn files_of(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(meta) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            if meta.is_dir() {
                walk(root, &path, out);
            } else if meta.is_file()
                && meta.len() <= SCRATCH_CAP_BYTES
                && let (Ok(rel), Ok(bytes)) = (path.strip_prefix(root), std::fs::read(&path))
            {
                out.insert(rel.to_string_lossy().into_owned(), bytes);
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

/// What changed from `before` to `after`, file by file; for JSON files the
/// changed key paths, those under a command key highlighted.
fn diff(before: &Path, after: &Path, command_keys: &[&str]) -> Vec<ConfigChange> {
    let old = files_of(before);
    let new = files_of(after);
    let paths: BTreeSet<&String> = old.keys().chain(new.keys()).collect();
    let mut changes = Vec::new();
    for path in paths {
        let (change, keys) = match (old.get(path), new.get(path)) {
            (None, Some(bytes)) => (Change::Added, json_keys(None, Some(bytes), command_keys)),
            (Some(bytes), None) => (Change::Removed, json_keys(Some(bytes), None, command_keys)),
            (Some(a), Some(b)) if a != b => {
                (Change::Changed, json_keys(Some(a), Some(b), command_keys))
            }
            _ => continue,
        };
        let mut listed = keys.command.clone();
        listed.extend(keys.other);
        changes.push(ConfigChange {
            path: path.clone(),
            change,
            keys: listed,
            command_keys: keys.command,
            omitted: keys.omitted,
        });
    }
    changes
}

/// The changed keys of one JSON file.
#[derive(Debug, Default)]
struct KeyDiff {
    /// Paths that end at a key that runs commands; never cut.
    command: Vec<String>,
    /// The other changed paths, at most [`KEYS_LISTED`].
    other: Vec<String>,
    /// Other changed paths beyond them.
    omitted: usize,
}

/// The key paths that differ between two JSON documents; empty when either
/// is no JSON object. The keys that run commands are found over the whole
/// document before anything is cut.
fn json_keys(before: Option<&[u8]>, after: Option<&[u8]>, command_keys: &[&str]) -> KeyDiff {
    let parse = |bytes: Option<&[u8]>| -> Option<Value> {
        match bytes {
            None => Some(Value::Object(serde_json::Map::new())),
            Some(bytes) => serde_json::from_slice::<Value>(bytes)
                .ok()
                .filter(Value::is_object),
        }
    };
    let mut keys = KeyDiff::default();
    if let (Some(before), Some(after)) = (parse(before), parse(after)) {
        changed_keys(&before, &after, "", command_keys, &mut keys);
    }
    keys
}

fn changed_keys(before: &Value, after: &Value, at: &str, command_keys: &[&str], out: &mut KeyDiff) {
    walk_keys(Some(before), Some(after), at, command_keys, out);
}

/// The changes from `before` to `after` below `at`. Wherever either side is
/// an object or an array, the walk goes on into it, against nothing when the
/// other side is missing or a scalar: a key that runs commands is found
/// wherever it appears, also where an object replaced `null`.
fn walk_keys(
    before: Option<&Value>,
    after: Option<&Value>,
    at: &str,
    command_keys: &[&str],
    out: &mut KeyDiff,
) {
    if before == after {
        return;
    }
    let container = |value: Option<&Value>| {
        matches!(value, Some(Value::Object(map)) if !map.is_empty())
            || matches!(value, Some(Value::Array(items)) if !items.is_empty())
    };
    if !container(before) && !container(after) {
        out.push_other(at);
        return;
    }
    let found = (out.command.len(), out.other.len() + out.omitted);
    let (a, b) = (
        before.and_then(Value::as_object),
        after.and_then(Value::as_object),
    );
    if a.is_some() || b.is_some() {
        let keys: BTreeSet<&String> = a
            .into_iter()
            .flat_map(serde_json::Map::keys)
            .chain(b.into_iter().flat_map(serde_json::Map::keys))
            .collect();
        for key in keys {
            let path = if at.is_empty() {
                key.clone()
            } else {
                format!("{at}.{key}")
            };
            let (x, y) = (a.and_then(|a| a.get(key)), b.and_then(|b| b.get(key)));
            if x == y {
                continue;
            }
            if command_keys.contains(&key.as_str()) {
                out.command.push(path);
            } else {
                walk_keys(x, y, &path, command_keys, out);
            }
        }
    }
    let (a, b) = (
        before.and_then(Value::as_array),
        after.and_then(Value::as_array),
    );
    if a.is_some() || b.is_some() {
        let len = a.map_or(0, Vec::len).max(b.map_or(0, Vec::len));
        for index in 0..len {
            let (x, y) = (a.and_then(|a| a.get(index)), b.and_then(|b| b.get(index)));
            walk_keys(x, y, &format!("{at}[{index}]"), command_keys, out);
        }
    }
    // A container that changed shape without a listed key below it, such as
    // `null` replaced by `{}`: the path itself.
    if (out.command.len(), out.other.len() + out.omitted) == found {
        out.push_other(at);
    }
}

impl KeyDiff {
    fn push_other(&mut self, path: &str) {
        if self.other.len() < KEYS_LISTED {
            self.other.push(path.to_owned());
        } else {
            self.omitted += 1;
        }
    }
}

fn config_failed(agent: &str, what: &str) -> Diagnostic {
    Diagnostic::builder(AGENT_019, Severity::Error)
        .why(format!(
            "the configuration run of {agent} was not taken over: {what}; config/ of the depot \
             is as it was"
        ))
        .fix(FixAction::CopyCommand(format!(
            "humanitl agents config {agent}"
        )))
        .build()
}

#[cfg(test)]
mod tests;
