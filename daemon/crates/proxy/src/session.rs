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

use std::sync::{Arc, PoisonError, RwLock};
use std::time::Duration;

use humanitl_config::AskMode;

use crate::llm_warnings::LlmWarningMemory;

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
///
/// Dazu gehört das Gedächtnis der Warnung `LLM_005` (HUM-354): Es gilt für
/// einen Lauf, und [`SessionSettings::set`] ist die eine Stelle, die vom Start
/// eines Laufs erfährt.
#[derive(Debug)]
pub struct SessionSettings {
    state: RwLock<SessionState>,
    llm_warnings: Arc<LlmWarningMemory>,
}

impl SessionSettings {
    /// Die Einstellungen, mit denen der Daemon startet.
    #[must_use]
    pub fn new(state: SessionState) -> Self {
        Self {
            state: RwLock::new(state),
            llm_warnings: Arc::new(LlmWarningMemory::new()),
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
    ///
    /// Der Sandbox-Dienst ruft das bei jedem Start eines Laufs. Ein neuer Lauf
    /// ist ein neues Gespräch mit dem Modell, deshalb vergisst `LLM_005` hier,
    /// wovor es schon gewarnt hat: Eine Warnung, die im letzten Lauf niemand
    /// gesehen hat, darf im nächsten nicht schweigen.
    pub fn set(&self, state: SessionState) {
        let mut slot = self.state.write().unwrap_or_else(PoisonError::into_inner);
        *slot = state;
        drop(slot);
        self.llm_warnings.reset();
    }

    /// Das Gedächtnis der Warnung `LLM_005`, das mit jedem Lauf neu beginnt.
    ///
    /// [`RulesPipeline::with_warnings`](crate::pipeline::RulesPipeline::with_warnings)
    /// nimmt genau diesen Wert.
    #[must_use]
    pub fn llm_warnings(&self) -> Arc<LlmWarningMemory> {
        Arc::clone(&self.llm_warnings)
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

    #[test]
    fn a_new_run_resets_the_llm_warning_memory() {
        // Rot, sobald `set` das Gedächtnis nicht mehr leert: Dann schwiege
        // eine Warnung, die im letzten Lauf niemand gesehen hat, bis zum
        // Neustart des Daemons (HUM-354).
        use humanitl_core::{Finding, FindingKind, FindingLocation, SessionId, Tier};

        let settings = SessionSettings::default();
        let memory = settings.llm_warnings();
        let session = SessionId::new();
        let value = "ghp_A1b2C3d4E5f6G7h8I9j0K1l2M3n4O5p6Q7r8";
        let finding = Finding::new(
            FindingKind::ApiKey("github".to_owned()),
            0..value.len(),
            FindingLocation::Body,
            Tier::Checksum,
            value,
        );
        let endpoint = "127.0.0.1:8080";
        assert_eq!(
            memory
                .observe(session, endpoint, std::slice::from_ref(&finding))
                .new,
            1
        );
        assert_eq!(
            memory
                .observe(session, endpoint, std::slice::from_ref(&finding))
                .new,
            0
        );
        settings.set(SessionState::for_config(AskMode::Ui, 300, None));
        assert_eq!(
            memory
                .observe(session, endpoint, std::slice::from_ref(&finding))
                .new,
            1
        );
    }
}
