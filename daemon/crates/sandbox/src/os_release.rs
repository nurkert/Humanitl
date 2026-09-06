//! Welcher Paketverwalter auf dieser Maschine `bubblewrap` nachinstalliert
//! (HUM-044).
//!
//! `SANDBOX_001` und `SANDBOX_002` tragen einen Befehl, den ein Mensch in
//! seine Shell einfügt. Auf Fedora, Arch und openSUSE war das bis hierher
//! `sudo apt install bubblewrap` — ein Befehl, der dort nicht existiert. Die
//! Distribution steht in `/etc/os-release`, und dieses Modul liest sie.
//!
//! # Die eine Regel, die dieses Modul trägt
//!
//! **Kein Zeichen aus `/etc/os-release` erscheint jemals im erzeugten
//! Befehl.** Die Datei liegt zwar unter `/etc` und gehört `root`, aber sie ist
//! trotzdem eine Eingabe von außen, und ihr Inhalt würde hier zu einem Befehl,
//! den jemand ausführt. Deshalb ist die Übersetzung eine Auswahl und keine
//! Zusammensetzung: `ID` und `ID_LIKE` wählen einen der vier Werte von
//! [`PackageManager`], und [`PackageManager::install_bubblewrap`] gibt eine
//! von vier festen Zeichenketten zurück, die im Binärprogramm stehen. Der
//! Aufzählungstyp trägt kein Feld, in dem Text aus der Datei überleben könnte,
//! und er hat mit Absicht kein [`std::fmt::Display`]: Wer `format!("sudo {}
//! install bubblewrap", …)` schreiben wollte, findet nichts, was sich
//! einsetzen ließe.
//!
//! Das ist dieselbe Regel wie in [`crate::doctor::shell_command`] und
//! `crate::summary::copy_command`, nur in ihrer strengsten Form: Dort wird ein
//! Wort von außen geprüft und darf dann durch; hier kommt gar kein Wort von
//! außen durch. Geprüft wird genau einmal, an der Stelle, die den Wert
//! erzeugt — hier also beim Übersetzen von `ID` in eine Variante — und danach
//! nie wieder.
//!
//! # Die Auswahl
//!
//! `ID` zuerst. Nennt es keine der vier Familien, wird `ID_LIKE` durchgegangen
//! (eine Liste, durch Leerzeichen getrennt, die ähnlichste Distribution
//! zuerst); der erste Eintrag, der eine Familie nennt, gewinnt. Bleibt auch
//! das ohne Treffer, ist die Antwort `apt`.
//!
//! # Was hier nicht scheitern darf
//!
//! Fehlende Datei, unlesbare Datei, leere Datei, Bytes, die kein UTF-8 sind:
//! alles ergibt still `apt`. Das hier ist ein Vorschlag in einer
//! Fehlermeldung, keine Sicherheitsentscheidung; eine Fehlermeldung, die beim
//! Erklären ihres eigenen Fehlers scheitert, hilft niemandem.

use std::fs::File;
use std::io::Read as _;
use std::path::Path;

/// Wo die Kennung der Distribution steht (`os-release(5)`).
pub const OS_RELEASE_PATH: &str = "/etc/os-release";

/// Wo sie steht, wenn [`OS_RELEASE_PATH`] fehlt.
///
/// `os-release(5)` nennt beide Orte: Distributionen liefern die Datei unter
/// `/usr`, und `/etc` ist die Kopie, die der Betreiber ändern darf. Fehlt die
/// erste, gilt die zweite.
pub const OS_RELEASE_FALLBACK_PATH: &str = "/usr/lib/os-release";

/// So viele Bytes werden höchstens gelesen.
///
/// Eine echte `os-release` ist unter zwei Kilobyte groß. Der Deckel steht
/// hier, damit ein Leser, der auf etwas anderes zeigt als auf die erwartete
/// Datei, nicht beliebig viel Speicher füllt; was danach kommt, wird nicht
/// gelesen und ändert die Antwort nicht.
const MAX_BYTES: u64 = 64 * 1024;

/// Die Familien, die eine Kennung nennen kann, und ihr Paketverwalter.
///
/// Die Liste enthält die Kennungen (`ID`), die im Feld vorkommen, nicht jede
/// denkbare Ableitung: Wer hier fehlt, kommt über `ID_LIKE` an, und wer auch
/// das nicht setzt, bekommt `apt`. Die Einträge sind kleingeschrieben, so wie
/// `os-release(5)` es für `ID` vorschreibt.
const FAMILIES: &[(&str, PackageManager)] = &[
    ("debian", PackageManager::Apt),
    ("ubuntu", PackageManager::Apt),
    ("raspbian", PackageManager::Apt),
    ("linuxmint", PackageManager::Apt),
    ("pop", PackageManager::Apt),
    ("devuan", PackageManager::Apt),
    ("kali", PackageManager::Apt),
    ("fedora", PackageManager::Dnf),
    ("rhel", PackageManager::Dnf),
    ("centos", PackageManager::Dnf),
    ("rocky", PackageManager::Dnf),
    ("almalinux", PackageManager::Dnf),
    ("ol", PackageManager::Dnf),
    ("amzn", PackageManager::Dnf),
    ("arch", PackageManager::Pacman),
    ("manjaro", PackageManager::Pacman),
    ("endeavouros", PackageManager::Pacman),
    ("artix", PackageManager::Pacman),
    ("suse", PackageManager::Zypper),
    ("sles", PackageManager::Zypper),
    ("sled", PackageManager::Zypper),
];

/// Der Anfang jeder Kennung des openSUSE-Zweigs.
///
/// `opensuse-leap`, `opensuse-tumbleweed`, `opensuse-microos` und was dort
/// noch entsteht, tragen alle dasselbe `zypper`; eine Liste, die jede Fassung
/// einzeln nennt, wäre veraltet, bevor sie geschrieben ist.
const OPENSUSE_PREFIX: &str = "opensuse";

/// Der Paketverwalter, mit dem auf dieser Maschine installiert wird.
///
/// Vier Varianten, kein Feld: Was aus `/etc/os-release` kommt, wählt eine
/// Variante aus und ist danach vergessen. Siehe die Modulbeschreibung.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum PackageManager {
    /// Debian, Ubuntu und deren Ableitungen — und die Antwort, wenn nichts
    /// erkannt wurde.
    #[default]
    Apt,
    /// Fedora, RHEL, `CentOS`, Rocky, Alma, Oracle, Amazon Linux.
    Dnf,
    /// Arch und dessen Ableitungen.
    Pacman,
    /// openSUSE und SUSE Linux Enterprise.
    Zypper,
}

impl PackageManager {
    /// Alle vier, in fester Reihenfolge.
    ///
    /// Ein Test, der beweisen will, dass ein Befehl aus der geschlossenen
    /// Menge stammt, braucht die Menge.
    pub const ALL: [Self; 4] = [Self::Apt, Self::Dnf, Self::Pacman, Self::Zypper];

    /// Der Befehl, der `bubblewrap` mit diesem Paketverwalter nachinstalliert.
    ///
    /// Vier Zeichenketten, die im Binärprogramm stehen. Hier wird nichts
    /// zusammengesetzt und nichts eingesetzt; das ist der Grund, aus dem der
    /// Rückgabetyp `&'static str` ist und nicht `String`.
    #[must_use]
    pub const fn install_bubblewrap(self) -> &'static str {
        match self {
            Self::Apt => "sudo apt install bubblewrap",
            Self::Dnf => "sudo dnf install bubblewrap",
            Self::Pacman => "sudo pacman -S bubblewrap",
            Self::Zypper => "sudo zypper install bubblewrap",
        }
    }

    /// Übersetzt eine einzelne Kennung, wie sie in `ID` oder in einem Eintrag
    /// von `ID_LIKE` steht.
    ///
    /// `None` heißt „diese Kennung nennt keine der vier Familien"; der
    /// Aufrufer geht dann weiter, statt zu raten.
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        let id = id.trim().to_ascii_lowercase();
        if id.is_empty() {
            return None;
        }
        if let Some((_, manager)) = FAMILIES.iter().find(|(name, _)| *name == id) {
            return Some(*manager);
        }
        if id.starts_with(OPENSUSE_PREFIX) {
            return Some(Self::Zypper);
        }
        None
    }

    /// Wählt den Paketverwalter aus dem Text einer `os-release`.
    ///
    /// Gelesen werden genau zwei Schlüssel, `ID` und `ID_LIKE`; jede andere
    /// Zeile, jeder Kommentar und jeder unbekannte Schlüssel wird übergangen.
    /// Kommt ein Schlüssel mehrfach vor, gilt der letzte — so wie eine Shell
    /// die Datei läse.
    #[must_use]
    pub fn from_os_release(text: &str) -> Self {
        let mut id = None;
        let mut id_like = None;
        for line in text.lines() {
            // Der Wagenrücklauf einer Datei mit CRLF-Zeilenenden ist hier
            // schon weg: `str::lines` trennt an `\n` und schneidet ein
            // vorangehendes `\r` ab. `trim` nimmt, was danach noch stört —
            // eingerückte Zeilen und Leerzeichen am Zeilenende.
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            match key.trim_end() {
                "ID" => id = Some(unquote(value.trim())),
                "ID_LIKE" => id_like = Some(unquote(value.trim())),
                _ => {}
            }
        }

        if let Some(manager) = id.as_deref().and_then(Self::from_id) {
            return manager;
        }
        if let Some(like) = id_like.as_deref() {
            // `ID_LIKE` ist nach Ähnlichkeit sortiert, die nächste
            // Verwandtschaft zuerst; der erste Treffer ist deshalb der beste.
            for candidate in like.split_whitespace() {
                if let Some(manager) = Self::from_id(candidate) {
                    return manager;
                }
            }
        }
        Self::Apt
    }

    /// Liest die erste Datei der Liste, die sich lesen lässt, und wählt daraus.
    ///
    /// Der Pfad ist ein Argument, damit ein Test auf eine eigene Datei zeigen
    /// kann: Ein Test, der die `os-release` des Entwicklungsrechners liest,
    /// beweist nichts und antwortet auf jedem Rechner anders.
    ///
    /// Eine Datei, die sich öffnen und lesen lässt, beendet die Suche, auch
    /// wenn sie leer ist oder keinen der beiden Schlüssel enthält — der
    /// zweite Pfad ist der Ersatz für eine fehlende Datei, nicht für eine
    /// unvollständige.
    #[must_use]
    pub fn from_files<P: AsRef<Path>>(candidates: &[P]) -> Self {
        for candidate in candidates {
            if let Some(text) = read_capped(candidate.as_ref()) {
                return Self::from_os_release(&text);
            }
        }
        Self::Apt
    }

    /// Der Paketverwalter dieser Maschine, aus [`OS_RELEASE_PATH`] und
    /// [`OS_RELEASE_FALLBACK_PATH`].
    #[must_use]
    pub fn detect() -> Self {
        Self::from_files(&[OS_RELEASE_PATH, OS_RELEASE_FALLBACK_PATH])
    }
}

/// Der Befehl, der `bubblewrap` auf dieser Maschine nachinstalliert.
///
/// Die eine Stelle, die `SANDBOX_001`, `SANDBOX_002` und die Zeile `bwrap` des
/// Doctors benutzen. Sie teilen sie sich, damit die Oberfläche und
/// `humanitl doctor` auf derselben Maschine nicht zwei verschiedene Befehle
/// zeigen können — das ist die Zusage „in der App wie in `humanitl doctor`"
/// aus HUM-044.
///
/// Gelesen wird bei jedem Aufruf; die Datei ist klein, und alle Aufrufer sind
/// Fehlerpfade, die je Bericht höchstens ein paar Mal vorkommen. Ein
/// prozessweiter Zwischenspeicher wäre versteckter Zustand ohne messbaren
/// Gewinn.
#[must_use]
pub fn install_command() -> &'static str {
    PackageManager::detect().install_bubblewrap()
}

/// Liest höchstens [`MAX_BYTES`] und gibt sie als Text zurück; `None`, wenn
/// die Datei fehlt oder sich nicht lesen lässt.
///
/// Bytes, die kein UTF-8 sind, werden ersetzt statt abgelehnt: Eine
/// `PRETTY_NAME` in einer fremden Kodierung darf die Kennung daneben nicht
/// verschlucken.
fn read_capped(path: &Path) -> Option<String> {
    let file = File::open(path).ok()?;
    let mut bytes = Vec::new();
    file.take(MAX_BYTES).read_to_end(&mut bytes).ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// Nimmt einem Wert die Anführungszeichen ab.
///
/// `os-release(5)` erlaubt einfache und doppelte Anführungszeichen; in
/// doppelten dürfen `"`, `\`, `$` und `` ` `` mit einem Rückstrich maskiert
/// sein. Alles andere bleibt, wie es ist. Was hier herauskommt, wird
/// verglichen und nie ausgeführt; ein Wert, der die Form verfehlt, trifft
/// deshalb einfach auf keine Familie.
fn unquote(value: &str) -> String {
    for quote in ['"', '\''] {
        if value.len() >= 2 && value.starts_with(quote) && value.ends_with(quote) {
            let inner = &value[1..value.len() - 1];
            return if quote == '"' {
                unescape(inner)
            } else {
                inner.to_owned()
            };
        }
    }
    value.to_owned()
}

/// Löst die vier Maskierungen auf, die in doppelten Anführungszeichen gelten.
fn unescape(inner: &str) -> String {
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(current) = chars.next() {
        if current != '\\' {
            out.push(current);
            continue;
        }
        match chars.next() {
            Some(next @ ('"' | '\\' | '$' | '`')) => out.push(next),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}
