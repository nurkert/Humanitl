//! Der Scanner zu `change_setting_fixes.rs` (HUM-507): findet jeden Bau
//! eines `ChangeSetting` im Rust-Quelltext unter `daemon/` außerhalb von
//! Testcode und jeden Bau von `FixAction.changeSetting` im Dart-Quelltext
//! unter `app/lib/`.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Die Wurzel des Arbeitsbereichs, `daemon/`.
pub fn daemon_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Alle `.rs`-Dateien unter `src` der Crates und Programme.
pub fn sources(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for group in ["crates", "bin"] {
        let Ok(entries) = std::fs::read_dir(root.join(group)) else {
            continue;
        };
        for entry in entries.flatten() {
            walk(&entry.path().join("src"), &mut out);
        }
    }
    out.sort();
    out
}

pub fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

/// Der Text mit Kommentaren und dem Inhalt von Zeichenketten und Zeichen als
/// Leerzeichen, gleich lang wie das Original; die Begrenzer bleiben.
///
/// Damit zählen Klammern in einer Zeichenkette oder einem Kommentar nicht.
pub fn masked(text: &str) -> Vec<u8> {
    let src = text.as_bytes();
    let mut out = src.to_vec();
    let mut i = 0;
    let blank = |out: &mut Vec<u8>, from: usize, to: usize| {
        for byte in &mut out[from..to] {
            if *byte != b'\n' {
                *byte = b' ';
            }
        }
    };
    while i < src.len() {
        match src[i] {
            b'/' if src.get(i + 1) == Some(&b'/') => {
                let end = src[i..]
                    .iter()
                    .position(|&b| b == b'\n')
                    .map_or(src.len(), |n| i + n);
                blank(&mut out, i, end);
                i = end;
            }
            b'/' if src.get(i + 1) == Some(&b'*') => {
                let mut depth = 0usize;
                let start = i;
                while i < src.len() {
                    if src[i] == b'/' && src.get(i + 1) == Some(&b'*') {
                        depth += 1;
                        i += 2;
                    } else if src[i] == b'*' && src.get(i + 1) == Some(&b'/') {
                        depth -= 1;
                        i += 2;
                        if depth == 0 {
                            break;
                        }
                    } else {
                        i += 1;
                    }
                }
                blank(&mut out, start, i);
            }
            b'r' if matches!(src.get(i + 1), Some(b'"' | b'#')) && raw_start(src, i) => {
                let mut j = i + 1;
                let mut hashes = 0;
                while src.get(j) == Some(&b'#') {
                    hashes += 1;
                    j += 1;
                }
                if src.get(j) != Some(&b'"') {
                    i += 1;
                    continue;
                }
                let open = j + 1;
                let mut k = open;
                while k < src.len() {
                    let closes = src
                        .get(k + 1..k + 1 + hashes)
                        .is_some_and(|tail| tail.iter().all(|&b| b == b'#'));
                    if src[k] == b'"' && closes {
                        break;
                    }
                    k += 1;
                }
                blank(&mut out, open, k.min(src.len()));
                i = k + 1 + hashes;
            }
            b'"' => {
                let open = i + 1;
                let mut k = open;
                while k < src.len() && src[k] != b'"' {
                    if src[k] == b'\\' {
                        k += 1;
                    }
                    k += 1;
                }
                blank(&mut out, open, k.min(src.len()));
                i = k + 1;
            }
            b'\'' => {
                // Ein Zeichen (`'a'`, `'\n'`, `'\u{7f}'`) oder eine
                // Lebensdauer (`'a`).
                let close = if src.get(i + 1) == Some(&b'\\') {
                    src[i + 2..]
                        .iter()
                        .position(|&b| b == b'\'')
                        .map(|n| i + 2 + n)
                } else {
                    let len = text[i + 1..].chars().next().map_or(1, char::len_utf8);
                    (src.get(i + 1 + len) == Some(&b'\'')).then_some(i + 1 + len)
                };
                match close {
                    Some(close) => {
                        blank(&mut out, i + 1, close);
                        i = close + 1;
                    }
                    None => i += 1,
                }
            }
            _ => i += 1,
        }
    }
    out
}

/// Ob das `r` bei `i` eine rohe Zeichenkette beginnt (`r"`, `r#"`, `br"`)
/// und nicht das Ende eines Namens ist.
pub fn raw_start(src: &[u8], i: usize) -> bool {
    match i.checked_sub(1).map(|n| src[n]) {
        None => true,
        Some(b'b') => i < 2 || !is_ident(src[i - 2]),
        Some(before) => !is_ident(before),
    }
}

pub const fn is_ident(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Das Ende eines Elements ab `from`: hinter der schließenden Klammer des
/// ersten `{` oder hinter dem ersten `;`, beides außerhalb von `()` und `[]`.
pub fn item_end(code: &[u8], from: usize) -> (usize, bool) {
    let mut depth = 0i32;
    let mut i = from;
    while i < code.len() {
        match code[i] {
            b'(' | b'[' => depth += 1,
            b')' | b']' => depth -= 1,
            b';' if depth == 0 => return (i + 1, true),
            b'{' if depth == 0 => return (matching(code, i) + 1, false),
            _ => {}
        }
        i += 1;
    }
    (code.len(), false)
}

/// Die schließende Klammer zu der öffnenden bei `open`.
pub fn matching(code: &[u8], open: usize) -> usize {
    let (left, right) = (code[open], if code[open] == b'{' { b'}' } else { b')' });
    let mut depth = 0i32;
    for (i, &byte) in code.iter().enumerate().skip(open) {
        if byte == left {
            depth += 1;
        } else if byte == right {
            depth -= 1;
            if depth == 0 {
                return i;
            }
        }
    }
    code.len() - 1
}

/// Ob ein `#[cfg(...)]` nur für Tests baut.
pub fn test_only(attr: &str) -> bool {
    let inner: String = attr.chars().filter(|c| !c.is_whitespace()).collect();
    inner == "#[cfg(test)]" || inner.starts_with("#[cfg(all(test,")
}

/// Die Bereiche einer Datei, die nur in Tests gebaut werden, und die Dateien
/// der Testmodule, die sie mit `mod NAME;` einbindet.
pub fn test_regions(file: &Path, text: &str, code: &[u8]) -> (Vec<(usize, usize)>, Vec<PathBuf>) {
    let mut regions = Vec::new();
    let mut files = Vec::new();
    let mut i = 0;
    while let Some(found) = find(code, b"#[cfg(", i) {
        let close = find(code, b"]", found).unwrap_or(code.len() - 1);
        let attr = &text[found..=close];
        i = close + 1;
        if !test_only(attr) {
            continue;
        }
        // Weitere Attribute vor dem Element überspringen, `#[path]` merken.
        let mut j = close + 1;
        let mut path_attr = None;
        loop {
            while j < code.len() && code[j].is_ascii_whitespace() {
                j += 1;
            }
            if code.get(j) == Some(&b'#') && code.get(j + 1) == Some(&b'[') {
                let end = find(code, b"]", j).unwrap_or(code.len() - 1);
                let attr = &text[j..=end];
                if let Some(rest) = attr.strip_prefix("#[path = \"") {
                    path_attr = rest.strip_suffix("\"]").map(str::to_owned);
                }
                j = end + 1;
            } else {
                break;
            }
        }
        let (end, declared) = item_end(code, j);
        let item = &text[j..end];
        if declared && let Some(name) = mod_name(item) {
            files.extend(module_files(file, &name, path_attr.as_deref()));
        }
        regions.push((found, end));
        i = end;
    }
    (regions, files)
}

pub fn find(code: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    code.get(from..)?
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|n| from + n)
}

/// `NAME` aus `mod NAME;`, auch mit Sichtbarkeit davor.
pub fn mod_name(item: &str) -> Option<String> {
    let words: Vec<&str> = item.trim_end_matches(';').split_whitespace().collect();
    let at = words.iter().position(|word| *word == "mod")?;
    words.get(at + 1).map(|name| (*name).to_owned())
}

/// Die Dateien, in denen `mod NAME;` aus `file` stehen kann.
pub fn module_files(file: &Path, name: &str, path_attr: Option<&str>) -> Vec<PathBuf> {
    let dir = file.parent().unwrap_or(Path::new("."));
    if let Some(path) = path_attr {
        return vec![dir.join(path)];
    }
    let stem = file
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("");
    let base = if matches!(stem, "lib" | "main" | "mod") {
        dir.to_path_buf()
    } else {
        dir.join(stem)
    };
    vec![base.join(format!("{name}.rs")), base.join(name)]
}

/// Ein Bau eines `ChangeSetting` im Quelltext.
#[derive(Debug)]
pub struct Found {
    /// Die Datei, relativ zur Wurzel des Scans.
    pub file: String,
    /// Die Zeile des Baus.
    pub line: usize,
    /// Der Rumpf, Leerraum gefaltet.
    pub body: String,
}

/// Jeder Bau eines `ChangeSetting` außerhalb von Testcode.
pub fn constructions(root: &Path) -> Vec<Found> {
    let files = sources(root);
    let mut texts = BTreeMap::new();
    let mut excluded: Vec<PathBuf> = Vec::new();
    for file in &files {
        let text = std::fs::read_to_string(file).unwrap();
        let code = masked(&text);
        let (regions, test_files) = test_regions(file, &text, &code);
        excluded.extend(test_files);
        texts.insert(file.clone(), (text, code, regions));
    }
    let excluded: Vec<PathBuf> = excluded.iter().map(|path| normalized(path)).collect();
    let mut out = Vec::new();
    for (file, (text, code, regions)) in &texts {
        let file_n = normalized(file);
        if excluded
            .iter()
            .any(|test| file_n == *test || file_n.starts_with(test))
        {
            continue;
        }
        let mut i = 0;
        while let Some(at) = find(code, b"ChangeSetting", i) {
            i = at + 1;
            let before = at.checked_sub(1).map(|n| code[n]);
            if before.is_some_and(is_ident) {
                continue;
            }
            let mut j = at + "ChangeSetting".len();
            while code.get(j).is_some_and(u8::is_ascii_whitespace) {
                j += 1;
            }
            if code.get(j) != Some(&b'{') {
                continue;
            }
            if regions.iter().any(|(from, to)| (*from..*to).contains(&at)) {
                continue;
            }
            let close = matching(code, j);
            let body_code = std::str::from_utf8(&code[j + 1..close]).unwrap();
            let definition = body_code.split(',').any(|part| {
                part.split_once(':')
                    .is_some_and(|(_, ty)| ty.trim() == "String")
            });
            if body_code.contains("..") || definition {
                // Ein Muster mit Rest oder die Definition der Art.
                continue;
            }
            if is_pattern(code, close + 1) {
                continue;
            }
            let body = text[j + 1..close]
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            let relative = file
                .strip_prefix(root)
                .unwrap_or(file)
                .to_string_lossy()
                .into_owned();
            out.push(Found {
                file: relative,
                line: text[..at].lines().count(),
                body,
            });
        }
    }
    out
}

/// Ob hinter der schließenden Klammer ein Muster weitergeht: `=>`, `|`,
/// eine Zuweisung (`let … =`), `if` oder `else`.
pub fn is_pattern(code: &[u8], from: usize) -> bool {
    let mut i = from;
    while code
        .get(i)
        .is_some_and(|b| b.is_ascii_whitespace() || *b == b')')
    {
        i += 1;
    }
    let rest = &code[i.min(code.len())..];
    rest.starts_with(b"=>")
        || rest.starts_with(b"|") && !rest.starts_with(b"||")
        || rest.starts_with(b"=") && !rest.starts_with(b"==")
        || rest.starts_with(b"if ")
        || rest.starts_with(b"else")
}

pub fn normalized(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

/// Ein Literal: `"…".to_owned()`, `"…".to_string()`, `"…".into()`,
/// `String::from("…")` oder `String::new()`.
pub fn literal(expr: &str) -> Option<String> {
    let expr = expr.trim();
    if expr == "String::new()" {
        return Some(String::new());
    }
    let inner = expr
        .strip_prefix("String::from(")
        .and_then(|rest| rest.strip_suffix(')'))
        .or_else(|| {
            [".to_owned()", ".to_string()", ".into()"]
                .iter()
                .find_map(|suffix| expr.strip_suffix(suffix))
        })?;
    let quoted = inner.strip_prefix('"')?.strip_suffix('"')?;
    if quoted.contains('"') && !quoted.contains("\\\"") {
        return None;
    }
    Some(quoted.replace("\\\"", "\"").replace("\\\\", "\\"))
}

/// `key: …, value: …` in seine beiden Ausdrücke.
pub fn fields(body: &str) -> Option<(String, String)> {
    let mut key = None;
    let mut value = None;
    let mut depth = 0i32;
    let mut part = String::new();
    let mut quoted = false;
    let mut parts = Vec::new();
    let mut previous = ' ';
    for c in body.chars() {
        match c {
            '"' if previous != '\\' => quoted = !quoted,
            '(' | '[' | '{' if !quoted => depth += 1,
            ')' | ']' | '}' if !quoted => depth -= 1,
            ',' if !quoted && depth == 0 => {
                parts.push(std::mem::take(&mut part));
                previous = c;
                continue;
            }
            _ => {}
        }
        part.push(c);
        previous = c;
    }
    parts.push(part);
    for part in parts {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (name, expr) = part
            .split_once(':')
            .map_or((part, part), |(name, expr)| (name.trim(), expr.trim()));
        match name {
            "key" => key = Some(expr.to_owned()),
            "value" => value = Some(expr.to_owned()),
            _ => return None,
        }
    }
    Some((key?, value?))
}

/// Jeder Bau von `FixAction.changeSetting(…)` in `app/lib/` (HUM-507).
///
/// Die App baut eigene Befunde (`client_diagnostics.dart`) und übersetzt
/// die des Daemons; beides darf keinen Wert vorschlagen, den
/// `humanitl config set` ablehnt. Kommentare und der Inhalt von
/// Zeichenketten zählen nicht; der Rumpf sind die benannten Argumente.
pub fn dart_constructions(app_lib: &Path) -> Vec<Found> {
    let mut files = Vec::new();
    dart_walk(app_lib, &mut files);
    files.sort();
    let mut out = Vec::new();
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        let code = dart_masked(&text);
        let needle = b"FixAction.changeSetting(";
        let mut i = 0;
        while let Some(at) = find(&code, needle, i) {
            i = at + 1;
            if at > 0 && is_ident(code[at - 1]) {
                continue;
            }
            // Die Deklaration der Art (`const factory FixAction.changeSetting(`).
            if text[..at].trim_end().ends_with("factory") {
                continue;
            }
            let open = at + needle.len() - 1;
            let close = matching(&code, open);
            let body = text[open + 1..close]
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            out.push(Found {
                file: file
                    .strip_prefix(app_lib)
                    .unwrap_or(&file)
                    .to_string_lossy()
                    .into_owned(),
                line: text[..at].lines().count(),
                body,
            });
        }
    }
    out
}

fn dart_walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            dart_walk(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "dart")
            && !path.to_string_lossy().ends_with(".g.dart")
            && !path.to_string_lossy().ends_with(".freezed.dart")
        {
            out.push(path);
        }
    }
}

/// Dart-Quelltext mit Kommentaren und dem Inhalt von Zeichenketten als
/// Leerzeichen, gleich lang wie das Original.
pub fn dart_masked(text: &str) -> Vec<u8> {
    let src = text.as_bytes();
    let mut out = src.to_vec();
    let mut i = 0;
    let blank = |out: &mut Vec<u8>, from: usize, to: usize| {
        for byte in &mut out[from..to.min(src.len())] {
            if *byte != b'\n' {
                *byte = b' ';
            }
        }
    };
    while i < src.len() {
        match src[i] {
            b'/' if src.get(i + 1) == Some(&b'/') => {
                let end = src[i..]
                    .iter()
                    .position(|&b| b == b'\n')
                    .map_or(src.len(), |n| i + n);
                blank(&mut out, i, end);
                i = end;
            }
            b'/' if src.get(i + 1) == Some(&b'*') => {
                let end = find(src, b"*/", i + 2).map_or(src.len(), |n| n + 2);
                blank(&mut out, i, end);
                i = end;
            }
            quote @ (b'\'' | b'"') => {
                let triple = src.get(i..i + 3) == Some(&[quote; 3][..]);
                let raw = i > 0 && src[i - 1] == b'r';
                let open = if triple { i + 3 } else { i + 1 };
                let mut k = open;
                while k < src.len() {
                    if !raw && src[k] == b'\\' {
                        k += 2;
                        continue;
                    }
                    if triple {
                        if src.get(k..k + 3) == Some(&[quote; 3][..]) {
                            break;
                        }
                    } else if src[k] == quote || src[k] == b'\n' {
                        break;
                    }
                    k += 1;
                }
                blank(&mut out, open, k);
                i = k + if triple { 3 } else { 1 };
            }
            _ => i += 1,
        }
    }
    out
}

/// Die Konstanten `static const String NAME = '…';` einer Dart-Datei, etwa
/// `FormKeys.sandboxWorkDir`.
pub fn dart_string_constants(text: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("static const String ") else {
            continue;
        };
        let Some((name, value)) = rest.split_once(" = ") else {
            continue;
        };
        let value = value.trim_end_matches(';').trim();
        if let Some(value) = dart_literal(value) {
            out.insert(name.trim().to_owned(), value);
        }
    }
    out
}

/// Ein Dart-Literal ohne Interpolation: `'…'` oder `"…"`.
pub fn dart_literal(expr: &str) -> Option<String> {
    let expr = expr.trim();
    let quote = expr.chars().next().filter(|c| *c == '\'' || *c == '"')?;
    let inner = expr.strip_prefix(quote)?.strip_suffix(quote)?;
    (!inner.contains(quote) && !inner.contains('$') && !inner.contains('\\'))
        .then(|| inner.to_owned())
}
