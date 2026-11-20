//! `Projects.proposal`: der Erststart-Bildschirm als Daten (HUM-536,
//! ADR-0024 Punkt 3).
//!
//! Ein Aufruf liefert alles, was der Bildschirm zeigt: je Feld (Agent mit
//! Quelle, Ordnermodus, Isolation, Nachfragemodus, Modellserver, Profil) die
//! Optionen, die Vorgabe, ob jede Option wählbar ist und sonst warum, die
//! Sperren der Firmenvorgabe, den Hinweis auf Cloud-Inferenz und die Zeile
//! „Defaults: …" für den Weg ohne Bildschirm.
//!
//! **Die Vorgaben sind der Start, der ohne Bildschirm geschähe.** Aufgelöst
//! wird wie beim Start ([`super::check_for`]): der Ordner als Projekt mit
//! seinem Projekt-Profil, darüber die gespeicherte Projektwahl, darüber die
//! Teilwahl der Anfrage, darüber die Sperren. Modus, Backend, Profil und
//! Schreibmodus kommen aus der Vorbereitung eines `Plan` im Sandbox-Dienst
//! ([`crate::SandboxService::proposal_plan`]); eine zweite Tabelle der
//! Vorgaben gibt es nicht.
//!
//! **Die Kernel-Stufe wird nicht gestartet.** Ob ein Backend da ist, sagt
//! die Liste der Backends über das Gedächtnis der Suche, das sich `Doctor`
//! und `SandboxBackends` teilen (HUM-502), bis zur Frist des Aufrufers.
//!
//! **Prüfen ohne Speichern.** Die Teilwahl der Anfrage liegt nur für diese
//! Antwort über der gespeicherten Wahl; gespeichert wird mit `save_choice`.
//!
//! **Die Werte der Sitzung wie beim Start** (HUM-567). `Proposal.cli_overrides`
//! sind die Werte, die ein Start dieses Aufrufs als `Start.cli_overrides`
//! schickte, mit derselben Erlaubnisliste. Sie liegen auf der Ebene der
//! Kommandozeile über Wahl und Teilwahl (`projects::resolve_at_with`); die
//! Untergrenze hebt also nur, und ein Backend unter ihr nennt der Vorschlag
//! mit `SANDBOX_035` wie der Start. Die Wahl selbst wird ohne sie geprüft
//! wie beim Speichern, denn gespeichert werden sie nie: Auch die Wahl der
//! Vorgaben (`StartProposal.choice`) kommt aus dem Vorschlag ohne sie; nur
//! Felder, Befunde und die Zeile der Vorgaben zeigen den Start mit ihnen.

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use humanitl_config::agents::{SourceLock, effective_source};
use humanitl_config::{Config, Env, Paths, ProjectChoice, Resolved, SandboxMode};
use humanitl_core::block::sanitize_note;
use humanitl_core::diagnostics::codes::{AGENT_001, AGENT_009, AGENT_015};
use humanitl_core::{Diagnostic, FixAction, Severity};
use humanitl_recorder::{ProjectRow, Recorder};
use humanitl_sandbox::agent::depot::{Depot, KnownAgent};
use humanitl_sandbox::{AUTO_BACKEND, AdapterRegistry, BackendEntry, BackendSearches};

use super::{Context, ProjectIdentity, blocking, check_for, refused};
use crate::v1;

mod fields;

pub use fields::{
    AGENT, ASK_MODE, AgentFacts, Built, DISPOSABLE, FOLDER_MODE, ISOLATION, Inputs, KERNEL,
    LLM_SERVER, PROFILE, PlanFacts, ProfileFacts, READ_ONLY, READ_WRITE, SHARED_KERNEL,
    ServerFacts, build,
};

/// Was einem Backend auf dem System fehlt, oder nichts.
pub type MissingOnSystem = Arc<dyn Fn(&'static str) -> Option<Diagnostic> + Send + Sync>;

/// Woher der Vorschlag weiß, welche Backends es gibt.
#[derive(Clone)]
pub enum BackendFacts {
    /// Das Gedächtnis der Suche im Daemon, bis zur Frist des Aufrufers
    /// (`grpc-timeout`); eine Suche, die dann noch läuft, ist „wird geprüft"
    /// (`DOCTOR_012`) und die Stufe nicht wählbar.
    Searches {
        /// Das Gedächtnis, geteilt mit `Doctor` und `SandboxBackends`.
        searches: BackendSearches,
        /// Die Frist; `None` wartet auf jede Suche.
        deadline: Option<Instant>,
    },
    /// Eine gegebene Antwort je Backend: der Fake und Tests, die ein
    /// System ohne `/dev/kvm` spielen.
    Given(MissingOnSystem),
}

impl Default for BackendFacts {
    fn default() -> Self {
        Self::Searches {
            searches: BackendSearches::new(),
            deadline: None,
        }
    }
}

impl std::fmt::Debug for BackendFacts {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Searches { deadline, .. } => f
                .debug_struct("Searches")
                .field("deadline", deadline)
                .finish_non_exhaustive(),
            Self::Given(_) => f.write_str("Given"),
        }
    }
}

impl BackendFacts {
    /// Die Backends, gemessen an der Gruppe `sandbox` und der Anforderung
    /// des Sandbox-Profils. Blockierend bis zur Frist.
    fn list(&self, config: &Config, profile_backend: &str, paths: &Paths) -> Vec<BackendEntry> {
        match self {
            Self::Searches { searches, deadline } => humanitl_sandbox::list_backends_within(
                &config.sandbox,
                profile_backend,
                paths,
                searches,
                *deadline,
            ),
            Self::Given(missing) => {
                humanitl_sandbox::list_backends_with(&config.sandbox, profile_backend, |name| {
                    missing(name)
                })
            }
        }
    }
}

/// Was der Vorschlag über `Context` hinaus braucht.
#[derive(Debug, Clone, Default)]
pub struct Ports {
    /// Woher die Liste der Backends kommt.
    pub backends: BackendFacts,
    /// Die Sperre der Quelle der Agenten (HUM-518).
    pub source_lock: SourceLock,
}

/// Beantwortet `proposal`.
///
/// # Errors
///
/// `CONFIG_003` ohne Ordner, für einen relativen, fehlenden oder ersetzten
/// und für ein unbekanntes Projekt; `CONFIG_024` für eine gespeicherte Wahl,
/// die nicht mehr lädt, und für eine Teilwahl, die nicht gälte (fremder
/// Schlüssel, gesperrter Wert, Untergrenze); die Befunde des Ladens.
pub(crate) async fn answer(
    ctx: &Context,
    recorder: &Recorder,
    asked: v1::projects_request::Proposal,
) -> Result<v1::ProjectsResponse, Diagnostic> {
    let overrides = session_values(&asked.cli_overrides)?;
    let (row, path) = target(recorder, &asked).await?;
    let choice = merged(row.as_ref(), &asked.values)?;
    let ctx = ctx.clone();
    let proposal = blocking("Projects", move || {
        propose(&ctx, row.as_ref(), &path, &choice, &overrides)
    })
    .await??;
    Ok(v1::ProjectsResponse {
        proposal: Some(proposal),
        ..v1::ProjectsResponse::default()
    })
}

/// Die Werte der Sitzung einer Anfrage (HUM-567): derselbe getrimmte Pfad
/// und derselbe ungetrimmte Wert wie beim Start, jeder Pfad gegen dieselbe
/// Erlaubnisliste ([`crate::session::check_override_key`]).
///
/// # Errors
///
/// `CONFIG_003` für einen Pfad, den eine Sitzung nicht setzt.
pub fn session_values(
    overrides: &[v1::sandbox_request::CliOverride],
) -> Result<Vec<(String, String)>, Diagnostic> {
    overrides
        .iter()
        .map(|entry| {
            let path = entry.path.trim();
            crate::session::check_override_key(path)?;
            Ok((path.to_owned(), entry.value.clone()))
        })
        .collect()
}

/// Die Auflösung eines Starts in `path` mit dieser Wahl und diesen Werten
/// der Sitzung (HUM-567): die Wahl geprüft wie beim Speichern
/// ([`check_for`], ohne die Werte), darüber die Werte auf der Ebene der
/// Kommandozeile.
///
/// # Errors
///
/// Die Befunde von [`check_for`] und des Ladens mit den Werten.
fn resolve_with_session(
    ctx: &Context,
    path: &str,
    choice: &ProjectChoice,
    overrides: &[(String, String)],
) -> Result<Resolved, Diagnostic> {
    let checked = check_for(&ctx.paths, &ctx.locks, path, choice)?;
    if overrides.is_empty() {
        return Ok(checked);
    }
    super::resolve_at_with(&ctx.paths, &ctx.locks, path, Some(choice), overrides)
}

/// Der Ordner und, wenn das Register ihn kennt, seine Zeile.
async fn target(
    recorder: &Recorder,
    asked: &v1::projects_request::Proposal,
) -> Result<(Option<ProjectRow>, String), Diagnostic> {
    if !asked.project_id.trim().is_empty() {
        let row = super::lookup(recorder, &asked.project_id).await?;
        if row.replaced_at.is_some() {
            return Err(refused(format!(
                "the project {} was replaced by another folder at the same path; ask for the \
                 folder instead",
                row.id
            )));
        }
        let path = row.path.clone();
        return Ok((Some(row), path));
    }
    let given = asked.path.trim();
    if given.is_empty() || !Path::new(given).is_absolute() {
        return Err(refused(
            "a proposal is for a project id or an absolute folder".to_owned(),
        ));
    }
    let identity = ProjectIdentity::of(Path::new(given)).map_err(|err| {
        refused(format!(
            "{} is no folder a project can start in ({err})",
            sanitize_note(given)
        ))
    })?;
    let row = recorder
        .current_project_at(identity.path_text(), Some((identity.dev, identity.ino)))
        .await
        .map_err(humanitl_recorder::RecorderError::into_diagnostic)?
        // Ein anderer Ordner am alten Pfad ist ein neues Projekt (HUM-532).
        .filter(|row| {
            row.dev
                .zip(row.ino)
                .is_none_or(|known| known == (identity.dev, identity.ino))
        });
    Ok((row, identity.path_text()))
}

/// Die gespeicherte Wahl mit der Teilwahl darüber: Schlüssel für Schlüssel,
/// ein leerer Wert nimmt den Schlüssel heraus.
///
/// # Errors
///
/// `CONFIG_024` für eine gespeicherte Wahl, die nicht lädt, und für einen
/// Schlüssel außerhalb der Liste.
pub fn merged(
    row: Option<&ProjectRow>,
    values: &[v1::ChoiceValue],
) -> Result<ProjectChoice, Diagnostic> {
    let stored = row
        .and_then(|row| row.choice.as_deref())
        .map(ProjectChoice::from_json)
        .transpose()?
        .unwrap_or_default();
    let mut pairs: BTreeMap<String, String> = stored
        .iter()
        .map(|(key, value)| (key.to_owned(), value.to_owned()))
        .collect();
    for value in values {
        let key = value.key.trim().to_owned();
        let text = value.value.trim();
        if text.is_empty() {
            pairs.remove(&key);
        } else {
            pairs.insert(key, text.to_owned());
        }
    }
    ProjectChoice::from_pairs(pairs)
}

/// Der Vorschlag für diesen Ordner mit dieser Wahl und diesen Werten der
/// Sitzung. Blockierend.
fn propose(
    ctx: &Context,
    row: Option<&ProjectRow>,
    path: &str,
    choice: &ProjectChoice,
    overrides: &[(String, String)],
) -> Result<v1::StartProposal, Diagnostic> {
    let resolved = resolve_with_session(ctx, path, choice, overrides)?;
    let config = &resolved.config;
    let searched = match &ctx.proposal.backends {
        BackendFacts::Searches { searches, deadline } => Some((searches, *deadline)),
        BackendFacts::Given(_) => None,
    };
    let plan = ctx.sandbox.as_ref().map_or_else(
        || assumed_plan(config),
        |sandbox| sandbox.proposal_plan(config, Path::new(path), searched),
    );
    let profile_backend = match (&ctx.sandbox, &plan) {
        (Some(sandbox), Ok(plan)) => sandbox
            .profile_backend(&plan.profile)
            .unwrap_or_else(|_| AUTO_BACKEND.to_owned()),
        _ => AUTO_BACKEND.to_owned(),
    };
    let backends = ctx
        .proposal
        .backends
        .list(config, &profile_backend, &ctx.paths);
    let profiles = ctx.sandbox.as_ref().map_or_else(
        || {
            vec![ProfileFacts {
                name: config.sandbox.profile.clone(),
                state: Ok(()),
            }]
        },
        crate::SandboxService::sandbox_profiles,
    );
    let inputs = Inputs {
        resolved: &resolved,
        locks: &ctx.locks,
        plan,
        backends: &backends,
        agents: agents(&ctx.paths, config, &ctx.proposal.source_lock),
        servers: servers(ctx, path, choice, overrides, &resolved),
        profiles,
        cloud: crate::cloud::status(&ctx.paths, config),
    };
    let mut built = build(&inputs);
    keep_unedited(&mut built, choice);
    // Die Wahl zum Speichern kommt ohne die Werte der Sitzung (HUM-567):
    // Sie gelten für einen Lauf, nicht für das Projekt. Ein Client, der die
    // Vorgaben speichert, schriebe sonst etwa das Backend dieses Aufrufs in
    // die Projektwahl. Die Felder zeigen den Start mit ihnen.
    if !overrides.is_empty() {
        built.choice = propose(ctx, row, path, choice, &[])?.choice;
    }
    Ok(proposal(row, path, built))
}

/// Schlüssel der Wahl, die kein Feld des Bildschirms schreibt (heute
/// `sandbox.min_isolation`), bleiben in der Wahl der Vorgaben: Wer die
/// Vorgaben unverändert speichert, verliert sie sonst still.
fn keep_unedited(built: &mut Built, choice: &ProjectChoice) {
    let edited: Vec<String> = built
        .fields
        .iter()
        .flat_map(|field| field.keys.iter().cloned())
        .collect();
    built.choice.extend(
        choice
            .iter()
            .filter(|(key, _)| !edited.iter().any(|edited| edited == key))
            .map(|(key, value)| v1::ChoiceValue {
                key: key.to_owned(),
                value: value.to_owned(),
            }),
    );
}

/// Der Vorschlag mit Ordner, Name und Kennung.
#[must_use]
pub fn proposal(row: Option<&ProjectRow>, path: &str, built: Built) -> v1::StartProposal {
    let name = row.map_or_else(
        || {
            Path::new(path).file_name().map_or_else(
                || path.to_owned(),
                |name| name.to_string_lossy().into_owned(),
            )
        },
        |row| row.display_name.clone(),
    );
    v1::StartProposal {
        path: sanitize_note(path),
        display_name: sanitize_note(&name),
        project_id: row.map(|row| row.id.to_string()).unwrap_or_default(),
        has_choice: row.is_some_and(|row| row.choice.is_some()),
        fields: built.fields,
        choice: built.choice,
        defaults_line: sanitize_note(&built.defaults_line),
        diagnostics: built.diagnostics,
        cloud_inference: built.cloud,
    }
}

/// Was ein Daemon ohne Sandbox-Dienst (der Fake, Tests) als Plan annimmt:
/// das Backend aus der Wahl ohne Suche, der Modus der Konfiguration, eine
/// Wegwerf-Schicht, wo sie gewünscht ist.
///
/// # Errors
///
/// Die Befunde der Wahl des Backends (`SANDBOX_035`, `SANDBOX_036`,
/// `SANDBOX_048`).
pub fn assumed_plan(config: &Config) -> Result<PlanFacts, Diagnostic> {
    let chosen = humanitl_sandbox::choose_backend(&config.sandbox, AUTO_BACKEND)?;
    Ok(PlanFacts {
        isolation: chosen.isolation,
        mode: if config.sandbox.mode == SandboxMode::Direct {
            SandboxMode::Direct
        } else {
            SandboxMode::Disposable
        },
        mode_notice: None,
        work_mode: config.sandbox.work_mode,
        profile: config.sandbox.profile.clone(),
        disposable: Ok(()),
    })
}

/// Je Adapter, woher sein Programm käme: die wirksame Quelle, ob es auf dem
/// Host liegt, ob das Depot es hält, und der Hinweis auf Cloud-Inferenz.
/// Blockierend: sieht in Verzeichnisse.
#[must_use]
pub fn agents(paths: &Paths, config: &Config, lock: &SourceLock) -> Vec<AgentFacts> {
    let registry = AdapterRegistry::builtin();
    registry
        .ids()
        .into_iter()
        .map(|id| {
            let program = registry.get(id).map_or(id, |adapter| adapter.program());
            agent_facts(paths, config, lock, id, program)
        })
        .collect()
}

/// Die Tatsachen eines Agenten mit dieser Kennung und diesem Programm.
pub(crate) fn agent_facts(
    paths: &Paths,
    config: &Config,
    lock: &SourceLock,
    id: &str,
    program: &str,
) -> AgentFacts {
    let known = humanitl_sandbox::agent::depot::recipe::known(id);
    let mut with_agent = config.clone();
    id.clone_into(&mut with_agent.agent.adapter);
    AgentFacts {
        id: id.to_owned(),
        display_name: known.map_or(id, |agent| agent.display_name).to_owned(),
        source: effective_source(id, config, lock).source,
        host: host_program(paths.env(), config, program),
        depot: depot_state(paths, id, known),
        cloud: crate::cloud::status(paths, &with_agent),
    }
}

/// Ob das Programm eines Agenten auf dem Host liegt: `agent.command`, wenn
/// gesetzt, sonst der Programmname im `PATH` des Daemons und in den
/// Programmordnern des Nutzers (HUM-278). `agent.command` ersetzt die
/// Kommandozeile jedes Adapters (der Start setzt es ohne Blick auf den
/// Adapter, `with_command_override`); ein anderer Adapter startet also
/// dasselbe Kommando, und so wird er auch geprüft.
fn host_program(env: &Env, config: &Config, program: &str) -> Result<(), Diagnostic> {
    let command = config
        .agent
        .command
        .as_ref()
        .and_then(|command| command.first())
        .map_or(program, String::as_str);
    let dirs = humanitl_sandbox::agent::locate::search_dirs(
        env.non_empty("PATH").map(OsStr::new),
        env.non_empty("HOME").map(Path::new),
    );
    let found = if Path::new(command).is_absolute() {
        humanitl_sandbox::agent::is_executable_file(Path::new(command))
    } else {
        dirs.iter()
            .filter(|dir| dir.is_absolute())
            .any(|dir| humanitl_sandbox::agent::is_executable_file(&dir.join(command)))
    };
    if found {
        return Ok(());
    }
    Err(Diagnostic::builder(AGENT_001, Severity::Blocking)
        .why(format!(
            "{} is not on this computer: neither in the PATH of the daemon nor in the program \
             folders of the user",
            sanitize_note(command)
        ))
        .fix(FixAction::CopyCommand(
            humanitl_sandbox::agent::locate::agent_command_fix(program),
        ))
        .build())
}

/// Ob das Depot den Agenten hält: ein Rezept, eine installierte Version,
/// und als Hinweis, ob er angemeldet ist.
fn depot_state(
    paths: &Paths,
    id: &str,
    known: Option<&KnownAgent>,
) -> Result<Vec<Diagnostic>, Diagnostic> {
    let Some(agent) = known.filter(|agent| agent.recipe.is_some()) else {
        return Err(Diagnostic::builder(AGENT_015, Severity::Blocking)
            .why(format!(
                "the adapter of {} brings no recipe for the depot yet",
                known.map_or(id, |agent| agent.display_name)
            ))
            .build());
    };
    let depot = Depot::of(paths, id);
    depot.installed()?;
    if depot.logged_in() {
        return Ok(Vec::new());
    }
    Ok(vec![
        Diagnostic::builder(AGENT_009, Severity::Warning)
            .why(format!(
                "the depot holds {} and no login for it; the agent asks for one in the session",
                agent.display_name
            ))
            .fix(FixAction::CopyCommand(format!(
                "humanitl agents login {id}"
            )))
            .build(),
    ])
}

/// Die gespeicherten Server, je mit dem Hinweis auf Cloud-Inferenz einer
/// Sitzung, die ihn wählt: aufgelöst wie beim Start mit `llm.server` in der
/// Wahl, weil erst das Laden den Eintrag an die flachen Schlüssel setzt.
fn servers(
    ctx: &Context,
    path: &str,
    choice: &ProjectChoice,
    overrides: &[(String, String)],
    resolved: &Resolved,
) -> Vec<ServerFacts> {
    let mut names: Vec<String> = resolved.config.llm.servers.keys().cloned().collect();
    if let Some(chosen) = resolved
        .config
        .llm
        .choice
        .as_ref()
        .filter(|chosen| chosen.known)
        && !names.contains(&chosen.name)
    {
        names.push(chosen.name.clone());
    }
    let default = resolved
        .config
        .llm
        .choice
        .as_ref()
        .map(|chosen| chosen.name.clone());
    names
        .into_iter()
        .map(|name| {
            let pairs = choice
                .iter()
                .map(|(key, value)| (key.to_owned(), value.to_owned()))
                .filter(|(key, _)| key != humanitl_config::servers::SERVER)
                .chain([(humanitl_config::servers::SERVER.to_owned(), name.clone())]);
            let with = ProjectChoice::from_pairs(pairs)
                .and_then(|with| resolve_with_session(ctx, path, &with, overrides))
                .ok();
            let cloud = with
                .as_ref()
                .and_then(|with| crate::cloud::status(&ctx.paths, &with.config));
            // Wie der Start (`SessionResolver::resolve`): eine Wahl des
            // Projekt-Profils, die für diesen Ordner nicht bestätigt ist, hält
            // ihn auf (`CONFIG_022`). Für die Vorgabe gilt die Auflösung ohne
            // Bildschirm, für jede andere Option die mit ihr in der Wahl.
            let refusal = if default.as_deref() == Some(name.as_str()) {
                humanitl_config::assign::unconfirmed(&ctx.paths, &resolved.config.llm)
            } else {
                with.and_then(|with| {
                    humanitl_config::assign::unconfirmed(&ctx.paths, &with.config.llm)
                })
            };
            ServerFacts {
                name,
                cloud,
                refusal,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
