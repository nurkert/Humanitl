//! Ein Profil, das das Projekt-Profil mit `name` wählt, unter der Grenze des
//! Projekt-Profils.
//!
//! Die Wahl kommt aus dem Projektordner, also gilt für das gewählte Profil
//! dieselbe Grenze wie für das Projekt-Profil selbst: Ein Schlüssel mit
//! `x-project-scope = "denied"` (oder ein Pfad unter `sandbox.mounts`) bleibt
//! draußen, und von seinen Regeln gelten nur `block` und `ask`. Anders als im
//! Projekt-Profil bricht das den Start nicht ab: Das gewählte Profil ist ein
//! mitgeliefertes, sein Text gehört nicht dem Repository. Jedes übergangene
//! Stück bekommt einen eigenen Befund (`CONFIG_003`, Warning), der den
//! Schlüssel oder die Regel und das Profil nennt, und der Start läuft weiter.
//!
//! Wählt der Mensch dasselbe Profil selbst (`--profile`, das Profil der
//! Sitzung), gilt es ganz; dann setzt [`mod@crate::resolve`] keinen Namen in
//! [`crate::Sources::project_named`], und hier geschieht nichts.
//!
//! Ohne `FixAction`, wie bei jedem Befund an der Grenze des Projekt-Profils:
//! Ein Knopf trüge den Wunsch des Projekts mit einem Klick weiter. Wer das
//! Profil ganz will, wählt es selbst; das steht im Text.

use std::path::Path;

use humanitl_core::diagnostics::codes::CONFIG_003;
use humanitl_core::{Diagnostic, Severity};
use toml::Value as TomlValue;

use crate::alias;
use crate::load::is_mount_key;
use crate::profile::{ConfigOverlay, Profile};
use crate::schema;
use crate::scope::ProjectScope;

/// Die Aktionen, die eine Regel eines vom Projekt gewählten Profils haben
/// darf: Sie halten an oder fragen, sie lassen nie etwas durch.
const KEPT_ACTIONS: &[&str] = &["block", "ask"];

/// Legt die Grenze des Projekt-Profils an das Profil, das es gewählt hat.
///
/// Entfernt aus `profile` jeden Schlüssel, den das Projekt-Profil nicht setzen
/// dürfte, jede Regel außer `block` und `ask` und jede Regeldatei, und gibt je
/// entferntem Stück einen Befund zurück. `project` ist der Pfad des
/// Projekt-Profils, das die Wahl getroffen hat.
pub(crate) fn hold_to_project_scope(profile: &mut Profile, project: &Path) -> Vec<Diagnostic> {
    let name = profile.source.name();
    let origin = profile.source.origin();
    let mut notes = Vec::new();

    let mut kept = std::collections::BTreeMap::new();
    for (written_as, value) in profile.overlay.iter() {
        let path = alias::canonical(written_as).unwrap_or(written_as);
        if denied_for_a_project(path) {
            let key = if path == written_as {
                path.to_owned()
            } else {
                format!("{written_as} (the old name of {path})")
            };
            notes.push(ignored(format!(
                "{key} (from {origin}) is ignored: the project profile {} names the profile \
                 {name}, and a profile a project profile names may set only the keys the \
                 project profile itself may set. Choose the profile yourself with --profile \
                 {name} to apply it in full.",
                project.display()
            )));
            continue;
        }
        kept.insert(written_as.to_owned(), value.clone());
    }
    profile.overlay = ConfigOverlay::from_entries(kept);

    let inline = std::mem::take(&mut profile.rules.inline);
    for (index, rule) in inline.into_iter().enumerate() {
        let action = rule.get("action").and_then(TomlValue::as_str);
        if action.is_some_and(|action| KEPT_ACTIONS.contains(&action)) {
            profile.rules.inline.push(rule);
            continue;
        }
        notes.push(ignored(format!(
            "rule {} of [rules].inline (action {}, from {origin}) is ignored: the project \
             profile {} names the profile {name}, and a profile a project profile names may \
             only bring rules that block or ask. Choose the profile yourself with --profile \
             {name} to apply it in full.",
            index + 1,
            action.map_or_else(|| "missing".to_owned(), |action| format!("{action:?}")),
            project.display()
        )));
    }

    // Eine Regeldatei kann jede Aktion enthalten, und gelesen wird sie erst in
    // der Regel-Crate; hier lässt sich nicht prüfen, ob sie nur anhält oder
    // fragt. Sie bleibt deshalb ganz draußen.
    for file in std::mem::take(&mut profile.rules.files) {
        notes.push(ignored(format!(
            "the rule file {} of [rules].files (from {origin}) is ignored: the project \
             profile {} names the profile {name}, and a profile a project profile names may \
             only bring inline rules that block or ask. Choose the profile yourself with \
             --profile {name} to apply it in full.",
            file.display(),
            project.display()
        )));
    }
    notes
}

/// Ob das Projekt-Profil den Pfad nicht setzen dürfte.
///
/// Ein Pfad, den das Schema nicht kennt, ist hier nicht gesperrt: Er bleibt
/// im Profil, und das Laden lehnt ihn als unbekannten Schlüssel ab
/// (`CONFIG_002`), wie aus jedem anderen Profil.
fn denied_for_a_project(path: &str) -> bool {
    is_mount_key(path)
        || schema::field(path)
            .filter(|field| !field.group)
            .is_some_and(|field| field.project_scope == ProjectScope::Denied)
}

/// Der Befund für ein übergangenes Stück.
fn ignored(why: String) -> Diagnostic {
    Diagnostic::builder(CONFIG_003, Severity::Warning)
        .why(why)
        .build()
}
