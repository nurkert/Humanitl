//! `humanitl go` in einem Ordner, dessen Sitzung schon läuft (GitHub #13).
//!
//! Das Schließen des Terminals von `go` lässt die Sitzung laufen
//! (`terminal.on_close = "detach"`). Ein zweites `go` im selben Ordner
//! startet deshalb nichts und bekommt kein `CLI_005`, sondern hängt sich an
//! die laufende Sitzung an: ohne Ladeanimation, mit einer Zeile davor, sonst
//! wie das erste `go` (Präfixtaste, Ende). Ob die Sitzung in diesem
//! Ordner läuft, sagt der Daemon (`Projects.get`, `ProjectEntry.running`);
//! der Befehl vergleicht keine Pfade selbst (ADR-018).
//!
//! **An genau diese Sitzung.** Der Anschluss nennt die Sandbox, die der
//! Daemon für den Ordner genannt hat (`ProjectEntry.sandbox_id`). Endet sie
//! zwischen Frage und Anschluss und startet eine andere, lehnt der Daemon mit
//! `IPC_006` ab, statt dieses Terminal an die fremde Sitzung zu hängen. Erst
//! ab Vertrag 1.62 und nur mit einer genannten Kennung ([`rejoinable`]);
//! davor startet `go` wie vor #13, und der Daemon antwortet `CLI_005`.
//!
//! **Ein zweites `go` stoppt nie** ([`REJOINED_OWNS`]): Es hat die Sitzung
//! nicht gestartet. Sie gehört der Oberfläche, die sie nur mit der
//! bestätigten Handlung beendet, oder einem früheren `go`, dessen Terminal
//! sie bewusst laufen ließ. `SIGHUP`, `SIGTERM` und `SIGINT` hängen nur ab,
//! gleich was `terminal.on_close` sagt; der Schlüssel gilt für eine Sitzung,
//! die dieses `go` gestartet hat (Entscheidung aus dem Review).
//!
//! **Die Signale stehen ab der ersten Anfrage** ([`Signals`]): Ein `SIGHUP`
//! während `Projects.get` oder während der Anschluss aufgebaut wird, nähme
//! der Kernel sonst mit seinem Standardverhalten, und danach schrieb niemand
//! mehr kontrolliert. Startet `go` statt dessen, reicht es die Signale an den
//! Lauf weiter (`Launch::early_signals`), der seine Sitzung dann stoppt, wie
//! `terminal.on_close` es sagt.
//!
//! Warten Änderungen einer beendeten Wegwerf-Sitzung dieses Ordners, fragt
//! `go` vor dem Start nach ihnen ([`crate::cmd::changes::review`]): Ohne
//! Entscheidung lehnte der Daemon den Start mit `SANDBOX_030` ab.
//!
//! Ohne Terminal, mit `--json` oder an einem Daemon vor `Projects` (Minor
//! 45) bleibt es beim Start; den Befund über die laufende Sitzung schickt
//! dann der Daemon (`CLI_005`), und eine Zeile davor sagt, warum dieser
//! Aufruf sich nicht anhängt.

use std::path::Path;

use humanitl_core::block::sanitize_note;
use humanitl_core::diagnostics::codes;
use humanitl_ipc::client::Client;
use humanitl_ipc::v1;

use super::PROJECTS_MINOR;
use crate::cmd::attach::{Ending, Signals};
use crate::cmd::run::Launch;
use crate::cmd::{Context, Failure};

/// Ein zweites `go` besitzt die Sitzung nicht, an die es sich hängt: Kein
/// Signal stoppt sie ([`crate::cmd::run::OnSignal::stops`]).
const REJOINED_OWNS: bool = false;

/// Ab dieser Minor nennt der Daemon die Sandbox eines Projekts
/// (`ProjectEntry.sandbox_id`, GitHub #13).
const PINNED_MINOR: u32 = 62;

/// Die Sandbox, an die sich ein zweites `go` hängen darf: die, die der
/// Daemon für den Ordner genannt hat, und nur an einem Daemon, der sie nennt
/// (ab 1.62). Ohne sie hinge der Anschluss an der Sitzung, die gerade läuft,
/// welche es auch sei; dann startet `go` wie vor #13.
fn rejoinable(entry: &v1::ProjectEntry, minor: u32) -> Option<&str> {
    (minor >= PINNED_MINOR && entry.running && !entry.sandbox_id.is_empty())
        .then_some(entry.sandbox_id.as_str())
}

/// Was [`rejoin`] über diesen Aufruf wissen muss.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Here<'a> {
    /// Der Ordner dieses Aufrufs.
    pub work_dir: &'a Path,
    /// Die Vertrags-Minor des Daemons.
    pub minor: u32,
    /// Ob dieser Aufruf sein Terminal an eine Sitzung hängen kann
    /// (`cmd::run::attaches`).
    pub attaching: bool,
    /// Ob er nach wartenden Änderungen fragen kann: Tasten von `stdin`, die
    /// Frage auf `stderr`, kein `--json`.
    pub asking: bool,
    /// Ob er etwas für den Start wünscht, das eine laufende Sitzung nicht
    /// mehr ändert: den Agenten, Wörter nach `--`, einen Ordnerschalter oder
    /// `--kernel`.
    pub wishes: bool,
}

/// Hängt sich an die Sitzung dieses Ordners, wenn sie läuft, und fragt
/// sonst nach wartenden Änderungen. `Some(code)`: Der Befehl ist fertig,
/// nichts startet. `None`: weiter zum Start.
///
/// # Errors
///
/// Die Befunde des Anschlusses ([`crate::cmd::attach::attach_pinned`],
/// `IPC_006` für eine Sitzung, die inzwischen eine andere ist) und der
/// Präfixtaste.
pub(crate) async fn rejoin(
    ctx: &Context,
    client: &mut Client,
    launch: &mut Launch,
    here: Here<'_>,
) -> Result<Option<u8>, Failure> {
    let mut signals = Signals::listen();
    let entry = project_here(client, here.work_dir, here.minor).await;
    // Ein Signal während der Frage: Nichts ist angehängt, nichts gestartet.
    if let Some(number) = signals.pending() {
        launch.signalled = Some(number);
        return Ok(Some(128_u8.saturating_add(number)));
    }
    let Some(entry) = entry else {
        launch.early_signals = Some(signals);
        return Ok(None);
    };
    if let Some(pinned) = rejoinable(&entry, here.minor) {
        if here.attaching {
            let pinned = pinned.to_owned();
            return Box::pin(attach_running(
                ctx, client, launch, &entry, here, signals, &pinned,
            ))
            .await
            .map(Some);
        }
        ctx.render.note(
            "[humanitl] a session runs in this folder; humanitl go attaches to it only at a \
             terminal and without --json. From anywhere: humanitl sandbox attach",
        );
    } else if !entry.running && entry.changes_waiting && here.asking {
        match crate::cmd::changes::review(ctx).await {
            Ok(None) => {}
            // Ein Signal an der Frage: nichts angefasst, nichts gestartet.
            Ok(Some(number)) => {
                launch.signalled = Some(number);
                return Ok(Some(128_u8.saturating_add(number)));
            }
            // Eine Übernahme, die der Daemon ablehnt, steht als Befund da;
            // der Start sagt danach selbst, ob er geht.
            Err(found) => ctx.render.diagnostic(&found.diagnostic),
        }
    }
    launch.early_signals = Some(signals);
    Ok(None)
}

/// Der Eintrag des Registers für diesen Ordner, wie der Daemon ihn sieht;
/// `None` an einem Daemon ohne `Projects`, ohne Register oder für einen
/// Ordner, den es nicht kennt.
async fn project_here(
    client: &mut Client,
    work_dir: &Path,
    minor: u32,
) -> Option<v1::ProjectEntry> {
    if minor < PROJECTS_MINOR {
        return None;
    }
    client
        .projects(v1::ProjectsRequest {
            op: Some(v1::projects_request::Op::Get(v1::projects_request::Get {
                project_id: String::new(),
                path: work_dir.display().to_string(),
            })),
        })
        .await
        .ok()?
        .into_inner()
        .project
}

/// Hängt dieses Terminal an die Sitzung `pinned`: Abhängen und jedes Signal
/// lassen sie laufen ([`REJOINED_OWNS`]), ihr Ende bringt den Exit-Code des
/// Agenten und danach die Frage nach wartenden Änderungen.
async fn attach_running(
    ctx: &Context,
    client: &mut Client,
    launch: &mut Launch,
    entry: &v1::ProjectEntry,
    here: Here<'_>,
    mut signals: Signals,
    pinned: &str,
) -> Result<u8, Failure> {
    let keys = crate::cmd::attach::resolve_keys(ctx, None)?;
    let ending = if let Some(number) = signals.pending() {
        Ending::Signal(number)
    } else {
        let name = if entry.display_name.is_empty() {
            &entry.path
        } else {
            &entry.display_name
        };
        ctx.render.note(&format!(
            "Attached to the running session in {}",
            sanitize_note(name)
        ));
        if here.wishes {
            ctx.render.note(
                "[humanitl] the agent, the words after --, --disposable, --rw, --ro and \
                 --kernel of this call do not apply to a session that already runs",
            );
        }
        let mut ending =
            crate::cmd::attach::attach_pinned(ctx, false, keys, pinned, &mut signals).await;
        // Schon jemand schreibt (die Oberfläche, ein anderes Terminal): Dieser
        // Aufruf sieht zu, wie `sandbox attach --read-only`.
        if let Ok(Ending::Stream(Err(found))) | Err(found) = &ending
            && found.diagnostic.code == codes::TERM_001
        {
            ctx.render
                .note(crate::render::diagnostic_block(&found.diagnostic).trim_end());
            ctx.render.note(
                "[humanitl] another client writes to this session; this terminal only watches",
            );
            ending = crate::cmd::attach::attach_pinned(ctx, true, keys, pinned, &mut signals).await;
        }
        ending?.or_hung_up()
    };
    match ending {
        Ending::Detached => crate::cmd::attach::finish(ctx, Ending::Detached),
        Ending::Signal(number) => {
            let mut stopper = client.clone();
            Ok(
                crate::cmd::run::on_signal(ctx, &mut stopper, None, launch, REJOINED_OWNS, number)
                    .await,
            )
        }
        Ending::Stream(outcome) => {
            // Die Sitzung ist zu Ende. Ob Änderungen der Wegwerf-Schicht
            // warten, sagt der Daemon; `go` fragt danach wie nach einem
            // eigenen Lauf.
            if let Some(after) = project_here(client, here.work_dir, here.minor).await {
                launch.changes_waiting = after.changes_waiting;
            }
            outcome
        }
    }
}

#[cfg(test)]
mod tests {
    use humanitl_ipc::v1;

    use super::rejoinable;

    /// Angehängt wird nur an eine genannte Sandbox eines Daemons ab 1.62;
    /// davor und ohne Kennung startet `go` wie vor #13 (Review).
    ///
    /// Mutation proof: dropping the minor gate fails the 1.61 row; dropping
    /// the empty-id check fails the row without an id.
    #[test]
    fn only_a_named_sandbox_of_a_new_daemon_is_rejoined() {
        let entry = |running: bool, id: &str| v1::ProjectEntry {
            running,
            sandbox_id: id.to_owned(),
            ..v1::ProjectEntry::default()
        };
        assert_eq!(rejoinable(&entry(true, "sbx"), 62), Some("sbx"));
        assert_eq!(rejoinable(&entry(true, "sbx"), 61), None);
        assert_eq!(rejoinable(&entry(true, ""), 62), None);
        assert_eq!(rejoinable(&entry(false, "sbx"), 62), None);
    }
}
