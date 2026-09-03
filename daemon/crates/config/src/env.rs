//! Die Umgebung als Wert, nicht als globaler Zustand.
//!
//! Alles, was Pfade sucht oder `HUMANITL_*`-Variablen liest, bekommt eine
//! [`Env`] übergeben. Der einzige Ort, der `std::env` anfasst, ist
//! [`Env::from_process`]. Damit sind Pfade und Präzedenz ohne
//! `std::env::set_var` prüfbar, und Tests laufen parallel, ohne sich
//! gegenseitig die Umgebung umzustellen.

use std::collections::BTreeMap;
use std::collections::btree_map::Iter;

/// Eine Umgebung: Variablen plus die Nutzerkennung, die für
/// `$XDG_RUNTIME_DIR`-Ersatzpfade nötig ist.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Env {
    vars: BTreeMap<String, String>,
    uid: u32,
}

impl Env {
    /// Die Umgebung des laufenden Prozesses.
    ///
    /// Variablen, deren Name oder Wert kein gültiges UTF-8 ist, fallen weg;
    /// Humanitl kennt keinen Schlüssel, der so aussieht. Gelesen wird über
    /// `vars_os`, weil `std::env::vars` bei so einer Variablen abbricht, und
    /// ein Daemon nicht wegen einer fremden Variablen sterben darf.
    #[must_use]
    pub fn from_process() -> Self {
        Self {
            vars: std::env::vars_os()
                .filter_map(|(key, value)| {
                    Some((key.into_string().ok()?, value.into_string().ok()?))
                })
                .collect(),
            uid: current_uid(),
        }
    }

    /// Eine Umgebung aus Paaren, für Tests und für den Fake-Modus.
    ///
    /// Die Nutzerkennung ist die des laufenden Prozesses; [`Env::with_uid`]
    /// setzt sie auf einen festen Wert.
    #[must_use]
    pub fn from_pairs<K, V, I>(pairs: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        Self {
            vars: pairs
                .into_iter()
                .map(|(key, value)| (key.into(), value.into()))
                .collect(),
            uid: current_uid(),
        }
    }

    /// Setzt die Nutzerkennung.
    #[must_use]
    pub const fn with_uid(mut self, uid: u32) -> Self {
        self.uid = uid;
        self
    }

    /// Setzt eine Variable.
    #[must_use]
    pub fn with(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.vars.insert(key.into(), value.into());
        self
    }

    /// Der Wert einer Variablen.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&str> {
        self.vars.get(key).map(String::as_str)
    }

    /// Der Wert einer Variablen, wenn er nicht leer ist.
    ///
    /// Die XDG-Spezifikation behandelt eine leere Variable wie eine nicht
    /// gesetzte; ein leerer Pfad wäre auch nicht zu gebrauchen.
    #[must_use]
    pub fn non_empty(&self, key: &str) -> Option<&str> {
        self.get(key).filter(|value| !value.is_empty())
    }

    /// Die Nutzerkennung.
    #[must_use]
    pub const fn uid(&self) -> u32 {
        self.uid
    }

    /// Alle Variablen, nach Namen sortiert.
    pub fn iter(&self) -> Iter<'_, String, String> {
        self.vars.iter()
    }
}

impl<'a> IntoIterator for &'a Env {
    type Item = (&'a String, &'a String);
    type IntoIter = Iter<'a, String, String>;

    fn into_iter(self) -> Self::IntoIter {
        self.vars.iter()
    }
}

#[cfg(unix)]
fn current_uid() -> u32 {
    use std::os::unix::fs::MetadataExt as _;

    std::fs::metadata("/proc/self").map_or(0, |meta| meta.uid())
}

#[cfg(not(unix))]
const fn current_uid() -> u32 {
    0
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::Env;

    #[test]
    fn pairs_and_uid_are_injected() {
        let env = Env::from_pairs([("HOME", "/home/x"), ("EMPTY", "")]).with_uid(1000);
        assert_eq!(env.get("HOME"), Some("/home/x"));
        assert_eq!(env.non_empty("EMPTY"), None);
        assert_eq!(env.get("EMPTY"), Some(""));
        assert_eq!(env.uid(), 1000);
        assert_eq!(env.iter().count(), 2);
    }

    #[test]
    fn process_env_is_readable() {
        let env = Env::from_process();
        assert_eq!(env.get("HUMANITL_A_VARIABLE_NOBODY_SETS"), None);
    }
}
