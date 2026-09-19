//! Welches Unterkommando welche RPC des Daemons bedient (ADR-018, HUM-078).
//!
//! `clap` kennt keine eigenen Attribute an den `derive`-Typen; die Zuordnung
//! RPC → Unterkommando steht deshalb hier als Tabelle und nicht als
//! `#[humanitl(rpc = "…")]` am Unterkommando. Gelesen wird sie zweimal:
//!
//! - `cargo xtask docs` liest diese Datei als Text und erzeugt daraus die
//!   Spalte „CLI" von `docs/reference/parity.md`. Eine RPC ohne Zeile hier
//!   und ohne Eintrag in `daemon/xtask/parity_exempt.toml` bricht den Lauf und
//!   damit den CI-Job `parity-check`.
//! - Die Tests unten prüfen jede Zeile gegen die `clap`-Struktur: Die Wörter
//!   vor dem ersten Schalter sind ein Pfad aus Unterkommandos, und jeder
//!   Schalter ist ein Argument dieses Unterkommandos. Ein umbenanntes
//!   Unterkommando fällt so auf, bevor die Tabelle es falsch nennt.
//!
//! Das Format ist deshalb streng: eine Zeile je Paar, genau
//! `("Humanitl.<Rpc>", "<unterkommando> [--schalter [wert]]"),`, Kommentare nur
//! als eigene Zeile. Mehrere Zeilen für dieselbe RPC sind erlaubt und stehen
//! in der Tabelle untereinander.
//!
//! Nur Unterkommandos, die die RPC wirklich über gRPC aufrufen, stehen hier.
//! `GetConfig` und `SetConfig` fehlen deshalb: `config get` und `config set`
//! lesen und schreiben die Konfiguration lokal; sie stehen als Ausnahme in
//! `daemon/xtask/parity_exempt.toml`.
//!
//! Im Binary selbst hat die Tabelle keine Aufgabe; das Modul wird nur für die
//! Tests übersetzt.

/// Paare aus RPC-Name (`Service.Methode`) und Unterkommando samt Schalter.
pub static PARITY: &[(&str, &str)] = &[
    ("Humanitl.Audit", "audit export"),
    ("Humanitl.Audit", "audit verify"),
    ("Humanitl.Decide", "flows decide"),
    ("Humanitl.DiscoverLlm", "llm discover"),
    ("Humanitl.Doctor", "doctor"),
    ("Humanitl.GetBody", "flows show --body"),
    ("Humanitl.GetFlow", "flows show"),
    ("Humanitl.GetInfo", "daemon status"),
    ("Humanitl.GetSessionSummary", "sessions summary"),
    ("Humanitl.ListFlows", "flows list"),
    ("Humanitl.ProbeLlm", "llm test"),
    ("Humanitl.Rules", "rules add"),
    ("Humanitl.Rules", "rules disable"),
    ("Humanitl.Rules", "rules dry-run"),
    ("Humanitl.Rules", "rules enable"),
    ("Humanitl.Rules", "rules list"),
    ("Humanitl.Rules", "rules reload"),
    ("Humanitl.Rules", "rules remove"),
    ("Humanitl.Rules", "rules reorder"),
    ("Humanitl.Rules", "rules test"),
    ("Humanitl.Rules", "rules update"),
    ("Humanitl.Sandbox", "run"),
    // Bis es `flows watch` gibt, liest nur die Moderation im Terminal den
    // Ereignisstrom mit.
    ("Humanitl.Subscribe", "run --ask terminal"),
    ("Humanitl.Terminal", "sandbox attach"),
];

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use clap::Command;

    use super::PARITY;
    use crate::cli::command;

    /// Löst die Wörter eines Eintrags gegen die `clap`-Struktur auf.
    ///
    /// Nach dem Pfad aus Unterkommandos darf nur noch stehen: ein langer
    /// Schalter des Unterkommandos, und direkt dahinter höchstens ein Wert,
    /// wenn der Schalter einen nimmt; kennt `clap` die erlaubten Werte, muss es
    /// einer davon sein. Jedes andere Wort, auch ein kurzer Schalter, ist ein
    /// Fehler und wird nicht übersprungen.
    ///
    /// Liefert den Befund als Text, damit ein Test alle falschen Zeilen auf
    /// einmal nennen kann.
    fn resolve(root: &Command, entry: &str) -> Result<(), String> {
        let mut words = entry.split(' ').peekable();
        let mut leaf = root;
        let mut path = String::from("humanitl");
        while let Some(word) = words.next_if(|word| !word.starts_with('-')) {
            path.push(' ');
            path.push_str(word);
            leaf = leaf
                .find_subcommand(word)
                .ok_or_else(|| format!("`{path}` is not a subcommand"))?;
        }
        if leaf.has_subcommands() {
            return Err(format!("`{path}` is a group, not a subcommand"));
        }
        while let Some(word) = words.next() {
            let flag = word.strip_prefix("--").ok_or_else(|| {
                format!("`{path}`: `{word}` is neither a long flag nor its value")
            })?;
            let arg = leaf
                .get_arguments()
                .find(|arg| {
                    arg.get_long() == Some(flag)
                        || arg
                            .get_all_aliases()
                            .is_some_and(|aliases| aliases.contains(&flag))
                })
                .ok_or_else(|| format!("`{path}` has no flag `--{flag}`"))?;
            if !arg.get_action().takes_values() {
                continue;
            }
            if let Some(value) = words.next_if(|word| !word.starts_with('-')) {
                let allowed = arg.get_possible_values();
                if !allowed.is_empty() && !allowed.iter().any(|pv| pv.matches(value, false)) {
                    return Err(format!("`{path} --{flag}` does not take `{value}`"));
                }
            }
        }
        Ok(())
    }

    /// Der Befehl mit allen globalen Argumenten an jedem Unterkommando.
    fn built() -> Command {
        let mut root = command();
        root.build();
        root
    }

    #[test]
    fn every_entry_names_a_subcommand_and_its_flags() {
        let root = built();
        let wrong: Vec<String> = PARITY
            .iter()
            .filter_map(|(rpc, entry)| resolve(&root, entry).err().map(|e| format!("{rpc}: {e}")))
            .collect();
        assert!(
            wrong.is_empty(),
            "parity table disagrees with clap: {wrong:#?}"
        );
    }

    #[test]
    fn a_renamed_subcommand_is_caught() {
        let root = built();
        assert!(resolve(&root, "flows show --body").is_ok());
        assert!(resolve(&root, "run --ask terminal").is_ok());
        assert!(resolve(&root, "flows watch").is_err());
        assert!(resolve(&root, "flows").is_err());
        assert!(resolve(&root, "flows show --bodies").is_err());
    }

    #[test]
    fn every_word_after_the_subcommand_is_checked() {
        let root = built();
        assert!(resolve(&root, "flows show --body request").is_ok());
        // Ein Wert zu viel, ein Wert, den der Schalter nicht nimmt, ein Wert
        // zu einem Schalter ohne Wert und ein kurzer Schalter.
        assert!(resolve(&root, "run --ask terminal bogus").is_err());
        assert!(resolve(&root, "run --ask termnal").is_err());
        assert!(resolve(&root, "flows show --raw request").is_err());
        assert!(resolve(&root, "flows show -q").is_err());
    }

    /// Ob eine RPC im Vertrag steht, prüft `cargo xtask docs` gegen den
    /// Descriptor; hier bleibt nur, was ohne ihn zu sehen ist.
    #[test]
    fn every_pair_is_unique() {
        let mut seen = std::collections::BTreeSet::new();
        for (rpc, entry) in PARITY {
            assert!(seen.insert((rpc, entry)), "duplicate: {rpc} {entry}");
        }
    }
}
