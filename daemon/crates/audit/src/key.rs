//! Der HMAC-Schlüssel der Kette.
//!
//! **Heute eine Datei, morgen der Keyring.** Die Spezifikation nimmt den
//! Schlüssel aus `KeyStore.derive(AuditHmac)`; den `KeyStore` baut HUM-048,
//! und bis dahin liegt er als 32 Zufallsbytes in
//! `Paths::audit_key_path()` (`$XDG_DATA_HOME/humanitl/keys/audit.key`). Das
//! Log sagt es selbst: `daemon.started` trägt `key_origin: "file"`.
//!
//! Die Naht ist [`AuditKey`]: Wer schreibt oder prüft, bekommt einen
//! `AuditKey` und fragt nicht, woher er kommt. HUM-048 ersetzt
//! [`AuditKey::load_or_create_file`] durch einen Weg über den Keyring, der
//! [`AuditKey::from_bytes`] mit [`KeyOrigin::Keyring`] ruft; Schreiber und
//! Prüfung ändern sich dabei nicht. Wer dann eine Kette hat, die mit dem
//! Dateischlüssel geschrieben ist, prüft sie weiter mit dieser Datei.
//!
//! Die Datei wird behandelt wie `ca.key` in `humanitl-proxy` (HUM-014), mit
//! denselben Prüfungen in derselben Strenge:
//!
//! 1. Das Verzeichnis ist `0700`; ein vorhandenes, das Gruppe oder Andere
//!    öffnen dürfen, wird auf `0700` gezogen.
//! 2. Angelegt wird mit `create_new` und `O_NOFOLLOW`, zuerst unter einem
//!    nicht vorhersagbaren Namen, dann per `link` an den endgültigen Pfad.
//!    `link` scheitert, wenn dort schon etwas liegt; zwei Starts zugleich
//!    überschreiben sich deshalb nie, und ein halb geschriebener Schlüssel
//!    liegt nie unter dem echten Namen.
//! 3. Beim Laden: `symlink_metadata` statt `metadata` (ein Symlink wird
//!    abgelehnt, nicht verfolgt), eine reguläre Datei, keine Rechte für Gruppe
//!    oder Andere, genau 32 Bytes. Dazu, über `ca.rs` hinaus, der Eigentümer:
//!    Ein Schlüssel, der einem anderen Nutzer gehört, kann von ihm stammen.
//! 4. Fail-closed: Ein Schlüssel, den andere lesen konnten, ist verbrannt. Er
//!    wird nicht still ersetzt und nicht still weiterbenutzt; der Start endet
//!    mit [`AUDIT_005`].

use std::fmt;
use std::fs::{self, DirBuilder, OpenOptions};
use std::io::{self, Read as _, Write as _};
use std::os::unix::fs::{
    DirBuilderExt as _, MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use humanitl_core::diagnostics::codes::AUDIT_005;
use humanitl_core::{Diagnostic, FixAction, Severity};
use zeroize::Zeroizing;

use crate::kinds::KeyOrigin;

/// Länge des Schlüssels in Bytes.
pub const KEY_LEN: usize = 32;
/// Rechte des Schlüsselverzeichnisses.
pub const KEY_DIR_MODE: u32 = 0o700;
/// Rechte der Schlüsseldatei: der Nutzer, sonst niemand.
pub const KEY_MODE: u32 = 0o600;

/// Zähler für die temporären Namen, damit zwei Threads desselben Prozesses nie
/// denselben Pfad wählen.
static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Der Schlüssel, mit dem jeder Record seinen MAC bekommt.
pub struct AuditKey {
    bytes: Zeroizing<[u8; KEY_LEN]>,
    origin: KeyOrigin,
    path: Option<PathBuf>,
    created: bool,
}

impl fmt::Debug for AuditKey {
    /// Zeigt Herkunft und Pfad; den Schlüssel nie.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuditKey")
            .field("origin", &self.origin)
            .field("path", &self.path)
            .field("created", &self.created)
            .field("bytes", &"[elided]")
            .finish()
    }
}

impl AuditKey {
    /// Ein Schlüssel aus Bytes, die jemand anderes beschafft hat.
    ///
    /// Der Weg für HUM-048 (Keyring) und für Tests mit festem Schlüssel.
    #[must_use]
    pub fn from_bytes(bytes: [u8; KEY_LEN], origin: KeyOrigin) -> Self {
        Self {
            bytes: Zeroizing::new(bytes),
            origin,
            path: None,
            created: false,
        }
    }

    /// Die 32 Bytes. Nie anzeigen, nie protokollieren.
    #[must_use]
    pub fn bytes(&self) -> &[u8; KEY_LEN] {
        &self.bytes
    }

    /// Woher der Schlüssel stammt.
    #[must_use]
    pub const fn origin(&self) -> KeyOrigin {
        self.origin
    }

    /// Die Datei, aus der er stammt, falls es eine gibt.
    #[must_use]
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Wahr, wenn dieser Aufruf den Schlüssel neu angelegt hat.
    #[must_use]
    pub const fn was_created(&self) -> bool {
        self.created
    }

    /// Lädt den Dateischlüssel oder legt ihn an, wenn es `path` nicht gibt.
    ///
    /// # Errors
    ///
    /// [`AUDIT_005`], wenn das Verzeichnis nicht anlegbar ist, die Datei ein
    /// Symlink oder keine reguläre Datei ist, Gruppe oder Andere Rechte haben,
    /// sie einem anderen Nutzer gehört, sie nicht genau 32 Bytes hat, oder der
    /// Zufall des Kerns nicht zu haben ist.
    pub fn load_or_create_file(path: &Path) -> Result<Self, Diagnostic> {
        let dir = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        ensure_dir(dir)?;
        match fs::symlink_metadata(path) {
            Ok(_) => Self::load(path, false),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Self::create(dir, path),
            Err(err) => Err(unusable(path, &format!("cannot inspect it: {err}"))),
        }
    }

    fn create(dir: &Path, path: &Path) -> Result<Self, Diagnostic> {
        let mut bytes = Zeroizing::new([0_u8; KEY_LEN]);
        getrandom::fill(bytes.as_mut_slice()).map_err(|err| {
            unusable(
                path,
                &format!("the kernel gave no random bytes for a new key: {err}"),
            )
        })?;
        let name = path
            .file_name()
            .map_or_else(|| "audit.key".into(), |name| name.to_string_lossy());
        let tmp = dir.join(format!(
            ".{name}.tmp-{}-{}",
            std::process::id(),
            TMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let written = (|| -> io::Result<()> {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .custom_flags(libc::O_NOFOLLOW)
                .mode(KEY_MODE)
                .open(&tmp)?;
            // Nach dem Anlegen noch einmal, damit die `umask` nichts ändert.
            file.set_permissions(fs::Permissions::from_mode(KEY_MODE))?;
            file.write_all(bytes.as_slice())?;
            file.sync_all()
        })();
        if let Err(err) = written {
            // Best effort: der Rest bleibt nicht liegen; der Befund ist der
            // Fehler beim Schreiben.
            let _ = fs::remove_file(&tmp);
            return Err(unusable(path, &format!("cannot write a new key: {err}")));
        }
        let linked = fs::hard_link(&tmp, path);
        let _ = fs::remove_file(&tmp);
        match linked {
            Ok(()) => {}
            // Ein zweiter Start war schneller. Sein Schlüssel gilt, und er
            // wird genauso geprüft wie jeder vorhandene.
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                return Self::load(path, false);
            }
            Err(err) => return Err(unusable(path, &format!("cannot place the new key: {err}"))),
        }
        // Zurücklesen: geladen wird, was auf der Platte liegt, nicht, was wir
        // zu schreiben glaubten.
        Self::load(path, true)
    }

    fn load(path: &Path, created: bool) -> Result<Self, Diagnostic> {
        check_key_file(path)?;
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(path)
            .map_err(|err| unusable(path, &format!("cannot open it: {err}")))?;
        // Ein Byte mehr als nötig lesen: Nur so fällt eine zu lange Datei auf.
        let mut buffer = Zeroizing::new(Vec::with_capacity(KEY_LEN + 1));
        (&mut file)
            .take(u64::try_from(KEY_LEN).unwrap_or(u64::MAX) + 1)
            .read_to_end(&mut buffer)
            .map_err(|err| unusable(path, &format!("cannot read it: {err}")))?;
        let bytes: [u8; KEY_LEN] = buffer.as_slice().try_into().map_err(|_| {
            refused(
                path,
                &format!(
                    "{} holds {} bytes; an audit key is exactly {KEY_LEN}",
                    path.display(),
                    if buffer.len() > KEY_LEN {
                        format!("more than {KEY_LEN}")
                    } else {
                        buffer.len().to_string()
                    }
                ),
                burnt_fix(path),
            )
        })?;
        Ok(Self {
            bytes: Zeroizing::new(bytes),
            origin: KeyOrigin::File,
            path: Some(path.to_owned()),
            created,
        })
    }
}

/// Der Schlüssel muss eine reguläre Datei sein, die nur der Nutzer lesen darf
/// und die ihm gehört.
fn check_key_file(path: &Path) -> Result<(), Diagnostic> {
    // `symlink_metadata`, nicht `metadata`: Ein Symlink an dieser Stelle
    // zeigte sonst auf eine fremde Datei, deren Rechte hier geprüft würden,
    // während gelesen würde, worauf er zeigt.
    let meta = fs::symlink_metadata(path)
        .map_err(|err| unusable(path, &format!("cannot inspect it: {err}")))?;
    if !meta.is_file() {
        return Err(refused(
            path,
            &format!(
                "{} is not a regular file (a symlink here is refused, not followed)",
                path.display()
            ),
            burnt_fix(path),
        ));
    }
    let owner = rustix::process::geteuid().as_raw();
    if meta.uid() != owner {
        return Err(refused(
            path,
            &format!(
                "{} belongs to uid {}, not to you (uid {owner}); a key someone else owns may \
                 be theirs",
                path.display(),
                meta.uid()
            ),
            FixAction::CopyCommand(format!(
                "ls -ln {}",
                shell_quote(&path.display().to_string())
            )),
        ));
    }
    let mode = meta.permissions().mode() & 0o777;
    if mode & 0o077 != 0 {
        return Err(refused(
            path,
            &format!(
                "{} has mode {mode:04o}; only the owner may read the audit key (0600), and a key \
                 others could read must be treated as burnt: whoever read it can forge the MAC \
                 of every record",
                path.display()
            ),
            burnt_fix(path),
        ));
    }
    Ok(())
}

/// Legt `dir` mit `0700` an (Eltern mit Standardrechten) und zieht die Rechte
/// eines vorhandenen Verzeichnisses auf `0700`, falls Gruppe oder Andere
/// etwas dürfen.
fn ensure_dir(dir: &Path) -> Result<(), Diagnostic> {
    let result = (|| -> io::Result<()> {
        if let Some(parent) = dir.parent() {
            fs::create_dir_all(parent)?;
        }
        match DirBuilder::new().mode(KEY_DIR_MODE).create(dir) {
            Ok(()) => {}
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {}
            Err(err) => return Err(err),
        }
        let meta = fs::metadata(dir)?;
        if !meta.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotADirectory,
                "exists but is not a directory",
            ));
        }
        if meta.permissions().mode() & 0o077 != 0 {
            fs::set_permissions(dir, fs::Permissions::from_mode(KEY_DIR_MODE))?;
        }
        Ok(())
    })();
    result.map_err(|err| {
        let quoted = shell_quote(&dir.display().to_string());
        Diagnostic::builder(AUDIT_005, Severity::Error)
            .why(format!(
                "the key directory {} is not usable: {err}",
                dir.display()
            ))
            .fix(FixAction::CopyCommand(format!(
                "mkdir -p {quoted} && chmod 700 {quoted}"
            )))
            .build()
    })
}

/// Der Vorschlag für einen Schlüssel, dem nicht mehr zu trauen ist.
///
/// Löschen und nicht `chmod`: Wer ihn lesen konnte, hat ihn womöglich schon.
/// Der nächste Start legt einen neuen an und weigert sich dann, an die Kette
/// des alten anzuhängen (`AUDIT_001`); die alte Kette bleibt als Beleg liegen
/// und lässt sich ohne Schlüssel weiter auf Reihenfolge und Hashes prüfen.
fn burnt_fix(path: &Path) -> FixAction {
    FixAction::CopyCommand(format!("rm {}", shell_quote(&path.display().to_string())))
}

fn refused(path: &Path, why: &str, fix: FixAction) -> Diagnostic {
    Diagnostic::builder(AUDIT_005, Severity::Blocking)
        .why(format!(
            "{why}; the audit log is not written with {}",
            path.display()
        ))
        .fix(fix)
        .build()
}

fn unusable(path: &Path, why: &str) -> Diagnostic {
    Diagnostic::builder(AUDIT_005, Severity::Error)
        .why(format!("the audit key {}: {why}", path.display()))
        .fix(FixAction::CopyCommand(format!(
            "ls -ln {}",
            shell_quote(&path.display().to_string())
        )))
        .build()
}

/// Setzt einen Pfad in einfache Anführungszeichen, wenn `sh` ihn sonst nicht
/// als ein Wort läse.
pub(crate) fn shell_quote(arg: &str) -> String {
    let safe = |c: char| {
        c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-' | '+' | ':' | '@' | '~')
    };
    if !arg.is_empty() && arg.chars().all(safe) {
        return arg.to_owned();
    }
    let mut out = String::with_capacity(arg.len() + 2);
    out.push('\'');
    for c in arg.chars() {
        if c == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::fs;
    use std::os::unix::fs::PermissionsExt as _;

    use super::{AuditKey, KEY_LEN, shell_quote};
    use crate::kinds::KeyOrigin;

    fn mode_of(path: &std::path::Path) -> u32 {
        fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn a_new_key_is_32_bytes_0600_in_a_0700_directory_and_stays_the_same() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("keys").join("audit.key");

        let first = AuditKey::load_or_create_file(&path).unwrap();
        assert!(first.was_created());
        assert_eq!(first.origin(), KeyOrigin::File);
        assert_eq!(fs::read(&path).unwrap().len(), KEY_LEN);
        assert_eq!(mode_of(&path), 0o600);
        assert_eq!(mode_of(path.parent().unwrap()), 0o700);
        assert_ne!(first.bytes(), &[0; KEY_LEN], "random, not zeroes");

        let second = AuditKey::load_or_create_file(&path).unwrap();
        assert!(!second.was_created());
        assert_eq!(first.bytes(), second.bytes());
        assert!(
            fs::read_dir(path.parent().unwrap()).unwrap().count() == 1,
            "no temporary file is left behind"
        );
    }

    #[test]
    fn an_open_key_directory_is_tightened() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("keys");
        fs::create_dir(&dir).unwrap();
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
        AuditKey::load_or_create_file(&dir.join("audit.key")).unwrap();
        assert_eq!(mode_of(&dir), 0o700);
    }

    #[test]
    fn a_key_others_can_read_is_refused_not_replaced() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("audit.key");
        AuditKey::load_or_create_file(&path).unwrap();
        let before = fs::read(&path).unwrap();
        for mode in [0o640, 0o604, 0o660] {
            fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
            let error = AuditKey::load_or_create_file(&path).unwrap_err();
            assert_eq!(error.code.as_str(), "AUDIT_005");
            assert!(error.why.contains(&format!("{mode:04o}")), "{}", error.why);
            assert!(error.why.contains("burnt"), "{}", error.why);
        }
        assert_eq!(
            fs::read(&path).unwrap(),
            before,
            "a refused key is not rewritten"
        );
    }

    #[test]
    fn a_key_of_the_wrong_length_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        for len in [0, 31, 33, 64] {
            let path = tmp.path().join(format!("k{len}"));
            fs::write(&path, vec![1_u8; len]).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
            let error = AuditKey::load_or_create_file(&path).unwrap_err();
            assert_eq!(error.code.as_str(), "AUDIT_005", "{len} bytes");
            assert!(error.why.contains("exactly 32"), "{}", error.why);
        }
    }

    #[test]
    fn a_symlink_in_place_of_the_key_is_refused_not_followed() {
        let tmp = tempfile::tempdir().unwrap();
        let real = tmp.path().join("real.key");
        fs::write(&real, [9_u8; KEY_LEN]).unwrap();
        fs::set_permissions(&real, fs::Permissions::from_mode(0o600)).unwrap();
        let link = tmp.path().join("audit.key");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let error = AuditKey::load_or_create_file(&link).unwrap_err();
        assert_eq!(error.code.as_str(), "AUDIT_005");
        assert!(error.why.contains("symlink"), "{}", error.why);
    }

    #[test]
    fn debug_never_shows_the_key() {
        let key = AuditKey::from_bytes([0xab; KEY_LEN], KeyOrigin::File);
        let shown = format!("{key:?}");
        assert!(
            !shown.contains("171") && !shown.contains("ab, ab"),
            "{shown}"
        );
        assert!(shown.contains("elided"));
    }

    #[test]
    fn quoting_leaves_plain_paths_alone() {
        assert_eq!(shell_quote("/a/b.key"), "/a/b.key");
        assert_eq!(shell_quote("/a b/it's"), "'/a b/it'\\''s'");
    }
}
