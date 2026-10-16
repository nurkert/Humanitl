//! Ob noch ein Prozess aus einer Kopie unter `~/.local/lib/humanitl` laufen
//! kann (HUM-210).
//!
//! Eine Kopie zu löschen, aus der ein Daemon läuft, nimmt ihm den Shim: Er
//! sucht ihn neben `/proc/self/exe`, und dort steht dann nur noch
//! `…/bin/humanitld (deleted)`. Jede Sandbox scheitert danach mit
//! `SANDBOX_011`, bis jemand den Dienst von Hand neu startet. Deshalb fragt
//! das Aufräumen vor jedem `remove_dir_all` den Kernel, ob ein Prozess sein
//! Programm aus diesem Verzeichnis hat.
//!
//! **Was sich nicht klären lässt, zählt als laufend.** Die Kopie bleibt dann
//! liegen, und ein späterer Lauf räumt sie weg; eine Kopie zu viel kostet
//! Platz, eine zu wenig kostet jede Sandbox. Unklar ist es in diesen Fällen:
//!
//! - `/proc` lässt sich nicht lesen, auch nicht ein einzelner Eintrag beim
//!   Durchgehen.
//! - systemd sagt nichts Verwertbares über den Hauptprozess des Dienstes
//!   ([`MainPid::Unknown`]): `systemctl --user show -p MainPID` scheitert,
//!   antwortet ohne Zeile `MainPID=` oder mit einem Wert, der keine Zahl ist.
//! - systemd nennt einen Hauptprozess ([`MainPid::Pid`]), dessen Programm sich
//!   nicht lesen lässt, der in diesem `/proc` gar nicht auftaucht oder der hier
//!   nicht `humanitld` ist. So sieht es aus, wenn die Kommandozeile in einem
//!   anderen PID-Namensraum läuft als der Daemon; sie sieht ihn dann nicht oder
//!   unter seiner Nummer einen anderen Prozess.
//! - Ein Prozess lässt sein Programm nicht lesen, und sein `status` sagt nicht
//!   verlässlich, wem er gehört und wie er heißt: `status` fehlt (außer der
//!   Prozess ist inzwischen fort), lässt sich nicht lesen oder trägt `Name:`
//!   oder `Uid:` nicht.
//! - Ein Prozess dieses Kontos namens `humanitld` lässt sein Programm nicht
//!   lesen, etwa weil er sich mit `PR_SET_DUMPABLE` verschlossen hat.
//!
//! Prozesse anderer Konten, deren Programm sich nicht lesen lässt, zählen
//! nicht, sobald ihr `status` oder, wenn auch das nicht lesbar ist, das
//! Verzeichnis unter `/proc` das Konto nennt: Ihre Daemons laufen aus ihren
//! eigenen Kopien, und nur Kopien des eigenen Kontos räumt die Kommandozeile
//! weg. So hält auch unter `hidepid` oder einem LSM kein fremder Prozess das
//! Aufräumen auf; offen bleibt es nur bei Prozessen dieses Kontos oder bei
//! solchen, deren Konto sich gar nicht feststellen lässt.

use std::path::Path;

/// Wo der Kernel die Prozesse zeigt.
const PROC: &str = "/proc";

/// Der Name des Daemons, wie ihn `/proc/<pid>/status` in `Name:` führt.
const DAEMON_COMM: &str = "humanitld";

/// Was systemd über den Hauptprozess des Dienstes sagt
/// (`systemctl --user show -p MainPID`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MainPid {
    /// systemd nennt keinen: `MainPID=0`, der Dienst läuft nicht.
    Absent,
    /// Der Hauptprozess.
    Pid(u32),
    /// Die Frage ging nicht durch; es ist offen, ob und woraus er läuft.
    Unknown,
}

impl MainPid {
    /// Liest die Antwort von `systemctl --user show -p MainPID <unit>`.
    ///
    /// Nur `MainPID=0` heißt bei systemd: kein Hauptprozess. Eine Antwort ohne
    /// diese Zeile oder mit einem Wert, der keine Zahl ist, lässt die Frage
    /// offen.
    pub(super) fn parse(output: &str) -> Self {
        let value = output
            .lines()
            .find_map(|line| line.trim().strip_prefix("MainPID="));
        match value.map(str::parse::<u32>) {
            Some(Ok(0)) => Self::Absent,
            Some(Ok(pid)) => Self::Pid(pid),
            None | Some(Err(_)) => Self::Unknown,
        }
    }
}

/// Ob ein Prozess sein Programm aus `dir` oder einem Unterverzeichnis davon
/// haben kann; im Zweifel wahr (siehe Moduldoku).
///
/// `owner` ist das Konto, dessen Kopien geräumt werden. Verglichen wird mit
/// dem Pfad, wie er dasteht, und mit dem aufgelösten: Der Kernel nennt den Pfad
/// ohne Verweise, `dir` kann einen enthalten, etwa ein Heimatverzeichnis
/// hinter einem Verweis. Das Anhängsel ` (deleted)` einer gelöschten Datei
/// stört den Vergleich nicht, denn es hängt am letzten Bestandteil, und `dir`
/// ist ein Vorfahr.
pub(super) fn runs_from(dir: &Path, owner: u32, main: MainPid) -> bool {
    runs_from_in(Path::new(PROC), dir, owner, main)
}

/// [`runs_from`] über ein beliebiges `/proc`; für Tests mit einem
/// nachgebauten Baum, der keinen Prozess des Rechners berührt.
fn runs_from_in(proc_root: &Path, dir: &Path, owner: u32, main: MainPid) -> bool {
    match main {
        MainPid::Unknown => return true,
        MainPid::Pid(pid) => {
            if !is_daemon(&proc_root.join(pid.to_string())) {
                return true;
            }
        }
        MainPid::Absent => {}
    }
    let Ok(entries) = std::fs::read_dir(proc_root) else {
        return true;
    };
    any_runs_from(
        entries.map(|entry| entry.map(|entry| entry.path())),
        dir,
        owner,
    )
}

/// Geht die Einträge eines `/proc` durch; ein Eintrag, der sich nicht lesen
/// lässt, zählt als laufend.
fn any_runs_from(
    entries: impl Iterator<Item = std::io::Result<std::path::PathBuf>>,
    dir: &Path,
    owner: u32,
) -> bool {
    let resolved = std::fs::canonicalize(dir).ok();
    for entry in entries {
        let Ok(process) = entry else {
            return true;
        };
        let pid = process.file_name().and_then(|name| name.to_str());
        if !pid.is_some_and(is_pid) {
            continue;
        }
        let runs = match std::fs::read_link(process.join("exe")) {
            Ok(exe) => {
                let under_real = resolved.as_ref().is_some_and(|real| exe.starts_with(real));
                exe.starts_with(dir) || under_real
            }
            // Fort, oder ein Kernel-Thread ohne Programm.
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
            Err(_) => hidden_daemon(&process, owner),
        };
        if runs {
            return true;
        }
    }
    false
}

/// Ob der Prozess unter `proc_dir` sichtbar der Daemon ist: sein Programm
/// lässt sich lesen und heißt `humanitld` (auch mit ` (deleted)`), oder sein
/// `status` nennt `Name: humanitld`.
///
/// In einem anderen PID-Namensraum kann die Nummer, die systemd nennt, hier
/// zu einem ganz anderen, lesbaren Prozess gehören. Wäre lesbar schon genug,
/// ginge die Prüfung auf den Hauptprozess dann ins Leere (HUM-210, Review).
fn is_daemon(proc_dir: &Path) -> bool {
    let Ok(exe) = std::fs::read_link(proc_dir.join("exe")) else {
        return false;
    };
    let named = exe
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with(DAEMON_COMM));
    named
        || std::fs::read_to_string(proc_dir.join("status")).is_ok_and(|status| {
            status
                .lines()
                .any(|line| line.strip_prefix("Name:").map(str::trim) == Some(DAEMON_COMM))
        })
}

/// Ob ein Eintrag unter `/proc` ein Prozess ist: nur Ziffern.
fn is_pid(name: &str) -> bool {
    !name.is_empty() && name.bytes().all(|byte| byte.is_ascii_digit())
}

/// Ob der Prozess unter `proc_dir`, dessen Programm sich nicht lesen ließ, ein
/// Daemon dieses Kontos sein kann.
///
/// `status` bleibt lesbar, auch wenn `exe` es nicht ist; `Name:` ist der Name
/// des Programms, `Uid:` beginnt mit dem echten Konto. Fehlt `status`, weil der
/// Prozess inzwischen fort ist, zählt er nicht. Lässt es sich sonst nicht
/// lesen oder fehlt eines der beiden Felder, ist offen, wem er gehört, und er
/// zählt als laufend.
fn hidden_daemon(proc_dir: &Path, owner: u32) -> bool {
    let status = match std::fs::read_to_string(proc_dir.join("status")) {
        Ok(status) => status,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return false,
        // Unter `hidepid` oder einem LSM bleibt `stat` auf dem Verzeichnis
        // möglich und nennt das Konto. Nur ein Prozess dieses Kontos, oder
        // einer, dessen Konto sich gar nicht feststellen lässt, hält die Kopie.
        Err(_) => {
            use std::os::unix::fs::MetadataExt as _;
            return match std::fs::metadata(proc_dir) {
                Ok(meta) => meta.uid() == owner,
                Err(_) => true,
            };
        }
    };
    let field = |key: &str| {
        status
            .lines()
            .find_map(|line| line.strip_prefix(key))
            .map(str::trim)
    };
    let uid = field("Uid:")
        .and_then(|value| value.split_whitespace().next())
        .and_then(|value| value.parse::<u32>().ok());
    match (field("Name:"), uid) {
        (Some(name), Some(uid)) => name == DAEMON_COMM && uid == owner,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::os::unix::fs::MetadataExt as _;

    use super::{MainPid, runs_from};

    /// Das Konto dieses Tests.
    fn me() -> u32 {
        std::fs::metadata("/proc/self").expect("/proc/self").uid()
    }

    /// Der Testprozess selbst läuft aus dem Verzeichnis seines Binaries, aus
    /// einem leeren Verzeichnis daneben läuft nichts.
    #[test]
    fn the_running_test_binary_is_found_and_an_empty_directory_is_not() {
        let exe = std::env::current_exe().expect("the test binary");
        let dir = exe.parent().expect("its directory");
        assert!(
            runs_from(dir, me(), MainPid::Absent),
            "{} is not seen",
            dir.display()
        );

        let empty = tempfile::tempdir().expect("an empty directory");
        assert!(!runs_from(empty.path(), me(), MainPid::Absent));
    }

    /// Ein Hauptprozess, den dieses `/proc` nicht zeigt, und eine offene
    /// Antwort von systemd lassen jede Kopie stehen; ein sichtbarer nicht.
    #[test]
    fn an_invisible_or_unknown_main_process_keeps_every_copy() {
        let empty = tempfile::tempdir().expect("an empty directory");
        // Größer als jede mögliche Prozessnummer (`PID_MAX_LIMIT`, 2^22).
        assert!(runs_from(empty.path(), me(), MainPid::Pid(4_194_304)));
        assert!(runs_from(empty.path(), me(), MainPid::Unknown));
    }

    /// Ein nachgebautes `/proc` mit einem lesbaren Prozess `pid`, dessen
    /// Programm `exe` ist und dessen `status` `name` nennt.
    fn proc_with_a_visible_process(pid: u32, exe: &str, name: &str) -> tempfile::TempDir {
        let root = tempfile::tempdir().expect("a fake /proc");
        let process = root.path().join(pid.to_string());
        std::fs::create_dir(&process).expect("a process directory");
        std::os::unix::fs::symlink(exe, process.join("exe")).expect("a readable exe");
        std::fs::write(process.join("status"), format!("Name:\t{name}\n")).expect("the status");
        root
    }

    /// Gehört die Nummer, die systemd nennt, hier zu einem anderen lesbaren
    /// Prozess -- in einem anderen PID-Namensraum ist das möglich --, bleibt
    /// jede Kopie. Ist es sichtbar der Daemon, nach Programm oder nach
    /// `status`, entscheidet die Suche über die Kopie (HUM-210, Review).
    #[test]
    fn a_main_process_that_is_not_the_daemon_keeps_every_copy() {
        use super::runs_from_in;

        let copy = tempfile::tempdir().expect("a copy");
        let other = proc_with_a_visible_process(78, "/usr/bin/sleep", "sleep");
        assert!(runs_from_in(
            other.path(),
            copy.path(),
            me(),
            MainPid::Pid(78)
        ));

        let daemon = proc_with_a_visible_process(77, "/opt/x/bin/humanitld (deleted)", "x");
        assert!(!runs_from_in(
            daemon.path(),
            copy.path(),
            me(),
            MainPid::Pid(77)
        ));
        let by_status = proc_with_a_visible_process(79, "/opt/x/bin/other", "humanitld");
        assert!(!runs_from_in(
            by_status.path(),
            copy.path(),
            me(),
            MainPid::Pid(79)
        ));
    }

    /// Lässt sich `status` nicht lesen (`hidepid`, ein LSM), nennt das
    /// Verzeichnis unter `/proc` das Konto: Ein Prozess eines anderen Kontos
    /// hält die Kopie nicht, einer dieses Kontos schon (HUM-210, Review).
    #[test]
    fn under_hidepid_a_foreign_process_does_not_keep_the_copy() {
        use super::runs_from_in;

        let copy = tempfile::tempdir().expect("a copy");
        let proc_root = proc_with_a_hidden_process("humanitld", me());
        let status = proc_root.path().join("123/status");
        std::fs::remove_file(&status).expect("the status goes");
        std::fs::create_dir(&status).expect("an unreadable status");
        // Das Verzeichnis gehört dem Konto des Tests; für ein anderes Konto
        // ist der Prozess fremd.
        assert!(!runs_from_in(
            proc_root.path(),
            copy.path(),
            me() + 1,
            MainPid::Absent
        ));
        assert!(runs_from_in(
            proc_root.path(),
            copy.path(),
            me(),
            MainPid::Absent
        ));
    }

    /// Ein nachgebautes `/proc` mit einem Prozess `123`, dessen `exe` sich
    /// nicht lesen lässt (eine gewöhnliche Datei statt eines Verweises, wie
    /// bei einem Prozess, der sich mit `PR_SET_DUMPABLE` verschlossen hat),
    /// und dem `status` aus `name` und `uid`.
    fn proc_with_a_hidden_process(name: &str, uid: u32) -> tempfile::TempDir {
        let root = tempfile::tempdir().expect("a fake /proc");
        let process = root.path().join("123");
        std::fs::create_dir(&process).expect("a process directory");
        std::fs::write(process.join("exe"), b"").expect("an unreadable exe");
        std::fs::write(
            process.join("status"),
            format!("Name:\t{name}\nUmask:\t0022\nUid:\t{uid}\t{uid}\t{uid}\t{uid}\n"),
        )
        .expect("the status");
        root
    }

    /// Ein Daemon dieses Kontos, dessen Programm sich nicht lesen lässt, lässt
    /// jede Kopie stehen; ein anderer Prozess oder ein Daemon eines anderen
    /// Kontos nicht (HUM-210, Review).
    #[test]
    fn a_hidden_daemon_of_this_account_keeps_every_copy() {
        use super::runs_from_in;

        let copy = tempfile::tempdir().expect("a copy");
        let proc_root = proc_with_a_hidden_process("humanitld", me());
        assert!(runs_from_in(
            proc_root.path(),
            copy.path(),
            me(),
            MainPid::Absent
        ));

        let other_name = proc_with_a_hidden_process("ssh-agent", me());
        assert!(!runs_from_in(
            other_name.path(),
            copy.path(),
            me(),
            MainPid::Absent
        ));
        let other_account = proc_with_a_hidden_process("humanitld", me() + 1);
        assert!(!runs_from_in(
            other_account.path(),
            copy.path(),
            me(),
            MainPid::Absent
        ));
    }

    /// Lässt sich das `status` eines Prozesses mit unlesbarem Programm nicht
    /// lesen oder fehlt darin `Name:` oder `Uid:`, ist offen, wem er gehört,
    /// und jede Kopie bleibt. Ist es fort, weil der Prozess eben endete, zählt
    /// er nicht (HUM-210, Review).
    #[test]
    fn an_unclear_status_keeps_every_copy_and_a_gone_one_does_not() {
        use super::runs_from_in;

        let copy = tempfile::tempdir().expect("a copy");
        let unreadable = proc_with_a_hidden_process("humanitld", me());
        let status = unreadable.path().join("123/status");
        std::fs::remove_file(&status).expect("the status goes");
        // Ein Verzeichnis statt der Datei: Lesen scheitert, aber nicht mit
        // `NotFound`.
        std::fs::create_dir(&status).expect("an unreadable status");
        assert!(runs_from_in(
            unreadable.path(),
            copy.path(),
            me(),
            MainPid::Absent
        ));

        for incomplete in ["Name:\thumanitld\n", "Uid:\t1\t1\t1\t1\n"] {
            let proc_root = proc_with_a_hidden_process("humanitld", me());
            std::fs::write(proc_root.path().join("123/status"), incomplete)
                .expect("an incomplete status");
            assert!(
                runs_from_in(proc_root.path(), copy.path(), me(), MainPid::Absent),
                "{incomplete:?}"
            );
        }

        let gone = proc_with_a_hidden_process("humanitld", me());
        std::fs::remove_file(gone.path().join("123/status")).expect("the status goes");
        assert!(!runs_from_in(
            gone.path(),
            copy.path(),
            me(),
            MainPid::Absent
        ));
    }

    /// Ein Eintrag, der sich beim Durchgehen von `/proc` nicht lesen lässt,
    /// lässt jede Kopie stehen (HUM-210, Review).
    #[test]
    fn an_unreadable_proc_entry_keeps_every_copy() {
        use super::any_runs_from;

        let copy = tempfile::tempdir().expect("a copy");
        let entries = vec![Err(std::io::Error::other("readdir failed"))];
        assert!(any_runs_from(entries.into_iter(), copy.path(), me()));
        let none: Vec<std::io::Result<std::path::PathBuf>> = Vec::new();
        assert!(!any_runs_from(none.into_iter(), copy.path(), me()));
    }

    /// Nur `MainPID=0` heißt „kein Hauptprozess"; eine leere Antwort lässt die
    /// Frage offen (HUM-210, Review).
    #[test]
    fn main_pid_is_read_from_the_show_output() {
        assert_eq!(MainPid::parse("MainPID=4242\n"), MainPid::Pid(4242));
        assert_eq!(MainPid::parse("MainPID=0\n"), MainPid::Absent);
        assert_eq!(MainPid::parse(""), MainPid::Unknown);
        assert_eq!(MainPid::parse("MainPID=x\n"), MainPid::Unknown);
    }
}
