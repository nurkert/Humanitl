//! Der Updater gegen einen Fake von GitHub am Loopback (HUM-382).
//!
//! Kein Test greift ins Internet: Die Quelle zeigt auf einen axum-Server, der
//! die Antwort der Release-API, Manifest, Signatur und Datei ausliefert. Das
//! Schlüsselpaar erzeugt jeder Test selbst; der echte private Schlüssel der
//! Release-Pipeline ist nie im Repository.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::http::{StatusCode, header};
use axum::response::IntoResponse;
use axum::routing::get;
use humanitl_config::ResolverConfig;
use humanitl_core::diagnostics::codes::{UPDATE_001, UPDATE_002, UPDATE_003};
use humanitl_proxy::self_update::{MANIFEST_LINUX_ARCHIVE, MANIFEST_MACOS, Version};
use humanitl_proxy::{
    ClientTls, Direct, Resolver, ResolverPort, SelfUpdate, TrustAnchors, UpdateSource, Upstream,
};
use rcgen::{KeyPair, PKCS_ED25519, PublicKeyData as _, SigningKey as _};
use sha2::{Digest as _, Sha256};
use url::Url;

/// Was der Fake ausliefert.
struct Fake {
    tag: String,
    manifest: Vec<u8>,
    signature: Vec<u8>,
    payload: Vec<u8>,
    /// Wahr: Die Datei liegt hinter einer Umleitung, wie bei GitHub.
    redirect: bool,
    /// Ein Ziel außerhalb, auf das die Datei umgeleitet wird.
    foreign_redirect: bool,
}

fn key() -> (KeyPair, [u8; 32]) {
    let pair = KeyPair::generate_for(&PKCS_ED25519).unwrap();
    let public: [u8; 32] = pair.der_bytes().try_into().unwrap();
    (pair, public)
}

fn manifest_for(port: u16, version: &str, payload: &[u8], size: u64) -> Vec<u8> {
    let sha: [u8; 32] = Sha256::digest(payload).into();
    let hex = hex_of(&sha);
    format!(
        "{{\"version\":\"{version}\",\"url\":\"http://127.0.0.1:{port}/releases/download/v{version}/Humanitl-{version}-macos.zip\",\
         \"sha256\":\"{hex}\",\"size\":{size},\"min_os\":\"13.0\"}}"
    )
    .into_bytes()
}

fn updater(port: u16, public_key: [u8; 32]) -> SelfUpdate {
    let resolver = Arc::new(ResolverPort::from_config(&ResolverConfig::default()).unwrap());
    let upstream = Upstream::new(
        Arc::new(Direct::default()),
        resolver as Arc<dyn Resolver>,
        ClientTls::new(&TrustAnchors::bundled(), &[], false).unwrap(),
        ResolverConfig::default().prefer,
        Duration::from_secs(5),
    );
    let source = UpdateSource {
        latest_url: Url::parse(&format!("http://127.0.0.1:{port}/latest")).unwrap(),
        download_prefix: format!("http://127.0.0.1:{port}/releases/download/"),
        public_key,
        allow_private: true,
    };
    SelfUpdate::new(upstream, source, "0.0.1")
}

#[tokio::test]
async fn a_signed_release_is_found_and_downloaded_behind_a_redirect() {
    let (pair, public) = key();
    let payload = b"zip bytes of a release".repeat(1000);
    let port = fixed_fake(&pair, "0.0.9", payload.clone(), 0, true, false, false).await;
    let updater = updater(port, public);
    let release = updater.latest(MANIFEST_MACOS).await.unwrap();
    assert_eq!(release.version, Version::parse("0.0.9").unwrap());

    let dir = tempfile::tempdir().unwrap();
    let dest = dir.path().join("update.zip");
    let mut seen = 0;
    updater
        .download(&release, &dest, |done| seen = done)
        .await
        .unwrap();
    assert_eq!(std::fs::read(&dest).unwrap(), payload);
    assert_eq!(seen, payload.len() as u64);
}

#[tokio::test]
async fn a_bad_signature_refuses_before_any_download() {
    let (_pair, public) = key();
    let (other, _) = key();
    let port = fixed_fake(&other, "0.0.9", b"x".to_vec(), 0, false, false, false).await;
    let refused = updater(port, public)
        .latest(MANIFEST_MACOS)
        .await
        .unwrap_err();
    assert_eq!(refused.code, UPDATE_002);
}

#[tokio::test]
async fn a_missing_manifest_is_a_failed_check() {
    let (pair, public) = key();
    let port = fixed_fake(&pair, "0.0.9", b"x".to_vec(), 0, false, false, false).await;
    let refused = updater(port, public)
        .latest(MANIFEST_LINUX_ARCHIVE)
        .await
        .unwrap_err();
    assert_eq!(refused.code, UPDATE_001);
}

#[tokio::test]
async fn a_tag_that_differs_from_the_manifest_is_refused() {
    let (pair, public) = key();
    let port = fixed_fake(&pair, "0.0.9", b"x".to_vec(), 0, false, false, true).await;
    let refused = updater(port, public)
        .latest(MANIFEST_MACOS)
        .await
        .unwrap_err();
    assert_eq!(refused.code, UPDATE_002);
    assert!(refused.why.contains("tagged"), "{}", refused.why);
}

#[tokio::test]
async fn a_sha_or_size_mismatch_refuses_the_download() {
    let (pair, public) = key();
    // Das Manifest nennt ein Byte mehr, als der Server liefert.
    let port = fixed_fake(&pair, "0.0.9", b"payload".to_vec(), 1, false, false, false).await;
    let updater_a = updater(port, public);
    let release = updater_a.latest(MANIFEST_MACOS).await.unwrap();
    let dir = tempfile::tempdir().unwrap();
    let refused = updater_a
        .download(&release, &dir.path().join("a.zip"), |_| {})
        .await
        .unwrap_err();
    assert_eq!(refused.code, UPDATE_003);

    // Das Manifest nennt ein Byte weniger: Abbruch, sobald es mehr wird.
    let port = fixed_fake(&pair, "0.0.9", b"payload".to_vec(), -1, false, false, false).await;
    let updater_b = updater(port, public);
    let release = updater_b.latest(MANIFEST_MACOS).await.unwrap();
    let refused = updater_b
        .download(&release, &dir.path().join("b.zip"), |_| {})
        .await
        .unwrap_err();
    assert_eq!(refused.code, UPDATE_003);

    // Gleiche Größe, anderer Inhalt.
    let mut release = release;
    release.size = b"payload".len() as u64;
    release.sha256 = [0; 32];
    let refused = updater_b
        .download(&release, &dir.path().join("c.zip"), |_| {})
        .await
        .unwrap_err();
    assert_eq!(refused.code, UPDATE_003);
    assert!(refused.why.contains("SHA-256"), "{}", refused.why);
}

#[tokio::test]
async fn a_redirect_outside_github_is_refused() {
    let (pair, public) = key();
    let port = fixed_fake(&pair, "0.0.9", b"x".to_vec(), 0, false, true, false).await;
    let updater = updater(port, public);
    let release = updater.latest(MANIFEST_MACOS).await.unwrap();
    let dir = tempfile::tempdir().unwrap();
    let refused = updater
        .download(&release, &dir.path().join("x.zip"), |_| {})
        .await
        .unwrap_err();
    assert_eq!(refused.code, UPDATE_001);
    assert!(
        refused.why.contains("not a GitHub download host"),
        "{}",
        refused.why
    );
}

/// Bindet zuerst, baut dann Manifest und Fake mit genau diesem Port.
async fn fixed_fake(
    pair: &KeyPair,
    version: &str,
    payload: Vec<u8>,
    size_delta: i64,
    redirect: bool,
    foreign_redirect: bool,
    wrong_tag: bool,
) -> u16 {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let size = u64::try_from(i64::try_from(payload.len()).unwrap() + size_delta).unwrap();
    let manifest = manifest_for(port, version, &payload, size);
    let signature = pair.sign(&manifest).unwrap();
    let fake = Fake {
        tag: if wrong_tag {
            "v0.0.10".to_owned()
        } else {
            format!("v{version}")
        },
        manifest,
        signature,
        payload,
        redirect,
        foreign_redirect,
    };
    serve_listener(listener, fake);
    port
}

fn serve_listener(listener: tokio::net::TcpListener, fake: Fake) {
    let fake = Arc::new(fake);
    let port = listener.local_addr().unwrap().port();
    let base = format!("http://127.0.0.1:{port}");
    let app = router(base, &fake);
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
}

fn router(base: String, fake: &Arc<Fake>) -> Router {
    let latest = {
        let fake = Arc::clone(fake);
        move || async move {
            let dl = format!("{base}/releases/download/{}", fake.tag);
            axum::Json(serde_json::json!({
                "tag_name": fake.tag,
                "assets": [
                    {"name": MANIFEST_MACOS, "browser_download_url": format!("{dl}/{MANIFEST_MACOS}")},
                    {"name": format!("{MANIFEST_MACOS}.sig"), "browser_download_url": format!("{dl}/{MANIFEST_MACOS}.sig")},
                ]
            }))
        }
    };
    let manifest = {
        let fake = Arc::clone(fake);
        move || async move { fake.manifest.clone() }
    };
    let signature = {
        let fake = Arc::clone(fake);
        move || async move { fake.signature.clone() }
    };
    let file = {
        let fake = Arc::clone(fake);
        move || async move {
            if fake.foreign_redirect {
                return (
                    StatusCode::FOUND,
                    [(header::LOCATION, "https://evil.example/x.zip".to_owned())],
                )
                    .into_response();
            }
            if fake.redirect {
                return (
                    StatusCode::FOUND,
                    [(header::LOCATION, "/storage/blob".to_owned())],
                )
                    .into_response();
            }
            fake.payload.clone().into_response()
        }
    };
    let blob = {
        let fake = Arc::clone(fake);
        move || async move { fake.payload.clone() }
    };
    Router::new()
        .route("/latest", get(latest))
        .route(
            &format!("/releases/download/{{tag}}/{MANIFEST_MACOS}"),
            get(manifest),
        )
        .route(
            &format!("/releases/download/{{tag}}/{MANIFEST_MACOS}.sig"),
            get(signature),
        )
        .route("/releases/download/{tag}/{file}", get(file))
        .route("/storage/blob", get(blob))
}

fn hex_of(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut out, b| {
        let _ = write!(out, "{b:02x}");
        out
    })
}
