//! Die Nutzer-Unit des Daemons: rendern, prüfen, schreiben, zurücknehmen
//! (HUM-044).
//!
//! `humanitl daemon install` ist das Eingriffsreichste, was dieses Produkt
//! außerhalb der Sandbox tut: Es legt eine Datei auf dem Rechner eines
//! Menschen ab, die von da an bei jeder Anmeldung einen Dienst startet. Dieses
//! Modul hält die vier Zusagen, die daran hängen, und hält sie an einer
//! Stelle, damit sie prüfbar sind, ohne dass ein Test systemd braucht:
//!
//! 1. **Genau eine Datei, an einem genannten Ort.**
//!    `$XDG_CONFIG_HOME/systemd/user/humanitld.service`, sonst
//!    `~/.config/systemd/user/humanitld.service`. Keine System-Unit, kein
//!    `sudo`, keine zweite Datei, kein Socket.
//! 2. **Sichtbar, bevor es geschieht.** Der Inhalt entsteht hier und wird vom
//!    Aufrufer angezeigt, bevor irgendetwas geschrieben wird.
//! 3. **Wiederholbar.** Ein zweiter Aufruf mit demselben Ergebnis schreibt
//!    nicht noch einmal ([`Written::Unchanged`]).
//! 4. **Nie über fremdes Eigentum.** Die erste Zeile der Vorlage ist die
//!    Marke. Fehlt sie in einer vorhandenen Datei, gehört die Datei jemand
//!    anderem, und [`prepare`] liefert `DAEMON_005`, statt sie zu ersetzen.
//!
//! Dazu kommt die fünfte, die im Fehlerfall zählt: Geschrieben wird über eine
//! Nachbardatei und `rename`, und [`rollback`] stellt den Zustand von vorher
//! wieder her. Nach einem gescheiterten Aufruf liegt entweder die alte Fassung
//! da oder gar keine — nie eine halbe und nie eine, die niemand bestellt hat.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use humanitl_config::Paths;
use humanitl_core::diagnostics::codes::{DAEMON_005, DAEMON_006, DAEMON_007};
use humanitl_core::{Diagnostic, FixAction, Severity};

/// Die Vorlage, wie sie im Repository liegt.
///
/// Eingebettet und nicht zur Laufzeit gelesen: Die Unit, die der Befehl
/// schreibt, gehört zu der Fassung des Programms, das ihn ausführt. Eine
/// Datei, die daneben liegt und sich ändern kann, wäre eine zweite Quelle.
pub const TEMPLATE: &str = include_str!("../../../../../packaging/systemd/humanitld.service");

/// Der Platzhalter der Vorlage, in den der Pfad des Daemons kommt.
pub const PLACEHOLDER: &str = "{humanitld}";

/// Die Marke in der ersten Zeile: Nur eine Datei, die sie trägt, wird ersetzt.
pub const MARKER: &str = "# humanitl daemon install: written by Humanitl";

/// Der Name der Unit.
pub const UNIT_NAME: &str = "humanitld.service";

/// Der Name des Daemons, wie er neben der Kommandozeile liegt.
pub const DAEMON_NAME: &str = "humanitld";

/// Zeichen, die in einem `ExecStart` nicht wörtlich stehen bleiben.
///
/// systemd zerlegt `ExecStart` an Leerraum, kennt `"` und `'` als Anführung,
/// `\` als Fluchtzeichen und `%` als Einleitung eines Spezifikators (`%h` ist
/// das Heimatverzeichnis). Ein Pfad mit einem dieser Zeichen ergäbe also eine
/// Zeile, die etwas anderes startet, als sie zeigt — derselbe Fehler wie ein
/// interpolierter `CopyCommand` (`humanitl_sandbox::doctor::shell_command`).
/// Er wird hier nicht zitiert, sondern abgelehnt: Ein Daemon-Pfad mit einem
/// Prozentzeichen ist kein Fall, den irgendjemand braucht, und eine Ablehnung
/// mit Grund ist besser als eine Zitierung, deren Auslegung niemand nachprüft.
const REFUSED_IN_EXEC_START: &[char] = &['"', '\'', '\\', '%', ';'];

/// Was ein Schreibvorgang mit der Datei gemacht hat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Written {
    /// Die Datei stand schon genau so da; nichts wurde angefasst.
    Unchanged,
    /// Es gab sie nicht; sie wurde angelegt.
    Created,
    /// Es gab eine Fassung mit der Marke; sie wurde ersetzt.
    Replaced {
        /// Was vorher darin stand, für [`rollback`].
        previous: String,
    },
}

impl Written {
    /// Das Wort für die Ausgabe.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Unchanged => "unchanged",
            Self::Created => "created",
            Self::Replaced { .. } => "replaced",
        }
    }

    /// Wahr, wenn dieser Aufruf die Datei angefasst hat.
    #[must_use]
    pub const fn changed(&self) -> bool {
        !matches!(self, Self::Unchanged)
    }
}

/// Das Verzeichnis der Nutzer-Units.
///
/// `$XDG_CONFIG_HOME/systemd/user`, sonst `~/.config/systemd/user`. Der Weg
/// führt über [`Paths::config_dir`], weil dort dieselbe XDG-Auflösung steht wie
/// für alles andere; das Anhängsel `humanitl` fällt wieder weg, denn die Unit
/// gehört systemd und nicht diesem Produkt.
#[must_use]
pub fn unit_dir(paths: &Paths) -> PathBuf {
    let config = paths.config_dir();
    // `config_dir()` endet immer auf `humanitl`, also gibt es immer ein
    // Elternverzeichnis; der Zweig ohne ist nur da, weil der Typ ihn zulässt.
    let base = config
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
    base.join("systemd").join("user")
}

/// Der Pfad der Unit.
#[must_use]
pub fn unit_path(paths: &Paths) -> PathBuf {
    unit_dir(paths).join(UNIT_NAME)
}

/// Der Daemon neben `current_exe`, aufgelöst bis zur wirklichen Datei.
///
/// Nie aus `PATH` und nie aus einem Konfigurationswert: Was beim Anmelden
/// startet, soll dieselbe Fassung sein wie das Programm, das die Unit
/// geschrieben hat. Ein `PATH`, der sich später ändert, würde sonst eine
/// andere starten, ohne dass jemand etwas angefasst hätte.
///
/// **Symlinks werden aufgelöst, nicht abgelehnt.** Von den beiden möglichen
/// Antworten auf einen symbolisch verlinkten `humanitld` — ablehnen oder
/// kanonisieren — steht hier die zweite, aus zwei Gründen. Erstens ist der
/// Verweis der Normalfall einer Installation: Ein Paket legt das Programm unter
/// `/usr/lib/humanitl/` ab und verlinkt es nach `/usr/bin`, und ein Nutzer
/// verlinkt aus `~/.local/bin` in einen Ordner mit Versionsnummer. Eine
/// Ablehnung machte `daemon install` genau dort unbrauchbar, wo es gebraucht
/// wird. Zweitens ist der Verweis dieselbe Gefahr wie `PATH`, die dieses Modul
/// schon abwehrt: Wer ihn später umhängt, ändert sonst, was beim nächsten
/// Anmelden startet, ohne die Unit anzufassen. [`std::fs::canonicalize`] löst
/// jeden Verweis und jedes `..` im ganzen Pfad auf, und in `ExecStart` steht
/// danach die Datei selbst.
///
/// # Errors
///
/// `DAEMON_007`, wenn neben der Kommandozeile keine ausführbare Datei
/// `humanitld` liegt, ihr wirklicher Pfad sich nicht auflösen lässt oder er
/// nicht wörtlich in ein `ExecStart` passt.
pub fn daemon_binary(current_exe: &Path) -> Result<PathBuf, Diagnostic> {
    let Some(dir) = current_exe.parent() else {
        return Err(missing_daemon(current_exe, "it has no directory"));
    };
    let candidate = dir.join(DAEMON_NAME);
    if !candidate.is_file() {
        return Err(missing_daemon(
            &candidate,
            "there is no such file next to the running humanitl",
        ));
    }
    let real = std::fs::canonicalize(&candidate).map_err(|error| {
        missing_daemon(
            &candidate,
            &format!("its real path cannot be resolved: {error}"),
        )
    })?;
    if !real.is_file() {
        return Err(missing_daemon(
            &real,
            "the link next to the running humanitl does not lead to a file",
        ));
    }
    exec_start_word(&real)?;
    Ok(real)
}

/// Der Pfad als das eine Wort, das er in `ExecStart` sein muss.
///
/// Der Weg vom Pfad zum Text führt über [`Path::to_str`] und nicht über
/// `to_string_lossy`: Ein Pfad, der kein gültiges UTF-8 ist, wird von der
/// verlustbehafteten Fassung zu einem *anderen* Pfad — jedem ungültigen Byte
/// entspricht dort `U+FFFD` — und geprüft würde dann der andere, geschrieben
/// ebenfalls. Die Unit startete eine Datei, die es unter diesem Namen nicht
/// gibt, oder schlimmer eine, die jemand unter dem ersetzten Namen anlegt.
/// Deshalb wird ein solcher Pfad abgelehnt.
///
/// # Errors
///
/// `DAEMON_007`, wenn der Pfad kein gültiges UTF-8 ist, nicht absolut ist,
/// Leerraum trägt oder eines der Zeichen aus [`REFUSED_IN_EXEC_START`]
/// enthält.
pub fn exec_start_word(program: &Path) -> Result<String, Diagnostic> {
    let Some(text) = program.to_str().map(str::to_owned) else {
        return Err(missing_daemon(
            program,
            "its path is not valid UTF-8, and a lossy rendering of it would name a different \
             file than the one that is there",
        ));
    };
    if !program.is_absolute() {
        return Err(missing_daemon(program, "its path is not absolute"));
    }
    if let Some(found) = text
        .chars()
        .find(|c| c.is_whitespace() || c.is_control() || REFUSED_IN_EXEC_START.contains(c))
    {
        return Err(missing_daemon(
            program,
            &format!(
                "its path contains {found:?}, and systemd would read ExecStart as something \
                 other than this one file"
            ),
        ));
    }
    Ok(text)
}

/// Die Unit für diesen Daemon.
///
/// # Errors
///
/// `DAEMON_007`, wenn der Pfad nicht in ein `ExecStart` passt.
pub fn render(daemon: &Path) -> Result<String, Diagnostic> {
    let word = exec_start_word(daemon)?;
    Ok(TEMPLATE.replace(PLACEHOLDER, &word))
}

/// Was ein Schreiben an dieser Stelle täte, ohne etwas zu tun.
///
/// # Errors
///
/// `DAEMON_005`, wenn dort eine Datei ohne die Marke liegt; `DAEMON_006`, wenn
/// sie sich nicht lesen lässt.
pub fn prepare(path: &Path, contents: &str) -> Result<Written, Diagnostic> {
    match std::fs::read_to_string(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Written::Created),
        Err(error) => Err(unwritable(path, &format!("cannot be read: {error}"))),
        Ok(existing) => {
            if !carries_marker(&existing) {
                return Err(foreign(path));
            }
            if existing == contents {
                Ok(Written::Unchanged)
            } else {
                Ok(Written::Replaced { previous: existing })
            }
        }
    }
}

/// Wahr, wenn die erste Zeile die Marke ist.
#[must_use]
pub fn carries_marker(contents: &str) -> bool {
    contents
        .lines()
        .next()
        .is_some_and(|line| line.trim_end() == MARKER)
}

/// Schreibt die Unit, wenn [`prepare`] sagt, dass sie sich ändert.
///
/// Geschrieben wird über eine Nachbardatei und `rename`: Ein abgebrochener
/// Schreibvorgang hinterlässt damit keine halbe Unit, die systemd beim
/// nächsten Anmelden zu lesen versuchte. Die Nachbardatei wird in jedem
/// Fehlerfall wieder entfernt.
///
/// # Errors
///
/// `DAEMON_006`, wenn das Verzeichnis nicht anlegbar oder die Datei nicht
/// schreibbar ist.
pub fn write(path: &Path, contents: &str, plan: &Written) -> Result<(), Diagnostic> {
    if !plan.changed() {
        return Ok(());
    }
    let Some(dir) = path.parent() else {
        return Err(unwritable(path, "has no directory"));
    };
    std::fs::create_dir_all(dir)
        .map_err(|error| unwritable(dir, &format!("cannot be created: {error}")))?;
    let scratch = dir.join(format!("{UNIT_NAME}.new-{}", std::process::id()));
    if let Err(error) = std::fs::write(&scratch, contents) {
        let _ = std::fs::remove_file(&scratch);
        return Err(unwritable(&scratch, &format!("cannot be written: {error}")));
    }
    if let Err(error) = set_mode(&scratch) {
        let _ = std::fs::remove_file(&scratch);
        return Err(unwritable(
            &scratch,
            &format!("its mode cannot be set: {error}"),
        ));
    }
    if let Err(error) = std::fs::rename(&scratch, path) {
        let _ = std::fs::remove_file(&scratch);
        return Err(unwritable(path, &format!("cannot be replaced: {error}")));
    }
    Ok(())
}

/// Nimmt zurück, was [`write()`] getan hat.
///
/// Eine angelegte Datei verschwindet, eine ersetzte bekommt ihren alten Inhalt
/// zurück, eine unveränderte bleibt unberührt. Der Rückgabewert sagt, ob das
/// gelungen ist; ein misslungenes Zurücknehmen wird gemeldet und nicht
/// verschwiegen.
///
/// # Errors
///
/// `DAEMON_006`, wenn sich der Zustand von vorher nicht wiederherstellen lässt.
pub fn rollback(path: &Path, plan: &Written) -> Result<(), Diagnostic> {
    match plan {
        Written::Unchanged => Ok(()),
        Written::Created => match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(unwritable(
                path,
                &format!("cannot be taken back again: {error}"),
            )),
        },
        Written::Replaced { previous } => write(path, previous, &Written::Created),
    }
}

/// Die Endungen der Verzeichnisse, in denen die Aktivierung einer Unit steht.
///
/// `systemctl --user enable` legt für jedes `WantedBy=` einen Verweis
/// `<ziel>.wants/humanitld.service` an und für jedes `RequiredBy=` einen unter
/// `<ziel>.requires/`, jeweils neben der Unit selbst. Diese Vorlage nennt nur
/// `WantedBy=default.target`; `.requires` steht trotzdem hier, damit eine
/// spätere Zeile in der Vorlage nicht stillschweigend an der Rücknahme
/// vorbeiläuft.
const ENABLEMENT_DIR_SUFFIXES: [&str; 2] = [".wants", ".requires"];

/// Was an Aktivierung dieser Unit im Unit-Verzeichnis steht.
///
/// Mehr Zustand hat die Aktivierung einer Nutzer-Unit nicht: Sie besteht aus
/// Verweisen in `<ziel>.wants`- und `<ziel>.requires`-Verzeichnissen, nicht aus
/// einem Eintrag in einer Datenbank. Deshalb lässt sie sich hier lesen und
/// zurücknehmen, ohne dass ein Test systemd braucht — und deshalb funktioniert
/// die Rücknahme auch dann, wenn `systemctl` selbst gerade der Grund des
/// Fehlschlags ist.
///
/// [`Enablement::read`] nimmt den Zustand **vor** dem ersten `systemctl`-Aufruf
/// auf, [`Enablement::rollback`] entfernt danach genau das, was seither
/// dazugekommen ist. Was schon vorher dalag, bleibt liegen: `daemon install`
/// nimmt nur zurück, was dieser Lauf angerichtet hat.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Enablement {
    /// Die `*.wants`- und `*.requires`-Verzeichnisse, die es schon gab.
    dirs: BTreeSet<PathBuf>,
    /// Die Verweise auf [`UNIT_NAME`] darin, die es schon gab.
    links: BTreeSet<PathBuf>,
}

impl Enablement {
    /// Liest den Zustand der Aktivierung unter `dir`.
    ///
    /// Ein Verzeichnis, das sich nicht lesen lässt, ergibt einen leeren
    /// Zustand: Es gibt dann nichts, was dieser Lauf später als „schon vorher
    /// da" verschonen müsste, und die Rücknahme entfernt lieber einen Verweis
    /// zu viel als einen zu wenig.
    #[must_use]
    pub fn read(dir: &Path) -> Self {
        let mut state = Self::default();
        let Ok(entries) = std::fs::read_dir(dir) else {
            return state;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            if !ENABLEMENT_DIR_SUFFIXES
                .iter()
                .any(|suffix| name.ends_with(suffix))
            {
                continue;
            }
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let link = path.join(UNIT_NAME);
            if link.symlink_metadata().is_ok() {
                state.links.insert(link);
            }
            state.dirs.insert(path);
        }
        state
    }

    /// Entfernt, was seit diesem Zustand an Aktivierung dazugekommen ist.
    ///
    /// Zuerst die Verweise, dann die Verzeichnisse, die es vorher nicht gab und
    /// die jetzt leer sind. Ein Verzeichnis, das schon vorher dalag oder noch
    /// etwas anderes enthält, bleibt unberührt: Darin steht die Aktivierung
    /// anderer Dienste.
    ///
    /// # Errors
    ///
    /// `DAEMON_006`, wenn sich ein Verweis nicht entfernen lässt. Der Befund
    /// nennt alle, die stehen blieben; ein halb zurückgenommener Zustand wird
    /// gemeldet und nicht verschwiegen.
    pub fn rollback(&self, dir: &Path) -> Result<(), Diagnostic> {
        let now = Self::read(dir);
        let mut left: Vec<String> = Vec::new();
        for link in now.links.difference(&self.links) {
            match std::fs::remove_file(link) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => left.push(format!("{} ({error})", link.display())),
            }
        }
        for created in now.dirs.difference(&self.dirs) {
            // Schlägt fehl, solange noch etwas darin steht, und genau dann soll
            // es fehlschlagen.
            let _ = std::fs::remove_dir(created);
        }
        if left.is_empty() {
            return Ok(());
        }
        Err(unwritable(
            dir,
            &format!(
                "still holds the enablement links this run created: {}",
                left.join(", ")
            ),
        ))
    }
}

/// `0644`: systemd liest die Unit, und niemand sonst schreibt sie.
fn set_mode(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;

    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o644))
}

/// `DAEMON_005`: Dort liegt die Unit von jemand anderem.
fn foreign(path: &Path) -> Diagnostic {
    Diagnostic::builder(DAEMON_005, Severity::Blocking)
        .why(format!(
            "{} exists and does not start with the line Humanitl writes, so Humanitl did not \
             write it; it decides what starts at every login and is not overwritten",
            path.display()
        ))
        .fix(command_or_docs(&[
            "mv",
            &path.to_string_lossy(),
            &format!("{}.bak", path.display()),
        ]))
        .build()
}

/// `DAEMON_006`: Der Pfad liess sich nicht schreiben.
fn unwritable(path: &Path, why: &str) -> Diagnostic {
    Diagnostic::builder(DAEMON_006, Severity::Blocking)
        .why(format!("{} {why}", path.display()))
        .fix(command_or_docs(&["ls", "-ld", &path.to_string_lossy()]))
        .build()
}

/// `DAEMON_007`: Neben der Kommandozeile liegt kein brauchbares `humanitld`.
fn missing_daemon(path: &Path, why: &str) -> Diagnostic {
    Diagnostic::builder(DAEMON_007, Severity::Blocking)
        .why(format!(
            "the unit would start {}, but {why}; ExecStart names the daemon next to the running \
             humanitl and never one from PATH",
            path.display()
        ))
        .fix(command_or_docs(&["ls", "-l", &path.to_string_lossy()]))
        .build()
}

/// Ein Befehl, wenn er beweisbar dieselben Wörter bleibt, sonst die Erklärung.
///
/// Dieselbe Regel wie in `humanitl_sandbox::doctor::shell_command`, und mit
/// Absicht dieselbe Funktion: Jeder dieser Pfade kommt aus der Umgebung, und
/// ein Pfad, der durch Interpolation zu einem zweiten Befehl wird, ist im
/// Projekt schon dreimal vorgekommen (HUM-043, HUM-106, HUM-075).
fn command_or_docs(words: &[&str]) -> FixAction {
    humanitl_sandbox::doctor::command_fix(words)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};

    use humanitl_config::{Env, Paths};

    use super::{
        DAEMON_NAME, Enablement, MARKER, PLACEHOLDER, TEMPLATE, UNIT_NAME, Written, carries_marker,
        daemon_binary, exec_start_word, prepare, render, rollback, unit_dir, write,
    };

    /// Die Zeilen der Vorlage ohne Kommentare und Leerzeilen.
    fn directives(text: &str) -> impl Iterator<Item = &str> {
        text.lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
    }

    fn paths(config_home: &Path) -> Paths {
        Paths::new(
            Env::from_pairs([
                ("HOME", "/home/tester"),
                ("XDG_CONFIG_HOME", &config_home.to_string_lossy()),
            ])
            .with_uid(1000),
        )
    }

    #[test]
    fn the_template_carries_the_marker_and_exactly_one_placeholder() {
        assert!(carries_marker(TEMPLATE), "the first line is the marker");
        assert_eq!(
            TEMPLATE.matches(PLACEHOLDER).count(),
            1,
            "one ExecStart, one placeholder"
        );
        assert!(
            TEMPLATE.contains("WantedBy=default.target"),
            "a user unit that nothing wants never starts"
        );
        // Nur die Anweisungen, nicht die Kommentare: Dort steht mit Absicht,
        // **warum** es keine Socket-Unit und kein `ProtectHome` gibt, und ein
        // Test, der das Wort verboete, verboete die Begruendung.
        for line in directives(TEMPLATE) {
            assert!(
                !line.contains("humanitld.socket"),
                "no socket activation: the daemon refuses a socket somebody else holds: {line}"
            );
            assert!(
                !line.contains("ProtectHome"),
                "the project directory lies under $HOME and is mounted writable: {line}"
            );
            assert!(!line.contains("sudo"), "a directive calls sudo: {line}");
        }
    }

    /// Die Zeile, die am 2026-09-06 gemessen wurde: Ohne `AF_NETLINK` bringt
    /// bubblewrap `lo` im Namensraum nicht hoch, und ohne `lo` gibt es keine
    /// Brücke vom Shim zum Proxy.
    #[test]
    fn the_hardening_lets_bubblewrap_work() {
        assert!(
            TEMPLATE.contains("RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6 AF_NETLINK"),
            "bwrap needs AF_NETLINK for the loopback interface"
        );
        for path in [
            "-%h/.local/share/humanitl",
            "-%h/.config/humanitl",
            "-%t/humanitl",
        ] {
            assert!(
                TEMPLATE.contains(path),
                "{path} needs the leading dash: systemd refuses to start a unit whose \
                 ReadWritePaths does not exist, and on a fresh install none of the three does"
            );
        }
    }

    /// Der Wert der Zeile `SystemCallFilter=` der Vorlage, in seine Wörter
    /// zerlegt.
    ///
    /// Gibt es die Zeile nicht, ist das Ergebnis leer, und der Test darüber
    /// schlägt fehl, statt still nichts zu prüfen.
    fn system_call_filter(template: &str) -> Vec<String> {
        directives(template)
            .find_map(|line| line.strip_prefix("SystemCallFilter="))
            .into_iter()
            .flat_map(str::split_whitespace)
            .map(str::to_owned)
            .collect()
    }

    /// Jeder Syscall, den die genannten Gruppen und Namen erlauben.
    ///
    /// Fragt `systemd-analyze syscall-filter` und folgt den Untergruppen, die
    /// dessen Ausgabe nennt, bis nichts Neues mehr dazukommt: `@system-service`
    /// listet sechzehn weitere Gruppen und erst darunter seine eigenen
    /// Syscalls, und ohne diesen zweiten Schritt sähe der Test nur die halbe
    /// Menge. `None`, wenn es `systemd-analyze` auf dieser Maschine nicht gibt
    /// oder es den Dienst verweigert; dann ist nichts gemessen.
    fn allowed_syscalls(words: &[String]) -> Option<BTreeSet<String>> {
        let mut asked: BTreeSet<String> = BTreeSet::new();
        let mut syscalls: BTreeSet<String> = BTreeSet::new();
        let mut open: Vec<String> = Vec::new();
        for word in words {
            if word.starts_with('@') {
                open.push(word.clone());
            } else {
                syscalls.insert(word.clone());
            }
        }
        while let Some(group) = open.pop() {
            if !asked.insert(group.clone()) {
                continue;
            }
            let output = std::process::Command::new("systemd-analyze")
                .args(["syscall-filter", &group])
                .output()
                .ok()?;
            if !output.status.success() {
                return None;
            }
            for line in String::from_utf8_lossy(&output.stdout).lines() {
                let name = line.trim();
                if name.is_empty() || name.starts_with('#') || name == group {
                    continue;
                }
                if name.starts_with('@') {
                    open.push(name.to_owned());
                } else {
                    syscalls.insert(name.to_owned());
                }
            }
        }
        Some(syscalls)
    }

    /// `seccomp(2)` muss durch den Filter der Unit kommen.
    ///
    /// Der Filter einer Unit wird an jedes Kind vererbt, also auch an
    /// `humanitl-shim` in der Sandbox, und der Shim stellt die dritte
    /// Sandbox-Garantie mit genau diesem Aufruf her
    /// (`humanitl_shim::seccomp::apply` über `seccompiler`). Fehlt er in den
    /// Gruppen der Zeile, stirbt der Shim an `SIGSYS`, bevor er seinen Filter
    /// setzen kann.
    ///
    /// Gemessen und nicht behauptet: Der Test fragt `systemd-analyze` nach dem
    /// Inhalt der Gruppen, statt die Zeile mit sich selbst zu vergleichen. Ein
    /// Test, der nur nach dem Wort `@sandbox` suchte, hielte auch dann, wenn
    /// systemd die Gruppe umbaute — und er sagte nichts darüber, ob der Aufruf
    /// wirklich durchkommt.
    ///
    /// Auf systemd 262 kommt `seccomp` auch ohne das Wort `@sandbox` durch,
    /// weil `@system-service` die Gruppe `@default` enthält und `@default`
    /// ihrerseits `@sandbox`. Das steht in keiner Zeile von `systemd.exec(5)`,
    /// deshalb nennt die Vorlage `@sandbox` selbst und dieser Test misst die
    /// Erreichbarkeit statt des Wortes: Er wird rot, sobald die Zeile keinen Weg
    /// mehr zu `seccomp` hat, gleich über welche Gruppe der Weg lief.
    #[test]
    fn the_filter_lets_the_shim_install_its_own_seccomp_filter() {
        let words = system_call_filter(TEMPLATE);
        assert!(
            !words.is_empty(),
            "the template has no SystemCallFilter= line to measure"
        );
        let Some(allowed) = allowed_syscalls(&words) else {
            eprintln!(
                "SKIP the_filter_lets_the_shim_install_its_own_seccomp_filter: no usable \
                 systemd-analyze on this machine, so nothing about SystemCallFilter was verified"
            );
            return;
        };
        assert!(
            allowed.contains("seccomp"),
            "SystemCallFilter={} reaches {} syscalls, and seccomp is not among them; the shim \
             installs its filter with seccomp(2) and would die of SIGSYS. @sandbox carries it.",
            words.join(" "),
            allowed.len()
        );
        // Die andere Hälfte der Messung: `@mount` steht in der Zeile, weil
        // bubblewrap mountet und `pivot_root`t.
        assert!(
            allowed.contains("pivot_root"),
            "SystemCallFilter={} does not reach pivot_root, and bubblewrap needs it",
            words.join(" ")
        );
    }

    #[test]
    fn the_rendered_unit_names_the_daemon_beside_the_cli() {
        let unit = render(Path::new("/opt/humanitl/bin/humanitld")).expect("a plain path");
        assert!(unit.contains("ExecStart=/opt/humanitl/bin/humanitld\n"));
        assert!(!unit.contains(PLACEHOLDER));
        assert!(carries_marker(&unit), "the marker survives rendering");
    }

    #[test]
    fn a_path_systemd_would_read_differently_yields_no_unit() {
        for hostile in [
            "/opt/hum anitl/humanitld",
            "/opt/%h/humanitld",
            "/opt/hum\"anitl/humanitld",
            "/opt/hum\\anitl/humanitld",
            "/opt/hum;anitl/humanitld",
            "relative/humanitld",
        ] {
            let error = render(Path::new(hostile)).expect_err("{hostile} is refused");
            assert_eq!(error.code.as_str(), "DAEMON_007", "{hostile}");
        }
    }

    /// Ein Pfad, der kein gültiges UTF-8 ist, ergibt keine Unit.
    ///
    /// `to_string_lossy` machte daraus einen **anderen** Pfad: Jedes ungültige
    /// Byte würde `U+FFFD`, und dieses Zeichen ist weder Leerraum noch
    /// Steuerzeichen noch eines der abgelehnten — die Prüfung ließe es also
    /// durch, und in `ExecStart` stünde ein Dateiname, den es so nicht gibt.
    #[test]
    fn a_path_that_is_not_valid_utf8_yields_no_unit() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt as _;

        let raw = OsStr::from_bytes(b"/opt/hum\xffanitl/humanitld");
        let error = exec_start_word(Path::new(raw)).expect_err("invalid UTF-8 is refused");
        assert_eq!(error.code.as_str(), "DAEMON_007");
        assert!(
            error.why.contains("not valid UTF-8"),
            "the reason names what is wrong: {}",
            error.why
        );
    }

    /// Ein symbolisch verlinktes `humanitld` landet als die Datei in der Unit,
    /// auf die es zeigt.
    ///
    /// Ohne die Auflösung stünde der Verweis in `ExecStart`, und wer ihn später
    /// umhängt, änderte damit, was beim nächsten Anmelden startet — dieselbe
    /// Gefahr wie ein `humanitld` aus `PATH`, die dieses Modul schon abwehrt.
    #[test]
    fn a_symlinked_daemon_reaches_exec_start_as_the_file_it_points_at() {
        use std::os::unix::fs::PermissionsExt as _;

        let home = tempfile::tempdir().expect("a temporary directory");
        let root = std::fs::canonicalize(home.path()).expect("a real path");
        let real = root.join("versions").join("0.1.0");
        std::fs::create_dir_all(&real).expect("the version directory");
        let target = real.join("humanitld");
        std::fs::write(&target, b"#!/bin/sh\nexit 0\n").expect("the real daemon");
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755)).expect("0755");

        let bin = root.join("bin");
        std::fs::create_dir_all(&bin).expect("the bin directory");
        let cli = bin.join("humanitl");
        std::fs::write(&cli, b"#!/bin/sh\nexit 0\n").expect("the command line");
        std::os::unix::fs::symlink(&target, bin.join(DAEMON_NAME)).expect("the link");

        let resolved = daemon_binary(&cli).expect("a symlinked daemon is resolved, not refused");
        assert_eq!(resolved, target, "ExecStart names the file, not the link");
        let unit = render(&resolved).expect("a unit");
        assert!(
            unit.contains(&format!("ExecStart={}\n", target.display())),
            "{unit}"
        );
    }

    /// Ein Verweis, der ins Leere zeigt, ergibt `DAEMON_007`.
    #[test]
    fn a_dangling_link_next_to_the_command_line_yields_no_unit() {
        let home = tempfile::tempdir().expect("a temporary directory");
        let bin = home.path().join("bin");
        std::fs::create_dir_all(&bin).expect("the bin directory");
        let cli = bin.join("humanitl");
        std::fs::write(&cli, b"#!/bin/sh\nexit 0\n").expect("the command line");
        std::os::unix::fs::symlink(home.path().join("gone"), bin.join(DAEMON_NAME))
            .expect("the dangling link");

        let error = daemon_binary(&cli).expect_err("a link into nothing is refused");
        assert_eq!(error.code.as_str(), "DAEMON_007");
    }

    /// Die Rücknahme entfernt die Verweise dieses Laufs und nur die.
    ///
    /// `systemctl --user enable` legt sie unter `<ziel>.wants/` an; misslingt
    /// danach der Start, muss `daemon install` sie wieder wegnehmen, sonst
    /// startet der Dienst beim nächsten Anmelden trotz `DAEMON_008`.
    #[test]
    fn the_enablement_rollback_removes_this_runs_links_and_keeps_the_older_ones() {
        let home = tempfile::tempdir().expect("a temporary directory");
        let dir = home.path().join("systemd").join("user");
        let older = dir.join("multi-user.target.wants");
        std::fs::create_dir_all(&older).expect("an older wants directory");
        let kept = older.join(UNIT_NAME);
        std::fs::write(&kept, "an enablement from before").expect("the older link");

        let before = Enablement::read(&dir);

        // Was ein `enable --now` anlegt, das danach am Start scheitert.
        let fresh = dir.join("default.target.wants");
        std::fs::create_dir_all(&fresh).expect("the fresh wants directory");
        let link = fresh.join(UNIT_NAME);
        std::fs::write(&link, "an enablement from this run").expect("the fresh link");

        before.rollback(&dir).expect("the rollback");

        assert!(!link.exists(), "the link of this run is taken back");
        assert!(!fresh.exists(), "and its empty directory with it");
        assert!(kept.exists(), "an enablement from before this run stays");
        assert!(older.is_dir(), "and so does its directory");
    }

    #[test]
    fn the_unit_lives_under_xdg_config_home_and_not_under_the_app_directory() {
        let dir = unit_dir(&paths(Path::new("/home/tester/.config")));
        assert_eq!(dir, PathBuf::from("/home/tester/.config/systemd/user"));
    }

    #[test]
    fn a_second_install_writes_nothing() {
        let home = tempfile::tempdir().expect("a temporary directory");
        let unit = home.path().join(UNIT_NAME);
        let contents = render(Path::new("/opt/humanitl/humanitld")).expect("a unit");

        let first = prepare(&unit, &contents).expect("nothing is there yet");
        assert_eq!(first, Written::Created);
        write(&unit, &contents, &first).expect("the first write");

        let second = prepare(&unit, &contents).expect("the same file");
        assert_eq!(second, Written::Unchanged);
        assert!(!second.changed(), "a second install changes nothing");
    }

    #[test]
    fn a_unit_of_somebody_else_is_refused_and_stays_untouched() {
        let home = tempfile::tempdir().expect("a temporary directory");
        let unit = home.path().join(UNIT_NAME);
        let theirs = "[Service]\nExecStart=/usr/local/bin/humanitld --fake\n";
        std::fs::write(&unit, theirs).expect("their unit");

        let error = prepare(&unit, "anything").expect_err("a foreign file is not replaced");
        assert_eq!(error.code.as_str(), "DAEMON_005");
        assert_eq!(
            std::fs::read_to_string(&unit).expect("still there"),
            theirs,
            "the file of somebody else is not touched"
        );
    }

    #[test]
    fn a_file_that_carries_the_marker_is_replaced_and_can_be_taken_back() {
        let home = tempfile::tempdir().expect("a temporary directory");
        let unit = home.path().join(UNIT_NAME);
        let old = format!("{MARKER}\n[Service]\nExecStart=/old/humanitld\n");
        std::fs::write(&unit, &old).expect("our older unit");

        let contents = render(Path::new("/opt/humanitl/humanitld")).expect("a unit");
        let plan = prepare(&unit, &contents).expect("ours");
        assert_eq!(
            plan,
            Written::Replaced {
                previous: old.clone()
            }
        );
        write(&unit, &contents, &plan).expect("the write");
        assert_eq!(std::fs::read_to_string(&unit).expect("written"), contents);

        rollback(&unit, &plan).expect("the rollback");
        assert_eq!(
            std::fs::read_to_string(&unit).expect("restored"),
            old,
            "a failed install leaves the version from before"
        );
    }

    #[test]
    fn a_failed_install_leaves_nothing_behind() {
        let home = tempfile::tempdir().expect("a temporary directory");
        let unit = home.path().join("systemd").join("user").join(UNIT_NAME);
        let contents = render(Path::new("/opt/humanitl/humanitld")).expect("a unit");

        let plan = prepare(&unit, &contents).expect("nothing is there yet");
        write(&unit, &contents, &plan).expect("the write");
        assert!(unit.is_file());

        rollback(&unit, &plan).expect("the rollback");
        assert!(!unit.exists(), "what nobody asked for does not stay");
        let leftovers: Vec<PathBuf> = std::fs::read_dir(unit.parent().expect("the directory"))
            .expect("readable")
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .collect();
        assert!(leftovers.is_empty(), "no scratch file stays: {leftovers:?}");
    }

    #[test]
    fn the_mode_of_the_written_unit_is_0644() {
        use std::os::unix::fs::PermissionsExt as _;

        let home = tempfile::tempdir().expect("a temporary directory");
        let unit = home.path().join(UNIT_NAME);
        let contents = render(Path::new("/opt/humanitl/humanitld")).expect("a unit");
        let plan = prepare(&unit, &contents).expect("nothing is there yet");
        write(&unit, &contents, &plan).expect("the write");

        let mode = std::fs::metadata(&unit)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o644, "systemd reads it, nobody else writes it");
    }
}
