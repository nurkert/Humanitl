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

use humanitl_core::block::{NOTE_MAX_CHARS, sanitize_note};
use humanitl_core::diagnostics::lookup;
use humanitl_core::{Diagnostic, FixAction, Severity};
use serde_json::{Value, json};

pub use humanitl_core::shell::{shell_path, shell_word};

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
        plain(&diagnostic.title)
    );
    // Ein `String` nimmt jedes `write!` an; der `Result` kann nicht scheitern.
    let _ = writeln!(out, "{INDENT}why: {}", plain(&diagnostic.why));
    if let Some(fix) = diagnostic.fix.as_ref() {
        let _ = writeln!(out, "{INDENT}fix: {}", fix_shown(fix));
    }
    if let Some(docs) = docs_url(diagnostic) {
        let _ = writeln!(out, "{INDENT}docs: {docs}");
    }
    out
}

/// Was im Block hinter `fix:` steht, wenn der Vorschlag ein Befehl ist.
const FIX_WITHHELD: &str = "the command cannot be shown here without changing it; \
                            `--json` carries it verbatim as fix.command, the docs explain the step";

/// Der Behebungsvorschlag, wie ihn der Block zeigt: genau der Befehl oder
/// gar keiner (HUM-215).
///
/// Der Block zeigt nur, was [`plain`] durchlässt, und [`plain`] faltet
/// Leerraum, wirft Steuerzeichen weg und kürzt auf [`NOTE_MAX_CHARS`]. Auf
/// einen Befehl angewandt, hieße das: aus `'/home/u/Audit  2026'` würde
/// `'/home/u/Audit 2026'`, also eine andere Datei, und aus
/// `mkdir -p … && chmod 700 …` würde bei einem langen Pfad ein `mkdir` ohne
/// `chmod`. Ein Befehl, den jemand kopiert, muss aber genau der sein, den der
/// Befund meint. Deshalb erscheint die Zeile nur, wenn [`plain`] sie nicht
/// verändert; sonst steht an ihrer Stelle ein Verweis auf `--json`, das den
/// Befehl unverändert trägt, und auf die Doku-Zeile darunter.
///
/// Die Längenprüfung ist heute schon in [`plain`] enthalten, weil es auf
/// [`NOTE_MAX_CHARS`] kürzt; sie steht trotzdem da, weil sie die Zusage ist
/// und nicht davon abhängen soll, wo [`plain`] einmal kürzt.
fn fix_shown(fix: &FixAction) -> String {
    let line = fix_line(fix);
    if plain(&line) == line && line.chars().count() <= NOTE_MAX_CHARS {
        line
    } else {
        FIX_WITHHELD.to_owned()
    }
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
        FixAction::SetEnv { key, value } => format!("export {key}={}", shell_word(value)),
        FixAction::ChangeSetting { key, value } => {
            format!("humanitl config set {key} {}", shell_word(value))
        }
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

/// Text aus einem Befund, wie er in ein Terminal darf (HUM-068).
///
/// Ein `why` trägt Namen von Hosts, Pfade und Fehlertexte fremder Programme;
/// ein `fix` trägt Befehle. Beides kann Steuerzeichen enthalten, und ein
/// Terminal führt sie aus: `ESC ] 8 ; ; URL BEL` macht aus dem folgenden Text
/// einen Verweis, `CSI` verschiebt den Cursor, ein Bidi-Override dreht die
/// Leserichtung um. Der Block dieses Programms zeigt deshalb nur, was Zeichen
/// **sind**, nie was sie einem Terminal **sagen**: [`sanitize_note`] wirft
/// Steuerzeichen und unsichtbare Zeichen weg, [`one_line`] faltet den
/// Leerraum. Der sichtbare Text eines OSC-8-Verweises bleibt stehen — er ist
/// Text —, seine Wirkung nicht.
///
/// `--json` braucht das nicht: `serde_json` schreibt ein `ESC` als `\u001b`,
/// und damit ist es in jeder Ausgabe schon inert.
#[must_use]
pub fn plain(text: &str) -> String {
    one_line(&sanitize_note(text))
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

/// Ein Block aus beschrifteten Zeilen: `label:` links, der Wert bündig rechts.
///
/// Die zweite Form, in der die Kommandozeile ein Ergebnis zeigt. Eine Tabelle
/// ([`table`]) ist für viele gleichartige Zeilen richtig; ein Ergebnis aus
/// wenigen, verschieden benannten Angaben — die Prüfung der Audit-Kette etwa —
/// liest sich als Block besser, weil jede Zeile ihren eigenen Namen trägt.
///
/// Die Werte stehen in einer Spalte, also wird die Beschriftung aufgefüllt;
/// ein Wert bekommt nie Leerzeichen hinter sich. Jede Zeile endet mit einem
/// Zeilenumbruch.
#[must_use]
pub fn labeled(rows: &[(&str, String)]) -> String {
    let width = rows
        .iter()
        .map(|(label, _)| label.chars().count())
        .max()
        .unwrap_or(0);
    let mut out = String::new();
    for (label, value) in rows {
        let pad = (width + 1).saturating_sub(label.chars().count() + 1);
        out.push_str(label);
        out.push(':');
        out.push_str(&" ".repeat(pad + 1));
        out.push_str(&plain(value));
        out.push('\n');
    }
    out
}

/// Das Zeichen für eine bestandene oder gescheiterte Prüfung.
#[must_use]
pub const fn tick(passed: bool) -> &'static str {
    if passed { "✓" } else { "✗" }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    /// Ein `why`, das einen Verweis in das Terminal schreiben will, druckt
    /// seinen Text und nicht seine Wirkung (HUM-068).
    ///
    /// Der Fall ist nicht ausgedacht: `why` trägt Hostnamen und Fehlertexte
    /// fremder Programme, und ein Agent in der Sandbox entscheidet mit, was
    /// darin steht. Ein OSC-8-Verweis im Block machte aus dem Satz eines
    /// Befunds einen anklickbaren Link auf eine Adresse, die niemand gelesen
    /// hat.
    #[test]
    fn a_diagnostic_with_terminal_escapes_prints_them_inert() {
        let sneaky = "\u{1b}]8;;https://evil.example/pay\u{7}open the invoice\u{1b}]8;;\u{7}";
        let block = diagnostic_block(
            &Diagnostic::builder(SANDBOX_003, Severity::Blocking)
                .why(format!("bwrap is missing; {sneaky}"))
                .fix(FixAction::CopyCommand(format!("apt-get install {sneaky}")))
                .build(),
        );

        assert!(
            !block.contains('\u{1b}') && !block.contains('\u{7}'),
            "no escape and no bell reach the terminal: {block:?}"
        );
        // Der sichtbare Text bleibt: Er ist die Nachricht, und ihn zu
        // schlucken hieße, einen Befund zu verstümmeln.
        assert!(block.contains("open the invoice"), "{block}");
        assert!(block.contains("evil.example/pay"), "{block}");
        assert!(block.contains("bwrap is missing"), "{block}");
    }

    /// Auch der Titel: Er kommt aus dem Register und ist damit unser Text —
    /// die Zusicherung hält die Stelle trotzdem, weil ein Befund ihn
    /// überschreiben darf.
    #[test]
    fn a_title_with_escapes_prints_inert() {
        let block = diagnostic_block(
            &Diagnostic::builder(SANDBOX_003, Severity::Blocking)
                .title("bwrap \u{1b}[31mfehlt\u{1b}[0m")
                .why("nothing".to_owned())
                .build(),
        );
        assert!(!block.contains('\u{1b}'), "{block:?}");
        assert!(block.contains("bwrap [31mfehlt[0m"), "{block}");
    }

    use humanitl_core::diagnostics::codes::{DAEMON_001, SANDBOX_003};
    use humanitl_core::{Diagnostic, FixAction, Severity};

    use super::{diagnostic_block, diagnostic_json, labeled, one_line, table, tick};

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

    /// Die Werte stehen in einer Spalte, und keine Zeile endet auf Leerzeichen.
    #[test]
    fn a_labeled_block_aligns_its_values_and_pads_no_line_end() {
        let text = labeled(&[
            ("audit chain", "OK".to_owned()),
            ("records", "4213".to_owned()),
            ("warnings", "no HMAC key (file mode)".to_owned()),
        ]);

        assert_eq!(
            text, "audit chain: OK\nrecords:     4213\nwarnings:    no HMAC key (file mode)\n",
            "unexpected block:\n{text}"
        );
        assert!(text.lines().all(|line| !line.ends_with(' ')));
        // Ein Wert, der ein Terminal steuern will, druckt seinen Text und
        // nicht seine Wirkung -- wie im Befund (HUM-068).
        let sneaky = labeled(&[("head", "a3f9\u{1b}[31m".to_owned())]);
        assert!(!sneaky.contains('\u{1b}'), "{sneaky:?}");
    }

    /// Ein Vorschlag mit einer Liste kommt bei der Shell als diese Liste an.
    #[test]
    fn a_fix_with_a_list_is_one_shell_word() {
        let fix = FixAction::ChangeSetting {
            key: "llm.passthrough_paths".to_owned(),
            value: r#"["/v1/","/api/"]"#.to_owned(),
        };
        assert_eq!(
            super::fix_line(&fix),
            r#"humanitl config set llm.passthrough_paths '["/v1/","/api/"]'"#
        );
        assert_eq!(super::shell_word("300"), "300");
        // zsh macht aus einem nackten `=ls` den Pfad von `ls`.
        assert_eq!(super::shell_word("=ls"), "'=ls'");
        assert_eq!(super::shell_word("a=b"), "a=b");
        assert_eq!(super::shell_word(""), "''");
        assert_eq!(super::shell_word("it's"), r"'it'\''s'");
    }

    /// Nur ein `=` am Anfang wird gequotet, eines in der Mitte bleibt nackt.
    #[test]
    fn only_a_leading_equals_sign_is_quoted() {
        assert_eq!(super::shell_word("=foo"), "'=foo'");
        assert_eq!(super::shell_word("foo=bar"), "foo=bar");
        assert_eq!(super::shell_path(std::path::Path::new("=foo")), "'=foo'");
        assert_eq!(
            super::shell_path(std::path::Path::new("dir/foo=bar")),
            "dir/foo=bar"
        );
    }

    /// Ein Name mit Zeilenumbruch, Tabulator oder zwei Leerzeichen kommt im
    /// Block des Befunds so an, dass die Shell denselben Namen daraus macht.
    #[test]
    fn a_fix_with_whitespace_in_a_name_survives_the_block() {
        let name = "nl\nx  y\tz.csv";
        let word = super::shell_word(name);
        assert_eq!(word, r"$'nl\x0ax\x20\x20y\x09z.csv'");
        let block = diagnostic_block(
            &Diagnostic::builder(DAEMON_001, Severity::Error)
                .why("because")
                .fix(FixAction::CopyCommand(format!("mv -- {word} out")))
                .build(),
        );
        assert!(
            block.contains(&format!("  fix: mv -- {word} out\n")),
            "{block}"
        );
        assert_eq!(super::shell_word(r"a\b c"), r"'a\b c'");
    }

    /// Ein Name, der kein UTF-8 ist, und einer mit Umlaut: Der Vorschlag nennt
    /// genau diese Bytes, und `bash` unter `LC_ALL=C` findet die Datei damit.
    #[test]
    fn a_path_is_quoted_from_its_bytes_and_works_under_the_c_locale() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt as _;

        let dir = tempfile::tempdir().expect("a directory");
        for name in [&b"bad\xffname.csv"[..], "gr\u{fc}n l.csv".as_bytes()] {
            let path = dir.path().join(OsStr::from_bytes(name));
            std::fs::write(&path, b"x").expect("the file");
            let word = super::shell_path(&path);
            assert!(word.is_ascii(), "{word}");
            let target = dir.path().join("moved");
            let status = std::process::Command::new("bash")
                .arg("-c")
                .arg(format!("mv -n -- {word} {}", super::shell_path(&target)))
                .env("LC_ALL", "C")
                .status()
                .expect("bash runs");
            assert!(status.success(), "{word}");
            assert!(target.is_file(), "{word} named the file");
            assert!(!path.exists(), "{word}");
            std::fs::remove_file(&target).expect("clean up");
        }
    }

    /// Die Zeile hinter `fix:` im Block, ohne Einrückung.
    fn shown_fix(diagnostic: &Diagnostic) -> String {
        let block = diagnostic_block(diagnostic);
        block
            .lines()
            .find_map(|line| line.strip_prefix("  fix: "))
            .unwrap_or_else(|| panic!("a fix line in {block}"))
            .to_owned()
    }

    fn with_command(command: String) -> Diagnostic {
        Diagnostic::builder(DAEMON_001, Severity::Error)
            .why("because")
            .fix(FixAction::CopyCommand(command))
            .build()
    }

    /// Ein Befehl mit zwei Leerzeichen in einfachen Anführungszeichen wird
    /// im Block nicht zu einem anderen Befehl gefaltet (HUM-215): Er
    /// erscheint gar nicht, und an seiner Stelle steht der Verweis auf
    /// `--json`, das ihn unverändert trägt.
    #[test]
    fn a_command_the_block_would_change_is_withheld() {
        let command = "mv -n -- '/home/u/Audit  2026/a.jsonl' /tmp/x".to_owned();
        let diagnostic = with_command(command.clone());
        let shown = shown_fix(&diagnostic);
        assert_eq!(shown, super::FIX_WITHHELD);
        assert!(!shown.contains("Audit 2026"), "{shown}");
        assert_eq!(diagnostic_json(&diagnostic)["fix"]["command"], command);

        let tabbed = with_command("rm '/a\tb'".to_owned());
        assert_eq!(shown_fix(&tabbed), super::FIX_WITHHELD);
    }

    /// Randfälle des einzelnen Leerzeichens: am Rand eines Worts und neben
    /// einem `'`. Beides übersteht den Block unverändert (HUM-215).
    #[test]
    fn a_word_with_lone_spaces_survives_the_block() {
        for value in [" a b ", "a ' b"] {
            let command = format!("rm -- {}", super::shell_word(value));
            assert!(!command.contains("$'"), "{command}");
            assert_eq!(shown_fix(&with_command(command.clone())), command);
            assert_eq!(super::plain(&command), command);
        }
    }

    /// Ein Befehl über [`NOTE_MAX_CHARS`] wird nicht abgeschnitten, so dass
    /// etwa ein `mkdir` ohne sein `chmod` übrig bliebe, sondern ganz
    /// zurückgehalten.
    #[test]
    fn a_command_over_the_cap_is_withheld_not_cut() {
        let long = format!("/{}", "d".repeat(260));
        let command = format!("mkdir -p {long} && chmod 700 {long}");
        assert!(command.chars().count() > super::NOTE_MAX_CHARS);
        let diagnostic = with_command(command.clone());
        assert_eq!(shown_fix(&diagnostic), super::FIX_WITHHELD);
        assert_eq!(diagnostic_json(&diagnostic)["fix"]["command"], command);

        let fits = format!("mkdir -p /{} && chmod 700 /x", "d".repeat(400));
        assert!(fits.chars().count() <= super::NOTE_MAX_CHARS);
        assert_eq!(shown_fix(&with_command(fits.clone())), fits);
    }

    /// Der Weg des Befunds, von einem Pfad mit zwei Leerzeichen bis zu dem
    /// Befehl, den ein Mensch aus dem Block kopiert: Er verschiebt genau diese
    /// Datei und nicht die, deren Name ein Leerzeichen weniger hat.
    #[test]
    fn a_copied_fix_moves_the_file_the_diagnostic_means() {
        let dir = tempfile::tempdir().expect("a directory");
        let spaced = dir.path().join("Audit  2026");
        let folded = dir.path().join("Audit 2026");
        for sub in [&spaced, &folded] {
            std::fs::create_dir(sub).expect("a directory");
            std::fs::write(sub.join("a.jsonl"), b"x").expect("a file");
        }
        let source = spaced.join("a.jsonl");
        let target = spaced.join("a.jsonl.old");
        let command = format!(
            "mv -n -- {} {}",
            super::shell_path(&source),
            super::shell_path(&target)
        );
        let copied = shown_fix(&with_command(command));
        let status = std::process::Command::new("bash")
            .arg("-c")
            .arg(&copied)
            .env("LC_ALL", "C")
            .status()
            .expect("bash runs");
        assert!(status.success(), "{copied}");
        assert!(target.is_file() && !source.exists(), "{copied}");
        assert!(folded.join("a.jsonl").is_file(), "{copied}");
    }

    #[test]
    fn the_tick_is_a_check_or_a_cross() {
        assert_eq!(tick(true), "✓");
        assert_eq!(tick(false), "✗");
    }
}
