//! Container beenden und Reste aufräumen (HUM-433).
//!
//! Ein Container läuft abgekoppelt vom Client, der ihn startete: Ein
//! `SIGKILL` an `podman run` lässt ihn weiterlaufen (HUM-292 P7), und PID 1
//! überhört ein Signal, für das er keinen Handler hat (P5, P6). Beendet wird
//! deshalb über die Engine beim Namen: `podman kill --signal TERM`, nach der
//! Frist `podman kill --signal KILL` und `podman rm --force --time 0`. Die
//! Haltezeit von `podman stop` (zehn Sekunden) spielt keine Rolle. Stirbt der
//! Daemon, beendet die Lebensleine den Container (HUM-430); was ein Absturz
//! trotzdem liegen lässt, räumt [`sweep`] beim nächsten Start des Daemons
//! weg, erkannt am Label der Instanz.
//!
//! `--rm` und ein eigenes `rm` laufen gegeneinander: „no such container"
//! heißt Erfolg (`--ignore`).
//!
//! Endet der Client ohne angeforderten Stopp, etwa durch ein `SIGKILL`,
//! greift sein `--rm` nicht mehr. Der Daemon wartet dann, bis der Container
//! beendet ist, und entfernt ihn selbst ([`ContainerStop::remove_after_exit`],
//! HUM-452). Läuft zugleich ein Stopp, entfernt nur einer von beiden; das
//! entscheidet das Handle der Sitzung.

use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use humanitl_core::diagnostics::codes::SANDBOX_041;
use humanitl_core::{Diagnostic, FixAction, Severity};
use rustix::process::Signal;

use crate::launcher::ContainerStop;

/// Wie lange ein Befehl zum Beenden höchstens dauern darf.
const STOP_TIMEOUT: Duration = Duration::from_secs(10);

/// Wie lange das Entfernen nach einem ungeplanten Ende auf das Ende des
/// Containers wartet (HUM-452): die Frist, in der der Shim ihn nach dem
/// Schließen der Lebensleine beendet, und etwas Luft. Danach entfernt
/// `rm --force` ihn auch laufend.
const EXIT_WAIT: Duration = crate::KILL_GRACE.saturating_add(Duration::from_secs(5));

// Ein Stopp wartet höchstens `REMOVAL_BOUND` auf das Entfernen nach dem
// Ende; die drei Befehle dafür müssen hineinpassen.
const _: () = assert!(
    EXIT_WAIT.as_secs() + 2 * STOP_TIMEOUT.as_secs() < crate::handle::REMOVAL_BOUND.as_secs()
);

impl ContainerStop {
    /// Ein Schritt des Beendens: `SIGTERM` als Bitte an den Shim, `SIGKILL`
    /// mit anschließendem Entfernen.
    ///
    /// Ein gescheitertes `kill` allein zählt nicht: Ein Container, den es
    /// nicht mehr gibt, ist beendet. Nach `SIGKILL` aber fragt die Engine, ob
    /// er noch da ist; ist er es, oder antwortet sie nicht, kommt
    /// `SANDBOX_041` als Warnung zurück, mit dem Befehl, der ihn von Hand
    /// entfernt, und der Ausgabe von `kill` und `rm`.
    pub(crate) fn send(&self, signal: Signal) -> Option<Diagnostic> {
        let name = if signal == Signal::KILL {
            "KILL"
        } else {
            "TERM"
        };
        let killed = self.run(&["kill", "--signal", name, &self.name]);
        if signal != Signal::KILL {
            return None;
        }
        let removed = self.run(&["rm", "--force", "--time", "0", "--ignore", &self.name]);
        if self.gone() {
            return None;
        }
        Some(
            Diagnostic::builder(SANDBOX_041, Severity::Warning)
                .why(format!(
                    "the container {} may still be there after its session was ended ({}; {})",
                    self.name,
                    said("kill", &killed),
                    said("rm", &removed)
                ))
                .fix(FixAction::CopyCommand(format!(
                    "{} rm --force {}",
                    super::PROGRAM,
                    self.name
                )))
                .build(),
        )
    }

    /// Entfernt den Container, nachdem sein Client ohne angeforderten Stopp
    /// geendet hat (HUM-452): wartet höchstens [`EXIT_WAIT`], bis er beendet
    /// ist, entfernt ihn dann und fragt, ob er weg ist.
    ///
    /// Ist er danach noch da, oder antwortet die Engine nicht, kommt
    /// `SANDBOX_041` als Warnung zurück, mit dem Befehl, der ihn von Hand
    /// entfernt. Einen Container, den `--rm` schon entfernt hat, nimmt die
    /// Engine stumm hin.
    pub(crate) fn remove_after_exit(&self) -> Option<Diagnostic> {
        let waited = self.run_within(&["wait", &self.name], EXIT_WAIT);
        let removed = self.run(&["rm", "--force", "--time", "0", "--ignore", &self.name]);
        if self.gone() {
            return None;
        }
        Some(
            Diagnostic::builder(SANDBOX_041, Severity::Warning)
                .why(format!(
                    "the container {} may still be there after its client ended without a \
                     stop ({}; {})",
                    self.name,
                    said("wait", &waited),
                    said("rm", &removed)
                ))
                .fix(FixAction::CopyCommand(format!(
                    "{} rm --force {}",
                    super::PROGRAM,
                    self.name
                )))
                .build(),
        )
    }

    /// Ob die Engine sagt, dass es den Container nicht gibt. Nur Exit 1 von
    /// `container exists` belegt das; jeder andere Ausgang lässt es offen.
    fn gone(&self) -> bool {
        self.run(&["container", "exists", &self.name])
            .is_ok_and(|output| output.status.code() == Some(1))
    }

    fn run(&self, args: &[&str]) -> std::io::Result<Output> {
        self.run_within(args, STOP_TIMEOUT)
    }

    fn run_within(&self, args: &[&str], timeout: Duration) -> std::io::Result<Output> {
        let mut command = Command::new(&self.program);
        command
            .env_clear()
            .envs(self.env.iter().map(|(key, value)| (key, value)))
            .args(args);
        run_bounded(command, timeout)
    }
}

/// Wie ein Befehl der Engine ausging, für den Text eines Befunds.
fn said(what: &str, result: &std::io::Result<Output>) -> String {
    match result {
        Ok(output) => format!(
            "{what} ended with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ),
        Err(err) => format!("{what} did not run: {err}"),
    }
}

/// Startet `command` mit leerer Eingabe und wartet höchstens `timeout`;
/// danach wird es beendet, und der Fehler heißt `TimedOut`.
///
/// # Errors
///
/// Der Fehler von `spawn`, oder `TimedOut`.
pub(crate) fn run_bounded(mut command: Command, timeout: Duration) -> std::io::Result<Output> {
    // Die Ausgabe geht in anonyme Dateien statt in Pipes: Hält ein Kind der
    // Engine sie offen, nachdem der Befehl endete, wartet hier niemand auf
    // ihr Ende, und es bleibt kein Leser-Faden zurück.
    let stdout = std::fs::File::from(crate::platform::anonymous_file("engine-stdout")?);
    let stderr = std::fs::File::from(crate::platform::anonymous_file("engine-stderr")?);
    let mut child = command
        .stdin(Stdio::null())
        .stdout(stdout.try_clone()?)
        .stderr(stderr.try_clone()?)
        .spawn()?;
    // Die Kopien des Kindes sind mit `spawn` im Kind; die des Elternteils
    // gehen hier zu, damit nur `stdout` und `stderr` selbst offen bleiben.
    drop(command);
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if started.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!("no answer within {timeout:?}"),
            ));
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    Ok(Output {
        status,
        stdout: written(stdout),
        stderr: written(stderr),
    })
}

/// Wie viel Ausgabe eines Befehls der Engine höchstens gelesen wird. Groß
/// genug für jede Liste von `ps --all` und jede Antwort von `image inspect`,
/// die der Adapter liest; die Grenze schützt nur vor einem Kind, das ohne
/// Ende weiterschreibt.
const OUTPUT_CAP: u64 = 16 * 1024 * 1024;

/// Was bis jetzt in die anonyme Datei geschrieben wurde, höchstens
/// [`OUTPUT_CAP`].
fn written(mut file: std::fs::File) -> Vec<u8> {
    use std::io::{Read as _, Seek as _};
    let mut bytes = Vec::new();
    if file.rewind().is_ok() {
        let _ = file.take(OUTPUT_CAP).read_to_end(&mut bytes);
    }
    bytes
}

/// Die Namen aus `podman ps --format {{.Names}}`, eine Zeile je Container;
/// für die Tests, die nach Containern einer Instanz fragen.
#[cfg(test)]
pub(crate) fn names_of(listing: &str) -> Vec<String> {
    listing
        .lines()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Wie lange die Bestandsaufnahme beim Start des Daemons höchstens dauert:
/// Bis sie fertig ist, nimmt der Daemon keinen Start an.
const SNAPSHOT_TIMEOUT: Duration = Duration::from_secs(10);

/// Der Schlüssel des Labels, das die Sitzung eines Containers nennt.
pub(crate) const SESSION_LABEL_KEY: &str = "humanitl.session";

/// Ein Container einer früheren Sitzung, wie ihn die Bestandsaufnahme fand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Leftover {
    /// Die Kennung des Containers; entfernt wird nur sie.
    pub(crate) id: String,
    /// Sein Name, `humanitl-<Sitzung>`.
    pub(crate) name: String,
}

/// Die Schablone für `podman ps --format`: Kennung, Name, Sitzung.
const LISTING_FORMAT: &str = "{{.ID}} {{.Names}} {{index .Labels \"humanitl.session\"}}";

/// Die Container aus einer Liste nach [`LISTING_FORMAT`], die als Sitzung
/// von Humanitl angelegt sind: Der Name ist `humanitl-<Sitzung>`, und die
/// Sitzung steht gleich im Label. Ein fremder Container mit kopiertem Label
/// der Instanz erfüllt das nicht.
pub(crate) fn leftovers_of(listing: &str) -> Vec<Leftover> {
    listing
        .lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            let (id, name, session) = (fields.next()?, fields.next()?, fields.next()?);
            let parsed = uuid::Uuid::parse_str(session).ok()?;
            let ours = fields.next().is_none()
                && name == format!("humanitl-{parsed}")
                && session == parsed.to_string()
                && !id.is_empty()
                && id.chars().all(|c| c.is_ascii_hexdigit());
            ours.then(|| Leftover {
                id: id.to_owned(),
                name: name.to_owned(),
            })
        })
        .collect()
}

/// Die Container früherer Sitzungen dieser Instanz, synchron und begrenzt
/// ([`SNAPSHOT_TIMEOUT`]): beide Labels müssen stehen (Podman verknüpft
/// zwei Label-Filter mit „und").
///
/// # Errors
///
/// `SANDBOX_041` als Warnung, wenn Podman nicht oder nicht rechtzeitig
/// antwortet: Dann bleibt ungewiss, ob Reste liegen.
pub(crate) fn snapshot(
    engine: &ContainerStop,
    daemon_label: &str,
) -> Result<Vec<Leftover>, Diagnostic> {
    let output = engine.run_within(
        &[
            "ps",
            "--all",
            "--filter",
            &format!("label={daemon_label}"),
            "--filter",
            &format!("label={SESSION_LABEL_KEY}"),
            "--format",
            LISTING_FORMAT,
        ],
        SNAPSHOT_TIMEOUT,
    );
    match output {
        Ok(output) if output.status.success() => {
            Ok(leftovers_of(&String::from_utf8_lossy(&output.stdout)))
        }
        Ok(output) => Err(not_listed(&format!(
            "ended with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ))),
        Err(err) => Err(not_listed(&err.to_string())),
    }
}

/// `SANDBOX_041`: Die Bestandsaufnahme ist gescheitert.
fn not_listed(why: &str) -> Diagnostic {
    Diagnostic::builder(SANDBOX_041, Severity::Warning)
        .why(format!(
            "the containers of earlier sessions of this daemon could not be listed ({why}); \
             containers a crash left behind may still be there"
        ))
        .fix(FixAction::CopyCommand(format!(
            "{} ps --all --filter label={SESSION_LABEL_KEY}",
            super::PROGRAM
        )))
        .build()
}

/// Entfernt genau die Container aus `found`, bei ihrer Kennung, und meldet
/// je Container `SANDBOX_041` als Warnung: entfernt, oder nicht entfernbar
/// mit dem Befehl, der es von Hand tut. Ein Container, der erst nach der
/// Bestandsaufnahme entstand, bleibt unberührt.
pub(crate) fn remove(engine: &ContainerStop, found: &[Leftover]) -> Vec<Diagnostic> {
    found
        .iter()
        .map(|leftover| {
            let _ = engine.run(&["rm", "--force", "--time", "0", "--ignore", &leftover.id]);
            // Nur Exit 1 von `container exists` belegt, dass er weg ist; jeder
            // andere Ausgang (Fehler der Engine, 125, Zeitüberschreitung)
            // lässt die Frage offen und zählt als „noch da".
            let exists = engine
                .run(&["container", "exists", &leftover.id])
                .map_or(true, |output| output.status.code() != Some(1));
            let builder = Diagnostic::builder(SANDBOX_041, Severity::Warning);
            let name = &leftover.name;
            if exists {
                builder
                    .why(format!(
                        "the container {name} of an earlier session of this daemon is still \
                         there and could not be removed"
                    ))
                    .fix(FixAction::CopyCommand(format!(
                        "{} rm --force {}",
                        super::PROGRAM,
                        leftover.id
                    )))
                    .build()
            } else {
                builder
                    .why(format!(
                        "the container {name} of an earlier session of this daemon was still \
                         there and has been removed"
                    ))
                    .build()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    /// Ein Kind, das die Ausgabe offenhält, nachdem der Befehl endete, hält
    /// das Beenden nicht auf: Der Status zählt binnen der Frist.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_pipe_held_open_by_a_child_does_not_hold_the_command() {
        let mut command = Command::new("sh");
        command.args(["-c", "sleep 30 & echo said; exit 3"]);
        let started = Instant::now();
        let output = run_bounded(command, Duration::from_secs(2)).expect("the command ran");
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "waited {:?} for a pipe a child held",
            started.elapsed()
        );
        assert_eq!(output.status.code(), Some(3));
        assert_eq!(output.stdout, b"said\n", "the output is kept");
    }

    #[test]
    fn listed_names_skip_blank_lines() {
        assert_eq!(
            names_of("humanitl-a\n\n  humanitl-b \n"),
            ["humanitl-a", "humanitl-b"]
        );
        assert!(names_of("").is_empty());
    }

    /// Nur Container mit Namen `humanitl-<Sitzung>` und derselben Sitzung im
    /// Label zählen; ein fremder mit kopiertem Label bleibt.
    #[test]
    fn only_containers_named_after_their_session_label_are_leftovers() {
        let session = "0b9f3a2e-6c1d-4f00-8a00-000000000001";
        let listing = format!(
            "abc123 humanitl-{session} {session}\n\
             def456 someone-else {session}\n\
             0aa111 humanitl-{session} 0b9f3a2e-6c1d-4f00-8a00-000000000002\n\
             zzz humanitl-{session} {session}\n\
             bbb222 humanitl-x x\n\n"
        );
        assert_eq!(
            leftovers_of(&listing),
            [Leftover {
                id: "abc123".to_owned(),
                name: format!("humanitl-{session}"),
            }]
        );
    }

    /// Eine Attrappe für die Engine: Sie schreibt jeden Aufruf in eine Datei,
    /// listet zwei Container und behält beim Entfernen den zweiten; für ihn
    /// scheitert sogar die Nachfrage mit 125, und das ist kein Beleg dafür,
    /// dass er weg ist (nur Exit 1 ist einer).
    #[cfg(target_os = "linux")]
    #[test]
    fn the_sweep_removes_exactly_the_snapshot_and_warns_once_per_container() {
        let dir = tempfile::tempdir().expect("tempdir");
        let log = dir.path().join("calls");
        let fake = dir.path().join("engine");
        let a = "0b9f3a2e-6c1d-4f00-8a00-00000000000a";
        let b = "0b9f3a2e-6c1d-4f00-8a00-00000000000b";
        crate::test_support::write_program(
            &fake,
            &format!(
                "#!/bin/sh\necho \"$*\" >> {log}\n\
                 if [ \"$1\" = ps ]; then printf 'aaa111 humanitl-{a} {a}\\nbbb222 humanitl-{b} {b}\\n'; fi\n\
                 if [ \"$1\" = container ] && [ \"$3\" = bbb222 ]; then exit 125; fi\n\
                 if [ \"$1\" = container ]; then exit 1; fi\n",
                log = log.display()
            ),
        );
        let engine = ContainerStop {
            program: fake,
            name: String::new(),
            env: Vec::new(),
        };
        let found = snapshot(&engine, "humanitl.daemon=abc").expect("listed");
        assert_eq!(found.len(), 2);
        let calls = std::fs::read_to_string(&log).unwrap();
        assert!(
            calls.contains("--filter label=humanitl.daemon=abc --filter label=humanitl.session"),
            "{calls}"
        );
        let reported = remove(&engine, &found);
        assert_eq!(reported.len(), 2);
        assert!(reported.iter().all(|d| d.code.as_str() == "SANDBOX_041"));
        assert!(reported.iter().all(|d| d.severity == Severity::Warning));
        assert!(
            reported[0].why.contains("has been removed"),
            "{}",
            reported[0].why
        );
        assert!(
            reported[1].why.contains("could not be removed"),
            "{}",
            reported[1].why
        );
        assert!(reported[1].fix.is_some());
        let calls = std::fs::read_to_string(&log).unwrap();
        assert!(
            calls.contains("rm --force --time 0 --ignore aaa111"),
            "{calls}"
        );
        assert!(
            calls.contains("rm --force --time 0 --ignore bbb222"),
            "{calls}"
        );
        let listings = calls.lines().filter(|line| line.starts_with("ps ")).count();
        assert_eq!(listings, 1, "only the snapshot lists: {calls}");
    }

    /// Antwortet Podman nicht, ist das eine Warnung und keine Stille.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_failed_listing_is_a_warning_not_silence() {
        let dir = tempfile::tempdir().expect("tempdir");
        let fake = dir.path().join("engine");
        crate::test_support::write_program(&fake, "#!/bin/sh\necho broken >&2\nexit 125\n");
        let engine = ContainerStop {
            program: fake,
            name: String::new(),
            env: Vec::new(),
        };
        let err = snapshot(&engine, "humanitl.daemon=abc").expect_err("broken");
        assert_eq!(err.code.as_str(), "SANDBOX_041", "{}", err.why);
        assert_eq!(err.severity, Severity::Warning);
        assert!(err.why.contains("broken"), "{}", err.why);
    }
}
