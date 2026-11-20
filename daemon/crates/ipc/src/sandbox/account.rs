//! Anmeldung, Konfigurationslauf und Rückschrift der Zugangsdatei im Daemon
//! (HUM-520, ADR-0023 Punkte 4, 5 und 7).
//!
//! - **Anmeldung und Konfigurationslauf** sind Wartungsläufe wie Installation
//!   und Update ([`super::maintenance`]), aber mit dem Programm des Depots
//!   und einer schreibbaren Kopie seiner Konfiguration
//!   ([`humanitl_sandbox::agent::depot::account`]). Nach einer Anmeldung
//!   geht nur die vom Wächter geprüfte Zugangsdatei ins Depot; nach einem
//!   Konfigurationslauf wartet die Änderung im Zustand `confirm` auf die
//!   Bestätigung des Menschen.
//! - **URLs einer Anmeldung.** Die Ausgabe des Laufs geht auf dem Weg zum
//!   Terminal an einem Leser vorbei, der die URLs sammelt; offen dürfen
//!   Clients nur die zu den Anmeldehosts des Adapters (`openable`).
//! - **Callback.** Eine eingefügte URL wird geprüft und im Lauf mit einem
//!   Prozess an `127.0.0.1` zugestellt, nie über einen weitergeleiteten Port,
//!   und nur, wenn das Backend der Sandbox ein eigenes Loopback gibt.
//! - **Rückschrift aus jeder Sitzung.** Startet eine Sitzung einen Agenten
//!   aus dem Depot, beobachtet der Wächter die eine Zugangsdatei ihrer Kopie
//!   ([`humanitl_sandbox::agent::depot::writeback`]), außer
//!   `agents.<id>.credential_writeback = false`. Jedes Ergebnis geht über den
//!   Kanal der Sandbox-Meldungen ins Audit-Log, mit den SHA-256 alt und neu,
//!   nie dem Inhalt; eine Ablehnung steht bis zur nächsten Übernahme im
//!   Status des Agenten.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use humanitl_config::Config;
use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, FixAction, SandboxNotice, Severity};
use humanitl_sandbox::agent::depot::account::{self, AgentRun, ConfigChange};
use humanitl_sandbox::agent::depot::credential::{Refused, Taken};
use humanitl_sandbox::agent::depot::login_url;
use humanitl_sandbox::agent::depot::writeback::{Outcome, Writeback};
use humanitl_sandbox::agent::depot::{Depot, KnownAgent, LoginRecipe, MaintenanceTask};
use humanitl_sandbox::{OutputChunk, SandboxHandle};

use super::maintenance::{
    Job, RunShape, STATE_CONFIRM, STATE_DONE, STATE_FAILED, STATE_RUNNING, Staged,
};
use super::{Inner, SandboxService, joined_failed, lock};
use crate::session::SessionRequest;
use crate::{convert, v1};

/// Wie viel Ausgabe einer Anmeldung der Leser der URLs zurückhält.
const LOGIN_TEXT_CAP: usize = 64 * 1024;

/// Wie lange das Zustellen eines Callbacks höchstens dauert.
const DELIVERY_WAIT: std::time::Duration = std::time::Duration::from_secs(20);

/// Die abgelehnten Rückschriften je Agent, bis zur nächsten Übernahme.
pub(super) type Refusals = Arc<Mutex<BTreeMap<String, Diagnostic>>>;

/// Die URLs, die ein Anmeldelauf bisher ausgab.
pub(super) type LoginUrls = Arc<Mutex<Vec<v1::AgentLoginUrl>>>;

impl SandboxService {
    /// Stellt eine eingefügte Callback-URL in den laufenden Anmeldelauf von
    /// `agent` zu (HUM-520 Punkt 2).
    ///
    /// # Errors
    ///
    /// `AGENT_014` ohne laufenden Anmeldelauf dieses Agenten, ohne Callback
    /// des Adapters, auf einem Backend ohne eigenes Loopback, für eine andere
    /// URL als den Callback, oder wenn das Zustellen scheitert.
    pub async fn deliver_callback(&self, agent: &str, url: &str) -> Result<(), Diagnostic> {
        let inner = Arc::clone(&self.inner);
        let agent = agent.trim().to_owned();
        let url = url.to_owned();
        tokio::task::spawn_blocking(move || inner.deliver_callback(&agent, &url))
            .await
            .map_err(|error| joined_failed(&error))?
    }

    /// Die abgelehnten Rückschriften je Agent seit der letzten Übernahme.
    #[must_use]
    pub fn writeback_refusals(&self) -> BTreeMap<String, Diagnostic> {
        lock(&self.inner.writeback_refusals).clone()
    }

    /// Hält fest, dass die wartende Konfiguration von `agent` übernommen
    /// (`accepted`) oder verworfen ist; `waited` sagt, ob vor der
    /// Entscheidung ein Vorschlag auf der Platte lag. Siehe `decide_slot`.
    pub fn config_decided(&self, agent: &str, accepted: bool, waited: bool) {
        decide_slot(&mut lock(&self.inner.maintenance), agent, accepted, waited);
    }
}

/// Der Eintrag eines Agenten nach Übernehmen oder Verwerfen (HUM-521): ein
/// wartender Lauf steht danach auf `done` oder `discarded`. Lag der
/// Vorschlag nur auf der Platte (nach einem Neustart), ersetzt das Ergebnis
/// einen beendeten Lauf im Speicher, etwa ein späteres Update, statt dessen
/// Ergebnis stehen zu lassen. Ein laufender Lauf bleibt, wie er ist.
pub(super) fn decide_slot(
    runs: &mut BTreeMap<String, v1::AgentMaintenance>,
    agent: &str,
    accepted: bool,
    waited: bool,
) {
    let state = if accepted {
        STATE_DONE
    } else {
        crate::agents::DISCARDED
    };
    let agent = agent.trim();
    match runs.get_mut(agent) {
        Some(run) if run.state == STATE_CONFIRM => state.clone_into(&mut run.state),
        Some(run) if run.state == STATE_RUNNING => {}
        _ if waited => {
            runs.insert(
                agent.to_owned(),
                v1::AgentMaintenance {
                    task: MaintenanceTask::Config.as_str().to_owned(),
                    state: state.to_owned(),
                    ..v1::AgentMaintenance::default()
                },
            );
        }
        _ => {}
    }
}

impl Inner {
    /// Bereitet eine Anmeldung oder einen Konfigurationslauf vor.
    pub(super) fn stage_agent(
        &self,
        entry: &'static KnownAgent,
        task: MaintenanceTask,
    ) -> Result<Staged, Diagnostic> {
        let run = account::prepare(&self.paths, entry, task, self.session)?;
        let config = self
            .resolver
            .resolve(&SessionRequest::default())
            .map(|resolved| resolved.config)?;
        let prepared = self.run_session(
            &config,
            &RunShape {
                home: run.home(),
                agent: run.agent(),
                command: run.command(),
                env: run.env(),
                mount: Some(run.mount()),
            },
        )?;
        let rules = self.apply_run_rules(&config, run.session_rules())?;
        Ok(Staged {
            job: Job::Agent(Box::new(run)),
            prepared,
            rules,
        })
    }

    /// Ein Leser zwischen Lauf und Terminal, der die URLs einer Anmeldung
    /// sammelt. Zurück kommen der Sender für den Start und die gesammelten
    /// URLs; der Leser trägt sie auch in den laufenden Lauf ein.
    pub(super) fn read_login_urls(
        self: &Arc<Self>,
        login: LoginRecipe,
        forward: std::sync::mpsc::Sender<OutputChunk>,
    ) -> (std::sync::mpsc::Sender<OutputChunk>, LoginUrls) {
        let (tx, rx) = std::sync::mpsc::channel::<OutputChunk>();
        let urls: LoginUrls = Arc::new(Mutex::new(Vec::new()));
        let found = Arc::clone(&urls);
        let inner = Arc::clone(self);
        let agent_of_run = |runs: &BTreeMap<String, v1::AgentMaintenance>| {
            runs.iter()
                .find(|(_, run)| {
                    run.state == STATE_RUNNING && run.task == MaintenanceTask::Login.as_str()
                })
                .map(|(agent, _)| agent.clone())
        };
        let spawned = std::thread::Builder::new()
            .name("login-urls".to_owned())
            .spawn(move || {
                let mut text = String::new();
                for chunk in rx {
                    text.push_str(&String::from_utf8_lossy(&chunk.bytes));
                    if text.len() > LOGIN_TEXT_CAP {
                        let mut cut = text.len() - LOGIN_TEXT_CAP;
                        while !text.is_char_boundary(cut) {
                            cut += 1;
                        }
                        text.drain(..cut);
                    }
                    let now: Vec<v1::AgentLoginUrl> = login_url::login_urls(&text, &login)
                        .into_iter()
                        .map(|url| v1::AgentLoginUrl {
                            url: url.url,
                            openable: url.openable,
                        })
                        .collect();
                    {
                        let mut seen = lock(&found);
                        for url in now {
                            if !seen.iter().any(|known| known.url == url.url) {
                                seen.push(url);
                            }
                        }
                    }
                    let snapshot = lock(&found).clone();
                    {
                        let mut runs = lock(&inner.maintenance);
                        if let Some(agent) = agent_of_run(&runs)
                            && let Some(run) = runs.get_mut(&agent)
                        {
                            run.login_urls = snapshot;
                        }
                    }
                    if forward.send(chunk).is_err() {
                        // Nobody reads the terminal any more; the URLs are
                        // still collected.
                    }
                }
            });
        drop(spawned);
        (tx, urls)
    }

    /// Was eine Anmeldung auf dem Weg zum Terminal braucht: den Sender für
    /// den Start (an einem Leser der URLs vorbei), die gesammelten URLs und ob
    /// ein Client das Feld zum Einfügen der Callback-URL zeigt. Für jeden
    /// anderen Lauf geht die Ausgabe unverändert durch.
    pub(super) fn login_output(
        self: &Arc<Self>,
        staged: &Staged,
        output: std::sync::mpsc::Sender<OutputChunk>,
    ) -> (
        std::sync::mpsc::Sender<OutputChunk>,
        Option<LoginUrls>,
        bool,
    ) {
        let Job::Agent(run) = &staged.job else {
            return (output, None, false);
        };
        let Some(login) = run.login() else {
            return (output, None, false);
        };
        let paste =
            login_url::callback_offered(Some(login), staged.prepared.backend.own_loopback());
        let (tx, urls) = self.read_login_urls(*login, output);
        (tx, Some(urls), paste)
    }

    /// Übernimmt das Ergebnis einer Anmeldung oder eines Konfigurationslaufs.
    pub(super) async fn finish_agent_run(
        self: &Arc<Self>,
        run: AgentRun,
        succeeded: bool,
        own_loopback: bool,
        mut outcome: v1::AgentMaintenance,
        warnings: &[Diagnostic],
        mut found: Vec<v1::Diagnostic>,
    ) -> v1::AgentMaintenance {
        let agent = run.agent().id;
        if let Some(current) = lock(&self.maintenance).get(agent) {
            outcome.login_urls.clone_from(&current.login_urls);
            outcome.callback_paste = false;
        }
        let task = run.task();
        let result = tokio::task::spawn_blocking(move || match task {
            MaintenanceTask::Login => {
                Finished::Login(Box::new(run.commit_login(succeeded, own_loopback)))
            }
            _ => Finished::Config(Box::new(run.finish_config(succeeded))),
        })
        .await;
        let finished = match result {
            Ok(finished) => finished,
            Err(error) => {
                found.insert(0, convert::diagnostic_to_proto(&joined_failed(&error)));
                STATE_FAILED.clone_into(&mut outcome.state);
                outcome.diagnostics = found;
                return outcome;
            }
        };
        let warned = || warnings.iter().map(convert::diagnostic_to_proto);
        match finished {
            Finished::Login(login) => match *login {
                Ok(done) => {
                    self.report_writeback(agent, &Ok(done.taken));
                    STATE_DONE.clone_into(&mut outcome.state);
                    outcome.diagnostics = warned().collect();
                    tracing::info!(
                        session = %self.session,
                        agent,
                        named = ?done.named,
                        "a login into the agent depot was taken over"
                    );
                }
                Err(failed) => {
                    if let Some(refused) = failed.refused {
                        self.report_writeback(agent, &Err(refused));
                    }
                    if let Some(stored) = failed.stored {
                        self.report_writeback(agent, &Ok(stored));
                    }
                    if let Some(restored) = failed.restored {
                        self.report_writeback(agent, &Ok(restored));
                    }
                    found.insert(0, convert::diagnostic_to_proto(&failed.found));
                    found.extend(warned());
                    STATE_FAILED.clone_into(&mut outcome.state);
                    outcome.diagnostics = found;
                }
            },
            Finished::Config(config) => match *config {
                Ok(proposal) => {
                    if let Some(credential) = &proposal.credential {
                        self.report_writeback(agent, credential);
                    }
                    if proposal.changes.is_empty() {
                        STATE_DONE
                    } else {
                        STATE_CONFIRM
                    }
                    .clone_into(&mut outcome.state);
                    outcome.config_changes = proposal
                        .changes
                        .iter()
                        .map(config_change_to_proto)
                        .collect();
                    outcome.diagnostics = warned().collect();
                }
                Err(refused) => {
                    found.insert(0, convert::diagnostic_to_proto(&refused));
                    found.extend(warned());
                    STATE_FAILED.clone_into(&mut outcome.state);
                    outcome.diagnostics = found;
                }
            },
        }
        outcome
    }

    /// Meldet ein Ergebnis des Wächters ins Audit-Log und in den Status.
    pub(super) fn report_writeback(&self, agent: &str, result: &Result<Taken, Refused>) {
        report(
            self.sandbox_notices.as_ref(),
            &self.writeback_refusals,
            agent,
            result,
        );
    }

    /// Die Beobachtung der Zugangsdatei für eine Sitzung mit `agent` aus dem
    /// Depot; `None`, wenn `agents.<id>.credential_writeback = false` ist
    /// oder der Adapter kein Format der Zugangsdatei angibt.
    pub(super) fn writeback_for(&self, config: &Config, agent: &str) -> Option<Writeback> {
        let enabled = config
            .agents
            .settings(agent)
            .is_none_or(|settings| settings.credential_writeback);
        if !enabled {
            return None;
        }
        let recipe = self.depot_agent(agent)?.recipe?;
        let notices = self.sandbox_notices.clone();
        let refusals = Arc::clone(&self.writeback_refusals);
        Writeback::for_agent(
            Depot::of(&self.paths, agent),
            &recipe,
            self.session,
            Arc::new(move |outcome: &Outcome| {
                report(notices.as_ref(), &refusals, &outcome.agent, &outcome.result);
            }),
        )
    }

    /// Ein Agent des Depots, wie dieser Dienst ihn kennt
    /// ([`super::SandboxPorts::depot_agents`]).
    fn depot_agent(&self, agent: &str) -> Option<&'static KnownAgent> {
        self.depot_agents.iter().find(|known| known.id == agent)
    }

    /// Siehe [`SandboxService::deliver_callback`]. Blockiert.
    fn deliver_callback(&self, agent: &str, url: &str) -> Result<(), Diagnostic> {
        let refused = |why: String| {
            Diagnostic::builder(codes::AGENT_014, Severity::Error)
                .why(why)
                .fix(FixAction::CopyCommand(format!(
                    "humanitl agents login {agent}"
                )))
                .build()
        };
        let run = lock(&self.maintenance)
            .get(agent)
            .cloned()
            .ok_or_else(|| refused(format!("no login run of {agent:?} runs in this daemon")))?;
        if run.task != MaintenanceTask::Login.as_str() || run.state != STATE_RUNNING {
            return Err(refused(format!(
                "no login run of {agent} runs in this daemon; the last run is a {} run in \
                 state {}",
                run.task, run.state
            )));
        }
        let callback = self
            .depot_agent(agent)
            .and_then(|known| known.recipe)
            .and_then(|recipe| recipe.login_run)
            .and_then(|login| login.callback)
            .ok_or_else(|| {
                refused(format!(
                    "the adapter of {agent} waits for no callback; its login runs without one"
                ))
            })?;
        if !run.callback_paste {
            return Err(refused(
                "the backend of this run shares the host's loopback (Seatbelt); only the device \
                 flow works there, and no callback is delivered"
                    .to_owned(),
            ));
        }
        let inside = login_url::check_callback(url, &callback)?;
        let handle: Arc<SandboxHandle> = lock(&self.running)
            .as_ref()
            .filter(|running| running.handle.id.to_string() == run.sandbox_id)
            .map(|running| Arc::clone(&running.handle))
            .ok_or_else(|| refused("the login run has ended".to_owned()))?;
        let shell = handle.open_shell(&login_url::delivery_command(&inside), 80, 24)?;
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("login-callback".to_owned())
            .spawn(move || {
                let mut output = Vec::new();
                let code = shell.pump(|bytes| {
                    if output.len() < 4096 {
                        output.extend_from_slice(bytes);
                    }
                });
                let _ = done_tx.send((code, output));
            });
        if spawned.is_err() {
            return Err(refused("no thread to deliver the callback".to_owned()));
        }
        match done_rx.recv_timeout(DELIVERY_WAIT) {
            Ok((0, _)) => {
                tracing::info!(session = %self.session, agent, "a login callback was delivered");
                Ok(())
            }
            Ok((code, output)) => Err(refused(format!(
                "the callback did not reach the agent inside the run (curl ended with {code}: \
                 {})",
                String::from_utf8_lossy(&output).trim()
            ))),
            Err(_) => Err(refused(format!(
                "the callback was not delivered within {} s",
                DELIVERY_WAIT.as_secs()
            ))),
        }
    }
}

/// Was nach einem Lauf des Agenten feststeht.
enum Finished {
    Login(Box<Result<account::LoginDone, account::LoginFailed>>),
    Config(Box<Result<account::ConfigProposal, Diagnostic>>),
}

/// Ein Ergebnis des Wächters: ins Audit-Log über den Kanal der
/// Sandbox-Meldungen, eine Ablehnung in den Status, eine Übernahme löscht sie.
fn report(
    notices: Option<&humanitl_core::SandboxNoticeSink>,
    refusals: &Refusals,
    agent: &str,
    result: &Result<Taken, Refused>,
) {
    let notice = match result {
        Ok(Taken::Nothing) => return,
        Ok(Taken::Stored {
            old, new, warning, ..
        }) => {
            lock(refusals).remove(agent);
            if let Some(warning) = warning {
                tracing::warn!(agent, %warning, "the credential file is in place, but not synced");
            }
            tracing::info!(agent, sha256_old = ?old, sha256_new = %new, "the credential file went back into the depot");
            SandboxNotice::CredentialWriteback {
                agent: agent.to_owned(),
                accepted: true,
                code: None,
                sha256_old: old.clone(),
                // Empty when a failed login removed its new file again.
                sha256_new: (!new.is_empty()).then(|| new.clone()),
            }
        }
        Err(refused) => {
            lock(refusals).insert(agent.to_owned(), refused.found.clone());
            tracing::warn!(agent, code = %refused.found.code.as_str(), why = %refused.found.why, "the credential file was not taken back");
            SandboxNotice::CredentialWriteback {
                agent: agent.to_owned(),
                accepted: false,
                code: Some(refused.found.code.as_str().to_owned()),
                sha256_old: refused.old.clone(),
                sha256_new: refused.new.clone(),
            }
        }
    };
    if let Some(notices) = notices {
        notices.report(notice);
    }
}

/// Eine Änderung der Konfiguration im Protokoll.
pub(crate) fn config_change_to_proto(change: &ConfigChange) -> v1::AgentConfigChange {
    v1::AgentConfigChange {
        path: change.path.clone(),
        change: change.change.as_str().to_owned(),
        keys: change.keys.clone(),
        command_keys: change.command_keys.clone(),
        keys_omitted: u32::try_from(change.omitted).unwrap_or(u32::MAX),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    fn run(task: &str, state: &str) -> v1::AgentMaintenance {
        v1::AgentMaintenance {
            sandbox_id: "s1".to_owned(),
            task: task.to_owned(),
            state: state.to_owned(),
            version: "2.1.0".to_owned(),
            ..v1::AgentMaintenance::default()
        }
    }

    /// HUM-521 (Review): Nach einem Neustart wartet ein Vorschlag auf der
    /// Platte, im Speicher steht ein späteres Update. Die Entscheidung
    /// ersetzt dessen Ergebnis; ohne Vorschlag bleibt es.
    #[test]
    fn a_decision_on_a_disk_proposal_replaces_an_ended_run() {
        let mut runs = BTreeMap::from([("claude".to_owned(), run("update", STATE_DONE))]);
        decide_slot(&mut runs, "claude", true, true);
        assert_eq!(runs["claude"].task, "config");
        assert_eq!(runs["claude"].state, STATE_DONE);
        assert!(runs["claude"].version.is_empty());

        let mut runs = BTreeMap::from([("claude".to_owned(), run("update", STATE_DONE))]);
        decide_slot(&mut runs, "claude", false, true);
        assert_eq!(runs["claude"].task, "config");
        assert_eq!(runs["claude"].state, crate::agents::DISCARDED);

        let mut runs = BTreeMap::from([("claude".to_owned(), run("update", STATE_DONE))]);
        decide_slot(&mut runs, "claude", false, false);
        assert_eq!(
            runs["claude"].task, "update",
            "nothing waited, nothing replaced"
        );

        let mut runs = BTreeMap::from([("claude".to_owned(), run("config", STATE_CONFIRM))]);
        decide_slot(&mut runs, "claude", false, true);
        assert_eq!(runs["claude"].state, crate::agents::DISCARDED);
        assert_eq!(runs["claude"].sandbox_id, "s1");

        let mut runs = BTreeMap::from([("claude".to_owned(), run("login", STATE_RUNNING))]);
        decide_slot(&mut runs, "claude", true, true);
        assert_eq!(runs["claude"].state, STATE_RUNNING);
    }

    /// Jede Übernahme und jede Ablehnung geht als Meldung ins Audit-Log,
    /// mit den SHA-256 und nie dem Inhalt; eine Ablehnung steht im Status,
    /// bis eine Übernahme sie löscht.
    #[test]
    fn every_outcome_reaches_the_audit_channel_and_a_refusal_the_status() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = {
            let seen = Arc::clone(&seen);
            humanitl_core::SandboxNoticeSink::announcing(move |event| {
                if let humanitl_core::FlowEvent::Sandbox { notice, .. } = event {
                    seen.lock().unwrap().push(notice);
                }
            })
        };
        let refusals: Refusals = Arc::default();
        let refused = Refused {
            found: Diagnostic::builder(codes::AGENT_012, Severity::Warning)
                .why("no".to_owned())
                .build(),
            old: Some("aa".to_owned()),
            new: Some("bb".to_owned()),
        };
        report(Some(&sink), &refusals, "claude", &Err(refused));
        assert!(lock(&refusals).contains_key("claude"));
        report(Some(&sink), &refusals, "claude", &Ok(Taken::Nothing));
        report(
            Some(&sink),
            &refusals,
            "claude",
            &Ok(Taken::Stored {
                old: Some("aa".to_owned()),
                new: "cc".to_owned(),
                account: Some("someone".to_owned()),
                warning: None,
            }),
        );
        assert!(lock(&refusals).is_empty());
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 2);
        assert_eq!(
            seen[0],
            SandboxNotice::CredentialWriteback {
                agent: "claude".to_owned(),
                accepted: false,
                code: Some("AGENT_012".to_owned()),
                sha256_old: Some("aa".to_owned()),
                sha256_new: Some("bb".to_owned()),
            }
        );
        let text = format!("{:?}", seen[1]);
        assert!(
            !text.contains("someone"),
            "the account never reaches the log: {text}"
        );
    }
}
