//! Ob die Durchreiche eines Adapters eine Anfrage durchlässt, gefragt bei
//! der echten Regel-Engine (HUM-568). Geteilt von den Adapter-Tests, die
//! Methode und Pfad paarweise prüfen.

use humanitl_core::rule::{Action, Rule};
use humanitl_core::{HostName, Method, Scheme, SessionId, Upgrade};
use humanitl_rules::{RequestKey, RuleSet, Verdict};

/// Ob eine der `rules`, als mitgelieferte Gruppe geladen, `method` an `url`
/// erlaubt; `upgrade` für einen WebSocket.
pub fn passes(rules: &[Rule], method: &Method, url: &str, upgrade: Option<Upgrade>) -> bool {
    let url = url::Url::parse(url).unwrap();
    let host = HostName::parse(url.host_str().unwrap()).unwrap();
    let path = url.query().map_or_else(
        || url.path().to_owned(),
        |query| format!("{}?{query}", url.path()),
    );
    let mut key = RequestKey::new(
        &host,
        method,
        &path,
        Scheme::parse(url.scheme()).unwrap(),
        url.port_or_known_default().unwrap(),
    );
    key.upgrade = upgrade;
    let mut set = RuleSet::new();
    set.add_bundled(rules.iter().cloned());
    matches!(
        set.evaluate(&key, jiff::Timestamp::now(), SessionId::nil()),
        Verdict::Matched {
            action: Action::Allow,
            ..
        }
    )
}
