//! `humanitl daemon status` und `humanitl daemon install`.
//!
//! `status` ist ein dünner Client (ADR-018): verbinden, `GetInfo` rufen,
//! ausgeben. Was der Daemon kann, sagt er selbst; die Kommandozeile erfindet
//! nichts dazu.
//!
//! `install` ist das eine Unterkommando, das etwas auf dem Rechner des
//! Menschen zurücklässt. Was es schreibt und wohin, steht in [`crate::cmd::unit`];
//! hier steht, was drum herum geschieht, und das sind vier Zusagen:
//!
//! - **Es zeigt die Datei, bevor es sie schreibt.** Der ganze Text der Unit
//!   und beide `systemctl`-Aufrufe gehen vor dem ersten Schreibzugriff auf
//!   `stderr`, an [`crate::render::Renderer`] vorbei ([`announce`]). Wie bei
//!   der Ankündigung des Doctors darf kein Ausgabeschalter darüber
//!   entscheiden, ob ein Mensch erfährt, was gleich auf seiner Platte landet.
//! - **`--print` schreibt nichts.** Wer erst lesen will, bekommt genau
//!   dieselbe Datei zu sehen, die der Aufruf ohne den Schalter schriebe.
//! - **Nie mit `sudo`.** Der Daemon ist ein Nutzerdienst; jeder Aufruf hier
//!   ist `systemctl --user`.
//! - **Ein Fehlschlag lässt nichts liegen.** Nimmt systemd die Unit nicht an,
//!   wird der Zustand von vorher wiederhergestellt — die Datei **und** die
//!   Verweise, die `enable --now` angelegt hat — und `daemon-reload` noch
//!   einmal gefahren, damit auch systemds Bild davon stimmt.
//!
//! Die erste Zusage hängt an der Reihenfolge in [`install`]: Der Plan aus
//! [`crate::cmd::unit::prepare`] steht fest, bevor [`announce`] etwas sagt.
//! `prepare` liest nur, also ist vor der Ankündigung nach wie vor nichts
//! geschrieben — aber die Ankündigung nennt jetzt, was wirklich geschieht, und
//! nicht, was ein Aufruf im Regelfall täte.

use std::path::{Path, PathBuf};
use std::time::Duration;

use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, FixAction, Severity};
use humanitl_ipc::{PROTO_MAJOR, PROTO_MINOR, v1};
use serde_json::json;

use crate::cli::{DaemonCmd, InstallArgs};
use crate::cmd::{Context, EXIT_OK, Failure, status_diagnostic, unit};
use crate::render::table;

/// Wie lange ein `systemctl`-Aufruf höchstens dauern darf.
///
/// Ohne Frist hinge `daemon install` an einem systemd, das seinen Bus nicht
/// findet — und der Mensch säße vor einem Befehl, der eine Datei geschrieben
/// hat und nicht mehr zurückkommt.
const SYSTEMCTL_TIMEOUT: Duration = Duration::from_secs(20);

/// Das Wort, unter dem `--print` in der Ausgabe steht.
const PRINT_ACTION: &str = "print";

/// Die Variablen, die `systemctl --user` mitbekommt.
///
/// Nicht die ganze Umgebung: In ihr stehen Tokens und Schlüssel des Nutzers.
/// Nicht die leere: Ohne `$XDG_RUNTIME_DIR` und `$DBUS_SESSION_BUS_ADDRESS`
/// findet `systemctl --user` den Bus der Sitzung nicht und antwortet „Failed
/// to connect to user scope bus" — dieselbe Lehre wie in
/// `humanitl_sandbox::doctor` (HUM-075, Punkt 5).
const SESSION_ENV_KEYS: &[&str] = &[
    "PATH",
    "HOME",
    "XDG_RUNTIME_DIR",
    "DBUS_SESSION_BUS_ADDRESS",
];

/// Führt `humanitl daemon <cmd>` aus.
///
/// # Errors
///
/// `DAEMON_001`, wenn kein Daemon antwortet, `DAEMON_002`, wenn er eine
/// andere Major-Version des Vertrags spricht, `DAEMON_005` bis `DAEMON_008`
/// für die Wege, auf denen `install` nicht durchkommt.
pub async fn run(ctx: &Context, cmd: &DaemonCmd) -> Result<u8, Failure> {
    match cmd {
        DaemonCmd::Status => status(ctx).await,
        DaemonCmd::Install(args) => install(ctx, args).await,
    }
}

/// Was aus dem Versuch wurde, systemd von der Unit zu erzählen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Activation {
    /// `daemon-reload` und `enable --now` sind durchgelaufen.
    Enabled,
    /// `--no-start`: Es wurde nichts gerufen.
    Skipped,
    /// Es gibt kein `systemctl` in `PATH`. Die Unit liegt trotzdem richtig da.
    NoSystemctl,
}

impl Activation {
    /// Das Wort für die Ausgabe.
    const fn as_str(self) -> &'static str {
        match self {
            Self::Enabled => "enabled",
            Self::Skipped => "skipped",
            Self::NoSystemctl => "no systemctl",
        }
    }
}

/// `daemon install`.
async fn install(ctx: &Context, args: &InstallArgs) -> Result<u8, Failure> {
    let current = std::env::current_exe().map_err(|error| {
        Failure::new(
            Diagnostic::builder(codes::DAEMON_007, Severity::Blocking)
                .why(format!(
                    "the path of the running humanitl could not be read ({error}), so the unit \
                     has no ExecStart to name"
                ))
                .build(),
        )
    })?;
    let daemon = unit::daemon_binary(&current).map_err(Failure::new)?;
    let contents = unit::render(&daemon).map_err(Failure::new)?;
    let path = unit::unit_path(&ctx.paths);
    let systemctl = find_in_path(ctx, "systemctl");

    if args.print {
        announce(&path, &contents, None, args.no_start, systemctl.as_deref());
        report(ctx, &path, &daemon, PRINT_ACTION, Activation::Skipped);
        return Ok(EXIT_OK);
    }

    // Erst den Plan, dann die Ankündigung. [`unit::prepare`] liest nur; es
    // schreibt nichts, und die Zusage „sichtbar, bevor es geschieht" bleibt
    // damit unberührt. Umgekehrt wäre die Ankündigung eine Behauptung: Liegt
    // dort die Unit von jemand anderem (`DAEMON_005`) oder steht schon genau
    // dieser Text da, dann hätte der Mensch gelesen, dass seine Datei
    // geschrieben wird, und geschrieben wird sie nicht.
    let plan = unit::prepare(&path, &contents).map_err(Failure::new)?;
    announce(
        &path,
        &contents,
        Some(&plan),
        args.no_start,
        systemctl.as_deref(),
    );
    unit::write(&path, &contents, &plan).map_err(Failure::new)?;

    let activation = match (args.no_start, systemctl) {
        (true, _) => Activation::Skipped,
        (false, None) => Activation::NoSystemctl,
        (false, Some(systemctl)) => {
            activate(ctx, &systemctl, &path, &plan).await?;
            Activation::Enabled
        }
    };

    report(ctx, &path, &daemon, plan.as_str(), activation);
    Ok(EXIT_OK)
}

/// Sagt systemd Bescheid, und nimmt zurück, was dieser Aufruf angerichtet hat,
/// wenn es nicht klappt.
///
/// Zurückgenommen wird beides, was ein Lauf hinterlassen kann: die Unit und
/// ihre Aktivierung. `systemctl --user enable --now` ist ein Aufruf mit zwei
/// Schritten — es legt die Verweise unter `<ziel>.wants/` an und startet dann
/// den Dienst —, und misslingt der Start, bleiben die Verweise stehen. Ohne
/// ihre Rücknahme wäre `daemon install` nicht das eine Geschäft, als das es in
/// `docs/cli.md` steht: Der Dienst startete beim nächsten Anmelden, obwohl der
/// Befehl mit `DAEMON_008` abgebrochen ist.
///
/// [`unit::Enablement`] nimmt den Zustand vor dem ersten Aufruf auf, damit
/// wirklich nur zurückgenommen wird, was dieser Lauf angelegt hat: Wer den
/// Dienst schon vorher aktiviert hatte, behält ihn.
async fn activate(
    ctx: &Context,
    systemctl: &Path,
    path: &Path,
    plan: &unit::Written,
) -> Result<(), Failure> {
    let dir = path
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    let before = unit::Enablement::read(&dir);
    for step in [
        vec!["--user", "daemon-reload"],
        vec!["--user", "enable", "--now", unit::UNIT_NAME],
    ] {
        if let Err(why) = systemctl_run(ctx, systemctl, &step).await {
            // Zuerst den Dienst anhalten, aber nur, wenn dieser Lauf ihn
            // gestartet haben kann.
            //
            // `enable --now` startet die Unit. Scheitert sie dabei -- der
            // Daemon bricht ab, die Unit steht auf `failed` --, dann bleibt sie
            // in systemds Gedächtnis stehen, und `Restart=on-failure` versucht
            // es weiter. Eine Rücknahme, die nur die Datei wegnimmt, lässt
            // einen Neustartversuch auf eine Unit los, die es nicht mehr gibt.
            //
            // **Nur bei diesem Schritt.** Scheitert schon `daemon-reload`, hat
            // dieser Lauf nichts gestartet, und ein `stop` träfe den Daemon,
            // den der Mensch vorher selbst laufen hatte. Etwas anzuhalten, das
            // man nicht gestartet hat, ist schlimmer als ein Rest im
            // Gedächtnis von systemd.
            //
            // Beide Aufrufe ohne Prüfung des Ergebnisses: Sie räumen auf, und
            // ein Fehler dabei ändert nichts an dem Befund, der gleich
            // zurückgeht.
            if step.contains(&"enable") {
                let _ = systemctl_run(ctx, systemctl, &["--user", "stop", unit::UNIT_NAME]).await;
                let _ = systemctl_run(ctx, systemctl, &["--user", "reset-failed", unit::UNIT_NAME])
                    .await;
            }
            // Reihenfolge: erst die Verweise, dann die Unit. Andersherum stünde
            // zwischendurch ein Verweis auf eine Datei, die es nicht mehr gibt.
            let disabled = before.rollback(&dir);
            let taken_back = unit::rollback(path, plan);
            // Der zweite `daemon-reload` gehört zur Rücknahme: Nach dem ersten
            // kennt systemd die Unit, und eine, die es kennt und die nicht
            // mehr auf der Platte liegt, ist ein halber Zustand.
            let _ = systemctl_run(ctx, systemctl, &["--user", "daemon-reload"]).await;
            return Err(Failure::new(not_taken(
                path,
                &step,
                &why,
                plan,
                &taken_back,
                &disabled,
            )));
        }
    }
    Ok(())
}

/// Ein `systemctl`-Aufruf mit Frist und ohne die Umgebung des Nutzers.
///
/// `Ok(())` bei Exit 0, sonst ein Satz, der sagt, woran es lag.
async fn systemctl_run(ctx: &Context, systemctl: &Path, args: &[&str]) -> Result<(), String> {
    let mut command = tokio::process::Command::new(systemctl);
    command.args(args).env_clear().kill_on_drop(true);
    for key in SESSION_ENV_KEYS {
        if let Some(value) = ctx.env.non_empty(key) {
            command.env(key, value);
        }
    }
    let child = command.stdin(std::process::Stdio::null()).output();
    match tokio::time::timeout(SYSTEMCTL_TIMEOUT, child).await {
        Err(_elapsed) => Err(format!(
            "no answer within {} ms",
            SYSTEMCTL_TIMEOUT.as_millis()
        )),
        Ok(Err(error)) => Err(error.to_string()),
        Ok(Ok(output)) if output.status.success() => Ok(()),
        Ok(Ok(output)) => {
            let text = String::from_utf8_lossy(&output.stderr);
            let first = text
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .unwrap_or("-");
            Err(format!(
                "{}: {first}",
                output.status.code().map_or_else(
                    || "killed by a signal".to_owned(),
                    |code| format!("exit {code}")
                )
            ))
        }
    }
}

/// `DAEMON_008`: systemd hat die Unit nicht angenommen.
fn not_taken(
    path: &Path,
    step: &[&str],
    why: &str,
    plan: &unit::Written,
    taken_back: &Result<(), Diagnostic>,
    disabled: &Result<(), Diagnostic>,
) -> Diagnostic {
    let undone = match (plan, taken_back) {
        (unit::Written::Unchanged, _) => "nothing had been written".to_owned(),
        (_, Ok(())) => format!("{} is back the way it was", path.display()),
        (_, Err(diagnostic)) => format!(
            "and taking {} back again failed as well: {}",
            path.display(),
            diagnostic.why
        ),
    };
    // Eine gelungene Rücknahme der Aktivierung steht nicht im Text: Sie ist der
    // Normalfall und sagt einem Menschen nichts, was er tun müsste. Eine
    // misslungene steht darin, weil dann etwas liegen bleibt.
    let enablement = match disabled {
        Ok(()) => String::new(),
        Err(diagnostic) => format!("; {}", diagnostic.why),
    };
    Diagnostic::builder(codes::DAEMON_008, Severity::Blocking)
        .why(format!(
            "systemctl {} did not go through ({why}); {undone}{enablement}",
            step.join(" ")
        ))
        .fix(unit_fix(&[
            "systemctl",
            "--user",
            "status",
            unit::UNIT_NAME,
        ]))
        .build()
}

/// Sagt, was gleich geschieht — vor dem ersten Schreibzugriff.
///
/// **Geht mit Absicht nicht durch [`crate::render::Renderer`].** Dessen `note`
/// schweigt unter `--json` und unter `-q`, und ein Ausgabeschalter darf nicht
/// bestimmen, ob ein Mensch sieht, welche Datei sein Rechner gleich bekommt.
/// Dieselbe Regel wie bei der Ankündigung des Doctors (`cmd/doctor.rs`).
/// `stdout` bleibt unberührt, ein einziger JSON-Wert also weiterhin ein
/// einziger.
fn announce(
    path: &Path,
    contents: &str,
    plan: Option<&unit::Written>,
    no_start: bool,
    systemctl: Option<&Path>,
) {
    eprintln!("{}", headline(path, plan));
    eprintln!();
    for line in contents.lines() {
        eprintln!("  {line}");
    }
    eprintln!();
    match (plan, no_start, systemctl) {
        (None, _, _) => eprintln!("--print: nothing is written and nothing is started"),
        (Some(_), true, _) => eprintln!("--no-start: systemd is not told about the unit"),
        (Some(_), false, None) => {
            eprintln!("no systemctl in PATH: nothing is started");
        }
        (Some(_), false, Some(systemctl)) => {
            let shown = systemctl.display();
            eprintln!("then: {shown} --user daemon-reload");
            eprintln!(
                "then: {shown} --user enable --now {} (never with sudo)",
                unit::UNIT_NAME
            );
        }
    }
}

/// Die erste Zeile der Ankündigung: was mit dieser Datei geschieht.
///
/// `None` heißt `--print`. Sonst steht hier der Plan aus [`unit::prepare`], und
/// er steht so, wie er ausgeht: Eine Datei, die schon genau diesen Text trägt,
/// wird nicht geschrieben, und der Satz sagt das, statt es zu behaupten.
fn headline(path: &Path, plan: Option<&unit::Written>) -> String {
    let shown = path.display();
    match plan {
        None => format!("humanitl daemon install would write {shown}:"),
        Some(unit::Written::Created) => format!("humanitl daemon install writes {shown}:"),
        Some(unit::Written::Replaced { .. }) => {
            format!("humanitl daemon install replaces its own older {shown} with:")
        }
        Some(unit::Written::Unchanged) => {
            format!("humanitl daemon install writes nothing: {shown} already carries exactly this:")
        }
    }
}

/// Das Ergebnis: JSON oder Tabelle.
fn report(ctx: &Context, path: &Path, daemon: &Path, action: &str, activation: Activation) {
    if ctx.render.is_json() {
        ctx.render.value(&json!({
            "unit": path.display().to_string(),
            "exec_start": daemon.display().to_string(),
            "action": action,
            "activation": activation.as_str(),
        }));
        return;
    }
    let rows = vec![
        vec!["unit".to_owned(), path.display().to_string()],
        vec!["exec_start".to_owned(), daemon.display().to_string()],
        vec!["action".to_owned(), action.to_owned()],
        vec!["activation".to_owned(), activation.as_str().to_owned()],
    ];
    print!("{}", table(&["FIELD", "VALUE"], &rows));
    // Nur, wenn die Datei wirklich liegt und niemand sie gestartet hat. Unter
    // `--print` steht nichts da, und ein Hinweis, der das Gegenteil behauptet,
    // wäre die Sorte Satz, gegen die dieses Produkt gebaut ist.
    if action != PRINT_ACTION
        && (activation == Activation::NoSystemctl || activation == Activation::Skipped)
    {
        ctx.render.note(&format!(
            "the unit is in place; systemctl --user daemon-reload and systemctl --user enable \
             --now {} start it",
            unit::UNIT_NAME
        ));
    }
}

/// Sucht ein Programm im `PATH` der übergebenen Umgebung, nie in der des
/// Prozesses.
fn find_in_path(ctx: &Context, program: &str) -> Option<PathBuf> {
    let path = ctx.env.non_empty("PATH")?;
    path.split(':')
        .filter(|dir| !dir.is_empty())
        .map(|dir| Path::new(dir).join(program))
        .find(|candidate| candidate.is_file())
}

/// Ein Vorschlag, der beweisbar dieselben Wörter bleibt.
fn unit_fix(words: &[&str]) -> FixAction {
    humanitl_sandbox::doctor::command_fix(words)
}

/// `daemon status`.
async fn status(ctx: &Context) -> Result<u8, Failure> {
    let mut client = ctx.connect().await?;
    let info = client
        .get_info(())
        .await
        .map_err(|status| Failure::new(status_diagnostic(&status, "GetInfo")))?
        .into_inner();

    check_proto(&info)?;

    let socket = ctx.paths.daemon_socket().display().to_string();
    if ctx.render.is_json() {
        ctx.render.value(&json!({
            "socket": socket,
            "daemon_version": info.daemon_version,
            "proto_major": info.proto_major,
            "proto_minor": info.proto_minor,
            "capabilities": info.capabilities,
            "session_id": info.session_id,
        }));
        return Ok(EXIT_OK);
    }

    let session = if info.session_id.is_empty() {
        "-".to_owned()
    } else {
        info.session_id.clone()
    };
    let rows = vec![
        vec!["socket".to_owned(), socket],
        vec!["daemon".to_owned(), info.daemon_version.clone()],
        vec![
            "proto".to_owned(),
            format!("{}.{}", info.proto_major, info.proto_minor),
        ],
        vec!["session".to_owned(), session],
        vec![
            "capabilities".to_owned(),
            if info.capabilities.is_empty() {
                "-".to_owned()
            } else {
                info.capabilities.join(", ")
            },
        ],
    ];
    print!("{}", table(&["FIELD", "VALUE"], &rows));
    Ok(EXIT_OK)
}

/// Prüft, ob Client und Daemon dieselbe Major-Version sprechen.
///
/// Eine andere Major heißt: Nachrichten, die der eine schickt, versteht der
/// andere nicht mehr. Eine kleinere Minor beim Daemon ist dagegen kein
/// Fehler, nur eine Notiz: additive Änderungen bleiben lesbar.
pub fn check_proto(info: &v1::Info) -> Result<(), Failure> {
    if info.proto_major == PROTO_MAJOR {
        return Ok(());
    }
    Err(Failure::new(
        Diagnostic::builder(codes::DAEMON_002, Severity::Blocking)
            .why(format!(
                "the daemon speaks contract {}.{}, this humanitl speaks {PROTO_MAJOR}.{PROTO_MINOR}",
                info.proto_major, info.proto_minor
            ))
            .fix(FixAction::CopyCommand(
                "systemctl --user restart humanitld".to_owned(),
            ))
            .build(),
    ))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_ipc::{PROTO_MAJOR, PROTO_MINOR, v1};

    use super::check_proto;
    use crate::cmd::EXIT_DAEMON;

    fn info(major: u32) -> v1::Info {
        v1::Info {
            daemon_version: "0.0.0".to_owned(),
            proto_major: major,
            proto_minor: PROTO_MINOR,
            capabilities: vec!["hold".to_owned()],
            session_id: String::new(),
        }
    }

    #[test]
    fn the_same_major_is_accepted() {
        assert!(check_proto(&info(PROTO_MAJOR)).is_ok());
    }

    #[test]
    fn another_major_is_daemon_002_with_exit_two() {
        let failure = check_proto(&info(PROTO_MAJOR + 1)).expect_err("a newer daemon is refused");
        assert_eq!(failure.diagnostic.code.as_str(), "DAEMON_002");
        assert_eq!(failure.exit, EXIT_DAEMON);
    }
}
