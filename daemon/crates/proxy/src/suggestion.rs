//! Ein Vorschlag, der ein Geheimnis der Masken trüge, bleibt im Daemon
//! (HUM-271).
//!
//! Das `why` eines Befunds geht durch die Masken der Aufzeichnung
//! (`Recorder::masked_why`, HUM-256). Der Vorschlag daneben lässt sich so
//! nicht behandeln: `LIMIT_001` und `PROXY_008` bauen eine Regel aus Host,
//! Port und Pfadpräfix der Anfrage, und steht der Zugangsschlüssel des Modells
//! als Label des Hosts oder als Pfadsegment darin, wäre eine maskierte Regel
//! eine andere Regel. Sie träfe die Anfrage nicht mehr, aus der sie entstand,
//! und der Knopf täte etwas anderes, als er zeigt (`backlog/CONVENTIONS.md`
//! 4.10). Dasselbe gilt für einen Befehl zum Kopieren mit einem Pfad darin.
//!
//! Deshalb fällt ein solcher Vorschlag ganz weg, und das `why` sagt in einem
//! Satz, warum. Ein Vorschlag ohne Geheimnis bleibt Byte für Byte, wie er war.
//! Angewandt wird das auf jeden Befund im Ereignisstrom des Proxys, im
//! Trichter `HoldQueue::publish` vor Registry, Aufzeichnung und Strom; so
//! verlässt ein solcher Schlüssel den Daemon auf keinem dieser Wege und landet
//! nicht in der Datei. Die Sperre `Flow::send_refusal` steht schon vor diesem
//! Trichter am Flow; der Handler legt sie deshalb vorher durch
//! `HoldQueue::withhold_secret_fix`. Befunde außerhalb des Ereignisstroms
//! (etwa `SANDBOX_022`, die Zusammenfassung einer Sitzung) deckt das nicht ab.

use humanitl_core::{Diagnostic, FixAction, SecretMasks};

/// Der Satz, den das `why` bekommt, wenn der Vorschlag wegfällt.
pub const WITHHELD_SENTENCE: &str = "Humanitl offers no fix here: the suggestion would contain a \
     secret that Humanitl masks, such as the access key of the model in the host or path of the \
     request, and with the secret masked it would do something other than it shows. Write it by \
     hand if you still need it.";

/// Nimmt `diagnostic` den Vorschlag, wenn er ein Geheimnis aus `masks` trägt,
/// und hängt [`WITHHELD_SENTENCE`] an sein `why`.
///
/// Gibt zurück, ob der Vorschlag wegfiel. Ohne Vorschlag oder ohne Geheimnis
/// darin bleibt der Befund unverändert.
pub fn withhold_secret_fix(diagnostic: &mut Diagnostic, masks: &SecretMasks) -> bool {
    let Some(fix) = diagnostic.fix.as_ref() else {
        return false;
    };
    if !fix_texts(fix).iter().any(|text| masks.appears_in(text)) {
        return false;
    }
    diagnostic.fix = None;
    let why = diagnostic.why.trim_end();
    diagnostic.why = if why.is_empty() {
        WITHHELD_SENTENCE.to_owned()
    } else {
        format!("{why} {WITHHELD_SENTENCE}")
    };
    true
}

/// Jeder Text, den ein Vorschlag hinausträgt.
///
/// Eine Regel zählt mit jedem Feld, das Text trägt (Host, Pfadmuster,
/// Pfadpräfixe, Notiz), und dazu in der Form von `rules.yaml`, in der sie in
/// die Aufzeichnung und in die Datei des Nutzers geht: Was dort steht, steht
/// auch hier zur Prüfung.
fn fix_texts(fix: &FixAction) -> Vec<String> {
    match fix {
        FixAction::SetEnv { key, value } | FixAction::ChangeSetting { key, value } => {
            vec![key.clone(), value.clone()]
        }
        FixAction::AddRule(rule) => {
            let mut texts = vec![rule.matcher.host.to_string()];
            texts.extend(rule.matcher.path.as_ref().map(ToString::to_string));
            texts.extend(rule.matcher.path_prefixes.iter().cloned());
            texts.extend(rule.note.iter().cloned());
            texts.push(humanitl_rules::serialize_rules(
                &humanitl_rules::RuleSet::from_rules([(**rule).clone()]),
            ));
            texts
        }
        FixAction::InstallService => Vec::new(),
        FixAction::CopyCommand(text) | FixAction::OpenUrl(text) => vec![text.clone()],
        FixAction::RemountReadOnly(path) => vec![path.to_string_lossy().into_owned()],
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_core::diagnostics::codes::LIMIT_001;
    use humanitl_core::{SecretMasks, Severity};

    use super::*;

    const KEY: &str = "sk-live-0123456789abcdef";

    #[test]
    fn a_command_without_a_secret_stays_and_one_with_it_goes() {
        let masks = SecretMasks::default();
        masks.add(KEY);
        let plain = Diagnostic::builder(LIMIT_001, Severity::Warning)
            .why("why.")
            .fix(FixAction::CopyCommand(
                "curl -sS https://a.example/v1".to_owned(),
            ))
            .build();
        let mut kept = plain.clone();
        assert!(!withhold_secret_fix(&mut kept, &masks));
        assert_eq!(kept, plain);

        let mut leaking = Diagnostic::builder(LIMIT_001, Severity::Warning)
            .why("why.")
            .fix(FixAction::CopyCommand(format!(
                "curl -sS https://a.example/{KEY}/v1"
            )))
            .build();
        assert!(withhold_secret_fix(&mut leaking, &masks));
        assert_eq!(leaking.fix, None);
        assert_eq!(leaking.why, format!("why. {WITHHELD_SENTENCE}"));
    }
}
