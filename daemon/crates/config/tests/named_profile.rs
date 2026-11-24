//! Ein Profil, das das Projekt-Profil mit `name` nennt, steht unter der
//! Grenze des Projekt-Profils: Schlüssel mit `x-project-scope = "denied"` und
//! jede Regel außer `block` und `ask` bleiben draußen, je mit einer Warnung.
//! Wählt der Mensch dasselbe Profil selbst, gilt es ganz.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

use humanitl_config::model::AskMode;
use humanitl_config::{
    BUILTIN_PROFILES, Env, Origin, ProfileSelection, ProfileSource, ProjectScope, Resolved,
    Sources, WorkMode, load, resolve, schema,
};
use humanitl_core::{Diagnostic, Severity};

/// Ein Zuhause auf Zeit: `$XDG_CONFIG_HOME` und ein Projekt.
struct Home {
    dir: tempfile::TempDir,
}

impl Home {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(dir.path().join("cfg/humanitl/profiles")).expect("mkdir");
        std::fs::create_dir_all(dir.path().join("project/.humanitl")).expect("mkdir");
        Self { dir }
    }

    fn env(&self) -> Env {
        Env::from_pairs([
            ("HOME", self.dir.path().display().to_string()),
            (
                "XDG_CONFIG_HOME",
                self.dir.path().join("cfg").display().to_string(),
            ),
        ])
        .with_uid(current_uid())
    }

    fn project(&self) -> PathBuf {
        self.dir.path().join("project")
    }

    fn write_config(&self, body: &str) {
        std::fs::write(self.dir.path().join("cfg/humanitl/config.toml"), body).expect("write");
    }

    fn write_profile(&self, name: &str, body: &str) -> PathBuf {
        let path = self
            .dir
            .path()
            .join("cfg/humanitl/profiles")
            .join(format!("{name}.toml"));
        std::fs::write(&path, body).expect("write");
        path
    }

    fn write_project_profile(&self, body: &str) -> PathBuf {
        let path = self.project().join(".humanitl/profile.toml");
        std::fs::write(&path, body).expect("write");
        path
    }

    fn resolve(&self, selection: &ProfileSelection) -> Resolved {
        match resolve(selection, Some(&self.project()), &self.env(), &[]) {
            Ok(resolved) => resolved,
            Err(diagnostic) => panic!("resolve failed: {diagnostic}"),
        }
    }
}

fn current_uid() -> u32 {
    use std::os::unix::fs::MetadataExt as _;

    std::fs::metadata("/proc/self").map_or(0, |meta| meta.uid())
}

/// Die Warnungen `CONFIG_003`, die ein übergangenes Stück melden.
fn ignored(resolved: &Resolved) -> Vec<&Diagnostic> {
    resolved
        .diagnostics
        .iter()
        .filter(|diagnostic| {
            diagnostic.code.as_str() == "CONFIG_003"
                && diagnostic.severity == Severity::Warning
                && diagnostic.why.contains("is ignored")
        })
        .collect()
}

fn denied(path: &str) -> bool {
    schema::field(path).is_some_and(|field| field.project_scope == ProjectScope::Denied)
}

/// Der Nutzer will den Ordner nur lesbar; das Projekt-Profil nennt
/// `llm-only`, das `sandbox.work_mode = "rw"` setzt. Der Ordner bleibt nur
/// lesbar, und eine Warnung nennt den Schlüssel und das Profil.
///
/// Mutationsbeweis: Ruft `load` die Prüfung `hold_to_project_scope` nicht
/// auf, steht `work_mode` auf `rw` aus `llm-only`, und der Test wird rot.
#[test]
fn a_named_profile_cannot_open_a_folder_the_user_keeps_read_only() {
    let home = Home::new();
    home.write_config("[sandbox]\nmode = \"direct\"\nwork_mode = \"ro\"\n");
    home.write_project_profile("name = \"llm-only\"\n");

    let resolved = home.resolve(&ProfileSelection::any());
    assert_eq!(resolved.config.sandbox.work_mode, WorkMode::Ro);
    assert_eq!(resolved.origin("sandbox.work_mode"), Some(&Origin::Global));
    let notes = ignored(&resolved);
    let note = notes
        .iter()
        .find(|note| note.why.starts_with("sandbox.work_mode"))
        .unwrap_or_else(|| panic!("a warning names sandbox.work_mode: {notes:?}"));
    assert!(note.why.contains("llm-only"), "{}", note.why);
    assert!(note.fix.is_none(), "no button carries the wish further");
}

/// Jedes mitgelieferte Profil außer `default`, vom Projekt-Profil genannt,
/// trägt keinen Schlüssel bei, den das Projekt-Profil nicht setzen dürfte,
/// und keine Regel außer `block` und `ask`. Je übergangenem Stück steht
/// genau eine Warnung da.
///
/// Mutationsbeweis: Ruft `load` die Prüfung `hold_to_project_scope` nicht
/// auf, trägt `llm-only` `hold.ask_mode`, `sandbox.profile`,
/// `sandbox.work_mode` und `agent.adapter` bei, und der Test wird rot.
#[test]
fn every_bundled_profile_a_project_names_stays_within_the_project_scope() {
    for (name, _) in BUILTIN_PROFILES
        .iter()
        .filter(|(name, _)| *name != "default")
    {
        let home = Home::new();
        home.write_project_profile(&format!("name = \"{name}\"\n"));
        let resolved = home.resolve(&ProfileSelection::any());

        let builtin = Origin::ProfileBuiltin((*name).to_owned());
        let from_it: Vec<&str> = resolved
            .origins
            .iter()
            .filter(|(_, origin)| **origin == builtin)
            .map(|(path, _)| path.as_str())
            .filter(|path| denied(path))
            .collect();
        assert!(from_it.is_empty(), "{name}: {from_it:?}");

        let profile = resolved.profile(name).expect("in the chain");
        assert!(
            profile.overlay.keys().all(|path| !denied(path)),
            "{name}: {:?}",
            profile.overlay
        );
        assert!(
            profile.rules.inline.iter().all(|rule| {
                matches!(
                    rule.get("action").and_then(toml::Value::as_str),
                    Some("block" | "ask")
                )
            }),
            "{name}: {:?}",
            profile.rules.inline
        );
        assert!(profile.rules.files.is_empty(), "{name}");

        let bundled = ProfileSource::Builtin(name).load().expect("bundled");
        let dropped_keys = bundled.overlay.keys().filter(|path| denied(path)).count();
        let dropped_rules =
            bundled.rules.inline.len() - profile.rules.inline.len() + bundled.rules.files.len();
        assert_eq!(
            ignored(&resolved).len(),
            dropped_keys + dropped_rules,
            "{name}: one warning per ignored item: {:?}",
            resolved.diagnostics
        );
    }
}

/// Dieselbe Grenze für die Regeln, über das Laden: Ein Profil, das das
/// Projekt-Profil nennt, bringt `block` und `ask` mit, nicht `allow`,
/// `redact` oder eine Regeldatei; ein gesperrter Schlüssel unter seinem alten
/// Namen bleibt ebenso draußen. Je Stück eine Warnung, und der Start läuft.
///
/// Mutationsbeweis: Behält `hold_to_project_scope` jede Regel, ohne auf die
/// Aktion zu sehen, stehen `allow` und `redact` im Profil, und der Test wird
/// rot.
#[test]
fn a_named_profile_brings_only_rules_that_block_or_ask() {
    let home = Home::new();
    let work = home.write_profile(
        "work",
        "name = \"work\"\n\n\
         [config.hold]\nask_mode = \"none\"\ntimeout_secs = 5\n\n\
         [rules]\n\
         files = [\"team.yaml\"]\n\
         inline = [\n\
           { action = \"allow\", match = { host = \"open.example\" } },\n\
           { action = \"block\", match = { host = \"**\" } },\n\
           { action = \"ask\", match = { host = \"ask.example\" } },\n\
           { action = \"redact\", match = { host = \"redact.example\" } },\n\
         ]\n",
    );
    let project = home.write_project_profile("name = \"work\"\n");

    let resolved = load(&Sources {
        profiles: vec![ProfileSource::File(work)],
        profile_project: Some(project),
        project_named: Some("work".to_owned()),
        env: home.env(),
        ..Sources::empty()
    })
    .expect("the start continues");

    assert_eq!(resolved.config.hold.ask_mode, AskMode::Ui);
    assert_eq!(resolved.config.hold.timeout_secs, 5);
    let profile = resolved.profile("work").expect("in the chain");
    let actions: Vec<&str> = profile
        .rules
        .inline
        .iter()
        .filter_map(|rule| rule.get("action").and_then(toml::Value::as_str))
        .collect();
    assert_eq!(actions, ["block", "ask"]);
    assert!(profile.rules.files.is_empty());

    let notes = ignored(&resolved);
    assert_eq!(notes.len(), 4, "{notes:?}");
    assert!(
        notes
            .iter()
            .any(|note| note.why.starts_with("hold.ask_mode"))
    );
    assert!(notes.iter().any(|note| note.why.contains("\"allow\"")));
    assert!(notes.iter().any(|note| note.why.contains("\"redact\"")));
    assert!(notes.iter().any(|note| note.why.contains("team.yaml")));
    assert!(notes.iter().all(|note| note.why.contains("work")));
}

/// Wählt der Mensch `llm-only` selbst, gilt es ganz: ohne Projekt-Profil,
/// und auch, wenn das Projekt-Profil dasselbe Profil nennt.
///
/// Mutationsbeweis: Setzt `sources_with_global` den Namen in
/// `project_named` aus dem Projekt-Profil, auch wenn der Mensch ein Profil
/// gewählt hat (`project_wish(None, …)`), bleibt `work_mode` auf `ro`, und
/// der Test wird rot.
#[test]
fn a_profile_the_human_chooses_applies_in_full() {
    for project in [None, Some("name = \"llm-only\"\n")] {
        let home = Home::new();
        home.write_config("[sandbox]\nmode = \"direct\"\nwork_mode = \"ro\"\n");
        if let Some(body) = project {
            home.write_project_profile(body);
        }
        let resolved = home.resolve(&ProfileSelection::named("llm-only"));

        let builtin = Some(Origin::ProfileBuiltin("llm-only".to_owned()));
        assert_eq!(
            resolved.config.sandbox.work_mode,
            WorkMode::Rw,
            "{project:?}"
        );
        assert_eq!(resolved.origin("sandbox.work_mode"), builtin.as_ref());
        assert_eq!(resolved.config.hold.ask_mode, AskMode::None, "{project:?}");
        assert_eq!(resolved.config.agent.adapter, "opencode");
        assert_eq!(resolved.origin("agent.adapter"), builtin.as_ref());
        let profile = resolved.profile("llm-only").expect("in the chain");
        assert_eq!(
            profile.rules,
            ProfileSource::Builtin("llm-only")
                .load()
                .expect("bundled")
                .rules
        );
        assert!(ignored(&resolved).is_empty(), "{:?}", resolved.diagnostics);
    }
}

/// Lädt ein eigenes Profil `work`, das das Projekt-Profil nennt, wie
/// `sources_with_global` es nach der Wahl des Projekts zusammenstellt.
fn load_named(home: &Home, body: &str) -> Resolved {
    let work = home.write_profile("work", &format!("name = \"work\"\n\n{body}"));
    let project = home.write_project_profile("name = \"work\"\n");
    load(&Sources {
        profiles: vec![ProfileSource::File(work)],
        profile_project: Some(project),
        project_named: Some("work".to_owned()),
        env: home.env(),
        ..Sources::empty()
    })
    .unwrap_or_else(|diagnostic| panic!("the start continues: {diagnostic}"))
}

/// Ein Profil, das das Projekt-Profil nennt, stellt nichts unter `sandbox`
/// ein, außer der Untergrenze: kein Backend, keine Einstellung eines
/// Backends, keinen Modus. Jeder gesperrte Schlüssel, hier mit seinem
/// Vorgabewert geschrieben, wird mit genau einer Warnung übergangen, und der
/// Start läuft weiter.
///
/// Mutationsbeweis: Behält `hold_to_project_scope` gesperrte Schlüssel
/// (`if false && denied_for_a_project(path)`), stammt der Wert aus `work`, und
/// der Test wird rot.
#[test]
fn a_named_profile_sets_nothing_under_sandbox() {
    let defaults = toml::Value::try_from(humanitl_config::Config::default()).expect("defaults");
    let mut checked = Vec::new();
    for path in schema::leaf_paths()
        .into_iter()
        .filter(|path| path.starts_with("sandbox.") && denied(path))
    {
        let Some(value) = path
            .split('.')
            .try_fold(&defaults, |value, part| value.get(part))
        else {
            continue;
        };
        let resolved = load_named(&Home::new(), &format!("[config]\n{path} = {value}\n"));
        assert_ne!(
            resolved.origin(path),
            Some(&Origin::ProfileGlobal("work".to_owned())),
            "{path}"
        );
        let notes = ignored(&resolved);
        assert_eq!(notes.len(), 1, "{path}: {notes:?}");
        assert!(notes[0].why.starts_with(path), "{}", notes[0].why);
        checked.push(path);
    }
    assert!(checked.contains(&"sandbox.backend"), "{checked:?}");
    // Die Einstellungen der Backends liegen eine Ebene tiefer, je Backend in
    // einer eigenen Gruppe; mindestens zwei davon sind dabei.
    let groups: std::collections::BTreeSet<&str> = checked
        .iter()
        .filter_map(|path| {
            let parts: Vec<&str> = path.split('.').collect();
            (parts.len() == 3).then(|| parts[1])
        })
        .collect();
    assert!(groups.len() >= 2, "{checked:?}");
}

/// Wählt der Mensch `--profile work`, gilt `work` ganz, samt dem Backend,
/// auch wenn das Projekt-Profil denselben Namen trägt: Die Wahl ist seine,
/// und gleiche Namen allein machen sie nicht zu der des Projekts.
///
/// Mutationsbeweis: Hängt `load` die Grenze an den Namen, den das
/// Projekt-Profil trägt, statt an `project_named`, bleibt das Backend `auto`,
/// und der Test wird rot.
#[test]
fn a_profile_the_human_chooses_may_choose_the_backend() {
    let home = Home::new();
    home.write_profile(
        "work",
        "name = \"work\"\n\n[config.sandbox]\nbackend = \"seatbelt\"\n",
    );
    home.write_project_profile("name = \"work\"\n");

    let resolved = home.resolve(&ProfileSelection::named("work"));
    assert_eq!(
        resolved.config.sandbox.backend,
        humanitl_config::SandboxBackendName::Seatbelt
    );
    assert_eq!(
        resolved.origin("sandbox.backend"),
        Some(&Origin::ProfileGlobal("work".to_owned()))
    );
    assert!(
        resolved
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code.as_str() != "CONFIG_003"),
        "{:?}",
        resolved.diagnostics
    );
}

/// Ein gesperrter Schlüssel unter seinem alten Namen bleibt ebenso draußen,
/// mit genau einer Warnung, die beide Namen nennt.
///
/// Mutationsbeweis: Prüft `hold_to_project_scope` den Pfad, wie er dasteht,
/// statt ihn mit `alias::canonical` auf den heutigen zu bringen, gilt der
/// Wert, und der Test wird rot.
#[test]
fn a_denied_key_under_its_old_name_is_ignored() {
    let resolved = load_named(&Home::new(), "[config.recorder]\nmax_body_bytes = 1024\n");
    assert_eq!(
        resolved.config.limits.recorder_max_body_bytes,
        humanitl_config::Config::default()
            .limits
            .recorder_max_body_bytes
    );
    let notes = ignored(&resolved);
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(
        notes[0].why.starts_with(
            "recorder.max_body_bytes (the old name of limits.recorder_max_body_bytes)"
        ),
        "{}",
        notes[0].why
    );
}

/// Einhängungen bringt ein Profil, das das Projekt-Profil nennt, nicht mit:
/// Der Pfad wird mit einer Warnung übergangen, und der Start läuft weiter.
///
/// Mutationsbeweis: Fehlt der Zweig `is_mount_key` in
/// `denied_for_a_project`, kommt der Pfad ins Laden und bricht es als
/// unbekannter Schlüssel ab; der Test wird rot.
#[test]
fn a_mount_from_a_named_profile_is_ignored() {
    let resolved = load_named(
        &Home::new(),
        "[config.sandbox.mounts]\nextra_rw = [\"/etc\"]\n",
    );
    let notes = ignored(&resolved);
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(
        notes[0].why.starts_with("sandbox.mounts.extra_rw"),
        "{}",
        notes[0].why
    );
}

/// Die Grenze des Projekt-Profils deckt das Backend ganz: Unter `sandbox`
/// ist jeder Schlüssel auf der Projekt-Ebene gesperrt außer der Untergrenze,
/// die über alle Ebenen nur steigt. Ein Profil, das das Projekt-Profil nennt,
/// wählt deshalb weder das Backend noch stellt es eines ein (HUM-431).
#[test]
fn every_sandbox_key_but_the_floor_is_closed_to_a_project() {
    let open: Vec<&str> = schema::leaf_paths()
        .into_iter()
        .filter(|path| path.starts_with("sandbox.") && *path != "sandbox.min_isolation")
        .filter(|path| !denied(path))
        .collect();
    assert!(open.is_empty(), "{open:?}");
}
