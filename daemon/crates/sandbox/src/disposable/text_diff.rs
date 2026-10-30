//! Der Unterschied zweier Textdateien im Format `diff -u` (HUM-350).
//!
//! Der Daemon rechnet ihn, nicht der Client (ADR-018): Kommandozeile und
//! Oberfläche zeigen dieselben Zeilen. Der Algorithmus ist der von Myers
//! (1986) über Zeilen, mit gemeinsamem Anfang und Ende vorab abgeschnitten
//! und mit einer Obergrenze für die Zahl der Änderungen; darüber gibt es
//! keinen Text-Diff, sondern Größe und Hash, wie bei Binärdateien.
//!
//! Die Zeilen kommen vom Agenten. Steuerzeichen außer dem Tabulator und die
//! unsichtbaren Zeichen, mit denen sich ein anderer Text vortäuschen lässt
//! (Bidi-Steuerung, Zero-Width), erscheinen deshalb maskiert als `\u{…}`:
//! Was ein Mensch übernimmt, soll er so sehen, wie es in der Datei steht.

use std::fmt::Write as _;

use crate::summary::looks_like_text;

/// Bis zu dieser Größe je Seite gibt es einen Text-Diff.
pub const DIFF_MAX_BYTES: u64 = 1024 * 1024;

/// So lang wird ein Diff höchstens; danach steht eine Zeile, dass gekürzt
/// wurde.
pub const DIFF_MAX_OUTPUT: usize = 64 * 1024;

/// So viele eingefügte und gelöschte Zeilen rechnet der Algorithmus
/// höchstens; sein Speicher wächst mit dem Quadrat dieser Zahl.
const MAX_EDITS: usize = 1000;

/// Zeilen Kontext um eine Änderung.
const CONTEXT: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Equal(usize, usize),
    Delete(usize),
    Insert(usize),
}

/// Der Unterschied von `old` zu `new` im Format `diff -u`, mit `name` in
/// den Kopfzeilen.
///
/// `None`, wenn eine Seite kein Text ist (ein NUL-Byte am Anfang) oder sich
/// mehr als die Obergrenze an Zeilen geändert hat. Ein leerer Text heißt:
/// kein Unterschied.
#[must_use]
pub fn unified(old: &[u8], new: &[u8], name: &str) -> Option<String> {
    if !looks_like_text(old) || !looks_like_text(new) {
        return None;
    }
    let old = String::from_utf8_lossy(old);
    let new = String::from_utf8_lossy(new);
    let a: Vec<&str> = lines(&old);
    let b: Vec<&str> = lines(&new);
    let ops = diff_lines(&a, &b)?;
    if ops.iter().all(|op| matches!(op, Op::Equal(..))) {
        return Some(String::new());
    }
    let mut out = format!("--- a/{0}\n+++ b/{0}\n", escape(name));
    for hunk in hunks(&ops) {
        let (old_start, old_len, new_start, new_len) = hunk_range(&ops, &hunk);
        let _ = writeln!(out, "@@ -{old_start},{old_len} +{new_start},{new_len} @@");
        for op in &ops[hunk] {
            let (tag, text) = match *op {
                Op::Equal(i, _) => (' ', a[i]),
                Op::Delete(i) => ('-', a[i]),
                Op::Insert(j) => ('+', b[j]),
            };
            out.push(tag);
            out.push_str(&escape(text));
            out.push('\n');
            if out.len() > DIFF_MAX_OUTPUT {
                out.truncate(floor_char_boundary(&out, DIFF_MAX_OUTPUT));
                out.push_str("\n[diff cut at 64 KiB]\n");
                return Some(out);
            }
        }
    }
    Some(out)
}

fn lines(text: &str) -> Vec<&str> {
    if text.is_empty() {
        return Vec::new();
    }
    text.strip_suffix('\n')
        .unwrap_or(text)
        .split('\n')
        .collect()
}

/// Die Folge der Operationen von `a` nach `b`.
fn diff_lines(a: &[&str], b: &[&str]) -> Option<Vec<Op>> {
    let prefix = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let suffix = a[prefix..]
        .iter()
        .rev()
        .zip(b[prefix..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    let middle = myers(&a[prefix..a.len() - suffix], &b[prefix..b.len() - suffix])?;
    let mut ops: Vec<Op> = (0..prefix).map(|i| Op::Equal(i, i)).collect();
    ops.extend(middle.into_iter().map(|op| match op {
        Op::Equal(i, j) => Op::Equal(i + prefix, j + prefix),
        Op::Delete(i) => Op::Delete(i + prefix),
        Op::Insert(j) => Op::Insert(j + prefix),
    }));
    let (a_tail, b_tail) = (a.len() - suffix, b.len() - suffix);
    ops.extend((0..suffix).map(|i| Op::Equal(a_tail + i, b_tail + i)));
    Some(ops)
}

/// Myers über zwei Folgen; `None` über [`MAX_EDITS`] Änderungen.
///
/// `furthest[diagonal]` ist die weiteste Stelle in `old`, die auf dieser
/// Diagonale mit `edits` Änderungen erreichbar ist; `trace` hält den Stand vor
/// jedem Schritt für den Weg zurück.
#[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)]
fn myers(old: &[&str], new: &[&str]) -> Option<Vec<Op>> {
    let (old_len, new_len) = (old.len() as isize, new.len() as isize);
    let limit = (old.len() + new.len()).min(MAX_EDITS) as isize;
    let offset = limit + 1;
    let mut furthest = vec![0_isize; (2 * limit + 3) as usize];
    let mut trace: Vec<Vec<isize>> = Vec::new();
    let slot = |diagonal: isize| (diagonal + offset) as usize;
    for edits in 0..=limit {
        // Der Stand vor diesem Schritt, für die Diagonalen -(edits+1) bis edits+1.
        trace.push(furthest[slot(-edits - 1)..=slot(edits + 1)].to_vec());
        let mut diagonal = -edits;
        while diagonal <= edits {
            let down = diagonal == -edits
                || (diagonal != edits
                    && furthest[slot(diagonal - 1)] < furthest[slot(diagonal + 1)]);
            let mut old_pos = if down {
                furthest[slot(diagonal + 1)]
            } else {
                furthest[slot(diagonal - 1)] + 1
            };
            let mut new_pos = old_pos - diagonal;
            while old_pos < old_len
                && new_pos < new_len
                && old[old_pos as usize] == new[new_pos as usize]
            {
                old_pos += 1;
                new_pos += 1;
            }
            furthest[slot(diagonal)] = old_pos;
            if old_pos >= old_len && new_pos >= new_len {
                return Some(backtrack(&trace, old_len, new_len));
            }
            diagonal += 2;
        }
    }
    None
}

/// Der Weg zurück durch `trace`, vom Ende zum Anfang, als Folge der
/// Operationen von vorn.
#[allow(clippy::cast_sign_loss, clippy::cast_possible_wrap)]
fn backtrack(trace: &[Vec<isize>], old_len: isize, new_len: isize) -> Vec<Op> {
    let mut ops = Vec::new();
    let (mut old_pos, mut new_pos) = (old_len, new_len);
    for (edits, furthest) in trace.iter().enumerate().rev() {
        let edits = edits as isize;
        let at = |diagonal: isize| furthest[(diagonal + edits + 1) as usize];
        let diagonal = old_pos - new_pos;
        let previous =
            if diagonal == -edits || (diagonal != edits && at(diagonal - 1) < at(diagonal + 1)) {
                diagonal + 1
            } else {
                diagonal - 1
            };
        let previous_old = at(previous);
        let previous_new = previous_old - previous;
        while old_pos > previous_old && new_pos > previous_new {
            ops.push(Op::Equal((old_pos - 1) as usize, (new_pos - 1) as usize));
            old_pos -= 1;
            new_pos -= 1;
        }
        if edits > 0 {
            if old_pos == previous_old {
                ops.push(Op::Insert((new_pos - 1) as usize));
            } else {
                ops.push(Op::Delete((old_pos - 1) as usize));
            }
        }
        old_pos = previous_old;
        new_pos = previous_new;
    }
    ops.reverse();
    ops
}

/// Die Bereiche der Operationen, die je einen Abschnitt bilden: jede
/// Änderung mit [`CONTEXT`] Zeilen davor und danach, nahe Abschnitte
/// zusammengelegt.
fn hunks(ops: &[Op]) -> Vec<std::ops::Range<usize>> {
    let mut out: Vec<std::ops::Range<usize>> = Vec::new();
    for (index, op) in ops.iter().enumerate() {
        if matches!(op, Op::Equal(..)) {
            continue;
        }
        let start = index.saturating_sub(CONTEXT);
        let end = (index + CONTEXT + 1).min(ops.len());
        match out.last_mut() {
            Some(last) if start <= last.end => last.end = last.end.max(end),
            _ => out.push(start..end),
        }
    }
    out
}

/// Anfang und Länge eines Abschnitts auf beiden Seiten, 1-basiert wie
/// `diff -u`; eine Seite ohne Zeilen im Abschnitt nennt die Zeile davor
/// (`@@ -0,0 +1,3 @@` für eine neue Datei).
fn hunk_range(ops: &[Op], hunk: &std::ops::Range<usize>) -> (usize, usize, usize, usize) {
    let consumes = |op: &Op| match op {
        Op::Equal(..) => (1, 1),
        Op::Delete(_) => (1, 0),
        Op::Insert(_) => (0, 1),
    };
    let sum = |slice: &[Op]| {
        slice
            .iter()
            .map(consumes)
            .fold((0, 0), |(a, b), (x, y)| (a + x, b + y))
    };
    let (old_before, new_before) = sum(&ops[..hunk.start]);
    let (old_len, new_len) = sum(&ops[hunk.clone()]);
    let start = |before: usize, len: usize| if len == 0 { before } else { before + 1 };
    (
        start(old_before, old_len),
        old_len,
        start(new_before, new_len),
        new_len,
    )
}

/// Maskiert Steuerzeichen außer dem Tabulator und unsichtbare Zeichen.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        let hidden = matches!(
            ch,
            '\u{061C}'
                | '\u{200B}'..='\u{200F}'
                | '\u{2028}'..='\u{202E}'
                | '\u{2060}'..='\u{2064}'
                | '\u{2066}'..='\u{2069}'
                | '\u{FEFF}'
        );
        if (ch.is_control() && ch != '\t') || hidden {
            let _ = write!(out, "\\u{{{:04x}}}", u32::from(ch));
        } else {
            out.push(ch);
        }
    }
    out
}

fn floor_char_boundary(text: &str, mut index: usize) -> usize {
    while index > 0 && !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn a_changed_line_shows_with_its_context() {
        let old = b"one\ntwo\nthree\nfour\nfive\nsix\nseven\n";
        let new = b"one\ntwo\nthree\nFOUR\nfive\nsix\nseven\n";
        let diff = unified(old, new, "f.txt").unwrap();
        assert_eq!(
            diff,
            "--- a/f.txt\n+++ b/f.txt\n@@ -1,7 +1,7 @@\n one\n two\n three\n-four\n+FOUR\n five\n six\n seven\n"
        );
    }

    #[test]
    fn a_new_file_is_all_insertions() {
        let diff = unified(b"", b"a\nb\n", "n").unwrap();
        assert_eq!(diff, "--- a/n\n+++ b/n\n@@ -0,0 +1,2 @@\n+a\n+b\n");
    }

    #[test]
    fn a_removed_file_is_all_deletions() {
        let diff = unified(b"a\nb\n", b"", "n").unwrap();
        assert_eq!(diff, "--- a/n\n+++ b/n\n@@ -1,2 +0,0 @@\n-a\n-b\n");
    }

    #[test]
    fn equal_texts_have_an_empty_diff() {
        assert_eq!(unified(b"same\n", b"same\n", "s").unwrap(), "");
    }

    #[test]
    fn binary_content_has_no_text_diff() {
        assert!(unified(b"\0\x01", b"text", "b").is_none());
    }

    #[test]
    fn far_apart_changes_are_two_hunks() {
        let (mut old, mut new) = (String::new(), String::new());
        for line in 0..30 {
            let _ = writeln!(old, "{line}");
            match line {
                2 => new.push_str("two\n"),
                27 => new.push_str("twenty-seven\n"),
                _ => {
                    let _ = writeln!(new, "{line}");
                }
            }
        }
        let diff = unified(old.as_bytes(), new.as_bytes(), "x").unwrap();
        assert_eq!(diff.matches("@@ -").count(), 2, "{diff}");
        assert!(diff.contains("-27\n+twenty-seven\n"), "{diff}");
    }

    #[test]
    fn control_and_bidi_characters_are_shown_masked() {
        let diff = unified(b"", "a\x1b[2Jb\u{202E}c\n".as_bytes(), "e").unwrap();
        assert!(diff.contains("+a\\u{001b}[2Jb\\u{202e}c\n"), "{diff}");
        assert!(!diff.contains('\x1b'));
    }

    #[test]
    fn line_separators_and_invisible_marks_are_shown_masked() {
        let text = "a\u{061C}b\u{2028}c\u{2029}d\u{2060}e\u{2064}f\n";
        let diff = unified(b"", text.as_bytes(), "e").unwrap();
        for code in ["061c", "2028", "2029", "2060", "2064"] {
            assert!(diff.contains(&format!("\\u{{{code}}}")), "{code}: {diff}");
        }
        for ch in ['\u{061C}', '\u{2028}', '\u{2029}', '\u{2060}', '\u{2064}'] {
            assert!(!diff.contains(ch), "{ch:?} reached the diff");
        }
    }

    #[test]
    fn too_many_changes_give_no_text_diff() {
        let (mut old, mut new) = (String::new(), String::new());
        for line in 0..3000 {
            let _ = writeln!(old, "a{line}");
            let _ = writeln!(new, "b{line}");
        }
        assert!(unified(old.as_bytes(), new.as_bytes(), "big").is_none());
    }
}
