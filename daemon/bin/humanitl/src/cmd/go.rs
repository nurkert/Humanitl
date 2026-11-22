//! `humanitl go [AGENT]`: ein Agent in einem Projektordner, mit einem Befehl,
//! bis zu seinem Prompt (HUM-496, #8).
//!
//! `go` teilt Auflösen, Start, Anschluss und Signale mit `run`
//! ([`crate::cmd::run::launch`]); hier steht nur, was `go` anders macht:
//!
//! 1. **Der Agent ist ein Wort** (`humanitl go claude`), alles andere sind
//!    Schalter (ADR-0024 Punkt 7): `--dir`, `--disposable`, `--rw`, `--ro`,
//!    `--kernel`. Sie gelten für diesen einen Aufruf. Der Agent reist als
//!    `agent.adapter` in `Start.cli_overrides`, der Ordnermodus in
//!    `Start.sandbox_mode` und `Start.work_mode`, die Kernel-Stufe als
//!    Untergrenze `sandbox.min_isolation = "kernel"`. Ohne Wort und ohne
//!    Schalter bleiben diese Felder leer, und der Daemon löst mit der
//!    Projektwahl auf (HUM-532); für ein neues Projekt ist das der Vorschlag
//!    (HUM-536). `go` ruft `Projects.save_choice` nie: Kein Schalter ändert
//!    die gespeicherte Wahl.
//! 2. **Der Ordner ist das aktuelle Verzeichnis** oder `--dir`, nicht der in
//!    `config.toml` gespeicherte Ordner; `--work` und eine Variable
//!    `HUMANITL_SANDBOX__WORK_DIR` gelten als Wunsch dieses Aufrufs weiter.
//! 3. **Das Terminal geht an den Agenten**, sobald die Sitzung läuft, auch
//!    ohne eigenes Kommando. Ohne Terminal auf `stdin` oder mit `--json`
//!    liest `go` nur mit, wie `run`.
//! 4. **Die Ladeanimation** ([`crate::splash`]) steht, solange die Sitzung
//!    startet, mindestens eine Sekunde und nicht länger als der Start.
//! 5. **Am Ende die Frage**, was mit den wartenden Änderungen geschieht
//!    ([`crate::cmd::changes::review`]): übernehmen, verwerfen oder warten
//!    lassen. Nur auf einem Terminal und nur, wenn der Daemon nach dem Lauf
//!    Änderungen gemeldet hat.
//!
//! Der Befehl entscheidet nichts selbst (ADR-018): Ob die Kernel-Stufe geht,
//! welcher Ordnermodus gilt und was die Projektwahl sagt, entscheidet der
//! Daemon. Die Kennung des Agenten prüft `go` vor dem Verbinden gegen
//! dieselbe Liste der eingebauten Adapter, die auch der Daemon prüft
//! ([`AdapterRegistry::builtin`]), damit ein Tippfehler ohne Daemon und ohne
//! Start auffällt.

use std::ffi::OsString;
use std::io::IsTerminal as _;
use std::path::{Path, PathBuf};

use humanitl_config::{AskMode, Config};
use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, FixAction, Severity};
use humanitl_ipc::client::Client;
use humanitl_ipc::session::{AGENT_ADAPTER_KEY, SANDBOX_MIN_ISOLATION_KEY};
use humanitl_ipc::v1;
use humanitl_sandbox::agent::AdapterRegistry;
use humanitl_sandbox::{WORK_DIR_KEY, WorkDirSource};

use crate::cli::{GoArgs, RunArgs};
use crate::cmd::run::{Launch, launch};
use crate::cmd::{Context, EXIT_USER, Failure, status_diagnostic};
use crate::splash::{self, Palette};

/// Der Schlüssel des Schreibmodus, den `--rw` und `--ro` für diesen Aufruf
/// setzen.
const WORK_MODE_KEY: &str = "sandbox.work_mode";

/// Der Schlüssel des Ordnermodus, den `--disposable`, `--rw` und `--ro` für
/// diesen Aufruf festlegen.
const SANDBOX_MODE_KEY: &str = "sandbox.mode";

/// Die Variable, mit der ein Aufruf den Ordner ohne `--dir` wünscht.
const WORK_DIR_ENV: &str = "HUMANITL_SANDBOX__WORK_DIR";

/// Ab dieser Minor kennt der Daemon `Projects` (HUM-532).
const PROJECTS_MINOR: u32 = 45;

/// Führt `humanitl go` aus.
///
/// # Errors
///
/// `CLI_004` für einen unbekannten Agenten, für `--dir` neben `--work` und
/// für ein Wort oder einen Schalter, dem ein globaler Schalter widerspricht,
/// `SANDBOX_006` für ein `--dir`, das kein Ordner ist, sonst wie
/// `humanitl run`.
pub async fn run(ctx: &Context, args: &GoArgs) -> Result<u8, Failure> {
    let program = args.agent.as_deref().map(agent_program).transpose()?;
    let (start_ctx, folder) = for_this_start(ctx, args)?;
    let ctx = &start_ctx;
    let run_args = RunArgs {
        cmd: command(program, &args.cmd),
        disposable: args.disposable,
        direct: args.rw || args.ro,
        yes: args.yes,
    };
    let mut state = Launch {
        go: true,
        folder,
        ..Launch::default()
    };
    let code = Box::pin(launch(ctx, &run_args, &mut state)).await?;
    if state.changes_waiting && asks(ctx.render.is_json()) {
        // Der Exit-Code bleibt der des Agenten: Eine Übernahme, die der
        // Daemon ablehnt, steht als Befund da, und die Änderungen warten
        // weiter.
        match crate::cmd::changes::review(ctx).await {
            Ok(None) => {}
            // Ein Signal an der Frage: nichts angefasst, Exit wie bei jedem
            // anderen Signal `128 + n`.
            Ok(Some(number)) => return Ok(128_u8.saturating_add(number)),
            Err(found) => ctx.render.diagnostic(&found.diagnostic),
        }
    }
    Ok(code)
}

/// Das Programm des Agenten `id`, wie sein Adapter es nennt.
///
/// # Errors
///
/// `CLI_004` mit der Liste der Adapter, wenn es keinen mit dieser Kennung
/// gibt. Ein Ordner gleichen Namens ändert daran nichts: Das Wort ist immer
/// der Agent, der Ordner kommt nur über `--dir`.
fn agent_program(id: &str) -> Result<&'static str, Failure> {
    let registry = AdapterRegistry::builtin();
    if let Some(adapter) = registry.get(id) {
        return Ok(adapter.program());
    }
    let known = registry.ids();
    let mut found = Diagnostic::builder(codes::CLI_004, Severity::Error).why(format!(
        "there is no agent {id:?}; humanitl go knows: {}. A folder is chosen with --dir PATH.",
        known.join(", ")
    ));
    if let Some(first) = known.first() {
        found = found.fix(FixAction::CopyCommand(format!("humanitl go {first}")));
    }
    Err(Failure::with_exit(found.build(), EXIT_USER))
}

/// Das Kommando des Starts: mit einem Agenten sind die Wörter nach `--`
/// seine Argumente (`humanitl go claude -- --resume`), ohne einen ersetzen
/// sie ihn wie bei `humanitl run`. Mit einem Agenten und ohne Wörter bleibt
/// das Kommando leer, und der Daemon startet den Agenten mit dem eigenen
/// Start seines Adapters (Schlüssel, Loopback); ein `agent.command` einer
/// tieferen Ebene gilt dann nicht (`humanitl_ipc::session`, #8).
fn command(program: Option<&str>, words: &[OsString]) -> Vec<OsString> {
    match program {
        Some(program) if !words.is_empty() => std::iter::once(OsString::from(program))
            .chain(words.iter().cloned())
            .collect(),
        _ => words.to_vec(),
    }
}

/// Der Kontext dieses Starts und der Ordner, den er gewählt hat.
///
/// Der Ordner (`--dir`, sonst das aktuelle Verzeichnis) steht als
/// `sandbox.work_dir` unter den Werten der Kommandozeile, damit auch die
/// Auflösung des Clients das Projekt-Profil genau dieses Ordners liest und
/// nicht das des in `config.toml` gespeicherten. Ein Wunsch dieses Aufrufs
/// (`--work`, `HUMANITL_SANDBOX__WORK_DIR`, eine Datei aus `--config`) gilt
/// ohne `--dir` weiter; dann bleibt der Ordner `None`, und der Lauf löst ihn
/// auf wie `run`. Die Schalter stehen ebenso als Werte der Kommandozeile da.
///
/// # Errors
///
/// `CLI_004` für `--dir` neben `--work` und für einen globalen Schalter, der
/// dem Wort oder einem Schalter von `go` widerspricht, `SANDBOX_006` für ein
/// `--dir`, das kein Ordner ist.
fn for_this_start(
    ctx: &Context,
    args: &GoArgs,
) -> Result<(Context, Option<(PathBuf, WorkDirSource)>), Failure> {
    let folder = match &args.dir {
        Some(dir) => {
            if ctx.cli_config.iter().any(|(key, _)| key == WORK_DIR_KEY) {
                return Err(contradiction(
                    "--dir and --work both name the project folder; name it once, with --dir",
                    &format!("humanitl go --dir {}", dir.display()),
                ));
            }
            // `--dir` ist ein Schalter dieses Aufrufs; seine Herkunft nennt
            // der Daemon in einem `SANDBOX_006` wie die eines `--work`.
            Some((project_dir(&ctx.cwd, dir)?, WorkDirSource::Flag))
        }
        None if folder_wished(ctx) => None,
        None => Some((ctx.cwd.clone(), WorkDirSource::CurrentDir)),
    };
    let mut cli_config = ctx.cli_config.clone();
    if let Some((dir, _)) = &folder {
        set(&mut cli_config, WORK_DIR_KEY, &dir.display().to_string());
    }
    let switch = if args.ro {
        Some(("direct", "ro"))
    } else if args.rw {
        Some(("direct", "rw"))
    } else if args.disposable {
        Some(("disposable", "rw"))
    } else {
        None
    };
    if let Some((mode, work_mode)) = switch {
        agree(&cli_config, SANDBOX_MODE_KEY, mode, "the folder switch")?;
        agree(&cli_config, WORK_MODE_KEY, work_mode, "the folder switch")?;
        set(&mut cli_config, WORK_MODE_KEY, work_mode);
    }
    if args.kernel {
        // Kein Widerspruch möglich: Über alle Ebenen gilt die strengste
        // Untergrenze.
        set(&mut cli_config, SANDBOX_MIN_ISOLATION_KEY, "kernel");
    }
    if let Some(agent) = &args.agent {
        agree(&cli_config, AGENT_ADAPTER_KEY, agent, "the agent word")?;
        set(&mut cli_config, AGENT_ADAPTER_KEY, agent);
    }
    let start_ctx = Context {
        env: ctx.env.clone(),
        paths: ctx.paths.clone(),
        // Eine Datei aus `--config` ist relativ zum Verzeichnis des Aufrufs,
        // nicht zu dem aus `--dir`.
        config_file: ctx.config_file.as_ref().map(|file| ctx.cwd.join(file)),
        cwd: folder
            .as_ref()
            .map_or_else(|| ctx.cwd.clone(), |(dir, _)| dir.clone()),
        cli_config,
        profile: ctx.profile.clone(),
        profile_means: ctx.profile_means,
        render: ctx.render,
        source_lock: ctx.source_lock.clone(),
    };
    Ok((start_ctx, folder))
}

/// Ob dieser Aufruf den Ordner ohne `--dir` schon wünscht: `--work`, die
/// Variable `HUMANITL_SANDBOX__WORK_DIR` oder eine Datei aus `--config`.
fn folder_wished(ctx: &Context) -> bool {
    ctx.cli_config.iter().any(|(key, _)| key == WORK_DIR_KEY)
        || ctx.env.get(WORK_DIR_ENV).is_some()
        || ctx.config_file.is_some()
}

/// `CLI_004`, wenn ein globaler Schalter `key` auf einen anderen Wert setzt
/// als `what` von `go`: Beide gelten für diesen Aufruf, und keiner soll den
/// anderen still überstimmen.
fn agree(pairs: &[(String, String)], key: &str, value: &str, what: &str) -> Result<(), Failure> {
    match pairs.iter().find(|(given, _)| given == key) {
        Some((_, given)) if given != value => Err(contradiction(
            &format!(
                "{what} of humanitl go sets {key} = {value:?}, and a global switch sets it to \
                 {given:?}; name it once"
            ),
            "humanitl go --help",
        )),
        _ => Ok(()),
    }
}

/// Der Befund für zwei Angaben dieses Aufrufs, die sich widersprechen.
fn contradiction(why: &str, fix: &str) -> Failure {
    Failure::with_exit(
        Diagnostic::builder(codes::CLI_004, Severity::Error)
            .why(why.to_owned())
            .fix(FixAction::CopyCommand(fix.to_owned()))
            .build(),
        EXIT_USER,
    )
}

/// Setzt `key` unter den Werten der Kommandozeile auf `value`.
fn set(pairs: &mut Vec<(String, String)>, key: &str, value: &str) {
    pairs.retain(|(given, _)| given != key);
    pairs.push((key.to_owned(), value.to_owned()));
}

/// Der Ordner aus `--dir`, gegen das aktuelle Verzeichnis gelesen und
/// kanonisiert (unter macOS also `/private/var` statt `/var`).
///
/// # Errors
///
/// `SANDBOX_006`, wenn es den Pfad nicht gibt oder er kein Ordner ist. Der
/// Daemon prüft den Ordner beim Start noch einmal; hier fällt ein Tippfehler
/// auf, bevor irgendetwas verbindet.
fn project_dir(cwd: &Path, dir: &Path) -> Result<PathBuf, Failure> {
    let given = cwd.join(dir);
    let refused = |why: String, fix: Option<FixAction>| {
        Failure::with_exit(
            Diagnostic::builder(codes::SANDBOX_006, Severity::Error)
                .why(why)
                .maybe_fix(fix)
                .build(),
            EXIT_USER,
        )
    };
    match std::fs::canonicalize(&given) {
        Ok(path) if path.is_dir() => Ok(path),
        Ok(path) => Err(refused(
            format!(
                "the project directory from --dir is {}: it is not a directory",
                path.display()
            ),
            path.parent().map(|parent| {
                FixAction::CopyCommand(format!("humanitl go --dir {}", parent.display()))
            }),
        )),
        Err(err) => Err(refused(
            format!(
                "the project directory from --dir is {}: it cannot be opened ({err})",
                given.display()
            ),
            None,
        )),
    }
}

/// Eine Zeile für ein Projekt, das das Register noch nicht kennt: Es startet
/// mit den Vorgaben des Vorschlags, bis es den Erststart-Bildschirm gibt
/// (#9).
///
/// Ob der Ordner bekannt ist, sagt der Daemon (`Projects.get`); ein älterer
/// Daemon oder einer ohne Register bekommt keine Zeile.
pub(crate) async fn announce_new_project(
    ctx: &Context,
    client: &mut Client,
    work_dir: &Path,
    daemon_minor: u32,
) {
    if daemon_minor < PROJECTS_MINOR {
        return;
    }
    let asked = client
        .projects(v1::ProjectsRequest {
            op: Some(v1::projects_request::Op::Get(v1::projects_request::Get {
                project_id: String::new(),
                path: work_dir.display().to_string(),
            })),
        })
        .await;
    let Err(status) = asked else {
        return;
    };
    if status_diagnostic(&status, "Projects(Get)").code == codes::CONFIG_003 {
        ctx.render.note(&format!(
            "[humanitl] new project {}: starting with the defaults; humanitl projects proposal \
             shows them",
            work_dir.display()
        ));
    }
}

/// Ob nach dem Lauf gefragt wird: Tasten von `stdin`, die Frage auf
/// `stderr`, und keine JSON-Ausgabe.
fn asks(json: bool) -> bool {
    !json && std::io::stdin().is_terminal() && std::io::stderr().is_terminal()
}

/// Die Palette der Ladeanimation, oder `None`, wenn sie nicht erscheint.
pub(crate) fn splash_palette(config: &Config, json: bool) -> Option<Palette> {
    splash::wanted(
        config.terminal.splash,
        json,
        std::io::stdout().is_terminal() && std::io::stderr().is_terminal(),
    )
    .then(|| splash::palette(std::env::var_os("NO_COLOR").as_deref()))
}

/// `humanitl go` mit `--ask terminal`: `CLI_002`, bevor irgendetwas startet.
///
/// In `go` gehört dieses Terminal dem Agenten. Der Kasten von
/// `--ask terminal` bräuchte dieselbe Tastatur, und `go` startete dann den
/// Agenten, ohne dass man in seinem Terminal landete. Gleich, ob der Modus vom
/// Schalter, aus der Umgebung oder aus der Konfiguration kommt.
///
/// # Errors
///
/// `CLI_002`, wenn `hold.ask_mode` für diesen Aufruf `terminal` ist.
pub(crate) fn refuse_ask_terminal(config: &Config) -> Result<(), Failure> {
    if config.hold.ask_mode != AskMode::Terminal {
        return Ok(());
    }
    Err(Failure::with_exit(
        Diagnostic::builder(codes::CLI_002, Severity::Error)
            .why(
                "humanitl go hands this terminal to the agent, and --ask terminal (hold.ask_mode \
                 = \"terminal\") would need the same terminal for its questions. Decide in the \
                 app with --ask ui, or use humanitl run --ask terminal to answer here without \
                 the agent's terminal.",
            )
            .fix(FixAction::CopyCommand("humanitl go --ask ui".to_owned()))
            .build(),
        EXIT_USER,
    ))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn the_words_after_the_dashes_are_arguments_of_a_named_agent() {
        let words = [OsString::from("--resume")];
        assert_eq!(
            command(Some("claude"), &words),
            [OsString::from("claude"), OsString::from("--resume")]
        );
        assert!(command(Some("claude"), &[]).is_empty());
        assert_eq!(command(None, &words), words);
    }

    #[test]
    fn an_unknown_agent_names_every_known_one() {
        let found = agent_program("foo").expect_err("no such agent");
        assert_eq!(found.exit, EXIT_USER);
        assert_eq!(found.diagnostic.code, codes::CLI_004);
        for id in AdapterRegistry::builtin().ids() {
            assert!(
                found.diagnostic.why.contains(id),
                "{}",
                found.diagnostic.why
            );
        }
        assert_eq!(agent_program("claude").expect("known"), "claude");
    }

    #[test]
    fn a_switch_of_go_replaces_the_global_one() {
        let mut pairs = vec![
            ("sandbox.work_mode".to_owned(), "rw".to_owned()),
            ("llm.endpoint".to_owned(), "http://x".to_owned()),
        ];
        set(&mut pairs, WORK_MODE_KEY, "ro");
        assert_eq!(
            pairs,
            [
                ("llm.endpoint".to_owned(), "http://x".to_owned()),
                ("sandbox.work_mode".to_owned(), "ro".to_owned()),
            ]
        );
    }
}
