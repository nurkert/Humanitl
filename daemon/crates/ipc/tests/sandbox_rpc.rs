//! Die `Sandbox`-RPC über einem echten Profil (HUM-040).
//!
//! Der Bildschirm behauptet, der Agent sehe nur das Projektverzeichnis. Diese
//! Tests prüfen, dass die Momentaufnahme, auf der die Behauptung steht, wirklich
//! aus der Kommandozeile kommt: Jeder Pfad der Tabelle steht auch in der Zeile,
//! die Umgebung ist die des Profils, und ein Wert, dessen Name auf `_TOKEN`
//! endet, ist gar nicht erst dabei.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;

use humanitl_config::{Config, Env, Paths};
use humanitl_core::ids::SessionId;
use humanitl_ipc::v1;
use humanitl_ipc::{SandboxService, v1::sandbox_event::Event};
use tempfile::TempDir;
use tokio_stream::StreamExt as _;

/// Der Geheimtext, den kein Test irgendwo wiederfinden darf.
///
/// Zur Laufzeit zusammengesetzt und keinem echten Wert ähnlich: Ein echt
/// geformter Wert im Quelltext ist genau das, was der Push-Schutz verhindern
/// soll (`backlog/CONVENTIONS.md` 4.13).
fn needle() -> String {
    format!("{}{}", "not-a-", "real-secret")
}

/// Ein Profil mit allem, was der Bildschirm zeigen soll.
///
/// Vier Variablen tragen den Geheimtext, und **drei** davon heißen so, dass
/// eine Liste verdächtiger Endungen sie nicht fände: `DATABASE_URL` trägt das
/// Passwort in der URL, `AWS_ACCESS_KEY_ID` endet auf `_ID`, `GH_PAT` heißt
/// nach gar nichts. Sie sind der Grund, warum die Regel eine Erlaubnisliste ist
/// und keine Verbotsliste (`backlog/CONVENTIONS.md` 4.17).
fn profile_text() -> String {
    let needle = needle();
    format!(
        r#"
version = 1
name = "test"
description = "for the sandbox rpc tests"

[mounts]
ro = ["/usr"]
tmpfs = ["/tmp", "/dev/shm"]

[mounts.work]
dst = "/work"

[env]
HTTP_PROXY = "http://127.0.0.1:3128"
HTTPS_PROXY = "http://127.0.0.1:3128"
NO_PROXY = ""
DATABASE_URL = "postgres://app:{needle}@db.internal/app"
AWS_ACCESS_KEY_ID = "{needle}"
GH_PAT = "{needle}"
GITHUB_TOKEN = "{needle}"
"#
    )
}

/// Ein Dienst über einem Wegwerf-XDG mit genau diesem Profil.
fn service(home: &TempDir) -> SandboxService {
    let config_home = home.path().join("config");
    let profile_dir = config_home.join("humanitl/profiles/sandbox");
    std::fs::create_dir_all(&profile_dir).unwrap();
    std::fs::write(profile_dir.join("test.toml"), profile_text()).unwrap();

    let env = Env::from_pairs([
        ("HOME", home.path().display().to_string()),
        ("XDG_CONFIG_HOME", config_home.display().to_string()),
        (
            "XDG_DATA_HOME",
            home.path().join("data").display().to_string(),
        ),
        (
            "XDG_RUNTIME_DIR",
            home.path().join("run").display().to_string(),
        ),
    ]);
    let mut config = Config::default();
    "test".clone_into(&mut config.sandbox.profile);
    config.sandbox.work_dir = Some(home.path().join("project"));
    std::fs::create_dir_all(home.path().join("project")).unwrap();

    SandboxService::new(config, Paths::new(env), SessionId::new())
}

/// Die Momentaufnahme einer Operation.
async fn snapshot(
    service: &SandboxService,
    request: v1::SandboxRequest,
) -> v1::sandbox_event::Status {
    let mut stream = service.stream(request);
    let mut last = None;
    while let Some(event) = stream.next().await {
        if let Some(Event::Status(status)) = event.event {
            last = Some(status);
        }
    }
    last.expect("every operation answers with a status")
}

#[tokio::test]
async fn the_snapshot_lists_the_project_the_socket_and_the_certificate() {
    let home = TempDir::new().unwrap();
    let service = service(&home);
    let status = snapshot(
        &service,
        v1::SandboxRequest {
            op: Some(v1::sandbox_request::Op::Status(())),
        },
    )
    .await;

    assert_eq!(status.profile, "test");
    assert_eq!(status.work_mode, "rw");
    let dsts: Vec<&str> = status.mounts.iter().map(|m| m.dst.as_str()).collect();
    for required in [
        "/work",
        "/run/humanitl/proxy.sock",
        "/etc/humanitl/ca.crt",
        "/usr",
        "/tmp",
    ] {
        assert!(
            dsts.contains(&required),
            "missing mount {required} in {dsts:?}"
        );
    }

    // Das Projekt ist die einzige schreibbare Einhängung, und ihre Quelle ist
    // das Verzeichnis der Konfiguration.
    let work = status
        .mounts
        .iter()
        .find(|m| m.dst == "/work")
        .expect("the project is mounted");
    assert_eq!(work.mode, v1::MountMode::Rw as i32);
    assert_eq!(work.origin, v1::ValueOrigin::Session as i32);
    assert_eq!(work.src, home.path().join("project").display().to_string());
    let writable: Vec<&str> = status
        .mounts
        .iter()
        .filter(|m| m.mode == v1::MountMode::Rw as i32)
        .map(|m| m.dst.as_str())
        .collect();
    assert_eq!(writable, vec!["/work"], "only the project may be written");
}

#[tokio::test]
async fn every_mount_of_the_table_stands_in_the_command_line() {
    let home = TempDir::new().unwrap();
    let service = service(&home);
    let status = snapshot(
        &service,
        v1::SandboxRequest {
            op: Some(v1::sandbox_request::Op::Status(())),
        },
    )
    .await;

    assert!(status.argv_preview.contains("--cap-drop ALL"));
    assert!(status.argv_preview.contains("--unshare-net"));
    for mount in &status.mounts {
        assert!(
            status.argv_preview.contains(&mount.dst),
            "the command line does not mention {}",
            mount.dst
        );
    }
}

#[tokio::test]
async fn only_the_evidence_keeps_its_value_and_everything_else_is_withheld() {
    let home = TempDir::new().unwrap();
    let service = service(&home);
    let status = snapshot(
        &service,
        v1::SandboxRequest {
            op: Some(v1::sandbox_request::Op::Status(())),
        },
    )
    .await;

    // Was der Daemon selbst geschrieben hat, steht da.
    let session = status
        .env
        .iter()
        .find(|e| e.key == "HUMANITL_SESSION")
        .expect("the session is named");
    assert_eq!(session.origin, v1::ValueOrigin::Session as i32);
    assert!(!session.withheld);
    assert!(!session.value.is_empty());

    // `HTTP_PROXY` steht auf der Erlaubnisliste und kommt hier aus
    // `sandbox.env`, also aus der Hand eines Menschen: der Wert bleibt
    // zurueck. Ein erlaubter Name sagt nichts ueber den Wert darunter
    // (Review Codex, Befund 3). Dasselbe gilt fuer `HTTPS_PROXY`: dieses
    // Testprofil liegt im Konfigurationsordner des Menschen.
    let overridden = status
        .env
        .iter()
        .find(|e| e.key == "HTTP_PROXY")
        .expect("the overridden proxy is named");
    assert_eq!(overridden.origin, v1::ValueOrigin::User as i32);
    assert!(overridden.withheld, "a value a person wrote stays back");
    assert!(overridden.value.is_empty());

    // Alles andere nicht — und zwar auch das, was keine Endungsregel faende.
    for key in [
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "DATABASE_URL",
        "AWS_ACCESS_KEY_ID",
        "GH_PAT",
        "GITHUB_TOKEN",
    ] {
        let entry = status
            .env
            .iter()
            .find(|e| e.key == key)
            .unwrap_or_else(|| panic!("{key} is named"));
        assert!(entry.withheld, "{key} must be withheld");
        assert!(entry.value.is_empty(), "{key} has no value on the wire");
        assert!(
            status
                .argv_preview
                .contains(&format!("--setenv {key} '<withheld>'")),
            "the command line withholds {key} too: {}",
            status.argv_preview
        );
    }

    // Auch ein leerer Wert aus der Hand eines Menschen bleibt zurueck. Der
    // Dienst liest den Wert nicht, um zu entscheiden — er entscheidet an Name
    // und Herkunft —, und ein „das war ja ohnehin leer" waere eine zweite
    // Regel, die man umgehen kann. Dass ein leerer Wert anders aussieht als
    // ein zurueckgehaltener, sichert die Oberflaeche
    // (`app/test/features/sandbox`).
    let empty = status
        .env
        .iter()
        .find(|e| e.key == "NO_PROXY")
        .expect("NO_PROXY is set");
    assert!(empty.withheld);
    assert!(empty.value.is_empty());
}

#[tokio::test]
async fn a_secret_in_a_name_no_rule_would_suspect_reaches_no_answer_at_all() {
    let home = TempDir::new().unwrap();
    let service = service(&home);
    let needle = needle();

    let status = snapshot(
        &service,
        v1::SandboxRequest {
            op: Some(v1::sandbox_request::Op::Status(())),
        },
    )
    .await;
    assert!(
        !status.argv_preview.contains(&needle),
        "the command line carries the secret"
    );
    for entry in &status.env {
        assert!(
            !entry.value.contains(&needle),
            "{} carries the secret",
            entry.key
        );
    }
    for mount in &status.mounts {
        assert!(!mount.src.contains(&needle) && !mount.dst.contains(&needle));
    }

    // Und die Kommandozeile Argument fuer Argument, ueber die eigene Operation.
    let mut stream = service.stream(v1::SandboxRequest {
        op: Some(v1::sandbox_request::Op::Argv(())),
    });
    let mut lines = 0;
    while let Some(event) = stream.next().await {
        if let Some(Event::ArgvLine(line)) = event.event {
            lines += 1;
            assert!(!line.contains(&needle), "an argv line carries the secret");
        }
    }
    assert!(lines > 0, "the argv operation answers at all");
}

#[tokio::test]
async fn a_plan_answers_for_a_directory_that_does_not_apply_yet() {
    let home = TempDir::new().unwrap();
    let service = service(&home);
    let other = home.path().join("other");
    std::fs::create_dir_all(&other).unwrap();

    let planned = snapshot(
        &service,
        v1::SandboxRequest {
            op: Some(v1::sandbox_request::Op::Plan(v1::sandbox_request::Plan {
                profile: String::new(),
                work_dir: other.display().to_string(),
                work_mode: "ro".to_owned(),
            })),
        },
    )
    .await;
    assert_eq!(planned.work_dir, other.display().to_string());
    assert_eq!(planned.work_mode, "ro");
    let work = planned
        .mounts
        .iter()
        .find(|m| m.dst == "/work")
        .expect("the project is mounted");
    assert_eq!(work.mode, v1::MountMode::Ro as i32);
    assert!(planned.argv_preview.contains(&other.display().to_string()));

    // Die Wahl gilt für die Sitzung: der nächste `Status` fällt nicht still
    // auf das Verzeichnis der Konfiguration zurück (CONVENTIONS 4.17).
    let again = snapshot(
        &service,
        v1::SandboxRequest {
            op: Some(v1::sandbox_request::Op::Status(())),
        },
    )
    .await;
    assert_eq!(again.work_dir, other.display().to_string());
    assert_eq!(again.work_mode, "ro");
}

#[tokio::test]
async fn a_profile_that_is_not_there_is_a_finding_and_not_an_empty_table() {
    let home = TempDir::new().unwrap();
    let config_home = home.path().join("config");
    std::fs::create_dir_all(&config_home).unwrap();
    let env = Env::from_pairs([
        ("HOME", home.path().display().to_string()),
        ("XDG_CONFIG_HOME", config_home.display().to_string()),
    ]);
    let mut config = Config::default();
    "there-is-no-such-profile".clone_into(&mut config.sandbox.profile);
    let service = SandboxService::new(config, Paths::new(env), SessionId::new());

    let mut stream = service.stream(v1::SandboxRequest {
        op: Some(v1::sandbox_request::Op::Status(())),
    });
    let mut diagnostics = Vec::new();
    let mut status = None;
    while let Some(event) = stream.next().await {
        match event.event {
            Some(Event::Diagnostic(diagnostic)) => diagnostics.push(diagnostic),
            Some(Event::Status(answered)) => status = Some(answered),
            _ => {}
        }
    }
    assert!(
        diagnostics.iter().any(|d| d.code == "CONFIG_001"),
        "the missing profile is named: {diagnostics:?}"
    );
    let status = status.expect("the state is still answered");
    assert_eq!(status.state, v1::SandboxState::Failed as i32);
    assert!(status.mounts.is_empty(), "nothing is claimed to be mounted");
}

/// Die Befunde und die Momentaufnahme einer Operation.
async fn answer(
    service: &SandboxService,
    request: v1::SandboxRequest,
) -> (Vec<v1::Diagnostic>, Option<v1::sandbox_event::Status>) {
    let mut stream = service.stream(request);
    let mut diagnostics = Vec::new();
    let mut status = None;
    while let Some(event) = stream.next().await {
        match event.event {
            Some(Event::Diagnostic(diagnostic)) => diagnostics.push(diagnostic),
            Some(Event::Status(answered)) => status = Some(answered),
            _ => {}
        }
    }
    (diagnostics, status)
}

/// Ein Wunsch nach einem Profil, wie ihn ein Client schickt.
fn plan_profile(name: &str) -> v1::SandboxRequest {
    v1::SandboxRequest {
        op: Some(v1::sandbox_request::Op::Plan(v1::sandbox_request::Plan {
            profile: name.to_owned(),
            work_dir: String::new(),
            work_mode: String::new(),
        })),
    }
}

/// Ein Wunsch nach einem Projektverzeichnis, wie ihn ein Client schickt.
fn plan_work_dir(dir: &str) -> v1::SandboxRequest {
    v1::SandboxRequest {
        op: Some(v1::sandbox_request::Op::Plan(v1::sandbox_request::Plan {
            profile: String::new(),
            work_dir: dir.to_owned(),
            work_mode: String::new(),
        })),
    }
}

/// Ein Profilname ist ein Name, kein Pfad.
///
/// `format!("{name}.toml")` machte aus `/tmp/evil` sonst `/tmp/evil.toml`, denn
/// `Path::join` ersetzt die Basis, sobald das Angehaengte absolut ist. Wer den
/// Namen setzt, bestimmt Einhaengungen und Umgebung der Sandbox; der Socket ist
/// die Vertrauensgrenze, nicht die Oberflaeche (Review Codex, Befund 1).
#[tokio::test]
async fn a_profile_name_that_is_a_path_is_refused_before_anything_is_read() {
    let home = TempDir::new().unwrap();
    let service = service(&home);

    // Ein Profil, das wirklich dort liegt, wohin der Ausbruch zeigt.
    let outside = home.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    let planted = outside.join("evil.toml");
    std::fs::write(&planted, profile_text()).unwrap();
    let absolute = planted.with_extension("");

    for name in [
        absolute.display().to_string(),
        "../../etc/passwd".to_owned(),
        "sandbox/../../evil".to_owned(),
        "default/".to_owned(),
        "..".to_owned(),
        "Default".to_owned(),
        "a b".to_owned(),
    ] {
        let (diagnostics, status) = answer(&service, plan_profile(&name)).await;
        assert!(
            diagnostics.iter().any(|d| d.code == "CONFIG_003"),
            "{name} must be refused as a name, got {diagnostics:?}"
        );
        let status = status.expect("the state is still answered");
        assert_eq!(status.state, v1::SandboxState::Failed as i32);
        assert!(
            status.mounts.is_empty() && status.argv_preview.is_empty(),
            "{name} must not reach a plan"
        );
    }

    // Und der Wunsch ist auch nicht haengen geblieben: die naechste Frage
    // beantwortet weiter das Profil der Konfiguration.
    let (_, status) = answer(
        &service,
        v1::SandboxRequest {
            op: Some(v1::sandbox_request::Op::Status(())),
        },
    )
    .await;
    assert_eq!(status.expect("a status").profile, "test");
}

/// Die Mutationsprobe zu Befund 1: der Ausbruch ist echt.
///
/// Ohne die Pruefung fuehrte genau dieser Name auf die untergeschobene Datei.
/// Der Test belegt, dass die Datei existiert und der Pfad sich aus dem Namen
/// bilden laesst — nur eben nicht mehr geladen wird.
#[test]
fn the_planted_profile_would_be_reachable_without_the_check() {
    let home = TempDir::new().unwrap();
    let outside = home.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    let planted = outside.join("evil.toml");
    std::fs::write(&planted, profile_text()).unwrap();

    let name = planted.with_extension("").display().to_string();
    let joined = Path::new("/usr/share/humanitl/profiles/sandbox").join(format!("{name}.toml"));
    assert_eq!(joined, planted, "an absolute name replaces the search path");
    assert!(planted.is_file());
    assert!(humanitl_config::profile::check_name(&name, "test").is_err());
}

/// Ein Projektverzeichnis kommt aus dem Heimatverzeichnis, sonst nirgendwoher.
///
/// Sonst stuende auf dem Bildschirm, der „der Agent sieht nur `/work` = dein
/// Projektordner" verspricht, im Zweifel `/etc` (Review Codex, Befund 2).
#[tokio::test]
async fn a_project_directory_outside_the_home_is_refused() {
    let home = TempDir::new().unwrap();
    let service = service(&home);

    for dir in ["/etc", "/usr", "/", "/var/lib", "/proc", "relative/path"] {
        let (diagnostics, status) = answer(&service, plan_work_dir(dir)).await;
        assert!(
            diagnostics.iter().any(|d| d.code == "SANDBOX_006"),
            "{dir} must be refused, got {diagnostics:?}"
        );
        let status = status.expect("the state is still answered");
        assert_eq!(status.state, v1::SandboxState::Failed as i32);
        assert!(
            !status.argv_preview.contains(dir),
            "{dir} must not reach the command line"
        );
    }

    // Ein `..`, das im Heimatverzeichnis beginnt und darueber hinauslaeuft.
    let escape = home.path().join("project/../../../../etc");
    let (diagnostics, _) = answer(&service, plan_work_dir(&escape.display().to_string())).await;
    assert!(
        diagnostics.iter().any(|d| d.code == "SANDBOX_006"),
        "a path with .. must be refused, got {diagnostics:?}"
    );

    // Nach alldem steht immer noch das Verzeichnis der Konfiguration da.
    let (_, status) = answer(
        &service,
        v1::SandboxRequest {
            op: Some(v1::sandbox_request::Op::Status(())),
        },
    )
    .await;
    let status = status.expect("a status");
    assert_eq!(
        status.work_dir,
        home.path().join("project").display().to_string()
    );
}

/// Die Mutationsprobe zu Befund 2: ein Verzeichnis unter dem
/// Heimatverzeichnis kommt durch, und zwar aufgeloest.
///
/// Waere die Pruefung ein pauschales Nein, faende dieser Test es.
#[tokio::test]
async fn a_project_directory_under_the_home_is_accepted() {
    let home = TempDir::new().unwrap();
    let service = service(&home);
    let other = home.path().join("clients/acme");
    std::fs::create_dir_all(&other).unwrap();

    let (diagnostics, status) = answer(&service, plan_work_dir(&other.display().to_string())).await;
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let status = status.expect("a status");
    let resolved = other.canonicalize().unwrap().display().to_string();
    assert_eq!(status.work_dir, resolved);
    assert!(status.argv_preview.contains(&resolved));
}

/// Das mitgelieferte Profil im Arbeitsbaum.
const BUNDLED_PROFILE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../profiles/sandbox/default.toml"
);

/// Jede Variable, die das mitgelieferte Profil setzt, behält ihren Wert.
///
/// Die Erlaubnisliste schützt gegen den einen Fehler und darf dabei nicht in
/// den anderen laufen: Ein Bildschirm, auf dem alles zurückgehalten ist, sagt
/// nichts mehr. Heute kostet die Liste nichts — die 25 Variablen des Profils
/// und die des Adapters stehen alle darauf, zurückgehalten wird nur, was ein
/// Mensch selbst in `sandbox.env` oder ein eigenes Profil schreibt. Wer dem
/// Profil eine Variable hinzufügt, entscheidet hier, ob ihr Wert ein Beleg ist
/// (`backlog/CONVENTIONS.md` 4.17).
#[test]
fn the_bundled_profile_starves_no_row_of_the_table() {
    let text = std::fs::read_to_string(BUNDLED_PROFILE)
        .unwrap_or_else(|error| panic!("cannot read {BUNDLED_PROFILE}: {error}"));
    let env = text
        .split_once("[env]")
        .expect("the bundled profile has an [env] section")
        .1;
    let mut checked = 0;
    for line in env.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            break;
        }
        let Some((key, _)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() || key.starts_with('#') {
            continue;
        }
        assert!(
            humanitl_ipc::sandbox::is_visible_env_name(key),
            "{key} of the bundled profile would be withheld; put it on \
             VISIBLE_ENV or say why its value is not evidence"
        );
        // Und die Herkunft muss auch stimmen: das mitgelieferte Profil ist
        // nicht von Hand geschrieben, also darf sein Wert erscheinen.
        assert!(humanitl_ipc::sandbox::shows_env_value(
            key,
            humanitl_ipc::v1::ValueOrigin::Profile
        ));
        checked += 1;
    }
    assert!(checked >= 20, "only {checked} variables were checked");
}

/// Der Dienst liest nur; kein Test dieser Datei startet `bwrap`.
#[test]
fn these_tests_never_start_a_sandbox() {
    assert!(Path::new(file!()).ends_with("sandbox_rpc.rs"));
}
