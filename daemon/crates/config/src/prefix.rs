//! Die Tabelle der Präfixtaste (GitHub #10, ADR-0024 Punkt 8).
//!
//! Nach dem Präfix (`terminal.prefix`, Vorgabe `ctrl-b` wie in tmux) gilt
//! eine feste Tabelle: `d` hängt ab, `,` öffnet die Einstellungen, `i` den
//! Interceptor, `?` die Hilfe. Das Präfix zweimal oder sein Buchstabe
//! schickt es einmal an den Agenten, jede andere Taste geht mit dem Präfix
//! davor unverändert weiter.
//!
//! Die Tabelle steht nur hier. Aus ihr entstehen die Hilfe hinter `?` und
//! die Hilfe von `humanitl sandbox attach` (damit `docs/reference/cli.md`)
//! auf Englisch, und die Beschreibung des Schlüssels `terminal.prefix` im
//! Schema (damit `docs/CONFIG.md`) auf Deutsch. Gelesen wird die Taste nur
//! von der Kommandozeile; Daemon und Oberfläche lesen sie nicht.
//!
//! Dazu der Merker für die Umstellung der Vorgabe von `ctrl-a` auf `ctrl-b`:
//! Wer keinen Wert gesetzt hat, bekommt beim ersten Anhängen danach eine
//! Zeile ([`MIGRATION_LINE`]), einmal je Nutzer, gemerkt im
//! Datenverzeichnis und nie im Projekt.

use std::io;
use std::path::Path;

use crate::origin::{Origin, Resolved};
use crate::paths::Paths;

/// Was eine Taste der Tabelle tut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrefixAction {
    /// Abhängen; die Sitzung läuft weiter.
    Detach,
    /// Die Einstellungen im Terminal der Sitzung.
    Settings,
    /// Der Interceptor im Terminal der Sitzung.
    Interceptor,
    /// Die Hilfe mit dieser Tabelle.
    Help,
}

/// Eine Zeile der Tabelle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Binding {
    /// Die Taste nach dem Präfix, als ASCII-Byte.
    pub key: u8,
    /// Was sie tut.
    pub action: PrefixAction,
    /// Die Beschreibung für Hilfe und Kommandozeile, Englisch.
    pub en: &'static str,
    /// Die Beschreibung für das Schema und `docs/CONFIG.md`, Deutsch.
    pub de: &'static str,
}

/// Die feste Tabelle nach dem Präfix, in der Reihenfolge der Hilfe.
pub static TABLE: &[Binding] = &[
    Binding {
        key: b'd',
        action: PrefixAction::Detach,
        en: "detach; the session keeps running",
        de: "hängt ab und lässt die Sitzung laufen",
    },
    Binding {
        key: b',',
        action: PrefixAction::Settings,
        en: "settings (here soon; meanwhile humanitl settings)",
        de: "Einstellungen (im Terminal der Sitzung bald; bis dahin `humanitl settings`)",
    },
    Binding {
        key: b'i',
        action: PrefixAction::Interceptor,
        en: "interceptor (here soon; meanwhile humanitl intercept)",
        de: "Interceptor (im Terminal der Sitzung bald; bis dahin `humanitl intercept`)",
    },
    Binding {
        key: b'?',
        action: PrefixAction::Help,
        en: "this help",
        de: "zeigt diese Tabelle",
    },
];

/// Die Zeile der Tabelle für `key`, wenn es eine gibt.
#[must_use]
pub fn binding(key: u8) -> Option<&'static Binding> {
    TABLE.iter().find(|binding| binding.key == key)
}

/// Was eine Ansicht anstelle von Einstellungen und Interceptor zeigt,
/// solange es sie im Terminal der Sitzung nicht gibt (#11, #12): ein Satz,
/// auf zwei kurze Zeilen verteilt, damit er auch in 80 Spalten ganz steht.
#[must_use]
pub const fn coming(action: PrefixAction) -> Option<[&'static str; 2]> {
    match action {
        PrefixAction::Settings => Some([
            "The settings view in this terminal is coming;",
            "meanwhile run humanitl settings in another terminal.",
        ]),
        PrefixAction::Interceptor => Some([
            "The interceptor view in this terminal is coming;",
            "meanwhile run humanitl intercept in another terminal.",
        ]),
        PrefixAction::Detach | PrefixAction::Help => None,
    }
}

/// Die Hilfe zur Präfixtaste, Englisch, eine Zeile je Taste: für `?` und
/// die Hilfe der Kommandozeile.
///
/// `label` ist der Name des Präfixes (`Ctrl+B`), `letter` sein Buchstabe
/// (`b`). Jede Zeile bleibt mit der Vorgabe unter 76 Zeichen, damit die
/// Hilfe hinter `?` auch in 80 Spalten ganz steht.
#[must_use]
pub fn help_lines(label: &str, letter: char) -> Vec<String> {
    let mut lines: Vec<String> = TABLE
        .iter()
        .map(|binding| format!("{label} {}  {}", char::from(binding.key), binding.en))
        .collect();
    lines.push(format!(
        "{label} {label} or {label} {letter}  send {label} once to the agent"
    ));
    lines.push("          (a tmux in the sandbox, readline's one character back)".to_owned());
    lines.push(format!("{label} and any other key  both go to the agent"));
    lines.push("Ctrl+,    settings directly while the agent uses the kitty keyboard".to_owned());
    lines.push("          protocol (terminal.direct_keys)".to_owned());
    lines
}

/// Die Hilfe der Kommandozeile mit der Vorgabe `Ctrl+B`: Absatz für Absatz,
/// wie `clap` ihn als langen Hilfetext zeigt.
#[must_use]
pub fn help_text() -> String {
    help_lines("Ctrl+B", 'b')
        .into_iter()
        .map(|line| format!("  {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Die Beschreibung von `terminal.prefix` im Schema, Deutsch, mit der
/// Tabelle.
#[must_use]
pub fn schema_description() -> String {
    let table = TABLE
        .iter()
        .map(|binding| format!("`{}` {}", char::from(binding.key), binding.de))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "Die Präfixtaste im Terminal einer Sitzung (`humanitl sandbox attach`, `humanitl run \
         -- CMD` an einem Terminal, `humanitl go`): `ctrl-a` bis `ctrl-z`, `ctrl-\\`, `ctrl-]`, \
         `ctrl-^`, `ctrl-_`, oder `none`, dann gibt es keine und jedes Byte geht an den \
         Agenten. Danach gilt eine feste Tabelle: {table}. Das Präfix zweimal oder sein \
         Buchstabe schickt es einmal an den Agenten, so erreicht `Ctrl+B` ein tmux in der \
         Sandbox und eine Shell, für die es ein Zeichen zurück heißt; jede andere Taste geht \
         mit dem Präfix unverändert weiter. `ctrl-[` ist ausgenommen, weil jede Pfeiltaste \
         mit diesem Byte beginnt, `ctrl-h`, `ctrl-i`, `ctrl-j` und `ctrl-m`, weil Rücktaste, \
         Tab und Enter dieselben Bytes schicken. Der alte Name `terminal.detach_prefix` gilt \
         weiter. `--prefix` (oder `--detach-keys`) überschreibt den Wert für einen Aufruf."
    )
}

/// Die Zeile beim ersten Anhängen nach der Umstellung der Vorgabe.
pub const MIGRATION_LINE: &str = "The prefix key is now Ctrl+B (was Ctrl+A). Keep the old one: humanitl config set \
     terminal.prefix ctrl-a";

/// Ob die Zeile [`MIGRATION_LINE`] fällig ist: `terminal.prefix` steht auf
/// der Vorgabe, aus keiner Ebene gesetzt (auch nicht unter dem alten Namen),
/// und die Zeile wurde für diesen Nutzer noch nicht gezeigt.
#[must_use]
pub fn migration_due(paths: &Paths, resolved: &Resolved) -> bool {
    let unset = resolved
        .origin("terminal.prefix")
        .is_none_or(|origin| *origin == Origin::Default);
    unset && !paths.prefix_notice_path().exists()
}

/// Beansprucht die Zeile für diesen Aufruf: legt den Merker an, aber nur,
/// wenn es ihn noch nicht gibt (`create_new`, also `O_EXCL`). `true` heißt,
/// dieser Aufruf hat ihn angelegt und zeigt die Zeile; ein zweiter Anschluss,
/// der im selben Augenblick zum ersten Mal anhängt, bekommt `false` und
/// zeigt nichts (Review zu GitHub #10).
///
/// # Errors
///
/// Wenn das Datenverzeichnis oder die Datei aus einem anderen Grund nicht
/// angelegt werden kann; dann zeigt der Aufrufer die Zeile trotzdem, und sie
/// kommt beim nächsten Anhängen noch einmal. Das ist die harmlose Seite.
pub fn claim_migration(paths: &Paths) -> io::Result<bool> {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt as _;

    let file = paths.prefix_notice_path();
    if let Some(dir) = file.parent() {
        create_private_dir(dir)?;
    }
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(crate::paths::FILE_MODE)
        .open(&file)
    {
        Ok(mut created) => {
            created.write_all(b"ctrl-b\n")?;
            Ok(true)
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(false),
        Err(error) => Err(error),
    }
}

/// Legt das Datenverzeichnis mit den Rechten an, die der Daemon ihm gibt.
fn create_private_dir(dir: &Path) -> io::Result<()> {
    use std::os::unix::fs::DirBuilderExt as _;

    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(crate::paths::DIR_MODE)
        .create(dir)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::{
        MIGRATION_LINE, PrefixAction, TABLE, binding, claim_migration, help_lines, migration_due,
        schema_description,
    };
    use crate::{Env, Paths, Sources, load};

    /// Die Tabelle hat genau die vier Tasten der Spezifikation, jede einmal.
    #[test]
    fn the_table_has_the_four_keys_once() {
        let keys: Vec<u8> = TABLE.iter().map(|binding| binding.key).collect();
        assert_eq!(keys, b"d,i?");
        assert_eq!(binding(b'd').map(|b| b.action), Some(PrefixAction::Detach));
        assert_eq!(
            binding(b',').map(|b| b.action),
            Some(PrefixAction::Settings)
        );
        assert_eq!(
            binding(b'i').map(|b| b.action),
            Some(PrefixAction::Interceptor)
        );
        assert_eq!(binding(b'?').map(|b| b.action), Some(PrefixAction::Help));
        assert_eq!(binding(b'x'), None);
    }

    /// Hilfe und Schema entstehen aus der Tabelle: Jede Zeile steht in
    /// beiden, mit ihrer Taste.
    #[test]
    fn help_and_schema_are_made_from_the_table() {
        let help = help_lines("Ctrl+B", 'b').join("\n");
        let schema = schema_description();
        for binding in TABLE {
            let key = char::from(binding.key);
            assert!(
                help.contains(&format!("Ctrl+B {key}  {}", binding.en)),
                "{help}"
            );
            assert!(
                schema.contains(&format!("`{key}` {}", binding.de)),
                "{schema}"
            );
        }
        assert!(help.contains("Ctrl+B Ctrl+B or Ctrl+B b"), "{help}");
        assert!(help.contains("tmux"), "{help}");
        // Mit Rand und Rahmen der Ansicht passt jede Zeile in 80 Spalten.
        for line in help_lines("Ctrl+B", 'b') {
            assert!(line.len() <= 76, "{line:?} is {} long", line.len());
        }
        for action in [PrefixAction::Settings, PrefixAction::Interceptor] {
            for line in super::coming(action).expect("a sentence") {
                assert!(line.len() <= 76, "{line:?}");
            }
        }
    }

    fn paths_in(dir: &std::path::Path) -> Paths {
        Paths::new(Env::from_pairs([
            ("HOME", dir.display().to_string()),
            ("XDG_DATA_HOME", dir.join("data").display().to_string()),
        ]))
    }

    /// Die Zeile der Umstellung kommt beim ersten Anhängen ohne gesetzten
    /// Wert, nicht beim zweiten und nie mit gesetztem Wert, auch nicht unter
    /// dem alten Namen. Gemerkt wird im Datenverzeichnis.
    #[test]
    fn the_migration_line_comes_once_and_only_without_a_value() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        let unset = load(&Sources::empty()).unwrap();
        assert_eq!(unset.config.terminal.prefix, "ctrl-b");
        assert!(migration_due(&paths, &unset), "the first attach");
        assert!(claim_migration(&paths).unwrap(), "the first claim wins");
        assert!(!claim_migration(&paths).unwrap(), "a second claim loses");
        assert!(
            paths
                .prefix_notice_path()
                .starts_with(dir.path().join("data")),
            "remembered in the data folder"
        );
        assert!(!migration_due(&paths, &unset), "the second attach");

        let other = tempfile::tempdir().unwrap();
        let fresh = paths_in(other.path());
        for (key, value) in [
            ("HUMANITL_TERMINAL__PREFIX", "ctrl-a"),
            ("HUMANITL_TERMINAL__DETACH_PREFIX", "ctrl-a"),
        ] {
            let set = load(&Sources::empty().with_env(Env::from_pairs([(key, value)]))).unwrap();
            assert_eq!(set.config.terminal.prefix, "ctrl-a", "{key}");
            assert!(!migration_due(&fresh, &set), "{key} is set");
        }
        assert!(MIGRATION_LINE.contains("humanitl config set terminal.prefix ctrl-a"));
    }

    /// Zwei erste Anschlüsse im selben Augenblick: Genau einer beansprucht
    /// die Zeile. Mutationsbeweis: ohne `create_new` rot.
    #[test]
    fn two_first_attaches_claim_the_line_once() {
        let dir = tempfile::tempdir().unwrap();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
        let winners: usize = (0..8)
            .map(|_| {
                let paths = paths_in(dir.path());
                let barrier = std::sync::Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    usize::from(claim_migration(&paths).unwrap())
                })
            })
            .collect::<Vec<_>>()
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .sum();
        assert_eq!(winners, 1);
    }
}
