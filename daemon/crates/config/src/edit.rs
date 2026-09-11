//! Ein Wert in `config.toml`, geschrieben, ohne dem Menschen seine Datei
//! umzuschreiben (HUM-151).
//!
//! `config.toml` gehört dem Menschen: Er schreibt sie von Hand, mit
//! Kommentaren und in seiner Reihenfolge. Bis HUM-151 schrieb nichts in diesem
//! Repository hinein. Der erste Schreiber ist [`set_sandbox_env`], und er ist so
//! schmal wie sein Anlass: Der Fix eines `TLS_001` setzt eine Variable unter
//! `[sandbox.env]`. Welche Variablen ein Client setzen darf, entscheidet der
//! Aufrufer (`humanitl_ipc::config_rpc`), nicht dieses Modul.
//!
//! Drei Zusagen, und jede hat ihren Grund:
//!
//! - **Nur der eine Wert ändert sich.** Das Dokument wird mit `toml_edit`
//!   geändert und nicht neu serialisiert; Kommentare, Leerzeilen und
//!   Schreibweise bleiben. Danach wird das Ergebnis mit `toml` geparst und mit
//!   der alten Tabelle samt dem neuen Wert verglichen. Weicht es ab, wird nichts
//!   geschrieben: Eine Datei, die mehr ändert als versprochen, ist schlimmer als
//!   ein Befund.
//! - **Nie eine halbe Datei.** Geschrieben wird in eine Nebendatei im selben
//!   Verzeichnis, mit `fsync`, dann `rename`.
//! - **Eine verlinkte Datei bleibt verlinkt.** Wer `config.toml` aus einem
//!   Dotfile-Verzeichnis verlinkt, bekommt das Ziel geändert und nicht den Link
//!   durch eine Kopie ersetzt.

use std::fs::{self, DirBuilder, OpenOptions, Permissions};
use std::io::{self, Write as _};
use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

use humanitl_core::diagnostics::codes::{CONFIG_001, CONFIG_015};
use humanitl_core::{Diagnostic, Severity};
use toml_edit::{DocumentMut, Item, Table};

use crate::paths::{DIR_MODE, FILE_MODE};

/// Was [`set_sandbox_env`] mit der Datei getan hat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Written {
    /// Die Datei trägt den Wert jetzt und trug ihn vorher nicht.
    Changed,
    /// Der Wert stand schon so da; die Datei ist unberührt.
    Unchanged,
}

/// Ordnet die Schreiber dieses Prozesses. Zwei Aufträge zugleich läsen sonst
/// beide die alte Datei, und der zweite `rename` verwürfe den Wert des ersten.
static WRITE_ORDER: Mutex<()> = Mutex::new(());

/// Zähler für die Namen der Nebendateien, damit zwei Threads desselben
/// Prozesses nie denselben wählen.
static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Setzt `sandbox.env.<name>` in der `config.toml` unter `path` auf `value`.
///
/// Eine fehlende Datei gilt als leer und wird angelegt, ihr Verzeichnis mit
/// [`DIR_MODE`], die Datei mit [`FILE_MODE`]; eine vorhandene behält ihre
/// Rechte.
///
/// # Errors
///
/// `CONFIG_001`, wenn die Datei sich nicht lesen lässt oder schon vorher kein
/// TOML ist; sie bleibt dann unberührt. `CONFIG_015`, wenn `sandbox` oder
/// `sandbox.env` dort keine Tabelle ist, wenn die Änderung mehr als den einen
/// Wert änderte oder wenn die Datei sich nicht schreiben lässt.
pub fn set_sandbox_env(path: &Path, name: &str, value: &str) -> Result<Written, Diagnostic> {
    let _order = WRITE_ORDER.lock().unwrap_or_else(PoisonError::into_inner);
    let target = follow_link(path)?;
    let before = read_or_empty(&target)?;
    // Eine Markierung der Byte-Reihenfolge am Anfang und Zeilenenden mit
    // `\r\n` gehören zur Schreibweise des Menschen, und `toml_edit` gibt beides
    // nicht zurück. Sie werden abgenommen und beim Schreiben wieder angelegt.
    let (bom, body) = before
        .strip_prefix(BOM)
        .map_or(("", before.as_str()), |rest| (BOM, rest));
    let crlf = body.contains("\r\n");
    let old: toml::Table = body.parse().map_err(|err: toml::de::Error| {
        unreadable(&target, &format!("is not valid TOML: {err}"))
    })?;
    if current(&old, name) == Some(value) {
        return Ok(Written::Unchanged);
    }

    let not_a_table = "sandbox or sandbox.env there is not a table";
    let expected = with_value(old, name, value)
        .ok_or_else(|| not_edited(&target, name, value, not_a_table))?;
    let mut document: DocumentMut = body.parse().map_err(|err: toml_edit::TomlError| {
        unreadable(&target, &format!("is not valid TOML: {err}"))
    })?;
    insert(&mut document, name, value)
        .ok_or_else(|| not_edited(&target, name, value, not_a_table))?;
    let mut after = document.to_string();
    if crlf {
        after = after.replace("\r\n", "\n").replace('\n', "\r\n");
    }

    // Die Gegenprobe über den zweiten Parser. Kein bekannter Fall schlägt hier
    // an, außer einem Wert `nan` in der Datei, der sich selbst nie gleicht;
    // dann wird eben nicht geschrieben. Sie steht da, weil ein Fehler an dieser
    // Stelle die Datei eines Menschen verändern würde, ohne dass es jemand
    // merkt.
    if after.parse::<toml::Table>().ok().as_ref() != Some(&expected) {
        return Err(not_edited(
            &target,
            name,
            value,
            "the file read back after the edit did not equal the old file plus this one value",
        ));
    }
    write_atomic(&target, format!("{bom}{after}").as_bytes())?;
    Ok(Written::Changed)
}

/// Die Markierung der Byte-Reihenfolge, wie ein Editor sie an den Anfang einer
/// UTF-8-Datei setzen kann.
const BOM: &str = "\u{feff}";

/// Der Wert, der heute unter `sandbox.env.<name>` steht, wenn er ein Text ist.
fn current<'a>(table: &'a toml::Table, name: &str) -> Option<&'a str> {
    table.get("sandbox")?.get("env")?.get(name)?.as_str()
}

/// Die Tabelle, die nach dem Schreiben gelten muss: die alte mit dem einen
/// neuen Wert. `None`, wenn `sandbox` oder `sandbox.env` keine Tabelle ist.
fn with_value(mut table: toml::Table, name: &str, value: &str) -> Option<toml::Table> {
    let sandbox = table
        .entry("sandbox")
        .or_insert_with(|| toml::Value::Table(toml::Table::new()));
    let env = sandbox
        .as_table_mut()?
        .entry("env")
        .or_insert_with(|| toml::Value::Table(toml::Table::new()));
    env.as_table_mut()?
        .insert(name.to_owned(), toml::Value::String(value.to_owned()));
    Some(table)
}

/// Setzt den Wert im Dokument, wo immer `sandbox.env` steht: als eigener
/// Block, als Inline-Tabelle oder als gepunkteter Schlüssel. `None`, wenn eine
/// der beiden Stufen keine Tabelle ist.
///
/// Ein vorhandener Wert wird an seiner Stelle ersetzt und behält, was um ihn
/// herum steht, auch einen Kommentar dahinter.
fn insert(document: &mut DocumentMut, name: &str, value: &str) -> Option<()> {
    let prefix = new_block_prefix(document);
    let sandbox = document.entry("sandbox").or_insert_with(implicit_table);
    let env = sandbox
        .as_table_like_mut()?
        .entry("env")
        .or_insert_with(|| {
            let mut block = Table::new();
            block.decor_mut().set_prefix(prefix);
            Item::Table(block)
        })
        .as_table_like_mut()?;
    if let Some(Item::Value(old)) = env.get_mut(name) {
        let decor = old.decor().clone();
        *old = toml_edit::Value::from(value);
        *old.decor_mut() = decor;
    } else {
        env.insert(name, toml_edit::value(value));
    }
    Some(())
}

/// Was vor einem neuen Block `[sandbox.env]` steht, falls einer entsteht.
///
/// Eine Leerzeile trennt ihn vom Rest, wenn es einen Rest gibt. Gibt es noch
/// gar kein `sandbox`, kommt der Block ans Ende der Datei, und `toml_edit`
/// setzte ihn dort vor die Kommentare, die das Dokument beschließen — in einer
/// Datei, die nur aus Kommentaren besteht, also über ihre Kopfzeile. Diese
/// Kommentare wandern deshalb vor den Block, und der Text des Menschen bleibt
/// der Anfang der Datei.
fn new_block_prefix(document: &mut DocumentMut) -> String {
    let trailing = document.trailing().as_str().unwrap_or_default().to_owned();
    let has_content = !document.is_empty() || !trailing.trim().is_empty();
    let mut prefix = String::new();
    if document.get("sandbox").is_none() {
        prefix = trailing;
        document.set_trailing("");
    }
    if !prefix.is_empty() && !prefix.ends_with('\n') {
        prefix.push('\n');
    }
    if has_content && !prefix.ends_with("\n\n") {
        prefix.push('\n');
    }
    prefix
}

/// Eine Tabelle, die nur als Präfix ihrer Untertabellen dasteht: `[sandbox.env]`
/// ohne ein leeres `[sandbox]` darüber.
fn implicit_table() -> Item {
    let mut table = Table::new();
    table.set_implicit(true);
    Item::Table(table)
}

/// Der Pfad, der wirklich geschrieben wird: das Ziel eines Symlinks, sonst der
/// Pfad selbst, auch wenn es ihn noch nicht gibt.
fn follow_link(path: &Path) -> Result<PathBuf, Diagnostic> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_symlink() => fs::canonicalize(path)
            .map_err(|err| unwritable(path, &format!("is a link that cannot be followed: {err}"))),
        Ok(_) => Ok(path.to_path_buf()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(path.to_path_buf()),
        Err(err) => Err(unreadable(path, &format!("cannot be read: {err}"))),
    }
}

/// Der Text der Datei, oder nichts, wenn es sie nicht gibt.
fn read_or_empty(path: &Path) -> Result<String, Diagnostic> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(text),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(String::new()),
        Err(err) => Err(unreadable(path, &format!("cannot be read: {err}"))),
    }
}

/// Schreibt `bytes` nach `target`: erst in eine Nebendatei im selben
/// Verzeichnis, dann `rename`. Dieselbe Form wie `write_atomic` im CA-Speicher
/// des Proxys.
fn write_atomic(target: &Path, bytes: &[u8]) -> Result<(), Diagnostic> {
    let (Some(dir), Some(name)) = (target.parent(), target.file_name()) else {
        return Err(unwritable(target, "has no directory to be written into"));
    };
    let mode = match fs::metadata(target) {
        Ok(meta) => meta.permissions().mode() & 0o777,
        Err(err) if err.kind() == io::ErrorKind::NotFound => FILE_MODE,
        Err(err) => return Err(unwritable(target, &format!("cannot be inspected: {err}"))),
    };
    DirBuilder::new()
        .recursive(true)
        .mode(DIR_MODE)
        .create(dir)
        .map_err(|err| {
            unwritable(
                target,
                &format!("has no directory that could be created: {err}"),
            )
        })?;
    // Der Name ist nicht vorhersagbar, und `create_new` legt nie eine Datei
    // über einen vorhandenen Pfad, auch nicht über einen Symlink.
    let tmp = dir.join(format!(
        ".{}.tmp-{}-{}",
        name.to_string_lossy(),
        std::process::id(),
        TMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| -> io::Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
            .open(&tmp)?;
        file.set_permissions(Permissions::from_mode(mode))?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&tmp, target)
    })();
    result.map_err(|err| {
        // Best effort: Die Nebendatei bleibt nicht liegen; der Befund bleibt derselbe.
        let _ = fs::remove_file(&tmp);
        unwritable(target, &format!("could not be written: {err}"))
    })
}

/// `CONFIG_001`: Die Datei ist nicht lesbar oder kein TOML.
fn unreadable(path: &Path, what: &str) -> Diagnostic {
    Diagnostic::builder(CONFIG_001, Severity::Error)
        .why(format!(
            "{} {what}. Humanitl does not rewrite a file it cannot read; nothing was written.",
            path.display()
        ))
        .build()
}

/// `CONFIG_015`: Die Änderung hätte mehr getan als versprochen.
fn not_edited(path: &Path, name: &str, value: &str, reason: &str) -> Diagnostic {
    let line = format!("{name} = {}", toml::Value::String(value.to_owned()));
    Diagnostic::builder(CONFIG_015, Severity::Error)
        .why(format!(
            "{} was not changed: {reason}. Add {line} under [sandbox.env] by hand.",
            path.display()
        ))
        .build()
}

/// `CONFIG_015`: Die Datei ließ sich nicht schreiben.
fn unwritable(path: &Path, what: &str) -> Diagnostic {
    Diagnostic::builder(CONFIG_015, Severity::Error)
        .why(format!("{} {what}; nothing was written.", path.display()))
        .build()
}
