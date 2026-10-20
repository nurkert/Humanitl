//! Wem der Daemon auf dem Weg zum Ziel vertraut (HUM-257).
//!
//! Die Vertrauensanker für Verbindungen zum Ziel kommen per Vorgabe aus der
//! Zertifikatsablage des Systems (`/etc/ssl/certs` auf Debian), gelesen mit
//! `rustls-native-certs`. Damit gilt für den Daemon dasselbe wie für `curl`
//! und den Browser auf demselben Rechner: Eine Firmen-CA oder eine CA, die
//! der Mensch selbst mit `update-ca-certificates` installiert hat, wird für
//! den Weg zum Ziel angenommen. Der Schlüssel `tls.trust_system_store`
//! schaltet das ab; dann gelten nur die von Mozilla gepflegten Wurzeln aus
//! `webpki-roots`, wie vor HUM-257.
//!
//! Diese Anker gelten nur nach oben. Die CA, die der Proxy für die Sandbox
//! ausstellt ([`crate::ca`]), ist davon unberührt: Ihr vertrauen nur die
//! Werkzeuge in der Sandbox, und welche Wurzeln der Daemon zum Ziel prüft,
//! ändert nichts daran, wem der Agent glaubt.
//!
//! # Rückfall
//!
//! Liefert die Ablage keine einzige Wurzel, die rustls annimmt (leer,
//! unlesbar, auf einem Rechner ohne `ca-certificates`), fällt der Daemon auf
//! `webpki-roots` zurück, statt jede TLS-Verbindung scheitern zu lassen. Ein
//! einzelnes Zertifikat, das rustls ablehnt, kostet eine Warnung im Log und
//! nie den Start; die übrigen Wurzeln gelten weiter.
//!
//! # Einmal je Prozess
//!
//! [`TrustAnchors::configured`] liest die Ablage beim ersten Aufruf und hält
//! das Ergebnis für die Lebenszeit des Prozesses fest. Der Daemon baut beim
//! Start mehrere Verbindungsstapel (Proxy, Endpunkt-Probe); alle teilen sich
//! dieselbe Lesung, und die Zeile mit der Zahl der Wurzeln steht genau
//! einmal im Log. Eine Änderung an der Ablage wirkt nach dem Neustart des
//! Daemons, wie jede Änderung an der Konfiguration.

use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use humanitl_core::FixAction;
use rustls::RootCertStore;
use rustls_native_certs::CertificateResult;

/// Woher die Vertrauensanker zum Ziel stammen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustSource {
    /// Aus der Zertifikatsablage des Systems.
    SystemStore,
    /// Aus `webpki-roots`, weil die Ablage keine brauchbare Wurzel lieferte.
    BundledFallback,
    /// Aus `webpki-roots`, weil `tls.trust_system_store` aus ist.
    BundledByConfig,
}

impl TrustSource {
    /// Kurzname für das Log.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SystemStore => "system_store",
            Self::BundledFallback => "webpki_roots_fallback",
            Self::BundledByConfig => "webpki_roots",
        }
    }
}

/// Die Vertrauensanker für Verbindungen zum Ziel, samt Herkunft.
///
/// Gebaut einmal je Prozess über [`TrustAnchors::configured`]; Tests bauen
/// sie gezielt mit [`TrustAnchors::bundled`] oder [`TrustAnchors::from_paths`].
#[derive(Debug, Clone)]
pub struct TrustAnchors {
    roots: RootCertStore,
    source: TrustSource,
    rejected: usize,
    /// `SSL_CERT_FILE=…` und `SSL_CERT_DIR=…`, wenn die Umgebung die Ablage
    /// umgelenkt hat; leer sonst.
    redirected: Vec<String>,
}

impl TrustAnchors {
    /// Die Anker, die die Konfiguration verlangt, aus der einen Lesung des
    /// Prozesses.
    ///
    /// `trust_system_store` ist `tls.trust_system_store`. Beim ersten Aufruf
    /// je Wert wird gelesen und einmal ins Log geschrieben, wie viele Wurzeln
    /// woher kamen; jeder weitere Aufruf gibt eine Kopie derselben Anker.
    #[must_use]
    pub fn configured(trust_system_store: bool) -> Self {
        static SYSTEM: OnceLock<TrustAnchors> = OnceLock::new();
        static BUNDLED: OnceLock<TrustAnchors> = OnceLock::new();
        let cell = if trust_system_store {
            &SYSTEM
        } else {
            &BUNDLED
        };
        cell.get_or_init(|| {
            let anchors = if trust_system_store {
                Self::system_store()
            } else {
                Self::bundled()
            };
            anchors.log();
            anchors
        })
        .clone()
    }

    /// Liest die Zertifikatsablage des Systems, jedes Mal neu.
    ///
    /// `rustls-native-certs` sucht die Ablage wie OpenSSL; `SSL_CERT_FILE`
    /// und `SSL_CERT_DIR` lenken die Suche auf eine andere Datei oder ein
    /// anderes Verzeichnis. Ohne eine Wurzel, die rustls annimmt, sind es die
    /// Wurzeln aus `webpki-roots` ([`TrustSource::BundledFallback`]).
    #[must_use]
    pub fn system_store() -> Self {
        let redirect = env_redirect(
            std::env::var_os(ENV_CERT_FILE),
            std::env::var_os(ENV_CERT_DIR),
        );
        let home_uid = std::env::var_os("HOME")
            .and_then(|home| std::fs::metadata(home).ok())
            .map(|meta| meta.uid());
        for (name, path) in &redirect {
            if writable_by_this_account(path, home_uid) {
                // Kein Abbruch: Das eigene Konto darf seinen Daemon so
                // einrichten (`docs/SECURITY.md` 11). Aber ein Pfad, den das
                // Konto schreiben kann, ist womöglich der Projektordner, und
                // dann legt der Agent beim nächsten Start eine Wurzel hinein.
                tracing::warn!(
                    variable = name,
                    path = %path.display(),
                    "{name} points to a path this account can write; any process of this \
                     account, the agent included if it is a project folder, could plant a \
                     trusted root there. Point it at a root-owned path such as /etc/ssl/certs"
                );
            }
        }
        let mut anchors = Self::from_result(rustls_native_certs::load_native_certs());
        anchors.redirected = redirect
            .iter()
            .map(|(name, path)| format!("{name}={}", path.display()))
            .collect();
        anchors
    }

    /// Liest eine bestimmte Datei und ein bestimmtes Verzeichnis als
    /// Zertifikatsablage, mit demselben Rückfall wie [`Self::system_store`].
    #[must_use]
    pub fn from_paths(file: Option<&Path>, dir: Option<&Path>) -> Self {
        Self::from_result(rustls_native_certs::load_certs_from_paths(file, dir))
    }

    /// Nur die Wurzeln aus `webpki-roots`, wie bei
    /// `tls.trust_system_store = false`.
    #[must_use]
    pub fn bundled() -> Self {
        Self {
            roots: bundled_roots(),
            source: TrustSource::BundledByConfig,
            rejected: 0,
            redirected: Vec::new(),
        }
    }

    /// Wie viele Wurzeln gelten.
    #[must_use]
    pub fn len(&self) -> usize {
        self.roots.len()
    }

    /// Ob keine Wurzel gilt. Kommt nicht vor: Der Rückfall füllt die Liste.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.roots.is_empty()
    }

    /// Woher die Wurzeln stammen.
    #[must_use]
    pub const fn source(&self) -> TrustSource {
        self.source
    }

    /// Wie viele Einträge der Ablage nicht übernommen wurden: unlesbare
    /// Dateien, kaputtes PEM und Zertifikate, die rustls nicht als Anker
    /// annimmt.
    #[must_use]
    pub const fn rejected(&self) -> usize {
        self.rejected
    }

    /// Die Wurzeln als rustls-Ablage, für [`crate::ClientTls`].
    pub(crate) fn roots(&self) -> RootCertStore {
        self.roots.clone()
    }

    /// Wertet eine Lesung der Ablage aus: übernehmen, was rustls annimmt,
    /// den Rest zählen und einmal zusammengefasst warnen.
    fn from_result(result: CertificateResult) -> Self {
        let mut roots = RootCertStore::empty();
        let (accepted, refused) = roots.add_parsable_certificates(result.certs);
        let rejected = refused + result.errors.len();
        if rejected > 0 {
            // Eine Zeile, nicht eine je Eintrag: Eine Ablage mit hundert
            // kaputten Dateien soll das Log nicht fluten, und keiner dieser
            // Einträge hält den Start auf.
            let first = result
                .errors
                .first()
                .map_or_else(|| "rejected by rustls".to_owned(), ToString::to_string);
            tracing::warn!(
                accepted,
                rejected,
                first = %first,
                "some entries of the system certificate store were skipped"
            );
        }
        if roots.is_empty() {
            tracing::warn!(
                "the system certificate store yielded no usable root; falling back to the \
                 bundled webpki roots"
            );
            return Self {
                roots: bundled_roots(),
                source: TrustSource::BundledFallback,
                rejected,
                redirected: Vec::new(),
            };
        }
        Self {
            roots,
            source: TrustSource::SystemStore,
            rejected,
            redirected: Vec::new(),
        }
    }

    /// `SSL_CERT_FILE=…` und `SSL_CERT_DIR=…`, wenn die Umgebung des
    /// Prozesses die Ablage umgelenkt hat (HUM-257, Review).
    #[must_use]
    pub fn redirected(&self) -> &[String] {
        &self.redirected
    }

    /// Die eine Startzeile: wie viele Wurzeln, woher, wie viele übersprungen,
    /// und ob die Umgebung die Ablage umgelenkt hat (`env`, sonst `none`).
    fn log(&self) {
        let env = if self.redirected.is_empty() {
            "none".to_owned()
        } else {
            self.redirected.join(" ")
        };
        tracing::info!(
            anchors = self.len(),
            source = self.source.as_str(),
            skipped = self.rejected,
            env = %env,
            "upstream trust anchors loaded"
        );
    }
}

/// Die Variable, die `rustls-native-certs` statt der Ablage als Datei liest.
const ENV_CERT_FILE: &str = "SSL_CERT_FILE";
/// Die Variable, die `rustls-native-certs` statt der Ablage als Verzeichnisse
/// liest, getrennt wie `PATH`.
const ENV_CERT_DIR: &str = "SSL_CERT_DIR";

/// Welche Pfade die Umgebung als Ablage nennt, wie `rustls-native-certs` sie
/// liest: eine Datei aus `SSL_CERT_FILE`, Verzeichnisse aus `SSL_CERT_DIR`.
fn env_redirect(
    file: Option<std::ffi::OsString>,
    dirs: Option<std::ffi::OsString>,
) -> Vec<(&'static str, PathBuf)> {
    let mut out = Vec::new();
    if let Some(file) = file.filter(|file| !file.is_empty()) {
        out.push((ENV_CERT_FILE, PathBuf::from(file)));
    }
    if let Some(dirs) = dirs {
        out.extend(
            std::env::split_paths(&dirs)
                .filter(|dir| !dir.as_os_str().is_empty())
                .map(|dir| (ENV_CERT_DIR, dir)),
        );
    }
    out
}

/// Ob dieses Konto `path` oder, bei einer Datei, ihr Verzeichnis beschreiben
/// kann: Es gehört dem Besitzer von `$HOME` (`home_uid`), oder Gruppe und
/// andere dürfen schreiben.
///
/// Absichtlich nicht „gehört nicht root": Unter `PrivateUsers=yes` der
/// Nutzer-Unit erscheint root als `nobody`, und die Warnung stünde bei jeder
/// Installation. Was zählt, ist, ob das eigene Konto und damit ein Projekt
/// darunter den Inhalt ändern kann.
fn writable_by_this_account(path: &Path, home_uid: Option<u32>) -> bool {
    let writable = |candidate: &Path| {
        std::fs::metadata(candidate)
            .is_ok_and(|meta| home_uid == Some(meta.uid()) || meta.mode() & 0o022 != 0)
    };
    let parent_writable = std::fs::metadata(path).is_ok_and(|meta| meta.is_file())
        && path.parent().is_some_and(writable);
    writable(path) || parent_writable
}

/// Die Wurzeln aus `webpki-roots`.
fn bundled_roots() -> RootCertStore {
    RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    }
}

/// `why` und Vorschlag für ein Ziel, dessen Zertifikat von einer Stelle
/// unterschrieben ist, der dieser Rechner nicht vertraut (HUM-257).
///
/// Der Vorschlag hängt an der Herkunft der Anker: Ist die Ablage des Systems
/// abgeschaltet, ist der nächste Schritt, sie einzuschalten; sonst gehört die
/// CA in die Ablage. `update-ca-certificates` übernimmt nur, was vorher als
/// `.crt` unter `/usr/local/share/ca-certificates/` liegt, deshalb steht das
/// im `why` und nicht nur der Befehl.
pub(crate) fn untrusted_issuer(target: &str, source: TrustSource) -> (String, FixAction) {
    let base = format!(
        "the certificate of {target} is signed by an authority this machine does not trust"
    );
    match source {
        TrustSource::BundledByConfig => (
            format!(
                "{base}; tls.trust_system_store is off, so only the bundled public roots count \
                 and a company or self-installed CA is never accepted. Switching it on takes \
                 effect when the daemon restarts"
            ),
            FixAction::ChangeSetting {
                key: "tls.trust_system_store".to_owned(),
                value: "true".to_owned(),
            },
        ),
        TrustSource::SystemStore | TrustSource::BundledFallback => (
            format!(
                "{base} (or the server does not send its intermediate certificate). If this is \
                 your organisation's CA, copy it as a .crt file to \
                 /usr/local/share/ca-certificates/, run update-ca-certificates and restart the \
                 daemon"
            ),
            FixAction::CopyCommand("sudo update-ca-certificates".to_owned()),
        ),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    #[test]
    fn an_empty_store_falls_back_to_the_bundled_roots() {
        let dir = tempfile::tempdir().unwrap();
        let empty = dir.path().join("empty.pem");
        std::fs::write(&empty, b"").unwrap();
        let anchors = TrustAnchors::from_paths(Some(&empty), None);
        assert_eq!(anchors.source(), TrustSource::BundledFallback);
        assert_eq!(anchors.len(), webpki_roots::TLS_SERVER_ROOTS.len());
    }

    #[test]
    fn a_missing_store_falls_back_and_counts_the_error() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nope");
        let anchors = TrustAnchors::from_paths(None, Some(&missing));
        assert_eq!(anchors.source(), TrustSource::BundledFallback);
        assert_eq!(anchors.rejected(), 1, "the unreadable directory is counted");
        assert!(!anchors.is_empty());
    }

    #[test]
    fn the_environment_redirect_is_read_like_rustls_native_certs() {
        let redirect = env_redirect(Some("/x/ca.pem".into()), Some("/a:/b".into()));
        assert_eq!(
            redirect,
            vec![
                (ENV_CERT_FILE, PathBuf::from("/x/ca.pem")),
                (ENV_CERT_DIR, PathBuf::from("/a")),
                (ENV_CERT_DIR, PathBuf::from("/b")),
            ]
        );
        assert!(env_redirect(None, None).is_empty());
        assert!(env_redirect(Some("".into()), None).is_empty());
    }

    #[test]
    fn a_store_this_account_can_write_is_flagged_and_a_foreign_one_is_not() {
        // Ein Verzeichnis dieses Kontos, etwa ein Projektordner: gemeldet,
        // als Verzeichnis und über eine Datei darin.
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("ca.pem");
        std::fs::write(&file, b"").unwrap();
        let mine = std::fs::metadata(dir.path()).unwrap().uid();
        assert!(writable_by_this_account(dir.path(), Some(mine)));
        assert!(writable_by_this_account(&file, Some(mine)));

        // Dieselben Pfade, wenn `$HOME` einem anderen gehört und niemand
        // sonst schreiben darf: nicht gemeldet.
        let mut perms = std::fs::metadata(dir.path()).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
        std::fs::set_permissions(dir.path(), perms).unwrap();
        let mut perms = std::fs::metadata(&file).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o644);
        std::fs::set_permissions(&file, perms).unwrap();
        assert!(!writable_by_this_account(&file, Some(mine.wrapping_add(1))));

        // Für andere schreibbar: gemeldet, gleich wem es gehört.
        let mut perms = std::fs::metadata(&file).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o666);
        std::fs::set_permissions(&file, perms).unwrap();
        assert!(writable_by_this_account(&file, Some(mine.wrapping_add(1))));

        // Ein Pfad, den es nicht gibt, ist nichts, was jemand beschreibt.
        assert!(!writable_by_this_account(
            &dir.path().join("nope"),
            Some(mine)
        ));
    }

    #[test]
    fn the_opt_out_proposes_to_switch_the_store_back_on() {
        let (why, fix) = untrusted_issuer("llm.example:443", TrustSource::BundledByConfig);
        assert!(why.contains("llm.example:443"), "{why}");
        assert!(why.contains("tls.trust_system_store"), "{why}");
        assert!(why.contains("when the daemon restarts"), "{why}");
        assert!(
            matches!(fix, FixAction::ChangeSetting { ref key, .. } if key == "tls.trust_system_store")
        );

        let (why, fix) = untrusted_issuer("llm.example:443", TrustSource::SystemStore);
        assert!(why.contains("does not trust"), "{why}");
        assert!(
            matches!(fix, FixAction::CopyCommand(ref cmd) if cmd.contains("update-ca-certificates"))
        );
    }
}
