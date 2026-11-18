//! Was ein Start für den Startvorschlag bekäme (HUM-536, `Projects.proposal`).
//!
//! Der Vorschlag nimmt seine Vorgaben aus derselben Vorbereitung wie die
//! Momentaufnahme eines `Plan` ([`Inner::prepare_keyed_with`]): dasselbe
//! Sandbox-Profil, derselbe Ordner, dasselbe Backend aus `sandbox.backend`
//! und der Untergrenze, derselbe aufgelöste Modus. Anders als ein `Plan`
//! liest er weder die Wahl eines anderen Clients (`pending`) noch die
//! laufende Sitzung: Profil und Schreibmodus kommen aus der Konfiguration,
//! die der Aufrufer für dieses Projekt aufgelöst hat, und nichts wird
//! gemerkt. Gestartet wird nichts; die Stufe der Kernel-Ebene prüft der
//! Vorschlag nicht hier, sondern über das Gedächtnis der Backend-Suche
//! (HUM-502).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use humanitl_config::{Config, WorkMode};
use humanitl_core::Diagnostic;
use humanitl_sandbox::SandboxBackend as _;
use humanitl_sandbox::{BackendSearches, MountPolicy, SandboxProfile};

use super::{BackendPick, Inner, KeyNeed, PROFILE_DIRS, PROFILE_SUBDIR, SandboxService, tree_dirs};
use crate::projects::proposal::{PlanFacts, ProfileFacts};
use crate::session::work_mode_name;
use crate::v1;

impl SandboxService {
    /// Was ein Start mit `config` im Ordner `folder` bekäme, ohne etwas zu
    /// merken. Blockierend: liest Profil und Konfiguration und sucht das
    /// Backend wie ein `Plan`.
    ///
    /// # Errors
    ///
    /// Die Befunde, mit denen auch ein `Plan` ablehnt: `SANDBOX_006` für
    /// einen Ordner, der kein Projekt sein darf, `CONFIG_001` für ein
    /// Sandbox-Profil, das es nicht gibt, `SANDBOX_035` und `SANDBOX_036`
    /// für die Wahl des Backends, `CONFIG_003` für einen Adapter, den es
    /// nicht gibt.
    ///
    /// Mit `searches` sucht der Plan sein Backend über das Gedächtnis der
    /// Suche und wartet höchstens bis `deadline` (HUM-502): Eine langsame
    /// Suche hält die Antwort nicht auf, ihr Befund (`DOCTOR_012`) steht
    /// dann im Vorschlag. Ohne `searches` sucht er wie ein `Plan`.
    pub fn proposal_plan(
        &self,
        config: &Config,
        folder: &Path,
        searches: Option<(&BackendSearches, Option<Instant>)>,
    ) -> Result<PlanFacts, Diagnostic> {
        let pick = searches.map_or(BackendPick::Detect, |(searches, deadline)| {
            BackendPick::Searched { searches, deadline }
        });
        self.inner.proposal_plan(config, folder, &pick)
    }

    /// Die Sandbox-Profile, die ein Start fände, sortiert: die eigenen des
    /// Nutzers und die mitgelieferten, je geladen und geprüft wie beim Start
    /// (dieselbe Suche, dieselbe Prüfung der Einhängungen). Ein Profil, das
    /// sich nicht laden lässt, steht mit seinem Befund da. Blockierend:
    /// liest Verzeichnisse und Dateien.
    #[must_use]
    pub fn sandbox_profiles(&self) -> Vec<ProfileFacts> {
        let mut dirs: Vec<PathBuf> = vec![self.inner.paths.profiles_dir().join("sandbox")];
        dirs.extend(tree_dirs());
        dirs.extend(
            PROFILE_DIRS
                .iter()
                .map(|dir| Path::new(dir).join(PROFILE_SUBDIR)),
        );
        let mut names: Vec<String> = dirs
            .iter()
            .filter_map(|dir| std::fs::read_dir(dir).ok())
            .flatten()
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let path = entry.path();
                let name = path.file_stem()?.to_str()?.to_owned();
                (path.extension()? == "toml"
                    && path.is_file()
                    && humanitl_config::profile::check_name(&name, "a sandbox profile").is_ok())
                .then_some(name)
            })
            .collect();
        names.sort();
        names.dedup();
        let policy = MountPolicy::from_paths(&self.inner.paths);
        names
            .into_iter()
            .map(|name| {
                let state = self
                    .inner
                    .profile_path(&name)
                    .and_then(|(path, _)| SandboxProfile::load_validated(&path, &policy))
                    .map(|_profile| ());
                ProfileFacts { name, state }
            })
            .collect()
    }
}

impl Inner {
    fn proposal_plan(
        &self,
        config: &Config,
        folder: &Path,
        pick: &BackendPick<'_>,
    ) -> Result<PlanFacts, Diagnostic> {
        let config = Arc::new(config.clone());
        let prepared = self.prepare_for_proposal(&config, folder, pick)?;
        // Ob eine Wegwerf-Schicht ginge, fragt das Backend an einem Plan mit
        // schreibbarem Ordner; nur so heißt die Antwort für einen Ordner, der
        // heute nur lesbar eingestellt ist, nicht stets „nein".
        let disposable = if prepared.session.work_mode == WorkMode::Rw {
            prepared.disposable_support()
        } else {
            let mut writable = (*config).clone();
            writable.sandbox.work_mode = WorkMode::Rw;
            self.prepare_for_proposal(&Arc::new(writable), folder, pick)
                .and_then(|writable| writable.disposable_support())
        };
        Ok(PlanFacts {
            isolation: prepared.backend.isolation(),
            mode: prepared.plan_mode.mode,
            mode_notice: prepared.plan_mode.notice.clone(),
            work_mode: prepared.session.work_mode,
            profile: prepared.profile.name.clone(),
            disposable,
        })
    }

    /// Die Vorbereitung eines `Plan` für diesen Ordner, mit Profil und
    /// Schreibmodus der Konfiguration statt der laufenden Sitzung.
    fn prepare_for_proposal(
        &self,
        config: &Arc<Config>,
        folder: &Path,
        pick: &BackendPick<'_>,
    ) -> Result<super::Prepared, Diagnostic> {
        let plan = v1::sandbox_request::Plan {
            work_dir: folder.display().to_string(),
            profile: config.sandbox.profile.clone(),
            work_mode: work_mode_name(config.sandbox.work_mode).to_owned(),
            ..v1::sandbox_request::Plan::default()
        };
        let wish = self.plan_choice(config, &plan)?;
        self.prepare_picking(config, &wish, &plan, &[], KeyNeed::Plan, pick)
    }
}
