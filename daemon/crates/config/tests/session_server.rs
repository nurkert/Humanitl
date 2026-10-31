//! Ein Projekt, ein Profil oder ein Lauf wählt einen gespeicherten
//! Modellserver (HUM-400).
//!
//! Die Rangfolge Lauf vor Projekt vor Profil vor `llm.active`, die Ablehnung
//! eines Namens ohne Eintrag ohne stillen Rückfall, und dass der Schlüssel im
//! Projekt-Profil nur auswählt, nie selbst einen Endpunkt oder Schlüssel
//! festlegt.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

use humanitl_config::servers::{ServerChoice, ServerSource};
use humanitl_config::{ProfileSource, Resolved, Sources, load};

/// Zwei gespeicherte Server mit je eigenem Schlüssel; aktiv ist `home`.
const CONFIG: &str = r#"
[llm]
active = "home"

[llm.servers.home]
endpoint = "http://192.168.1.20:11434"
models = ["qwen3"]
api_key_ref = "secret-service:humanitl/home"

[llm.servers.work]
endpoint = "https://llm.work.example/v1"
models = ["big"]
api_key_ref = "secret-service:humanitl/work"

[llm.servers.cloud]
endpoint = "https://api.example.com/v1"
api_key_ref = "secret-service:humanitl/cloud"
"#;

struct Scratch {
    dir: tempfile::TempDir,
}

impl Scratch {
    fn new() -> Self {
        Self {
            dir: tempfile::tempdir().expect("tempdir"),
        }
    }

    fn write(&self, name: &str, text: &str) -> PathBuf {
        let path = self.dir.path().join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("mkdir");
        }
        std::fs::write(&path, text).expect("write");
        path
    }

    fn config(&self) -> PathBuf {
        self.write("config.toml", CONFIG)
    }

    /// Ein Profil der Sitzung, das `llm.server` setzt.
    fn profile(&self, server: &str) -> PathBuf {
        self.write(
            "profiles/team.toml",
            &format!("[config.llm]\nserver = \"{server}\"\n"),
        )
    }

    /// Ein Projekt-Profil mit diesem Text unter `[config]`.
    fn project(&self, body: &str) -> PathBuf {
        self.write(".humanitl/profile.toml", &format!("[config]\n{body}\n"))
    }
}

/// Lädt mit den gesetzten Ebenen; `None` lässt eine Ebene weg.
fn resolve(
    scratch: &Scratch,
    profile: Option<&str>,
    project: Option<&str>,
    run: Option<&str>,
) -> Resolved {
    let sources = Sources {
        global_toml: Some(scratch.config()),
        profiles: profile
            .map(|server| ProfileSource::File(scratch.profile(server)))
            .into_iter()
            .collect(),
        profile_project: project
            .map(|server| scratch.project(&format!("llm.server = \"{server}\""))),
        cli: run
            .map(|server| ("llm.server".to_owned(), server.to_owned()))
            .into_iter()
            .collect(),
        ..Sources::empty()
    };
    load(&sources).unwrap_or_else(|diagnostic| panic!("{diagnostic}"))
}

fn chosen(resolved: &Resolved) -> &ServerChoice {
    resolved
        .config
        .llm
        .choice
        .as_ref()
        .expect("a saved server applies")
}

fn endpoint(resolved: &Resolved) -> String {
    resolved
        .config
        .llm
        .endpoint
        .as_ref()
        .map(ToString::to_string)
        .unwrap_or_default()
}

#[test]
fn without_a_choice_the_active_server_applies_as_the_default() {
    let scratch = Scratch::new();
    let resolved = resolve(&scratch, None, None, None);
    let choice = chosen(&resolved);
    assert_eq!(choice.name, "home");
    assert_eq!(choice.source, ServerSource::Default);
    assert!(choice.refusal().is_none());
    assert_eq!(endpoint(&resolved), "http://192.168.1.20:11434/");
}

#[test]
fn a_profile_beats_the_active_server() {
    let scratch = Scratch::new();
    let resolved = resolve(&scratch, Some("cloud"), None, None);
    let choice = chosen(&resolved);
    assert_eq!(choice.name, "cloud");
    assert_eq!(choice.source, ServerSource::Profile("team".to_owned()));
    assert_eq!(endpoint(&resolved), "https://api.example.com/v1");
    assert_eq!(
        resolved.config.llm.api_key_ref.as_deref(),
        Some("secret-service:humanitl/cloud"),
        "the key reference follows the chosen server"
    );
}

/// Mutationsbeweis: Wer den Schlüssel `llm.server` des Projekts übergeht,
/// bekommt hier `home` (oder `cloud` aus dem Profil) und fällt.
#[test]
fn a_project_beats_a_profile_and_the_active_server() {
    let scratch = Scratch::new();
    let resolved = resolve(&scratch, Some("cloud"), Some("work"), None);
    let choice = chosen(&resolved);
    assert_eq!(choice.name, "work");
    let ServerSource::Project(path) = &choice.source else {
        panic!("the source is the project profile: {:?}", choice.source);
    };
    assert!(
        path.ends_with(".humanitl/profile.toml"),
        "{}",
        path.display()
    );
    assert_eq!(endpoint(&resolved), "https://llm.work.example/v1");
    assert_eq!(resolved.config.llm.models, vec!["big".to_owned()]);
    assert_eq!(
        resolved.config.llm.api_key_ref.as_deref(),
        Some("secret-service:humanitl/work")
    );
    assert!(
        resolved.config.llm.key_goes_with_endpoint(),
        "the chosen server's key goes with the chosen server's endpoint"
    );
}

#[test]
fn a_run_beats_the_project() {
    let scratch = Scratch::new();
    let resolved = resolve(&scratch, Some("cloud"), Some("work"), Some("home"));
    let choice = chosen(&resolved);
    assert_eq!(choice.name, "home");
    assert_eq!(choice.source, ServerSource::Run);
    assert_eq!(endpoint(&resolved), "http://192.168.1.20:11434/");
}

/// Mutationsbeweis: Ein stiller Rückfall auf `llm.active` ließe hier den
/// Endpunkt von `home` stehen und die Wahl ohne Ablehnung.
#[test]
fn an_unknown_name_applies_no_server_and_refuses_without_a_fallback() {
    let scratch = Scratch::new();
    let resolved = resolve(&scratch, None, Some("gone"), None);
    let choice = resolved
        .config
        .llm
        .choice
        .as_ref()
        .expect("the choice stands, unknown");
    assert_eq!(choice.name, "gone");
    assert!(!choice.known);
    assert!(
        resolved.config.llm.endpoint.is_none(),
        "no endpoint applies, in particular not the active one"
    );
    assert!(
        resolved.config.llm.api_key_ref.is_none(),
        "no key goes anywhere"
    );
    assert!(resolved.config.llm.models.is_empty());

    let refusal = choice.refusal().expect("a start refuses");
    assert_eq!(refusal.code.as_str(), "CONFIG_021");
    assert_eq!(refusal.severity, humanitl_core::Severity::Error);
    for part in ["project profile", "gone", "cloud, home, work", "llm.server"] {
        assert!(refusal.why.contains(part), "{part}: {}", refusal.why);
    }
    assert!(
        resolved
            .diagnostics
            .iter()
            .any(|found| found.code.as_str() == "CONFIG_021" && found.why.contains("gone")),
        "loading says it too, as a warning: {:?}",
        resolved.diagnostics
    );
}

#[test]
fn an_unknown_name_from_a_profile_names_the_profile() {
    let scratch = Scratch::new();
    let resolved = resolve(&scratch, Some("nope"), None, None);
    let refusal = resolved
        .config
        .llm
        .choice
        .as_ref()
        .and_then(ServerChoice::refusal)
        .expect("a start refuses");
    assert!(refusal.why.contains("the profile team"), "{}", refusal.why);
}

#[test]
fn a_project_cannot_define_an_endpoint_a_server_or_a_key() {
    let scratch = Scratch::new();
    for body in [
        "llm.endpoint = \"https://evil.example/v1\"",
        "llm.api_key_ref = \"secret-service:humanitl/home\"",
        "llm.active = \"cloud\"",
        "llm.models = [\"x\"]",
        "[config.llm.servers.evil]\nendpoint = \"https://evil.example/v1\"",
    ] {
        let sources = Sources {
            global_toml: Some(scratch.config()),
            profile_project: Some(scratch.project(body)),
            ..Sources::empty()
        };
        let refused = load(&sources).expect_err(body);
        assert_eq!(refused.code.as_str(), "CONFIG_003", "{body}: {refused}");
    }
}

/// Ein Projekt, das einen eigenen Namen erfindet, bekommt keinen Server: Der
/// Name wählt nur aus dem, was der Nutzer gespeichert hat.
#[test]
fn a_name_the_user_did_not_save_never_becomes_a_server() {
    let scratch = Scratch::new();
    let resolved = resolve(&scratch, None, Some("evil"), None);
    assert!(resolved.config.llm.endpoint.is_none());
    assert!(!resolved.config.llm.servers.contains_key("evil"));
}

/// Mutationsbeweis: Prüft `key_goes_with_endpoint` den global aktiven
/// Server statt des gewählten, geht der Schlüssel von `work` nicht mit dem
/// Endpunkt von `work` — und umgekehrt nie der von `home` dorthin.
#[test]
fn only_the_chosen_server_gets_its_key() {
    let scratch = Scratch::new();
    let resolved = resolve(&scratch, None, Some("work"), None);
    let mut llm = resolved.config.llm;
    assert!(llm.key_goes_with_endpoint());

    // Der Verweis des global aktiven Servers neben dem Endpunkt des
    // gewählten: Er geht nicht hinaus.
    llm.api_key_ref = Some("secret-service:humanitl/home".to_owned());
    assert!(!llm.key_goes_with_endpoint());
}

#[test]
fn a_higher_endpoint_shows_no_saved_server_and_withholds_the_key() {
    let scratch = Scratch::new();
    let sources = Sources {
        global_toml: Some(scratch.config()),
        profile_project: Some(scratch.project("llm.server = \"work\"")),
        cli: vec![(
            "llm.endpoint".to_owned(),
            "http://127.0.0.1:8080/v1".to_owned(),
        )],
        ..Sources::empty()
    };
    let resolved = load(&sources).unwrap();
    assert!(resolved.config.llm.choice.is_none());
    assert!(resolved.config.llm.api_key_ref.is_none());
}

#[test]
fn naming_default_on_a_flat_file_keeps_its_values() {
    let scratch = Scratch::new();
    let config = scratch.write(
        "flat.toml",
        "[llm]\nendpoint = \"http://192.168.1.20:11434\"\napi_key_ref = \"file:llm\"\n",
    );
    let sources = Sources {
        global_toml: Some(config),
        profile_project: Some(scratch.project("llm.server = \"default\"")),
        ..Sources::empty()
    };
    let resolved = load(&sources).unwrap();
    let choice = chosen(&resolved);
    assert_eq!(choice.name, "default");
    assert!(choice.known);
    assert_eq!(resolved.config.llm.api_key_ref.as_deref(), Some("file:llm"));
}

/// `llm.server` in `config.toml` ist eine Vorgabe wie `llm.active`, aber der
/// Befund nennt den Schlüssel, der wirklich gewählt hat (HUM-400, Review).
#[test]
fn a_choice_in_config_toml_names_its_own_key() {
    let scratch = Scratch::new();
    let config = scratch.write(
        "server.toml",
        &CONFIG.replace("active = \"home\"", "active = \"home\"\nserver = \"gone\""),
    );
    let resolved = load(&Sources {
        global_toml: Some(config),
        ..Sources::empty()
    })
    .unwrap();
    let choice = resolved
        .config
        .llm
        .choice
        .as_ref()
        .expect("the choice stands");
    assert_eq!(choice.source, ServerSource::Config);
    assert_eq!(choice.source.kind(), "default");
    let refusal = choice.refusal().expect("an unsaved name refuses");
    assert!(
        refusal
            .why
            .starts_with("llm.server in config.toml names the model server gone"),
        "{}",
        refusal.why
    );
}
