//! `humanitl sandbox changes|apply|discard` (HUM-350).
//!
//! Die Kommandozeile ist ein dünner Client (ADR-018): Diff, Prüfung und
//! Schreiben macht der Daemon über `SandboxRequest.changes`,
//! `.apply_changes` und `.discard_changes`; hier wird nur gezeigt und
//! gefragt. Übernehmen und Verwerfen fragen vorher nach, denn beides lässt
//! sich nicht rückgängig machen; `--yes` antwortet für Skripte.

use std::fmt::Write as _;
use std::io::{BufRead as _, IsTerminal as _};

use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, FixAction, Severity};
use humanitl_ipc::v1;
use serde_json::json;

use crate::cmd::{Context, EXIT_OK, Failure, from_proto, status_diagnostic};

/// Die erste Minor des Vertrags, die die Wegwerf-Sandbox kennt.
pub const DISPOSABLE_MINOR: u32 = 25;

/// `sandbox changes`: die wartenden Änderungen mit ihren Text-Diffs.
///
/// # Errors
///
/// `DAEMON_001` ohne Daemon, `CLI_004` für einen Daemon vor Vertrag 1.25,
/// `SANDBOX_031`, wenn nichts wartet.
pub async fn changes(ctx: &Context, stat: bool) -> Result<u8, Failure> {
    let changes = fetch(ctx).await?;
    if ctx.render.is_json() {
        ctx.render.value(&changes_json(&changes));
        return Ok(EXIT_OK);
    }
    for line in listing(&changes) {
        println!("{line}");
    }
    if !stat {
        print_diffs(&changes);
    }
    ctx.render
        .note("Apply: humanitl sandbox apply [ID ...]   Discard: humanitl sandbox discard");
    Ok(EXIT_OK)
}

/// `sandbox apply`: die gewählten Änderungen übernehmen, den Rest verwerfen.
///
/// # Errors
///
/// Wie [`changes`], dazu `CLI_004`, wenn ohne Terminal und ohne `--yes`
/// niemand bestätigen kann, und die Befunde des Daemons (`SANDBOX_031`), wenn
/// eine Prüfung vor dem ersten Schreiben scheitert.
pub async fn apply(
    ctx: &Context,
    ids: &[String],
    overwrite: bool,
    yes: bool,
) -> Result<u8, Failure> {
    let changes = fetch(ctx).await?;
    let chosen: Vec<&v1::DisposableChange> = if ids.is_empty() {
        changes.changes.iter().collect()
    } else {
        changes
            .changes
            .iter()
            .filter(|change| ids.iter().any(|id| id.trim() == change.id))
            .collect()
    };
    if !yes {
        let rest = changes.changes.len().saturating_sub(chosen.len());
        let question = format!(
            "Apply {} change(s) to {} and discard {rest} other(s)?",
            chosen.len(),
            changes.work_dir
        );
        let shown: Vec<String> = chosen.iter().map(|change| row(change)).collect();
        confirm(ctx, &question, &shown, "apply")?;
    }
    send_apply(ctx, &changes.sandbox_id, ids, overwrite, Out::Result).await?;
    Ok(EXIT_OK)
}

/// Schickt `ApplyChanges` und zeigt das Ergebnis.
async fn send_apply(
    ctx: &Context,
    sandbox_id: &str,
    ids: &[String],
    overwrite: bool,
    out: Out,
) -> Result<(), Failure> {
    let request = v1::sandbox_request::ApplyChanges {
        sandbox_id: sandbox_id.to_owned(),
        ids: ids.iter().map(|id| id.trim().to_owned()).collect(),
        overwrite,
        // `--overwrite` gilt für jede gewählte Änderung; einzeln bestätigt
        // nur die Oberfläche (HUM-351).
        overwrite_ids: Vec::new(),
    };
    let outcome = outcome_of(ask(ctx, v1::sandbox_request::Op::ApplyChanges(request)).await?)?;
    report(ctx, &outcome, out);
    Ok(())
}

/// `sandbox discard`: alle wartenden Änderungen verwerfen.
///
/// # Errors
///
/// Wie [`apply`].
pub async fn discard(ctx: &Context, yes: bool) -> Result<u8, Failure> {
    let changes = fetch(ctx).await?;
    if !yes {
        let question = format!(
            "Discard all {} change(s) of sandbox {}? They cannot be brought back.",
            changes.changes.len(),
            changes.sandbox_id
        );
        let shown: Vec<String> = changes.changes.iter().map(row).collect();
        confirm(ctx, &question, &shown, "discard")?;
    }
    send_discard(ctx, &changes.sandbox_id, Out::Result).await?;
    Ok(EXIT_OK)
}

/// Schickt `DiscardChanges` und zeigt das Ergebnis.
async fn send_discard(ctx: &Context, sandbox_id: &str, out: Out) -> Result<(), Failure> {
    let request = v1::sandbox_request::DiscardChanges {
        sandbox_id: sandbox_id.to_owned(),
    };
    let outcome = outcome_of(ask(ctx, v1::sandbox_request::Op::DiscardChanges(request)).await?)?;
    report(ctx, &outcome, out);
    Ok(())
}

/// Was der Mensch nach einem Lauf von `humanitl go` mit den wartenden
/// Änderungen tun will (HUM-496).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    /// Alle übernehmen.
    Apply,
    /// Alle verwerfen.
    Discard,
    /// Die Diffs zeigen und noch einmal fragen.
    View,
    /// Warten lassen; `humanitl sandbox changes|apply|discard` später.
    Keep,
}

/// Liest die Antwort auf die Frage nach dem Lauf. Alles außer `a`, `d` und
/// `v` heißt warten lassen: Eine Eingabe, die niemand so gemeint hat, ändert
/// nichts im Projekt.
#[must_use]
pub fn choice(answer: &str) -> Choice {
    match answer.trim().to_ascii_lowercase().as_str() {
        "a" | "apply" => Choice::Apply,
        "d" | "discard" => Choice::Discard,
        "v" | "view" => Choice::View,
        _ => Choice::Keep,
    }
}

/// Die Text-Diffs aller Änderungen auf `stdout`.
fn print_diffs(changes: &v1::DisposableChanges) {
    write_diffs(changes, Out::Result);
}

/// Die Text-Diffs aller Änderungen auf `stdout`, auf dem Weg von `out`.
fn write_diffs(changes: &v1::DisposableChanges, out: Out) {
    for change in &changes.changes {
        if !change.diff.is_empty() {
            out.write(format_args!("\n{}", change.diff));
        }
    }
}

/// Wie ein Ergebnis auf `stdout` geht.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Out {
    /// Das Ergebnis eines Befehls wie `sandbox changes`: über `print!`,
    /// damit eine geschlossene Pipe ihn still mit 141 beendet (HUM-285).
    Result,
    /// Die Antwort an der Frage nach den Änderungen von `humanitl go`:
    /// ohne Panik, wenn das Terminal inzwischen weg ist (GitHub #73).
    Review,
}

impl Out {
    /// Schreibt `text` auf `stdout`, wie es zu diesem Weg gehört.
    fn write(self, text: std::fmt::Arguments<'_>) {
        match self {
            Self::Result => print!("{text}"),
            Self::Review => crate::render::to_stdout(text),
        }
    }
}

/// Was auf die Frage nach dem Lauf kam.
#[derive(Debug, PartialEq, Eq)]
pub enum Answer {
    /// Eine Zeile vom Terminal; leer am Ende der Eingabe.
    Line(String),
    /// Ein Signal mit dieser Nummer.
    Signal(u8),
}

/// Die Zeile oder das Signal, was zuerst kommt. Ein Signal gewinnt, wenn
/// beides bereitsteht: `Ctrl+C` heißt nichts tun.
pub async fn line_or_signal<L, S>(line: L, signal: S) -> Answer
where
    L: std::future::Future<Output = String>,
    S: std::future::Future<Output = u8>,
{
    tokio::select! {
        biased;
        number = signal => Answer::Signal(number),
        text = line => Answer::Line(text),
    }
}

/// Liest eine Zeile von `stdin` in einem eigenen Thread.
///
/// Nicht in einem Thread der Laufzeit: Ein Lesen, das nach einem Signal
/// stehen bleibt, hielte sonst nichts auf, aber es gehört niemandem mehr.
/// Der Thread endet mit dem Prozess.
fn read_line() -> impl std::future::Future<Output = String> {
    let (tell, line) = tokio::sync::oneshot::channel();
    let _ = std::thread::Builder::new()
        .name("humanitl-answer".to_owned())
        .spawn(move || {
            let mut answer = String::new();
            let _ = std::io::stdin().lock().read_line(&mut answer);
            let _ = tell.send(answer);
        });
    async move { line.await.unwrap_or_default() }
}

/// Die Signale `SIGINT`, `SIGTERM` und `SIGHUP`, ab jetzt gehört.
///
/// Die Ströme von `drive` sind nach dem Lauf fallengelassen, der Handler der
/// Laufzeit aber bleibt: Ohne eigene Ströme verschluckte er ein `Ctrl+C` an
/// der Frage. Die Ströme entstehen beim Aufruf, nicht erst beim ersten Warten.
struct Signals(
    Option<(
        tokio::signal::unix::Signal,
        tokio::signal::unix::Signal,
        tokio::signal::unix::Signal,
    )>,
);

impl Signals {
    /// Legt die Ströme an; ohne Handler wartet [`Signals::next`] für immer.
    fn listen() -> Self {
        use tokio::signal::unix::{SignalKind, signal};

        match (
            signal(SignalKind::interrupt()),
            signal(SignalKind::terminate()),
            signal(SignalKind::hangup()),
        ) {
            (Ok(interrupt), Ok(terminate), Ok(hangup)) => {
                Self(Some((interrupt, terminate, hangup)))
            }
            _ => Self(None),
        }
    }

    /// Das nächste dieser Signale, als Nummer.
    async fn next(&mut self) -> u8 {
        let Some((interrupt, terminate, hangup)) = self.0.as_mut() else {
            return std::future::pending().await;
        };
        tokio::select! {
            _ = interrupt.recv() => 2,
            _ = terminate.recv() => 15,
            _ = hangup.recv() => 1,
        }
    }
}

/// Zeigt nach einem Lauf von `humanitl go`, was der Agent geändert hat, und
/// fragt, was damit geschehen soll (HUM-496). Übernehmen und Verwerfen
/// laufen über dieselben RPCs wie `sandbox apply` und `sandbox discard`; die
/// Antwort ist die Bestätigung.
///
/// Nur auf einem Terminal; der Aufrufer prüft das. Ein Signal an der Frage
/// lässt alles, wie es ist, und kommt als `Some(Nummer)` zurück.
///
/// # Errors
///
/// Wie [`apply`] und [`discard`].
pub async fn review(ctx: &Context) -> Result<Option<u8>, Failure> {
    use std::io::Write as _;

    // Gehört wird schon während der Frage nach den Änderungen: Hängt sie,
    // beendet `Ctrl+C` das Warten, ohne etwas anzufassen.
    let mut signals = Signals::listen();
    let mut stderr = std::io::stderr();
    let changes = tokio::select! {
        biased;
        number = signals.next() => {
            // Nach `SIGHUP` ist das Terminal weg: kein Wort mehr hinein
            // (GitHub #13). Die Änderungen warten im Daemon.
            if number != crate::cmd::run::SIGHUP {
                ctx.render.note(KEPT);
            }
            return Ok(Some(number));
        }
        fetched = fetch(ctx) => fetched?,
    };
    if changes.changes.is_empty() {
        return Ok(None);
    }
    for line in listing(&changes) {
        let _ = writeln!(stderr, "{line}");
    }
    loop {
        let _ = write!(
            stderr,
            "[a]pply all to {}, [d]iscard all, [v]iew the diffs, or keep them for later [k]? ",
            changes.work_dir
        );
        let _ = stderr.flush();
        let answer = match line_or_signal(read_line(), signals.next()).await {
            Answer::Line(answer) => answer,
            Answer::Signal(number) => {
                if number != crate::cmd::run::SIGHUP {
                    let _ = writeln!(stderr);
                    ctx.render.note(KEPT);
                }
                return Ok(Some(number));
            }
        };
        match choice(&answer) {
            Choice::Apply => {
                send_apply(ctx, &changes.sandbox_id, &[], false, Out::Review).await?;
                return Ok(None);
            }
            Choice::Discard => {
                send_discard(ctx, &changes.sandbox_id, Out::Review).await?;
                return Ok(None);
            }
            Choice::View => {
                // Ging das Terminal mit der Antwort zu, geht nichts mehr
                // hinein, und die Änderungen warten wie nach `SIGHUP`
                // (GitHub #73).
                if crate::tty::terminal_gone() {
                    return Ok(Some(crate::cmd::run::SIGHUP));
                }
                write_diffs(&changes, Out::Review);
                Out::Review.write(format_args!("\n"));
            }
            Choice::Keep => {
                ctx.render.note(KEPT);
                return Ok(None);
            }
        }
    }
}

/// Der Hinweis, wenn die Änderungen warten bleiben.
const KEPT: &str = "The changes keep waiting. Review: humanitl sandbox changes   Apply: \
                    humanitl sandbox apply   Discard: humanitl sandbox discard";

/// Fragt den Daemon nach den wartenden Änderungen.
async fn fetch(ctx: &Context) -> Result<v1::DisposableChanges, Failure> {
    let events = ask(ctx, v1::sandbox_request::Op::Changes(())).await?;
    events
        .into_iter()
        .find_map(|event| match event.event {
            Some(v1::sandbox_event::Event::Changes(changes)) => Some(changes),
            _ => None,
        })
        .ok_or_else(|| no_answer("changes"))
}

/// Schickt eine Operation und sammelt die Antwort bis zum Ende des Stroms.
///
/// Ein Befund der Stufe Fehler oder höher ist das Ergebnis; eine Warnung
/// begleitet es.
async fn ask(ctx: &Context, op: v1::sandbox_request::Op) -> Result<Vec<v1::SandboxEvent>, Failure> {
    let mut client = ctx.connect().await?;
    let info = client
        .get_info(())
        .await
        .map_err(|status| Failure::new(status_diagnostic(&status, "GetInfo")))?
        .into_inner();
    crate::cmd::daemon::check_proto(&info)?;
    if info.proto_minor < DISPOSABLE_MINOR {
        return Err(Failure::new(
            Diagnostic::builder(codes::CLI_004, Severity::Error)
                .why(format!(
                    "the daemon speaks contract 1.{}, and the disposable sandbox needs 1.{DISPOSABLE_MINOR}",
                    info.proto_minor
                ))
                .fix(FixAction::CopyCommand(
                    humanitl_core::diagnostics::RESTART_DAEMON_COMMAND.to_owned(),
                ))
                .build(),
        ));
    }
    let mut stream = client
        .sandbox(v1::SandboxRequest { op: Some(op) })
        .await
        .map_err(|status| Failure::new(status_diagnostic(&status, "Sandbox")))?
        .into_inner();
    let mut events = Vec::new();
    while let Some(event) = stream
        .message()
        .await
        .map_err(|status| Failure::new(status_diagnostic(&status, "Sandbox")))?
    {
        if let Some(v1::sandbox_event::Event::Diagnostic(wire)) = &event.event
            && let Some(diagnostic) = from_proto(wire)
        {
            if matches!(diagnostic.severity, Severity::Info | Severity::Warning)
                && wire.code != codes::SANDBOX_031.as_str()
            {
                ctx.render.diagnostic(&diagnostic);
            } else {
                return Err(Failure::new(diagnostic));
            }
        }
        events.push(event);
    }
    Ok(events)
}

fn outcome_of(events: Vec<v1::SandboxEvent>) -> Result<v1::DisposableOutcome, Failure> {
    events
        .into_iter()
        .find_map(|event| match event.event {
            Some(v1::sandbox_event::Event::ChangesOutcome(outcome)) => Some(outcome),
            _ => None,
        })
        .ok_or_else(|| no_answer("the outcome"))
}

fn no_answer(what: &str) -> Failure {
    Failure::new(
        Diagnostic::builder(codes::CLI_004, Severity::Error)
            .why(format!("the daemon answered without {what}"))
            .fix(FixAction::CopyCommand(
                "humanitl sandbox changes".to_owned(),
            ))
            .build(),
    )
}

/// Fragt auf dem Terminal nach; ohne Terminal ist `--yes` nötig.
fn confirm(ctx: &Context, question: &str, shown: &[String], verb: &str) -> Result<(), Failure> {
    use std::io::Write as _;

    let stdin = std::io::stdin();
    if ctx.render.is_json() || !stdin.is_terminal() {
        return Err(Failure::new(
            Diagnostic::builder(codes::CLI_004, Severity::Error)
                .why(format!(
                    "{verb} changes the project folder and cannot be undone, so it asks first, \
                     and there is no terminal to ask on"
                ))
                .fix(FixAction::CopyCommand(format!(
                    "humanitl sandbox {verb} --yes"
                )))
                .build(),
        ));
    }
    let mut stderr = std::io::stderr();
    for line in shown {
        let _ = writeln!(stderr, "  {line}");
    }
    let _ = write!(stderr, "{question} [y/N] ");
    let _ = stderr.flush();
    let mut answer = String::new();
    let _ = stdin.lock().read_line(&mut answer);
    if matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
        Ok(())
    } else {
        Err(Failure::new(
            Diagnostic::builder(codes::CLI_004, Severity::Info)
                .why(format!(
                    "nothing was done; the answer was not yes, so {verb} did not run"
                ))
                .fix(FixAction::CopyCommand(
                    "humanitl sandbox changes".to_owned(),
                ))
                .build(),
        ))
    }
}

/// Die Liste der Änderungen, eine Zeile je Änderung, mit Kopfzeile.
fn listing(changes: &v1::DisposableChanges) -> Vec<String> {
    let mut lines = vec![format!(
        "Changes of sandbox {} in {} ({}):",
        changes.sandbox_id,
        changes.work_dir,
        changes.changes.len()
    )];
    lines.extend(
        changes
            .changes
            .iter()
            .map(|change| format!("  {}", row(change))),
    );
    if changes.truncated {
        lines.push("  [the list is cut short; apply single ids, or discard]".to_owned());
    }
    lines
}

/// Eine Zeile: Kennung, Art, Pfad und was der Mensch wissen muss.
fn row(change: &v1::DisposableChange) -> String {
    let kind = match v1::FileChangeKind::try_from(change.kind).unwrap_or_default() {
        v1::FileChangeKind::Added => "added",
        v1::FileChangeKind::Modified => "modified",
        v1::FileChangeKind::Removed => "removed",
        v1::FileChangeKind::SymlinkAdded => "symlink",
        v1::FileChangeKind::ModeChanged => "mode",
        v1::FileChangeKind::Unspecified => "changed",
    };
    let mut line = format!("{}  {kind:<8} {}", change.id, change.path);
    if change.entry == "symlink" && !change.symlink_target.is_empty() {
        let _ = write!(line, " -> {}", change.symlink_target);
    }
    if change.binary {
        let short: String = change.sha256.chars().take(16).collect();
        let _ = write!(line, "  [binary, {} bytes, sha256 {short}]", change.size);
    }
    if change.stripped_bits != 0 {
        let _ = write!(line, "  [mode bits {:o} dropped]", change.stripped_bits);
    }
    if !change.conflict.is_empty() {
        let _ = write!(line, "  [conflict: {}]", change.conflict);
    }
    if !change.refused.is_empty() {
        let _ = write!(line, "  [never applied: {}]", change.refused);
    }
    line
}

fn report(ctx: &Context, outcome: &v1::DisposableOutcome, out: Out) {
    if ctx.render.is_json() {
        ctx.render.value(&json!({
            "sandbox_id": outcome.sandbox_id,
            "outcome": outcome.outcome,
            "applied": outcome.applied,
            "discarded": outcome.discarded,
        }));
        return;
    }
    out.write(format_args!(
        "{}: sandbox {}\n",
        outcome.outcome, outcome.sandbox_id
    ));
    for path in &outcome.applied {
        out.write(format_args!("  applied    {path}\n"));
    }
    for path in &outcome.discarded {
        out.write(format_args!("  discarded  {path}\n"));
    }
}

fn changes_json(changes: &v1::DisposableChanges) -> serde_json::Value {
    json!({
        "sandbox_id": changes.sandbox_id,
        "work_dir": changes.work_dir,
        "truncated": changes.truncated,
        "changes": changes.changes.iter().map(|change| json!({
            "id": change.id,
            "path": change.path,
            "mangled": change.mangled,
            "kind": v1::FileChangeKind::try_from(change.kind).unwrap_or_default().as_str_name(),
            "entry": change.entry,
            "size": change.size,
            "mode": change.mode,
            "sha256": change.sha256,
            "symlink_target": change.symlink_target,
            "diff": change.diff,
            "binary": change.binary,
            "conflict": change.conflict,
            "refused": change.refused,
            "stripped_bits": change.stripped_bits,
            "hardlinked": change.hardlinked,
        })).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn change(kind: v1::FileChangeKind) -> v1::DisposableChange {
        v1::DisposableChange {
            id: "0123456789abcdef".to_owned(),
            path: "src/main.rs".to_owned(),
            kind: kind.into(),
            entry: "file".to_owned(),
            size: 12,
            ..v1::DisposableChange::default()
        }
    }

    #[test]
    fn a_row_says_what_a_human_must_know_before_applying() {
        let mut modified = change(v1::FileChangeKind::Modified);
        modified.conflict = "it changed in the project after the start".to_owned();
        modified.stripped_bits = 0o4000;
        let line = row(&modified);
        assert!(
            line.starts_with("0123456789abcdef  modified src/main.rs"),
            "{line}"
        );
        assert!(line.contains("[conflict: it changed"), "{line}");
        assert!(line.contains("[mode bits 4000 dropped]"), "{line}");

        let mut symlink = change(v1::FileChangeKind::SymlinkAdded);
        symlink.entry = "symlink".to_owned();
        symlink.symlink_target = "/etc/passwd".to_owned();
        symlink.refused =
            "it is a symbolic link to /etc/passwd, which leaves the project".to_owned();
        let line = row(&symlink);
        assert!(line.contains("-> /etc/passwd"), "{line}");
        assert!(line.contains("[never applied:"), "{line}");

        let mut binary = change(v1::FileChangeKind::Added);
        binary.binary = true;
        binary.sha256 = "ab".repeat(32);
        assert!(row(&binary).contains("[binary, 12 bytes, sha256 abababababababab]"));
    }

    #[test]
    fn a_cut_list_says_so() {
        let changes = v1::DisposableChanges {
            sandbox_id: "s".to_owned(),
            work_dir: "/p".to_owned(),
            changes: vec![change(v1::FileChangeKind::Added)],
            truncated: true,
        };
        let lines = listing(&changes);
        assert_eq!(lines[0], "Changes of sandbox s in /p (1):");
        assert!(lines.last().is_some_and(|line| line.contains("cut short")));
    }

    /// After `humanitl go` only `a` applies and only `d` discards; anything
    /// else, an empty line included, leaves the project folder as it is
    /// (HUM-496).
    #[test]
    fn only_an_explicit_answer_touches_the_project() {
        assert_eq!(choice("a\n"), Choice::Apply);
        assert_eq!(choice(" Apply "), Choice::Apply);
        assert_eq!(choice("d"), Choice::Discard);
        assert_eq!(choice("v"), Choice::View);
        for kept in ["", "\n", "k", "y", "yes", "n", "ad"] {
            assert_eq!(choice(kept), Choice::Keep, "{kept:?}");
        }
    }

    /// A signal at the question after `humanitl go` is not swallowed by a
    /// blocked read: it ends the question, before and over a line (HUM-496).
    #[allow(clippy::expect_used)]
    #[tokio::test]
    async fn a_signal_at_the_question_is_not_swallowed() {
        let ended = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            line_or_signal(std::future::pending::<String>(), async { 2 }),
        )
        .await
        .expect("the signal ended the question");
        assert_eq!(ended, Answer::Signal(2));
        let both = line_or_signal(async { "a\n".to_owned() }, async { 15 }).await;
        assert_eq!(both, Answer::Signal(15), "a signal means: touch nothing");
        let line = line_or_signal(async { "d\n".to_owned() }, std::future::pending()).await;
        assert_eq!(line, Answer::Line("d\n".to_owned()));
    }
}
