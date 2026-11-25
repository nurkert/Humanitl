//! Die Wartungssitzung des Agent-Depots (HUM-563, ADR-0023 Punkt 5).
//!
//! `Agents.install` und `Agents.update` starten hier einen Wartungslauf: eine
//! Sandbox wie jede andere dieses Dienstes, mit Terminal, den drei Garantien
//! und dem Proxy, aber ohne Projekt. Den Lauf bereitet
//! [`humanitl_sandbox::agent::depot::maintenance`] vor; dieses Modul gibt ihm
//! die Sitzung:
//!
//! - **Kein `/work`.** Der Arbeitsordner des Laufs steht an der Stelle des
//!   Projekts, aber unter [`MAINTENANCE_HOME_DST`], und ist sein `HOME`. Das
//!   Sandbox-Profil der Konfiguration gilt ohne seine Erweiterungen
//!   (`mounts.extra_ro`, `mounts.extra_rw`): Der Lauf sieht nichts vom Host
//!   außer dem, was jede Sitzung sieht. Die Host-Installationen des Agenten
//!   sind verdeckt, damit der Lauf kein Programm des Hosts übernimmt. Der
//!   Wächter des Depots prüft diese Form in jedem Backend
//!   ([`humanitl_sandbox::agent::depot::guard`]).
//! - **Die Regeln des Laufs.** Die `allow`-Regeln für die Hosts des Rezepts
//!   gehen als Sitzungsregeln (`Expiry::Session`) in den Regelspeicher und
//!   nach dem Ende wieder hinaus; die gespeicherten Regeln berührt der Lauf
//!   nie. Die mitgelieferte Gruppe ist für die Dauer des Laufs nur
//!   `rules/default.yaml`, ohne Durchreiche zum Sprachmodell und ohne Regeln
//!   eines Profils: Ohne Halt gehen nur die Hosts des Rezepts hinaus (und was
//!   der Mensch selbst als Regel hinterlegt hat).
//! - **Kein Briefing, kein Adapter.** Das Kommando ist `/bin/sh -c` mit dem
//!   Skript des Rezepts; kein Agent-Adapter trägt etwas bei.
//! - **Nach dem Ende** übernimmt `MaintenanceRun::commit` das Ergebnis, nur
//!   wenn der Lauf mit 0 endete und die Garantien grün waren. Das Ergebnis
//!   steht danach in [`Inner::maintenance`] und in `Agents.status`.
//!
//! Der Dienst hält eine Sandbox: Ein Lauf braucht den Anspruch auf den Start
//! wie jede Sitzung und wird abgelehnt, solange eine andere läuft.
//!
//! Anmeldung und Konfigurationslauf (HUM-520) sind Wartungsläufe derselben
//! Art, die statt eines Rezepts das Programm des Depots starten; was für sie
//! anders ist, steht in [`super::account`].

use std::sync::Arc;

use humanitl_config::{Config, SandboxMode, WorkMode};
use humanitl_core::diagnostics::codes;
use humanitl_core::ids::RuleId;
use humanitl_core::{Diagnostic, Severity};
use humanitl_sandbox::agent::depot::maintenance::{
    self as depot_maintenance, MAINTENANCE_HOME_DST,
};
use humanitl_sandbox::agent::depot::{
    Committed, DepotSession, KnownAgent, Maintenance, MaintenanceRequest, MaintenanceRun,
    MaintenanceTask, ReservedAgent,
};
use humanitl_sandbox::{LaunchOptions, MountPolicy, SandboxProfile, SessionContext};
use tokio::sync::mpsc;

use super::{
    BackendPick, CA_BUNDLE_FILE, ENV_SESSION, EVENT_BUFFER, Inner, Prepared, SandboxService,
    SessionState, already_running, joined_failed, leaving_daemon, lock, shim_path,
};
use crate::session::{SessionRequest, bundled_rules};
use crate::{convert, v1};

/// `running`: der Lauf geht.
pub const STATE_RUNNING: &str = "running";
/// `done`: das Ergebnis ist übernommen, `current` nennt die Version.
pub const STATE_DONE: &str = "done";
/// `failed`: nichts übernommen, das Depot ist wie vorher.
pub const STATE_FAILED: &str = "failed";
/// `confirm`: ein Konfigurationslauf wartet auf die Bestätigung (HUM-520).
pub const STATE_CONFIRM: &str = "confirm";

impl SandboxService {
    /// Startet eine Installation oder ein Update von `agent` (HUM-563).
    ///
    /// Die Antwort kommt, sobald der Lauf steht (`state = running` mit der
    /// Kennung seiner Sandbox), oder sofort, wenn keiner nötig ist: Die
    /// verlangte Version liegt schon im Depot (`done`, ohne Sandbox).
    ///
    /// # Errors
    ///
    /// `AGENT_015` für einen Agenten ohne Rezept oder ein Backend, das keinen
    /// Lauf trägt; `CLI_005`, solange eine Sitzung läuft; `IPC_006`, wenn der
    /// Daemon endet; die Befunde von `prepare` (`AGENT_008`, `AGENT_011` mit
    /// dem Schritt `pin`, `AGENT_013`) und des Starts.
    pub async fn maintain(
        &self,
        agent: &str,
        task: MaintenanceTask,
        version: Option<String>,
    ) -> Result<v1::AgentMaintenance, Diagnostic> {
        Arc::clone(&self.inner).maintain(agent, task, version).await
    }

    /// Der letzte Wartungslauf je Agent in diesem Daemon.
    #[must_use]
    pub fn maintenance_runs(&self) -> std::collections::BTreeMap<String, v1::AgentMaintenance> {
        lock(&self.inner.maintenance).clone()
    }
}

/// Was nach dem Ende eines Laufs übernommen wird.
pub(super) enum Job {
    /// Installation oder Update (HUM-563).
    Install(Box<MaintenanceRun>),
    /// Anmeldung oder Konfiguration (HUM-520).
    Agent(Box<humanitl_sandbox::agent::depot::account::AgentRun>),
}

impl Job {
    fn agent(&self) -> &'static KnownAgent {
        match self {
            Self::Install(run) => run.agent(),
            Self::Agent(run) => run.agent(),
        }
    }
}

/// Ein vorbereiteter Lauf samt Sitzung, bereit zum Start.
pub(super) struct Staged {
    pub(super) job: Job,
    pub(super) prepared: Prepared,
    /// Die Sitzungsregeln des Laufs, schon im Regelspeicher.
    pub(super) rules: Vec<RuleId>,
}

impl Inner {
    async fn maintain(
        self: Arc<Self>,
        agent: &str,
        task: MaintenanceTask,
        version: Option<String>,
    ) -> Result<v1::AgentMaintenance, Diagnostic> {
        let entry = self
            .depot_agents
            .iter()
            .find(|known| known.id == agent.trim())
            .ok_or_else(|| {
                Diagnostic::builder(codes::AGENT_015, Severity::Error)
                    .why(format!("{agent:?} is no agent of the depot"))
                    .build()
            })?;
        let Some(claim) = self.claim_start() else {
            return Err(if self.is_leaving() {
                leaving_daemon()
            } else {
                already_running(&self.running_facts())
            });
        };
        let this = Arc::clone(&self);
        let staged = tokio::task::spawn_blocking(move || {
            if task.runs_agent() {
                this.stage_agent(entry, task).map(Ok)
            } else {
                this.stage(entry, task, version.as_deref())
            }
        })
        .await
        .map_err(|error| joined_failed(&error))??;
        let staged = match staged {
            Ok(staged) => staged,
            // No run: the version lay in the depot already.
            Err(outcome) => {
                lock(&self.maintenance).insert(entry.id.to_owned(), outcome.clone());
                return Ok(outcome);
            }
        };
        let (output_tx, output_rx) = std::sync::mpsc::channel();
        // A login's output passes the reader of its URLs on the way
        // (HUM-520).
        let (launch_tx, urls, callback_paste) = self.login_output(&staged, output_tx);
        let this = Arc::clone(&self);
        let launched = tokio::task::spawn_blocking(move || {
            let result = this.launch_with(
                &staged.prepared,
                &v1::sandbox_request::Plan::default(),
                None,
                None,
                Some(launch_tx),
            );
            (staged, result)
        })
        .await
        .map_err(|error| joined_failed(&error))?;
        let (staged, result) = launched;
        let (launched, _handle) = match result {
            Ok(launched) => launched,
            Err(diagnostic) => {
                self.unwind_rules(&staged);
                return Err(diagnostic);
            }
        };
        let outcome = v1::AgentMaintenance {
            sandbox_id: launched.sandbox.to_string(),
            task: task.as_str().to_owned(),
            state: STATE_RUNNING.to_owned(),
            callback_paste,
            ..v1::AgentMaintenance::default()
        };
        {
            let mut runs = lock(&self.maintenance);
            let mut entry_now = outcome.clone();
            if let Some(urls) = &urls {
                entry_now.login_urls.clone_from(&lock(urls));
            }
            runs.insert(entry.id.to_owned(), entry_now);
        }
        tracing::info!(
            session = %self.session,
            sandbox = %launched.sandbox,
            agent = entry.id,
            task = task.as_str(),
            "a maintenance run of the agent depot started"
        );
        let this = Arc::clone(&self);
        let warnings = launched.warnings;
        let answer = outcome.clone();
        tokio::spawn(async move {
            // The claim lives until the run is taken over: no other start
            // gets between the end and the takeover.
            let _claim = claim;
            // On the heap: the run carries a plan and its session.
            Box::pin(this.accompany_run(staged, outcome, warnings, output_rx)).await;
        });
        Ok(answer)
    }

    /// Bereitet Lauf und Sitzung vor. `Err(outcome)` im inneren Ergebnis,
    /// wenn kein Lauf nötig war.
    fn stage(
        &self,
        entry: &'static KnownAgent,
        task: MaintenanceTask,
        version: Option<&str>,
    ) -> Result<Result<Staged, v1::AgentMaintenance>, Diagnostic> {
        let request = MaintenanceRequest {
            paths: &self.paths,
            agent: entry.id,
            task,
            version,
            session: self.session,
        };
        let run = match depot_maintenance::prepare_known(&request, entry)? {
            Maintenance::Run(run) => run,
            Maintenance::Switch { version } => {
                let switched = depot_maintenance::switch(&self.paths, entry.id, &version)?;
                return Ok(Err(done(task, "", &switched)));
            }
            Maintenance::Current { version } => {
                return Ok(Err(v1::AgentMaintenance {
                    task: task.as_str().to_owned(),
                    state: STATE_DONE.to_owned(),
                    previous: version.clone(),
                    version,
                    ..v1::AgentMaintenance::default()
                }));
            }
        };
        let config = self
            .resolver
            .resolve(&SessionRequest::default())
            .map(|resolved| resolved.config)?;
        let prepared = self.maintenance_session(&config, &run)?;
        let rules = self.apply_run_rules(&config, run.session_rules())?;
        Ok(Ok(Staged {
            job: Job::Install(run),
            prepared,
            rules,
        }))
    }

    /// Die Sitzung des Laufs: das Sandbox-Profil der Konfiguration ohne
    /// Erweiterungen, der Arbeitsordner an [`MAINTENANCE_HOME_DST`] statt
    /// eines Projekts, die Host-Installationen des Agenten verdeckt.
    fn maintenance_session(
        &self,
        config: &Config,
        run: &MaintenanceRun,
    ) -> Result<Prepared, Diagnostic> {
        self.run_session(
            config,
            &RunShape {
                home: run.home(),
                agent: run.agent(),
                command: run.command(),
                env: run.env(),
                mount: None,
            },
        )
    }

    /// Die Sitzung eines Laufs: das Sandbox-Profil der Konfiguration ohne
    /// Erweiterungen, der Arbeitsordner an [`MAINTENANCE_HOME_DST`] statt
    /// eines Projekts, die Host-Installationen des Agenten verdeckt; für
    /// Anmeldung und Konfiguration (HUM-520) dazu das Programm des Depots
    /// und die Kopie der Konfiguration.
    pub(super) fn run_session(
        &self,
        config: &Config,
        shape: &RunShape<'_>,
    ) -> Result<Prepared, Diagnostic> {
        let policy = MountPolicy::from_paths(&self.paths);
        let (profile_path, _) = self.profile_path(&config.sandbox.profile)?;
        let mut profile = SandboxProfile::load_validated(&profile_path, &policy)?;
        profile.mounts.work.dst = MAINTENANCE_HOME_DST.into();
        profile.mounts.work.mode = Some(WorkMode::Rw);
        profile.mounts.extra_ro.clear();
        profile.mounts.extra_rw.clear();
        let home = std::fs::canonicalize(shape.home).map_err(|err| {
            Diagnostic::builder(codes::AGENT_011, Severity::Error)
                .why(format!(
                    "the work directory {} of the maintenance run cannot be resolved ({err})",
                    shape.home.display()
                ))
                .build()
        })?;
        let agent = shape.agent;
        let mut session_env = vec![(ENV_SESSION.to_owned(), self.session.to_string())];
        session_env.extend(shape.env.iter().cloned());
        let session = SessionContext {
            session: self.session,
            work_src: home.clone(),
            work_mode: WorkMode::Rw,
            proxy_socket_src: self.paths.proxy_socket(),
            ca_cert_src: self.paths.ca_cert_path(),
            ca_bundle_src: self.paths.ca_dir().join(CA_BUNDLE_FILE),
            shim_src: shim_path(),
            session_env,
            command: shape.command.clone(),
            files: Vec::new(),
            agent_program: None,
            agent_depot: DepotSession {
                reserved: vec![ReservedAgent {
                    agent: agent.id.to_owned(),
                    program: agent.program.to_owned(),
                    host_installs: agent.host_installs.iter().map(Into::into).collect(),
                    locked_by: None,
                }],
                maintenance_home: Some(home),
                maintenance_program: shape.mount.as_ref().map(|mount| mount.agent.clone()),
                mount: shape.mount.clone(),
                ..DepotSession::default()
            },
        };
        let backend = self.pick_backend(config, &profile, &BackendPick::Detect)?;
        Ok(Prepared {
            profile,
            session,
            backend,
            options: LaunchOptions {
                notices: self.sandbox_notices.clone(),
                ..LaunchOptions::default()
            },
            depot_scratch: std::sync::Mutex::new(None),
            env_origin: std::collections::BTreeMap::new(),
            warnings: Vec::new(),
            overlay_preview: None,
            wanted_mode: SandboxMode::Direct,
            // A maintenance run needs no model server (GitHub #112).
            refusal: None,
            plan_mode: humanitl_sandbox::ModeChoice {
                mode: SandboxMode::Direct,
                notice: None,
            },
        })
    }

    /// Die Regeln des Laufs gelten lassen: die Sitzungsregeln für die Hosts
    /// des Rezepts dazu, die mitgelieferte Gruppe nur `rules/default.yaml`,
    /// Frage-Modus und Frist aus der Konfiguration ohne Sprachmodell. Zurück
    /// kommen die Kennungen der Sitzungsregeln.
    ///
    /// Die mitgelieferte Gruppe bleibt nach dem Lauf so: Der Dienst hält eine
    /// Sandbox, und der nächste Start setzt die Gruppe seiner Sitzung
    /// (`Inner::apply_resolved`), bevor in ihr etwas läuft.
    pub(super) fn apply_run_rules(
        &self,
        config: &Config,
        session_rules: Vec<humanitl_core::rule::Rule>,
    ) -> Result<Vec<RuleId>, Diagnostic> {
        if let Some(settings) = self.settings.as_ref() {
            settings.set(SessionState::for_config(
                config.hold.ask_mode,
                config.hold.timeout_secs,
                None,
            ));
        }
        let Some(store) = self.rules.as_ref() else {
            return Ok(Vec::new());
        };
        let mut bare = config.clone();
        bare.llm = humanitl_config::LlmConfig::default();
        let (bundled, _) = bundled_rules(&bare, &[], self.session);
        store.set_bundled(&bundled.all());
        let mut added = Vec::new();
        for rule in session_rules {
            match store.add(&rule, None) {
                Ok(rule) => added.push(rule.id),
                Err(found) => {
                    for id in added {
                        let _ = store.remove(id);
                    }
                    return Err(found);
                }
            }
        }
        Ok(added)
    }

    /// Nimmt die Regeln des Laufs wieder heraus.
    pub(super) fn unwind_rules(&self, staged: &Staged) {
        let Some(store) = self.rules.as_ref() else {
            return;
        };
        for id in &staged.rules {
            let _ = store.remove(*id);
        }
    }

    /// Begleitet den Lauf bis zum Ende und übernimmt dann sein Ergebnis.
    async fn accompany_run(
        self: Arc<Self>,
        staged: Staged,
        mut outcome: v1::AgentMaintenance,
        warnings: Vec<Diagnostic>,
        output: std::sync::mpsc::Receiver<humanitl_sandbox::OutputChunk>,
    ) {
        let (tx, mut rx) = mpsc::channel::<v1::SandboxEvent>(EVENT_BUFFER);
        let collector = tokio::spawn(async move {
            let mut code = None;
            let mut found = Vec::new();
            let mut failed = false;
            while let Some(event) = rx.recv().await {
                match event.event {
                    Some(v1::sandbox_event::Event::Exit(exit)) => code = Some(exit.code),
                    Some(v1::sandbox_event::Event::Diagnostic(diagnostic)) => {
                        found.push(diagnostic);
                    }
                    Some(v1::sandbox_event::Event::Status(status))
                        if status.state == v1::SandboxState::Failed as i32 =>
                    {
                        failed = true;
                    }
                    _ => {}
                }
            }
            (code, found, failed)
        });
        self.attend(
            &v1::sandbox_request::Plan::default(),
            None,
            output,
            None,
            &tx,
        )
        .await;
        drop(tx);
        let (code, mut found, failed) = collector.await.unwrap_or((None, Vec::new(), true));
        self.unwind_rules(&staged);
        // Green guarantees and status 0, or nothing is taken over.
        let succeeded = code == Some(0) && !failed;
        let own_loopback = staged.prepared.backend.own_loopback();
        let Staged { job, .. } = staged;
        let agent = job.agent().id;
        let run = match job {
            Job::Install(run) => run,
            Job::Agent(run) => {
                let outcome = self
                    .finish_agent_run(*run, succeeded, own_loopback, outcome, &warnings, found)
                    .await;
                lock(&self.maintenance).insert(agent.to_owned(), outcome);
                return;
            }
        };
        let committed = tokio::task::spawn_blocking(move || run.commit(succeeded))
            .await
            .map_err(|error| joined_failed(&error))
            .and_then(|result| result);
        match committed {
            Ok(committed) => {
                let task = outcome.task.clone();
                outcome = v1::AgentMaintenance {
                    sandbox_id: outcome.sandbox_id,
                    diagnostics: warnings.iter().map(convert::diagnostic_to_proto).collect(),
                    ..done_with(&task, &committed)
                };
                tracing::info!(
                    session = %self.session,
                    agent,
                    version = %committed.version,
                    "a maintenance run of the agent depot was taken over"
                );
            }
            Err(refused) => {
                tracing::warn!(
                    session = %self.session,
                    agent,
                    code = ?code,
                    why = %refused.why,
                    "a maintenance run of the agent depot was not taken over"
                );
                found.insert(0, convert::diagnostic_to_proto(&refused));
                found.extend(warnings.iter().map(convert::diagnostic_to_proto));
                STATE_FAILED.clone_into(&mut outcome.state);
                outcome.diagnostics = found;
            }
        }
        lock(&self.maintenance).insert(agent.to_owned(), outcome);
    }
}

/// Was eine Sitzung eines Laufs braucht.
pub(super) struct RunShape<'a> {
    pub(super) home: &'a std::path::Path,
    pub(super) agent: &'static KnownAgent,
    pub(super) command: Vec<std::ffi::OsString>,
    pub(super) env: Vec<(String, String)>,
    pub(super) mount: Option<humanitl_sandbox::agent::depot::DepotMount>,
}

/// Der Agent, dessen Wartungslauf noch nicht übernommen ist: Er läuft oder
/// wird gerade übernommen. Sein Depot darf solange nicht gehen.
pub(super) fn running_agent(
    runs: &std::collections::BTreeMap<String, v1::AgentMaintenance>,
) -> Option<String> {
    runs.iter()
        .find(|(_, run)| run.state == STATE_RUNNING)
        .map(|(agent, _)| agent.clone())
}

/// Das Ergebnis einer Übernahme oder Umschaltung.
fn done(task: MaintenanceTask, sandbox: &str, committed: &Committed) -> v1::AgentMaintenance {
    v1::AgentMaintenance {
        sandbox_id: sandbox.to_owned(),
        ..done_with(task.as_str(), committed)
    }
}

fn done_with(task: &str, committed: &Committed) -> v1::AgentMaintenance {
    v1::AgentMaintenance {
        task: task.to_owned(),
        state: STATE_DONE.to_owned(),
        version: committed.version.clone(),
        previous: committed.previous.clone().unwrap_or_default(),
        ..v1::AgentMaintenance::default()
    }
}
