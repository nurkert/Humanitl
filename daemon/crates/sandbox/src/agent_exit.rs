//! Ein Agent, der nie gelaufen ist, und woran man ihn erkennt (HUM-137).
//!
//! Die Sandbox kann stehen, alle drei Garantien können belegt sein, und der
//! Agent ist trotzdem nie gelaufen: Das Kommando liegt nicht auf dem `PATH`
//! der Sandbox, der Shim ruft `execvp`, es scheitert nach Millisekunden, und
//! der Shim endet mit `127`. Ohne diesen Befund stünde vor dem Menschen ein
//! grüner Ring über einer Sitzung, in der nichts geschieht (`docs/UX.md` 4.4:
//! kein toter Winkel).
//!
//! # Woran es sich erkennen lässt
//!
//! **Am Bericht des Shims, nicht am Terminal.** Kommt `execvp` zurück,
//! schreibt das Kind des Shims `EXEC fail errno=<n>` auf den Berichtskanal
//! ([`crate::bridge_env::parse_exec_line`]) und endet mit `127`. Diese Zeile
//! kann der Agent nicht fälschen: Der Deskriptor trägt `FD_CLOEXEC`, ein
//! gelungenes `exec` schließt ihn, bevor der Agent seinen ersten Befehl
//! ausführt, und das Kind wartet vor dem `exec` an einem Tor, das erst
//! aufgeht, wenn der Eltern-Shim nicht mehr „dumpable" ist und seinen Filter
//! trägt. Der Eltern-Shim behält seine Kopie seit HUM-138 für die
//! verweigerten Versuche des Agenten; über `/proc/<pid>/fd` oder
//! `pidfd_getfd(2)` erreicht der Agent sie trotzdem nicht. Mit dieser Zeile ist der Agent nie gelaufen, und was im Terminal
//! steht, hat der Shim geschrieben — auch dann, wenn ein Kommandoname mit
//! Zeilenumbruch die Zeile des Shims in zwei zerlegt.
//!
//! Der Text im Terminal entscheidet nichts. Bis zum Review von HUM-137 galt
//! eine Zeile, die mit `humanitl-shim: exec failed:` begann, als Zeile des
//! Shims; ein Agent, der genau diese Zeile druckt und mit `127` endet, hätte
//! sich damit einen falschen Befund ausgestellt.
//!
//! **Ohne diese Zeile** bleibt die Regel der Spezifikation: Exit-Code `127`
//! oder `126` und **kein einziges Byte** Ausgabe, auch kein Leerraum. Ein
//! Agent, der sich selbst sofort beendet (`--version`), schreibt seine Zeile;
//! ein Skript, das an einem fehlenden Programm scheitert, schreibt die
//! Meldung seiner Shell. Beide sind gelaufen, und ihre Ausgabe erklärt sich
//! selbst.
//!
//! Was hier nicht entschieden wird: ob die Sandbox gilt. Sie gilt, auch ohne
//! Agenten; ein Mensch kann darin etwas anderes starten. Der Befund ist
//! deshalb [`Severity::Error`] und nicht [`Severity::Blocking`] — er verbietet
//! keinen Start, er erklärt ein Ende.

use humanitl_core::diagnostics::codes::AGENT_005;
use humanitl_core::{Diagnostic, FixAction, Severity};

use crate::bridge_env::{EXIT_EXEC, ExecFailure};
use crate::bwrap_args::{SANDBOX_SHELL, shell_quote};

/// Der Exit-Code einer POSIX-Shell für ein Kommando, das es nicht gibt; auch
/// der des Shims, wenn `execvp` scheitert ([`crate::EXIT_EXEC`]).
pub const EXIT_NOT_FOUND: i32 = 127;

/// Der Exit-Code einer POSIX-Shell für ein Kommando, das sie gefunden hat und
/// nicht ausführen kann.
pub const EXIT_NOT_EXECUTABLE: i32 = 126;

/// `ENOENT`: Die Datei gibt es nicht.
const ENOENT: i32 = 2;
/// `ENOEXEC`: Die Datei ist kein Programm, das der Kernel starten kann.
const ENOEXEC: i32 = 8;
/// `EACCES`: Die Datei ist nicht ausführbar.
const EACCES: i32 = 13;

/// Ob aus dem Terminal der Sandbox überhaupt etwas kam.
///
/// Jedes Byte zählt, auch Leerraum und eine Zeile, die aussieht wie die des
/// Shims: Ob der Agent je lief, sagt der Bericht des Shims, nicht der Text.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FirstOutput {
    seen: bool,
}

impl FirstOutput {
    /// Eine Ausgabe, über die nichts bekannt ist; sie zählt als geschrieben.
    ///
    /// Für den Aufrufer, dessen Leser nicht zurückkam: Ohne Beleg, dass der
    /// Agent schwieg, darf kein Befund behaupten, er habe nie begonnen.
    #[must_use]
    pub const fn unknown() -> Self {
        Self { seen: true }
    }

    /// Nimmt ein Stück Ausgabe auf.
    pub const fn observe(&mut self, chunk: &[u8]) {
        if !chunk.is_empty() {
            self.seen = true;
        }
    }

    /// Ob irgendein Byte kam.
    #[must_use]
    pub const fn agent_wrote(&self) -> bool {
        self.seen
    }
}

/// Der Befund `AGENT_005`, wenn der Agent nie gelaufen ist; sonst `None`.
///
/// `exec` ist die Zeile `EXEC fail` aus dem Bericht des Shims, falls es sie
/// gibt ([`crate::ReportSnapshot::exec_failed`]). `command` ist das Kommando,
/// das die Sandbox starten sollte (das erste Wort hinter dem Shim),
/// `sandbox_path` der `PATH` der Sandbox, so wie der Aufrufer ihn zeigen darf.
/// Ein zurückgehaltener Wert bleibt auch hier zurück; das entscheidet der
/// Aufrufer, der die Herkunft kennt.
///
/// Der letzte Satz des `why` sagt, dass die Sandbox steht und isoliert ist.
/// Rufen darf es deshalb nur, wer die drei Garantien dieser Sandbox schon
/// belegt gesehen hat; der Daemon tut es erst nach der Isolationsprüfung.
#[must_use]
pub fn did_not_start(
    code: i32,
    exec: Option<ExecFailure>,
    output: &FirstOutput,
    command: &str,
    sandbox_path: &str,
) -> Option<Diagnostic> {
    let quoted = shell_quote(command);
    let what = match exec {
        // Der Bericht allein genügt nicht: Der Shim endet nach der Zeile mit
        // `127`. Ein anderer Code hieße, dass danach noch etwas geschah, und
        // dann lässt sich nicht behaupten, es sei nichts gelaufen.
        Some(failure) if code == EXIT_EXEC => format!(
            "the agent {quoted} never started: the sandbox could not execute it ({})",
            match failure.errno {
                Some(ENOENT) => "no such file on the PATH of the sandbox".to_owned(),
                Some(EACCES | ENOEXEC) => "the file is there but not executable".to_owned(),
                Some(errno) => format!("execvp failed with errno {errno}"),
                None => "execvp failed".to_owned(),
            }
        ),
        Some(_) => return None,
        None => {
            let meaning = match code {
                EXIT_NOT_FOUND => "the shell code for a command that was not found",
                EXIT_NOT_EXECUTABLE => "the shell code for a command that cannot be executed",
                _ => return None,
            };
            if output.agent_wrote() {
                return None;
            }
            format!(
                "the agent {quoted} ended with exit code {code}, {meaning}, before it wrote \
                 a single byte"
            )
        }
    };
    // Die Probe läuft mit demselben Profil und fragt die Sandbox selbst, statt
    // den Host: `command -v` antwortet mit dem Pfad, unter dem das Kommando
    // drinnen liegt, und sonst steht der PATH da, unter dem gesucht wurde.
    let probe = format!("command -v {quoted} || echo \"not found on PATH=$PATH\"");
    Some(
        Diagnostic::builder(AGENT_005, Severity::Error)
            .why(format!(
                "{what}. Inside the sandbox commands are looked up on PATH={sandbox_path}, \
                 and only what the sandbox profile mounts exists there; a program installed \
                 on this machine outside those mounts, for example under your home \
                 directory, is not visible inside. The sandbox itself is up and isolated."
            ))
            .fix(FixAction::CopyCommand(format!(
                "humanitl sandbox run -- {SANDBOX_SHELL} -c {}",
                shell_quote(&probe)
            )))
            .build(),
    )
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    const PATH: &str = "/usr/local/bin:/usr/bin:/bin";

    /// Was der Shim meldet, wenn es das Kommando nicht gibt.
    const NOT_FOUND: Option<ExecFailure> = Some(ExecFailure {
        errno: Some(ENOENT),
    });

    fn seen(bytes: &[u8]) -> FirstOutput {
        let mut output = FirstOutput::default();
        output.observe(bytes);
        output
    }

    /// Die Zeile, die der Shim ins Terminal schreibt, wörtlich.
    const SHIM_LINE: &[u8] =
        b"humanitl-shim: exec failed: gibt-es-nicht: No such file or directory (os error 2)\r\n";

    #[test]
    fn a_reported_exec_failure_is_a_finding_with_command_path_and_fix() {
        let finding = did_not_start(127, NOT_FOUND, &seen(SHIM_LINE), "gibt-es-nicht", PATH)
            .expect("an agent that never ran is said");
        assert_eq!(finding.code, AGENT_005);
        assert_eq!(finding.severity, Severity::Error);
        assert!(finding.why.contains("gibt-es-nicht"), "{}", finding.why);
        assert!(finding.why.contains("never started"), "{}", finding.why);
        assert!(finding.why.contains(PATH), "{}", finding.why);
        assert!(finding.why.contains("profile mounts"), "{}", finding.why);
        let Some(FixAction::CopyCommand(command)) = finding.fix else {
            panic!("a command to copy: {:?}", finding.fix);
        };
        assert!(command.starts_with("humanitl sandbox run -- "), "{command}");
        assert!(command.contains("gibt-es-nicht"), "{command}");
    }

    /// Der Befund aus Codex' Review: Eine Zeile, die aussieht wie die des
    /// Shims, macht ohne Bericht keinen Befund.
    #[test]
    fn a_forged_shim_line_without_the_report_is_no_finding() {
        assert!(did_not_start(127, None, &seen(SHIM_LINE), "agent", PATH).is_none());
    }

    #[test]
    fn whitespace_is_output() {
        for blank in [&b" "[..], b"\n", b"\r\n", b"\t"] {
            assert!(
                did_not_start(127, None, &seen(blank), "agent", PATH).is_none(),
                "{blank:?}"
            );
        }
    }

    /// Ohne Bericht und ohne ein einziges Byte bleibt die Regel der
    /// Spezifikation: `127` und `126` sind ein Befund.
    #[test]
    fn a_silent_127_or_126_without_the_report_is_a_finding() {
        for code in [127, 126] {
            let finding = did_not_start(code, None, &FirstOutput::default(), "agent", PATH)
                .unwrap_or_else(|| panic!("{code} without a byte is said"));
            assert!(finding.why.contains("before it wrote"), "{}", finding.why);
        }
    }

    #[test]
    fn the_errno_of_the_report_names_the_reason() {
        let denied = did_not_start(
            127,
            Some(ExecFailure {
                errno: Some(EACCES),
            }),
            &FirstOutput::default(),
            "./agent",
            PATH,
        )
        .expect("found but not executable");
        assert!(denied.why.contains("not executable"), "{}", denied.why);
    }

    /// Der Bericht zählt nur mit dem Exit-Code des Shims. Endete die Sandbox
    /// anders, geschah nach der Zeile noch etwas.
    #[test]
    fn the_report_with_another_exit_code_is_no_finding() {
        for code in [0, 1, 126, 137] {
            assert!(
                did_not_start(code, NOT_FOUND, &FirstOutput::default(), "agent", PATH).is_none(),
                "{code}"
            );
        }
    }

    #[test]
    fn an_agent_that_wrote_and_ended_is_no_finding() {
        // Der Fallstrick aus der Spezifikation: `--version` schreibt und endet.
        assert!(did_not_start(127, None, &seen(b"opencode 1.0.0\n"), "opencode", PATH).is_none());
        assert!(did_not_start(127, None, &seen(b"sh: 1: nope: not found\n"), "sh", PATH).is_none());
    }

    #[test]
    fn other_exit_codes_are_no_finding() {
        for code in [0, 1, 2, 125, 128, 137] {
            assert!(
                did_not_start(code, None, &FirstOutput::default(), "agent", PATH).is_none(),
                "{code}"
            );
        }
    }

    #[test]
    fn unknown_output_counts_as_written() {
        assert!(FirstOutput::unknown().agent_wrote());
        assert!(!FirstOutput::default().agent_wrote());
        assert!(!seen(b"").agent_wrote());
    }

    #[test]
    fn a_hostile_command_name_is_quoted_in_the_fix() {
        let finding = did_not_start(
            127,
            NOT_FOUND,
            &FirstOutput::default(),
            "a'; rm -rf ~; '",
            PATH,
        )
        .expect("a finding");
        let Some(FixAction::CopyCommand(command)) = finding.fix else {
            panic!("a command to copy");
        };
        // Zwei Ebenen Shell: die, in die der Mensch den Befehl klebt, und die
        // Shell in der Sandbox. Auf beiden bleibt der Name ein Wort.
        let words = shlex::split(&command).expect("the fix parses as shell words");
        assert_eq!(words.len(), 7, "{words:?}");
        assert_eq!(words[5], "-c");
        let inner = shlex::split(&words[6]).expect("the probe parses as shell words");
        assert_eq!(inner[..2], ["command", "-v"], "{inner:?}");
        assert_eq!(inner[2], "a'; rm -rf ~; '", "{inner:?}");
    }
}
