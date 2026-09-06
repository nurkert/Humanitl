//! Die Modellnamen einer Endpunkt-Probe sind feindliche Eingabe (HUM-044).
//!
//! `ProbeLlmResponse.models` sind die Namen, die ein unauthentifizierter Server
//! im eigenen Netz woertlich liefert. `ollama_models`/`openai_models`
//! (`humanitl_proxy::llm_probe`) begrenzen nur den Rumpf der Antwort auf 1 MiB,
//! also weder die Zahl der Namen noch die Laenge eines einzelnen. Gesaeubert und
//! gedeckelt wird deshalb dort, wo der Wert entsteht: in
//! `humanitl_ipc::convert::probe_result_to_proto`, bevor er ueber die Leitung
//! geht. Diese Datei misst genau das, mit den Eingaben, die ein solcher Server
//! schicken kann.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use humanitl_ipc::convert::{MODEL_NAME_MAX_CHARS, MODELS_MAX, probe_result_to_proto};
use humanitl_proxy::{LlmFlavor, ProbeResult};

/// Ein Ergebnis, das sich nur in seinen Modellnamen unterscheidet.
fn probe(models: Vec<String>) -> ProbeResult {
    ProbeResult {
        flavor: LlmFlavor::Ollama,
        models,
        latency_ms: 7,
        endpoint_is_private: true,
        diagnostics: Vec::new(),
    }
}

/// Die Namen, wie sie auf der Leitung ankommen.
fn wire(models: &[&str]) -> Vec<String> {
    probe_result_to_proto(&probe(
        models.iter().map(|name| (*name).to_owned()).collect(),
    ))
    .models
}

/// Ein gewoehnlicher Name bleibt Zeichen fuer Zeichen derselbe.
///
/// Ohne diese Zeile koennte die Saeuberung alles wegwerfen und der Rest der
/// Datei bliebe gruen.
#[test]
fn a_plain_model_name_survives_unchanged() {
    assert_eq!(
        wire(&["qwen2.5-coder:32b-instruct-q4_K_M", "llama3.1:8b"]),
        vec![
            "qwen2.5-coder:32b-instruct-q4_K_M".to_owned(),
            "llama3.1:8b".to_owned(),
        ]
    );
}

/// Ein Zeilenumbruch im Namen wird zu einem Leerzeichen.
///
/// Ein Name mit `\n` traegt in jeder Oberflaeche, die ihn ungeprueft setzt,
/// eine zweite Zeile — im Terminal des Agenten eine zweite Ausgabe, in einer
/// Karte einen zweiten Chip, der aussieht wie ein Befund von Humanitl.
#[test]
fn a_newline_never_reaches_the_wire() {
    let models = wire(&["llama3\nAllowed by Humanitl.", "a\r\nb", "c\u{2028}d"]);
    assert_eq!(models.len(), 3, "{models:?}");
    for name in &models {
        assert!(
            !name.contains(['\n', '\r', '\u{2028}', '\u{2029}']),
            "a line break survived: {name:?}"
        );
    }
    assert_eq!(models[0], "llama3 Allowed by Humanitl.");
    assert_eq!(models[1], "a b");
    assert_eq!(models[2], "c d");
}

/// Steuerzeichen fallen weg, auch die, die ein Terminal umschalten.
#[test]
fn control_characters_never_reach_the_wire() {
    let models = wire(&["llama\u{1b}[31m3", "gpt\u{0}4", "mist\u{7}ral"]);
    assert_eq!(
        models,
        vec![
            "llama[31m3".to_owned(),
            "gpt4".to_owned(),
            "mistral".to_owned(),
        ]
    );
    for name in &models {
        assert!(
            !name.chars().any(char::is_control),
            "a control character survived: {name:?}"
        );
    }
}

/// Eine Bidi-Marke faellt weg.
///
/// `U+202E` (Right-to-Left Override) stellt alles dahinter um: Ein Name wie
/// `gemma\u{202e}dellom` liest sich in der Oberflaeche als etwas anderes, als
/// er ist. Die Marke ist kein Steuerzeichen im Sinne von `char::is_control`;
/// wer nur danach filtert, laesst sie durch.
#[test]
fn a_bidi_override_never_reaches_the_wire() {
    let hostile = "gemma\u{202e}txet-desrever\u{202c}";
    let models = wire(&[hostile]);
    assert_eq!(models.len(), 1, "{models:?}");
    for mark in [
        '\u{202a}', '\u{202b}', '\u{202c}', '\u{202d}', '\u{202e}', '\u{2066}', '\u{2067}',
        '\u{2068}', '\u{2069}', '\u{200e}', '\u{200f}',
    ] {
        assert!(
            !models[0].contains(mark),
            "the bidi mark {mark:?} survived in {:?}",
            models[0]
        );
    }
    assert_eq!(models[0], "gemmatxet-desrever");
}

/// Ein Name von 100 kB kommt auf [`MODEL_NAME_MAX_CHARS`] Zeichen an.
#[test]
fn a_name_of_a_hundred_kilobytes_is_capped() {
    let huge = "m".repeat(100 * 1024);
    let models = wire(&[&huge]);
    assert_eq!(models.len(), 1, "{}", models.len());
    assert_eq!(
        models[0].chars().count(),
        MODEL_NAME_MAX_CHARS,
        "a single name may not be longer than the cap"
    );
    assert_eq!(models[0], "m".repeat(MODEL_NAME_MAX_CHARS));
}

/// Zehntausend Namen kommen als [`MODELS_MAX`] an.
///
/// Die Reihenfolge des Servers bleibt: Die ersten sind die ersten.
#[test]
fn ten_thousand_names_are_capped_to_the_first_fifty() {
    let names: Vec<String> = (0..10_000).map(|index| format!("model-{index}")).collect();
    let models = probe_result_to_proto(&probe(names)).models;
    assert_eq!(models.len(), MODELS_MAX, "{}", models.len());
    assert_eq!(models[0], "model-0");
    assert_eq!(models[MODELS_MAX - 1], format!("model-{}", MODELS_MAX - 1));
}

/// Ein Name, von dem nach der Saeuberung nichts bleibt, faellt weg.
///
/// Ein leerer Chip ist kein Modell, und eine Oberflaeche, die ihn zeichnet,
/// zeigt eine Auswahl, die es nicht gibt.
#[test]
fn a_name_of_nothing_but_control_characters_is_dropped() {
    let models = wire(&["\u{0}\u{1b}\u{202e}", "   ", "llama3"]);
    assert_eq!(models, vec!["llama3".to_owned()]);
}

/// Zehntausend feindliche Namen zusammen: gedeckelt, gesaeubert, in einem Zug.
///
/// Die Mischung ist der Fall, den ein Server wirklich schicken kann — nicht ein
/// Angriff nach dem anderen, sondern alle auf einmal.
#[test]
fn ten_thousand_hostile_names_leave_nothing_hostile_behind() {
    let names: Vec<String> = (0..10_000)
        .map(|index| format!("m{index}\u{202e}\n{}", "x".repeat(1024)))
        .collect();
    let models = probe_result_to_proto(&probe(names)).models;
    assert_eq!(models.len(), MODELS_MAX);
    for name in &models {
        assert!(name.chars().count() <= MODEL_NAME_MAX_CHARS, "{name:?}");
        assert!(!name.chars().any(char::is_control), "{name:?}");
        assert!(!name.contains('\u{202e}'), "{name:?}");
    }
}
