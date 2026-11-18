//! Die Projektwahl als Ebene der Auflösung (HUM-532): über dem
//! Projekt-Profil, unter Umgebung und Kommandozeile, Herkunft
//! `project-choice`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

use humanitl_config::{Env, Origin, ProjectChoice, Sources, load};

const CONFIG: &str = r#"
[llm]
active = "home"

[llm.servers.home]
endpoint = "http://192.168.1.20:11434"

[llm.servers.work]
endpoint = "http://192.168.1.21:11434"

[llm.servers.lab]
endpoint = "http://192.168.1.22:11434"

[llm.servers.run]
endpoint = "http://192.168.1.23:11434"
"#;

struct Scratch {
    dir: tempfile::TempDir,
}

impl Scratch {
    fn new() -> Self {
        Self {
            dir: tempfile::tempdir().unwrap(),
        }
    }

    fn write(&self, name: &str, text: &str) -> PathBuf {
        let path = self.dir.path().join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        path
    }
}

fn choice(pairs: &[(&str, &str)]) -> ProjectChoice {
    ProjectChoice::from_pairs(pairs.iter().copied()).unwrap()
}

/// Der Wert der Wahl schlägt das Projekt-Profil und verliert gegen
/// `HUMANITL_*` und gegen die Kommandozeile.
#[test]
fn the_choice_beats_the_project_profile_and_loses_to_env_and_flags() {
    let scratch = Scratch::new();
    let global = scratch.write("config.toml", CONFIG);
    let project = scratch.write(
        "project/.humanitl/profile.toml",
        "[config.llm]\nserver = \"work\"\n",
    );
    let base = Sources {
        global_toml: Some(global),
        profile_project: Some(project),
        ..Sources::empty()
    };

    let chosen = load(
        &base
            .clone()
            .with_project_choice(Some(choice(&[("llm.server", "lab")]))),
    )
    .unwrap();
    assert_eq!(chosen.config.llm.choice.as_ref().unwrap().name, "lab");
    assert_eq!(chosen.origin("llm.server"), Some(&Origin::ProjectChoice));
    assert_eq!(Origin::ProjectChoice.kind(), "project-choice");

    let from_env = load(
        &base
            .clone()
            .with_project_choice(Some(choice(&[("llm.server", "lab")])))
            .with_env(Env::from_pairs([("HUMANITL_LLM__SERVER", "run")])),
    )
    .unwrap();
    assert_eq!(from_env.config.llm.choice.as_ref().unwrap().name, "run");
    assert!(matches!(
        from_env.origin("llm.server"),
        Some(Origin::Env(_))
    ));

    let from_flag = load(
        &base
            .with_project_choice(Some(choice(&[("llm.server", "lab")])))
            .with_cli([("llm.server", "run")]),
    )
    .unwrap();
    assert_eq!(from_flag.origin("llm.server"), Some(&Origin::Cli));
}

/// Ein Wert, den das Projekt-Profil nicht setzen darf, darf die Wahl
/// setzen, wenn er auf ihrer Liste steht: Ihre Grenze ist die Liste, nicht
/// `x-project-scope`.
#[test]
fn the_choice_may_set_a_key_the_project_profile_may_not() {
    let resolved = load(&Sources::empty().with_project_choice(Some(choice(&[
        ("hold.ask_mode", "terminal"),
        ("sandbox.work_mode", "ro"),
        ("sandbox.mode", "direct"),
    ]))))
    .unwrap();
    assert_eq!(
        resolved.origin("hold.ask_mode"),
        Some(&Origin::ProjectChoice)
    );
    assert_eq!(
        resolved.origin("sandbox.mode"),
        Some(&Origin::ProjectChoice)
    );
    assert_eq!(
        resolved.config.sandbox.work_mode,
        humanitl_config::WorkMode::Ro
    );
}

/// Beim Laden: Eine Wahl, die die Untergrenze senkt, ist `CONFIG_024`, nicht
/// ein still übergangener Wert; eine, die sie hebt, gilt.
#[test]
fn a_choice_below_the_floor_is_config_024_at_load() {
    let scratch = Scratch::new();
    let global = scratch.write("config.toml", "[sandbox]\nmin_isolation = \"namespaces\"\n");
    let base = Sources {
        global_toml: Some(global),
        ..Sources::empty()
    };
    let lowered = load(
        &base
            .clone()
            .with_project_choice(Some(choice(&[("sandbox.min_isolation", "policy")]))),
    )
    .unwrap_err();
    assert_eq!(lowered.code.as_str(), "CONFIG_024");

    let raised =
        load(&base.with_project_choice(Some(choice(&[("sandbox.min_isolation", "kernel")]))))
            .unwrap();
    assert_eq!(
        raised.config.sandbox.min_isolation,
        humanitl_config::MinIsolation::Kernel
    );
    assert_eq!(
        raised.origin("sandbox.min_isolation"),
        Some(&Origin::ProjectChoice)
    );
}
