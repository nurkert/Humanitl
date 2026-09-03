//! Das Sitzungs-Token: erzeugen, ablegen, prüfen.
//!
//! Der Socket des Daemons liegt in einem Verzeichnis, das nur der Nutzer
//! öffnen darf, und trägt selbst `0600`. Das Token ist die zweite Schranke: es
//! entsteht bei jedem Start neu, steht in
//! `$XDG_RUNTIME_DIR/humanitl/token` (`0600`) und muss in jedem Aufruf im
//! Metadaten-Kopf `x-humanitl-token` stehen — auch in `GetInfo`. Es gibt
//! keinen unauthentifizierten Endpunkt: schon die Auskunft, welcher Daemon
//! hier mit welchen Fähigkeiten läuft, gehört dem Nutzer.
//!
//! Verglichen wird in einer Zeit, die nicht vom Inhalt abhängt
//! ([`constant_time_eq`]). Das Token ist lokal und kurzlebig, aber es gibt
//! keinen Grund, an dieser Stelle einen Zeitkanal offenzulassen.
//!
//! Die Datei entsteht mit `0600` von der ersten Zeile an: erst wird ein
//! etwaiger Vorgänger entfernt, dann wird mit `create_new` und dem Modus
//! angelegt. Ein „schreiben und danach `chmod`" ließe einen Augenblick, in dem
//! der Schlüssel für andere lesbar wäre, und ein untergeschobener Symlink
//! zeigte auf ein fremdes Ziel.

use std::fs::{self, OpenOptions};
use std::io::{Read as _, Write as _};
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::Path;

use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, FixAction, Severity};
use tonic::metadata::MetadataMap;
use tonic::service::Interceptor;
use tonic::{Request, Status};

use crate::TOKEN_METADATA_KEY;
use crate::server_stub::diagnostic_to_status;

/// So viele Zufallsbytes trägt ein Token, hex also doppelt so viele Zeichen.
pub const TOKEN_BYTES: usize = 32;

/// Die Rechte von Token-Datei und Socket: nur der Nutzer selbst.
pub const TOKEN_MODE: u32 = 0o600;

/// Ein frisches Token: [`TOKEN_BYTES`] Bytes aus `/dev/urandom`, hex.
///
/// Keine eigene Kryptographie und keine Zufallsbibliothek: der Kernel liefert
/// den Zufall, und hex hält das Ergebnis frei von Zeichen, die ein
/// Metadaten-Kopf nicht trägt.
///
/// # Errors
///
/// [`Diagnostic`] mit `DAEMON_004`, wenn `/dev/urandom` nicht lesbar ist.
pub fn new_token() -> Result<String, Diagnostic> {
    let source = Path::new("/dev/urandom");
    let mut bytes = [0u8; TOKEN_BYTES];
    fs::File::open(source)
        .and_then(|mut file| file.read_exact(&mut bytes))
        .map_err(|error| io_diagnostic("read", source, &error))?;
    Ok(hex(&bytes))
}

/// Kodiert Bytes als Kleinbuchstaben-Hex.
///
/// Der Kern hat dieselbe Funktion, aber nur für sich selbst
/// (`humanitl_core::hex` ist `pub(crate)`); vier Zeilen sind billiger als eine
/// weitere Fremd-Crate.
fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from(DIGITS[usize::from(byte >> 4)]));
        out.push(char::from(DIGITS[usize::from(byte & 0x0f)]));
    }
    out
}

/// Schreibt die Token-Datei mit `0600` und ersetzt eine vorhandene.
///
/// # Errors
///
/// [`Diagnostic`] mit `DAEMON_004`, wenn die Datei nicht angelegt oder nicht
/// beschrieben werden kann.
pub fn write_token(path: &Path, token: &str) -> Result<(), Diagnostic> {
    // Ein Vorgänger kann eine fremde Datei oder ein Symlink sein; er wird
    // entfernt, nicht überschrieben. `create_new` scheitert danach auf jeden
    // Rest, statt ihm zu folgen.
    match fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(io_diagnostic("replace the token file", path, &error)),
    }
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(TOKEN_MODE)
        .open(path)
        .and_then(|mut file| file.write_all(token.as_bytes()))
        .map_err(|error| io_diagnostic("write the token file", path, &error))
}

/// Liest die Token-Datei, für Clients (CLI, Oberfläche, Tests).
///
/// # Errors
///
/// [`Diagnostic`] mit `DAEMON_001`, wenn die Datei fehlt oder leer ist: dann
/// läuft kein Daemon, dem dieses Token gehört.
pub fn read_token(path: &Path) -> Result<String, Diagnostic> {
    // Erst der Blick auf den Eintrag selbst, ohne einem Symlink zu folgen:
    // Ein untergeschobener Link an dieser Stelle zeigte sonst auf eine fremde
    // Datei, deren Inhalt als Token gelesen wuerde.
    let meta = fs::symlink_metadata(path).map_err(|error| {
        Diagnostic::builder(codes::DAEMON_001, Severity::Blocking)
            .why(format!(
                "cannot stat the session token {}: {error}",
                path.display()
            ))
            .fix(FixAction::CopyCommand("humanitld".to_owned()))
            .build()
    })?;
    if !meta.is_file() {
        return Err(Diagnostic::builder(codes::DAEMON_001, Severity::Blocking)
            .why(format!(
                "the session token {} is not a regular file (a symlink here is refused, not followed)",
                path.display()
            ))
            .fix(FixAction::CopyCommand("humanitld".to_owned()))
            .build());
    }
    let token = fs::read_to_string(path).map_err(|error| {
        Diagnostic::builder(codes::DAEMON_001, Severity::Blocking)
            .why(format!(
                "cannot read the session token {}: {error}",
                path.display()
            ))
            .fix(FixAction::CopyCommand("humanitld".to_owned()))
            .build()
    })?;
    let token = token.trim().to_owned();
    if token.is_empty() {
        return Err(Diagnostic::builder(codes::DAEMON_001, Severity::Blocking)
            .why(format!("the session token {} is empty", path.display()))
            .fix(FixAction::CopyCommand("humanitld".to_owned()))
            .build());
    }
    Ok(token)
}

/// Prüft den Metadaten-Kopf einer Anfrage gegen das erwartete Token.
///
/// # Errors
///
/// [`Status`] mit `Code::Unauthenticated` und `IPC_001` in den Details, wenn
/// der Kopf fehlt oder nicht passt. Beide Fälle liefern denselben Code; nur
/// der Grund im Befund unterscheidet sie, und der geht an den lokalen Nutzer,
/// nicht an einen fremden Aufrufer.
pub fn check_token(metadata: &MetadataMap, expected: &str) -> Result<(), Status> {
    let presented = metadata
        .get(TOKEN_METADATA_KEY)
        .map(tonic::metadata::MetadataValue::as_encoded_bytes);

    if presented.is_some_and(|value| constant_time_eq(value, expected.as_bytes())) {
        return Ok(());
    }

    let why = if presented.is_none() {
        format!("metadata key {TOKEN_METADATA_KEY} is missing")
    } else {
        format!("metadata key {TOKEN_METADATA_KEY} does not match the session token")
    };
    Err(diagnostic_to_status(
        &Diagnostic::builder(codes::IPC_001, Severity::Error)
            .why(why)
            .build(),
    ))
}

/// Der Interceptor, der jeden Aufruf des echten Dienstes prüft.
///
/// tonic ruft ihn vor der Methode, für jede RPC des Vertrags. Damit kann kein
/// Endpunkt vergessen werden — auch `GetInfo` nicht, das sonst der einzige
/// offene Weg in den Daemon wäre.
#[derive(Debug, Clone)]
pub struct TokenAuth {
    token: String,
}

impl TokenAuth {
    /// Der Interceptor zu einem Token.
    #[must_use]
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            token: token.into(),
        }
    }
}

impl Interceptor for TokenAuth {
    fn call(&mut self, request: Request<()>) -> Result<Request<()>, Status> {
        check_token(request.metadata(), &self.token)?;
        Ok(request)
    }
}

/// Vergleicht zwei Byte-Folgen in Zeit, die nicht vom Inhalt abhängt.
#[must_use]
pub fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut diff = 0u8;
    for (a, b) in left.iter().zip(right.iter()) {
        diff |= a ^ b;
    }
    diff == 0
}

/// Die Rechte einer Datei, als Oktalzahl ohne Typbits.
///
/// Gedacht für Tests und für den Selbsttest (`humanitl doctor`, HUM-075).
///
/// # Errors
///
/// [`Diagnostic`] mit `DAEMON_004`, wenn die Datei nicht lesbar ist.
pub fn file_mode(path: &Path) -> Result<u32, Diagnostic> {
    // Der Eintrag selbst, nicht sein Ziel: Rechte eines Symlinks sagen nichts
    // ueber die Datei, auf die er zeigt, und die Pruefung soll den Link sehen.
    let metadata =
        fs::symlink_metadata(path).map_err(|error| io_diagnostic("stat", path, &error))?;
    Ok(metadata.permissions().mode() & 0o777)
}

/// Ein Fehler des Dateisystems als Befund (`DAEMON_004`).
fn io_diagnostic(what: &str, path: &Path, error: &std::io::Error) -> Diagnostic {
    Diagnostic::builder(codes::DAEMON_004, Severity::Blocking)
        .why(format!("cannot {what} {}: {error}", path.display()))
        .build()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use tonic::metadata::{MetadataMap, MetadataValue};
    use tonic::{Code, Request};

    use super::{
        TOKEN_BYTES, TokenAuth, check_token, constant_time_eq, file_mode, new_token, read_token,
        write_token,
    };
    use crate::TOKEN_METADATA_KEY;

    fn metadata(token: &str) -> MetadataMap {
        let mut map = MetadataMap::new();
        map.insert(
            TOKEN_METADATA_KEY,
            MetadataValue::try_from(token).expect("a hex token is valid ascii"),
        );
        map
    }

    #[test]
    fn constant_time_eq_compares_content_and_length() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"ab"));
        assert!(constant_time_eq(b"", b""));
    }

    #[test]
    fn a_token_is_hex_and_never_the_same_twice() {
        let first = new_token().unwrap();
        let second = new_token().unwrap();
        assert_eq!(first.len(), TOKEN_BYTES * 2);
        assert!(first.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(first, second, "every start gets its own token");
    }

    #[test]
    fn the_token_file_is_0600_and_is_rewritten_on_every_start() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("token");

        let first = new_token().unwrap();
        write_token(&path, &first).unwrap();
        assert_eq!(file_mode(&path).unwrap(), 0o600);
        assert_eq!(read_token(&path).unwrap(), first);

        let second = new_token().unwrap();
        write_token(&path, &second).unwrap();
        assert_eq!(file_mode(&path).unwrap(), 0o600);
        assert_eq!(read_token(&path).unwrap(), second);
        assert_ne!(first, second);
    }

    #[test]
    fn a_world_readable_predecessor_is_replaced_not_reused() {
        use std::fs;
        use std::os::unix::fs::PermissionsExt as _;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("token");
        fs::write(&path, b"old").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();

        write_token(&path, "new").unwrap();

        assert_eq!(file_mode(&path).unwrap(), 0o600);
        assert_eq!(read_token(&path).unwrap(), "new");
    }

    #[test]
    fn a_missing_or_empty_token_file_is_daemon_001() {
        let dir = tempfile::tempdir().unwrap();
        let missing = read_token(&dir.path().join("token")).unwrap_err();
        assert_eq!(missing.code.as_str(), "DAEMON_001");

        let empty = dir.path().join("empty");
        write_token(&empty, "   ").unwrap();
        let error = read_token(&empty).unwrap_err();
        assert_eq!(error.code.as_str(), "DAEMON_001");
        assert!(error.why.contains("empty"), "{}", error.why);
    }

    /// Ein Symlink an der Stelle des Tokens wird abgelehnt, nicht verfolgt:
    /// sonst laese der Client eine fremde Datei als Token.
    #[test]
    fn a_symlinked_token_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("elsewhere");
        write_token(&real, "cafe").unwrap();
        let link = dir.path().join("token");
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let error = read_token(&link).unwrap_err();
        assert_eq!(error.code.as_str(), "DAEMON_001");
        assert!(error.why.contains("not a regular file"), "{}", error.why);
    }

    #[test]
    fn the_check_refuses_a_missing_and_a_wrong_token() {
        let expected = "cafe";
        assert!(check_token(&metadata(expected), expected).is_ok());

        let missing = check_token(&MetadataMap::new(), expected).unwrap_err();
        assert_eq!(missing.code(), Code::Unauthenticated);
        assert!(missing.message().contains("IPC_001"), "{missing}");

        let wrong = check_token(&metadata("beef"), expected).unwrap_err();
        assert_eq!(wrong.code(), Code::Unauthenticated);
    }

    #[test]
    fn the_interceptor_guards_every_call() {
        use tonic::service::Interceptor as _;

        let mut auth = TokenAuth::new("cafe");
        let mut good = Request::new(());
        *good.metadata_mut() = metadata("cafe");
        assert!(auth.call(good).is_ok());

        let error = auth.call(Request::new(())).unwrap_err();
        assert_eq!(error.code(), Code::Unauthenticated);
    }
}
