//! Die Kommandozeile: `clap`-Struktur, Konfigurations-Flags aus dem Schema.
//!
//! Zwei Hälften, die zusammen eine Kommandozeile ergeben:
//!
//! - Die Unterkommandos und ihre eigenen Argumente stehen als `derive`-Typen
//!   hier ([`Cli`], [`Cmd`], [`SandboxCmd`] und die anderen). Sie ändern sich
//!   mit den Issues.
//! - Die Konfigurations-Flags entstehen aus dem JSON-Schema von
//!   `humanitl_config::Config` ([`config_args`]), ein Flag je Blattfeld. Sie
//!   ändern sich mit dem Schema, ohne dass hier etwas nachgezogen wird
//!   (ADR-011). Der Wert wandert unverändert als Paar `(Pfad, Text)` in
//!   `humanitl_config::Sources::cli`, und die Präzedenz Kommandozeile über
//!   Umgebung über Profil über Datei über Vorgabe entsteht dort, nicht hier.
//!
//! Ein paar Flags haben zusätzlich den kurzen Namen aus `CONVENTIONS.md` 3.8:
//! `--profile` ist `--sandbox-profile`, `--work` ist `--sandbox-work-dir`,
//! `--ask` ist `--hold-ask-mode`, `--llm` ist `--llm-endpoint`. Sie sind
//! Zweitnamen desselben Arguments, nicht eigene Argumente: sonst gäbe es zwei
//! Wege, dasselbe Feld zu setzen, und eine Regel, welcher gewinnt.

use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::OnceLock;

use clap::builder::PossibleValuesParser;
use clap::{
    Arg, ArgAction, ArgMatches, Args, Command, CommandFactory, FromArgMatches, Parser, Subcommand,
};
use humanitl_config::schema;

/// Was am Ende von `humanitl --help` steht: die Exit-Codes aus
/// `backlog/CONVENTIONS.md` 3.8.
pub const EXIT_CODES_HELP: &str = "\
Exit codes:
  0   the command did what it says
  1   user error; the diagnostic on stderr says what and why
  2   the daemon is not reachable
  3   a sandbox isolation check failed
  4   a security violation, for example an authority mismatch
  10  rules test: the request would be blocked
  11  rules test: the request would be held for a decision

A failing command writes a diagnostic block to stderr, or with --json one line
of JSON to stdout.";

/// Die Zweitnamen aus `CONVENTIONS.md` 3.8: `(Schema-Pfad, kurzes Flag)`.
pub const SHORT_FLAGS: &[(&str, &str)] = &[
    ("sandbox.profile", "profile"),
    ("sandbox.work_dir", "work"),
    ("sandbox.work_mode", "work-mode"),
    ("hold.ask_mode", "ask"),
    ("llm.endpoint", "llm"),
];

/// Die Überschrift, unter der `--help` die Konfigurations-Flags sammelt.
const CONFIG_HEADING: &str = "Configuration (config.toml, HUMANITL_*, --json for the schema)";

/// Human-in-the-loop network moderation for AI agents.
#[derive(Debug, Parser)]
#[command(
    name = "humanitl",
    version,
    about = "Human-in-the-loop network moderation for AI agents",
    long_about = None,
    after_help = EXIT_CODES_HELP,
    after_long_help = EXIT_CODES_HELP
)]
pub struct Cli {
    /// Die Schalter, die vor und nach dem Unterkommando stehen dürfen.
    #[command(flatten)]
    pub global: GlobalOpts,
    /// Das Unterkommando.
    #[command(subcommand)]
    pub cmd: Cmd,
}

/// Die globalen Schalter.
#[derive(Debug, Clone, Args)]
pub struct GlobalOpts {
    /// Machine-readable output: one JSON value on stdout, diagnostics included.
    #[arg(long, global = true)]
    pub json: bool,

    /// Read this config file instead of the one in the user's config directory.
    #[arg(long, global = true, value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// Explain what is happening; repeat for more.
    #[arg(short = 'v', long, global = true, action = ArgAction::Count)]
    pub verbose: u8,

    /// Only the result, no notes.
    #[arg(short = 'q', long, global = true, conflicts_with = "verbose")]
    pub quiet: bool,
}

/// Die Unterkommandos.
#[derive(Debug, Subcommand)]
pub enum Cmd {
    /// Run an agent behind the proxy in a sandbox (arrives in HUM-067).
    Run(PlaceholderArgs),

    /// Start, plan and check the sandbox.
    Sandbox {
        /// Was mit der Sandbox geschehen soll.
        #[command(subcommand)]
        cmd: SandboxCmd,
    },

    /// List, add, remove and test rules (arrives in HUM-065).
    Rules(PlaceholderArgs),

    /// The flows of this and earlier sessions.
    Flows {
        /// Was mit den Flows geschehen soll.
        #[command(subcommand)]
        cmd: FlowsCmd,
    },

    /// Verify and export the audit chain (arrives in HUM-070).
    Audit(PlaceholderArgs),

    /// The resolved configuration and its schema.
    Config {
        /// Was mit der Konfiguration geschehen soll.
        #[command(subcommand)]
        cmd: ConfigCmd,
    },

    /// The background service.
    Daemon {
        /// Was mit dem Dienst geschehen soll.
        #[command(subcommand)]
        cmd: DaemonCmd,
    },
}

/// Ein Unterkommando, das es noch nicht gibt.
///
/// Es nimmt seine Argumente entgegen, statt sie als Tippfehler abzulehnen:
/// wer `humanitl rules list` schreibt, soll erfahren, dass das Kommando noch
/// nicht da ist, und nicht, dass `list` unbekannt sei.
#[derive(Debug, Args)]
pub struct PlaceholderArgs {
    /// Alles, was hinter dem Unterkommando steht.
    #[arg(
        trailing_var_arg = true,
        allow_hyphen_values = true,
        value_name = "ARGS"
    )]
    pub args: Vec<OsString>,
}

/// Die Unterkommandos von `humanitl sandbox`.
#[derive(Debug, Subcommand)]
pub enum SandboxCmd {
    /// Start a sandbox with CMD as the agent and exit with its exit code.
    Run {
        /// Host directory mounted at /tests/escape of the `test` profile.
        #[arg(long, value_name = "DIR")]
        tests_dir: Option<PathBuf>,
        /// Der Befehl in der Sandbox, hinter `--`.
        #[arg(last = true, value_name = "CMD", required = true)]
        cmd: Vec<OsString>,
    },

    /// Print the bwrap command line for the current profile without starting it.
    Argv {
        /// Der Befehl, der in der Zeile steht; ohne ihn der Agent aus der
        /// Konfiguration, sonst `/bin/sh`.
        #[arg(last = true, value_name = "CMD")]
        cmd: Vec<OsString>,
    },

    /// Start a short-lived sandbox and show the three isolation guarantees.
    Check,
}

/// Die Unterkommandos von `humanitl flows`.
#[derive(Debug, Subcommand)]
pub enum FlowsCmd {
    /// List flows, newest last.
    List {
        /// Der Filter, zum Beispiel `host:github.com state:blocked`.
        #[arg(value_name = "FILTER")]
        filter: Option<String>,
        /// Wie viele Zeilen höchstens; 0 bedeutet die Vorgabe des Dienstes.
        #[arg(long, default_value_t = 0, value_name = "N")]
        limit: u32,
    },

    /// Show one flow.
    Show {
        /// Die Id des Flows.
        #[arg(value_name = "ID")]
        id: String,
    },

    /// Decide one waiting flow: allow it or block it.
    ///
    /// Die Entscheidung des Menschen, wie die Oberfläche sie schickt, nur über
    /// das Terminal. Sie ist der Vorläufer von `--ask terminal` (HUM-021) und
    /// bleibt danach, weil ein Skript oder eine Fernsitzung keine Oberfläche
    /// hat.
    Decide {
        /// Die Id des Flows.
        #[arg(value_name = "ID")]
        id: String,
        /// Was mit ihm geschehen soll.
        #[arg(value_name = "VERDICT", value_parser = ["allow", "block"])]
        verdict: String,
        /// Begründung für den Agenten; sie steht im Block-Body unter `note:`.
        #[arg(long, value_name = "TEXT")]
        note: Option<String>,
    },
}

/// Die Unterkommandos von `humanitl config`.
#[derive(Debug, Subcommand)]
pub enum ConfigCmd {
    /// Print the resolved value of one key, for example the hold timeout.
    Get {
        /// Der Pfad des Feldes, mit Punkten getrennt.
        #[arg(value_name = "KEY")]
        key: String,
    },

    /// Print the JSON schema of the configuration.
    Schema,
}

/// Die Unterkommandos von `humanitl daemon`.
#[derive(Debug, Subcommand)]
pub enum DaemonCmd {
    /// Ask the daemon who it is.
    Status,
}

/// Ein gelesener Aufruf: die Unterkommandos und die Konfigurations-Flags.
#[derive(Debug)]
pub struct Invocation {
    /// Was `clap` aus den `derive`-Typen gelesen hat.
    pub cli: Cli,
    /// Die Konfigurations-Flags als `(Schema-Pfad, Text)`, in Pfad-Reihenfolge.
    pub config: Vec<(String, String)>,
}

/// Der vollständige `clap`-Befehl: Unterkommandos plus Konfigurations-Flags.
#[must_use]
pub fn command() -> Command {
    Cli::command().args(config_args())
}

/// Liest die Kommandozeile.
///
/// # Errors
///
/// Der Fehler von `clap`: eine unbekannte Option, ein fehlendes Argument oder
/// die Anforderung von `--help` und `--version`. Wie er gemeldet wird,
/// entscheidet der Aufrufer.
pub fn parse<I, T>(argv: I) -> Result<Invocation, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let matches = command().try_get_matches_from(argv)?;
    let cli = Cli::from_arg_matches(&matches)?;
    let config = config_pairs(&matches);
    Ok(Invocation { cli, config })
}

/// Ein Konfigurations-Flag, mit den Texten, die `clap` als `'static` braucht.
///
/// `clap` nimmt für Namen und Hilfetexte `&'static str`. Das Schema liefert
/// `String`; die Umrechnung passiert deshalb genau einmal beim ersten Aufruf
/// von [`flags`] und lebt danach so lange wie der Prozess.
#[derive(Debug)]
pub struct ConfigFlag {
    /// Der Pfad im Schema, zugleich die Kennung des Arguments.
    pub path: &'static str,
    /// Der lange Name, zum Beispiel `hold-timeout-secs`.
    pub long: String,
    /// Der Zweitname aus `CONVENTIONS.md` 3.8, falls es einen gibt.
    pub short: Option<&'static str>,
    /// Der Name des Wertes im Hilfetext.
    pub value_name: String,
    /// Beschreibung und Vorgabewert.
    pub help: String,
    /// Die erlaubten Werte, wenn das Feld eine Aufzählung ist.
    pub allowed: Option<Vec<String>>,
    /// Ob das Feld nur `true` und `false` kennt.
    pub boolean: bool,
}

/// Ein Flag je Blattfeld des Schemas, in Pfad-Reihenfolge.
///
/// Freie Tabellen (`resolver.overrides`) bekommen keines: ihre Schlüssel
/// stehen nicht im Schema, und eine ganze Tabelle lässt sich auf der
/// Kommandozeile nicht sinnvoll schreiben. Sie bleiben Datei und Umgebung
/// vorbehalten.
#[must_use]
pub fn flags() -> &'static [ConfigFlag] {
    static FLAGS: OnceLock<Vec<ConfigFlag>> = OnceLock::new();
    FLAGS.get_or_init(|| {
        schema::leaves()
            .into_iter()
            .filter(|field| !field.free_table)
            .map(|field| ConfigFlag {
                path: field.path.as_str(),
                long: flag_name(&field.path),
                short: short_flag(&field.path),
                value_name: value_name(field),
                help: help_text(field),
                allowed: field.allowed.clone(),
                boolean: is_boolean(field),
            })
            .collect()
    })
}

/// Die Argumente zu [`flags`].
#[must_use]
pub fn config_args() -> Vec<Arg> {
    flags()
        .iter()
        .map(|flag| {
            let mut arg = Arg::new(flag.path)
                .long(flag.long.as_str())
                .global(true)
                .help(flag.help.as_str())
                .value_name(flag.value_name.as_str())
                .help_heading(CONFIG_HEADING);
            if let Some(short) = flag.short {
                arg = arg.visible_alias(short);
            }
            if let Some(allowed) = flag.allowed.as_ref() {
                arg = arg.value_parser(PossibleValuesParser::new(
                    allowed.iter().map(String::as_str),
                ));
            } else if flag.boolean {
                // Ein Schalter ohne Wert heißt `true`; `--x=false` schaltet ab.
                // `require_equals` verhindert, dass `--x sandbox check` das
                // Unterkommando als Wert frisst.
                arg = arg
                    .num_args(0..=1)
                    .require_equals(true)
                    .default_missing_value("true")
                    .value_parser(PossibleValuesParser::new(["true", "false"]));
            }
            arg
        })
        .collect()
}

/// Die gesetzten Konfigurations-Flags als `(Pfad, Text)`.
#[must_use]
pub fn config_pairs(matches: &ArgMatches) -> Vec<(String, String)> {
    flags()
        .iter()
        .filter_map(|flag| {
            matches
                .try_get_one::<String>(flag.path)
                .ok()
                .flatten()
                .map(|value| (flag.path.to_owned(), value.clone()))
        })
        .collect()
}

/// Der Name des Flags zu einem Schema-Pfad: `hold.timeout_secs` wird
/// `hold-timeout-secs`.
#[must_use]
pub fn flag_name(path: &str) -> String {
    path.replace(['.', '_'], "-")
}

/// Der Zweitname aus `CONVENTIONS.md` 3.8, falls es einen gibt.
#[must_use]
pub fn short_flag(path: &str) -> Option<&'static str> {
    SHORT_FLAGS
        .iter()
        .find(|(key, _)| *key == path)
        .map(|(_, flag)| *flag)
}

/// Ob das Feld nur `true` und `false` kennt.
fn is_boolean(field: &schema::Field) -> bool {
    field
        .types
        .iter()
        .all(|kind| kind == "boolean" || kind == "null")
        && field.types.iter().any(|kind| kind == "boolean")
}

/// Der Hilfetext eines Konfigurations-Flags: die Beschreibung aus dem Schema
/// plus der Vorgabewert.
fn help_text(field: &schema::Field) -> String {
    let description = field.description.trim();
    let default = field.default_literal();
    if description.is_empty() {
        format!("{} [default: {default}]", field.path)
    } else {
        format!("{description} [default: {default}]")
    }
}

/// Der Name des Wertes im Hilfetext: der Typ, wie ihn die Doku zeigt.
fn value_name(field: &schema::Field) -> String {
    let label = field.type_label.trim();
    if label.is_empty() {
        "VALUE".to_owned()
    } else {
        label.to_uppercase().replace(' ', "_")
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::path::Path;

    use humanitl_config::schema;

    use super::{
        Cmd, ConfigCmd, EXIT_CODES_HELP, SHORT_FLAGS, SandboxCmd, command, flag_name, parse,
    };

    #[test]
    fn the_command_definition_is_sound() {
        command().debug_assert();
    }

    #[test]
    fn every_config_key_of_the_schema_has_a_flag() {
        let long: Vec<String> = command()
            .get_arguments()
            .filter_map(|arg| arg.get_long().map(ToOwned::to_owned))
            .collect();

        for field in schema::leaves() {
            if field.free_table {
                continue;
            }
            let expected = flag_name(&field.path);
            assert!(
                long.contains(&expected),
                "--{expected} is missing from the command line"
            );
        }
        // Die Schlüssel aus CONVENTIONS 3.7, wörtlich.
        for key in [
            "llm.endpoint",
            "llm.passthrough_paths",
            "hold.timeout_secs",
            "hold.ask_mode",
            "sandbox.profile",
            "sandbox.work_dir",
            "sandbox.work_mode",
            "agent.adapter",
            "agent.command",
            "recorder.retention_days",
            "ui.language",
            "ui.theme",
            "experimental.h2_upstream",
            "experimental.ws_hold",
        ] {
            assert!(
                long.contains(&flag_name(key)),
                "--{} is missing from the command line",
                flag_name(key)
            );
        }
    }

    #[test]
    fn the_short_flags_of_conventions_38_are_aliases_of_the_config_flags() {
        // Ein gültiger Wert je Flag: die Aufzählungen nehmen nicht jeden Text.
        let values = [
            ("sandbox.profile", "test"),
            ("sandbox.work_dir", "/tmp/project"),
            ("sandbox.work_mode", "ro"),
            ("hold.ask_mode", "none"),
            ("llm.endpoint", "http://127.0.0.1:1234"),
        ];
        assert_eq!(values.len(), SHORT_FLAGS.len());

        for (path, short) in SHORT_FLAGS {
            let value = values
                .iter()
                .find(|(key, _)| key == path)
                .map(|(_, value)| *value)
                .expect("every short flag has a test value");
            let argv = vec![
                "humanitl".to_owned(),
                "daemon".to_owned(),
                "status".to_owned(),
                format!("--{short}"),
                value.to_owned(),
            ];
            let invocation = parse(argv).expect("the short flag parses");
            assert!(
                invocation
                    .config
                    .iter()
                    .any(|(key, got)| key == path && got == value),
                "--{short} does not set {path}"
            );
        }
    }

    #[test]
    fn a_double_dash_keeps_the_flags_of_the_agent() {
        let invocation = parse([
            "humanitl", "sandbox", "run", "--", "sh", "-c", "exit 5", "--json",
        ])
        .expect("the command parses");

        assert!(
            !invocation.cli.global.json,
            "--json after -- is the agent's"
        );
        let Cmd::Sandbox {
            cmd: SandboxCmd::Run { cmd, tests_dir },
        } = invocation.cli.cmd
        else {
            panic!("expected sandbox run");
        };
        assert_eq!(cmd, ["sh", "-c", "exit 5", "--json"]);
        assert!(tests_dir.is_none());
    }

    #[test]
    fn the_tests_directory_is_a_flag_of_sandbox_run() {
        let invocation = parse([
            "humanitl",
            "sandbox",
            "run",
            "--tests-dir",
            "tests/escape",
            "--",
            "/bin/sh",
            "/tests/escape/esc-1-sockets.sh",
        ])
        .expect("the command parses");

        let Cmd::Sandbox {
            cmd: SandboxCmd::Run { cmd, tests_dir },
        } = invocation.cli.cmd
        else {
            panic!("expected sandbox run");
        };
        assert_eq!(tests_dir.as_deref(), Some(Path::new("tests/escape")));
        assert_eq!(cmd, ["/bin/sh", "/tests/escape/esc-1-sockets.sh"]);
    }

    #[test]
    fn global_flags_are_allowed_before_and_after_the_subcommand() {
        let before = parse(["humanitl", "--json", "config", "schema"]).expect("parses");
        let after = parse(["humanitl", "config", "schema", "--json"]).expect("parses");

        assert!(before.cli.global.json && after.cli.global.json);
        assert!(matches!(
            after.cli.cmd,
            Cmd::Config {
                cmd: ConfigCmd::Schema
            }
        ));
    }

    #[test]
    fn a_config_flag_lands_in_the_cli_layer() {
        let invocation = parse([
            "humanitl",
            "--hold-timeout-secs",
            "9",
            "config",
            "get",
            "hold.timeout_secs",
        ])
        .expect("parses");

        assert_eq!(
            invocation.config,
            vec![("hold.timeout_secs".to_owned(), "9".to_owned())]
        );
    }

    #[test]
    fn a_boolean_config_flag_needs_no_value() {
        let invocation =
            parse(["humanitl", "--experimental-ws-hold", "daemon", "status"]).expect("parses");
        assert!(
            invocation
                .config
                .iter()
                .any(|(key, value)| key == "experimental.ws_hold" && value == "true")
        );

        let explicit = parse([
            "humanitl",
            "--experimental-ws-hold=false",
            "daemon",
            "status",
        ])
        .expect("parses");
        assert!(
            explicit
                .config
                .iter()
                .any(|(key, value)| key == "experimental.ws_hold" && value == "false")
        );
    }

    #[test]
    fn an_enum_flag_only_takes_its_own_values() {
        assert!(parse(["humanitl", "--ask", "terminal", "daemon", "status"]).is_ok());
        assert!(parse(["humanitl", "--ask", "shout", "daemon", "status"]).is_err());
    }

    #[test]
    fn the_help_documents_the_exit_codes() {
        let text = command().render_help().to_string();

        assert!(text.contains(EXIT_CODES_HELP.lines().next().unwrap_or_default()));
        for line in ["0 ", "1 ", "2 ", "3 "] {
            assert!(
                text.contains(&format!("  {line}")),
                "exit code {line} is missing from --help"
            );
        }
    }

    #[test]
    fn a_placeholder_subcommand_swallows_its_arguments() {
        let invocation = parse(["humanitl", "rules", "list", "--json"]).expect("parses");
        let Cmd::Rules(args) = invocation.cli.cmd else {
            panic!("expected rules");
        };
        assert_eq!(args.args, ["list", "--json"]);
    }
}
