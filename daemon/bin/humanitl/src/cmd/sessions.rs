//! `humanitl sessions summary <id>`: was ein Sandbox-Lauf im Projekt
//! hinterlassen hat (HUM-043).
//!
//! Ein gRPC-Aufruf und sonst nichts (ADR-018). Gemessen, gescannt und
//! zusammengefasst wird im Daemon; hier wird gezählt, ausgerichtet und
//! geschrieben. Auch die Befunde (`SANDBOX_022` bis `SANDBOX_028`) kommen
//! fertig aus der Antwort — die Kommandozeile leitet keinen davon selbst ab.
//!
//! # Jeder Pfad hier ist Anzeige — der Befehl ist keiner
//!
//! Die Namen in einer Zusammenfassung hat der Agent geschrieben. Der Daemon
//! schickt sie durch `humanitl_core::block::sanitize_note`, bevor sie auf die
//! Leitung gehen. Dieses Kommando tut es **noch einmal**, und zwar mit Absicht:
//! Die Ausgabe landet in einem Terminal, `render::one_line` lässt `ESC`, `OSC`
//! und Bidi-Zeichen stehen, und die Ausgabe des Agenten ist einer der fünf
//! erklärten Seitenkanäle (BACKLOG.md 4.2). Die Säuberung ist idempotent; die
//! zweite kostet nichts und hängt nicht daran, dass am anderen Ende der
//! Leitung ein Daemon dieser Fassung steht.
//!
//! Mit keinem dieser Namen wird eine Datei geöffnet, ein Pfad gebaut oder
//! etwas verglichen: Zwei verschiedene Namen können dieselbe Anzeige ergeben.
//! Was sie auseinanderhält, ist `path_hash`, und der steht in `--json`.
//!
//! **`SymlinkEscape.fix_command` ist die Ausnahme, und zwar die aufschlussreiche.**
//! Er ist kein Anzeigetext, sondern ein Befehl, der im Daemon aus dem rohen
//! Host-Pfad entstanden **und dort geprüft** worden ist. Ihn hier noch einmal
//! zu säubern hieße, ihn bei `NOTE_MAX_CHARS` zu kappen — und weil `rm -- '…'`
//! acht Zeichen auf den Pfad legt, verlöre ein Pfad knapp unter der Grenze sein
//! schließendes Anführungszeichen. Aus einem bewiesen sicheren Befehl würde
//! eine offene Quotierung in der Shell eines Menschen.
//!
//! Die Regel dahinter gilt für die ganze Klasse und ist die Kehrseite der
//! Regel darüber: **Ein Wert wird genau einmal geprüft, an der Stelle, die ihn
//! erzeugt; danach wird er weder erneut geprüft noch erneut gesäubert.** Für
//! einen Anzeigepfad ist die erzeugende Stelle die Anzeige selbst, deshalb die
//! zweite Säuberung; für einen Befehl ist sie `copy_command` im Daemon,
//! deshalb keine.

use humanitl_core::block::sanitize_note;
use humanitl_ipc::client::Client;
use humanitl_ipc::v1;
use serde_json::{Value, json};

use crate::cli::SessionsCmd;
use crate::cmd::{Context, EXIT_OK, Failure, from_proto, status_diagnostic};
use crate::render::table;

/// Die Spalten von `sessions summary`.
const CHANGE_HEADERS: [&str; 4] = ["KIND", "SIZE", "PATH", "NOTE"];

/// Die Spalten der Fundliste.
const FINDING_HEADERS: [&str; 5] = ["KIND", "TIER", "LINE", "PATH", "PREVIEW"];

/// Die Spalten der Symlink-Liste.
const SYMLINK_HEADERS: [&str; 3] = ["PATH", "TARGET", "LEAVES PROJECT"];

/// Führt `humanitl sessions <cmd>` aus.
///
/// # Errors
///
/// `DAEMON_001`, wenn kein Daemon antwortet, `IPC_005`, wenn die Kennung keine
/// ist, `RECORDER_001`, wenn dieser Daemon ohne Aufzeichnung läuft, und
/// `SANDBOX_027`, wenn es zu dem Lauf keine Zusammenfassung gibt.
pub async fn run(ctx: &Context, cmd: &SessionsCmd) -> Result<u8, Failure> {
    let client = ctx.connect().await?;
    match cmd {
        SessionsCmd::Summary { id } => summary(ctx, client, id).await,
    }
}

/// `sessions summary ID`.
async fn summary(ctx: &Context, mut client: Client, id: &str) -> Result<u8, Failure> {
    let summary = client
        .get_session_summary(v1::SessionSummaryRef {
            sandbox_id: id.to_owned(),
        })
        .await
        // `status_diagnostic` liest den Befund aus den Details, wenn einer
        // dabei ist, und übersetzt sonst den gRPC-Code. Beides hier noch
        // einmal zu tun wäre eine zweite Fassung derselben Übersetzung.
        .map_err(|status| Failure::new(status_diagnostic(&status, "GetSessionSummary")))?
        .into_inner();

    if ctx.render.is_json() {
        ctx.render.value(&summary_json(&summary));
        return Ok(EXIT_OK);
    }

    print_summary(ctx, &summary);
    Ok(EXIT_OK)
}

/// Die Zusammenfassung als Tabellen, mit den Befunden dahinter.
fn print_summary(ctx: &Context, summary: &v1::SessionSummary) {
    ctx.render.line(&format!(
        "sandbox {} in {}",
        clean(&summary.sandbox_id),
        clean(&summary.work_dir)
    ));

    if summary.changes.is_empty() {
        ctx.render.line("no changes");
    } else {
        let rows: Vec<Vec<String>> = summary.changes.iter().map(change_row).collect();
        ctx.render.line(table(&CHANGE_HEADERS, &rows).trim_end());
    }

    if !summary.findings.is_empty() {
        let rows: Vec<Vec<String>> = summary.findings.iter().map(finding_row).collect();
        ctx.render.line("");
        ctx.render.line(table(&FINDING_HEADERS, &rows).trim_end());
    }

    if !summary.symlinks.is_empty() {
        let rows: Vec<Vec<String>> = summary.symlinks.iter().map(symlink_row).collect();
        ctx.render.line("");
        ctx.render.line(table(&SYMLINK_HEADERS, &rows).trim_end());
    }

    ctx.render.note(&format!(
        "{} change(s), {} finding(s), {} symlink(s), {} byte(s) scanned{}",
        summary.changes.len(),
        summary.findings.len(),
        summary.symlinks.len(),
        summary.scanned_bytes,
        if summary.truncated {
            "; a budget cut this short, so it is not everything that changed"
        } else {
            ""
        }
    ));

    // Die Befunde des Daemons, wie überall: als Block auf `stderr`, damit die
    // Tabelle auf `stdout` in eine Pipe passt.
    for diagnostic in summary.diagnostics.iter().filter_map(from_proto) {
        ctx.render
            .note(crate::render::diagnostic_block(&diagnostic).trim_end());
    }
}

/// Eine Zeile der Tabelle „changed files".
fn change_row(change: &v1::FileChange) -> Vec<String> {
    let mut note = Vec::new();
    // Zuerst, weil es das Wichtigste an der Zeile ist: In dieser Datei wurde
    // nicht nach Geheimnissen gesucht. „Kein Fund" heißt hier „nicht
    // nachgesehen", und das muss vor allem anderen stehen.
    if let Some(why) = scan_skip_note(change.unscanned) {
        note.push(why.to_owned());
    }
    if !change.unprotected_by.is_empty() {
        note.push(format!("no mask over {}", clean(&change.unprotected_by)));
    }
    if change.git_metadata {
        note.push("git metadata".to_owned());
    }
    if change.mangled {
        // Ohne diesen Vermerk sähe eine Zeile mit einem unsichtbaren Zeichen
        // im Namen aus wie eine gewöhnliche.
        note.push(format!("name shown differs, sha256 {}", change.path_hash));
    }
    vec![
        change_kind_name(change.kind).to_owned(),
        change.size.to_string(),
        clean(&change.path),
        note.join("; "),
    ]
}

/// Eine Zeile der Fundliste.
fn finding_row(finding: &v1::SummaryFinding) -> Vec<String> {
    vec![
        clean(&finding.kind),
        tier_name(finding.tier).to_owned(),
        finding.line.to_string(),
        clean(&finding.path),
        clean(&finding.display_prefix),
    ]
}

/// Eine Zeile der Symlink-Liste.
fn symlink_row(link: &v1::SymlinkEscape) -> Vec<String> {
    vec![
        clean(&link.path),
        clean(&link.target),
        if link.escapes { "yes" } else { "no" }.to_owned(),
    ]
}

/// Der Name einer Änderungsart, wie ihn die Tabelle zeigt.
fn change_kind_name(kind: i32) -> &'static str {
    match v1::FileChangeKind::try_from(kind) {
        Ok(v1::FileChangeKind::Added) => "added",
        Ok(v1::FileChangeKind::Modified) => "modified",
        Ok(v1::FileChangeKind::Removed) => "removed",
        Ok(v1::FileChangeKind::SymlinkAdded) => "symlink",
        Ok(v1::FileChangeKind::ModeChanged) => "mode",
        // Ein Daemon, der neuer ist als dieser Client, kennt eine Art mehr.
        // Sie als „added" auszugeben wäre eine Behauptung; der Strich ist die
        // Wahrheit.
        Ok(v1::FileChangeKind::Unspecified) | Err(_) => "-",
    }
}

/// Der Satz zu einer Datei, in die der Fundscan nicht gesehen hat.
///
/// `None` heißt gelesen. Ein Wert, den dieser Client nicht kennt, ist ein
/// neuerer Daemon und wird als „not searched" gemeldet statt verschwiegen: Dass
/// **nicht** gesucht wurde, ist die Aussage, auf die es ankommt; der Grund ist
/// das Beiwerk.
fn scan_skip_note(skip: i32) -> Option<&'static str> {
    match v1::ScanSkip::try_from(skip) {
        Ok(v1::ScanSkip::Unspecified) => None,
        Ok(v1::ScanSkip::TooLarge) => Some("not searched: larger than the scan reads"),
        Ok(v1::ScanSkip::Unreadable) => Some("not searched: could not be read"),
        Ok(v1::ScanSkip::Budget) => Some("not searched: the scan budget was spent"),
        Err(_) => Some("not searched"),
    }
}

/// Der Name einer Sicherheitsstufe.
fn tier_name(tier: i32) -> &'static str {
    match v1::FindingTier::try_from(tier) {
        Ok(v1::FindingTier::Checksum) => "checksum",
        Ok(v1::FindingTier::Regex) => "regex",
        Ok(v1::FindingTier::UserTerm) => "user_term",
        Ok(v1::FindingTier::Unspecified) | Err(_) => "-",
    }
}

/// Ein Text vom Daemon, wie er in ein Terminal darf.
///
/// Siehe die Modulbeschreibung: Die Säuberung ist die zweite und hängt nicht
/// daran, dass der Daemon am anderen Ende sie schon gemacht hat.
fn clean(text: &str) -> String {
    sanitize_note(text)
}

/// Die Zusammenfassung als `JSON`.
///
/// Sie trägt mehr als die Tabelle: `path_hash` und `mangled` je Zeile, weil
/// erst der Hash zwei Namen unterscheidet, die gleich aussehen, und
/// `fix_command`, weil ein Skript ihn braucht.
fn summary_json(summary: &v1::SessionSummary) -> Value {
    json!({
        "session_id": summary.session_id,
        "sandbox_id": clean(&summary.sandbox_id),
        "work_dir": clean(&summary.work_dir),
        "changes": summary.changes.iter().map(|change| json!({
            "path": clean(&change.path),
            "path_hash": change.path_hash,
            "mangled": change.mangled,
            "kind": change_kind_name(change.kind),
            "size": change.size,
            "git_metadata": change.git_metadata,
            "unprotected_by": clean(&change.unprotected_by),
            "unscanned": scan_skip_note(change.unscanned),
        })).collect::<Vec<Value>>(),
        "findings": summary.findings.iter().map(|finding| json!({
            "path": clean(&finding.path),
            "path_hash": finding.path_hash,
            "mangled": finding.mangled,
            "line": finding.line,
            "kind": clean(&finding.kind),
            "tier": tier_name(finding.tier),
            "display_prefix": clean(&finding.display_prefix),
            "value_hash": finding.value_hash,
        })).collect::<Vec<Value>>(),
        "symlinks": summary.symlinks.iter().map(|link| json!({
            "path": clean(&link.path),
            "path_hash": link.path_hash,
            "mangled": link.mangled,
            "target": clean(&link.target),
            "escapes": link.escapes,
            // **Nicht gesäubert, mit Absicht.** Der Befehl ist kein
            // Anzeigetext: Er ist im Daemon aus dem rohen Host-Pfad entstanden
            // und **dort** geprüft worden — `sanitize_note` ließ ihn
            // unverändert, `shlex` zitierte ihn zu genau einem wörtlichen Wort
            // und `shlex::split` machte daraus wieder genau diesen Pfad. Ihn
            // hier ein zweites Mal durch `sanitize_note` zu schicken, hieße,
            // ihn bei 256 Zeichen zu kappen: Ein 250 Zeichen langer Pfad
            // verlöre das schließende Anführungszeichen, und wer die Zeile
            // ausführt, bekäme eine offene Quotierung.
            //
            // Die Regel dahinter gilt für die ganze Klasse: Ein Wert wird
            // genau einmal geprüft, an der Stelle, die ihn erzeugt; danach
            // wird er weder erneut geprüft noch erneut gesäubert. Für die
            // Anzeigepfade oben gilt das Gegenteil, und aus demselben Grund:
            // Sie sind Anzeige und waren es immer.
            "fix_command": link.fix_command,
        })).collect::<Vec<Value>>(),
        "unprotected": summary.unprotected.iter().map(|path| clean(path)).collect::<Vec<String>>(),
        "scanned_bytes": summary.scanned_bytes,
        "truncated": summary.truncated,
        "diagnostics": summary.diagnostics.iter().filter_map(from_proto)
            .map(|diagnostic| crate::render::diagnostic_json(&diagnostic))
            .collect::<Vec<Value>>(),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_ipc::v1;

    use super::{change_kind_name, change_row, clean, summary_json, symlink_row, tier_name};

    fn change(path: &str) -> v1::FileChange {
        v1::FileChange {
            path: path.to_owned(),
            path_hash: "0123456789abcdef".to_owned(),
            mangled: true,
            kind: v1::FileChangeKind::Added as i32,
            size: 7,
            git_metadata: false,
            unprotected_by: ".git/hooks".to_owned(),
            unscanned: v1::ScanSkip::Unspecified as i32,
        }
    }

    /// Ein Symlink-Ziel mit `ESC ]` erscheint gesäubert — in der Tabelle wie im
    /// `JSON`.
    ///
    /// Der Angriff ist ein `OSC 8` im Namen, den der Agent selbst geschrieben
    /// hat: Er macht aus der Zeile im Terminal einen anklickbaren Verweis oder
    /// überschreibt sie. Der Daemon säubert, und diese Seite säubert noch
    /// einmal — sonst hinge die Zusage daran, dass am anderen Ende der Leitung
    /// ein Daemon dieser Fassung steht.
    #[test]
    fn an_escape_sequence_from_the_agent_never_reaches_the_terminal() {
        let hostile = "\u{1b}]8;;http://evil\u{7}link\r\nsecond";
        let link = v1::SymlinkEscape {
            path: hostile.to_owned(),
            path_hash: "0123456789abcdef".to_owned(),
            mangled: true,
            target: hostile.to_owned(),
            escapes: true,
            fix_command: String::new(),
        };
        let row = symlink_row(&link);
        for cell in &row {
            assert!(!cell.contains('\u{1b}'), "{cell:?}");
            assert!(!cell.contains('\r'), "{cell:?}");
            assert!(!cell.contains('\n'), "{cell:?}");
        }

        let summary = v1::SessionSummary {
            symlinks: vec![link],
            changes: vec![change(hostile)],
            ..v1::SessionSummary::default()
        };
        let text = summary_json(&summary).to_string();
        assert!(!text.contains("\\u001b"), "{text}");
        assert!(!text.contains("\\r"), "{text}");
        // Der Hash bleibt: Er ist das, was zwei gleich aussehende Namen
        // unterscheidet.
        assert!(text.contains("0123456789abcdef"), "{text}");
    }

    /// Ein Name, dessen Anzeige vom echten Namen abweicht, sagt es in der
    /// Zeile.
    #[test]
    fn a_mangled_name_says_so_in_its_row() {
        let row = change_row(&change("a\u{200b}b"));
        let note = row.last().expect("the note column");
        assert!(note.contains("name shown differs"), "{note}");
        assert!(note.contains("0123456789abcdef"), "{note}");
        assert!(note.contains("no mask over .git/hooks"), "{note}");
    }

    /// Was dieser Client nicht kennt, wird nicht geraten.
    #[test]
    fn an_unknown_enum_value_is_a_dash_and_not_a_guess() {
        assert_eq!(change_kind_name(v1::FileChangeKind::Added as i32), "added");
        assert_eq!(change_kind_name(0), "-");
        assert_eq!(change_kind_name(99), "-");
        assert_eq!(tier_name(v1::FindingTier::Checksum as i32), "checksum");
        assert_eq!(tier_name(0), "-");
        assert_eq!(tier_name(99), "-");
    }

    /// „Nicht durchsucht" steht in der Zeile, und zwar zuerst.
    ///
    /// Ohne diesen Vermerk läse sich eine Zeile ohne Fund wie eine saubere
    /// Datei — dabei wurde in ihr gar nicht nachgesehen.
    #[test]
    fn a_file_that_was_not_searched_says_so_first() {
        let mut change = change("dump.bin");
        change.unscanned = v1::ScanSkip::TooLarge as i32;
        let row = change_row(&change);
        let note = row.last().expect("the note column");
        assert!(note.starts_with("not searched"), "{note}");
        assert!(note.contains("larger than the scan reads"), "{note}");

        // Ein Grund, den dieser Client nicht kennt, bleibt „not searched":
        // Dass nichts gesucht wurde, ist die Aussage, auf die es ankommt.
        change.unscanned = 99;
        let row = change_row(&change);
        assert!(
            row.last()
                .expect("the note column")
                .starts_with("not searched"),
            "{row:?}"
        );

        // Und eine gelesene Datei sagt nichts dergleichen.
        change.unscanned = v1::ScanSkip::Unspecified as i32;
        let row = change_row(&change);
        assert!(
            !row.last()
                .expect("the note column")
                .contains("not searched"),
            "{row:?}"
        );
    }

    /// Der Befehl für die Zwischenablage wird **nicht** noch einmal gesäubert.
    ///
    /// Er ist im Daemon aus dem rohen Host-Pfad entstanden und dort geprüft
    /// worden; `sanitize_note` kappt bei
    /// [`humanitl_core::block::NOTE_MAX_CHARS`] Zeichen, und `rm -- '…'` legt
    /// acht Zeichen auf den Pfad. Ein Pfad knapp unter der Grenze verlöre so
    /// sein schließendes Anführungszeichen — aus einem bewiesen sicheren Befehl
    /// würde eine offene Quotierung in der Shell eines Menschen.
    ///
    /// Die Regel dahinter: Ein Wert wird genau einmal geprüft, an der Stelle,
    /// die ihn erzeugt.
    #[test]
    fn the_copy_command_is_passed_through_untouched() {
        // Ein Pfad, der die Prüfung in `copy_command` gerade noch besteht:
        // `sanitize_note` lässt ihn unverändert, weil er unter dem Deckel
        // liegt. Der Befehl darum herum liegt darüber — genau das ist die
        // Falle. Das Leerzeichen erzwingt die Anführungszeichen; ohne
        // Sonderzeichen bräuchte der Pfad keine, und der Fall fiele nicht auf.
        let filler = "p".repeat(humanitl_core::block::NOTE_MAX_CHARS - 12);
        let path = format!("/home/u/{filler} x");
        assert!(path.chars().count() <= humanitl_core::block::NOTE_MAX_CHARS);
        let command = humanitl_sandbox::summary::copy_command(std::path::Path::new(&path))
            .expect("the daemon builds a command for a path this plain");
        assert!(
            command.chars().count() > humanitl_core::block::NOTE_MAX_CHARS,
            "the case only bites over the cap: {} chars",
            command.chars().count()
        );
        let link = v1::SymlinkEscape {
            path: "link".to_owned(),
            path_hash: "0123456789abcdef".to_owned(),
            mangled: false,
            target: "/etc".to_owned(),
            escapes: true,
            fix_command: command.clone(),
        };
        let summary = v1::SessionSummary {
            symlinks: vec![link],
            ..v1::SessionSummary::default()
        };
        let value = summary_json(&summary);
        let shown = value["symlinks"][0]["fix_command"]
            .as_str()
            .expect("the command travels as a string");
        assert_eq!(shown, command, "the command is not cut short");
        assert!(shown.ends_with('\''), "the quoting still closes: {shown}");
        // Die Gegenprobe: Gesäubert wäre er kürzer und offen.
        let cut = humanitl_core::block::sanitize_note(&command);
        assert_ne!(cut, command);
        assert!(!cut.ends_with('\''), "{cut}");
    }

    #[test]
    fn cleaning_is_idempotent() {
        let once = clean("a\u{1b}]8;;x\u{7}b");
        assert_eq!(clean(&once), once);
    }
}
