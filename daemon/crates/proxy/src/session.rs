//! Was eine Sitzung an der laufenden Proxy-Sitzung ändern darf.
//!
//! Der Daemon startet einmal und löst dabei seine Konfiguration auf. Die
//! Sitzung, die `humanitl run` startet, kommt danach und bringt ein eigenes
//! Profil, einen eigenen Frage-Modus und einen eigenen Sprachmodell-Endpunkt
//! mit (HUM-067). Der Proxy läuft zu diesem Zeitpunkt schon; Handler,
//! Pipeline und Meta-Endpunkt sind gebaut und hängen an einer Sitzung.
//!
//! Damit die drei Werte trotzdem gelten, liest der Proxy sie nicht mehr als
//! Kopie beim Bau, sondern hier — an genau einer Stelle, die der
//! Sandbox-Dienst beim Start beschreibt. Das ist der Unterschied zwischen
//! „der Wert von damals" und „der Wert, der gilt".
//!
//! # Was hier nicht steht
//!
//! Die Regeln. Sie haben mit dem [`RulesStore`](crate::rules_store::RulesStore)
//! schon einen Ort, der sich ändern lässt und Zuhörer benachrichtigt; eine
//! zweite Stelle daneben wäre eine zweite Wahrheit über denselben Regelsatz.
//! Alles andere aus der Konfiguration ändert sich innerhalb eines Daemons
//! nicht: Wer die Grenzen, die Detektoren oder den Resolver anders will,
//! startet den Daemon neu.

use std::sync::{PoisonError, RwLock};
use std::time::Duration;

use humanitl_config::AskMode;

/// Die drei Werte, die eine Sitzung mitbringt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionState {
    /// Wo gefragt wird (`hold.ask_mode`).
    pub ask_mode: AskMode,
    /// Wie lange eine gehaltene Anfrage wartet.
    ///
    /// Schon umgerechnet: Bei [`AskMode::None`] ist die Frist null, und die
    /// Warteschlange nimmt den Fluss mit abgelaufener Frist an. Die
    /// Umrechnung steht an einer Stelle ([`SessionState::for_config`]), damit
    /// nicht zwei Aufrufer verschiedene Antworten auf dieselbe Frage geben.
    pub hold_timeout: Duration,
    /// Der Endpunkt des Sprachmodells als `host:port`, falls einer gilt.
    pub llm: Option<String>,
}

impl SessionState {
    /// Der Zustand, den diese Konfiguration ergibt.
    ///
    /// `ask_mode` bestimmt die Frist: Ohne Frage ist sie null, sonst
    /// `hold.timeout_secs`.
    #[must_use]
    pub fn for_config(ask_mode: AskMode, timeout_secs: u64, llm: Option<String>) -> Self {
        Self {
            ask_mode,
            hold_timeout: match ask_mode {
                AskMode::None => Duration::ZERO,
                AskMode::Ui | AskMode::Terminal => Duration::from_secs(timeout_secs),
            },
            llm,
        }
    }
}

/// Der geteilte Stand einer Sitzung: einer schreibt, viele lesen.
///
/// Gelesen wird je gehaltenem Fluss und je Anfrage an `humanitl.internal`,
/// also selten; geschrieben genau beim Start einer Sitzung. Ein `RwLock`
/// reicht dafür, und er hält die drei Werte zusammen — ein Frage-Modus ohne
/// die Frist, die zu ihm gehört, wäre ein Zustand, den niemand gewählt hat.
#[derive(Debug)]
pub struct SessionSettings {
    state: RwLock<SessionState>,
}

impl SessionSettings {
    /// Die Einstellungen, mit denen der Daemon startet.
    #[must_use]
    pub fn new(state: SessionState) -> Self {
        Self {
            state: RwLock::new(state),
        }
    }

    /// Der Stand, der gerade gilt.
    #[must_use]
    pub fn get(&self) -> SessionState {
        self.state.read().map_or_else(
            |poisoned| poisoned.into_inner().clone(),
            |state| state.clone(),
        )
    }

    /// Wie lange eine gehaltene Anfrage jetzt wartet.
    #[must_use]
    pub fn hold_timeout(&self) -> Duration {
        self.get().hold_timeout
    }

    /// Setzt den Stand für die Sitzung, die gerade startet.
    pub fn set(&self, state: SessionState) {
        let mut slot = self.state.write().unwrap_or_else(PoisonError::into_inner);
        *slot = state;
    }
}

impl Default for SessionSettings {
    /// Die Vorgaben der Konfiguration, ohne Sprachmodell.
    fn default() -> Self {
        let config = humanitl_config::HoldConfig::default();
        Self::new(SessionState::for_config(
            config.ask_mode,
            config.timeout_secs,
            None,
        ))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::time::Duration;

    use humanitl_config::AskMode;

    use super::{SessionSettings, SessionState};

    #[test]
    fn without_a_question_the_deadline_is_zero() {
        let state = SessionState::for_config(AskMode::None, 300, None);
        assert_eq!(state.hold_timeout, Duration::ZERO);
    }

    #[test]
    fn with_a_question_the_deadline_is_the_configured_one() {
        for mode in [AskMode::Ui, AskMode::Terminal] {
            let state = SessionState::for_config(mode, 42, None);
            assert_eq!(state.hold_timeout, Duration::from_secs(42));
        }
    }

    #[test]
    fn a_session_replaces_all_three_values_at_once() {
        let settings = SessionSettings::new(SessionState::for_config(AskMode::Ui, 300, None));
        settings.set(SessionState::for_config(
            AskMode::None,
            300,
            Some("model.lan:11434".to_owned()),
        ));

        let state = settings.get();
        assert_eq!(state.ask_mode, AskMode::None);
        assert_eq!(state.hold_timeout, Duration::ZERO);
        assert_eq!(state.llm.as_deref(), Some("model.lan:11434"));
    }
}
