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
//! `--work` ist `--sandbox-work-dir`, `--ask` ist `--hold-ask-mode`, `--llm`
//! ist `--llm-endpoint`. Sie sind Zweitnamen desselben Arguments, nicht eigene
//! Argumente: sonst gäbe es zwei Wege, dasselbe Feld zu setzen, und eine Regel,
//! welcher gewinnt.
//!
//! `--profile` ist die Ausnahme und ein eigenes Argument ([`GlobalOpts::profile`]).
//! Was es benennt, entscheidet das Unterkommando: unter `humanitl sandbox` das
//! bwrap-Profil unter `profiles/sandbox/`, überall sonst das Profil der Sitzung
//! (HUM-066). Die Entscheidung fällt in [`crate::cmd::ProfileMeaning`], die
//! Begründung steht in `backlog/CONVENTIONS.md` 4.23.

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

10 and 11 do not occur yet: the contract has no operation that evaluates one
URL against the rule set, and humanitl rules test says so instead of guessing.

A failing command writes a diagnostic block to stderr, or with --json one line
of JSON to stdout.";

/// Die Zweitnamen aus `CONVENTIONS.md` 3.8: `(Schema-Pfad, kurzes Flag)`.
pub const SHORT_FLAGS: &[(&str, &str)] = &[
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

    /// Use this profile for the session; falls back to the sandbox profile of
    /// that name when no session profile exists.
    #[arg(long = "profile", global = true, value_name = "NAME")]
    pub profile: Option<String>,
}

/// Die Unterkommandos.
#[derive(Debug, Subcommand)]
pub enum Cmd {
    /// Show the session a profile resolves to; starting it arrives in HUM-067.
    Run(RunArgs),

    /// Start, plan and check the sandbox.
    Sandbox {
        /// Was mit der Sandbox geschehen soll.
        #[command(subcommand)]
        cmd: SandboxCmd,
    },

    /// List, add, change, reorder, dry-run and reload rules.
    Rules {
        /// Was mit den Regeln geschehen soll.
        #[command(subcommand)]
        cmd: RulesCmd,
    },

    /// The flows of this and earlier sessions.
    Flows {
        /// Was mit den Flows geschehen soll.
        #[command(subcommand)]
        cmd: FlowsCmd,
    },

    /// What the agent left in the project during a sandbox run.
    Sessions {
        /// Was mit den Läufen geschehen soll.
        #[command(subcommand)]
        cmd: SessionsCmd,
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

    /// Check this machine: one line per precondition, with a fix.
    Doctor(DoctorArgs),
}

/// Die Argumente von `humanitl doctor`.
///
/// `--json` steht schon global bereit; hier bleibt der eine Schalter, der eine
/// Verbindung ins Netz auslöst.
#[derive(Debug, Args)]
pub struct DoctorArgs {
    // Der Schalter heißt nicht `--probe` und nicht `--llm`: `--llm` ist nach
    // [`SHORT_FLAGS`] der Zweitname von `--llm-endpoint` und **setzt** die
    // Adresse, statt sie zu prüfen. Der Text des Doc-Kommentars ist der
    // Hilfetext von `clap` und deshalb englisch (CONVENTIONS.md 3.9).
    /// Contact llm.endpoint as part of the report; without this, doctor opens
    /// no connection at all.
    ///
    /// The endpoint is named on stderr before it is contacted. The probe runs
    /// in the daemon (ProbeLlm): two GET requests, /api/tags then /v1/models,
    /// no credentials, no redirects.
    #[arg(long)]
    pub probe_llm: bool,
}

/// Die Argumente von `humanitl run`.
///
/// `--profile`, `--work`, `--ask` und `--llm` stehen als globale Argumente
/// schon bereit; hier bleibt der Befehl hinter `--`, den HUM-067 in der
/// Sandbox startet.
#[derive(Debug, Args)]
pub struct RunArgs {
    /// Der Befehl in der Sandbox, hinter `--`; ohne ihn der Agent aus der
    /// Konfiguration.
    #[arg(last = true, value_name = "CMD")]
    pub cmd: Vec<OsString>,
}

/// Ein Unterkommando, das es noch nicht gibt.
///
/// Es nimmt seine Argumente entgegen, statt sie als Tippfehler abzulehnen:
/// wer `humanitl audit verify` schreibt, soll erfahren, dass das Kommando noch
/// nicht da ist, und nicht, dass `verify` unbekannt sei.
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

    /// Attach this terminal to the running session.
    ///
    /// One client writes, any number watch. Ctrl+C reaches the agent as byte
    /// 0x03, not as a signal: the sandbox has no controlling terminal. Detach
    /// by closing the connection; the session keeps running.
    Attach {
        /// Watch without sending anything.
        #[arg(long)]
        read_only: bool,
    },
}

/// Die Unterkommandos von `humanitl sessions`.
#[derive(Debug, Subcommand)]
pub enum SessionsCmd {
    /// Show what a sandbox run changed in the project.
    ///
    /// Changed files, possible secrets and symlinks that leave the project.
    /// ID is the sandbox id from the log line of the run or from
    /// `humanitl sandbox` in the window; it is the only value this command
    /// needs.
    ///
    /// Every path comes from the agent and is shown, never reopened: two
    /// different names can look the same, so each row carries the first 16 hex
    /// characters of the sha256 of the real name.
    Summary {
        /// Die Kennung des Sandbox-Laufs.
        #[arg(value_name = "ID")]
        id: String,
    },
}

/// Die Unterkommandos von `humanitl flows`.
#[derive(Debug, Subcommand)]
pub enum FlowsCmd {
    /// List flows, newest first; --asc turns the order around.
    ///
    /// SIZE is request plus response in bytes, MS the duration in
    /// milliseconds, PATH shortened in the middle to 40 characters. What the
    /// daemon does not know stays a dash.
    List {
        /// Der Filter, zum Beispiel `host:github.com state:blocked`. Mehrere
        /// Wörter werden mit einem Leerzeichen verbunden und unverändert an
        /// den Dienst gereicht.
        #[arg(value_name = "FILTER")]
        filter: Vec<String>,
        /// Wie viele Zeilen höchstens; 0 bedeutet die Vorgabe des Dienstes.
        #[arg(long, default_value_t = 0, value_name = "N")]
        limit: u32,
        /// Wonach sortiert wird.
        #[arg(
            long,
            default_value = "ts",
            value_name = "KEY",
            value_parser = ["ts", "host", "duration", "size"]
        )]
        sort: String,
        /// Aufsteigend statt absteigend: die älteste Zeile zuerst.
        #[arg(long)]
        asc: bool,
    },

    /// Show one flow, or one of its bodies.
    Show {
        /// Die Id des Flows.
        #[arg(value_name = "ID")]
        id: String,
        /// Statt der Felder einen der beiden Bodies ausgeben.
        #[arg(long, value_name = "WHICH", value_parser = ["request", "response"])]
        body: Option<String>,
        /// Den Body Byte für Byte schreiben statt als Text.
        #[arg(long, requires = "body")]
        raw: bool,
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

/// Die Methoden, über die eine Regel geschrieben werden kann.
///
/// `METHOD_OTHER` steht nicht darunter: eine Regel über allem, was der Daemon
/// nicht kennt, wäre eine Regel über etwas, das niemand nachlesen kann
/// (`humanitl_ipc::convert::rule_from_proto`).
pub const RULE_METHODS: [&str; 9] = [
    "GET", "HEAD", "POST", "PUT", "PATCH", "DELETE", "OPTIONS", "CONNECT", "TRACE",
];

/// Die Unterkommandos von `humanitl rules`.
///
/// Jedes davon ist genau ein `Rules`-Aufruf am Daemon (ADR-018). Ob eine Regel
/// gültig ist, entscheidet der Dienst; die Kommandozeile baut nur die
/// Wire-Form und gibt den Befund wieder, den sie zurückbekommt.
#[derive(Debug, Subcommand)]
pub enum RulesCmd {
    /// List the rules in the order in which they are evaluated.
    List {
        /// Auch mitgelieferte und abgelaufene Regeln zeigen.
        #[arg(long)]
        all: bool,
    },

    /// Add a rule. The daemon checks it and answers with the whole set.
    Add {
        /// Die Felder der neuen Regel; --action und --host sind Pflicht.
        #[command(flatten)]
        rule: RuleArgs,
    },

    /// Change one rule. What is not given stays as it is.
    Update {
        /// Die Id der Regel.
        #[arg(value_name = "ID")]
        id: String,
        /// Die Felder, die sich ändern sollen.
        #[command(flatten)]
        rule: RuleArgs,
    },

    /// Remove one rule.
    Remove {
        /// Die Id der Regel.
        #[arg(value_name = "ID")]
        id: String,
    },

    /// Move one rule to another place inside its own group.
    Reorder {
        /// Die Id der Regel.
        #[arg(value_name = "ID")]
        id: String,
        /// Der neue Platz, 1-basiert und innerhalb der Gruppe der Regel.
        #[arg(value_name = "POSITION", value_parser = clap::value_parser!(u32).range(1..))]
        position: u32,
    },

    /// Show which of the last recorded requests a rule would have matched.
    #[command(name = "dry-run")]
    DryRun {
        /// Die Felder der Regel; --action und --host sind Pflicht.
        #[command(flatten)]
        rule: RuleArgs,
        /// Wie viele der zuletzt aufgezeichneten Anfragen geprüft werden;
        /// 0 bedeutet die Vorgabe des Dienstes.
        #[arg(long, default_value_t = 0, value_name = "N")]
        scan: u32,
    },

    /// Switch a bundled rule off. It stays in the list and decides nothing.
    Disable {
        /// Die Id der mitgelieferten Regel.
        #[arg(value_name = "ID")]
        id: String,
    },

    /// Switch a bundled rule back on.
    Enable {
        /// Die Id der mitgelieferten Regel.
        #[arg(value_name = "ID")]
        id: String,
    },

    /// Read rules.yaml again and report what changed.
    Reload,

    /// Evaluate one URL against the rule set (needs an RPC that is not there yet).
    Test {
        /// Die vollständige URL, zum Beispiel `https://api.github.com/repos/x`.
        #[arg(value_name = "URL")]
        url: String,
        /// Die Methode der gedachten Anfrage.
        #[arg(
            long,
            value_name = "M",
            value_parser = PossibleValuesParser::new(RULE_METHODS),
            ignore_case = true
        )]
        method: Option<String>,
        /// Ob die gedachte Anfrage ein Upgrade trägt.
        #[arg(long, value_name = "UPGRADE", value_parser = ["websocket"])]
        upgrade: Option<String>,
    },
}

/// Die Felder einer Regel, wie die Kommandozeile sie schreibt.
///
/// Dieselbe Struktur trägt bei `add` und `dry-run` eine vollständige Regel und
/// bei `update` nur die Felder, die sich ändern sollen. Deshalb ist hier alles
/// wahlfrei; dass `--action` und `--host` bei `add` und `dry-run` da sein
/// müssen, prüft [`crate::cmd::rules`] und meldet es als Befund. Alles andere
/// prüft der Daemon, und sein Befund nennt Zeile und Feld.
#[derive(Debug, Clone, Args)]
pub struct RuleArgs {
    /// What happens to a matching request.
    #[arg(long, value_name = "ACTION", value_parser = ["allow", "block", "ask", "redact"])]
    pub action: Option<String>,

    /// Host pattern: a label glob, `ip:ADDRESS` or `cidr:ADDRESS/LEN`.
    #[arg(long, value_name = "PATTERN")]
    pub host: Option<String>,

    /// One HTTP method; repeat the flag for more. Without it, every method matches.
    #[arg(
        long,
        value_name = "M",
        value_parser = PossibleValuesParser::new(RULE_METHODS),
        ignore_case = true
    )]
    pub method: Vec<String>,

    /// Path glob, or a regular expression when it starts with `~`.
    #[arg(long, value_name = "P")]
    pub path: Option<String>,

    /// Only requests with this scheme.
    #[arg(long, value_name = "S", value_parser = ["http", "https", "ws", "wss"])]
    pub scheme: Option<String>,

    /// Only requests to this port.
    #[arg(long, value_name = "N", value_parser = clap::value_parser!(u16).range(1..))]
    pub port: Option<u16>,

    /// Only requests that carry this upgrade.
    #[arg(long, value_name = "UPGRADE", value_parser = ["none", "websocket"])]
    pub upgrade: Option<String>,

    /// never, session, or a point in time in RFC 3339.
    #[arg(long, value_name = "WHEN")]
    pub expires: Option<String>,

    /// Why the rule exists. It ends up in rules.yaml and in the window.
    #[arg(long, value_name = "TEXT")]
    pub note: Option<String>,

    /// Place inside the group of the rule, 1-based; without it at the end.
    #[arg(long, value_name = "N", value_parser = clap::value_parser!(u32).range(1..))]
    pub position: Option<u32>,

    /// Allow private destinations (RFC 1918, loopback, link-local, CGNAT).
    #[arg(
        long,
        value_name = "BOOL",
        num_args = 0..=1,
        require_equals = true,
        default_missing_value = "true"
    )]
    pub allow_private: Option<bool>,
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

    /// Print the JSON schema of the configuration, or the list of profiles.
    Schema {
        /// List the profiles --profile can choose instead of the schema.
        #[arg(long)]
        profiles: bool,
    },
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
        Cmd, ConfigCmd, EXIT_CODES_HELP, FlowsCmd, RulesCmd, SHORT_FLAGS, SandboxCmd, command,
        flag_name, parse,
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
                cmd: ConfigCmd::Schema { profiles: false }
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
        let invocation =
            parse(["humanitl", "audit", "verify", "--since", "yesterday"]).expect("parses");
        let Cmd::Audit(args) = invocation.cli.cmd else {
            panic!("expected audit");
        };
        assert_eq!(args.args, ["verify", "--since", "yesterday"]);
    }

    #[test]
    fn a_rule_is_written_field_by_field() {
        let invocation = parse([
            "humanitl",
            "rules",
            "add",
            "--action",
            "allow",
            "--host",
            "**.github.com",
            "--method",
            "get",
            "--method",
            "POST",
            "--port",
            "8443",
            "--expires",
            "session",
            "--allow-private",
        ])
        .expect("parses");

        let Cmd::Rules {
            cmd: RulesCmd::Add { rule },
        } = invocation.cli.cmd
        else {
            panic!("expected rules add");
        };
        assert_eq!(rule.action.as_deref(), Some("allow"));
        assert_eq!(rule.host.as_deref(), Some("**.github.com"));
        // `ignore_case` lässt die Schreibweise des Nutzers stehen; die
        // Übersetzung in die Wire-Form macht daraus GET und POST.
        assert_eq!(rule.method, ["get", "POST"]);
        assert_eq!(rule.port, Some(8443));
        assert_eq!(rule.expires.as_deref(), Some("session"));
        assert_eq!(rule.allow_private, Some(true));
    }

    #[test]
    fn a_rule_only_takes_the_actions_and_methods_of_the_contract() {
        assert!(parse(["humanitl", "rules", "add", "--action", "shout"]).is_err());
        assert!(parse(["humanitl", "rules", "add", "--method", "PROPFIND"]).is_err());
        assert!(parse(["humanitl", "rules", "add", "--port", "0"]).is_err());
        assert!(parse(["humanitl", "rules", "reorder", "id", "0"]).is_err());
    }

    #[test]
    fn a_filter_may_be_several_words_and_the_order_is_a_flag() {
        let invocation = parse([
            "humanitl",
            "flows",
            "list",
            "host:github.com",
            "findings:>0",
            "--sort",
            "size",
            "--asc",
        ])
        .expect("parses");

        let Cmd::Flows {
            cmd:
                FlowsCmd::List {
                    filter,
                    sort,
                    asc,
                    limit,
                },
        } = invocation.cli.cmd
        else {
            panic!("expected flows list");
        };
        assert_eq!(filter, ["host:github.com", "findings:>0"]);
        assert_eq!(sort, "size");
        assert!(asc);
        assert_eq!(limit, 0);
    }

    #[test]
    fn raw_needs_a_body() {
        assert!(parse(["humanitl", "flows", "show", "id", "--raw"]).is_err());
        assert!(
            parse([
                "humanitl", "flows", "show", "id", "--body", "response", "--raw"
            ])
            .is_ok()
        );
    }
}
