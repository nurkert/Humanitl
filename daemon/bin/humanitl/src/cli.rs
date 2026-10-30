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
    /// Start a session in the current directory, or show the session a profile resolves to.
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

    /// Verify and export the audit chain.
    Audit {
        /// Was mit der Audit-Kette geschehen soll.
        #[command(subcommand)]
        cmd: AuditCmd,
    },

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

    /// Find LLM servers in the local network.
    Llm {
        /// Was mit dem LLM-Server geschehen soll.
        #[command(subcommand)]
        cmd: LlmCmd,
    },
}

/// Die Unterkommandos von `humanitl llm`.
#[derive(Debug, Subcommand)]
pub enum LlmCmd {
    /// Search the local /24 for OpenAI-compatible servers and Ollama.
    ///
    /// Connection attempts only, to four ports, and only when you ask for it.
    /// Nothing is written and no credentials are sent.
    Discover {
        /// The network to search, as CIDR; without it, the local /24 of the
        /// default route. Never wider than a /24.
        #[arg(long, value_name = "CIDR")]
        subnet: Option<String>,

        /// A port to ask, repeatable; without it 11434, 1234, 8000 and 8080.
        #[arg(long, value_name = "PORT")]
        port: Vec<u16>,
    },

    /// Ask one endpoint what it is: which API answered, its models, how long.
    ///
    /// GET requests in the daemon for the model list of the endpoint (and
    /// /api/tags first on a plain Ollama server without a path prefix), no
    /// credentials and no redirects. Nothing goes through the sandbox.
    Test {
        /// The address as it would stand in `llm.endpoint`.
        #[arg(value_name = "URL")]
        url: String,

        /// Deadline for the whole probe; without it the daemon's default.
        #[arg(long = "timeout-ms", value_name = "N")]
        timeout_ms: Option<u32>,

        /// Send the stored access key along, if URL has the origin of
        /// llm.endpoint; the daemon never gives it to another address.
        #[arg(long = "with-key")]
        with_key: bool,

        /// With --with-key: the key of this saved server instead of the
        /// active one, sent only if URL has the origin of that server.
        #[arg(long, value_name = "NAME", requires = "with_key")]
        server: Option<String>,
    },

    /// The access key of the LLM endpoint, kept in the system keyring.
    ///
    /// The key never stands in config.toml, on a command line, in a log or in
    /// the recording; the configuration file carries only a reference to it.
    Key {
        /// The saved server whose key is meant; without it, the active one.
        #[arg(long, value_name = "NAME", global = true)]
        server: Option<String>,

        /// What to do with the key.
        #[command(subcommand)]
        cmd: KeyCmd,
    },

    /// Saved model servers: endpoint, models and access key per entry.
    ///
    /// The active server is what `llm.endpoint`, `llm.models` and
    /// `llm.api_key_ref` show; only its address gets the stored key. A switch
    /// applies to the next session, a running session keeps its server.
    Server {
        /// What to do with the servers.
        #[command(subcommand)]
        cmd: ServerCmd,
    },
}

/// Die Unterkommandos von `humanitl llm server` (HUM-289).
#[derive(Debug, Subcommand)]
pub enum ServerCmd {
    /// List the saved servers; the active one is marked.
    Ls,

    /// Save a server. Its key follows with `humanitl llm key set --server NAME`.
    Add {
        /// 1 to 32 characters a-z, 0-9, - and _.
        #[arg(value_name = "NAME")]
        name: String,

        /// The address as it would stand in `llm.endpoint`.
        #[arg(value_name = "URL")]
        url: String,

        /// A model of this server, repeatable; the first is the default.
        #[arg(long, value_name = "MODEL")]
        model: Vec<String>,

        /// Make it the active server right away.
        #[arg(long = "use")]
        activate: bool,
    },

    /// Make a server the active one, from the next session on.
    Use {
        /// The saved server.
        #[arg(value_name = "NAME")]
        name: String,
    },

    /// Rename a server. Its key stays where it is.
    Rename {
        /// The saved server.
        #[arg(value_name = "NAME")]
        name: String,

        /// The new name.
        #[arg(value_name = "NEW")]
        new_name: String,
    },

    /// Remove a server and, unless --keep-key, its key from the keyring.
    Rm {
        /// The saved server.
        #[arg(value_name = "NAME")]
        name: String,

        /// The server to use afterwards; required for the active server
        /// while there are others.
        #[arg(long = "use", value_name = "NEXT")]
        replacement: Option<String>,

        /// Leave the key of the server in the keyring.
        #[arg(long = "keep-key")]
        keep_key: bool,
    },
}

/// Die Unterkommandos von `humanitl llm key` (HUM-140).
#[derive(Debug, Subcommand)]
pub enum KeyCmd {
    /// Whether a key is stored, and where: system keyring or 0600 file.
    Status,

    /// Store the key. It is read from stdin, never from the command line;
    /// at a terminal the input is hidden.
    Set,

    /// Remove the key and its reference from the configuration file.
    Remove,
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
    /// in the daemon (ProbeLlm): GET requests for the model list of the
    /// endpoint (and /api/tags first on a plain Ollama server without a path
    /// prefix), no credentials, no redirects.
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
    // Der Text der Doc-Kommentare an den Argumenten ist der Hilfetext von
    // `clap` und deshalb englisch (CONVENTIONS.md 3.9).
    /// The command to run in the sandbox, after `--`; without it the agent
    /// from the configuration.
    #[arg(last = true, value_name = "CMD")]
    pub cmd: Vec<OsString>,

    /// Keep the project folder untouched: it lies read-only under a layer in
    /// RAM, and after the run `humanitl sandbox changes` shows what the agent
    /// changed, `apply` writes it and `discard` drops it. The same as
    /// sandbox.mode = "disposable" for this run: where the system cannot do
    /// it, the start refuses instead of writing to the folder. Without this
    /// flag and without --direct the run follows sandbox.mode, by default
    /// "auto": disposable where the system allows it, otherwise direct with a
    /// notice.
    #[arg(long, conflicts_with = "direct")]
    pub disposable: bool,

    /// Let the agent write to the project folder directly, as without the
    /// disposable sandbox. The same as sandbox.mode = "direct" for this run.
    #[arg(long)]
    pub direct: bool,
}

/// Die Unterkommandos von `humanitl audit`.
///
/// Beide gehen zuerst an den Daemon. Nur `verify` prüft die Kette: Der Daemon
/// prüft mit dem HMAC-Schlüssel und den Ankern, und nur diese Prüfung ist die
/// ganze. `--file` ist die schwächere Fassung für eine Datei, die woanders
/// herkommt; sie prüft Kette und Kanonik und sagt in ihrer Ausgabe, was sie
/// nicht geprüft hat (HUM-070). `export` prüft nicht: Er verweigert eine
/// Zeile, die kein Record ist, auch eine leere, mit `AUDIT_001`, und kopiert
/// oder formatiert sonst die Records, ohne Hash, MAC oder Anker anzusehen
/// (HUM-214).
#[derive(Debug, Subcommand)]
pub enum AuditCmd {
    /// Check the hash chain: every record, its predecessor and the anchors.
    Verify {
        /// Verify this file instead of asking the daemon; without key and
        /// anchors, so the answer is weaker and says so.
        #[arg(long, value_name = "PATH")]
        file: Option<PathBuf>,
    },

    /// Write the records to a file, as JSON lines or as CSV.
    Export {
        /// `jsonl` keeps every record as it stands, `csv` is one row per record.
        #[arg(long, value_name = "FORMAT", value_parser = ["jsonl", "csv"])]
        format: String,

        /// Where the export is written. An existing file is not overwritten.
        #[arg(long, value_name = "FILE")]
        out: PathBuf,

        /// Only records at or after this point in time (RFC 3339).
        #[arg(long, value_name = "TS")]
        since: Option<String>,

        /// Only records before this point in time (RFC 3339).
        #[arg(long, value_name = "TS")]
        until: Option<String>,

        /// Export this file instead of asking the daemon.
        #[arg(long, value_name = "PATH")]
        file: Option<PathBuf>,
    },
}

/// Die Unterkommandos von `humanitl sandbox`.
#[derive(Debug, Subcommand)]
pub enum SandboxCmd {
    /// Start a sandbox with CMD as the agent and exit with its exit code.
    Run {
        /// Host directory mounted at /tests/escape of the `test` profile.
        #[arg(long, value_name = "DIR")]
        tests_dir: Option<PathBuf>,
        /// Run with the project folder read-only under a layer in RAM; at the
        /// end the changes are listed and discarded (this command keeps no
        /// layer; `humanitl run --disposable` keeps them for apply). Without
        /// this flag the run is direct unless sandbox.mode is "disposable":
        /// the default "auto" means direct here, because nothing could apply
        /// the changes afterwards.
        #[arg(long, conflicts_with = "direct")]
        disposable: bool,
        /// Run direct even if sandbox.mode is "disposable".
        #[arg(long)]
        direct: bool,
        /// The command to run in the sandbox, after `--`.
        #[arg(last = true, value_name = "CMD", required = true)]
        cmd: Vec<OsString>,
    },

    /// Print the bwrap command line for the current profile without starting it.
    Argv {
        /// The command the line ends with; without it the agent from the
        /// configuration, otherwise `/bin/sh`.
        #[arg(last = true, value_name = "CMD")]
        cmd: Vec<OsString>,
    },

    /// Start a short-lived sandbox and show the three isolation guarantees.
    Check,

    /// Attach this terminal to the running session.
    ///
    /// One client writes, any number watch. Ctrl+C goes to the session's
    /// terminal as byte 0x03, which interrupts its foreground program, as in
    /// any terminal; this terminal itself never reaches the sandbox. Detach
    /// with Ctrl+A d, as in screen; the session keeps running, and a later
    /// attach shows it. Ctrl+A Ctrl+A (or Ctrl+A a) sends one Ctrl+A to the
    /// agent.
    Attach {
        /// Watch without sending anything. Ctrl+A d detaches here too, and so
        /// does Ctrl+C.
        #[arg(long)]
        read_only: bool,
        /// The prefix of the detach key: ctrl-a to ctrl-z (not ctrl-h, ctrl-i,
        /// ctrl-j, ctrl-m: Backspace, Tab and Enter send them), ctrl-\,
        /// ctrl-], ctrl-^, ctrl-_, or none for no detach key. Default from
        /// `terminal.detach_prefix`, which is ctrl-a.
        #[arg(long, value_name = "SPEC")]
        detach_keys: Option<String>,
        /// Attach to a further shell from `humanitl sandbox exec` instead of
        /// the agent's terminal.
        #[arg(long, value_name = "ID")]
        shell: Option<String>,
    },

    /// Open a further shell in the running sandbox, next to the agent.
    ///
    /// Like docker exec: the shell gets a terminal of its own, the agent
    /// keeps running and keeps its terminal. The shell carries the same
    /// limits as the agent (filter, mounts, network through the proxy, rules)
    /// and sees the same project folder. It ends when its process exits or
    /// when the sandbox stops; at most four live at a time. Detach with
    /// Ctrl+A d, attach again with `humanitl sandbox attach --shell ID`.
    /// Without a terminal on stdin the command only watches the output.
    Exec {
        /// The prefix of the detach key, as for `humanitl sandbox attach`.
        #[arg(long, value_name = "SPEC")]
        detach_keys: Option<String>,
        /// The command to run, after `--`; without it an interactive bash,
        /// or sh when the sandbox has no bash.
        #[arg(last = true, value_name = "CMD")]
        cmd: Vec<OsString>,
    },

    /// Show what a disposable run changed: files changed, created and
    /// deleted, with a diff for text files and size and hash for others.
    ///
    /// Each row starts with the id that `humanitl sandbox apply` takes. A
    /// conflict means the file changed in the project since the start; a
    /// change marked "never applied" (a symbolic link out of the project, a
    /// device, FIFO or socket) is not written to the host.
    Changes {
        /// Only the list, without the diffs.
        #[arg(long)]
        stat: bool,
    },

    /// Write the changes of a disposable run to the project folder and
    /// discard the rest.
    ///
    /// Without IDs all changes; with IDs only those. Asks before it writes.
    /// Nothing is written when one of the chosen changes cannot be applied as
    /// it is; the changes keep waiting then.
    Apply {
        /// The ids from `humanitl sandbox changes`.
        #[arg(value_name = "ID")]
        ids: Vec<String>,
        /// Also write files that changed in the project since the start.
        #[arg(long)]
        overwrite: bool,
        /// Do not ask; for scripts.
        #[arg(long)]
        yes: bool,
    },

    /// Drop all changes of a disposable run. Asks first.
    Discard {
        /// Do not ask; for scripts.
        #[arg(long)]
        yes: bool,
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
        /// The id of the sandbox run.
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
    ///
    /// ID is the end of the flow id, as many characters as it takes to tell
    /// the rows of this page apart and at least eight; `flows show` takes it
    /// as it stands. DECISION reads `aborted` where the client gave up its TLS
    /// connection to the proxy before there was anything to decide (nobody
    /// blocked it; `flows show` names the cause), and `passthrough` for the
    /// requests to the model endpoint, which only --passthrough lists.
    List {
        /// The filter, for example `host:github.com state:blocked`. Several
        /// words are joined with one space and passed to the daemon
        /// unchanged.
        #[arg(value_name = "FILTER")]
        filter: Vec<String>,
        /// At most this many rows; 0 means the daemon's default.
        #[arg(long, default_value_t = 0, value_name = "N")]
        limit: u32,
        /// The column to sort by.
        #[arg(
            long,
            default_value = "ts",
            value_name = "KEY",
            value_parser = ["ts", "host", "duration", "size"]
        )]
        sort: String,
        /// Ascending instead of descending: the oldest row first.
        #[arg(long)]
        asc: bool,
        /// Include the requests the passthrough let through to the model
        /// endpoint; DECISION reads `passthrough` for them.
        #[arg(long)]
        passthrough: bool,
    },

    /// Show one flow, or one of its bodies.
    Show {
        /// The id of the flow: the whole id, or its end as `flows list`
        /// prints it (at least eight characters), looked up among the last
        /// 1000 flows.
        #[arg(value_name = "ID")]
        id: String,
        /// Print one of the two bodies instead of the fields.
        #[arg(long, value_name = "WHICH", value_parser = ["request", "response"])]
        body: Option<String>,
        /// Write the body byte for byte instead of as text.
        #[arg(long, requires = "body")]
        raw: bool,
    },

    /// Decide one waiting flow: allow it or block it.
    ///
    /// The human decision the window sends, from the terminal. It came before
    /// `--ask terminal` (HUM-021) and stays, because a script or a remote
    /// session has no window.
    Decide {
        /// The id of the flow.
        #[arg(value_name = "ID")]
        id: String,
        /// What happens to it.
        #[arg(value_name = "VERDICT", value_parser = ["allow", "block"])]
        verdict: String,
        /// A reason for the agent; it stands in the block body under `note:`.
        #[arg(long, value_name = "TEXT")]
        note: Option<String>,
        /// Die Regel, die die Entscheidung hinterlässt; ohne `--remember` keine.
        #[command(flatten)]
        remember: RememberArgs,
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
        /// Show bundled and expired rules as well.
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
        /// The id of the rule.
        #[arg(value_name = "ID")]
        id: String,
        /// Die Felder, die sich ändern sollen.
        #[command(flatten)]
        rule: RuleArgs,
    },

    /// Remove one rule.
    Remove {
        /// The id of the rule.
        #[arg(value_name = "ID")]
        id: String,
    },

    /// Move one rule to another place inside its own group.
    Reorder {
        /// The id of the rule.
        #[arg(value_name = "ID")]
        id: String,
        /// The new place, 1-based, inside the group of the rule.
        #[arg(value_name = "POSITION", value_parser = clap::value_parser!(u32).range(1..))]
        position: u32,
    },

    /// Show which of the last recorded requests a rule would have matched.
    #[command(name = "dry-run")]
    DryRun {
        /// Die Felder der Regel; --action und --host sind Pflicht.
        #[command(flatten)]
        rule: RuleArgs,
        /// How many of the most recently recorded requests to check; 0 means
        /// the daemon's default.
        #[arg(long, default_value_t = 0, value_name = "N")]
        scan: u32,
    },

    /// Switch a bundled rule off. It stays in the list and decides nothing.
    Disable {
        /// The id of the bundled rule.
        #[arg(value_name = "ID")]
        id: String,
    },

    /// Switch a bundled rule back on.
    Enable {
        /// The id of the bundled rule.
        #[arg(value_name = "ID")]
        id: String,
    },

    /// Read rules.yaml again and report what changed.
    Reload,

    /// Evaluate one URL against the rule set; exit 10 block, 11 ask.
    Test {
        /// The full URL, for example `https://api.github.com/repos/x`.
        #[arg(value_name = "URL")]
        url: String,
        /// The method of the imagined request.
        #[arg(
            long,
            value_name = "M",
            value_parser = PossibleValuesParser::new(RULE_METHODS),
            ignore_case = true
        )]
        method: Option<String>,
        /// Whether the imagined request carries an upgrade.
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

/// Die Regel, die eine Entscheidung hinterlässt.
///
/// Eigene Flags statt `#[command(flatten)] RuleArgs`: `RuleArgs` trägt ein
/// `--note`, und `flows decide` hat `--note` schon als Notiz an den Agenten
/// (HUM-072). Zwei Argumente mit demselben langen Namen lässt `clap` nicht zu
/// und bricht beim Start ab. Aus demselben Grund trägt hier jedes Argument ein
/// eigenes `id`: `clap` leitet die Kennung sonst aus dem Feldnamen ab, und
/// `note` gäbe es dann zweimal in `flows decide`. Die Brücke zurück ist
/// [`RememberArgs::rule_args`], damit `crate::cmd::rules::rule_from_args` die
/// einzige Stelle bleibt, die aus Flags eine Wire-Regel baut.
///
/// Es gibt kein Flag für die Aktion: Die Regel bekommt sie aus dem Verdikt,
/// damit keine Regel entstehen kann, die dem gerade gefällten Urteil
/// widerspricht.
#[derive(Debug, Clone, Args)]
pub struct RememberArgs {
    /// Host pattern of the rule to remember. Without it nothing is remembered.
    #[arg(id = "remember_host", long = "remember", value_name = "PATTERN")]
    pub host: Option<String>,

    /// One HTTP method for the rule; repeat the flag for more.
    #[arg(
        id = "remember_method",
        long = "remember-method",
        value_name = "M",
        requires = "remember_host",
        value_parser = PossibleValuesParser::new(RULE_METHODS),
        ignore_case = true
    )]
    pub method: Vec<String>,

    /// Path glob of the rule, or a regular expression when it starts with `~`.
    #[arg(
        id = "remember_path",
        long = "remember-path",
        value_name = "P",
        requires = "remember_host"
    )]
    pub path: Option<String>,

    /// never, session, or a point in time in RFC 3339. Without it: session.
    #[arg(
        id = "remember_expires",
        long = "remember-expires",
        value_name = "WHEN",
        requires = "remember_host"
    )]
    pub expires: Option<String>,

    /// Why the rule exists. It ends up in rules.yaml and in the window.
    #[arg(
        id = "remember_note",
        long = "remember-note",
        value_name = "TEXT",
        requires = "remember_host"
    )]
    pub note: Option<String>,
}

impl RememberArgs {
    /// Die Flags als [`RuleArgs`], oder `None`, wenn `--remember` fehlt.
    ///
    /// Ohne `--remember-expires` steht `session` in der Regel: Ein leeres
    /// `expires` liest `humanitl_ipc::convert::expiry_from_proto` als `never`,
    /// und eine dauerhafte Regel als Nebenwirkung einer einzelnen Freigabe ist
    /// genau die Überraschung, die das Produkt nicht macht.
    #[must_use]
    pub fn rule_args(&self, verdict: &str) -> Option<RuleArgs> {
        let host = self.host.clone()?;
        Some(RuleArgs {
            action: Some(verdict.to_owned()),
            host: Some(host),
            method: self.method.clone(),
            path: self.path.clone(),
            scheme: None,
            port: None,
            upgrade: None,
            expires: Some(self.expires.clone().unwrap_or_else(|| "session".to_owned())),
            note: self.note.clone(),
            position: None,
            allow_private: None,
        })
    }
}

/// Die Unterkommandos von `humanitl config`.
#[derive(Debug, Subcommand)]
pub enum ConfigCmd {
    /// Print the resolved value of one key, or of every key without one.
    Get {
        /// The path of the key, separated by dots; without it every leaf value.
        #[arg(value_name = "KEY")]
        key: Option<String>,

        /// Add the layer that set each value.
        #[arg(long)]
        origin: bool,
    },

    /// Write one key into config.toml, after checking it against the schema.
    Set {
        /// The path of the key, separated by dots.
        #[arg(value_name = "KEY")]
        key: String,

        /// The value, read by the type of the key: `5m` for a duration,
        /// `32MiB` for a size, `true`/`false`, JSON for a list, `null` or `-`
        /// to remove the key. A value that starts with `-` is a value, not a
        /// flag.
        #[arg(value_name = "VALUE", allow_hyphen_values = true)]
        value: String,

        /// Write into the profile of this project instead of the global file.
        #[arg(long)]
        project: bool,
    },

    /// Print the JSON schema of the configuration, or the list of profiles.
    Schema {
        /// List the profiles --profile can choose instead of the schema.
        #[arg(long)]
        profiles: bool,
    },

    /// Open config.toml in $VISUAL or $EDITOR and check it afterwards.
    Edit,
}

/// Die Unterkommandos von `humanitl daemon`.
#[derive(Debug, Subcommand)]
pub enum DaemonCmd {
    /// Ask the daemon who it is.
    Status,

    /// Write the systemd user unit that starts the daemon at login.
    Install(InstallArgs),

    /// Show the journal of the daemon: journalctl --user -u humanitld.
    Logs(LogsArgs),

    /// Restart the background service: launchctl kickstart on macOS,
    /// systemctl --user restart on Linux.
    Restart,

    /// Stop and disable the service and remove the unit daemon install wrote.
    Uninstall(UninstallArgs),

    /// Ask the daemon to end now if no agent session holds it (after an upgrade).
    RetireIfIdle,
}

/// Die Argumente von `humanitl daemon uninstall` (HUM-077).
///
/// Entfernt wird nur, was `daemon install` angelegt hat: die Unit mit der
/// Marke, die Verweise der Aktivierung und mit `--purge-binaries` die Kopien
/// aus einem `AppImage`. Die Units des Pakets gehören dem Paket; sie werden
/// abgemeldet, aber nicht gelöscht.
#[derive(Debug, Args)]
pub struct UninstallArgs {
    // Der Text der Doc-Kommentare ist der Hilfetext von `clap` und deshalb
    // englisch (CONVENTIONS.md 3.9).
    /// Also remove the copies an `AppImage` placed under `~/.local/lib/humanitl`.
    #[arg(long)]
    pub purge_binaries: bool,
}

/// Die Argumente von `humanitl daemon logs`.
///
/// Der Befehl ist eine Durchreiche an `journalctl`: Das Terminal geht
/// unverändert an das Kind, und sein Exit-Code wird der eigene. Ein zweiter
/// Leser für Journal-Einträge wäre eine zweite Quelle für dieselbe Wahrheit.
#[derive(Debug, Args)]
pub struct LogsArgs {
    // Der Text der Doc-Kommentare ist der Hilfetext von `clap` und deshalb
    // englisch (CONVENTIONS.md 3.9).
    /// Keep the journal open and show new lines as they arrive.
    #[arg(short = 'f', long)]
    pub follow: bool,

    /// How many lines to show; the default of journalctl without it.
    #[arg(short = 'n', long = "lines", value_name = "N")]
    pub lines: Option<u32>,
}

/// Die Argumente von `humanitl daemon install`.
///
/// Es gibt mit Absicht kein `--force`: Eine Unit-Datei, die Humanitl nicht
/// geschrieben hat, wird nie überschrieben, auch nicht auf Zuruf. Wer eine
/// eigene führt, legt sie beiseite; der Befund `DAEMON_005` nennt den Befehl
/// dazu.
#[derive(Debug, Args)]
pub struct InstallArgs {
    // Der Text der Doc-Kommentare ist der Hilfetext von `clap` und deshalb
    // englisch (CONVENTIONS.md 3.9).
    /// Show the unit and the commands, write nothing and start nothing.
    #[arg(long)]
    pub print: bool,

    /// Write the unit, but do not tell systemd about it.
    ///
    /// Without this, the command runs `systemctl --user daemon-reload` and
    /// `systemctl --user enable --now humanitld.service` after writing. Never
    /// with sudo: the daemon is a user service.
    #[arg(long)]
    pub no_start: bool,

    /// Take humanitld and humanitl-shim from this directory instead of from
    /// the directory of the running humanitl.
    ///
    /// From an `AppImage` (`$APPIMAGE` set), DIR/.. must also hold
    /// share/humanitl/catalog/domains.yaml and profiles/sandbox/default.toml,
    /// as in the image; they are copied with the binaries.
    #[arg(long = "bin-dir", value_name = "DIR")]
    pub bin_dir: Option<PathBuf>,

    /// Only from an `AppImage`, and only when daemon install ran before: if the
    /// installed copy is another version, copy this one and restart the
    /// service. Otherwise do nothing.
    ///
    /// The `AppImage` runs this at every start (`AppRun`, HUM-077); the first
    /// install stays a click in the setup or `--cli daemon install`.
    #[arg(long, conflicts_with_all = ["print", "no_start", "bin_dir"])]
    pub refresh: bool,
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
        AuditCmd, Cmd, ConfigCmd, EXIT_CODES_HELP, FlowsCmd, RulesCmd, SHORT_FLAGS, SandboxCmd,
        command, flag_name, parse,
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
            cmd: SandboxCmd::Run { cmd, tests_dir, .. },
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
            cmd: SandboxCmd::Run { cmd, tests_dir, .. },
        } = invocation.cli.cmd
        else {
            panic!("expected sandbox run");
        };
        assert_eq!(tests_dir.as_deref(), Some(Path::new("tests/escape")));
        assert_eq!(cmd, ["/bin/sh", "/tests/escape/esc-1-sockets.sh"]);
    }

    /// `--direct` and `--disposable` choose the mode of one run, for `run`
    /// and `sandbox run`, and never both at once (HUM-357).
    #[test]
    fn direct_and_disposable_are_flags_of_one_run_and_exclude_each_other() {
        let invocation = parse(["humanitl", "run", "--direct"]).expect("--direct parses");
        let Cmd::Run(args) = invocation.cli.cmd else {
            panic!("expected run");
        };
        assert!(args.direct && !args.disposable);

        let invocation =
            parse(["humanitl", "sandbox", "run", "--direct", "--", "true"]).expect("parses");
        let Cmd::Sandbox {
            cmd: SandboxCmd::Run {
                direct, disposable, ..
            },
        } = invocation.cli.cmd
        else {
            panic!("expected sandbox run");
        };
        assert!(direct && !disposable);

        for argv in [
            &["humanitl", "run", "--direct", "--disposable"][..],
            &[
                "humanitl",
                "sandbox",
                "run",
                "--disposable",
                "--direct",
                "--",
                "true",
            ][..],
        ] {
            let refused = parse(argv.iter().copied()).expect_err("both at once");
            assert_eq!(
                refused.kind(),
                clap::error::ErrorKind::ArgumentConflict,
                "{argv:?}"
            );
        }
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
            parse(["humanitl", "--experimental-h2-upstream", "daemon", "status"]).expect("parses");
        assert!(
            invocation
                .config
                .iter()
                .any(|(key, value)| key == "experimental.h2_upstream" && value == "true")
        );

        let explicit = parse([
            "humanitl",
            "--experimental-h2-upstream=false",
            "daemon",
            "status",
        ])
        .expect("parses");
        assert!(
            explicit
                .config
                .iter()
                .any(|(key, value)| key == "experimental.h2_upstream" && value == "false")
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

    /// `audit export` nimmt beide Grenzen und das Ziel entgegen; `--format`
    /// nimmt nur die beiden Formate des Vertrags.
    #[test]
    fn audit_export_reads_its_range_and_refuses_an_unknown_format() {
        let invocation = parse([
            "humanitl",
            "audit",
            "export",
            "--format",
            "csv",
            "--out",
            "/tmp/audit.csv",
            "--since",
            "2026-09-01T00:00:00Z",
        ])
        .expect("parses");
        let Cmd::Audit {
            cmd:
                AuditCmd::Export {
                    format,
                    out,
                    since,
                    until,
                    file,
                },
        } = invocation.cli.cmd
        else {
            panic!("expected audit export");
        };
        assert_eq!(format, "csv");
        assert_eq!(out, Path::new("/tmp/audit.csv"));
        assert_eq!(since.as_deref(), Some("2026-09-01T00:00:00Z"));
        assert_eq!(until, None);
        assert_eq!(file, None);

        assert!(
            parse([
                "humanitl", "audit", "export", "--format", "xml", "--out", "/tmp/a"
            ])
            .is_err(),
            "only jsonl and csv are in the contract"
        );
    }

    /// `config get` ohne Schlüssel ist die ganze Konfiguration und kein
    /// Aufruffehler.
    #[test]
    fn config_get_without_a_key_is_every_leaf() {
        let invocation = parse(["humanitl", "config", "get", "--origin"]).expect("parses");
        let Cmd::Config {
            cmd: ConfigCmd::Get { key, origin },
        } = invocation.cli.cmd
        else {
            panic!("expected config get");
        };
        assert_eq!(key, None);
        assert!(origin);
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
                    passthrough,
                },
        } = invocation.cli.cmd
        else {
            panic!("expected flows list");
        };
        assert_eq!(filter, ["host:github.com", "findings:>0"]);
        assert_eq!(sort, "size");
        assert!(asc);
        assert_eq!(limit, 0);
        assert!(!passthrough, "the model traffic stays out unless asked for");
    }

    /// `--passthrough` nimmt die Durchreiche zum Modell in die Liste (HUM-316).
    #[test]
    fn passthrough_is_a_flag_of_flows_list() {
        let invocation = parse(["humanitl", "flows", "list", "--passthrough"]).expect("parses");
        let Cmd::Flows {
            cmd: FlowsCmd::List { passthrough, .. },
        } = invocation.cli.cmd
        else {
            panic!("expected flows list");
        };
        assert!(passthrough);
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
