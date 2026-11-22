//! Die Präfixtaste im Terminal einer Sitzung (GitHub #10, ADR-0024 Punkt 8).
//!
//! Ein Präfix (`terminal.prefix`, Vorgabe `Ctrl+B` wie in tmux, `--prefix`
//! für einen Aufruf) und danach eine feste Tabelle: `d` hängt ab, `,`
//! Einstellungen, `i` Interceptor, `?` Hilfe. Das Präfix zweimal oder sein
//! Buchstabe schickt es einmal an den Agenten; jede andere Taste geht mit dem
//! Präfix davor unverändert weiter. Die Tabelle steht einmal, in
//! [`humanitl_config::prefix::TABLE`]; Hilfe, `docs/reference/cli.md` und die
//! Beschreibung von `terminal.prefix` entstehen daraus.
//!
//! Dieselbe Taste gilt auf allen drei Wegen in das Terminal einer Sitzung:
//! `humanitl sandbox attach`, `humanitl run -- CMD` an einem Terminal und
//! `humanitl go`; alle drei hängen über `cmd/attach.rs` an.
//!
//! - [`keys`]: die Zustandsmaschine der Tastatur.
//! - [`stream`]: was die Ausgabe des Agenten über das Terminal sagt
//!   (kitty-Protokoll, Alternativschirm).
//! - [`view`]: das Bild des Agenten und die kleine Ansicht darüber.
//!
//! # Was die Taste nie tut
//!
//! Sie lässt den Agenten keine Steuerfolge an das eigene Terminal schicken,
//! die der Filter `FullScreen` des Daemons nicht durchlässt: Die Ausgabe des
//! Agenten geht unverändert und nur gelesen durch [`stream`]; was die
//! Kommandozeile selbst schreibt (Ansicht, Verlassen), sind feste Folgen,
//! in denen kein Byte des Agenten steht. Das kitty-Protokoll schaltet sie
//! nie ein, sie liest nur, ob der Agent es eingeschaltet hat.

pub mod keys;
pub mod stream;
pub mod view;

use humanitl_config::DetachKeys;

/// Die Präfixtaste eines Anschlusses, wie Schalter und Konfiguration sie
/// ergeben.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Keys {
    /// Das Präfix, oder keines.
    pub prefix: DetachKeys,
    /// Ob `Ctrl+,` im kitty-Protokoll direkt die Einstellungen öffnet
    /// (`terminal.direct_keys`).
    pub direct: bool,
    /// Ob die Zeile zur neuen Vorgabe beim Anhängen fällig sein kann:
    /// `terminal.prefix` ist aus keiner Ebene gesetzt.
    pub migration: bool,
}

impl Keys {
    /// Ohne Präfix und ohne direkte Taste.
    pub const fn disabled(self) -> bool {
        matches!(self.prefix, DetachKeys::Disabled)
    }

    /// Der Hinweis beim Anhängen: `Detach: Ctrl+B d, help: Ctrl+B ?`.
    /// `None` ohne Präfix.
    pub fn hint(self) -> Option<String> {
        self.prefix
            .label()
            .map(|label| format!("Detach: {label} d, help: {label} ?"))
    }
}

/// Der lange Hilfetext von `humanitl sandbox attach`, mit der Tabelle.
pub fn attach_help() -> String {
    format!(
        "Attach this terminal to the running session.\n\n\
         One client writes, any number watch. Ctrl+C goes to the session's terminal as byte \
         0x03, which interrupts its foreground program, as in any terminal; this terminal \
         itself never reaches the sandbox. The session keeps running after a detach, and a \
         later attach shows it.\n\n\
         The prefix key (terminal.prefix, default Ctrl+B as in tmux, --prefix for one call) \
         is followed by a fixed table. The same table works in humanitl run -- CMD at a \
         terminal and in humanitl go:\n\n{}",
        humanitl_config::prefix::help_text()
    )
}

#[cfg(test)]
mod tests {
    use humanitl_config::prefix::TABLE;

    /// Die Hilfe von `humanitl sandbox attach` und die Tabelle in
    /// `docs/cli.md` nennen jede Taste der Tabelle; die Seite ist von Hand
    /// geschrieben, also hält dieser Test sie an der Tabelle fest.
    #[test]
    fn help_and_docs_name_every_key_of_the_table() {
        let help = super::attach_help();
        let docs = include_str!("../../../../../docs/cli.md");
        for binding in TABLE {
            let key = char::from(binding.key);
            assert!(
                help.contains(&format!("Ctrl+B {key}  {}", binding.en)),
                "{help}"
            );
            assert!(
                docs.contains(&format!("| `{key}` |")),
                "docs/cli.md lacks {key}"
            );
        }
    }
}
