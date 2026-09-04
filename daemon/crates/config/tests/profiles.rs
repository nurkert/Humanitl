//! Profile: die mitgelieferten, die eigenen und das des Projekts (HUM-066).
//!
//! Was hier geprüft wird, prüft kein Compiler: dass die zwei mitgelieferten
//! Profile als Datei und eingebettet dasselbe sagen, dass die Präzedenz über
//! sieben Ebenen die Reihenfolge hält, dass ein Overlay feldweise wirkt und
//! dass ein Projekt-Profil nichts setzen darf, was ihm nicht zusteht.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};

use humanitl_config::model::AskMode;
use humanitl_config::{
    BUILTIN_PROFILES, Config, Env, Origin, ProfileSelection, ProfileSource, Resolved,
    available_profiles, builtin_names, resolve,
};
use humanitl_core::{Action, HostName, Method, Scheme, SessionId};
use humanitl_rules::{RequestKey, Verdict, parse_rules};

/// Ein Zuhause auf Zeit: `$XDG_CONFIG_HOME`, ein Projekt, eine Umgebung.
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

    fn resolve(&self, selection: &ProfileSelection, cli: &[(&str, &str)]) -> Resolved {
        let pairs: Vec<(String, String)> = cli
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect();
        match resolve(selection, Some(&self.project()), &self.env(), &pairs) {
            Ok(resolved) => resolved,
            Err(diagnostic) => panic!("resolve failed: {diagnostic}"),
        }
    }

    fn resolve_err(
        &self,
        selection: &ProfileSelection,
        cli: &[(&str, &str)],
    ) -> humanitl_core::Diagnostic {
        let pairs: Vec<(String, String)> = cli
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect();
        match resolve(selection, Some(&self.project()), &self.env(), &pairs) {
            Ok(_) => panic!("resolve was expected to fail"),
            Err(diagnostic) => diagnostic,
        }
    }
}

fn current_uid() -> u32 {
    use std::os::unix::fs::MetadataExt as _;

    std::fs::metadata("/proc/self").map_or(0, |meta| meta.uid())
}

fn repo_profile(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../profiles")
        .join(format!("{name}.toml"))
}

#[test]
fn builtin_profiles_parse() {
    assert_eq!(builtin_names(), vec!["default", "llm-only"]);
    for (name, _) in BUILTIN_PROFILES {
        let profile = ProfileSource::Builtin(name)
            .load()
            .unwrap_or_else(|diagnostic| panic!("{name}: {diagnostic}"));
        assert_eq!(profile.name, *name);
        assert!(
            profile.description.is_some(),
            "{name} has no description; the profile list would show an empty column"
        );
    }
}

#[test]
fn the_bundled_files_and_the_embedded_copies_are_the_same_text() {
    // Sie sind es durch `include_str!`, aber nur, solange beide Namen
    // beieinander stehen. Der Test hält fest, dass keine Datei unter
    // `profiles/` ohne Einbettung bleibt und keine Einbettung ohne Datei.
    for (name, embedded) in BUILTIN_PROFILES {
        let path = repo_profile(name);
        let on_disk = std::fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("{}: {err}", path.display()));
        assert_eq!(&on_disk, embedded, "{name} differs between file and binary");
    }

    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../profiles");
    for entry in std::fs::read_dir(&dir).expect("the profiles directory exists") {
        let path = entry.expect("an entry").path();
        if !path.is_file() || path.extension().is_none_or(|ext| ext != "toml") {
            continue;
        }
        let name = path
            .file_stem()
            .expect("a stem")
            .to_string_lossy()
            .into_owned();
        assert!(
            builtin_names().contains(&name.as_str()),
            "{} is shipped but not embedded; a missing profiles directory would lose it",
            path.display()
        );
    }
}

#[test]
fn the_default_profile_sets_no_value() {
    // Das Profil `default` liegt über `config.toml`. Ein Wert darin — auch
    // einer, der nur den Vorgabewert wiederholt — machte die Datei des Nutzers
    // für diesen Schlüssel wirkungslos, ohne dass er es sähe. Es beschreibt den
    // gängigen Fall deshalb als Kommentar und setzt nichts.
    let profile = ProfileSource::Builtin("default")
        .load()
        .expect("the default profile parses");
    assert!(
        profile.overlay.is_empty(),
        "the bundled default profile sets {:?} and would silence config.toml for it",
        profile.overlay.keys().collect::<Vec<_>>()
    );

    let home = Home::new();
    home.write_config("[hold]\ntimeout_secs = 42\n");
    let resolved = home.resolve(&ProfileSelection::any(), &[]);
    assert_eq!(resolved.config.hold.timeout_secs, 42);
    assert_eq!(resolved.origin("hold.timeout_secs"), Some(&Origin::Global));
    assert_eq!(resolved.config, {
        let mut expected = Config::default();
        expected.hold.timeout_secs = 42;
        expected
    });
}

#[test]
fn precedence_global_profile_project_env_cli() {
    let home = Home::new();
    home.write_config("[hold]\ntimeout_secs = 2\n");
    home.write_profile("work", "[config.hold]\ntimeout_secs = 3\n");
    home.write_project_profile("[config.hold]\ntimeout_secs = 4\n");

    let selection = ProfileSelection::named("work");
    let with_env = home.env().with("HUMANITL_HOLD__TIMEOUT_SECS", "5");
    let all = resolve(
        &selection,
        Some(&home.project()),
        &with_env,
        &[("hold.timeout_secs".to_owned(), "6".to_owned())],
    )
    .expect("the ladder resolves");
    assert_eq!(all.config.hold.timeout_secs, 6);
    assert_eq!(all.origin("hold.timeout_secs"), Some(&Origin::Cli));

    // Jede Ebene darunter wirklich besetzt: ohne die Kommandozeile gewinnt die
    // Umgebung, ohne sie das Projekt, ohne das Projekt das Profil, ohne das
    // Profil die Datei.
    let without_cli =
        resolve(&selection, Some(&home.project()), &with_env, &[]).expect("the ladder resolves");
    assert_eq!(without_cli.config.hold.timeout_secs, 5);
    assert_eq!(
        without_cli.origin("hold.timeout_secs"),
        Some(&Origin::Env("HUMANITL_HOLD__TIMEOUT_SECS".to_owned()))
    );

    let resolved = home.resolve(&selection, &[]);
    assert_eq!(resolved.config.hold.timeout_secs, 4);
    assert!(matches!(
        resolved.origin("hold.timeout_secs"),
        Some(Origin::ProfileProject(_))
    ));

    std::fs::remove_file(home.project().join(".humanitl/profile.toml")).expect("remove");
    let resolved = home.resolve(&selection, &[]);
    assert_eq!(resolved.config.hold.timeout_secs, 3);
    assert_eq!(
        resolved.origin("hold.timeout_secs"),
        Some(&Origin::ProfileGlobal("work".to_owned()))
    );

    std::fs::remove_file(home.dir.path().join("cfg/humanitl/profiles/work.toml")).expect("remove");
    let resolved = home.resolve(&ProfileSelection::any(), &[]);
    assert_eq!(resolved.config.hold.timeout_secs, 2);
    assert_eq!(resolved.origin("hold.timeout_secs"), Some(&Origin::Global));
}

#[test]
fn the_profile_chain_names_every_layer_that_spoke() {
    let home = Home::new();
    home.write_profile("work", "[config.ui]\ntheme = \"light\"\n");
    let project = home.write_project_profile("[config.hold]\ntimeout_secs = 7\n");

    let resolved = home.resolve(&ProfileSelection::named("work"), &[]);
    assert_eq!(
        resolved.profile_chain(),
        vec![
            Origin::ProfileBuiltin("default".to_owned()),
            Origin::ProfileGlobal("work".to_owned()),
            Origin::ProfileProject(project),
        ]
    );
}

#[test]
fn project_profile_inherits_named() {
    let home = Home::new();
    home.write_project_profile("name = \"llm-only\"\n\n[config.hold]\ntimeout_secs = 7\n");

    let resolved = home.resolve(&ProfileSelection::any(), &[]);
    assert_eq!(resolved.config.hold.ask_mode, AskMode::None);
    assert_eq!(resolved.config.hold.timeout_secs, 7);
    assert_eq!(
        resolved.origin("hold.ask_mode"),
        Some(&Origin::ProfileBuiltin("llm-only".to_owned()))
    );
    assert!(matches!(
        resolved.origin("hold.timeout_secs"),
        Some(Origin::ProfileProject(_))
    ));
}

/// Ein Projekt-Profil darf mit `name` nur ein mitgeliefertes Profil wählen.
///
/// Ohne diese Grenze setzte ein geklontes Repository über den Umweg `name`
/// jeden Schlüssel, den ihm die Projekt-Ebene verwehrt: `agent.command`
/// bestimmt den Prozess in der Sandbox, `sandbox.profile` ihre Einhängefläche.
#[test]
fn a_project_may_only_choose_a_bundled_profile() {
    let home = Home::new();
    home.write_profile(
        "loose",
        "name = \"loose\"\n\n[config.agent]\ncommand = [\"/bin/sh\", \"-c\", \"id\"]\n\n\
         [config.sandbox]\nprofile = \"wide-open\"\n",
    );
    home.write_project_profile("name = \"loose\"\n");

    let resolved = home.resolve(&ProfileSelection::any(), &[]);
    assert_eq!(
        resolved.config.agent.command, None,
        "the repository chose a profile of the user and got its agent command"
    );
    assert_eq!(resolved.config.sandbox.profile, "default");
    assert_eq!(resolved.origin("agent.command"), Some(&Origin::Default));
    assert_eq!(
        resolved.profile_chain(),
        vec![
            Origin::ProfileBuiltin("default".to_owned()),
            Origin::ProfileProject(home.project().join(".humanitl/profile.toml")),
        ]
    );
    let note = resolved
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code.as_str() == "CONFIG_009")
        .expect("the ignored wish of the project is reported");
    assert!(note.why.contains("loose"), "{}", note.why);

    // Auf der Kommandozeile gewählt gilt dasselbe Profil sehr wohl: Dort
    // entscheidet der Mensch, nicht das Repository.
    let resolved = home.resolve(&ProfileSelection::named("loose"), &[]);
    assert_eq!(
        resolved.config.agent.command,
        Some(vec!["/bin/sh".to_owned(), "-c".to_owned(), "id".to_owned()])
    );
}

/// Das Projektverzeichnis ist `sandbox.work_dir`, nicht das aktuelle.
#[test]
fn the_work_directory_decides_which_project_profile_applies() {
    let home = Home::new();
    let elsewhere = home.dir.path().join("elsewhere");
    std::fs::create_dir_all(elsewhere.join(".humanitl")).expect("mkdir");
    std::fs::write(
        elsewhere.join(".humanitl/profile.toml"),
        "[config.hold]\ntimeout_secs = 77\n",
    )
    .expect("write");
    // Das aktuelle Verzeichnis trägt ein anderes Profil; es darf nicht wirken.
    home.write_project_profile("[config.hold]\ntimeout_secs = 5\n");

    let resolved = home.resolve(
        &ProfileSelection::any(),
        &[("sandbox.work_dir", elsewhere.to_str().expect("a path"))],
    );
    assert_eq!(resolved.config.hold.timeout_secs, 77);
    assert_eq!(
        resolved.origin("hold.timeout_secs"),
        Some(&Origin::ProfileProject(
            elsewhere.join(".humanitl/profile.toml")
        ))
    );

    // Ohne `sandbox.work_dir` bleibt es beim übergebenen Verzeichnis.
    let resolved = home.resolve(&ProfileSelection::any(), &[]);
    assert_eq!(resolved.config.hold.timeout_secs, 5);
}

/// Ein Name wird zu einem Pfad; die Prüfung steht deshalb an beiden Stellen.
#[test]
fn a_profile_name_never_reaches_a_file_outside_the_profile_directory() {
    let home = Home::new();
    // Der Name zeigt aus dem Profilverzeichnis heraus genau auf diese Datei:
    // <dir>/cfg/humanitl/profiles/../../../outside.toml ist <dir>/outside.toml.
    let outside = home.dir.path().join("outside.toml");
    std::fs::write(&outside, "[config.hold]\ntimeout_secs = 13\n").expect("write");
    let paths = humanitl_config::Paths::new(home.env());
    let escaping = "../../../outside";
    assert!(
        paths.profile_path(escaping).exists(),
        "the test would prove nothing if the path did not reach a file"
    );

    // Weder über die strenge Auflösung …
    let diagnostic = home.resolve_err(&ProfileSelection::named(escaping), &[]);
    assert_eq!(diagnostic.code.as_str(), "CONFIG_003");

    // … noch über die Ebenen selbst, die aus einem Namen einen Pfad machen.
    assert_eq!(
        humanitl_config::profile_layers(&paths, escaping),
        vec![ProfileSource::Builtin("default")]
    );
    assert!(!humanitl_config::profile_exists(&paths, escaping));
}

/// Ein Profil, das es gibt, sich aber nicht lesen lässt, ist ein Profil.
#[test]
fn a_broken_profile_still_counts_as_a_profile() {
    let home = Home::new();
    home.write_profile("work", "[config.hold\n");
    let paths = humanitl_config::Paths::new(home.env());

    assert!(
        humanitl_config::profile_exists(&paths, "work"),
        "a broken profile that counts as absent changes what --profile means"
    );
    assert_eq!(
        home.resolve_err(&ProfileSelection::named("work"), &[])
            .code
            .as_str(),
        "CONFIG_001"
    );
}

#[test]
fn a_name_on_the_command_line_beats_the_name_of_the_project() {
    let home = Home::new();
    home.write_project_profile("name = \"llm-only\"\n");
    home.write_profile("work", "[config.hold]\ntimeout_secs = 33\n");

    let resolved = home.resolve(&ProfileSelection::named("work"), &[]);
    assert_eq!(resolved.config.hold.timeout_secs, 33);
    assert_eq!(resolved.config.hold.ask_mode, AskMode::Ui);
}

#[test]
fn a_field_of_a_group_does_not_reset_its_siblings() {
    // Der Fallstrick des Issues: `[config.hold]` mit nur `timeout_secs` im
    // Projekt darf `ask_mode` aus dem benannten Profil nicht auf die Vorgabe
    // zurückwerfen.
    let home = Home::new();
    home.write_project_profile("name = \"llm-only\"\n\n[config.hold]\ntimeout_secs = 9\n");

    let resolved = home.resolve(&ProfileSelection::any(), &[]);
    assert_eq!(resolved.config.hold.ask_mode, AskMode::None);
    assert_eq!(resolved.config.hold.timeout_secs, 9);
}

#[test]
fn project_profile_cannot_mount() {
    let home = Home::new();
    home.write_project_profile("[config.sandbox.mounts]\nextra_rw = [\"/etc\"]\n");

    let diagnostic = home.resolve_err(&ProfileSelection::any(), &[]);
    assert_eq!(diagnostic.code.as_str(), "CONFIG_003");
    assert!(
        diagnostic.why.contains("mount host paths"),
        "{}",
        diagnostic.why
    );

    home.write_project_profile("[config.sandbox.mounts]\nextra_ro = [\"/etc\"]\n");
    assert_eq!(
        home.resolve_err(&ProfileSelection::any(), &[])
            .code
            .as_str(),
        "CONFIG_003"
    );
}

#[test]
fn project_profile_cannot_choose_the_sandbox_profile_or_the_agent_command() {
    for line in [
        "[config.sandbox]\nprofile = \"test\"\n",
        "[config.agent]\ncommand = [\"sh\"]\n",
        "[config.hold]\nask_mode = \"terminal\"\n",
    ] {
        let home = Home::new();
        home.write_project_profile(line);
        let diagnostic = home.resolve_err(&ProfileSelection::any(), &[]);
        assert_eq!(diagnostic.code.as_str(), "CONFIG_003", "{line}");
        assert!(
            diagnostic.why.contains("project profile"),
            "{line}: {}",
            diagnostic.why
        );
    }
}

#[test]
fn a_project_profile_may_not_bring_rules() {
    let home = Home::new();
    home.write_project_profile(
        "[rules]\ninline = [{ action = \"allow\", match = { host = \"**\" } }]\n",
    );
    let diagnostic = home.resolve_err(&ProfileSelection::any(), &[]);
    assert_eq!(diagnostic.code.as_str(), "CONFIG_003");
    assert!(
        diagnostic.why.contains("cloned repository"),
        "{}",
        diagnostic.why
    );
}

#[test]
fn unknown_profile_config_001() {
    let home = Home::new();
    let diagnostic = home.resolve_err(&ProfileSelection::named("nowhere"), &[]);
    assert_eq!(diagnostic.code.as_str(), "CONFIG_001");
    assert!(diagnostic.why.contains("llm-only"), "{}", diagnostic.why);
    assert!(diagnostic.fix.is_some(), "the diagnostic names no way out");
}

#[test]
fn a_profile_name_is_never_a_path() {
    let home = Home::new();
    let diagnostic = home.resolve_err(&ProfileSelection::named("../../etc/passwd"), &[]);
    assert_eq!(diagnostic.code.as_str(), "CONFIG_003");
}

#[test]
fn a_file_that_shadows_a_bundled_profile_is_reported() {
    let home = Home::new();
    home.write_profile(
        "llm-only",
        "name = \"llm-only\"\n\n[config.hold]\nask_mode = \"ui\"\n",
    );

    let resolved = home.resolve(&ProfileSelection::named("llm-only"), &[]);
    assert_eq!(resolved.config.hold.ask_mode, AskMode::Ui);
    let note = resolved
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code.as_str() == "CONFIG_008")
        .expect("the shadowed bundled profile is reported");
    assert!(note.why.contains("llm-only"), "{}", note.why);

    // Eine wortgleiche Kopie ändert nichts und wird nicht gemeldet.
    let bundled = std::fs::read_to_string(repo_profile("llm-only")).expect("the bundled file");
    home.write_profile("llm-only", &bundled);
    let resolved = home.resolve(&ProfileSelection::named("llm-only"), &[]);
    assert!(
        resolved
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code.as_str() != "CONFIG_008"),
        "{:?}",
        resolved.diagnostics
    );
}

#[test]
fn a_project_profile_of_another_account_is_a_warning_not_a_refusal() {
    let home = Home::new();
    home.write_project_profile("[config.hold]\ntimeout_secs = 12\n");
    let env = home.env().with_uid(current_uid().wrapping_add(1));

    let resolved = resolve(&ProfileSelection::any(), Some(&home.project()), &env, &[])
        .expect("a foreign owner does not stop the start");
    assert_eq!(resolved.config.hold.timeout_secs, 12);
    let warning = resolved
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code.as_str() == "CONFIG_007")
        .expect("the foreign owner is reported");
    assert_eq!(warning.severity, humanitl_core::Severity::Warning);
}

#[test]
fn the_profile_list_names_the_bundled_and_the_own_ones() {
    let home = Home::new();
    home.write_profile("work", "description = \"the office\"\n");
    let paths = humanitl_config::Paths::new(home.env());

    let (summaries, diagnostics) = available_profiles(&paths);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let names: Vec<&str> = summaries.iter().map(|entry| entry.name.as_str()).collect();
    assert_eq!(names, vec!["default", "llm-only", "work"]);
    assert_eq!(
        summaries[2].description.as_deref(),
        Some("the office"),
        "the list shows what a profile is for"
    );
    assert!(matches!(summaries[0].source, ProfileSource::Builtin(_)));
}

#[test]
fn a_broken_profile_is_listed_as_a_diagnostic_and_not_swallowed() {
    let home = Home::new();
    home.write_profile("broken", "[hold]\ntimeout_secs = 1\n");
    let paths = humanitl_config::Paths::new(home.env());

    let (summaries, diagnostics) = available_profiles(&paths);
    assert_eq!(summaries.len(), 3, "{summaries:?}");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code.as_str(), "CONFIG_002");
    let broken = summaries
        .iter()
        .find(|entry| entry.name == "broken")
        .expect("the broken profile has a row");
    assert!(broken.broken, "{broken:?}");
    assert!(broken.description.is_none());
}

/// Eine unlesbare Datei, die ein mitgeliefertes Profil verdeckt, darf in der
/// Liste nicht als brauchbares mitgeliefertes Profil stehen.
///
/// `layer` nimmt in dieser Lage die Datei, und jeder Aufruf endet mit
/// `CONFIG_001`. Eine Liste, die stattdessen `bundled` zeigte, lüde genau zu
/// diesem Aufruf ein.
#[test]
fn a_broken_file_that_shadows_a_bundled_profile_is_not_advertised_as_usable() {
    let home = Home::new();
    home.write_profile("llm-only", "[config.hold\n");
    let paths = humanitl_config::Paths::new(home.env());

    let (summaries, diagnostics) = available_profiles(&paths);
    let entry = summaries
        .iter()
        .find(|entry| entry.name == "llm-only")
        .expect("the shadowed name still has a row");
    assert!(
        entry.broken,
        "the list claims the bundled profile is usable: {entry:?}"
    );
    assert!(
        matches!(entry.source, ProfileSource::File(_)),
        "the file decides, so the file is what the list must name: {entry:?}"
    );
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code.as_str(), "CONFIG_001");

    // Und der Aufruf, zu dem die Liste sonst eingeladen hätte, scheitert.
    assert_eq!(
        home.resolve_err(&ProfileSelection::named("llm-only"), &[])
            .code
            .as_str(),
        "CONFIG_001"
    );
}

/// Der Name eines Profils kommt ohne `name`-Schlüssel aus dem Dateistamm — und
/// ein Stamm ist genauso wenig von selbst ein Name wie ein getippter.
#[test]
fn a_file_stem_that_is_no_name_is_refused_like_a_typed_one() {
    let home = Home::new();
    let path = home.write_profile("Work.Profile", "description = \"grossgeschrieben\"\n");
    let source = ProfileSource::File(path);

    let diagnostic = source
        .load()
        .expect_err("a file stem outside the character set is no profile name");
    assert_eq!(diagnostic.code.as_str(), "CONFIG_003");
    assert!(
        diagnostic.why.contains("Work.Profile"),
        "{}",
        diagnostic.why
    );

    // Und die Liste zeigt ihn als das, was er ist, statt ihn zu verschweigen.
    let paths = humanitl_config::Paths::new(home.env());
    let (summaries, diagnostics) = available_profiles(&paths);
    assert!(summaries.iter().any(|entry| entry.broken), "{summaries:?}");
    assert_eq!(diagnostics.len(), 1);
}

#[test]
fn a_profile_resolves_its_rule_files_against_itself() {
    let home = Home::new();
    let path = home.write_profile("work", "[rules]\nfiles = [\"team.yaml\"]\n");

    let resolved = home.resolve(&ProfileSelection::named("work"), &[]);
    let profile = resolved
        .profile("work")
        .expect("the profile is in the chain");
    assert_eq!(
        profile.rule_files(),
        vec![path.parent().expect("a directory").join("team.yaml")]
    );
}

#[test]
fn a_profile_that_does_not_parse_names_the_file_the_line_and_the_way_out() {
    let home = Home::new();
    home.write_profile("llm-only", "name = \"llm-only\"\n[config.hold\n");
    let diagnostic = home.resolve_err(&ProfileSelection::named("llm-only"), &[]);

    assert_eq!(diagnostic.code.as_str(), "CONFIG_001");
    assert!(
        diagnostic.why.contains("llm-only.toml"),
        "{}",
        diagnostic.why
    );
    assert!(diagnostic.why.contains("line 2"), "{}", diagnostic.why);
    assert!(
        diagnostic.why.contains("Nothing starts"),
        "{}",
        diagnostic.why
    );
    assert!(
        diagnostic.why.contains("bundled profile llm-only"),
        "a shadowing file names the way back: {}",
        diagnostic.why
    );
}

#[test]
fn the_rules_document_carries_the_version_of_the_parser() {
    // `humanitl-config` darf nicht auf `humanitl-rules` zeigen und schreibt die
    // Fassungsnummer deshalb selbst hin. Hier stehen beide Seiten nebeneinander.
    assert_eq!(
        humanitl_config::RULES_DOCUMENT_VERSION,
        humanitl_rules::RULES_VERSION
    );
}

#[test]
fn llm_only_blocks_everything_but_passthrough() {
    let home = Home::new();
    let resolved = home.resolve(&ProfileSelection::named("llm-only"), &[]);
    assert_eq!(resolved.config.hold.ask_mode, AskMode::None);

    let profile = resolved
        .profile("llm-only")
        .expect("the profile is in the chain");
    let document = profile
        .rules_document()
        .expect("llm-only brings its own rules");
    let (mut rules, warnings) =
        parse_rules(&document).unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"));
    assert_eq!(rules.len(), 1);
    assert!(warnings.is_empty(), "{warnings:?}");

    // Der Adapter stellt seine Durchreichregel voran; sie steht hier als
    // dieselbe YAML, die `OpenCodeAdapter::llm_passthrough` baut
    // (`backlog/CONVENTIONS.md` 4.21), damit dieser Test keine Kante nach
    // außen braucht.
    let (passthrough, _) = parse_rules(PASSTHROUGH_YAML).expect("the passthrough rule parses");
    rules.prepend_bundled(passthrough.iter().cloned());

    let session = SessionId::new();
    let now = chrono::Utc::now();
    let llm = HostName::parse("ollama.lan").expect("a host");
    let key = RequestKey::new(
        &llm,
        &Method::POST,
        "/v1/chat/completions",
        Scheme::Http,
        11434,
    );
    assert_eq!(rules.evaluate(&key, now, session).action(), Action::Allow);

    let elsewhere = HostName::parse("github.com").expect("a host");
    let key = RequestKey::new(&elsewhere, &Method::GET, "/", Scheme::Https, 443);
    let verdict = rules.evaluate(&key, now, session);
    assert_eq!(verdict.action(), Action::Block);
    assert!(
        !matches!(verdict, Verdict::Default),
        "nothing is left to a person in llm-only"
    );

    // Auch ein Pfad am Sprachmodell, der keine Inferenz macht, wird geblockt
    // statt durchgereicht: die Durchreiche nennt Endpunkte, keine Fläche.
    let key = RequestKey::new(&llm, &Method::POST, "/api/pull", Scheme::Http, 11434);
    assert_eq!(rules.evaluate(&key, now, session).action(), Action::Block);
}

/// Die Durchreichregel, wie der OpenCode-Adapter sie für `http://ollama.lan:11434` baut.
const PASSTHROUGH_YAML: &str = "\
version: 1
rules:
  - id: 01920000-0000-7000-8000-0000000000ff
    action: allow
    match:
      host: ollama.lan
      method: [POST, GET]
      path_prefixes: [/api/chat, /api/generate, /v1/chat/completions, /v1/models]
      scheme: http
      port: 11434
    expires: never
    allow_private: true
    passthrough_llm: true
    note: \"LLM passthrough. Logged, never held.\"
";
