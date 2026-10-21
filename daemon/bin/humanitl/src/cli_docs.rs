//! `docs/reference/cli.md` wird aus der `clap`-Struktur erzeugt (HUM-059).
//!
//! Die Seite ist damit nie veraltet: Wer ein Unterkommando, einen Schalter
//! oder einen Hilfetext ändert, lässt `cargo xtask docs` laufen (oder diesen
//! Test mit `UPDATE_CLI_DOCS=1`) und legt die Änderung mit ins Commit. Ohne
//! die Variable vergleicht der Test die Datei auf der Platte mit dem, was er
//! erzeugt, und fällt bei jeder Abweichung; `cargo xtask docs --check` und
//! `cargo test --workspace` in der CI fahren genau diesen Vergleich.
//!
//! Gerendert wird der lange Hilfetext jedes Befehls, wie `--help` ihn zeigt,
//! in Deklarationsreihenfolge und rekursiv. Die Konfigurations-Flags aus dem
//! Schema (`cli::config_args`) stehen nicht unter jedem Unterkommando, sondern
//! einmal als Tabelle: Sie sind global, und hundertfach wiederholt würden sie
//! die Seite unlesbar machen. Ihre Bedeutung steht in `docs/CONFIG.md`.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use clap::Command;

use crate::cli;

/// Die erzeugte Datei, vom Repository aus gesehen.
pub const OUTPUT: &str = "docs/reference/cli.md";

/// Ort der erzeugten Datei.
fn docs_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(OUTPUT)
}

/// Die Seite als Markdown.
///
/// Deterministisch: kein Wert aus der Umgebung, keine Terminalbreite (ohne
/// das Merkmal `wrap_help` bricht `clap` fest bei 100 Zeichen um), und die
/// Schalter tragen keine `env`-Angabe, deren aktueller Wert in die Hilfe
/// rutschen könnte.
pub fn render() -> String {
    let mut out = String::new();
    out.push_str("# CLI reference\n\n");
    out.push_str("Applies to: Humanitl 0.1\n\n");
    out.push_str(
        "<!-- Generated from daemon/bin/humanitl/src/cli.rs by `cargo xtask docs`.\n     \
         Do not edit by hand: change the clap definitions and rerun the task. -->\n\n",
    );
    out.push_str(
        "`humanitl` is a thin client of the daemon (ADR-018): every subcommand calls an RPC of\n\
         `humanitld` or reads a local file, and formats the answer. This page is the `--help` text\n\
         of every command, generated from the code. What the commands do in detail, step by\n\
         step, is described in [`docs/cli.md`](../cli.md); which RPC each subcommand serves is in\n\
         [`parity.md`](parity.md).\n\n",
    );

    let mut root = cli::command();
    root.build();

    out.push_str("## Commands\n\n");
    let mut index = Vec::new();
    collect(&root, &mut index);
    for (path, about) in &index {
        let _ = writeln!(out, "- [`{path}`](#{}) {about}", anchor(path));
    }
    out.push('\n');

    out.push_str("## Configuration flags\n\n");
    out.push_str(
        "Every leaf key of the configuration schema is also a global flag: `hold.timeout_secs`\n\
         becomes `--hold-timeout-secs`, and the environment variable `HUMANITL_HOLD__TIMEOUT_SECS`.\n\
         A flag wins over the environment, which wins over profiles and `config.toml`. Types,\n\
         defaults and the meaning of each key are in [`docs/CONFIG.md`](../CONFIG.md); the table\n\
         below lists the flag names and their short aliases.\n\n",
    );
    out.push_str("| Flag | Alias | Value | Key |\n|---|---|---|---|\n");
    for flag in cli::flags() {
        let alias = flag
            .short
            .map_or_else(|| "-".to_owned(), |short| format!("`--{short}`"));
        let value = flag.allowed.as_ref().map_or_else(
            || {
                if flag.boolean {
                    "`true`, `false`".to_owned()
                } else {
                    format!("`{}`", flag.value_name)
                }
            },
            |allowed| {
                allowed
                    .iter()
                    .map(|value| format!("`{value}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            },
        );
        let _ = writeln!(
            out,
            "| `--{}` | {alias} | {value} | `{}` |",
            flag.long, flag.path
        );
    }
    out.push('\n');

    render_command(&root, &mut out);
    out
}

/// Alle Befehle unterhalb von `cmd`, als Pfad und erste Zeile des Hilfetexts.
fn collect(cmd: &Command, into: &mut Vec<(String, String)>) {
    for sub in visible_subcommands(cmd) {
        let about = sub.get_about().map(ToString::to_string).unwrap_or_default();
        into.push((bin_name(sub), about));
        collect(sub, into);
    }
}

/// Die Unterkommandos, die `--help` zeigt, ohne `help` selbst.
fn visible_subcommands(cmd: &Command) -> impl Iterator<Item = &Command> {
    cmd.get_subcommands()
        .filter(|sub| !sub.is_hide_set() && sub.get_name() != "help")
}

/// Der volle Aufrufpfad, zum Beispiel `humanitl sandbox check`.
fn bin_name(cmd: &Command) -> String {
    cmd.get_bin_name()
        .map_or_else(|| cmd.get_name().to_owned(), ToOwned::to_owned)
}

/// Der Anker einer Überschrift `## humanitl sandbox check` nach GitHub.
fn anchor(path: &str) -> String {
    path.chars()
        .filter_map(|c| match c {
            ' ' => Some('-'),
            '-' | '_' => Some(c),
            c if c.is_ascii_alphanumeric() => Some(c.to_ascii_lowercase()),
            _ => None,
        })
        .collect()
}

/// Ein Abschnitt je Befehl: Überschrift und langer Hilfetext.
fn render_command(cmd: &Command, out: &mut String) {
    let _ = writeln!(out, "## {}\n", bin_name(cmd));
    out.push_str("```text\n");
    out.push_str(&help_text(cmd));
    out.push_str("```\n\n");
    for sub in visible_subcommands(cmd) {
        render_command(sub, out);
    }
}

/// Der lange Hilfetext ohne die Konfigurations-Flags, ohne Farbe und ohne
/// Leerzeichen am Zeilenende.
fn help_text(cmd: &Command) -> String {
    let mut cmd = cmd.clone();
    for flag in cli::flags() {
        cmd = cmd.mut_arg(flag.path, |arg| arg.hide(true));
    }
    let rendered = cmd.render_long_help().to_string();
    let mut text = String::new();
    for line in rendered.lines() {
        text.push_str(line.trim_end());
        text.push('\n');
    }
    while text.ends_with("\n\n") {
        text.pop();
    }
    text
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::{anchor, docs_path, render};

    /// Die Datei auf der Platte ist die, die aus der `clap`-Struktur entsteht.
    #[test]
    fn cli_reference_in_sync() {
        let path = docs_path();
        let rendered = render();

        if std::env::var_os("UPDATE_CLI_DOCS").is_some() {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap_or_else(|err| panic!("{err}"));
            }
            if std::fs::read_to_string(&path).ok().as_deref() != Some(rendered.as_str()) {
                std::fs::write(&path, &rendered).unwrap_or_else(|err| panic!("{err}"));
            }
            return;
        }

        let current = std::fs::read_to_string(&path).unwrap_or_else(|err| {
            panic!(
                "{} is missing ({err}); run `cargo xtask docs`",
                path.display()
            )
        });
        assert!(
            current == rendered,
            "docs/reference/cli.md is stale; run `cargo xtask docs` and commit the result"
        );
    }

    /// Jedes Unterkommando, das `--help` kennt, steht mit Überschrift und
    /// Hilfetext auf der Seite, auch ein tief geschachteltes.
    #[test]
    fn every_subcommand_has_a_section() {
        let page = render();
        for path in [
            "humanitl run",
            "humanitl sandbox check",
            "humanitl rules test",
            "humanitl audit verify",
            "humanitl llm key set",
        ] {
            assert!(page.contains(&format!("\n## {path}\n")), "{path} missing");
            assert!(
                page.contains(&format!("(#{})", anchor(path))),
                "{path} not indexed"
            );
        }
        // Die Konfigurations-Flags stehen einmal in der Tabelle und nicht im
        // Hilfetext jedes Befehls.
        assert!(page.contains("| `--hold-timeout-secs` |"));
        assert!(
            !page.contains("--hold-timeout-secs <"),
            "config flag repeated in a help text"
        );
    }

    #[test]
    fn anchors_follow_github() {
        assert_eq!(anchor("humanitl llm key set"), "humanitl-llm-key-set");
        assert_eq!(anchor("humanitl rules dry-run"), "humanitl-rules-dry-run");
    }
}
