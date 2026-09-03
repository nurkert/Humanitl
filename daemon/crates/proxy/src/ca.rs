//! Die eigene Zertifizierungsstelle des Proxys und das Env-Kit der Sandbox (HUM-014).
//!
//! Der Proxy bricht TLS auf (ADR-001). Dafür braucht er eine CA, der die
//! Werkzeuge in der Sandbox vertrauen. Sie entsteht beim ersten Start unter
//! `Paths::ca_dir()` (`$XDG_DATA_HOME/humanitl/ca/`): `ca.key` mit `0600`,
//! `ca.crt` mit `0644`. Danach wird sie nur noch gelesen; ein zweiter Aufruf
//! liefert Byte für Byte dieselben Dateien.
//!
//! Drei Regeln, die dieses Modul durchsetzt:
//!
//! 1. **Nie in den Host-Trust-Store.** Es gibt keine Funktion, die die CA
//!    irgendwo außer in ihr eigenes Verzeichnis schreibt. Vertrauen bekommt
//!    sie nur in der Sandbox: über das Bundle, das der Launcher als
//!    `/etc/humanitl/ca.crt` einhängt, und über das Env-Kit
//!    (`docs/SECURITY.md`, Abschnitt 5).
//! 2. **Der Schlüssel verlässt den Prozess nicht.** [`CaStore`] hält ihn als
//!    [`Zeroizing`]; `Debug` zeigt ihn nicht; er gehört in keinen
//!    `LaunchPlan` und in kein Log.
//! 3. **Fail-closed beim Laden.** Ein unlesbarer Schlüssel, ein Zertifikat,
//!    das nicht zum Schlüssel passt, oder ein Schlüssel mit Rechten für andere
//!    Nutzer ist [`TLS_005`], nie ein stilles Neuanlegen: ein Schlüssel, den
//!    andere lesen konnten, ist verbrannt, und das soll jemand sehen.
//!
//! Leaf-Zertifikate je Host kommen aus [`LeafCache`], einem begrenzten
//! LRU-Speicher, den HUM-015 hinter hudsuckers `CertificateAuthority` hängt.
//!
//! Nicht im MVP: ein Java-Truststore (`cacerts.p12`, `JAVA_TOOL_OPTIONS`).
//! JVM-Werkzeuge in der Sandbox sehen bis dahin TLS-Fehler, also fail-closed;
//! ein späteres Issue erzeugt die PKCS#12-Datei und ergänzt Env-Kit und
//! Profil. Ebenso Post-MVP: Keyring-Unlock und eine ephemere CA je Sitzung.

use std::collections::HashMap;
use std::fmt::{self, Write as _};
use std::fs::{self, DirBuilder, OpenOptions};
use std::io::{self, Write as _};
use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{self, AtomicU64};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use humanitl_config::Paths;
use humanitl_core::diagnostics::codes::{TLS_004, TLS_005};
use humanitl_core::{Diagnostic, FixAction, HostName, SessionId, Severity};
use rcgen::{
    BasicConstraints, CertificateParams, DistinguishedName, DnType, ExtendedKeyUsagePurpose, IsCa,
    Issuer, KeyPair, KeyUsagePurpose, PKCS_ECDSA_P256_SHA256, SanType,
};
use rustls::client::WebPkiServerVerifier;
use rustls::client::danger::ServerCertVerifier as _;
use rustls::crypto::CryptoProvider;
use rustls::pki_types::pem::PemObject as _;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::{RootCertStore, ServerConfig};
use sha2::{Digest as _, Sha256};
use time::{Duration, OffsetDateTime};
use zeroize::Zeroizing;

/// Dateiname des privaten Schlüssels im CA-Verzeichnis.
pub const KEY_FILE: &str = "ca.key";
/// Dateiname des CA-Zertifikats.
pub const CERT_FILE: &str = "ca.crt";
/// Dateiname des Bundles, das die Sandbox als [`SANDBOX_CA_PATH`] sieht.
pub const BUNDLE_FILE: &str = "ca-bundle.crt";
/// Rechte des CA-Verzeichnisses.
pub const DIR_MODE: u32 = 0o700;
/// Rechte des Schlüssels: der Nutzer, sonst niemand.
pub const KEY_MODE: u32 = 0o600;
/// Rechte von Zertifikat und Bundle: öffentlich, sie enthalten kein Geheimnis.
pub const CERT_MODE: u32 = 0o644;

/// Wo die Sandbox das Bundle sieht (`[network].ca_cert_dst` im Profil).
pub const SANDBOX_CA_PATH: &str = "/etc/humanitl/ca.crt";
/// Wo die Sandbox den Proxy erreicht (`[network].bridges` im Profil).
pub const SANDBOX_PROXY_URL: &str = "http://127.0.0.1:3128";
/// Das Zertifikatsverzeichnis in der Sandbox; `/etc/ssl` ist nur lesbar eingehängt.
pub const SANDBOX_CERT_DIR: &str = "/etc/ssl/certs";
/// Die Variable, die das Env-Kit um die Sitzung ergänzt.
pub const ENV_KIT_SESSION_KEY: &str = "HUMANITL_SESSION";

/// ALPN, das der Proxy dem Client anbietet: in M1 nur HTTP/1.1 (CONVENTIONS 4.10).
pub const ALPN_HTTP1: &[u8] = b"http/1.1";
/// Standardgröße des Leaf-Speichers.
pub const DEFAULT_LEAF_CAPACITY: usize = 1000;

/// Wo Distributionen ihr Root-Bundle ablegen, in Prüfreihenfolge.
pub const SYSTEM_BUNDLE_CANDIDATES: &[&str] = &[
    // Debian, Ubuntu, Arch, Alpine (Paket ca-certificates)
    "/etc/ssl/certs/ca-certificates.crt",
    // Fedora, RHEL
    "/etc/pki/tls/certs/ca-bundle.crt",
    // openSUSE
    "/etc/ssl/ca-bundle.pem",
    // Alpine (Symlink), macOS
    "/etc/ssl/cert.pem",
];

/// Das Env-Kit: jede Variable, die ein Werkzeug in der Sandbox auf den Proxy
/// und die CA lenkt, mit dem Wert, den die Sandbox sieht.
///
/// Das ist die verbindliche Tabelle aus HUM-014; `profiles/sandbox/*.toml`
/// trägt dieselben Paare unter `[env]`, und `tests/ca.rs` gleicht beide ab.
/// Nicht hier: `HOME`, `USER`, `TERM`, `LANG`, `PATH` (Sache des Profils),
/// `HUMANITL_SESSION` (siehe [`env_kit`]), `JAVA_TOOL_OPTIONS` (kein
/// Truststore im MVP), `GOFLAGS` (Go liest `SSL_CERT_FILE`).
///
/// `NO_PROXY` steht ausdrücklich leer: ein Agent-Image mit `NO_PROXY=*` würde
/// sonst den Proxy umgehen wollen, und `--clearenv` allein schützt nicht vor
/// einem Wert, den ein Werkzeug selbst als Default annimmt.
pub const ENV_KIT: &[(&str, &str)] = &[
    ("HTTP_PROXY", SANDBOX_PROXY_URL),
    ("HTTPS_PROXY", SANDBOX_PROXY_URL),
    ("http_proxy", SANDBOX_PROXY_URL),
    ("https_proxy", SANDBOX_PROXY_URL),
    ("ALL_PROXY", SANDBOX_PROXY_URL),
    ("NO_PROXY", ""),
    ("no_proxy", ""),
    ("SSL_CERT_FILE", SANDBOX_CA_PATH),
    ("SSL_CERT_DIR", SANDBOX_CERT_DIR),
    ("CURL_CA_BUNDLE", SANDBOX_CA_PATH),
    ("REQUESTS_CA_BUNDLE", SANDBOX_CA_PATH),
    ("PIP_CERT", SANDBOX_CA_PATH),
    ("NODE_EXTRA_CA_CERTS", SANDBOX_CA_PATH),
    ("NPM_CONFIG_CAFILE", SANDBOX_CA_PATH),
    ("DENO_CERT", SANDBOX_CA_PATH),
    ("GIT_SSL_CAINFO", SANDBOX_CA_PATH),
    ("CARGO_HTTP_CAINFO", SANDBOX_CA_PATH),
    ("NIX_SSL_CERT_FILE", SANDBOX_CA_PATH),
    ("AWS_CA_BUNDLE", SANDBOX_CA_PATH),
    ("HUMANITL", "1"),
];

/// Das Env-Kit einer Sitzung: alle Paare aus [`ENV_KIT`] plus
/// `HUMANITL_SESSION=<session-id>`, in der Form, die `LaunchPlan.env` und
/// `--setenv` erwarten.
#[must_use]
pub fn env_kit(session: SessionId) -> Vec<(String, String)> {
    let mut kit: Vec<(String, String)> = ENV_KIT
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect();
    kit.push((ENV_KIT_SESSION_KEY.to_owned(), session.to_string()));
    kit
}

const CA_VALIDITY: Duration = Duration::days(10 * 365);
const LEAF_VALIDITY: Duration = Duration::days(365);
/// `NotBefore` liegt einen Tag zurück, damit ein Uhrenversatz nicht stört.
const BACKDATE: Duration = Duration::days(1);
/// Ein Leaf, das in weniger als dieser Zeit abläuft, wird neu ausgestellt.
const RENEW_MARGIN: Duration = Duration::hours(1);
const CN_PREFIX: &str = "Humanitl Local CA ";
const ORGANIZATION: &str = "Humanitl";
/// Der Host, für den der Selbsttest beim Laden ein Leaf ausstellt.
const SELF_CHECK_HOST: &str = "selfcheck.humanitl.invalid";
const MACHINE_ID_FILES: &[&str] = &["/etc/machine-id", "/var/lib/dbus/machine-id"];
/// Längstes `CommonName`, das X.520 erlaubt.
const CN_MAX_LEN: usize = 64;

/// Die CA einer Installation: Schlüssel, Zertifikat, Aussteller.
///
/// Entsteht mit [`CaStore::load_or_create`] und ändert sich danach nicht mehr.
/// Alles, was den Schlüssel braucht (Leafs, Selbsttest), geht über Methoden;
/// der Schlüssel selbst ist nur über [`CaStore::key_pem`] erreichbar, und
/// diese Methode ist für die Übergabe an hudsuckers `RcgenAuthority` gedacht,
/// nicht für Anzeige oder Protokoll.
pub struct CaStore {
    dir: PathBuf,
    key_pem: Zeroizing<String>,
    cert_pem: String,
    cert_der: CertificateDer<'static>,
    issuer: Issuer<'static, KeyPair>,
    provider: Arc<CryptoProvider>,
    created: bool,
}

impl fmt::Debug for CaStore {
    /// Zeigt Verzeichnis und Fingerabdruck; den Schlüssel nie.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CaStore")
            .field("dir", &self.dir)
            .field("fingerprint_sha256", &self.fingerprint_sha256())
            .field("created", &self.created)
            .field("key_pem", &"[elided]")
            .finish_non_exhaustive()
    }
}

impl CaStore {
    /// Die CA unter `Paths::ca_dir()`, angelegt oder geladen.
    ///
    /// # Errors
    ///
    /// Wie [`CaStore::load_or_create`].
    pub fn open(paths: &Paths) -> Result<Self, Diagnostic> {
        let store = Self::load_or_create(&paths.ca_dir())?;
        // Bei jedem Start neu, damit Aktualisierungen der System-Wurzeln in der
        // Sandbox ankommen; das Bundle ist die Datei, die als
        // `/etc/humanitl/ca.crt` eingehängt wird.
        store.write_bundle_with_system_roots()?;
        Ok(store)
    }

    /// Lädt die CA aus `ca_dir` oder legt sie an, wenn weder `ca.key` noch
    /// `ca.crt` existiert.
    ///
    /// Beim Anlegen: ECDSA P-256, `CN = Humanitl Local CA <kurz-id>` mit acht
    /// Hex-Zeichen aus dem Hash der `machine-id`, zehn Jahre gültig,
    /// `NotBefore` einen Tag zurück. Das Verzeichnis wird `0700`, der Schlüssel
    /// `0600`, das Zertifikat `0644`; beide Dateien entstehen über eine
    /// temporäre Datei und `rename`, damit kein halb geschriebener Schlüssel
    /// liegen bleibt. Was danach auf der Platte liegt, wird zurückgelesen und
    /// ist die Wahrheit.
    ///
    /// Beim Laden: Rechte des Schlüssels, Lesbarkeit beider Dateien, dann ein
    /// Selbsttest, der ein Leaf ausstellt und es mit rustls gegen das
    /// Zertifikat prüft. So fällt ein Schlüssel, der nicht zum Zertifikat
    /// gehört, hier auf und nicht erst beim ersten Handschlag in der Sandbox.
    ///
    /// # Errors
    ///
    /// [`TLS_004`], wenn Verzeichnis oder Dateien nicht angelegt, geschrieben
    /// oder umbenannt werden können (Fix: `mkdir -p … && chmod 700 …`).
    /// [`TLS_005`], wenn nur eine der beiden Dateien existiert, eine nicht
    /// lesbar oder nicht PEM ist, der Schlüssel Rechte für Gruppe oder Andere
    /// trägt, oder Schlüssel und Zertifikat nicht zusammenpassen (Fix:
    /// `rm -r <ca_dir>`, der nächste Start legt eine frische CA an).
    pub fn load_or_create(ca_dir: &Path) -> Result<Self, Diagnostic> {
        let key_path = ca_dir.join(KEY_FILE);
        let cert_path = ca_dir.join(CERT_FILE);
        match (key_path.exists(), cert_path.exists()) {
            (true, true) => Self::load(ca_dir),
            (false, false) => Self::create(ca_dir),
            (true, false) => Err(corrupt(
                ca_dir,
                format!(
                    "{} exists but {} is missing",
                    key_path.display(),
                    cert_path.display()
                ),
            )),
            (false, true) => Err(corrupt(
                ca_dir,
                format!(
                    "{} exists but {} is missing",
                    cert_path.display(),
                    key_path.display()
                ),
            )),
        }
    }

    fn create(dir: &Path) -> Result<Self, Diagnostic> {
        ensure_dir(dir)?;
        let now = OffsetDateTime::now_utc();
        let key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
            .map_err(|err| corrupt(dir, format!("key generation failed: {err}")))?;
        let params = ca_params(&short_id(), now);
        let cert = params
            .self_signed(&key)
            .map_err(|err| corrupt(dir, format!("self-signing the CA failed: {err}")))?;
        let key_pem = Zeroizing::new(key.serialize_pem());
        write_atomic(dir, KEY_FILE, key_pem.as_bytes(), KEY_MODE)?;
        if let Err(err) = write_atomic(dir, CERT_FILE, cert.pem().as_bytes(), CERT_MODE) {
            // Kein halbes Verzeichnis zurücklassen: ohne Zertifikat ist der
            // Schlüssel wertlos, und der nächste Start soll frisch anlegen
            // statt TLS_005 zu melden. Scheitert das Löschen, bleibt der
            // ursprüngliche Befund der wichtigere.
            let _ = fs::remove_file(dir.join(KEY_FILE));
            return Err(err);
        }

        // Zurücklesen: geladen wird, was auf der Platte liegt, nicht, was wir
        // zu schreiben glaubten. Der Selbsttest läuft damit auch hier.
        let mut store = Self::load(dir)?;
        store.created = true;
        Ok(store)
    }

    fn load(dir: &Path) -> Result<Self, Diagnostic> {
        // Auch beim Laden: ein Verzeichnis, das jemand nachträglich geöffnet
        // hat, wird wieder auf 0700 gezogen, nicht nur beim Anlegen.
        ensure_dir(dir)?;
        let key_path = dir.join(KEY_FILE);
        let cert_path = dir.join(CERT_FILE);
        check_key_mode(dir, &key_path)?;
        let key_pem = Zeroizing::new(read_pem(dir, &key_path)?);
        let cert_pem = read_pem(dir, &cert_path)?;

        let key = KeyPair::from_pem(&key_pem).map_err(|err| {
            corrupt(
                dir,
                format!("{} is not a usable key: {err}", key_path.display()),
            )
        })?;
        let cert_der = CertificateDer::from_pem_slice(cert_pem.as_bytes()).map_err(|err| {
            corrupt(
                dir,
                format!("{} is not a PEM certificate: {err}", cert_path.display()),
            )
        })?;
        let issuer = Issuer::from_ca_cert_der(&cert_der, key).map_err(|err| {
            corrupt(
                dir,
                format!("{} is not an X.509 certificate: {err}", cert_path.display()),
            )
        })?;

        let store = Self {
            dir: dir.to_owned(),
            key_pem,
            cert_pem,
            cert_der,
            issuer,
            provider: Arc::new(rustls::crypto::ring::default_provider()),
            created: false,
        };
        store.self_check()?;
        Ok(store)
    }

    /// Stellt ein Leaf für den Selbsttest-Host aus und prüft es gegen das
    /// Zertifikat. Schlägt das fehl, gehören Schlüssel und Zertifikat nicht
    /// zusammen oder das Zertifikat taugt nicht als Anker.
    fn self_check(&self) -> Result<(), Diagnostic> {
        let host = HostName::Dns(SELF_CHECK_HOST.to_owned());
        let leaf = self.issue_leaf(&host)?;
        self.verify_leaf(&leaf.cert, &host, UnixTime::now())
            .map_err(|err| {
                corrupt(
                    &self.dir,
                    format!(
                        "{} and {} do not belong together: {}",
                        self.key_path().display(),
                        self.cert_path().display(),
                        err.why
                    ),
                )
            })
    }

    /// Das CA-Verzeichnis.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// `ca.key`.
    #[must_use]
    pub fn key_path(&self) -> PathBuf {
        self.dir.join(KEY_FILE)
    }

    /// `ca.crt`.
    #[must_use]
    pub fn cert_path(&self) -> PathBuf {
        self.dir.join(CERT_FILE)
    }

    /// `ca-bundle.crt`: die Datei, die der Launcher als [`SANDBOX_CA_PATH`]
    /// einhängt. Existiert erst nach [`CaStore::write_bundle`].
    #[must_use]
    pub fn bundle_path(&self) -> PathBuf {
        self.dir.join(BUNDLE_FILE)
    }

    /// Wahr, wenn dieser Aufruf die CA neu angelegt hat.
    #[must_use]
    pub const fn was_created(&self) -> bool {
        self.created
    }

    /// Das Zertifikat als PEM, genau der Inhalt von `ca.crt`.
    #[must_use]
    pub fn cert_pem(&self) -> &str {
        &self.cert_pem
    }

    /// Das Zertifikat als DER.
    #[must_use]
    pub fn cert_der(&self) -> &CertificateDer<'static> {
        &self.cert_der
    }

    /// Der private Schlüssel als PEM (PKCS#8), genau der Inhalt von `ca.key`.
    ///
    /// Nur für die Übergabe an einen Aussteller wie hudsuckers
    /// `RcgenAuthority`. Nie anzeigen, nie protokollieren, nie in einen
    /// `LaunchPlan` legen.
    #[must_use]
    pub fn key_pem(&self) -> &Zeroizing<String> {
        &self.key_pem
    }

    /// Der Krypto-Anbieter, mit dem Leafs und Selbsttest arbeiten (ring).
    #[must_use]
    pub fn provider(&self) -> Arc<CryptoProvider> {
        Arc::clone(&self.provider)
    }

    /// SHA-256 über das DER des Zertifikats, als `AB:CD:…`, wie es Browser
    /// und `openssl x509 -fingerprint -sha256` zeigen. Für die Oberfläche.
    #[must_use]
    pub fn fingerprint_sha256(&self) -> String {
        let digest = Sha256::digest(self.cert_der.as_ref());
        let mut out = String::with_capacity(digest.len() * 3);
        for (i, byte) in digest.iter().enumerate() {
            if i > 0 {
                out.push(':');
            }
            // Schreiben in einen String schlägt nie fehl.
            let _ = write!(out, "{byte:02X}");
        }
        out
    }

    /// Stellt ein Leaf-Zertifikat für `host` aus, mit frischem P-256-Schlüssel.
    ///
    /// SAN ist der Host (DNS-Name oder IP-Adresse), `CN` dazu, falls er in
    /// 64 Zeichen passt; `CA:FALSE`, `digitalSignature`, `serverAuth`, ein Jahr
    /// gültig, `NotBefore` einen Tag zurück. Ohne Speicher: für jeden Aufruf ein
    /// neues Zertifikat. Der Proxy nutzt [`LeafCache`].
    ///
    /// # Errors
    ///
    /// [`TLS_005`], wenn Schlüsselerzeugung oder Signatur fehlschlagen; das
    /// heißt, das CA-Material ist unbrauchbar.
    pub fn issue_leaf(&self, host: &HostName) -> Result<Leaf, Diagnostic> {
        let now = OffsetDateTime::now_utc();
        let key =
            KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).map_err(|err| leaf_error(host, &err))?;

        let mut params = CertificateParams::default();
        params.not_before = now - BACKDATE;
        params.not_after = now + LEAF_VALIDITY;
        params.subject_alt_names = vec![match host {
            HostName::Dns(name) => SanType::DnsName(
                name.as_str()
                    .try_into()
                    .map_err(|err| leaf_error(host, &err))?,
            ),
            HostName::Ip(ip) => SanType::IpAddress(*ip),
        }];
        let mut dn = DistinguishedName::new();
        dn.push(DnType::OrganizationName, ORGANIZATION);
        let cn = host.to_string();
        if cn.len() <= CN_MAX_LEN {
            dn.push(DnType::CommonName, cn);
        }
        params.distinguished_name = dn;
        params.is_ca = IsCa::ExplicitNoCa;
        params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
        params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        params.use_authority_key_identifier_extension = true;

        let cert = params
            .signed_by(&key, &self.issuer)
            .map_err(|err| leaf_error(host, &err))?;
        Ok(Leaf {
            host: host.clone(),
            cert: cert.der().clone(),
            key: PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(key.serialize_der())),
            not_after: params.not_after,
        })
    }

    /// Prüft ein Leaf mit rustls/webpki gegen diese CA als einzigen Anker:
    /// Kette, Gültigkeit zum Zeitpunkt `at`, `serverAuth`, und dass es für
    /// `host` gilt.
    ///
    /// # Errors
    ///
    /// [`TLS_005`] mit dem Grund aus rustls, wenn das Leaf nicht gilt.
    pub fn verify_leaf(
        &self,
        cert: &CertificateDer<'_>,
        host: &HostName,
        at: UnixTime,
    ) -> Result<(), Diagnostic> {
        let mut roots = RootCertStore::empty();
        roots
            .add(self.cert_der.clone())
            .map_err(|err| corrupt(&self.dir, format!("the CA is no trust anchor: {err}")))?;
        let verifier =
            WebPkiServerVerifier::builder_with_provider(Arc::new(roots), self.provider())
                .build()
                .map_err(|err| corrupt(&self.dir, format!("cannot build a verifier: {err}")))?;
        let name = server_name(host).map_err(|err| leaf_error(host, &err))?;
        verifier
            .verify_server_cert(cert, &[], &name, &[], at)
            .map(|_| ())
            .map_err(|err| leaf_error(host, &err))
    }

    /// Setzt das Bundle zusammen, das die Sandbox als [`SANDBOX_CA_PATH`]
    /// sieht: zuerst diese CA, dann jedes Zertifikat aus `system_pem`, als
    /// saubere PEM-Blöcke ohne Kommentare oder Fremdes.
    ///
    /// Die System-Roots sind dabei, damit ein Werkzeug in der Sandbox auch
    /// dem echten Zertifikat des LLM-Hosts vertraut, wenn der Proxy die
    /// Verbindung dorthin nicht aufbricht (Passthrough). Unlesbare Blöcke
    /// werden übersprungen. Liefert Text und Zahl der übernommenen
    /// System-Zertifikate.
    #[must_use]
    pub fn render_bundle(&self, system_pem: &[u8]) -> (String, usize) {
        let mut out = String::with_capacity(self.cert_pem.len() + system_pem.len() + 1);
        out.push_str(self.cert_pem.trim_end());
        out.push('\n');
        let mut count = 0;
        for cert in CertificateDer::pem_slice_iter(system_pem).flatten() {
            out.push_str(&pem_block(cert.as_ref()));
            count += 1;
        }
        (out, count)
    }

    /// Schreibt [`CaStore::render_bundle`] nach `ca-bundle.crt` (`0644`,
    /// atomar). Wird bei jedem Daemon-Start neu erzeugt, damit Updates der
    /// System-Roots ankommen.
    ///
    /// # Errors
    ///
    /// [`TLS_004`], wenn die Datei nicht geschrieben werden kann.
    pub fn write_bundle(&self, system_pem: Option<&[u8]>) -> Result<Bundle, Diagnostic> {
        let (text, system_certs) = self.render_bundle(system_pem.unwrap_or_default());
        ensure_dir(&self.dir)?;
        write_atomic(&self.dir, BUNDLE_FILE, text.as_bytes(), CERT_MODE)?;
        Ok(Bundle {
            path: self.bundle_path(),
            system_source: None,
            system_certs,
        })
    }

    /// Wie [`CaStore::write_bundle`], mit dem Root-Bundle des Hosts aus
    /// [`read_system_bundle`]. Fehlt es, enthält das Bundle nur diese CA;
    /// `Bundle::system_source` sagt, was gefunden wurde.
    ///
    /// # Errors
    ///
    /// Wie [`CaStore::write_bundle`].
    pub fn write_bundle_with_system_roots(&self) -> Result<Bundle, Diagnostic> {
        let system = read_system_bundle();
        let mut bundle = self.write_bundle(system.as_ref().map(|(_, pem)| pem.as_slice()))?;
        bundle.system_source = system.map(|(path, _)| path);
        Ok(bundle)
    }
}

/// Ein ausgestelltes Leaf-Zertifikat samt Schlüssel.
pub struct Leaf {
    /// Der Host, für den es gilt.
    pub host: HostName,
    /// Das Zertifikat als DER.
    pub cert: CertificateDer<'static>,
    /// Der private Schlüssel (PKCS#8).
    pub key: PrivateKeyDer<'static>,
    /// Ende der Gültigkeit.
    pub not_after: OffsetDateTime,
}

impl fmt::Debug for Leaf {
    /// Zeigt Host und Ablauf; den Schlüssel nie.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Leaf")
            .field("host", &self.host)
            .field("not_after", &self.not_after)
            .field("key", &"[elided]")
            .finish_non_exhaustive()
    }
}

impl Leaf {
    /// Eine rustls-Serverkonfiguration mit diesem Leaf: sichere
    /// Protokollversionen, keine Client-Authentisierung, ALPN nur
    /// [`ALPN_HTTP1`] (M1; HTTP/2 kommt in M6 hierher).
    ///
    /// # Errors
    ///
    /// [`TLS_005`], wenn rustls Zertifikat oder Schlüssel ablehnt.
    pub fn server_config(
        &self,
        provider: Arc<CryptoProvider>,
    ) -> Result<Arc<ServerConfig>, Diagnostic> {
        let why = |err: &dyn fmt::Display| {
            Diagnostic::builder(TLS_005, Severity::Error)
                .why(format!(
                    "rustls rejected the certificate for {}: {err}",
                    self.host
                ))
                .build()
        };
        let mut config = ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .map_err(|err| why(&err))?
            .with_no_client_auth()
            .with_single_cert(vec![self.cert.clone()], self.key.clone_key())
            .map_err(|err| why(&err))?;
        config.alpn_protocols = vec![ALPN_HTTP1.to_vec()];
        Ok(Arc::new(config))
    }
}

/// Was [`CaStore::write_bundle`] geschrieben hat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bundle {
    /// Die Datei, die der Launcher als [`SANDBOX_CA_PATH`] einhängt.
    pub path: PathBuf,
    /// Das Root-Bundle des Hosts, falls eines gefunden wurde.
    pub system_source: Option<PathBuf>,
    /// Wie viele System-Zertifikate hinter der CA stehen.
    pub system_certs: usize,
}

/// Sucht das Root-Bundle des Hosts entlang [`SYSTEM_BUNDLE_CANDIDATES`] und
/// nimmt das erste, das mindestens ein gültiges Zertifikat enthält.
///
/// Lesbarkeit allein genügt nicht: Ein abgeschnittenes oder leeres erstes
/// Bundle ergäbe sonst ein Bundle ohne eine einzige System-Wurzel, und in der
/// Sandbox schlüge jede Verbindung zu einem fremden Host fehl, ohne dass
/// jemand sähe, warum. Wer nichts Gültiges liefert, wird übersprungen.
#[must_use]
pub fn read_system_bundle() -> Option<(PathBuf, Vec<u8>)> {
    SYSTEM_BUNDLE_CANDIDATES.iter().find_map(|candidate| {
        let pem = fs::read(candidate).ok()?;
        let usable = CertificateDer::pem_slice_iter(&pem).flatten().next();
        usable.map(|_| (PathBuf::from(candidate), pem))
    })
}

/// Leaf-Zertifikate je Host, begrenzt und LRU.
///
/// Ein Eintrag ist eine fertige [`ServerConfig`]; wer denselben Host noch
/// einmal fragt, bekommt denselben `Arc`. Ist der Speicher voll, fliegt der
/// am längsten nicht benutzte Eintrag. Ein Leaf, das in weniger als einer
/// Stunde abläuft, wird neu ausgestellt. Die Ausstellung läuft außerhalb des
/// Locks; stellen zwei Aufrufer denselben Host gleichzeitig aus, gewinnt der
/// erste Eintrag, der zweite wird verworfen.
pub struct LeafCache {
    ca: Arc<CaStore>,
    capacity: usize,
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    entries: HashMap<HostName, Entry>,
    tick: u64,
}

struct Entry {
    config: Arc<ServerConfig>,
    not_after: OffsetDateTime,
    last_used: u64,
}

impl fmt::Debug for LeafCache {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LeafCache")
            .field("capacity", &self.capacity)
            .field("len", &self.len())
            .finish_non_exhaustive()
    }
}

impl LeafCache {
    /// Ein Speicher für höchstens `capacity` Hosts (mindestens einen).
    #[must_use]
    pub fn new(ca: Arc<CaStore>, capacity: usize) -> Self {
        Self {
            ca,
            capacity: capacity.max(1),
            inner: Mutex::new(Inner::default()),
        }
    }

    /// Die Serverkonfiguration für `host`, aus dem Speicher oder frisch.
    ///
    /// # Errors
    ///
    /// Wie [`CaStore::issue_leaf`] und [`Leaf::server_config`].
    pub fn server_config(&self, host: &HostName) -> Result<Arc<ServerConfig>, Diagnostic> {
        let now = OffsetDateTime::now_utc();
        if let Some(config) = self.touch(host, now) {
            return Ok(config);
        }
        let leaf = self.ca.issue_leaf(host)?;
        let config = leaf.server_config(self.ca.provider())?;
        Ok(self.insert(host.clone(), config, leaf.not_after))
    }

    /// Wie viele Hosts der Speicher hält.
    #[must_use]
    pub fn len(&self) -> usize {
        self.lock().entries.len()
    }

    /// Wahr, wenn noch kein Host ausgestellt wurde.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Die Obergrenze.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    /// Wahr, wenn `host` im Speicher liegt; zählt nicht als Zugriff.
    #[must_use]
    pub fn contains(&self, host: &HostName) -> bool {
        self.lock().entries.contains_key(host)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn touch(&self, host: &HostName, now: OffsetDateTime) -> Option<Arc<ServerConfig>> {
        let mut inner = self.lock();
        inner.tick += 1;
        let tick = inner.tick;
        let expiring = inner
            .entries
            .get(host)
            .is_some_and(|entry| entry.not_after - now < RENEW_MARGIN);
        if expiring {
            inner.entries.remove(host);
            return None;
        }
        let entry = inner.entries.get_mut(host)?;
        entry.last_used = tick;
        Some(Arc::clone(&entry.config))
    }

    fn insert(
        &self,
        host: HostName,
        config: Arc<ServerConfig>,
        not_after: OffsetDateTime,
    ) -> Arc<ServerConfig> {
        let mut inner = self.lock();
        inner.tick += 1;
        let tick = inner.tick;
        if let Some(existing) = inner.entries.get_mut(&host) {
            existing.last_used = tick;
            return Arc::clone(&existing.config);
        }
        while inner.entries.len() >= self.capacity {
            let oldest = inner
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_used)
                .map(|(host, _)| host.clone());
            match oldest {
                Some(host) => {
                    inner.entries.remove(&host);
                }
                None => break,
            }
        }
        inner.entries.insert(
            host,
            Entry {
                config: Arc::clone(&config),
                not_after,
                last_used: tick,
            },
        );
        config
    }
}

fn ca_params(short_id: &str, now: OffsetDateTime) -> CertificateParams {
    let mut params = CertificateParams::default();
    params.not_before = now - BACKDATE;
    params.not_after = now + CA_VALIDITY;
    let mut dn = DistinguishedName::new();
    dn.push(DnType::OrganizationName, ORGANIZATION);
    dn.push(DnType::CommonName, format!("{CN_PREFIX}{short_id}"));
    params.distinguished_name = dn;
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
    params
}

/// Acht Hex-Zeichen aus dem Hash der `machine-id`, damit zwei Installationen
/// im `CN` unterscheidbar sind. Die `machine-id` selbst steht nicht im
/// Zertifikat. Ohne `machine-id` (Container) zählt Zeit plus PID.
fn short_id() -> String {
    let seed = MACHINE_ID_FILES
        .iter()
        .find_map(|path| fs::read_to_string(path).ok())
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.as_nanos());
            format!("fallback:{nanos}:{}", std::process::id())
        });
    let digest = Sha256::new()
        .chain_update(b"humanitl-ca:")
        .chain_update(seed.as_bytes())
        .finalize();
    digest[..4]
        .iter()
        .fold(String::with_capacity(8), |mut out, byte| {
            let _ = write!(out, "{byte:02x}");
            out
        })
}

fn server_name(
    host: &HostName,
) -> Result<ServerName<'static>, rustls::pki_types::InvalidDnsNameError> {
    match host {
        HostName::Dns(name) => ServerName::try_from(name.clone()),
        HostName::Ip(ip) => Ok(ServerName::IpAddress((*ip).into())),
    }
}

/// Ein PEM-Block `CERTIFICATE` mit 64 Zeichen je Zeile.
fn pem_block(der: &[u8]) -> String {
    let encoded = BASE64.encode(der);
    let mut out = String::with_capacity(encoded.len() + encoded.len() / 64 + 64);
    out.push_str("-----BEGIN CERTIFICATE-----\n");
    for line in encoded.as_bytes().chunks(64) {
        out.push_str(&String::from_utf8_lossy(line));
        out.push('\n');
    }
    out.push_str("-----END CERTIFICATE-----\n");
    out
}

/// Legt `dir` mit `0700` an (Eltern mit Standardrechten) und zieht die Rechte
/// eines vorhandenen Verzeichnisses auf `0700`, falls Gruppe oder Andere
/// etwas dürfen.
fn ensure_dir(dir: &Path) -> Result<(), Diagnostic> {
    let result = (|| -> io::Result<()> {
        if let Some(parent) = dir.parent() {
            fs::create_dir_all(parent)?;
        }
        match DirBuilder::new().mode(DIR_MODE).create(dir) {
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
            fs::set_permissions(dir, fs::Permissions::from_mode(DIR_MODE))?;
        }
        Ok(())
    })();
    result.map_err(|err| not_writable(dir, format!("{}: {err}", dir.display())))
}

/// Zähler für die temporären Namen in [`write_atomic`], damit zwei Threads
/// derselben Prozess-ID nie denselben Pfad wählen.
static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Schreibt `bytes` als `dir/name` mit genau `mode`: erst in eine temporäre
/// Datei daneben, dann `rename`. Die Rechte werden nach dem Anlegen gesetzt,
/// damit die `umask` sie nicht verändert.
fn write_atomic(dir: &Path, name: &str, bytes: &[u8], mode: u32) -> Result<(), Diagnostic> {
    let target = dir.join(name);
    // Der Name ist bewusst nicht vorhersagbar und die Datei wird mit
    // `create_new` angelegt: Wer den Pfad erraten und dort vorher einen
    // Symlink hinlegen könnte, würde sonst den frisch geschriebenen
    // Schlüssel in ein fremdes Verzeichnis lenken. `O_NOFOLLOW` schließt
    // denselben Weg für eine bereits liegende Datei.
    let tmp = dir.join(format!(
        ".{name}.tmp-{}-{}",
        std::process::id(),
        TMP_COUNTER.fetch_add(1, atomic::Ordering::Relaxed)
    ));
    let result = (|| -> io::Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .custom_flags(libc::O_NOFOLLOW)
            .mode(mode)
            .open(&tmp)?;
        file.set_permissions(fs::Permissions::from_mode(mode))?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&tmp, &target)
    })();
    result.map_err(|err| {
        // Best effort: der Rest bleibt nicht liegen; ein Fehler hier ändert nichts am Befund.
        let _ = fs::remove_file(&tmp);
        not_writable(dir, format!("{}: {err}", target.display()))
    })
}

/// Liest eine PEM-Datei; alles, was nicht lesbar oder kein UTF-8 ist, ist
/// [`TLS_005`].
fn read_pem(dir: &Path, path: &Path) -> Result<String, Diagnostic> {
    fs::read_to_string(path).map_err(|err| corrupt(dir, format!("{}: {err}", path.display())))
}

/// Der Schlüssel muss eine reguläre Datei sein, die nur der Nutzer lesen darf.
fn check_key_mode(dir: &Path, key_path: &Path) -> Result<(), Diagnostic> {
    // `symlink_metadata`, nicht `metadata`: Ein Symlink an dieser Stelle
    // zeigte sonst auf eine fremde Datei, deren Rechte hier geprüft würden,
    // während gelesen und geschrieben würde, worauf er zeigt.
    let meta = fs::symlink_metadata(key_path)
        .map_err(|err| corrupt(dir, format!("{}: {err}", key_path.display())))?;
    if !meta.is_file() {
        return Err(corrupt(
            dir,
            format!(
                "{} is not a regular file (a symlink here is refused, not followed)",
                key_path.display()
            ),
        ));
    }
    let mode = meta.permissions().mode() & 0o777;
    if mode & 0o077 != 0 {
        return Err(corrupt(
            dir,
            format!(
                "{} has mode {mode:04o}; only the owner may read the CA key (0600), and a key \
                 others could read must be treated as burnt",
                key_path.display()
            ),
        ));
    }
    Ok(())
}

fn not_writable(dir: &Path, why: String) -> Diagnostic {
    let quoted = shell_quote(&dir.display().to_string());
    Diagnostic::builder(TLS_004, Severity::Error)
        .why(why)
        .fix(FixAction::CopyCommand(format!(
            "mkdir -p {quoted} && chmod 700 {quoted}"
        )))
        .build()
}

/// Ein Fehler an einem einzelnen Leaf, nicht an der CA.
///
/// Bewusst nicht [`corrupt`]: Scheitert das Ausstellen oder die Prüfung für
/// einen Host, liegt das am Host oder an der Prüfung, nicht an den Dateien auf
/// der Platte. Wer hier den Vorschlag `rm -r` läse, würfe eine gesunde CA weg.
fn leaf_error(host: &HostName, err: &dyn fmt::Display) -> Diagnostic {
    Diagnostic::builder(TLS_005, Severity::Error)
        .why(format!(
            "cannot issue or verify a certificate for {host}: {err}; the CA itself is intact"
        ))
        .build()
}

fn corrupt(dir: &Path, why: impl fmt::Display) -> Diagnostic {
    let quoted = shell_quote(&dir.display().to_string());
    Diagnostic::builder(TLS_005, Severity::Error)
        .why(format!(
            "{why}; removing the directory makes the next start create a fresh CA, and the \
             sandbox bundle is rebuilt from it"
        ))
        .fix(FixAction::CopyCommand(format!("rm -r {quoted}")))
        .build()
}

/// Setzt einen Pfad in einfache Anführungszeichen, wenn `sh` ihn sonst nicht
/// als ein Wort läse.
fn shell_quote(arg: &str) -> String {
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

    use std::path::Path;
    use std::sync::Arc;

    use humanitl_core::HostName;
    use time::{Duration, OffsetDateTime};

    use super::{CaStore, LeafCache, RENEW_MARGIN, pem_block, shell_quote, short_id};

    /// Ein Zertifikat, das in weniger als [`RENEW_MARGIN`] abläuft, wird beim
    /// nächsten Zugriff verworfen und neu ausgestellt. Ohne diesen Test bliebe
    /// die Erneuerung ungeprüft, bis in einem Jahr das erste Leaf abläuft.
    #[test]
    fn a_leaf_close_to_expiry_is_reissued() {
        let dir = tempfile::tempdir().unwrap();
        let ca = Arc::new(CaStore::load_or_create(dir.path()).unwrap());
        let cache = LeafCache::new(Arc::clone(&ca), 8);
        let host = HostName::parse("example.test").unwrap();

        let first = cache.server_config(&host).unwrap();
        assert!(cache.contains(&host));
        // Denselben Eintrag kurz vor den Ablauf schieben; sein `not_after`
        // liegt danach innerhalb der Frist.
        {
            let mut inner = cache.lock();
            let entry = inner.entries.get_mut(&host).unwrap();
            entry.not_after = OffsetDateTime::now_utc() + RENEW_MARGIN - Duration::minutes(1);
        }

        let second = cache.server_config(&host).unwrap();
        assert!(
            !Arc::ptr_eq(&first, &second),
            "the expiring leaf was handed out again instead of being reissued"
        );
        assert_eq!(cache.len(), 1);
        let _ = Path::new(".");
    }

    #[test]
    fn pem_block_wraps_at_64_columns() {
        let block = pem_block(&[0u8; 100]);
        let lines: Vec<&str> = block.lines().collect();
        assert_eq!(lines[0], "-----BEGIN CERTIFICATE-----");
        assert_eq!(lines[lines.len() - 1], "-----END CERTIFICATE-----");
        for line in &lines[1..lines.len() - 1] {
            assert!(line.len() <= 64, "{line}");
        }
        assert_eq!(lines[1].len(), 64);
        assert!(block.ends_with('\n'));
    }

    #[test]
    fn shell_quote_leaves_plain_paths_and_quotes_the_rest() {
        assert_eq!(shell_quote("/home/x/.local/share"), "/home/x/.local/share");
        assert_eq!(shell_quote("/tmp/a b"), "'/tmp/a b'");
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
        assert_eq!(shell_quote(""), "''");
    }

    #[test]
    fn short_id_is_eight_lowercase_hex_digits() {
        let id = short_id();
        assert_eq!(id.len(), 8);
        assert!(
            id.chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        );
        assert_eq!(id, short_id(), "the id is stable on one machine");
    }
}
