//! Der HMAC-Schlüssel der Kette.
//!
//! **Heute eine Datei, morgen der Keyring.** Die Spezifikation nimmt den
//! Schlüssel aus `KeyStore.derive(AuditHmac)`; den `KeyStore` baut HUM-048,
//! und bis dahin liegt er als 32 Zufallsbytes in
//! `Paths::audit_key_path()` (`$XDG_DATA_HOME/humanitl/keys/audit.key`). Das
//! Log sagt es selbst: `daemon.started` trägt `key_origin: "file"`.
//!
//! Die Naht ist [`AuditKey`]: Wer schreibt oder prüft, bekommt einen
//! `AuditKey` und fragt nicht, woher er kommt. Die Datei liest und legt
//! `humanitl-audit-log` an (`AuditKeyFile::load_or_create_file`, HUM-155),
//! denn diese Crate öffnet keine Datei. HUM-048 ersetzt diesen Weg durch einen
//! über den Keyring, der [`AuditKey::from_bytes`] mit [`KeyOrigin::Keyring`]
//! ruft; Schreiber und Prüfung ändern sich dabei nicht. Wer dann eine Kette
//! hat, die mit dem Dateischlüssel geschrieben ist, prüft sie weiter mit
//! dieser Datei.

use std::fmt;
use std::path::{Path, PathBuf};

use zeroize::Zeroizing;

use crate::kinds::KeyOrigin;

/// Länge des Schlüssels in Bytes.
pub const KEY_LEN: usize = 32;
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

    /// Ein Schlüssel, den `humanitl-audit-log` aus der Datei `path` gelesen
    /// hat; `created` sagt, ob derselbe Aufruf ihn angelegt hat.
    #[must_use]
    pub fn from_file(bytes: [u8; KEY_LEN], path: PathBuf, created: bool) -> Self {
        Self {
            bytes: Zeroizing::new(bytes),
            origin: KeyOrigin::File,
            path: Some(path),
            created,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AuditKey, KEY_LEN};
    use crate::kinds::KeyOrigin;

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
}
