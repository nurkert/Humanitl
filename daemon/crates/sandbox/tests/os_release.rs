//! Welcher Befehl `bubblewrap` nachinstalliert, gelesen aus `/etc/os-release`
//! (HUM-044).
//!
//! Zwei Fragen stehen hier, und die zweite ist die wichtigere:
//!
//! 1. Trifft die Auswahl auf den zwölf Fixture-Dateien unten zu — Debian,
//!    Ubuntu, Fedora, ein RHEL-Abkömmling über `ID_LIKE`, Arch, Manjaro,
//!    `SteamOS`, openSUSE, SLES, eine unbekannte Distribution, eine Datei mit
//!    CRLF-Zeilenenden, eine mit eingerückten Zeilen?
//! 2. **Kommt aus der Datei jemals ein Zeichen in den Befehl?** Nein, und das
//!    ist keine Frage der Sorgfalt beim Zusammenbauen, sondern der Form:
//!    `ID` und `ID_LIKE` wählen eine von vier Varianten aus, und jede Variante
//!    liefert eine feste Zeichenkette. Die feindliche Datei unten setzt
//!    `ID=x"; touch …; "`; das Ergebnis muss einer der vier festen Befehle
//!    sein.
//!
//! Keine dieser Prüfungen liest die `os-release` des Rechners, auf dem sie
//! läuft. Ein Test, der das täte, bewiese nichts und gäbe auf jedem Rechner
//! eine andere Antwort; die einzige Zeile, die die echte Maschine anfasst,
//! prüft nur die Zugehörigkeit zur geschlossenen Menge.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use humanitl_sandbox::{
    OS_RELEASE_FALLBACK_PATH, OS_RELEASE_PATH, PackageManager, install_command,
};

/// Die vier Befehle, wörtlich, unabhängig von der Aufzählung.
///
/// Absichtlich noch einmal abgeschrieben: Ein Test, der
/// `manager.install_bubblewrap()` mit `manager.install_bubblewrap()`
/// vergleicht, hält nichts fest. Wer einen der vier Befehle ändert, muss diese
/// Liste anfassen und dabei nachdenken.
const EXPECTED_COMMANDS: [(PackageManager, &str); 4] = [
    (PackageManager::Apt, "sudo apt install bubblewrap"),
    (PackageManager::Dnf, "sudo dnf install bubblewrap"),
    (PackageManager::Pacman, "sudo pacman -S bubblewrap"),
    (PackageManager::Zypper, "sudo zypper install bubblewrap"),
];

/// Schreibt genau diese Bytes in eine Datei des Verzeichnisses.
///
/// `fs::write` und nicht `writeln!`: Die Fixture mit CRLF-Zeilenenden ist nur
/// dann eine, wenn niemand unterwegs Zeilenenden übersetzt.
fn fixture(dir: &Path, name: &str, content: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, content.as_bytes()).expect("write the fixture");
    path
}

/// Die Auswahl über eine Datei, wie sie in Wirklichkeit gelesen wird.
fn from_fixture(dir: &Path, name: &str, content: &str) -> PackageManager {
    let path = fixture(dir, name, content);
    PackageManager::from_files(&[path])
}

/// Ein Befehl aus der geschlossenen Menge — oder ein Testfehler.
fn assert_is_a_fixed_command(command: &str) {
    assert!(
        EXPECTED_COMMANDS
            .iter()
            .any(|(_, literal)| *literal == command),
        "not one of the four fixed install commands: {command:?}"
    );
}

/// Die zwölf Dateien, die es auf echten Rechnern gibt, und ihre Antwort.
///
/// Der Name jeder Datei ist der der Distribution, aus deren `os-release` der
/// Inhalt abgeschrieben ist.
const FIXTURES: &[(&str, &str, PackageManager)] = &[
    (
        "debian-12",
        "PRETTY_NAME=\"Debian GNU/Linux 12 (bookworm)\"\n\
             NAME=\"Debian GNU/Linux\"\n\
             VERSION_ID=\"12\"\n\
             VERSION_CODENAME=bookworm\n\
             ID=debian\n\
             HOME_URL=\"https://www.debian.org/\"\n",
        PackageManager::Apt,
    ),
    (
        "ubuntu-24.04",
        "PRETTY_NAME=\"Ubuntu 24.04.1 LTS\"\n\
             NAME=\"Ubuntu\"\n\
             VERSION_ID=\"24.04\"\n\
             ID=ubuntu\n\
             ID_LIKE=debian\n\
             UBUNTU_CODENAME=noble\n",
        PackageManager::Apt,
    ),
    (
        "fedora-40",
        "NAME=\"Fedora Linux\"\n\
             VERSION=\"40 (Workstation Edition)\"\n\
             ID=fedora\n\
             VERSION_ID=40\n\
             PLATFORM_ID=\"platform:f40\"\n",
        PackageManager::Dnf,
    ),
    (
        // Eine Kennung, die die Tabelle nicht kennt: Der Treffer kann nur
        // über `ID_LIKE` kommen.
        "eurolinux-9",
        "NAME=\"EuroLinux\"\n\
             ID=\"eurolinux\"\n\
             ID_LIKE=\"rhel centos fedora\"\n\
             VERSION_ID=\"9.4\"\n",
        PackageManager::Dnf,
    ),
    (
        "arch",
        "NAME=\"Arch Linux\"\n\
             PRETTY_NAME=\"Arch Linux\"\n\
             ID=arch\n\
             BUILD_ID=rolling\n",
        PackageManager::Pacman,
    ),
    (
        "manjaro",
        "NAME=\"Manjaro Linux\"\n\
             ID=manjaro\n\
             ID_LIKE=arch\n\
             BUILD_ID=rolling\n",
        PackageManager::Pacman,
    ),
    (
        // Wieder eine unbekannte Kennung; nur `ID_LIKE` rettet sie.
        "steamos",
        "NAME=\"SteamOS\"\n\
             ID=steamos\n\
             ID_LIKE=arch\n",
        PackageManager::Pacman,
    ),
    (
        // `opensuse-leap` steht in keiner Liste: Der Anfang der Kennung
        // entscheidet.
        "opensuse-leap-15.6",
        "NAME=\"openSUSE Leap\"\n\
             VERSION=\"15.6\"\n\
             ID=\"opensuse-leap\"\n\
             ID_LIKE=\"suse opensuse\"\n",
        PackageManager::Zypper,
    ),
    (
        "sles-15",
        "NAME=\"SLES\"\n\
             VERSION=\"15-SP6\"\n\
             ID=\"sles\"\n",
        PackageManager::Zypper,
    ),
    (
        // Weder Kennung noch Verwandtschaft sind bekannt: apt.
        "frobnicate",
        "NAME=\"Frobnicate OS\"\n\
             ID=frobnicate\n\
             VERSION_ID=\"3\"\n",
        PackageManager::Apt,
    ),
    (
        // Zeilenenden aus einer anderen Welt. Der Wagenrücklauf darf nicht
        // Teil der Kennung werden, sonst hieße sie `arch\r`.
        "crlf",
        "NAME=\"Arch Linux\"\r\nID=arch\r\nBUILD_ID=rolling\r\n",
        PackageManager::Pacman,
    ),
    (
        // Eingerückte Zeilen und Leerzeichen hinter dem Wert: erlaubt ist
        // das nicht, vorkommen tut es trotzdem.
        "whitespace",
        "  NAME=\"Arch Linux\"  \n\tID=arch   \n",
        PackageManager::Pacman,
    ),
];

/// Jede der zwölf Dateien führt auf ihren Paketverwalter und auf dessen
/// festen Befehl.
#[test]
fn install_command_per_os_release() {
    let dir = tempfile::tempdir().expect("tempdir");
    for (name, content, expected) in FIXTURES {
        let found = from_fixture(dir.path(), name, content);
        assert_eq!(found, *expected, "os-release fixture {name}");
        assert_eq!(
            found.install_bubblewrap(),
            EXPECTED_COMMANDS
                .iter()
                .find(|(manager, _)| manager == expected)
                .expect("every variant has a literal")
                .1,
            "install command for {name}"
        );
    }
}

/// Die feindliche Datei: `ID` trägt eine Shell-Zeile.
///
/// Nichts davon darf im Befehl auftauchen. Der Test hält die Form fest, nicht
/// die Sorgfalt: Wer aus `ID` einen Namen für einen Paketverwalter machte und
/// ihn in `format!("sudo {name} install bubblewrap")` einsetzte, fiele hier
/// durch, auch wenn er vorher „gesäubert" hätte.
#[test]
fn a_hostile_id_never_reaches_the_command() {
    let dir = tempfile::tempdir().expect("tempdir");
    let marker = dir.path().join("pwn");
    let content = format!(
        "NAME=\"Trouble\"\nID=x\"; touch {}; \"\nVERSION_ID=\"1\"\n",
        marker.display()
    );

    let found = from_fixture(dir.path(), "hostile-id", &content);
    let command = found.install_bubblewrap();

    assert_is_a_fixed_command(command);
    assert_eq!(
        found,
        PackageManager::Apt,
        "an unknown ID falls back to apt"
    );
    for forbidden in [";", "\"", "'", "touch", "pwn", "$", "`", "|", "&"] {
        assert!(
            !command.contains(forbidden),
            "the command carries {forbidden:?} from the file: {command:?}"
        );
    }
    assert!(
        !marker.exists(),
        "reading an os-release must not run anything"
    );
}

/// Dasselbe für `ID_LIKE`: Auch die Liste der Verwandten ist fremder Text.
#[test]
fn a_hostile_id_like_never_reaches_the_command() {
    let dir = tempfile::tempdir().expect("tempdir");
    let marker = dir.path().join("pwn");
    let content = format!(
        "ID=frobnicate\nID_LIKE=\"$(touch {}) debian\"\n",
        marker.display()
    );

    let found = from_fixture(dir.path(), "hostile-id-like", &content);

    assert_eq!(found, PackageManager::Apt, "the last entry is debian");
    assert_is_a_fixed_command(found.install_bubblewrap());
    assert!(
        !marker.exists(),
        "reading an os-release must not run anything"
    );
}

/// `ID` schlägt `ID_LIKE`, auch wenn beide etwas sagen.
#[test]
fn the_id_decides_before_the_id_like() {
    let dir = tempfile::tempdir().expect("tempdir");
    let found = from_fixture(
        dir.path(),
        "id-beats-id-like",
        "ID=fedora\nID_LIKE=debian\n",
    );
    assert_eq!(found, PackageManager::Dnf);
}

/// `ID_LIKE` ist nach Ähnlichkeit sortiert; der erste Treffer gewinnt.
#[test]
fn the_id_like_list_is_walked_from_the_front() {
    let dir = tempfile::tempdir().expect("tempdir");
    let found = from_fixture(
        dir.path(),
        "id-like-order",
        "ID=unheard-of\nID_LIKE=\"frobnicate arch debian\"\n",
    );
    assert_eq!(
        found,
        PackageManager::Pacman,
        "arch steht vor debian in der Liste"
    );
}

/// Anführungszeichen, einfache wie doppelte, und die Maskierungen darin.
#[test]
fn quoted_values_are_read_without_their_quotes() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert_eq!(
        from_fixture(dir.path(), "double", "ID=\"fedora\"\n"),
        PackageManager::Dnf
    );
    assert_eq!(
        from_fixture(dir.path(), "single", "ID='arch'\n"),
        PackageManager::Pacman
    );
    assert_eq!(
        from_fixture(
            dir.path(),
            "escaped",
            "ID=\"debian\"\nPRETTY_NAME=\"a \\\"quote\\\"\"\n"
        ),
        PackageManager::Apt
    );
}

/// Kommentare, Leerzeilen und unbekannte Schlüssel stören nicht.
#[test]
fn comments_blank_lines_and_unknown_keys_are_ignored() {
    let dir = tempfile::tempdir().expect("tempdir");
    let found = from_fixture(
        dir.path(),
        "noisy",
        "# a comment\n\
         \n\
         SOMETHING_ELSE=whatever\n\
         ID=opensuse-tumbleweed\n\
         \n\
         # trailing comment\n",
    );
    assert_eq!(found, PackageManager::Zypper);
}

/// Eine Datei, die es nicht gibt, ist kein Fehler.
#[test]
fn a_missing_file_is_apt() {
    let dir = tempfile::tempdir().expect("tempdir");
    let found = PackageManager::from_files(&[dir.path().join("nothing-here")]);
    assert_eq!(found, PackageManager::Apt);
    assert_is_a_fixed_command(found.install_bubblewrap());
}

/// Eine leere Datei ist kein Fehler.
#[test]
fn an_empty_file_is_apt() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert_eq!(from_fixture(dir.path(), "empty", ""), PackageManager::Apt);
}

/// Ein Verzeichnis an der Stelle der Datei ist kein Fehler.
///
/// `open` gelingt auf einem Verzeichnis, `read` nicht; das ist der Fall, in
/// dem ein Leser ohne Vorsicht mit einem `unwrap` endet.
#[test]
fn a_directory_in_place_of_the_file_is_apt() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("os-release");
    fs::create_dir(&path).expect("create the directory");
    assert_eq!(PackageManager::from_files(&[path]), PackageManager::Apt);
}

/// Eine Datei ohne Leserecht ist kein Fehler.
#[test]
fn an_unreadable_file_is_apt() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = fixture(dir.path(), "unreadable", "ID=fedora\n");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).expect("chmod 000");

    if fs::read(&path).is_ok() {
        // Als `root` hält `chmod 000` niemanden ab; dann prüft dieser Test
        // nichts und sagt es, statt etwas Falsches zu behaupten.
        eprintln!("SKIP an_unreadable_file_is_apt: running as root");
        return;
    }

    assert_eq!(PackageManager::from_files(&[path]), PackageManager::Apt);
}

/// Fehlt die erste Datei, wird die zweite gelesen.
#[test]
fn the_fallback_path_is_read_when_the_first_is_missing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let missing = dir.path().join("etc-os-release");
    let fallback = fixture(dir.path(), "usr-lib-os-release", "ID=fedora\n");
    assert_eq!(
        PackageManager::from_files(&[missing, fallback]),
        PackageManager::Dnf
    );
}

/// Die erste lesbare Datei entscheidet, auch wenn sie nichts hergibt.
///
/// `/usr/lib/os-release` ist der Ersatz für eine **fehlende** Datei, nicht für
/// eine unvollständige: Wer in `/etc` eine leere Datei hinlegt, hat damit
/// etwas gesagt.
#[test]
fn the_first_readable_file_ends_the_search() {
    let dir = tempfile::tempdir().expect("tempdir");
    let first = fixture(dir.path(), "first-empty", "");
    let second = fixture(dir.path(), "second-fedora", "ID=fedora\n");
    assert_eq!(
        PackageManager::from_files(&[first, second]),
        PackageManager::Apt
    );
}

/// Die vier Befehle sind wörtlich das, was sie sein sollen, und tragen kein
/// einziges Zeichen, das eine Shell umdeutet.
#[test]
fn every_install_command_is_a_plain_literal() {
    assert_eq!(PackageManager::ALL.len(), EXPECTED_COMMANDS.len());
    for (manager, literal) in EXPECTED_COMMANDS {
        assert_eq!(manager.install_bubblewrap(), literal);
        assert!(
            literal
                .chars()
                .all(|c| c.is_ascii_lowercase() || c == 'S' || c == ' ' || c == '-'),
            "unexpected character in {literal:?}"
        );
    }
    for manager in PackageManager::ALL {
        assert!(
            EXPECTED_COMMANDS.iter().any(|(known, _)| *known == manager),
            "{manager:?} has no expected literal in this test"
        );
    }
}

/// Ohne jede Kennung ist die Antwort `apt`.
#[test]
fn the_default_is_apt() {
    assert_eq!(PackageManager::default(), PackageManager::Apt);
    assert_eq!(PackageManager::from_os_release(""), PackageManager::Apt);
    assert_eq!(PackageManager::from_id(""), None);
    assert_eq!(PackageManager::from_id("frobnicate"), None);
}

/// Gelesen wird an den beiden Orten, die `os-release(5)` nennt.
#[test]
fn the_paths_are_the_documented_ones() {
    assert_eq!(OS_RELEASE_PATH, "/etc/os-release");
    assert_eq!(OS_RELEASE_FALLBACK_PATH, "/usr/lib/os-release");
}

/// Die einzige Zeile, die die echte Maschine anfasst.
///
/// Welcher der vier Befehle hier herauskommt, hängt vom Rechner ab; dass es
/// einer der vier ist, hängt von nichts ab.
#[test]
fn the_command_of_this_machine_is_one_of_the_four() {
    assert_is_a_fixed_command(install_command());
}
