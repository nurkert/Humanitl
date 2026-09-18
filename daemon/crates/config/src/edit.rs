//! Ein Wert in `config.toml`, geschrieben, ohne dem Menschen seine Datei
//! umzuschreiben (HUM-151, verallgemeinert in HUM-070).
//!
//! `config.toml` gehört dem Menschen: Er schreibt sie von Hand, mit
//! Kommentaren und in seiner Reihenfolge. Zwei Schreiber gibt es: den Fix
//! eines `TLS_001`, der eine Variable unter `[sandbox.env]` setzt
//! ([`set_sandbox_env`], über `humanitl_ipc::config_rpc`), und
//! `humanitl config set` ([`set_value`]). Beide gehen durch dieselbe Funktion,
//! damit es genau einen Weg in diese Datei gibt. Welche Schlüssel ein Client
//! setzen darf, entscheidet der Aufrufer, nicht dieses Modul.
//!
//! Vier Zusagen, und jede hat ihren Grund:
//!
//! - **Nur der eine Wert ändert sich.** Das Dokument wird mit `toml_edit`
//!   geändert und nicht neu serialisiert; Kommentare, Leerzeilen und
//!   Schreibweise bleiben. Danach wird das Ergebnis mit `toml` geparst und mit
//!   der alten Tabelle samt dem neuen Wert verglichen. Weicht es ab, wird nichts
//!   geschrieben: Eine Datei, die mehr ändert als versprochen, ist schlimmer als
//!   ein Befund.
//! - **Nie eine halbe Datei.** Geschrieben wird in eine Nebendatei im selben
//!   Verzeichnis, mit `fsync`, dann `rename`. Vor dem `rename` darf der Aufrufer
//!   die Nebendatei prüfen (`check`); lehnt er ab, verschwindet sie, und die
//!   Datei bleibt, wie sie war.
//! - **Eine verlinkte Datei bleibt verlinkt.** Wer `config.toml` aus einem
//!   Dotfile-Verzeichnis verlinkt, bekommt das Ziel geändert und nicht den Link
//!   durch eine Kopie ersetzt. Eine Markierung der Byte-Reihenfolge und
//!   Zeilenenden mit `\r\n` bleiben ebenfalls.
//! - **Kein Schreiber verliert den Wert eines anderen.** Innerhalb eines
//!   Prozesses ordnet ein Mutex, über Prozesse hinweg eine `flock`-Sperre auf
//!   dem Verzeichnis der Datei: Der Daemon (`SetConfig`) und eine zweite
//!   Kommandozeile lesen die Datei erst, wenn der erste Schreiber umbenannt hat.

use std::fs::{self, DirBuilder, File, OpenOptions, Permissions};
use std::io::{self, Write as _};
use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

use humanitl_core::diagnostics::codes::{CONFIG_001, CONFIG_015};
use humanitl_core::{Diagnostic, Severity};
use rustix::fs::{FlockOperation, flock};
use toml_edit::{DocumentMut, Item, Table};

use crate::paths::{DIR_MODE, FILE_MODE};

/// Was ein Schreiben mit der Datei getan hat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Written {
    /// Die Datei trägt den Wert jetzt und trug ihn vorher nicht.
    Changed,
    /// Der Wert stand schon so da; die Datei ist unberührt.
    Unchanged,
}

/// Ordnet die Schreiber dieses Prozesses; die `flock`-Sperre aus [`DirLock`]
/// ordnet die über Prozesse hinweg. Zwei Aufträge zugleich läsen sonst beide
/// die alte Datei, und der zweite `rename` verwürfe den Wert des ersten.
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
/// Wie [`set_value`].
pub fn set_sandbox_env(path: &Path, name: &str, value: &str) -> Result<Written, Diagnostic> {
    let segments = ["sandbox".to_owned(), "env".to_owned(), name.to_owned()];
    set_value(
        path,
        &segments,
        Some(&toml::Value::String(value.to_owned())),
        |_| Ok(()),
    )
}

/// Setzt den Wert unter dem Pfad `segments` in der TOML-Datei `path`, oder
/// entfernt ihn, wenn `value` `None` ist.
///
/// Fehlende Tabellen auf dem Weg entstehen; die letzte davon als eigener
/// Block `[a.b]`, die darüber nur als Präfix, damit kein leeres `[a]` in der
/// Datei steht. Wo der Weg schon steht — als Block, als Inline-Tabelle oder
/// als gepunkteter Schlüssel —, wird er in seiner eigenen Form ergänzt.
///
/// `check` bekommt die fertige Nebendatei, bevor sie die Datei ersetzt, und
/// kann sie ablehnen; `humanitl config set` lädt damit die Konfiguration so,
/// wie der nächste Start sie laden wird. Steht der Wert schon so da, bekommt
/// `check` die Datei selbst, und sie bleibt in jedem Fall unberührt.
///
/// # Errors
///
/// `CONFIG_001`, wenn die Datei sich nicht lesen lässt oder schon vorher kein
/// TOML ist; sie bleibt dann unberührt. `CONFIG_015`, wenn ein Schritt des
/// Pfades dort keine Tabelle ist, wenn die Änderung mehr als den einen Wert
/// änderte oder wenn die Datei sich nicht schreiben lässt. Der Befund von
/// `check`, wenn der die Nebendatei ablehnt.
pub fn set_value(
    path: &Path,
    segments: &[String],
    value: Option<&toml::Value>,
    check: impl FnOnce(&Path) -> Result<(), Diagnostic>,
) -> Result<Written, Diagnostic> {
    let _order = WRITE_ORDER.lock().unwrap_or_else(PoisonError::into_inner);
    let target = follow_link(path)?;
    let _lock = DirLock::acquire(&target)?;
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

    let not_a_table = "a step of the path there is not a table";
    let expected = match value {
        Some(value) => with_value(old.clone(), segments, value),
        None => without_value(old.clone(), segments),
    }
    .ok_or_else(|| not_edited(&target, segments, value, not_a_table))?;
    if expected == old {
        // Ein Wert, der schon so dasteht, wird trotzdem geprüft, und zwar an
        // der Datei selbst: Stand er falsch da, bekommt der Mensch den Befund
        // und kein „geschrieben". Ein Entfernen, das nichts entfernt, bringt
        // keinen Wert mit und bleibt ungeprüft.
        if value.is_some() {
            check(&target)?;
        }
        return Ok(Written::Unchanged);
    }

    let mut document: DocumentMut = body.parse().map_err(|err: toml_edit::TomlError| {
        unreadable(&target, &format!("is not valid TOML: {err}"))
    })?;
    match value {
        Some(value) => insert(&mut document, segments, value),
        None => remove(&mut document, segments),
    }
    .ok_or_else(|| not_edited(&target, segments, value, not_a_table))?;
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
            segments,
            value,
            "the file read back after the edit did not equal the old file plus this one change",
        ));
    }
    write_atomic(&target, format!("{bom}{after}").as_bytes(), check)?;
    Ok(Written::Changed)
}

/// Die Markierung der Byte-Reihenfolge, wie ein Editor sie an den Anfang einer
/// UTF-8-Datei setzen kann.
const BOM: &str = "\u{feff}";

/// Die Tabelle, die nach dem Schreiben gelten muss: die alte mit dem einen
/// neuen Wert. `None`, wenn ein Schritt des Pfades keine Tabelle ist.
fn with_value(
    mut table: toml::Table,
    segments: &[String],
    value: &toml::Value,
) -> Option<toml::Table> {
    let (last, groups) = segments.split_last()?;
    let mut cursor = &mut table;
    for group in groups {
        cursor = cursor
            .entry(group.clone())
            .or_insert_with(|| toml::Value::Table(toml::Table::new()))
            .as_table_mut()?;
    }
    cursor.insert(last.clone(), value.clone());
    Some(table)
}

/// Die Tabelle ohne den einen Wert. Fehlt er schon, ist sie die alte.
/// `None`, wenn ein Schritt des Pfades keine Tabelle ist.
fn without_value(mut table: toml::Table, segments: &[String]) -> Option<toml::Table> {
    let (last, groups) = segments.split_last()?;
    let mut cursor = &mut table;
    for group in groups {
        match cursor.get_mut(group) {
            None => return Some(table),
            Some(next) => cursor = next.as_table_mut()?,
        }
    }
    cursor.remove(last);
    Some(table)
}

/// Setzt den Wert im Dokument. `None`, wenn ein Schritt keine Tabelle ist.
///
/// Ein vorhandener Wert wird an seiner Stelle ersetzt und behält, was um ihn
/// herum steht, auch einen Kommentar dahinter. Ein vorhandener Block
/// `[a.b]`, der eine Tabelle ersetzt bekommt, bleibt ein Block.
fn insert(document: &mut DocumentMut, segments: &[String], value: &toml::Value) -> Option<()> {
    let (last, groups) = segments.split_last()?;
    let prefix = new_block_prefix(document, groups);
    let mut cursor: &mut dyn toml_edit::TableLike = document.as_table_mut();
    for (index, group) in groups.iter().enumerate() {
        let is_last_group = index + 1 == groups.len();
        let prefix = prefix.clone();
        cursor = cursor
            .entry(group)
            .or_insert_with(|| {
                let mut block = Table::new();
                if is_last_group {
                    block.decor_mut().set_prefix(prefix);
                } else {
                    block.set_implicit(true);
                }
                Item::Table(block)
            })
            .as_table_like_mut()?;
    }
    let new = edit_value(value)?;
    match cursor.get_mut(last) {
        Some(Item::Value(old)) => {
            let decor = old.decor().clone();
            *old = new;
            *old.decor_mut() = decor;
        }
        Some(Item::Table(old)) => {
            // Ein Block bleibt ein Block: Seine Kopfzeile und was davor steht
            // gehören dem Menschen.
            let toml_edit::Value::InlineTable(inline) = new else {
                return None;
            };
            let decor = old.decor().clone();
            let mut block = inline.into_table();
            *block.decor_mut() = decor;
            *old = block;
        }
        _ => {
            cursor.insert(last, Item::Value(new));
        }
    }
    Some(())
}

/// Entfernt den Wert aus dem Dokument. Fehlt er, geschieht nichts.
fn remove(document: &mut DocumentMut, segments: &[String]) -> Option<()> {
    let (last, groups) = segments.split_last()?;
    let mut cursor: &mut dyn toml_edit::TableLike = document.as_table_mut();
    for group in groups {
        match cursor.get_mut(group) {
            None => return Some(()),
            Some(next) => cursor = next.as_table_like_mut()?,
        }
    }
    cursor.remove(last);
    Some(())
}

/// Ein TOML-Wert als `toml_edit`-Wert, Tabellen als Inline-Tabellen.
fn edit_value(value: &toml::Value) -> Option<toml_edit::Value> {
    Some(match value {
        toml::Value::String(text) => toml_edit::Value::from(text.as_str()),
        toml::Value::Integer(number) => toml_edit::Value::from(*number),
        toml::Value::Float(number) => toml_edit::Value::from(*number),
        toml::Value::Boolean(flag) => toml_edit::Value::from(*flag),
        toml::Value::Datetime(at) => at.to_string().parse::<toml_edit::Value>().ok()?,
        toml::Value::Array(items) => {
            let mut array = toml_edit::Array::new();
            for item in items {
                array.push(edit_value(item)?);
            }
            toml_edit::Value::Array(array)
        }
        toml::Value::Table(entries) => {
            let mut inline = toml_edit::InlineTable::new();
            for (key, item) in entries {
                inline.insert(key, edit_value(item)?);
            }
            toml_edit::Value::InlineTable(inline)
        }
    })
}

/// Was vor einem neuen Block steht, falls einer entsteht.
///
/// Eine Leerzeile trennt ihn vom Rest, wenn es einen Rest gibt. Gibt es die
/// oberste Tabelle des Pfades noch gar nicht, kommt der Block ans Ende der
/// Datei, und `toml_edit` setzte ihn dort vor die Kommentare, die das Dokument
/// beschließen — in einer Datei, die nur aus Kommentaren besteht, also über
/// ihre Kopfzeile. Diese Kommentare wandern deshalb vor den Block, und der
/// Text des Menschen bleibt der Anfang der Datei.
fn new_block_prefix(document: &mut DocumentMut, groups: &[String]) -> String {
    let Some(top) = groups.first() else {
        return String::new();
    };
    let trailing = document.trailing().as_str().unwrap_or_default().to_owned();
    let has_content = !document.is_empty() || !trailing.trim().is_empty();
    let mut prefix = String::new();
    if document.get(top).is_none() {
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

/// Die Sperre über alle Schreiber einer Datei, auch in anderen Prozessen.
///
/// Gesperrt wird das Verzeichnis der Datei und nicht eine Nebendatei: Eine
/// Sperrdatei bliebe als Rest neben `config.toml` liegen, und das Verzeichnis
/// gibt es ohnehin. Die Sperre endet mit dem Schließen des Deskriptors.
struct DirLock {
    /// Hält die Sperre, solange es lebt.
    _dir: File,
}

impl DirLock {
    /// Legt das Verzeichnis an, wenn es fehlt, und sperrt es exklusiv.
    fn acquire(target: &Path) -> Result<Self, Diagnostic> {
        let dir = target
            .parent()
            .filter(|dir| !dir.as_os_str().is_empty())
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
        DirBuilder::new()
            .recursive(true)
            .mode(DIR_MODE)
            .create(&dir)
            .map_err(|err| {
                unwritable(
                    target,
                    &format!("has no directory that could be created: {err}"),
                )
            })?;
        let handle = File::open(&dir).map_err(|err| {
            unwritable(
                target,
                &format!("has a directory that cannot be opened: {err}"),
            )
        })?;
        flock(&handle, FlockOperation::LockExclusive).map_err(|err| {
            unwritable(
                target,
                &format!("could not be locked against other writers: {err}"),
            )
        })?;
        Ok(Self { _dir: handle })
    }
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
/// Verzeichnis, dann `check`, dann `rename`. Dieselbe Form wie `write_atomic`
/// im CA-Speicher des Proxys.
fn write_atomic(
    target: &Path,
    bytes: &[u8],
    check: impl FnOnce(&Path) -> Result<(), Diagnostic>,
) -> Result<(), Diagnostic> {
    let (Some(dir), Some(name)) = (target.parent(), target.file_name()) else {
        return Err(unwritable(target, "has no directory to be written into"));
    };
    let mode = match fs::metadata(target) {
        Ok(meta) => meta.permissions().mode() & 0o777,
        Err(err) if err.kind() == io::ErrorKind::NotFound => FILE_MODE,
        Err(err) => return Err(unwritable(target, &format!("cannot be inspected: {err}"))),
    };
    // Der Name ist nicht vorhersagbar, und `create_new` legt nie eine Datei
    // über einen vorhandenen Pfad, auch nicht über einen Symlink.
    let tmp = dir.join(format!(
        ".{}.tmp-{}-{}",
        name.to_string_lossy(),
        std::process::id(),
        TMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let written = (|| -> io::Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
            .open(&tmp)?;
        file.set_permissions(Permissions::from_mode(mode))?;
        file.write_all(bytes)?;
        file.sync_all()
    })();
    if let Err(err) = written {
        // Best effort: Die Nebendatei bleibt nicht liegen; der Befund bleibt derselbe.
        let _ = fs::remove_file(&tmp);
        return Err(unwritable(target, &format!("could not be written: {err}")));
    }
    if let Err(refused) = check(&tmp) {
        let _ = fs::remove_file(&tmp);
        return Err(refused);
    }
    fs::rename(&tmp, target).map_err(|err| {
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
///
/// Der Satz nennt die Zeile, die ein Mensch von Hand schreiben kann, und den
/// Block, unter den sie gehört.
fn not_edited(
    path: &Path,
    segments: &[String],
    value: Option<&toml::Value>,
    reason: &str,
) -> Diagnostic {
    let (last, groups) = segments
        .split_last()
        .map_or(("", &[][..]), |(last, groups)| (last.as_str(), groups));
    let block = groups.join(".");
    let by_hand = match value {
        Some(value) => format!("Add {last} = {value} under [{block}] by hand."),
        None => format!("Remove {last} from [{block}] by hand."),
    };
    Diagnostic::builder(CONFIG_015, Severity::Error)
        .why(format!(
            "{} was not changed: {reason}. {by_hand}",
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::fs;

    use humanitl_core::diagnostics::codes::CONFIG_003;
    use humanitl_core::{Diagnostic, Severity};

    use super::{Written, set_value};

    fn segments(path: &str) -> Vec<String> {
        path.split('.').map(ToOwned::to_owned).collect()
    }

    /// Eine Tabelle als Wert ersetzt einen vorhandenen Block und bleibt dabei
    /// ein Block, mit dem Kommentar darüber.
    #[test]
    fn a_table_value_keeps_the_block_it_replaces() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "# mine\n[sandbox.env]\nOLD = \"x\"\n").unwrap();

        let mut table = toml::Table::new();
        table.insert("FOO".to_owned(), toml::Value::String("bar".to_owned()));
        let written = set_value(
            &path,
            &segments("sandbox.env"),
            Some(&toml::Value::Table(table)),
            |_| Ok(()),
        )
        .unwrap();

        assert_eq!(written, Written::Changed);
        let after = fs::read_to_string(&path).unwrap();
        assert!(after.starts_with("# mine\n[sandbox.env]\n"), "{after}");
        assert!(after.contains("FOO = \"bar\""), "{after}");
        assert!(!after.contains("OLD"), "{after}");
    }

    /// `None` entfernt den Wert und lässt den Rest stehen; ein fehlender Wert
    /// ist kein Fehler, sondern nichts zu tun.
    #[test]
    fn none_removes_the_value_and_nothing_else() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(
            &path,
            "[llm]\n# keep\nendpoint = \"http://a\"\nmodel = \"m\"\n",
        )
        .unwrap();

        assert_eq!(
            set_value(&path, &segments("llm.endpoint"), None, |_| Ok(())).unwrap(),
            Written::Changed
        );
        let after = fs::read_to_string(&path).unwrap();
        assert!(!after.contains("endpoint"), "{after}");
        assert!(after.contains("model = \"m\""), "{after}");

        assert_eq!(
            set_value(&path, &segments("llm.endpoint"), None, |_| Ok(())).unwrap(),
            Written::Unchanged
        );
        assert_eq!(
            set_value(&path, &segments("nope.endpoint"), None, |_| Ok(())).unwrap(),
            Written::Unchanged
        );
    }

    /// Lehnt `check` ab, bleibt die Datei, wie sie war, und keine Nebendatei
    /// liegt herum.
    #[test]
    fn a_refused_check_leaves_the_file_and_no_leftover() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "[hold]\ntimeout_secs = 42\n").unwrap();

        let refused = set_value(
            &path,
            &segments("hold.timeout_secs"),
            Some(&toml::Value::Integer(0)),
            |candidate| {
                let text = fs::read_to_string(candidate).unwrap();
                assert!(text.contains("timeout_secs = 0"), "{text}");
                Err(Diagnostic::builder(CONFIG_003, Severity::Error)
                    .why("0 is out of range")
                    .build())
            },
        )
        .unwrap_err();

        assert_eq!(refused.code, CONFIG_003);
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "[hold]\ntimeout_secs = 42\n"
        );
        let names: Vec<String> = fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["config.toml".to_owned()], "{names:?}");
    }
}
