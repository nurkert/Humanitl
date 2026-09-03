//! Ausgabe: Befunde als lesbarer Block, Ergebnisse als Tabelle oder als JSON.
//!
//! Die Kommandozeile hat zwei Leser, und beide bekommen dieselbe Information
//! in ihrer Form. Ein Mensch bekommt sie auf `stderr` als Block, den man ohne
//! Handbuch versteht:
//!
//! ```text
//! error[SANDBOX_003]: User-Namespaces nicht erlaubt
//!   why: bwrap: setting up uid map: Permission denied
//!   fix: sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0
//!   docs: https://github.com/nurkert/Humanitl/blob/main/docs/DIAGNOSTICS.md#sandbox_003
//! ```
//!
//! Ein Programm bekommt mit `--json` denselben Befund als eine Zeile JSON auf
//! `stdout`. Die Trennung ist bewusst: `stdout` trägt das Ergebnis, `stderr`
//! trägt, was schiefging und was man dagegen tun kann. Wer die Ausgabe in eine
//! Pipe steckt, verliert damit keinen Befund und bekommt keinen dazu.

use humanitl_core::diagnostics::lookup;
use humanitl_core::{Diagnostic, FixAction, Severity};
use serde_json::{Value, json};

/// Wo die Befunde erklärt sind. Der Anker kommt aus dem Register.
pub const DOCS_BASE: &str = concat!(
    env!("CARGO_PKG_REPOSITORY"),
    "/blob/main/docs/DIAGNOSTICS.md"
);

/// Die Einrückung der Zeilen unter der Überschrift eines Befunds.
const INDENT: &str = "  ";

/// Wohin und wie ausgegeben wird.
#[derive(Debug, Clone, Copy)]
pub struct Renderer {
    /// `--json`: ein JSON-Wert je Aufruf auf `stdout`.
    json: bool,
    /// Wie oft `-v` angegeben wurde.
    verbose: u8,
    /// `-q`: nur das Ergebnis, keine Hinweise.
    quiet: bool,
}

impl Renderer {
    /// Der Renderer aus den globalen Schaltern.
    #[must_use]
    pub const fn new(json: bool, verbose: u8, quiet: bool) -> Self {
        Self {
            json,
            verbose,
            quiet,
        }
    }

    /// Ob die Ausgabe maschinenlesbar ist.
    #[must_use]
    pub const fn is_json(self) -> bool {
        self.json
    }

    /// Ob zusätzliche Erklärungen erwünscht sind (`-v`).
    #[must_use]
    pub const fn is_verbose(self) -> bool {
        self.verbose > 0
    }

    /// Eine Zeile des Ergebnisses auf `stdout`; im JSON-Modus nichts.
    pub fn line(self, text: &str) {
        if !self.json {
            println!("{text}");
        }
    }

    /// Ein Hinweis auf `stderr`, der das Ergebnis begleitet; mit `-q` nichts.
    ///
    /// Hinweise gehören nicht in eine Pipe: sie erklären das Ergebnis, sie
    /// sind es nicht.
    pub fn note(self, text: &str) {
        if !self.json && !self.quiet {
            eprintln!("{text}");
        }
    }

    /// Ein Hinweis, den nur `-v` zeigt.
    pub fn detail(self, text: &str) {
        if self.is_verbose() {
            self.note(text);
        }
    }

    /// Das Ergebnis im JSON-Modus: ein Wert als eine Zeile auf `stdout`.
    pub fn value(self, value: &Value) {
        if self.json {
            println!("{value}");
        }
    }

    /// Einen Befund ausgeben: als eine Zeile JSON oder als Block auf `stderr`.
    pub fn diagnostic(self, diagnostic: &Diagnostic) {
        if self.json {
            println!("{}", diagnostic_json(diagnostic));
        } else {
            eprint!("{}", diagnostic_block(diagnostic));
        }
    }
}

/// Der Befund als Block, wie ihn ein Mensch liest. Endet mit einem Zeilenumbruch.
#[must_use]
pub fn diagnostic_block(diagnostic: &Diagnostic) -> String {
    use std::fmt::Write as _;

    let mut out = format!(
        "{}[{}]: {}\n",
        severity_word(diagnostic.severity),
        diagnostic.code,
        diagnostic.title
    );
    // Ein `String` nimmt jedes `write!` an; der `Result` kann nicht scheitern.
    let _ = writeln!(out, "{INDENT}why: {}", one_line(&diagnostic.why));
    if let Some(fix) = diagnostic.fix.as_ref() {
        let _ = writeln!(out, "{INDENT}fix: {}", one_line(&fix_line(fix)));
    }
    if let Some(docs) = docs_url(diagnostic) {
        let _ = writeln!(out, "{INDENT}docs: {docs}");
    }
    out
}

/// Der Befund als JSON-Wert, eine Zeile für Werkzeuge.
#[must_use]
pub fn diagnostic_json(diagnostic: &Diagnostic) -> Value {
    let mut value = json!({
        "code": diagnostic.code.as_str(),
        "severity": diagnostic.severity.as_str(),
        "title": diagnostic.title,
        "why": diagnostic.why,
    });
    if let Some(object) = value.as_object_mut() {
        if let Some(fix) = diagnostic.fix.as_ref() {
            object.insert("fix".to_owned(), fix_json(fix));
        }
        if let Some(docs) = docs_url(diagnostic) {
            object.insert("docs".to_owned(), Value::String(docs));
        }
    }
    value
}

/// Das Wort vor der eckigen Klammer: die Stufe des Befunds.
#[must_use]
pub const fn severity_word(severity: Severity) -> &'static str {
    severity.as_str()
}

/// Die Adresse, unter der der Befund erklärt ist.
///
/// Erst die Adresse am Befund selbst, dann der Anker aus dem Register. Ein
/// Code, den das Register nicht kennt, hat keine.
#[must_use]
pub fn docs_url(diagnostic: &Diagnostic) -> Option<String> {
    if let Some(docs) = diagnostic.docs.as_ref() {
        return Some(docs.clone());
    }
    lookup(diagnostic.code).map(|info| format!("{DOCS_BASE}{}", info.docs_anchor))
}

/// Der Behebungsvorschlag als eine Zeile, die man abtippen oder kopieren kann.
#[must_use]
pub fn fix_line(fix: &FixAction) -> String {
    match fix {
        FixAction::SetEnv { key, value } => format!("export {key}={value}"),
        FixAction::ChangeSetting { key, value } => format!("humanitl config set {key} {value}"),
        FixAction::CopyCommand(command) | FixAction::OpenUrl(command) => command.clone(),
        FixAction::InstallService => "humanitl daemon install".to_owned(),
        FixAction::AddRule(rule) => format!("add the rule {} ({})", rule.id, rule.action),
        FixAction::RemountReadOnly(path) => {
            format!("mount {} read-only", path.display())
        }
    }
}

/// Der Behebungsvorschlag als JSON: die Art und ihre Werte.
#[must_use]
pub fn fix_json(fix: &FixAction) -> Value {
    let mut value = json!({ "kind": fix.as_str(), "command": fix_line(fix) });
    if let Some(object) = value.as_object_mut() {
        match fix {
            FixAction::SetEnv { key, value: v } | FixAction::ChangeSetting { key, value: v } => {
                object.insert("key".to_owned(), Value::String(key.clone()));
                object.insert("value".to_owned(), Value::String(v.clone()));
            }
            FixAction::OpenUrl(url) => {
                object.insert("url".to_owned(), Value::String(url.clone()));
            }
            FixAction::RemountReadOnly(path) => {
                object.insert("path".to_owned(), Value::String(path.display().to_string()));
            }
            FixAction::AddRule(rule) => {
                object.insert("rule_id".to_owned(), Value::String(rule.id.to_string()));
            }
            FixAction::CopyCommand(_) | FixAction::InstallService => {}
        }
    }
    value
}

/// Bringt einen Text auf eine Zeile: Umbrüche werden zu Leerzeichen.
///
/// Ein `why` aus der Fehlerausgabe eines fremden Programms kann mehrzeilig
/// sein; der Block bliebe sonst nicht lesbar, weil die Einrückung verrutscht.
#[must_use]
pub fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Eine Tabelle mit Kopfzeile, Spalten nach dem längsten Eintrag ausgerichtet.
///
/// Die letzte Spalte wird nicht aufgefüllt, damit kein Zeilenende Leerzeichen
/// trägt.
#[must_use]
pub fn table(headers: &[&str], rows: &[Vec<String>]) -> String {
    let mut widths: Vec<usize> = headers.iter().map(|head| head.chars().count()).collect();
    for row in rows {
        for (index, cell) in row.iter().enumerate() {
            let width = cell.chars().count();
            if let Some(current) = widths.get_mut(index)
                && *current < width
            {
                *current = width;
            }
        }
    }

    let mut out = String::new();
    let header_cells: Vec<String> = headers.iter().map(|head| (*head).to_owned()).collect();
    out.push_str(&row_line(&header_cells, &widths));
    out.push('\n');
    for row in rows {
        out.push_str(&row_line(row, &widths));
        out.push('\n');
    }
    out
}

/// Eine Zeile der Tabelle, mit zwei Leerzeichen zwischen den Spalten.
fn row_line(cells: &[String], widths: &[usize]) -> String {
    let last = cells.len().saturating_sub(1);
    cells
        .iter()
        .enumerate()
        .map(|(index, cell)| {
            let width = widths.get(index).copied().unwrap_or(0);
            if index == last {
                cell.clone()
            } else {
                let pad = width.saturating_sub(cell.chars().count());
                format!("{cell}{}", " ".repeat(pad))
            }
        })
        .collect::<Vec<_>>()
        .join("  ")
}

/// Das Zeichen für eine bestandene oder gescheiterte Prüfung.
#[must_use]
pub const fn tick(passed: bool) -> &'static str {
    if passed { "✓" } else { "✗" }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_core::diagnostics::codes::{DAEMON_001, SANDBOX_003};
    use humanitl_core::{Diagnostic, FixAction, Severity};

    use super::{diagnostic_block, diagnostic_json, one_line, table, tick};

    fn sandbox_diagnostic() -> Diagnostic {
        Diagnostic::builder(SANDBOX_003, Severity::Blocking)
            .why("bwrap: setting up uid map: Permission denied")
            .fix(FixAction::CopyCommand(
                "sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0".to_owned(),
            ))
            .build()
    }

    #[test]
    fn the_block_has_the_shape_from_the_issue() {
        let text = diagnostic_block(&sandbox_diagnostic());
        let lines: Vec<&str> = text.lines().collect();

        assert_eq!(
            lines[0],
            "blocking[SANDBOX_003]: User-Namespaces nicht erlaubt"
        );
        assert_eq!(
            lines[1],
            "  why: bwrap: setting up uid map: Permission denied"
        );
        assert_eq!(
            lines[2],
            "  fix: sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0"
        );
        assert!(lines[3].starts_with("  docs: https://"), "{:?}", lines[3]);
        assert!(lines[3].ends_with("#sandbox_003"), "{:?}", lines[3]);
        assert!(text.ends_with('\n'));
    }

    #[test]
    fn a_block_without_a_fix_has_three_lines() {
        let diagnostic = Diagnostic::builder(DAEMON_001, Severity::Blocking)
            .why("no socket at /run/user/1000/humanitl/daemon.sock")
            .build();
        let text = diagnostic_block(&diagnostic);

        assert_eq!(text.lines().count(), 3);
        assert!(!text.contains("fix:"));
    }

    #[test]
    fn a_multiline_why_stays_on_one_line() {
        let diagnostic = Diagnostic::builder(DAEMON_001, Severity::Error)
            .why("first line\nsecond line")
            .build();

        assert!(diagnostic_block(&diagnostic).contains("  why: first line second line\n"));
        assert_eq!(one_line("  a \n b  "), "a b");
    }

    #[test]
    fn the_json_form_is_one_line_and_carries_code_and_fix() {
        let value = diagnostic_json(&sandbox_diagnostic());
        let text = value.to_string();

        assert!(!text.contains('\n'));
        assert_eq!(value["code"], "SANDBOX_003");
        assert_eq!(value["severity"], "blocking");
        assert_eq!(value["fix"]["kind"], "copy_command");
        assert_eq!(
            value["fix"]["command"],
            "sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0"
        );
        assert!(
            value["docs"]
                .as_str()
                .is_some_and(|docs| docs.ends_with("#sandbox_003"))
        );
    }

    #[test]
    fn a_table_aligns_every_column_but_the_last() {
        let rows = vec![
            vec!["a".to_owned(), "long value".to_owned()],
            vec!["bbbb".to_owned(), "x".to_owned()],
        ];
        let text = table(&["ID", "VALUE"], &rows);

        assert_eq!(
            text, "ID    VALUE\na     long value\nbbbb  x\n",
            "unexpected table:\n{text}"
        );
        assert!(text.lines().all(|line| !line.ends_with(' ')));
    }

    #[test]
    fn the_tick_is_a_check_or_a_cross() {
        assert_eq!(tick(true), "✓");
        assert_eq!(tick(false), "✗");
    }
}
