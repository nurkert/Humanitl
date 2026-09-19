//! Die Paritäts-Tabelle `docs/reference/parity.md` (ADR-018, HUM-078).
//!
//! Vier Quellen, keine davon wird hier gepflegt:
//!
//! - die Service-Methoden des Vertrags, aus dem Descriptor, den `protox` aus
//!   `proto/` übersetzt (derselbe Weg wie `cargo xtask proto`);
//! - `PARITY` in `daemon/bin/humanitl/src/parity.rs`, RPC → Unterkommando;
//! - `parity` in `app/lib/core/parity.dart`, RPC → Ort in der Oberfläche;
//! - `daemon/xtask/parity_exempt.toml`, RPCs ohne sinnvolles Unterkommando,
//!   jede mit Begründung.
//!
//! Die beiden Tabellen in Rust und Dart werden als Text gelesen, Zeile für
//! Zeile mit einem festen Muster. Eine Zeile im Block, die weder leer noch
//! Kommentar ist und nicht passt, ist ein Fehler und wird nicht übersprungen:
//! Sonst verschwände ein Eintrag still aus der Tabelle, nur weil jemand ihn
//! anders umbrochen hat.
//!
//! Die Härte ist asymmetrisch wie in ADR-018: Eine RPC ohne CLI-Zeile und ohne
//! Ausnahme bricht den Lauf, eine RPC ohne UI-Zeile ist eine Warnung. Einträge,
//! die eine RPC nennen, die es nicht gibt, brechen ihn ebenfalls, in allen drei
//! Quellen; eine veraltete Zeile wäre sonst eine Lüge in der Tabelle.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::path::Path;

use protox::prost_reflect::DescriptorPool;
use protox::prost_reflect::prost_types::FileDescriptorSet;
use regex::Regex;
use serde::Deserialize;

/// Die CLI-Tabelle, relativ zur Wurzel des Repositories.
pub const CLI_SOURCE: &str = "daemon/bin/humanitl/src/parity.rs";
/// Die UI-Registry, relativ zur Wurzel des Repositories.
pub const UI_SOURCE: &str = "app/lib/core/parity.dart";
/// Die Ausnahmen, relativ zur Wurzel des Repositories.
pub const EXEMPT_SOURCE: &str = "daemon/xtask/parity_exempt.toml";
/// Die erzeugte Tabelle, relativ zur Wurzel des Repositories.
pub const OUTPUT: &str = "docs/reference/parity.md";

/// Befunde, die die Tabelle verhindern; jeder eine Zeile.
#[derive(Debug, PartialEq, Eq)]
pub struct Findings(pub Vec<String>);

impl fmt::Display for Findings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, finding) in self.0.iter().enumerate() {
            if i > 0 {
                writeln!(f)?;
            }
            write!(f, "{finding}")?;
        }
        Ok(())
    }
}

impl Error for Findings {}

/// Eine RPC, die bewusst kein Unterkommando hat.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Exempt {
    /// RPC-Name, `Service.Methode`.
    pub rpc: String,
    /// Warum die Kommandozeile hier nichts anbietet.
    pub reason: String,
}

/// Der Inhalt von `parity_exempt.toml`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExemptFile {
    #[serde(default)]
    exempt: Vec<Exempt>,
}

/// Alles, woraus die Tabelle entsteht, schon gelesen.
#[derive(Debug, Default)]
pub struct Sources {
    /// Die Methoden des Vertrags als `Service.Methode`.
    pub rpcs: Vec<String>,
    /// `(RPC, Unterkommando)` aus der CLI-Tabelle.
    pub cli: Vec<(String, String)>,
    /// `(RPC, Ort)` aus der UI-Registry.
    pub ui: Vec<(String, String)>,
    /// Die Ausnahmen.
    pub exempt: Vec<Exempt>,
}

/// Die fertige Tabelle und die Warnungen, die sie nicht verhindern.
#[derive(Debug)]
pub struct Table {
    /// Der Inhalt von `docs/reference/parity.md`.
    pub markdown: String,
    /// Je RPC ohne Ort in der Oberfläche eine Zeile.
    pub warnings: Vec<String>,
}

/// Die Methoden aller Services im Descriptor als `Service.Methode`, sortiert.
///
/// # Errors
///
/// Wenn der Descriptor sich nicht zu einem Pool fügt.
pub fn rpcs(fds: FileDescriptorSet) -> Result<Vec<String>, Box<dyn Error>> {
    let pool = DescriptorPool::from_file_descriptor_set(fds)?;
    let mut names: Vec<String> = pool
        .services()
        .flat_map(|service| {
            service
                .methods()
                .map(|method| format!("{}.{}", service.name(), method.name()))
                .collect::<Vec<_>>()
        })
        .collect();
    names.sort();
    Ok(names)
}

/// Liest die Zeilen eines Blocks zwischen `start` und `end` mit `entry`.
///
/// Der Block beginnt mit der ersten Zeile, die `start` enthält, und endet mit
/// der ersten Zeile danach, die nach Abzug von Leerraum genau `end` ist. Die
/// Anfangszeile muss mit `opener` enden: Ein Eintrag hinter der öffnenden
/// Klammer stünde sonst außerhalb jeder Prüfung.
fn parse_block(
    source: &str,
    text: &str,
    (start, opener): (&str, &str),
    end: &str,
    entry: &Regex,
    comment: &str,
) -> Result<Vec<(String, String)>, Findings> {
    let mut lines = text.lines().enumerate();
    let Some((index, first)) = lines.find(|(_, line)| line.contains(start)) else {
        return Err(Findings(vec![format!(
            "{source}: no line contains `{start}`"
        )]));
    };
    if !first.trim_end().ends_with(opener) {
        return Err(Findings(vec![format!(
            "{source}:{}: the line with `{start}` must end with `{opener}`; entries go on lines of their own",
            index + 1
        )]));
    }
    let mut pairs = Vec::new();
    let mut problems = Vec::new();
    for (index, line) in lines {
        let trimmed = line.trim();
        if trimmed == end {
            return if problems.is_empty() {
                Ok(pairs)
            } else {
                Err(Findings(problems))
            };
        }
        if trimmed.is_empty() || trimmed.starts_with(comment) {
            continue;
        }
        match entry.captures(line) {
            Some(caps) => pairs.push((caps[1].to_owned(), caps[2].to_owned())),
            None => problems.push(format!(
                "{source}:{}: not in the strict one-entry-per-line format: {trimmed}",
                index + 1
            )),
        }
    }
    problems.push(format!("{source}: the block never ends with `{end}`"));
    Err(Findings(problems))
}

/// Liest `PARITY` aus dem Text von `daemon/bin/humanitl/src/parity.rs`.
///
/// # Errors
///
/// Eine Zeile im Block, die nicht `("Humanitl.X", "wörter"),` ist, oder ein
/// Block ohne Anfang oder Ende.
pub fn parse_cli(text: &str) -> Result<Vec<(String, String)>, Findings> {
    let entry = Regex::new(
        r#"^\s*\("([A-Z][A-Za-z0-9]*\.[A-Z][A-Za-z0-9]*)",\s*"([a-z][a-z0-9 -]*)"\),\s*$"#,
    )
    .map_err(|error| Findings(vec![error.to_string()]))?;
    parse_block(
        CLI_SOURCE,
        text,
        ("pub static PARITY", "&["),
        "];",
        &entry,
        "//",
    )
}

/// Liest `parity` aus dem Text von `app/lib/core/parity.dart`.
///
/// # Errors
///
/// Eine Zeile im Block, die nicht `'Humanitl.X': 'pfad',` ist, oder ein
/// Block ohne Anfang oder Ende.
pub fn parse_ui(text: &str) -> Result<Vec<(String, String)>, Findings> {
    let entry = Regex::new(
        r"^\s*'([A-Z][A-Za-z0-9]*\.[A-Z][A-Za-z0-9]*)':\s*'([a-z][a-z0-9_]*(?:/[a-z][a-z0-9_]*)*)',\s*$",
    )
    .map_err(|error| Findings(vec![error.to_string()]))?;
    parse_block(
        UI_SOURCE,
        text,
        ("const parity = ", "{"),
        "};",
        &entry,
        "//",
    )
}

/// Liest `parity_exempt.toml`.
///
/// # Errors
///
/// Wenn die Datei kein gültiges TOML in der erwarteten Form ist.
pub fn parse_exempt(text: &str) -> Result<Vec<Exempt>, Findings> {
    toml::from_str::<ExemptFile>(text)
        .map(|file| file.exempt)
        .map_err(|error| Findings(vec![format!("{EXEMPT_SOURCE}: {error}")]))
}

/// Prüft die Quellen gegeneinander und erzeugt die Tabelle.
///
/// `ui_exists` sagt, ob es zu einem Ort der UI-Registry die Datei
/// `app/lib/<ort>.dart` gibt; in den Tests steht dort eine feste Antwort.
///
/// # Errors
///
/// Alle Befunde auf einmal: RPCs ohne CLI-Zeile und ohne Ausnahme, Einträge zu
/// RPCs, die es nicht gibt, Ausnahmen ohne Begründung oder mit CLI-Zeile,
/// doppelte Einträge und Orte ohne Datei.
pub fn build(sources: &Sources, ui_exists: impl Fn(&str) -> bool) -> Result<Table, Findings> {
    let known: BTreeSet<&str> = sources.rpcs.iter().map(String::as_str).collect();
    let mut problems = Vec::new();

    let mut cli: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for (rpc, entry) in &sources.cli {
        if !known.contains(rpc.as_str()) {
            problems.push(format!(
                "{CLI_SOURCE}: `{rpc}` is not a method of the contract"
            ));
        }
        if !cli.entry(rpc).or_default().insert(entry) {
            problems.push(format!("{CLI_SOURCE}: `{rpc}` → `{entry}` appears twice"));
        }
    }

    let mut ui: BTreeMap<&str, &str> = BTreeMap::new();
    for (rpc, place) in &sources.ui {
        if !known.contains(rpc.as_str()) {
            problems.push(format!(
                "{UI_SOURCE}: `{rpc}` is not a method of the contract"
            ));
        }
        if ui.insert(rpc, place).is_some() {
            problems.push(format!("{UI_SOURCE}: `{rpc}` appears twice"));
        }
        if !ui_exists(place) {
            problems.push(format!(
                "{UI_SOURCE}: `{rpc}` points at app/lib/{place}.dart, which does not exist"
            ));
        }
    }

    let mut exempt: BTreeMap<&str, &str> = BTreeMap::new();
    for Exempt { rpc, reason } in &sources.exempt {
        if !known.contains(rpc.as_str()) {
            problems.push(format!(
                "{EXEMPT_SOURCE}: `{rpc}` is not a method of the contract"
            ));
        }
        if reason.trim().is_empty() {
            problems.push(format!("{EXEMPT_SOURCE}: `{rpc}` has no reason"));
        }
        if cli.contains_key(rpc.as_str()) {
            problems.push(format!(
                "{EXEMPT_SOURCE}: `{rpc}` is exempt but has a CLI entry in {CLI_SOURCE}; drop one of them"
            ));
        }
        if exempt.insert(rpc, reason).is_some() {
            problems.push(format!("{EXEMPT_SOURCE}: `{rpc}` appears twice"));
        }
    }

    for rpc in &sources.rpcs {
        if !cli.contains_key(rpc.as_str()) && !exempt.contains_key(rpc.as_str()) {
            problems.push(format!(
                "`{rpc}` has no CLI subcommand: add it to PARITY in {CLI_SOURCE}, or to {EXEMPT_SOURCE} with a reason (ADR-018)"
            ));
        }
    }

    if !problems.is_empty() {
        return Err(Findings(problems));
    }

    // `known` ist eine sortierte Menge; die Tabelle folgt ihrer Reihenfolge.
    let rpcs: Vec<&str> = known.into_iter().collect();
    let gaps: Vec<&str> = rpcs
        .iter()
        .copied()
        .filter(|rpc| !ui.contains_key(rpc))
        .collect();
    let warnings = gaps
        .iter()
        .map(|rpc| format!("`{rpc}` has no UI location in {UI_SOURCE}"))
        .collect();
    Ok(Table {
        markdown: render(&rpcs, &cli, &ui, &exempt, &gaps),
        warnings,
    })
}

/// Schreibt die Tabelle als Markdown; die Eingaben sind schon sortiert.
fn render(
    rpcs: &[&str],
    cli: &BTreeMap<&str, BTreeSet<&str>>,
    ui: &BTreeMap<&str, &str>,
    exempt: &BTreeMap<&str, &str>,
    gaps: &[&str],
) -> String {
    let mut out = String::from(concat!(
        "# Paritäts-Tabelle\n",
        "\n",
        "<!-- Erzeugt von `cargo xtask docs` (HUM-078). Nicht von Hand ändern. -->\n",
        "\n",
        "Jede Fähigkeit ist zuerst eine RPC; Kommandozeile und Oberfläche sind\n",
        "dünne Clients derselben Proto (ADR-018). Die Tabelle stellt jede Methode\n",
        "des Dienstes neben ihr Unterkommando und ihren Ort in der Oberfläche.\n",
        "\n",
        "Quellen:\n",
        "\n",
        "- RPC: die Service-Methoden unter `proto/humanitl/v1/`;\n",
        "- CLI: `PARITY` in `daemon/bin/humanitl/src/parity.rs`;\n",
        "- UI: `parity` in `app/lib/core/parity.dart`, Pfad unter `app/lib/`;\n",
        "- Ausnahmen: `daemon/xtask/parity_exempt.toml`.\n",
        "\n",
        "Eine RPC ohne CLI-Zeile und ohne Ausnahme bricht den CI-Job\n",
        "`parity-check`; eine RPC ohne Ort in der Oberfläche ist eine Warnung.\n",
        "\n",
        "| RPC | CLI | UI |\n",
        "|---|---|---|\n",
    ));
    for rpc in rpcs {
        let cli_cell = match cli.get(rpc) {
            Some(entries) => entries
                .iter()
                .map(|entry| format!("`humanitl {entry}`"))
                .collect::<Vec<_>>()
                .join("<br>"),
            None => "Ausnahme".to_owned(),
        };
        let ui_cell = ui
            .get(rpc)
            .map_or_else(|| "fehlt".to_owned(), |place| format!("`{place}`"));
        out.push_str(&format!("| `{rpc}` | {cli_cell} | {ui_cell} |\n"));
    }

    out.push_str("\n## UI-Lücken\n\n");
    if gaps.is_empty() {
        out.push_str("Keine.\n");
    } else {
        for rpc in gaps {
            out.push_str(&format!("- `{rpc}`\n"));
        }
    }

    out.push_str("\n## Ausnahmen\n\n");
    if exempt.is_empty() {
        out.push_str("Keine. Jede RPC hat ein Unterkommando.\n");
    } else {
        out.push_str("| RPC | Begründung |\n|---|---|\n");
        for (rpc, reason) in exempt {
            let reason = reason.split_whitespace().collect::<Vec<_>>().join(" ");
            out.push_str(&format!("| `{rpc}` | {reason} |\n"));
        }
    }
    out
}

/// Liest alle Quellen unter `root` und erzeugt die Tabelle.
///
/// # Errors
///
/// Eine Quelle fehlt oder ist unlesbar, oder [`build`] hat Befunde.
pub fn generate(root: &Path, fds: FileDescriptorSet) -> Result<Table, Box<dyn Error>> {
    let read = |relative: &str| {
        std::fs::read_to_string(root.join(relative))
            .map_err(|error| Findings(vec![format!("{relative}: {error}")]))
    };
    let sources = Sources {
        rpcs: rpcs(fds)?,
        cli: parse_cli(&read(CLI_SOURCE)?)?,
        ui: parse_ui(&read(UI_SOURCE)?)?,
        exempt: parse_exempt(&read(EXEMPT_SOURCE)?)?,
    };
    let lib = root.join("app/lib");
    Ok(build(&sources, |place| {
        lib.join(format!("{place}.dart")).is_file()
    })?)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use protox::prost_reflect::prost_types::{
        FileDescriptorProto, MethodDescriptorProto, ServiceDescriptorProto,
    };

    use super::*;

    /// Ein Descriptor mit einem Service `Humanitl` und diesen Methoden.
    fn fixture(methods: &[&str]) -> FileDescriptorSet {
        let method = |name: &&str| MethodDescriptorProto {
            name: Some((*name).to_owned()),
            input_type: Some(".fixture.Empty".to_owned()),
            output_type: Some(".fixture.Empty".to_owned()),
            ..MethodDescriptorProto::default()
        };
        FileDescriptorSet {
            file: vec![FileDescriptorProto {
                name: Some("fixture.proto".to_owned()),
                package: Some("fixture".to_owned()),
                syntax: Some("proto3".to_owned()),
                message_type: vec![protox::prost_reflect::prost_types::DescriptorProto {
                    name: Some("Empty".to_owned()),
                    ..Default::default()
                }],
                service: vec![ServiceDescriptorProto {
                    name: Some("Humanitl".to_owned()),
                    method: methods.iter().map(method).collect(),
                    ..ServiceDescriptorProto::default()
                }],
                ..FileDescriptorProto::default()
            }],
        }
    }

    fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
        list.iter()
            .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
            .collect()
    }

    #[test]
    fn missing_cli_fails() {
        let sources = Sources {
            rpcs: rpcs(fixture(&["Decide", "Watch"])).unwrap(),
            cli: pairs(&[("Humanitl.Decide", "flows decide")]),
            ..Sources::default()
        };
        let findings = build(&sources, |_| true).unwrap_err();
        assert_eq!(findings.0.len(), 1, "{findings}");
        assert!(
            findings.0[0].starts_with("`Humanitl.Watch` has no CLI subcommand"),
            "{findings}"
        );
    }

    #[test]
    fn exempt_listed() {
        let exempt = parse_exempt(
            "[[exempt]]\nrpc = \"Humanitl.Watch\"\nreason = \"\"\"A stream\nfor screens.\"\"\"\n",
        )
        .unwrap();
        let sources = Sources {
            rpcs: rpcs(fixture(&["Decide", "Watch"])).unwrap(),
            cli: pairs(&[("Humanitl.Decide", "flows decide")]),
            ui: pairs(&[("Humanitl.Watch", "features/intercept/queue")]),
            exempt,
        };
        let table = build(&sources, |_| true).unwrap();
        assert!(
            table
                .markdown
                .contains("| `Humanitl.Watch` | Ausnahme | `features/intercept/queue` |")
        );
        assert!(table.markdown.contains("## Ausnahmen\n\n| RPC | Begründung |\n|---|---|\n| `Humanitl.Watch` | A stream for screens. |\n"));
        assert_eq!(
            table.warnings,
            vec!["`Humanitl.Decide` has no UI location in app/lib/core/parity.dart"]
        );
    }

    #[test]
    fn an_exemption_needs_a_reason_and_a_real_rpc() {
        let exempt = parse_exempt(
            "[[exempt]]\nrpc = \"Humanitl.Watch\"\nreason = \"  \"\n\n[[exempt]]\nrpc = \"Humanitl.Gone\"\nreason = \"old\"\n",
        )
        .unwrap();
        let sources = Sources {
            rpcs: rpcs(fixture(&["Watch"])).unwrap(),
            exempt,
            ..Sources::default()
        };
        let findings = build(&sources, |_| true).unwrap_err();
        assert_eq!(
            findings.0,
            vec![
                "daemon/xtask/parity_exempt.toml: `Humanitl.Watch` has no reason",
                "daemon/xtask/parity_exempt.toml: `Humanitl.Gone` is not a method of the contract",
            ]
        );
        assert!(parse_exempt("[[exempt]]\nrpc = \"Humanitl.Watch\"\n").is_err());
    }

    #[test]
    fn stale_entries_fail() {
        let sources = Sources {
            rpcs: rpcs(fixture(&["Decide"])).unwrap(),
            cli: pairs(&[
                ("Humanitl.Decide", "flows decide"),
                ("Humanitl.Gone", "gone"),
            ]),
            ui: pairs(&[("Humanitl.Decide", "features/moved/away")]),
            exempt: vec![Exempt {
                rpc: "Humanitl.Decide".to_owned(),
                reason: "why".to_owned(),
            }],
        };
        let findings = build(&sources, |_| false).unwrap_err();
        assert_eq!(findings.0.len(), 3, "{findings}");
        assert!(findings.0[0].contains("`Humanitl.Gone` is not a method"));
        assert!(findings.0[1].contains("app/lib/features/moved/away.dart, which does not exist"));
        assert!(findings.0[2].contains("is exempt but has a CLI entry"));
    }

    #[test]
    fn the_table_is_sorted_and_joins_subcommands() {
        let sources = Sources {
            rpcs: rpcs(fixture(&["Rules", "Audit"])).unwrap(),
            cli: pairs(&[
                ("Humanitl.Rules", "rules list"),
                ("Humanitl.Audit", "audit verify"),
                ("Humanitl.Rules", "rules add"),
            ]),
            ui: pairs(&[("Humanitl.Rules", "features/rules/rules_screen")]),
            ..Sources::default()
        };
        let table = build(&sources, |_| true).unwrap();
        let rows: Vec<&str> = table
            .markdown
            .lines()
            .filter(|line| line.starts_with("| `"))
            .collect();
        assert_eq!(
            rows,
            vec![
                "| `Humanitl.Audit` | `humanitl audit verify` | fehlt |",
                "| `Humanitl.Rules` | `humanitl rules add`<br>`humanitl rules list` | `features/rules/rules_screen` |",
            ]
        );
        assert!(
            table
                .markdown
                .contains("## UI-Lücken\n\n- `Humanitl.Audit`\n")
        );
        assert!(table.markdown.contains("## Ausnahmen\n\nKeine."));
    }

    #[test]
    fn a_line_out_of_format_is_an_error_not_a_skip() {
        let good = "pub static PARITY: &[(&str, &str)] = &[\n    // note\n    (\"Humanitl.Decide\", \"flows decide\"),\n\n    (\"Humanitl.GetBody\", \"flows show --body\"),\n];\n";
        assert_eq!(
            parse_cli(good).unwrap(),
            pairs(&[
                ("Humanitl.Decide", "flows decide"),
                ("Humanitl.GetBody", "flows show --body")
            ])
        );
        let wrapped = "pub static PARITY: &[(&str, &str)] = &[\n    (\n        \"Humanitl.Decide\",\n        \"flows decide\",\n    ),\n];\n";
        assert!(parse_cli(wrapped).is_err());
        assert!(parse_cli("pub static PARITY: &[(&str, &str)] = &[\n").is_err());
        // Ein Eintrag auf der Anfangszeile wird nicht still verschluckt.
        let on_start =
            "pub static PARITY: &[(&str, &str)] = &[(\"Humanitl.Decide\", \"flows decide\"),\n];\n";
        let findings = parse_cli(on_start).unwrap_err();
        assert!(findings.0[0].contains("must end with `&[`"), "{findings}");
        assert!(parse_ui("const parity = <String, String>{'Humanitl.Decide': 'x',\n};\n").is_err());

        let dart = "const parity = <String, String>{\n  // note\n  'Humanitl.Decide': 'features/intercept/widgets/action_bar',\n};\n";
        assert_eq!(
            parse_ui(dart).unwrap(),
            pairs(&[("Humanitl.Decide", "features/intercept/widgets/action_bar")])
        );
        assert!(
            parse_ui("const parity = <String, String>{\n  \"Humanitl.Decide\": 'x',\n};\n")
                .is_err()
        );
    }

    #[test]
    fn the_real_contract_lists_the_service_methods() {
        let proto = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../proto"));
        let names = rpcs(crate::compile_protos(proto, false).unwrap()).unwrap();
        assert!(names.contains(&"Humanitl.Decide".to_owned()), "{names:?}");
        assert!(names.contains(&"Humanitl.Terminal".to_owned()), "{names:?}");
    }
}
