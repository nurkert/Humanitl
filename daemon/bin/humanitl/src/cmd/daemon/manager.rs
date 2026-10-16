//! Was der systemd-Verwalter des Nutzers sieht, nicht was die Kommandozeile
//! über ihn annimmt (HUM-217).
//!
//! Das Unit-Verzeichnis ([`unit::unit_dir`]) kommt aus dem `XDG_CONFIG_HOME`
//! dieser Kommandozeile. `systemctl --user` dagegen redet über D-Bus mit dem
//! Verwalter, und der sucht Units und legt Verweise der Aktivierung nach
//! seiner eigenen Umgebung an. Steht `XDG_CONFIG_HOME` nur in der Shell-rc,
//! fallen beide auseinander:
//!
//! - Die Unit liegt dort, wo systemd nicht sucht. Nach `daemon-reload` fragt
//!   [`check_loaded_from`] deshalb, aus welcher Datei systemd die Unit lädt
//!   (`FragmentPath`), und bricht ab, wenn es nicht die geschriebene ist. Sagt
//!   systemd dazu nichts, oder gibt es kein `daemon-reload` (`--no-start`),
//!   prüft [`check_searched`], ob das Verzeichnis unter den Suchpfaden des
//!   Verwalters steht (`UnitPath`). Kommt auch darauf keine Antwort, gilt die
//!   Unit nicht als geprüft und geht zurück ([`Elsewhere::Silent`]). Nur wenn
//!   `systemctl` den Bus der Sitzung gar nicht erreicht, bleibt es unter
//!   `--no-start` bei der Unit: Dieser Weg braucht keine Sitzung.
//! - `enable` legt seine Verweise unter dem Verzeichnis des Verwalters an,
//!   nicht unter dem der Kommandozeile. Eine Rücknahme, die nur dort nach
//!   Verweisen sucht, ließe sie liegen, und der Dienst startete bei jedem
//!   Anmelden trotz `DAEMON_008`. [`ManagerEnablement`] fragt deshalb vor dem
//!   `enable` systemd selbst (`is-enabled`) und nimmt nach einem Fehlschlag mit
//!   `disable` zurück, was vorher nicht aktiviert war. Die Suche nach Dateien
//!   bleibt als Rückfall ([`unit::Enablement`]). Sagt `is-enabled` zu einer
//!   Unit nichts, wird sie gar nicht erst aktiviert ([`not_asked`]): Eine
//!   Rücknahme, die nur noch die Dateien hätte, übersähe genau die Verweise
//!   unter dem Verzeichnis des Verwalters.

use std::path::{Path, PathBuf};

use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, Severity};

use super::{no_bus, systemctl_capture, systemctl_stdout, unit_fix};
use crate::cmd::unit;

/// Was systemd vor dem `enable` über die Aktivierung jeder Unit sagte.
///
/// `Some(true)`: dauerhaft aktiviert oder verlinkt, bleibt nach einem
/// Fehlschlag, wie es war. `Some(false)`: nicht dauerhaft aktiviert, wird nach
/// einem Fehlschlag wieder abgemeldet. `None`: systemd hat nichts gesagt; dann
/// bleibt es bei der Suche nach Dateien.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct ManagerEnablement {
    /// Je Unit, was `is-enabled` sagte.
    before: Vec<(String, Option<bool>)>,
}

impl ManagerEnablement {
    /// Fragt systemd nach dem Stand jeder Unit, bevor `enable` etwas anlegt.
    pub(super) async fn read(ctx: &crate::cmd::Context, systemctl: &Path, names: &[&str]) -> Self {
        let mut before = Vec::with_capacity(names.len());
        for name in names {
            let output = systemctl_stdout(ctx, systemctl, &["--user", "is-enabled", name]).await;
            before.push(((*name).to_owned(), enabled_state(&output)));
        }
        Self { before }
    }

    /// Was systemd über `name` sagte; `None`, wenn nichts oder nicht gefragt.
    pub(super) fn state_of(&self, name: &str) -> Option<bool> {
        self.before
            .iter()
            .find(|(unit, _)| unit == name)
            .and_then(|(_, state)| *state)
    }

    /// Die Units, zu denen `is-enabled` nichts gesagt hat.
    pub(super) fn unanswered(&self) -> Vec<&str> {
        self.before
            .iter()
            .filter(|(_, state)| state.is_none())
            .map(|(name, _)| name.as_str())
            .collect()
    }

    /// Die Units, die vorher nachweislich nicht aktiviert waren.
    pub(super) fn not_enabled_before(&self) -> Vec<&str> {
        self.before
            .iter()
            .filter(|(_, state)| *state == Some(false))
            .map(|(name, _)| name.as_str())
            .collect()
    }

    /// Meldet nach einem Fehlschlag ab, was vorher nicht aktiviert war, über
    /// systemd und damit dort, wo der Verwalter seine Verweise angelegt hat.
    ///
    /// # Errors
    ///
    /// Der Satz, warum `disable` nicht durchging. Der Aufrufer nimmt danach
    /// trotzdem die Verweise zurück, die er selbst findet.
    pub(super) async fn undo(
        &self,
        ctx: &crate::cmd::Context,
        systemctl: &Path,
    ) -> Result<(), String> {
        let names = self.not_enabled_before();
        if names.is_empty() {
            return Ok(());
        }
        let mut call = vec!["--user", "disable"];
        call.extend_from_slice(&names);
        systemctl_capture(ctx, systemctl, &call)
            .await
            .map(drop)
            .map_err(|why| format!("systemctl {} did not go through ({why})", call.join(" ")))
    }
}

/// Liest die erste Zeile von `systemctl is-enabled`.
///
/// `enabled`, `linked` und `alias` heißen: Es gibt schon eine dauerhafte
/// Aktivierung oder einen Verweis, die dieser Lauf nicht angelegt hat.
///
/// `enabled-runtime` und `linked-runtime` zählen als nicht aktiviert: Diese
/// Verweise liegen unter `/run` und sind nach dem nächsten Neustart fort,
/// `enable` legt dagegen dauerhafte an. Bliebe ein solcher dauerhafter Verweis
/// nach einem Fehlschlag liegen, startete der Dienst bei jedem Anmelden; ein
/// `disable`, das dabei auch die vorübergehende Aktivierung nimmt, kostet nur,
/// was bis zum Neustart ohnehin ginge.
///
/// Jeder andere Zustand (`disabled`, `static`, `masked`, `not-found`, …)
/// heißt: nicht aktiviert. Auf eine maskierte Unit legt `enable` keinen
/// Verweis an, das `disable` danach ändert nichts. Keine Antwort heißt:
/// unbekannt.
fn enabled_state(output: &str) -> Option<bool> {
    let state = output
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())?;
    Some(matches!(state, "enabled" | "linked" | "alias"))
}

/// Was `systemctl --user show -p FragmentPath` über die Datei der Unit sagt.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Fragment {
    /// Die Antwort trug keine Zeile `FragmentPath=`.
    Unanswered,
    /// systemd kennt keine Datei für die Unit.
    Missing,
    /// Die Datei, aus der systemd die Unit lädt.
    File(PathBuf),
}

/// Liest die Antwort von `systemctl --user show -p FragmentPath`.
fn fragment_of(output: &str) -> Fragment {
    let Some(value) = output
        .lines()
        .find_map(|line| line.trim().strip_prefix("FragmentPath="))
    else {
        return Fragment::Unanswered;
    };
    match value.trim() {
        "" => Fragment::Missing,
        value => Fragment::File(PathBuf::from(value)),
    }
}

/// Die Suchpfade des Verwalters aus `systemctl --user show -p UnitPath`;
/// `None`, wenn die Antwort keine Zeile `UnitPath=` trug.
fn unit_path_of(output: &str) -> Option<Vec<PathBuf>> {
    let value = output
        .lines()
        .find_map(|line| line.trim().strip_prefix("UnitPath="))?;
    Some(value.split_whitespace().map(PathBuf::from).collect())
}

/// Wo systemd die Unit stattdessen sieht, wenn nicht unter dem Verzeichnis
/// dieser Kommandozeile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Elsewhere {
    /// systemd lädt sie aus dieser Datei.
    Loads(PathBuf),
    /// systemd kennt keine Datei für sie.
    Nothing,
    /// systemd sucht nur in diesen Verzeichnissen, das der Kommandozeile ist
    /// nicht darunter.
    Searches(Vec<PathBuf>),
    /// systemd hat weder `FragmentPath` noch `UnitPath` beantwortet; wo es
    /// sucht, ist offen.
    Silent,
}

/// Prüft nach `daemon-reload`, dass systemd die Unit aus `path` lädt.
///
/// Sagt systemd zu `FragmentPath` nichts, entscheidet [`check_searched`].
///
/// # Errors
///
/// Wo systemd die Unit stattdessen sieht ([`Elsewhere`]).
pub(super) async fn check_loaded_from(
    ctx: &crate::cmd::Context,
    systemctl: &Path,
    path: &Path,
) -> Result<(), Elsewhere> {
    let call = ["--user", "show", "-p", "FragmentPath", unit::UNIT_NAME];
    let fragment = systemctl_capture(ctx, systemctl, &call)
        .await
        .map_or(Fragment::Unanswered, |output| fragment_of(&output));
    match fragment {
        Fragment::File(fragment) if same_file(&fragment, path) => Ok(()),
        Fragment::File(fragment) => Err(Elsewhere::Loads(fragment)),
        Fragment::Missing => Err(Elsewhere::Nothing),
        Fragment::Unanswered => check_searched(ctx, systemctl, path).await,
    }
}

/// Prüft, ohne dass systemd neu geladen haben muss, dass das Verzeichnis von
/// `path` unter den Suchpfaden des Verwalters steht (`UnitPath`).
///
/// Für `--no-start`, das systemd nichts von der Unit erzählt, und als Rückfall
/// für [`check_loaded_from`]. Antwortet systemd nicht oder ohne Zeile
/// `UnitPath=`, gilt die Unit nicht als geprüft. Nur wenn `systemctl` den Bus
/// der Sitzung nicht erreicht, ist das kein Befund: Unter `--no-start` ist
/// eine Einrichtung ohne Sitzung gewollt, und auf dem Weg mit Aktivierung ist
/// schon `daemon-reload` daran gescheitert.
///
/// # Errors
///
/// [`Elsewhere::Searches`] mit den Verzeichnissen, in denen systemd sucht,
/// oder [`Elsewhere::Silent`], wenn es das nicht sagt.
pub(super) async fn check_searched(
    ctx: &crate::cmd::Context,
    systemctl: &Path,
    path: &Path,
) -> Result<(), Elsewhere> {
    let Some(dir) = path.parent() else {
        return Ok(());
    };
    let call = ["--user", "show", "-p", "UnitPath"];
    let searched = match systemctl_capture(ctx, systemctl, &call).await {
        Err(why) if no_bus(&why) => return Ok(()),
        Err(_) => return Err(Elsewhere::Silent),
        Ok(output) => unit_path_of(&output).ok_or(Elsewhere::Silent)?,
    };
    if searched.iter().any(|candidate| same_file(candidate, dir)) {
        Ok(())
    } else {
        Err(Elsewhere::Searches(searched))
    }
}

/// Derselbe Pfad, wörtlich oder nach Auflösen beider: systemd nennt den
/// aufgelösten, die Kommandozeile kann einen Verweis im Pfad haben.
fn same_file(left: &Path, right: &Path) -> bool {
    left == right
        || matches!(
            (std::fs::canonicalize(left), std::fs::canonicalize(right)),
            (Ok(a), Ok(b)) if a == b
        )
}

/// `DAEMON_008`: systemd lädt die Unit nicht aus dem Verzeichnis, in das diese
/// Kommandozeile sie geschrieben hat; der Satz nennt beide Verzeichnisse.
pub(super) fn not_loaded(path: &Path, elsewhere: &Elsewhere, undone: &str) -> Diagnostic {
    let ours = path.parent().unwrap_or(path).display();
    let theirs = match elsewhere {
        Elsewhere::Loads(fragment) => format!(
            "loads {} from {}",
            unit::UNIT_NAME,
            fragment.parent().unwrap_or(fragment).display()
        ),
        Elsewhere::Nothing => format!("finds no {} at all", unit::UNIT_NAME),
        Elsewhere::Searches(dirs) => format!(
            "searches only {}",
            dirs.iter()
                .map(|dir| dir.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Elsewhere::Silent => "did not say where it looks (neither FragmentPath nor UnitPath \
                              answered)"
            .to_owned(),
    };
    Diagnostic::builder(codes::DAEMON_008, Severity::Blocking)
        .why(format!(
            "humanitl wrote the unit to {ours}, but the systemd user manager {theirs}: it \
             searches other directories than this humanitl, which takes {ours} from its \
             XDG_CONFIG_HOME. Run humanitl with the XDG_CONFIG_HOME of the user manager \
             (systemctl --user show-environment); {undone}"
        ))
        .fix(unit_fix(&["systemctl", "--user", "show", "-p", "UnitPath"]))
        .build()
}

/// `DAEMON_008`: systemd hat zu `is-enabled` einer Unit nichts gesagt; dann
/// wird sie nicht aktiviert, denn eine Rücknahme nach einem gescheiterten
/// Start fände die Verweise unter dem Verzeichnis des Verwalters nicht.
pub(super) fn not_asked(names: &[&str], undone: &str) -> Diagnostic {
    let mut call = vec!["systemctl", "--user", "is-enabled"];
    call.extend_from_slice(names);
    Diagnostic::builder(codes::DAEMON_008, Severity::Blocking)
        .why(format!(
            "systemctl --user is-enabled gave no answer for {}, so a failed start could leave \
             enablement links behind that this run could not take back through systemd; nothing \
             was enabled, {undone}",
            names.join(", ")
        ))
        .fix(unit_fix(&call))
        .build()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::path::{Path, PathBuf};

    use super::{
        Elsewhere, Fragment, ManagerEnablement, enabled_state, fragment_of, not_asked, not_loaded,
        same_file, unit_path_of,
    };

    /// Der Befund ohne Antwort von `is-enabled` nennt die Units und schlägt
    /// genau die Frage vor, die unbeantwortet blieb.
    #[test]
    fn the_finding_without_is_enabled_names_the_units() {
        let finding = not_asked(&["humanitld.socket"], "nothing had been written");
        assert_eq!(finding.code.as_str(), "DAEMON_008");
        assert!(finding.why.contains("humanitld.socket"), "{}", finding.why);
        assert_eq!(
            finding.fix,
            Some(humanitl_core::FixAction::CopyCommand(
                "systemctl --user is-enabled humanitld.socket".to_owned()
            ))
        );
    }

    /// Nur dauerhafte Aktivierungen zählen als „schon da"; die unter `/run`
    /// werden nach einem Fehlschlag mit abgemeldet (HUM-217, Review).
    #[test]
    fn is_enabled_states_are_read_as_systemd_means_them() {
        for state in ["enabled\n", "linked", "alias\n"] {
            assert_eq!(enabled_state(state), Some(true), "{state}");
        }
        for state in [
            "disabled\n",
            "static",
            "not-found",
            "masked",
            "masked-runtime",
            "enabled-runtime",
            "linked-runtime",
        ] {
            assert_eq!(enabled_state(state), Some(false), "{state}");
        }
        assert_eq!(enabled_state(""), None);
        assert_eq!(enabled_state("\n  \n"), None);
    }

    /// Nur was vorher nachweislich nicht aktiviert war, wird abgemeldet; ein
    /// unbekannter Stand bleibt der Suche nach Dateien überlassen.
    #[test]
    fn only_units_that_were_not_enabled_are_disabled() {
        let state = ManagerEnablement {
            before: vec![
                ("a.service".to_owned(), Some(false)),
                ("b.socket".to_owned(), Some(true)),
                ("c.service".to_owned(), None),
            ],
        };
        assert_eq!(state.not_enabled_before(), vec!["a.service"]);
        assert_eq!(state.unanswered(), vec!["c.service"]);
        assert_eq!(state.state_of("b.socket"), Some(true));
        assert_eq!(state.state_of("c.service"), None);
        assert_eq!(state.state_of("d.service"), None);
    }

    #[test]
    fn the_fragment_path_is_read_from_the_show_output() {
        assert_eq!(
            fragment_of("FragmentPath=/x/systemd/user/humanitld.service\n"),
            Fragment::File(PathBuf::from("/x/systemd/user/humanitld.service"))
        );
        assert_eq!(fragment_of("FragmentPath=\n"), Fragment::Missing);
        assert_eq!(fragment_of(""), Fragment::Unanswered);
    }

    #[test]
    fn the_unit_path_is_read_from_the_show_output() {
        assert_eq!(
            unit_path_of("UnitPath=/a/systemd/user /etc/systemd/user\n"),
            Some(vec![
                PathBuf::from("/a/systemd/user"),
                PathBuf::from("/etc/systemd/user")
            ])
        );
        assert_eq!(unit_path_of("MainPID=0\n"), None);
    }

    /// Ein Verzeichnis hinter einem Verweis ist dasselbe wie sein Ziel: So
    /// sieht es aus, wenn `~/.config` ein Verweis eines Dotfile-Verwalters ist
    /// und systemd den aufgelösten Pfad nennt (HUM-217, Review).
    #[test]
    fn a_path_through_a_linked_directory_is_the_same_file() {
        let base = tempfile::tempdir().expect("a base");
        let real = base.path().join("real");
        std::fs::create_dir(&real).expect("the real directory");
        std::fs::write(real.join("humanitld.service"), "").expect("a unit");
        let linked = base.path().join("linked");
        std::os::unix::fs::symlink(&real, &linked).expect("a linked directory");

        assert!(same_file(
            &real.join("humanitld.service"),
            &linked.join("humanitld.service")
        ));
        assert!(!same_file(
            &real.join("humanitld.service"),
            &base.path().join("other.service")
        ));
    }

    /// Der Befund nennt das Verzeichnis dieser Kommandozeile und das, aus dem
    /// systemd lädt oder in dem es sucht.
    #[test]
    fn the_finding_names_both_directories() {
        let path = Path::new("/cli/systemd/user/humanitld.service");
        for (elsewhere, theirs) in [
            (
                Elsewhere::Loads(PathBuf::from("/manager/systemd/user/humanitld.service")),
                "/manager/systemd/user",
            ),
            (
                Elsewhere::Searches(vec![PathBuf::from("/manager/systemd/user")]),
                "/manager/systemd/user",
            ),
        ] {
            let finding = not_loaded(path, &elsewhere, "it is gone again");
            assert_eq!(finding.code.as_str(), "DAEMON_008");
            assert!(finding.why.contains("/cli/systemd/user"), "{}", finding.why);
            assert!(finding.why.contains(theirs), "{}", finding.why);
            assert!(finding.fix.is_some());
        }
    }
}
