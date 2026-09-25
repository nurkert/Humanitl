//! Die Nutzer-Unit des Daemons: rendern, prüfen, schreiben, zurücknehmen
//! (HUM-044).
//!
//! `humanitl daemon install` ist das Eingriffsreichste, was dieses Produkt
//! außerhalb der Sandbox tut: Es legt eine Datei auf dem Rechner eines
//! Menschen ab, die von da an bei jeder Anmeldung einen Dienst startet. Dieses
//! Modul hält die vier Zusagen, die daran hängen, und hält sie an einer
//! Stelle, damit sie prüfbar sind, ohne dass ein Test systemd braucht:
//!
//! 1. **Höchstens eine Datei, an einem genannten Ort.**
//!    `$XDG_CONFIG_HOME/systemd/user/humanitld.service`, sonst
//!    `~/.config/systemd/user/humanitld.service`. Keine System-Unit, kein
//!    `sudo`, keine zweite Datei, kein Socket. Hat das Paket die Units schon
//!    unter [`SYSTEM_UNIT_DIR`] abgelegt, schreibt der Befehl **gar nichts**
//!    und aktiviert nur, was dort liegt ([`SystemUnits`], HUM-053): Eine
//!    Kopie unter `~/.config` verdeckte die Fassung des Pakets, und jedes
//!    Update des Pakets liefe an ihr vorbei. Eine solche Kopie aus einer
//!    früheren Installation legt `daemon install` deshalb beiseite, eine
//!    fremde lässt es liegen (`cmd::daemon::packaged`, HUM-211).
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

/// Der Name der Socket-Unit, die das Paket neben die Dienst-Unit legt.
pub const SOCKET_NAME: &str = "humanitld.socket";

/// Wo das Paket die Units ablegt (HUM-053, `packaging/deb/build-deb.sh`).
///
/// Das ist der Ort, den systemd für Nutzer-Units aus Paketen vorsieht
/// (`systemd.unit(5)`, „User Unit Search Path"). `/etc/systemd/user` gehört
/// dem Verwalter des Rechners und `/usr/local` keinem Paket; beide sind kein
/// Hinweis auf eine Installation durch das Paket.
pub const SYSTEM_UNIT_DIR: &str = "/usr/lib/systemd/user";

/// Die Variable, die [`SYSTEM_UNIT_DIR`] ersetzt (HUM-077).
///
/// Für Tests: Auf einem Rechner mit installiertem Paket liefen die Tests von
/// `daemon install` und `daemon uninstall` sonst den Weg des Pakets. Ein
/// einfacher Unterstrich, also kein Konfigurationsschlüssel; der Lader
/// übergeht sie (`humanitl_config::load`). Sie ändert nur, wo die
/// Kommandozeile nach den Units des Pakets sucht, nicht, was systemd lädt.
pub const SYSTEM_UNIT_DIR_ENV: &str = "HUMANITL_SYSTEM_UNIT_DIR";

/// Wo die Units des Pakets gesucht werden: [`SYSTEM_UNIT_DIR`], außer
/// [`SYSTEM_UNIT_DIR_ENV`] nennt ein anderes Verzeichnis.
#[must_use]
pub fn system_unit_dir(env: &humanitl_config::Env) -> PathBuf {
    env.non_empty(SYSTEM_UNIT_DIR_ENV)
        .map_or_else(|| PathBuf::from(SYSTEM_UNIT_DIR), PathBuf::from)
}

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
    daemon_binary_in(dir)
}

/// Der Daemon in diesem Verzeichnis, aufgelöst bis zur wirklichen Datei.
///
/// Der Regelfall ist das Verzeichnis der laufenden Kommandozeile
/// ([`daemon_binary`]); `--bin-dir` nennt ein anderes, etwa das Bundle eines
/// Pakets, das die Kommandozeile über einen Verweis erreicht hat.
///
/// # Errors
///
/// `DAEMON_007`, wie bei [`daemon_binary`].
pub fn daemon_binary_in(dir: &Path) -> Result<PathBuf, Diagnostic> {
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

/// `DAEMON_007` für ein Binary, das nicht dort liegt, wo es liegen müsste.
///
/// Derselbe Befund wie für ein fehlendes `humanitld`, nur mit dem Pfad, den
/// der Aufrufer nennt: `daemon install` aus einem `AppImage` sucht auch den Shim
/// (HUM-070).
#[must_use]
pub fn missing_binary(path: &Path, why: &str) -> Diagnostic {
    missing_daemon(path, why)
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
/// [`Enablement::read_for`] nimmt den Zustand **vor** dem ersten `systemctl`-Aufruf
/// auf, [`Enablement::rollback`] entfernt danach genau das, was seither
/// dazugekommen ist. Was schon vorher dalag, bleibt liegen: `daemon install`
/// nimmt nur zurück, was dieser Lauf angerichtet hat.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Enablement {
    /// Die Namen der Units, deren Verweise zählen.
    names: Vec<String>,
    /// Die `*.wants`- und `*.requires`-Verzeichnisse, die es schon gab.
    dirs: BTreeSet<PathBuf>,
    /// Die Verweise auf eine der Units darin, die es schon gab.
    links: BTreeSet<PathBuf>,
}

impl Enablement {
    /// Liest den Zustand der Aktivierung der genannten Units unter `dir`.
    ///
    /// Ein Verzeichnis, das sich nicht lesen lässt, ergibt einen leeren
    /// Zustand: Es gibt dann nichts, was dieser Lauf später als „schon vorher
    /// da" verschonen müsste, und die Rücknahme entfernt lieber einen Verweis
    /// zu viel als einen zu wenig.
    #[must_use]
    pub fn read_for(dir: &Path, names: &[&str]) -> Self {
        let mut state = Self {
            names: names.iter().map(|name| (*name).to_owned()).collect(),
            ..Self::default()
        };
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
            for unit in &state.names {
                let link = path.join(unit);
                if link.symlink_metadata().is_ok() {
                    state.links.insert(link);
                }
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
    /// **Nur Verweise, die dieser Lauf angelegt hat** (HUM-211, Review): neu
    /// seit dem Zustand und in diesem Augenblick noch ein Verweis auf die Unit
    /// dieses Namens neben `unit`, also dorthin, wohin `systemctl --user
    /// enable` ihn gelegt hat ([`points_at`]: auch über einen Verweis im Pfad
    /// oder ein relatives Ziel). Alles andere unter demselben Namen bleibt
    /// liegen ([`remove_link_if`]) und steht im Befund.
    ///
    /// # Errors
    ///
    /// `DAEMON_006`, wenn sich ein neuer Verweis nicht entfernen lässt oder
    /// liegen blieb, weil er nicht auf die Unit zeigt. Der Befund nennt alle,
    /// die stehen blieben; ein halb zurückgenommener Zustand wird gemeldet und
    /// nicht verschwiegen.
    pub fn rollback(&self, dir: &Path, unit: &Path) -> Result<(), Diagnostic> {
        let names: Vec<&str> = self.names.iter().map(String::as_str).collect();
        let now = Self::read_for(dir, &names);
        let mut left: Vec<String> = Vec::new();
        for link in now.links.difference(&self.links) {
            let Some(name) = link.file_name() else {
                continue;
            };
            let expected = unit.with_file_name(name);
            match remove_link_if(link, |target| points_at(link, target, &expected)) {
                // Gegangen, oder inzwischen ohnehin fort.
                Ok(removed) if removed || link.symlink_metadata().is_err() => {}
                Ok(_) => left.push(format!(
                    "{} (left alone: it does not point at {})",
                    link.display(),
                    expected.display()
                )),
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

/// Benennt `from` in `to` um, ohne etwas zu überschreiben, das unter `to`
/// liegt (`renameat2` mit `RENAME_NOREPLACE`, HUM-211).
///
/// # Errors
///
/// Der Fehler des Aufrufs; `AlreadyExists`, wenn unter `to` etwas liegt.
pub fn move_no_replace(from: &Path, to: &Path) -> std::io::Result<()> {
    rustix::fs::renameat_with(
        rustix::fs::CWD,
        from,
        rustix::fs::CWD,
        to,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .map_err(std::io::Error::from)
}

/// Wohin ein Verweis zeigt, als Pfad neben dem Verweis aufgelöst.
#[must_use]
pub fn link_target(link: &Path, target: &Path) -> PathBuf {
    if target.is_absolute() {
        target.to_path_buf()
    } else {
        link.parent()
            .map_or_else(|| target.to_path_buf(), |dir| dir.join(target))
    }
}

/// Wahr, wenn der Verweis `link` mit dem Ziel `target` auf `expected` zeigt:
/// wörtlich, oder nach Auflösen beider Pfade.
///
/// systemd löst seine Suchpfade auf, bevor es Verweise schreibt: Ist
/// `~/.config` selbst ein Verweis (ein Dotfile-Verwalter), steht im Verweis der
/// aufgelöste Pfad, und ein relatives Ziel (`../humanitld.service`) gilt
/// neben dem Verweis. Beide sind Verweise dieses Laufs.
#[must_use]
pub fn points_at(link: &Path, target: &Path, expected: &Path) -> bool {
    let resolved = link_target(link, target);
    if resolved == expected {
        return true;
    }
    match (
        std::fs::canonicalize(&resolved),
        std::fs::canonicalize(expected),
    ) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

/// Ein Name für einen Verweis, den [`remove_link_if`] kurz beiseitezieht:
/// versteckt, und je Aufruf ein anderer, auch innerhalb eines Prozesses.
fn held_name(name: &std::ffi::OsStr) -> std::ffi::OsString {
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.subsec_nanos());
    let mut held = std::ffi::OsString::from(".");
    held.push(name);
    held.push(format!(
        ".humanitl-{}-{}-{nanos}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    held
}

/// Entfernt `link`, aber nur, wenn es ein Verweis ist, dessen Ziel `ours`
/// annimmt; `Ok(true)`, wenn er ging, `Ok(false)`, wenn er blieb oder fehlte.
///
/// Geprüft wird nicht am Namen, sondern an dem, was entfernt würde: Der
/// Verweis wird erst mit `renameat2(RENAME_NOREPLACE)` auf einen eigenen Namen
/// im selben Verzeichnis gezogen, dort gelesen und nur dann gelöscht. Legt
/// jemand zwischen Prüfung und Löschen etwas anderes unter den Namen, trifft
/// das Löschen es nicht. Gehört das Gezogene nicht diesem Lauf, geht es auf
/// demselben Weg zurück (HUM-211, Review).
///
/// # Errors
///
/// Der Fehler beim Umbenennen oder Löschen, mit dem Zwischennamen, wenn er
/// daran hing. Scheitert das Löschen, geht der Verweis unter seinen Namen
/// zurück; scheitert auch das, nennt der Fehler, unter welchem Namen er jetzt
/// liegt.
pub fn remove_link_if(link: &Path, ours: impl Fn(&Path) -> bool) -> std::io::Result<bool> {
    let Some(name) = link.file_name() else {
        return Ok(false);
    };
    let held = link.with_file_name(held_name(name));
    match move_no_replace(link, &held) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(std::io::Error::new(
                error.kind(),
                format!("it cannot be moved to {} ({error})", held.display()),
            ));
        }
    }
    let put_back = |why: String| {
        move_no_replace(&held, link).map_or_else(
            |error| {
                std::io::Error::new(
                    error.kind(),
                    format!("{why}; it now lies at {} ({error})", held.display()),
                )
            },
            |()| std::io::Error::other(why.clone()),
        )
    };
    if std::fs::read_link(&held).is_ok_and(|target| ours(&target)) {
        return match std::fs::remove_file(&held) {
            Ok(()) => Ok(true),
            Err(error) => Err(put_back(format!("it cannot be removed ({error})"))),
        };
    }
    match move_no_replace(&held, link) {
        Ok(()) => Ok(false),
        Err(error) => Err(std::io::Error::new(
            error.kind(),
            format!("it now lies at {} ({error})", held.display()),
        )),
    }
}

/// Jeder Verweis der Aktivierung der genannten Units unter `dir`
/// (`<ziel>.wants/<name>`, `<ziel>.requires/<name>`).
///
/// Für `daemon uninstall` (HUM-077): Ohne `systemctl` entfernt der Befehl sie
/// selbst, und danach prüft er, dass keiner stehen blieb. Derselbe Leser wie
/// [`Enablement::read_for`], damit Anlegen und Entfernen dieselben Verweise
/// meinen.
#[must_use]
pub fn enablement_links(dir: &Path, names: &[&str]) -> Vec<PathBuf> {
    Enablement::read_for(dir, names).links.into_iter().collect()
}

/// Die Units, die das Paket unter [`SYSTEM_UNIT_DIR`] abgelegt hat.
///
/// Liegen sie da, gibt es für `daemon install` nichts zu schreiben: Die
/// Dienst-Unit trägt den Pfad des Pakets, und eine Kopie unter `~/.config`
/// verdeckte sie (`systemd.unit(5)`: eine Nutzer-Unit gleichen Namens gewinnt).
/// Übrig bleibt das Aktivieren, und das tut jeder Mensch für sich selbst: Das
/// Paket läuft als root und kann `systemctl --user` nicht rufen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemUnits {
    /// Die Dienst-Unit des Pakets.
    pub service: PathBuf,
    /// Die Socket-Unit daneben, wenn es sie gibt. Pakete vor HUM-053 brachten
    /// keine mit.
    pub socket: Option<PathBuf>,
    /// Der Text der Dienst-Unit, für die Ankündigung.
    pub text: String,
}

impl SystemUnits {
    /// Die Units, die das Paket unter `dir` abgelegt hat; `None`, wenn dort
    /// keine lesbare `humanitld.service` liegt.
    #[must_use]
    pub fn find(dir: &Path) -> Option<Self> {
        let service = dir.join(UNIT_NAME);
        let text = std::fs::read_to_string(&service).ok()?;
        let socket = dir.join(SOCKET_NAME);
        Some(Self {
            service,
            socket: socket.is_file().then_some(socket),
            text,
        })
    }

    /// Die Namen, die `systemctl --user enable --now` bekommt: der Socket,
    /// und nur ohne Socket der Dienst (HUM-164).
    ///
    /// **Nur der Socket.** Ein Client, der kein Token findet, öffnet den
    /// Socket trotzdem einmal und wartet kurz auf das Token
    /// (`humanitl_ipc::client::token_or_wake`); diese erste Verbindung startet
    /// den Dienst. Er muss deshalb nicht mit jeder Sitzung starten. Der Socket
    /// hält den Pfad über jeden Neustart des Dienstes hinweg, und wer in dieser
    /// Zeit verbindet, wartet, statt abzuprallen. Pakete vor HUM-053 bringen
    /// keinen Socket mit; dort bleibt es der Dienst.
    #[must_use]
    pub fn names(&self) -> Vec<&'static str> {
        if self.socket.is_some() {
            vec![SOCKET_NAME]
        } else {
            vec![UNIT_NAME]
        }
    }

    /// Jede Unit des Pakets, die ein früherer Lauf aktiviert haben kann: erst
    /// der Socket, dann der Dienst.
    ///
    /// Für `daemon uninstall`: Vor HUM-164 aktivierte `daemon install` beide,
    /// und ein Verweis auf den Dienst aus dieser Zeit liegt womöglich noch da.
    #[must_use]
    pub fn all_names(&self) -> Vec<&'static str> {
        let mut names = Vec::with_capacity(2);
        if self.socket.is_some() {
            names.push(SOCKET_NAME);
        }
        names.push(UNIT_NAME);
        names
    }

    /// Das Programm, das die Dienst-Unit startet, wie es in `ExecStart` steht.
    #[must_use]
    pub fn exec_start(&self) -> PathBuf {
        self.text
            .lines()
            .map(str::trim)
            .find_map(|line| line.strip_prefix("ExecStart="))
            .and_then(|value| value.split_whitespace().next())
            .map_or_else(|| self.service.clone(), PathBuf::from)
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
        DAEMON_NAME, Enablement, MARKER, PLACEHOLDER, SOCKET_NAME, SystemUnits, TEMPLATE,
        UNIT_NAME, Written, carries_marker, daemon_binary, exec_start_word, prepare, render,
        rollback, unit_dir, write,
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
        // Jede dieser Zeilen legt, gemessen am 2026-09-18 mit `systemd-run
        // --user`, das Sandbox-Backend oder den Agenten still
        // (`docs/INSTALL.md#hardening`). Geprüft werden nur Anweisungen; in
        // den Kommentaren stehen die Namen mit Absicht, samt Grund.
        for (refused, why) in [
            (
                "RestrictNamespaces",
                "the sandbox needs user, mnt, pid, net, ipc and uts namespaces",
            ),
            (
                "ProtectKernelTunables",
                "a masked /proc/sys stops the sandbox from mounting its own /proc",
            ),
            (
                "ProtectKernelLogs",
                "a masked /proc/kmsg stops the sandbox from mounting its own /proc",
            ),
            (
                "ProtectHostname",
                "together with the rest it stops the sandbox from mounting its own /proc",
            ),
            (
                "RestrictSUIDSGID",
                "the sandbox fails with \"Can't open source /usr\"",
            ),
            (
                "MemoryDenyWriteExecute",
                "the filter is inherited, and node dies in the sandbox",
            ),
            (
                "UMask",
                "inherited: every file the agent writes would change its mode",
            ),
            (
                "SystemCallArchitectures",
                "the shim probes its filter with an x32 call and dies of SIGSYS",
            ),
            (
                "PrivateDevices",
                "without /dev/ptmx the daemon opens no terminal for the agent (TERM_002)",
            ),
        ] {
            assert!(
                !directives(TEMPLATE).any(|line| line.starts_with(refused)),
                "{refused} is set, but {why}"
            );
        }
        assert!(
            !directives(TEMPLATE).any(|line| line == "CapabilityBoundingSet="),
            "an empty bounding set stops the sandbox from starting"
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
        // Gemessen am 2026-09-18 mit den Escape-Tests unter der Unit (HUM-053):
        // bubblewrap benennt den UTS-Namensraum der Sandbox und scheitert ohne
        // `sethostname` mit „Can't set hostname to sandbox".
        assert!(
            allowed.contains("sethostname"),
            "SystemCallFilter={} does not reach sethostname, and bubblewrap names the sandbox",
            words.join(" ")
        );
    }

    /// `Type=notify` und der Daemon, der `READY=1` schickt, gehören zusammen:
    /// Ohne die Zeile wartet systemd nicht auf die Meldung, und eine Unit, die
    /// nach dem Dienst startet, fände ihn womöglich ohne Socket.
    #[test]
    fn the_unit_waits_for_the_ready_notification() {
        let types: Vec<&str> = directives(TEMPLATE)
            .filter_map(|line| line.strip_prefix("Type="))
            .collect();
        assert_eq!(types, ["notify"], "one Type=, and it is notify");
    }

    /// Die Zahl, die `systemd-analyze security` für die gerenderte Unit nennt.
    ///
    /// `None`, wenn es `systemd-analyze` hier nicht gibt oder es seine
    /// Suchpfade nicht anlegen kann (etwa in einer Umgebung mit
    /// schreibgeschütztem Dateisystem); dann ist nichts gemessen.
    fn exposure(unit: &str) -> Option<f64> {
        let dir = tempfile::tempdir().ok()?;
        let path = dir.path().join(UNIT_NAME);
        std::fs::write(&path, unit).ok()?;
        let output = std::process::Command::new("systemd-analyze")
            .args(["--user", "security", "--offline=true"])
            .arg(&path)
            .env("XDG_RUNTIME_DIR", dir.path())
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&output.stdout);
        let line = text
            .lines()
            .find(|line| line.contains("Overall exposure level"))?;
        let value = line.rsplit(':').next()?.split_whitespace().next()?;
        value.parse().ok()
    }

    /// Der Wert aus `systemd-analyze security` bleibt bei 4.0 oder darunter
    /// (HUM-053, Test „Exposure ≤ 4.0"), und `docs/INSTALL.md` nennt einen
    /// Wert, der das auch tut.
    ///
    /// Gemessen und nicht behauptet: Eine Zeile weniger in der Härtung hebt
    /// die Zahl, und der Test wird rot. Die Messung braucht `systemd-analyze`
    /// mit `--offline`; wo es fehlt, sagt der Test das und prüft nur die
    /// Dokumentation.
    #[test]
    fn the_exposure_stays_at_or_below_the_documented_value() {
        let docs = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../docs/INSTALL.md"),
        )
        .expect("docs/INSTALL.md is in the repository");
        let documented: f64 = docs
            .lines()
            .find_map(|line| line.strip_prefix("Overall exposure level: "))
            .and_then(|rest| rest.split_whitespace().next())
            .and_then(|value| value.parse().ok())
            .expect("docs/INSTALL.md names the exposure as 'Overall exposure level: N.N'");
        assert!(
            documented <= 4.0,
            "docs/INSTALL.md documents {documented}, above the 4.0 of HUM-053"
        );

        let unit = render(Path::new("/usr/lib/humanitl/bin/humanitld")).expect("a unit");
        let Some(measured) = exposure(&unit) else {
            eprintln!(
                "SKIP the_exposure_stays_at_or_below_the_documented_value: no usable \
                 systemd-analyze security --offline here, so the exposure was not measured"
            );
            return;
        };
        assert!(
            measured <= 4.0,
            "systemd-analyze security rates the unit {measured}, above the 4.0 of HUM-053"
        );
    }

    /// Das Paket legt die Units ab; `daemon install` findet beide und
    /// aktiviert nur den Socket, der den Dienst beim ersten Client startet
    /// (HUM-164). `daemon uninstall` meldet beide ab, erst den Socket.
    #[test]
    fn packaged_units_are_found_and_only_the_socket_is_enabled() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        assert_eq!(SystemUnits::find(dir.path()), None, "nothing installed");

        let unit = render(Path::new("/usr/lib/humanitl/bin/humanitld")).expect("a unit");
        std::fs::write(dir.path().join(UNIT_NAME), &unit).expect("the service");
        let only_service = SystemUnits::find(dir.path()).expect("the service alone counts");
        assert_eq!(
            only_service.names(),
            [UNIT_NAME],
            "a package before HUM-053"
        );
        assert_eq!(only_service.all_names(), [UNIT_NAME]);

        std::fs::write(dir.path().join(SOCKET_NAME), "[Socket]\n").expect("the socket");
        let both = SystemUnits::find(dir.path()).expect("both units");
        assert_eq!(both.names(), [SOCKET_NAME], "the socket starts the service");
        assert_eq!(both.all_names(), [SOCKET_NAME, UNIT_NAME]);
        assert_eq!(
            both.exec_start(),
            PathBuf::from("/usr/lib/humanitl/bin/humanitld")
        );
    }

    /// Die Rücknahme kennt jede genannte Unit, nicht nur den Dienst: Ein
    /// gescheitertes `enable --now humanitld.socket humanitld.service` darf
    /// keinen Verweis auf den Socket zurücklassen.
    #[test]
    fn the_rollback_takes_back_the_socket_link_as_well() {
        let home = tempfile::tempdir().expect("a temporary directory");
        let dir = home.path().join("systemd").join("user");
        std::fs::create_dir_all(&dir).expect("the unit directory");
        let before = Enablement::read_for(&dir, &[SOCKET_NAME, UNIT_NAME]);

        let sockets = dir.join("sockets.target.wants");
        std::fs::create_dir_all(&sockets).expect("the sockets directory");
        std::os::unix::fs::symlink(dir.join(SOCKET_NAME), sockets.join(SOCKET_NAME))
            .expect("the socket link");
        let default = dir.join("default.target.wants");
        std::fs::create_dir_all(&default).expect("the default directory");
        std::os::unix::fs::symlink(dir.join(UNIT_NAME), default.join(UNIT_NAME))
            .expect("the service link");

        before
            .rollback(&dir, &dir.join(UNIT_NAME))
            .expect("the rollback");
        assert!(!sockets.exists(), "the socket link and its directory go");
        assert!(!default.exists(), "the service link and its directory go");
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

        let before = Enablement::read_for(&dir, &[UNIT_NAME]);

        // Was ein `enable --now` anlegt, das danach am Start scheitert.
        let fresh = dir.join("default.target.wants");
        std::fs::create_dir_all(&fresh).expect("the fresh wants directory");
        let link = fresh.join(UNIT_NAME);
        std::os::unix::fs::symlink(dir.join(UNIT_NAME), &link).expect("the fresh link");

        before
            .rollback(&dir, &dir.join(UNIT_NAME))
            .expect("the rollback");

        assert!(!link.exists(), "the link of this run is taken back");
        assert!(!fresh.exists(), "and its empty directory with it");
        assert!(kept.exists(), "an enablement from before this run stays");
        assert!(older.is_dir(), "and so does its directory");
    }

    /// Die Rücknahme entfernt nur, was `enable` für diese Unit angelegt hat:
    /// Ein neuer Eintrag unter demselben Namen, der woandershin zeigt oder
    /// gar kein Verweis ist, bleibt liegen, samt seinem Verzeichnis (HUM-211,
    /// Review).
    #[test]
    fn the_rollback_leaves_what_does_not_point_at_the_unit() {
        let home = tempfile::tempdir().expect("a temporary directory");
        let dir = home.path().join("systemd").join("user");
        std::fs::create_dir_all(&dir).expect("the unit directory");
        let before = Enablement::read_for(&dir, &[SOCKET_NAME, UNIT_NAME]);

        let default = dir.join("default.target.wants");
        std::fs::create_dir_all(&default).expect("the default directory");
        let elsewhere = home.path().join("elsewhere.service");
        std::os::unix::fs::symlink(&elsewhere, default.join(UNIT_NAME)).expect("a foreign link");
        let sockets = dir.join("sockets.target.wants");
        std::fs::create_dir_all(&sockets).expect("the sockets directory");
        std::fs::write(sockets.join(SOCKET_NAME), "a file").expect("a regular file");

        let error = before
            .rollback(&dir, &dir.join(UNIT_NAME))
            .expect_err("a new link left alone is reported");
        assert_eq!(error.code.as_str(), "DAEMON_006", "{}", error.why);
        assert!(error.why.contains("left alone"), "{}", error.why);

        assert_eq!(
            std::fs::read_link(default.join(UNIT_NAME)).expect("the foreign link stays"),
            elsewhere
        );
        assert_eq!(
            std::fs::read_to_string(sockets.join(SOCKET_NAME)).expect("the file stays"),
            "a file"
        );
        let names: Vec<_> = std::fs::read_dir(&default)
            .expect("the directory stays")
            .map(|entry| entry.expect("an entry").file_name())
            .collect();
        assert_eq!(names.len(), 1, "nothing held back is left: {names:?}");
    }

    /// systemd schreibt den aufgelösten Pfad, wenn `~/.config` ein Verweis
    /// ist, und ein relatives Ziel gilt neben dem Verweis: Beides sind
    /// Verweise dieses Laufs, und die Rücknahme nimmt sie weg (HUM-211,
    /// Review).
    #[test]
    fn the_rollback_takes_back_resolved_and_relative_links() {
        let home = tempfile::tempdir().expect("a temporary directory");
        let real = home.path().join("dotfiles");
        std::fs::create_dir_all(real.join("systemd").join("user")).expect("the real directory");
        let config = home.path().join("config");
        std::os::unix::fs::symlink(&real, &config).expect("~/.config is a link");
        let dir = config.join("systemd").join("user");
        std::fs::write(dir.join(UNIT_NAME), "[Service]\n").expect("the unit");
        std::fs::write(dir.join(SOCKET_NAME), "[Socket]\n").expect("the socket");
        let before = Enablement::read_for(&dir, &[SOCKET_NAME, UNIT_NAME]);

        let default = dir.join("default.target.wants");
        std::fs::create_dir_all(&default).expect("the default directory");
        std::os::unix::fs::symlink(
            real.join("systemd").join("user").join(UNIT_NAME),
            default.join(UNIT_NAME),
        )
        .expect("a link to the resolved path");
        let sockets = dir.join("sockets.target.wants");
        std::fs::create_dir_all(&sockets).expect("the sockets directory");
        std::os::unix::fs::symlink(format!("../{SOCKET_NAME}"), sockets.join(SOCKET_NAME))
            .expect("a relative link");

        before
            .rollback(&dir, &dir.join(UNIT_NAME))
            .expect("the rollback");
        assert!(!default.exists(), "the resolved link and its directory go");
        assert!(!sockets.exists(), "the relative link and its directory go");
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
