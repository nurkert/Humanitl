//! Updates von Humanitl selbst: das letzte Release, sein signiertes Manifest
//! und der Download des Zips (HUM-382).
//!
//! Das ist die eine Verbindung, die der Daemon für sich selbst aufbaut. Sie
//! geht über einen eigenen [`Upstream`] mit dem Egress `Direct`, nie über den
//! Proxy der Sandbox, und nur zu GitHub: `api.github.com` für die Frage nach
//! dem letzten Release, `github.com` für Manifest, Signatur und Zip, und von
//! dort zu dem Speicher, auf den GitHub einen Download umleitet.
//!
//! Vertraut wird keiner dieser Stellen. Vertraut wird nur dem Schlüssel, dessen
//! öffentliche Hälfte hier einkompiliert ist ([`RELEASE_KEY`]): Die
//! Release-Pipeline signiert die Bytes jedes Manifests (`update-macos.json`,
//! `update-linux-x86_64.json`, `update-linux-x86_64-appimage.json`) mit Ed25519, und
//! erst nach der Prüfung dieser Signatur liest der Daemon, was darin steht
//! (Version, Adresse, SHA-256 und Größe der Datei). Ein GitHub-Konto ohne den
//! Schlüssel kann so kein Update ausliefern, nur eines verhindern.
//!
//! Die Prüfung der Signatur nimmt den Ed25519-Algorithmus, den rustls für
//! TLS ohnehin mitbringt; es gibt hier keine eigene Kryptographie.

use std::net::IpAddr;
use std::path::Path;
use std::time::Duration;

use base64::Engine as _;
use bytes::Bytes;
use http_body_util::BodyExt as _;
use humanitl_core::diagnostics::codes::{UPDATE_001, UPDATE_002, UPDATE_003};
use humanitl_core::{
    Authority, BodyRef, Diagnostic, FixAction, HostName, HttpRequest, Method, Scheme, Severity,
};
use hyper::Response;
use hyper::body::Incoming;
use hyper::header::{HeaderName, HeaderValue, LOCATION};
use sha2::{Digest as _, Sha256};
use tokio::io::AsyncWriteExt as _;
use url::Url;

use crate::body::{self, BufferError};
use crate::upstream::Upstream;

/// Die öffentliche Hälfte des Release-Schlüssels, Base64 der rohen 32 Bytes.
///
/// Die private Hälfte liegt nur als Geheimnis `HUMANITL_UPDATE_SIGNING_KEY`
/// im Repository auf GitHub und wird nur in den beiden Signierschritten der
/// Release-Pipeline gelesen, im Linux-Job und im macOS-Job
/// (`.github/workflows/release.yml`, `packaging/release/sign-update.sh`).
pub const RELEASE_KEY: &str = "y9Z8VvO1NlbXBb+tkFbSWN+JdsPNoDv7UW2oJh5sE9s=";

/// Wo das letzte Release steht.
pub const LATEST_RELEASE_URL: &str =
    "https://api.github.com/repos/nurkert/Humanitl/releases/latest";

/// Unter diesem Präfix muss jede Datei eines Releases liegen: Manifest,
/// Signatur und das Zip, das das Manifest nennt.
pub const DOWNLOAD_PREFIX: &str = "https://github.com/nurkert/Humanitl/releases/download/";

/// Die Seite mit allen Releases, für den Download von Hand.
pub const RELEASES_PAGE: &str = "https://github.com/nurkert/Humanitl/releases";

/// Das Manifest für das macOS-Zip unter den Dateien eines Releases.
pub const MANIFEST_MACOS: &str = "update-macos.json";

/// Das Manifest für das Linux-Archiv (`tar.gz`, x86-64).
pub const MANIFEST_LINUX_ARCHIVE: &str = "update-linux-x86_64.json";

/// Das Manifest für das Linux-AppImage (x86-64).
pub const MANIFEST_LINUX_APPIMAGE: &str = "update-linux-x86_64-appimage.json";

/// Die Endung der Signatur neben jedem Manifest: die 64 Bytes der
/// Ed25519-Signatur über die Bytes des Manifests.
pub const SIGNATURE_SUFFIX: &str = ".sig";

/// Ein Name für Meldungen über das Manifest.
const MANIFEST_ASSET: &str = "the update manifest";

/// Wohin GitHub einen Download umleiten darf: nur über HTTPS und nur zu
/// diesen Namen. Was dort liegt, prüft danach ohnehin die Prüfsumme.
const REDIRECT_HOSTS: &[&str] = &[
    "github.com",
    "objects.githubusercontent.com",
    "release-assets.githubusercontent.com",
];

/// So viele Umleitungen folgt eine Anfrage höchstens.
const MAX_REDIRECTS: usize = 5;

/// Obergrenze für die Antwort auf die Frage nach dem letzten Release; sie
/// trägt die Release-Notes mit.
const RELEASE_JSON_CAP: u64 = 2 * 1024 * 1024;

/// Obergrenze für das Manifest.
const MANIFEST_CAP: u64 = 16 * 1024;

/// Obergrenze für die Signatur; gültig sind genau 64 Bytes.
const SIGNATURE_CAP: u64 = 1024;

/// Das größte Zip, das ein Manifest nennen darf.
pub const MAX_ZIP_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// So lange darf zwischen zwei Stücken einer Antwort nichts kommen.
const IDLE: Duration = Duration::from_secs(60);

/// Woher die Updates kommen und wem sie trauen.
///
/// Der Daemon nimmt [`UpdateSource::github`]. Tests setzen eine Quelle auf
/// einen Server am Loopback und einen Schlüssel, den sie selbst erzeugt haben.
#[derive(Debug, Clone)]
pub struct UpdateSource {
    /// Die Adresse, die das letzte Release als JSON der GitHub-API liefert.
    pub latest_url: Url,
    /// Unter diesem Präfix liegen Manifest, Signatur und Zip.
    pub download_prefix: String,
    /// Die öffentliche Hälfte des Release-Schlüssels, roh.
    pub public_key: [u8; 32],
    /// Erlaubt private Adressen und Umleitungen auf den Ursprung von
    /// `latest_url`. Nur für Tests gegen einen Server am Loopback.
    pub allow_private: bool,
}

impl UpdateSource {
    /// Die Quelle des Daemons: GitHub mit dem einkompilierten Schlüssel.
    ///
    /// # Errors
    ///
    /// [`Diagnostic`] mit `UPDATE_002`, wenn die einkompilierten Konstanten
    /// nicht lesbar sind; ein Test hält fest, dass das nie passiert.
    pub fn github() -> Result<Self, Diagnostic> {
        let latest_url = Url::parse(LATEST_RELEASE_URL).map_err(|err| {
            manifest_refused(format!("the built-in release address is not a URL: {err}"))
        })?;
        Ok(Self {
            latest_url,
            download_prefix: DOWNLOAD_PREFIX.to_owned(),
            public_key: decode_key(RELEASE_KEY)?,
            allow_private: false,
        })
    }
}

/// Liest einen öffentlichen Schlüssel aus Base64 der rohen 32 Bytes.
///
/// # Errors
///
/// [`Diagnostic`] mit `UPDATE_002`, wenn der Text kein Base64 ist oder nicht
/// genau 32 Bytes trägt.
pub fn decode_key(text: &str) -> Result<[u8; 32], Diagnostic> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(text.trim())
        .map_err(|err| manifest_refused(format!("the release key is not base64: {err}")))?;
    <[u8; 32]>::try_from(bytes.as_slice()).map_err(|_len| {
        manifest_refused(format!(
            "the release key has {} bytes, an Ed25519 key has 32",
            bytes.len()
        ))
    })
}

/// Eine Version der Form `MAJOR.MINOR.PATCH`, nur Ziffern.
///
/// Mehr braucht es nicht: Humanitl veröffentlicht `0.0.N` und später
/// `0.N.M`. Eine Vorabversion mit Bindestrich ist kein Update.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    /// Hauptversion.
    pub major: u64,
    /// Nebenversion.
    pub minor: u64,
    /// Patch-Stand.
    pub patch: u64,
}

impl Version {
    /// Liest `1.2.3`; ein führendes `v` wie im Tag-Namen ist erlaubt.
    ///
    /// `None` für alles andere: weniger oder mehr als drei Teile, ein Teil
    /// ohne Ziffern, mit Vorzeichen oder Leerraum, führende Nullen außer `0`
    /// selbst, oder eine Zahl über `u64`.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.strip_prefix('v').unwrap_or(text);
        let mut parts = text.split('.');
        let mut next = || -> Option<u64> {
            let part = parts.next()?;
            if part.is_empty()
                || !part.bytes().all(|b| b.is_ascii_digit())
                || (part.len() > 1 && part.starts_with('0'))
            {
                return None;
            }
            part.parse().ok()
        };
        let version = Self {
            major: next()?,
            minor: next()?,
            patch: next()?,
        };
        if parts.next().is_some() {
            return None;
        }
        Some(version)
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Was ein geprüftes Manifest über ein Release sagt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    /// Die Version des Releases.
    pub version: Version,
    /// Die Adresse des Zips, unter [`UpdateSource::download_prefix`].
    pub url: Url,
    /// Der SHA-256 des Zips.
    pub sha256: [u8; 32],
    /// Die Größe des Zips in Bytes.
    pub size: u64,
    /// Die kleinste macOS-Version, auf der das Release läuft, etwa `13.0`.
    pub min_os: String,
}

/// Prüft die Signatur des Manifests und liest es danach.
///
/// Die Signatur gilt den Bytes, wie sie geladen wurden; gelesen wird erst
/// nach der Prüfung, damit nichts Unsigniertes eine Entscheidung beeinflusst.
///
/// # Errors
///
/// [`Diagnostic`] mit `UPDATE_002`, wenn die Signatur nicht 64 Bytes hat oder
/// nicht zu `public_key` passt, das Manifest kein JSON-Objekt mit `version`,
/// `url`, `sha256`, `size` und `min_os` ist, eine Version nicht der Form
/// `MAJOR.MINOR.PATCH` folgt, die Größe 0 oder über [`MAX_ZIP_BYTES`] liegt,
/// oder die Adresse nicht unter `download_prefix` liegt.
pub fn verify_manifest(
    manifest: &[u8],
    signature: &[u8],
    public_key: &[u8; 32],
    download_prefix: &str,
) -> Result<Release, Diagnostic> {
    if signature.len() != 64 {
        return Err(manifest_refused(format!(
            "the signature of {MANIFEST_ASSET} has {} bytes, an Ed25519 signature has 64",
            signature.len()
        )));
    }
    if !ed25519_valid(public_key, manifest, signature) {
        return Err(manifest_refused(format!(
            "the signature of {MANIFEST_ASSET} does not match the Humanitl release key, so \
             nothing in it is trusted"
        )));
    }
    parse_manifest(manifest, download_prefix)
}

/// Liest ein Manifest, dessen Signatur schon geprüft ist.
fn parse_manifest(manifest: &[u8], download_prefix: &str) -> Result<Release, Diagnostic> {
    let value: serde_json::Value = serde_json::from_slice(manifest)
        .map_err(|err| manifest_refused(format!("{MANIFEST_ASSET} is not JSON: {err}")))?;
    let field = |name: &str| -> Result<&serde_json::Value, Diagnostic> {
        value
            .get(name)
            .ok_or_else(|| manifest_refused(format!("{MANIFEST_ASSET} has no field {name:?}")))
    };
    let text = |name: &str| -> Result<&str, Diagnostic> {
        field(name)?.as_str().ok_or_else(|| {
            manifest_refused(format!(
                "the field {name:?} of {MANIFEST_ASSET} is not text"
            ))
        })
    };

    let version_text = text("version")?;
    let version = Version::parse(version_text).ok_or_else(|| {
        manifest_refused(format!(
            "{MANIFEST_ASSET} names the version {version_text:?}, not one of the form 1.2.3"
        ))
    })?;
    let url = allowed_download(text("url")?, download_prefix)?;
    let sha_text = text("sha256")?;
    let sha256 = parse_sha256(sha_text).ok_or_else(|| {
        manifest_refused(format!(
            "the sha256 of {MANIFEST_ASSET} is not 64 hexadecimal characters"
        ))
    })?;
    let size = field("size")?
        .as_u64()
        .filter(|size| (1..=MAX_ZIP_BYTES).contains(size))
        .ok_or_else(|| {
            manifest_refused(format!(
                "the size in {MANIFEST_ASSET} is not a whole number between 1 and \
                 {MAX_ZIP_BYTES}"
            ))
        })?;
    let min_os = text("min_os")?;
    if min_os.is_empty()
        || min_os.len() > 16
        || !min_os.split('.').all(|part| {
            !part.is_empty() && part.len() <= 4 && part.bytes().all(|b| b.is_ascii_digit())
        })
    {
        return Err(manifest_refused(format!(
            "{MANIFEST_ASSET} names the minimum macOS {min_os:?}, not one of the form 13.0"
        )));
    }
    Ok(Release {
        version,
        url,
        sha256,
        size,
        min_os: min_os.to_owned(),
    })
}

/// Prüft eine Adresse gegen das Präfix der Release-Dateien.
///
/// Verglichen wird die Adresse, wie `url` sie liest: `..`-Segmente sind dann
/// schon aufgelöst, und `https://github.com/nurkert/Humanitl/releases/download/../../x`
/// liegt nicht mehr unter dem Präfix. Zugangsdaten in der Adresse und ein
/// Fragment werden abgelehnt.
///
/// # Errors
///
/// [`Diagnostic`] mit `UPDATE_002`, wenn die Adresse nicht lesbar ist oder
/// nicht unter `prefix` liegt.
pub fn allowed_download(text: &str, prefix: &str) -> Result<Url, Diagnostic> {
    let outside = || {
        manifest_refused(format!(
            "{text:?} lies outside {prefix}, where every Humanitl release lives"
        ))
    };
    let url = Url::parse(text).map_err(|_err| outside())?;
    if !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || url.query().is_some()
        || !url.as_str().starts_with(prefix)
        || url.as_str().len() == prefix.len()
    {
        return Err(outside());
    }
    Ok(url)
}

/// Der Dateiname, den ein Manifest dieser Art für `version` nennen muss;
/// `None` für einen Manifest-Namen, den der Daemon nicht kennt.
#[must_use]
pub fn expected_file(manifest_name: &str, version: Version) -> Option<String> {
    match manifest_name {
        MANIFEST_MACOS => Some(format!("Humanitl-{version}-macos.zip")),
        MANIFEST_LINUX_ARCHIVE => Some(format!("humanitl-{version}-linux-x86_64.tar.gz")),
        MANIFEST_LINUX_APPIMAGE => Some(format!("Humanitl-{version}-x86_64.AppImage")),
        _ => None,
    }
}

/// Bindet ein geprüftes Manifest an seine Art: Die Adresse muss genau
/// `<prefix>v<version>/<datei>` sein, mit dem Dateinamen aus
/// [`expected_file`]. Ein gültig signiertes Manifest des Zips taugt so nicht
/// als Manifest des `AppImage`s, und keines zeigt auf die Datei eines anderen
/// Releases.
///
/// # Errors
///
/// `UPDATE_002`, wenn Art oder Adresse nicht passen.
pub fn bound_to_kind(
    release: &Release,
    manifest_name: &str,
    prefix: &str,
) -> Result<(), Diagnostic> {
    let file = expected_file(manifest_name, release.version).ok_or_else(|| {
        manifest_refused(format!(
            "{manifest_name} is not a manifest this daemon knows"
        ))
    })?;
    let expected = format!("{prefix}v{}/{file}", release.version);
    if release.url.as_str() != expected {
        return Err(manifest_refused(format!(
            "{manifest_name} names {}, but a manifest of this kind must name {expected}",
            release.url
        )));
    }
    Ok(())
}

/// Liest 64 Hexadezimalzeichen als SHA-256.
fn parse_sha256(text: &str) -> Option<[u8; 32]> {
    if text.len() != 64 {
        return None;
    }
    let mut out = [0_u8; 32];
    for (index, chunk) in text.as_bytes().chunks(2).enumerate() {
        let pair = std::str::from_utf8(chunk).ok()?;
        *out.get_mut(index)? = u8::from_str_radix(pair, 16).ok()?;
    }
    Some(out)
}

/// Prüft eine Ed25519-Signatur mit dem Algorithmus aus dem Krypto-Anbieter
/// von rustls (ring), demselben, der die TLS-Verbindungen des Daemons prüft.
#[must_use]
pub fn ed25519_valid(public_key: &[u8; 32], message: &[u8], signature: &[u8]) -> bool {
    let provider = rustls::crypto::ring::default_provider();
    provider
        .signature_verification_algorithms
        .mapping
        .iter()
        .find(|(scheme, _)| *scheme == rustls::SignatureScheme::ED25519)
        .and_then(|(_, algorithms)| algorithms.first())
        .is_some_and(|algorithm| {
            algorithm
                .verify_signature(public_key, message, signature)
                .is_ok()
        })
}

/// Fragt GitHub nach dem letzten Release und lädt es.
pub struct SelfUpdate {
    upstream: Upstream,
    source: UpdateSource,
    user_agent: HeaderValue,
    current: Option<Version>,
}

impl std::fmt::Debug for SelfUpdate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SelfUpdate")
            .field("source", &self.source)
            .finish_non_exhaustive()
    }
}

impl SelfUpdate {
    /// Ein Updater über diesem Weg nach draußen.
    ///
    /// `version` steht in der Kopfzeile `User-Agent`, die die GitHub-API
    /// verlangt; mehr schickt keine Anfrage mit.
    #[must_use]
    pub fn new(upstream: Upstream, source: UpdateSource, version: &str) -> Self {
        let user_agent = HeaderValue::from_str(&format!("Humanitl/{version}"))
            .unwrap_or_else(|_invalid| HeaderValue::from_static("Humanitl"));
        Self {
            upstream,
            source,
            user_agent,
            current: Version::parse(version),
        }
    }

    /// Ob `release` neuer ist als die Version dieses Daemons.
    ///
    /// Eine Version, die nicht der Form `1.2.3` folgt, gilt als älter als
    /// jedes Release; ein gleiches oder älteres Release ist nie ein Update.
    #[must_use]
    pub fn newer(&self, release: &Release) -> bool {
        self.current.is_none_or(|current| release.version > current)
    }

    /// Das letzte Release, mit geprüftem Manifest.
    ///
    /// Fragt die API nach dem letzten Release, sucht darin das Manifest
    /// `manifest` (etwa [`MANIFEST_MACOS`]) und seine Signatur mit
    /// [`SIGNATURE_SUFFIX`], lädt beide von Adressen unter dem Präfix und
    /// prüft die Signatur. Die Version im Manifest muss die des Tags sein:
    /// Ein altes, gültig signiertes Manifest unter einem neuen Tag wird
    /// abgelehnt, statt still eine andere Version anzubieten.
    ///
    /// # Errors
    ///
    /// [`Diagnostic`] mit `UPDATE_001`, wenn eine Anfrage scheitert, die
    /// Antwort kein Release ist oder Manifest und Signatur fehlen; mit
    /// `UPDATE_002`, wenn [`verify_manifest`] ablehnt oder Tag und Manifest
    /// verschiedene Versionen nennen.
    pub async fn latest(&self, manifest_name: &str) -> Result<Release, Diagnostic> {
        let latest = self.source.latest_url.clone();
        let json = self
            .get_bytes(&latest, RELEASE_JSON_CAP, "application/vnd.github+json")
            .await?;
        let value: serde_json::Value = serde_json::from_slice(&json).map_err(|err| {
            check_failed(format!(
                "GitHub's answer about the latest release is not JSON: {err}"
            ))
        })?;
        let tag = value
            .get("tag_name")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| check_failed("GitHub's answer names no release tag".to_owned()))?;
        let asset = |name: &str| -> Result<Url, Diagnostic> {
            let found = value
                .get("assets")
                .and_then(serde_json::Value::as_array)
                .and_then(|assets| {
                    assets.iter().find(|asset| {
                        asset.get("name").and_then(serde_json::Value::as_str) == Some(name)
                    })
                })
                .and_then(|asset| asset.get("browser_download_url"))
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| {
                    check_failed(format!(
                        "the release {tag} carries no {name}; it cannot be installed from inside \
                         the app"
                    ))
                })?;
            allowed_download(found, &self.source.download_prefix)
        };
        let manifest_url = asset(manifest_name)?;
        let signature_url = asset(&format!("{manifest_name}{SIGNATURE_SUFFIX}"))?;
        let manifest = self
            .get_bytes(&manifest_url, MANIFEST_CAP, "application/octet-stream")
            .await?;
        let signature = self
            .get_bytes(&signature_url, SIGNATURE_CAP, "application/octet-stream")
            .await?;
        let release = verify_manifest(
            &manifest,
            &signature,
            &self.source.public_key,
            &self.source.download_prefix,
        )?;
        if Version::parse(tag) != Some(release.version) {
            return Err(manifest_refused(format!(
                "the release is tagged {tag:?}, but its signed manifest names {}; nothing is \
                 offered",
                release.version
            )));
        }
        bound_to_kind(&release, manifest_name, &self.source.download_prefix)?;
        Ok(release)
    }

    /// Lädt das Zip von `release` nach `dest` und prüft Größe und SHA-256.
    ///
    /// Gezählt und gehasht wird beim Schreiben: Mehr Bytes als angekündigt
    /// brechen sofort ab. `progress` bekommt nach jedem Stück die Zahl der
    /// bisher geschriebenen Bytes. Die Datei wird neu angelegt, nie
    /// überschrieben; bei einem Fehler bleibt sie liegen, und der Aufrufer
    /// räumt sein Verzeichnis weg.
    ///
    /// # Errors
    ///
    /// [`Diagnostic`] mit `UPDATE_001`, wenn die Anfrage scheitert oder der
    /// Strom abreißt; mit `UPDATE_003`, wenn Größe oder SHA-256 nicht
    /// stimmen oder die Datei nicht geschrieben werden kann.
    pub async fn download(
        &self,
        release: &Release,
        dest: &Path,
        mut progress: impl FnMut(u64) + Send,
    ) -> Result<(), Diagnostic> {
        allowed_download(release.url.as_str(), &self.source.download_prefix)?;
        let response = self.get(&release.url, "application/octet-stream").await?;
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(dest)
            .await
            .map_err(|err| {
                download_refused(format!("{} could not be created: {err}", dest.display()))
            })?;
        let mut body = response.into_body();
        let mut hasher = Sha256::new();
        let mut written: u64 = 0;
        loop {
            let Ok(next) = tokio::time::timeout(IDLE, body.frame()).await else {
                return Err(check_failed(format!(
                    "the download of {} stalled for {} s",
                    release.url,
                    IDLE.as_secs()
                )));
            };
            let Some(frame) = next else { break };
            let frame = frame.map_err(|err| {
                check_failed(format!("the download of {} broke off: {err}", release.url))
            })?;
            let Some(data) = frame.data_ref() else {
                continue;
            };
            written = written.saturating_add(data.len() as u64);
            if written > release.size {
                return Err(download_refused(format!(
                    "the download is larger than the {} bytes the signed manifest names",
                    release.size
                )));
            }
            hasher.update(data);
            file.write_all(data).await.map_err(|err| {
                download_refused(format!("{} could not be written: {err}", dest.display()))
            })?;
            progress(written);
        }
        file.flush().await.map_err(|err| {
            download_refused(format!("{} could not be written: {err}", dest.display()))
        })?;
        file.sync_all().await.map_err(|err| {
            download_refused(format!("{} could not be written: {err}", dest.display()))
        })?;
        if written != release.size {
            return Err(download_refused(format!(
                "the download has {written} bytes, the signed manifest names {}",
                release.size
            )));
        }
        let digest: [u8; 32] = hasher.finalize().into();
        if digest != release.sha256 {
            return Err(download_refused(
                "the SHA-256 of the download differs from the one in the signed manifest"
                    .to_owned(),
            ));
        }
        Ok(())
    }

    /// Ein `GET` mit gelesenem Körper, höchstens `cap` Bytes.
    async fn get_bytes(
        &self,
        url: &Url,
        cap: u64,
        accept: &'static str,
    ) -> Result<Bytes, Diagnostic> {
        let response = self.get(url, accept).await?;
        body::buffer(response.into_body(), cap, IDLE)
            .await
            .map_err(|error| {
                check_failed(match error {
                    BufferError::Cap => format!("{url} answered with more than {cap} bytes"),
                    BufferError::Idle => format!("{url} stalled for {} s", IDLE.as_secs()),
                    BufferError::Read | BufferError::Budget => {
                        format!("the answer of {url} broke off")
                    }
                })
            })
    }

    /// Ein `GET`, das Umleitungen folgt, solange sie erlaubt sind; die
    /// Antwort ist eine mit Status `200`.
    async fn get(&self, url: &Url, accept: &'static str) -> Result<Response<Incoming>, Diagnostic> {
        let mut current = url.clone();
        for _ in 0..=MAX_REDIRECTS {
            let request = self.request(&current, accept)?;
            let response = self
                .upstream
                .forward(&request, Bytes::new(), self.source.allow_private)
                .await
                .map_err(|err| check_failed(format!("{current} did not answer: {err}")))?;
            let status = response.status().as_u16();
            match status {
                200 => return Ok(response),
                301 | 302 | 303 | 307 | 308 => {
                    let location = response
                        .headers()
                        .get(LOCATION)
                        .and_then(|value| value.to_str().ok())
                        .ok_or_else(|| {
                            check_failed(format!("{current} redirected without a location"))
                        })?;
                    let next = current.join(location).map_err(|_err| {
                        check_failed(format!("{current} redirected to an unreadable location"))
                    })?;
                    if !self.redirect_allowed(&next) {
                        return Err(check_failed(format!(
                            "{current} redirected to {}, which is not a GitHub download host",
                            next.origin().ascii_serialization()
                        )));
                    }
                    current = next;
                }
                other => {
                    return Err(check_failed(format!(
                        "{current} answered with status {other}"
                    )));
                }
            }
        }
        Err(check_failed(format!(
            "{url} redirected more than {MAX_REDIRECTS} times"
        )))
    }

    /// Ob eine Umleitung nach `next` erlaubt ist: HTTPS zu einem der
    /// [`REDIRECT_HOSTS`], im Test auch der Ursprung der Quelle.
    fn redirect_allowed(&self, next: &Url) -> bool {
        if self.source.allow_private && next.origin() == self.source.latest_url.origin() {
            return true;
        }
        next.scheme() == "https"
            && next.port().is_none()
            && next.username().is_empty()
            && next.password().is_none()
            && next
                .host_str()
                .is_some_and(|host| REDIRECT_HOSTS.contains(&host))
    }

    /// Baut die Anfrage für `url`, mit `User-Agent` und `Accept`, sonst nichts.
    fn request(&self, url: &Url, accept: &'static str) -> Result<HttpRequest, Diagnostic> {
        let scheme = match url.scheme() {
            "https" => Scheme::Https,
            "http" if self.source.allow_private => Scheme::Http,
            other => {
                return Err(check_failed(format!(
                    "{url} uses the scheme {other:?}; updates come over https only"
                )));
            }
        };
        let host = url
            .host_str()
            .map(|host| host.trim_start_matches('[').trim_end_matches(']'))
            .and_then(|host| {
                host.parse::<IpAddr>()
                    .ok()
                    .map(HostName::Ip)
                    .or_else(|| HostName::parse(host).ok())
            })
            .ok_or_else(|| check_failed(format!("{url} has no host Humanitl can read")))?;
        let port = url.port_or_known_default().unwrap_or(443);
        let mut path = url.path().to_owned();
        if let Some(query) = url.query() {
            path.push('?');
            path.push_str(query);
        }
        let mut request = HttpRequest::new(Method::GET, scheme, Authority::new(host, port), path);
        request.headers.insert(
            HeaderName::from_static("accept"),
            HeaderValue::from_static(accept),
        );
        request.headers.insert(
            HeaderName::from_static("user-agent"),
            self.user_agent.clone(),
        );
        request.body = BodyRef::empty();
        Ok(request)
    }
}

/// Der Verweis auf die Release-Seite, an jedem Befund dieses Moduls.
fn releases_page() -> FixAction {
    FixAction::OpenUrl(RELEASES_PAGE.to_owned())
}

/// `UPDATE_001`: Die Frage nach dem letzten Release oder der Download ist
/// gescheitert.
fn check_failed(why: String) -> Diagnostic {
    Diagnostic::builder(UPDATE_001, Severity::Error)
        .why(why)
        .fix(releases_page())
        .build()
}

/// `UPDATE_002`: Das Manifest wird nicht angenommen.
fn manifest_refused(why: String) -> Diagnostic {
    Diagnostic::builder(UPDATE_002, Severity::Error)
        .why(why)
        .fix(releases_page())
        .build()
}

/// `UPDATE_003`: Der Download passt nicht zum Manifest.
fn download_refused(why: String) -> Diagnostic {
    Diagnostic::builder(UPDATE_003, Severity::Error)
        .why(why)
        .fix(releases_page())
        .build()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use rcgen::{KeyPair, PKCS_ED25519, PublicKeyData as _, SigningKey as _};

    use super::*;

    /// Ein Schlüsselpaar nur für diesen Test; der echte private Schlüssel
    /// ist nie im Repository.
    fn test_key() -> (KeyPair, [u8; 32]) {
        let pair = KeyPair::generate_for(&PKCS_ED25519).unwrap();
        let public: [u8; 32] = pair.der_bytes().try_into().unwrap();
        (pair, public)
    }

    fn manifest(version: &str, url: &str) -> Vec<u8> {
        format!(
            "{{\"version\":\"{version}\",\"url\":\"{url}\",\"sha256\":\"{}\",\"size\":1234,\
             \"min_os\":\"13.0\"}}",
            "ab".repeat(32)
        )
        .into_bytes()
    }

    const ZIP: &str =
        "https://github.com/nurkert/Humanitl/releases/download/v0.0.9/Humanitl-0.0.9-macos.zip";

    #[test]
    fn the_built_in_key_and_addresses_are_readable() {
        let source = UpdateSource::github().unwrap();
        assert_eq!(source.public_key.len(), 32);
        assert!(!source.allow_private);
        assert!(
            source
                .latest_url
                .as_str()
                .starts_with("https://api.github.com/")
        );
    }

    #[test]
    fn a_good_signature_yields_the_release() {
        let (pair, public) = test_key();
        let bytes = manifest("0.0.9", ZIP);
        let signature = pair.sign(&bytes).unwrap();
        let release = verify_manifest(&bytes, &signature, &public, DOWNLOAD_PREFIX).unwrap();
        assert_eq!(release.version, Version::parse("0.0.9").unwrap());
        assert_eq!(release.url.as_str(), ZIP);
        assert_eq!(release.sha256, [0xab; 32]);
        assert_eq!(release.size, 1234);
        assert_eq!(release.min_os, "13.0");
    }

    #[test]
    fn a_bad_signature_is_refused() {
        let (pair, public) = test_key();
        let bytes = manifest("0.0.9", ZIP);
        let signature = pair.sign(&bytes).unwrap();

        // Ein Bit im Manifest gekippt.
        let mut tampered = bytes.clone();
        tampered[12] ^= 1;
        let refused = verify_manifest(&tampered, &signature, &public, DOWNLOAD_PREFIX).unwrap_err();
        assert_eq!(refused.code, UPDATE_002);
        assert!(refused.why.contains("does not match"), "{}", refused.why);

        // Ein anderer Schlüssel.
        let (_other, other_public) = test_key();
        let refused =
            verify_manifest(&bytes, &signature, &other_public, DOWNLOAD_PREFIX).unwrap_err();
        assert_eq!(refused.code, UPDATE_002);

        // Keine Signatur, eine zu kurze, eine aus Nullen.
        for signature in [Vec::new(), vec![0_u8; 63], vec![0_u8; 64]] {
            let refused =
                verify_manifest(&bytes, &signature, &public, DOWNLOAD_PREFIX).unwrap_err();
            assert_eq!(refused.code, UPDATE_002);
        }
    }

    #[test]
    fn a_signed_manifest_outside_the_allowlist_is_refused() {
        let (pair, public) = test_key();
        for url in [
            "https://github.com/nurkert/Humanitl/releases/download/../../evil/x.zip",
            "https://github.com/other/Humanitl/releases/download/v0.0.9/x.zip",
            "http://github.com/nurkert/Humanitl/releases/download/v0.0.9/x.zip",
            "https://github.com.evil.example/nurkert/Humanitl/releases/download/v0.0.9/x.zip",
            "https://user@github.com/nurkert/Humanitl/releases/download/v0.0.9/x.zip",
            "https://github.com/nurkert/Humanitl/releases/download/",
            "https://github.com/nurkert/Humanitl/releases/download/v0.0.9/x.zip?x=1",
            "not a url",
        ] {
            let bytes = manifest("0.0.9", url);
            let signature = pair.sign(&bytes).unwrap();
            let refused =
                verify_manifest(&bytes, &signature, &public, DOWNLOAD_PREFIX).expect_err(url);
            assert_eq!(refused.code, UPDATE_002, "{url}");
        }
    }

    #[test]
    fn a_signed_manifest_with_broken_fields_is_refused() {
        let (pair, public) = test_key();
        let sha = "ab".repeat(32);
        for text in [
            "[]".to_owned(),
            "{}".to_owned(),
            format!(
                r#"{{"version":"0.0","url":"{ZIP}","sha256":"{sha}","size":1,"min_os":"13.0"}}"#
            ),
            format!(
                r#"{{"version":"0.0.9","url":"{ZIP}","sha256":"zz","size":1,"min_os":"13.0"}}"#
            ),
            format!(
                r#"{{"version":"0.0.9","url":"{ZIP}","sha256":"{sha}","size":0,"min_os":"13.0"}}"#
            ),
            format!(
                r#"{{"version":"0.0.9","url":"{ZIP}","sha256":"{sha}","size":-1,"min_os":"13.0"}}"#
            ),
            format!(
                r#"{{"version":"0.0.9","url":"{ZIP}","sha256":"{sha}","size":1,"min_os":"x"}}"#
            ),
            format!(r#"{{"version":"0.0.9","url":"{ZIP}","sha256":"{sha}","size":1}}"#),
        ] {
            let signature = pair.sign(text.as_bytes()).unwrap();
            let refused = verify_manifest(text.as_bytes(), &signature, &public, DOWNLOAD_PREFIX)
                .expect_err(&text);
            assert_eq!(refused.code, UPDATE_002, "{text}");
        }
    }

    /// Eine Signatur, wie `packaging/release/sign-update.sh` sie mit
    /// `openssl pkeyutl -sign -rawin` schreibt, mit einem Wegwerf-Schlüssel
    /// erzeugt, dessen private Hälfte gelöscht ist: Der Daemon nimmt das
    /// Format der Pipeline an, Byte für Byte.
    #[test]
    fn a_signature_made_by_openssl_is_accepted() {
        let public = decode_key("2IHgvS8p344lALdDQLlRnLG8zdLIDOfG58iMQZOsDYo=").unwrap();
        let manifest = concat!(
            r#"{"version":"0.0.7","url":"https://github.com/nurkert/Humanitl/releases/download/v0.0.7/Humanitl-0.0.7-macos.zip","#,
            r#""sha256":"d4e4877bac978b7952f0d544fc52ebff5411d351d129f1f056fa43f11da9af2b","size":8,"min_os":"12.0"}"#,
            "\n"
        );
        let signature = base64::engine::general_purpose::STANDARD
            .decode("kL28g7q0eocUsnr9aEnZNd5QZySPb5RfOj8iPsXBj4a3XKd5WTKZ/3zZ92VYFaaK52qunPVQK9Y/p8zwsLAjDQ==")
            .unwrap();
        let release =
            verify_manifest(manifest.as_bytes(), &signature, &public, DOWNLOAD_PREFIX).unwrap();
        assert_eq!(release.size, 8);
        assert_eq!(release.min_os, "12.0");
    }

    #[test]
    fn a_manifest_is_bound_to_its_kind() {
        let (pair, public) = test_key();
        let bytes = manifest("0.0.9", ZIP);
        let signature = pair.sign(&bytes).unwrap();
        let release = verify_manifest(&bytes, &signature, &public, DOWNLOAD_PREFIX).unwrap();
        bound_to_kind(&release, MANIFEST_MACOS, DOWNLOAD_PREFIX).unwrap();
        for other in [
            MANIFEST_LINUX_ARCHIVE,
            MANIFEST_LINUX_APPIMAGE,
            "update-x.json",
        ] {
            assert_eq!(
                bound_to_kind(&release, other, DOWNLOAD_PREFIX)
                    .unwrap_err()
                    .code,
                UPDATE_002,
                "{other}"
            );
        }
        // Die Datei eines anderen Releases unter einem gültigen Manifest.
        let elsewhere = manifest(
            "0.0.9",
            "https://github.com/nurkert/Humanitl/releases/download/v0.0.8/Humanitl-0.0.9-macos.zip",
        );
        let signature = pair.sign(&elsewhere).unwrap();
        let release = verify_manifest(&elsewhere, &signature, &public, DOWNLOAD_PREFIX).unwrap();
        assert!(bound_to_kind(&release, MANIFEST_MACOS, DOWNLOAD_PREFIX).is_err());
    }

    #[test]
    fn versions_compare_by_number() {
        let v = |text| Version::parse(text).unwrap();
        assert!(v("0.0.10") > v("0.0.9"));
        assert!(v("0.1.0") > v("0.0.99"));
        assert!(v("1.0.0") > v("0.99.99"));
        assert_eq!(v("v0.0.3"), v("0.0.3"));
        assert_eq!(v("0.0.3").to_string(), "0.0.3");
        for bad in [
            "",
            "0.0",
            "0.0.0.0",
            "0.0.x",
            "0.0.-1",
            "0.0.01",
            " 0.0.1",
            "0.0.1-rc1",
            "+1.0.0",
        ] {
            assert_eq!(Version::parse(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn sha256_hex_reads_both_cases() {
        assert_eq!(parse_sha256(&"AB".repeat(32)), Some([0xab; 32]));
        assert_eq!(parse_sha256(&"ab".repeat(31)), None);
        assert_eq!(parse_sha256(&format!("{}g0", "ab".repeat(31))), None);
    }
}
