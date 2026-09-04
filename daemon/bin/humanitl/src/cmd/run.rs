//! `humanitl run`: das Profil einer Sitzung auflösen und zeigen.
//!
//! Vollständig ist das Kommando erst mit HUM-067; dort startet es Daemon,
//! Sandbox und Agenten. Was hier schon steht, ist der Teil, der zu HUM-066
//! gehört: die Auswahl des Profils über [`humanitl_config::resolve()`] und die
//! Auskunft, welche Sitzung dabei herauskommt.
//!
//! Das ist mehr als eine Vorschau. `run` ist der Weg, auf dem ein
//! Projekt-Profil zum ersten Mal gelesen wird; ein Profil, das Host-Pfade
//! einhängen will oder einen gesperrten Schlüssel setzt, verweigert hier den
//! Start mit `CONFIG_003`, bevor irgendetwas läuft. Genau das prüft das
//! Akzeptanzkriterium des Issues.
//!
//! Gestartet wird nichts, und das Kommando behauptet es auch nicht: Es endet
//! mit `CLI_003` und nennt das Issue, das den Start bringt. Ein Exit 0 hier
//! hieße für jedes Skript „der Agent läuft", und `stdout` bleibt leer, weil es
//! kein Ergebnis gibt. Die aufgelöste Sitzung steht unter `-v` auf `stderr`.

use humanitl_config::{Origin, Resolved};

use crate::cli::RunArgs;
use crate::cmd::{Context, Failure, not_yet_failure};

/// Führt `humanitl run` aus.
///
/// # Errors
///
/// `CONFIG_001` bis `CONFIG_003`, wenn das Profil nicht lädt, und `CLI_003`,
/// solange der Start selbst fehlt (HUM-067).
pub fn run(ctx: &Context, args: &RunArgs) -> Result<u8, Failure> {
    let resolved = ctx.config()?;
    ctx.render.detail(&session_lines(&resolved, args));
    Err(not_yet_failure("humanitl run", "HUM-067"))
}

/// Die aufgelöste Sitzung als Text, eine Zeile je Aussage.
fn session_lines(resolved: &Resolved, args: &RunArgs) -> String {
    let config = &resolved.config;
    let mut lines = vec![format!("profiles: {}", chain(resolved))];
    lines.push(format!("ask mode: {:?}", config.hold.ask_mode));
    lines.push(format!("hold timeout: {} s", config.hold.timeout_secs));
    lines.push(format!("sandbox profile: {}", config.sandbox.profile));
    lines.push(format!("work mode: {:?}", config.sandbox.work_mode));
    lines.push(format!(
        "work dir: {}",
        config.sandbox.work_dir.as_ref().map_or_else(
            || "the current directory".to_owned(),
            |dir| dir.display().to_string()
        )
    ));
    lines.push(format!("agent: {}", config.agent.adapter));
    lines.push(format!(
        "llm endpoint: {}",
        config
            .llm
            .endpoint
            .as_ref()
            .map_or_else(|| "-".to_owned(), ToString::to_string)
    ));
    lines.push(format!("rule files: {}", rule_files(resolved).join(", ")));
    lines.push(format!("profile rules: {}", inline_rules(resolved)));
    if !args.cmd.is_empty() {
        lines.push(format!(
            "command: {}",
            args.cmd
                .iter()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join(" ")
        ));
    }
    lines.join("\n")
}

/// Die Profil-Kette als eine Zeile.
fn chain(resolved: &Resolved) -> String {
    let chain: Vec<String> = resolved
        .profile_chain()
        .iter()
        .map(Origin::to_string)
        .collect();
    if chain.is_empty() {
        "none".to_owned()
    } else {
        chain.join(" then ")
    }
}

/// Die Regeldateien aller beteiligten Profile, schon aufgelöst.
fn rule_files(resolved: &Resolved) -> Vec<String> {
    let files: Vec<String> = resolved
        .profiles
        .iter()
        .flat_map(humanitl_config::Profile::rule_files)
        .map(|path| path.display().to_string())
        .collect();
    if files.is_empty() {
        vec!["-".to_owned()]
    } else {
        files
    }
}

/// Wie viele Regeln die Profile selbst mitbringen.
fn inline_rules(resolved: &Resolved) -> usize {
    resolved
        .profiles
        .iter()
        .map(|profile| profile.rules.inline.len())
        .sum()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_config::{Env, ProfileSelection, resolve};

    use super::{chain, inline_rules, session_lines};
    use crate::cli::RunArgs;

    fn resolved(name: &str) -> humanitl_config::Resolved {
        let empty = tempfile::tempdir().expect("tempdir");
        let env = Env::from_pairs([
            ("HOME", empty.path().display().to_string()),
            (
                "XDG_CONFIG_HOME",
                empty.path().join("cfg").display().to_string(),
            ),
        ]);
        resolve(&ProfileSelection::named(name), None, &env, &[]).expect("the profile resolves")
    }

    #[test]
    fn the_lines_name_the_chain_and_the_session() {
        let resolved = resolved("llm-only");
        let text = session_lines(&resolved, &RunArgs { cmd: Vec::new() });

        assert!(text.contains("profile builtin default"), "{text}");
        assert!(text.contains("profile builtin llm-only"), "{text}");
        assert!(text.contains("ask mode: None"), "{text}");
        assert_eq!(inline_rules(&resolved), 1);
        assert_eq!(
            chain(&resolved),
            "profile builtin default then profile builtin llm-only"
        );
    }
}
