//! Die Sperren einer Firmenvorgabe über jeden Schlüssel (HUM-534, BACKLOG
//! Abschnitt 9, Punkt 21).
//!
//! Eine Sperre legt einen Schlüssel auf genau einen Wert fest. Sie ist eine
//! eigene Ebene der Auflösung, die oberste ([`crate::Origin::Policy`]): Weder
//! Umgebung noch Kommandozeile noch Projektwahl ändern einen gesperrten Wert.
//! Jeder Client zeigt ihn nur lesbar, mit der Vorgabe als Herkunft, und
//! `SetConfig` sowie `humanitl config set` lehnen das Schreiben mit
//! `AGENT_010` ab, dem Code der Sperre aus HUM-518 ([`refuse_locked_write`]).
//!
//! Die Vorgabedatei gibt es noch nicht. Bis dahin ist [`PolicyLocks::of_policy`]
//! leer, und Sperren kommen aus den Tests ([`PolicyLocks::with`]); die Sperre
//! der Quelle der Agenten aus HUM-518 ([`crate::agents::SourceLock`]) bleibt
//! daneben bestehen, weil sie nur `depot` erlaubt und keinen festen Wert.

use std::collections::BTreeMap;

use humanitl_core::diagnostics::codes::AGENT_010;
use humanitl_core::{Diagnostic, Severity};

use crate::paths::Paths;

/// Ein Wert, den eine Firmenvorgabe festlegt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockedValue {
    /// Der einzige Wert, den der Schlüssel haben darf, als Text wie auf der
    /// Kommandozeile.
    pub value: String,
    /// Wer sperrt, so wie die Clients es nennen: die Vorgabedatei.
    pub by: String,
}

/// Die Sperren einer Firmenvorgabe, je Schlüssel ein fester Wert.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PolicyLocks {
    locks: BTreeMap<String, LockedValue>,
}

impl PolicyLocks {
    /// Keine Sperre.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// Die Sperren aus der Firmen-Vorgabedatei; heute keine. Die eine Stelle,
    /// an der die Datei später gelesen wird.
    #[must_use]
    pub fn of_policy(paths: &Paths) -> Self {
        let _ = paths;
        Self::none()
    }

    /// Fügt eine Sperre hinzu.
    #[must_use]
    pub fn with(
        mut self,
        key: impl Into<String>,
        value: impl Into<String>,
        by: impl Into<String>,
    ) -> Self {
        self.locks.insert(
            key.into(),
            LockedValue {
                value: value.into(),
                by: by.into(),
            },
        );
        self
    }

    /// Die Sperre eines Schlüssels.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&LockedValue> {
        self.locks.get(key)
    }

    /// Die Sperre, die `key` trifft: seine eigene oder die eines
    /// Vorfahren. Eine Sperre auf `sandbox.env` gilt für jedes
    /// `sandbox.env.NAME`, eine auf `llm.servers` für jeden Eintrag darin
    /// (HUM-534). Liefert den gesperrten Pfad mit.
    #[must_use]
    pub fn covering(&self, key: &str) -> Option<(&str, &LockedValue)> {
        let mut path = key;
        loop {
            if let Some((locked, value)) = self.locks.get_key_value(path) {
                return Some((locked.as_str(), value));
            }
            path = path.rsplit_once('.')?.0;
        }
    }

    /// Alle Sperren, nach Schlüssel geordnet.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &LockedValue)> {
        self.locks
            .iter()
            .map(|(key, locked)| (key.as_str(), locked))
    }

    /// Ob nichts gesperrt ist.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.locks.is_empty()
    }
}

/// Lehnt das Schreiben eines gesperrten Schlüssels ab, gleich mit welchem
/// Wert: Ein gesperrter Wert ist in jedem Client nur lesbar.
///
/// # Errors
///
/// `AGENT_010`, dessen `why` Schlüssel, Wert und Vorgabe nennt; ohne `fix`,
/// denn um eine Vorgabe führt kein Weg herum.
pub fn refuse_locked_write(key: &str, locks: &PolicyLocks) -> Result<(), Diagnostic> {
    let Some((path, locked)) = locks.covering(key) else {
        return Ok(());
    };
    let what = if path == key {
        format!("{key} to {:?}", locked.value)
    } else {
        format!("{path}, and with it {key}")
    };
    Err(Diagnostic::builder(AGENT_010, Severity::Blocking)
        .why(format!(
            "the company policy {} fixes {what}; writing it is refused",
            locked.by
        ))
        .build())
}

/// [`refuse_locked_write`] für jeden Schlüssel, den ein Schreibweg berühren
/// würde, bevor er `config.toml` oder den Schlüsselbund anfasst: der eine
/// Eingang für `LlmServers`, `LlmKey`, `Egress` und `Agents` (HUM-534).
///
/// # Errors
///
/// `AGENT_010` für den ersten gesperrten Schlüssel.
pub fn refuse_locked_keys<I, K>(keys: I, locks: &PolicyLocks) -> Result<(), Diagnostic>
where
    I: IntoIterator<Item = K>,
    K: AsRef<str>,
{
    if locks.is_empty() {
        return Ok(());
    }
    keys.into_iter()
        .try_for_each(|key| refuse_locked_write(key.as_ref(), locks))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::{PolicyLocks, refuse_locked_write};

    #[test]
    fn a_locked_key_is_refused_with_the_code_of_the_lock_and_others_pass() {
        let locks = PolicyLocks::none().with("hold.ask_mode", "ui", "/etc/humanitl/policy.toml");
        let refused = refuse_locked_write("hold.ask_mode", &locks).unwrap_err();
        assert_eq!(refused.code.as_str(), "AGENT_010");
        assert!(
            refused.why.contains("/etc/humanitl/policy.toml"),
            "{}",
            refused.why
        );
        refuse_locked_write("hold.timeout_secs", &locks).unwrap();
        refuse_locked_write("hold.ask_mode", &PolicyLocks::none()).unwrap();
    }

    /// Eine Sperre auf einer Tabelle gilt für jeden Schlüssel darin (Review
    /// HUM-534). Mutationsbeweis: Fragte `covering` nur den Schlüssel selbst,
    /// ginge `sandbox.env.CURL_CA_BUNDLE` durch.
    #[test]
    fn a_lock_on_an_ancestor_covers_its_children() {
        let locks = PolicyLocks::none().with("sandbox.env", "{}", "/etc/humanitl/policy.toml");
        let refused = refuse_locked_write("sandbox.env.CURL_CA_BUNDLE", &locks).unwrap_err();
        assert_eq!(refused.code.as_str(), "AGENT_010");
        assert!(refused.why.contains("sandbox.env"), "{}", refused.why);
        refuse_locked_write("sandbox.envx", &locks).unwrap();
        refuse_locked_write("sandbox.mode", &locks).unwrap();
        assert!(locks.covering("sandbox").is_none());
    }
}
