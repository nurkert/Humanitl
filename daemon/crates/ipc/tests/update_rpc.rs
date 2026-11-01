//! Der Update-RPC gegen einen Fake von GitHub am Loopback (HUM-382).
//!
//! Kein Test greift ins Internet und keiner installiert etwas außerhalb
//! eines Wegwerf-Verzeichnisses. Der Fake ist ein kleiner HTTP/1.1-Server
//! auf `127.0.0.1`, der zählt, wie oft er gefragt wurde. Gültig signierte
//! Manifeste prüfen die Tests des Proxy-Crates (`tests/self_update.rs`);
//! hier geht es darum, wann der RPC fragt und was er ablehnt.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use humanitl_config::Config;
use humanitl_core::SessionId;
use humanitl_ipc::server::IpcServer;
use humanitl_ipc::v1;
use humanitl_ipc::v1::humanitl_server::Humanitl as _;
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio_stream::StreamExt as _;
use tonic::Request;
use url::Url;

/// Ein Server, der auf jede Frage nach `/latest` ein Release mit Manifest
/// und einer falschen Signatur nennt.
async fn fake_github() -> (u16, Arc<AtomicUsize>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let hits = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&hits);
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                return;
            };
            counter.fetch_add(1, Ordering::SeqCst);
            tokio::spawn(async move {
                let mut buf = vec![0_u8; 4096];
                let n = stream.read(&mut buf).await.unwrap_or(0);
                let head = String::from_utf8_lossy(&buf[..n]).into_owned();
                let path = head.split_whitespace().nth(1).unwrap_or("/").to_owned();
                let dl = format!("http://127.0.0.1:{port}/releases/download/v0.0.999");
                let body: Vec<u8> = match path.as_str() {
                    "/latest" => format!(
                        "{{\"tag_name\":\"v0.0.999\",\"assets\":[\
                         {{\"name\":\"update-linux-x86_64.json\",\"browser_download_url\":\"{dl}/update-linux-x86_64.json\"}},\
                         {{\"name\":\"update-linux-x86_64.json.sig\",\"browser_download_url\":\"{dl}/update-linux-x86_64.json.sig\"}},\
                         {{\"name\":\"update-macos.json\",\"browser_download_url\":\"{dl}/update-macos.json\"}},\
                         {{\"name\":\"update-macos.json.sig\",\"browser_download_url\":\"{dl}/update-macos.json.sig\"}}]}}"
                    )
                    .into_bytes(),
                    p if p.rsplit_once('.').is_some_and(|(_, ext)| ext == "sig") => vec![7_u8; 64],
                    _ => format!(
                        "{{\"version\":\"0.0.999\",\"url\":\"{dl}/x.zip\",\"sha256\":\"{}\",\"size\":1,\"min_os\":\"13.0\"}}",
                        "00".repeat(32)
                    )
                    .into_bytes(),
                };
                let head = format!(
                    "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(head.as_bytes()).await;
                let _ = stream.write_all(&body).await;
                let _ = stream.shutdown().await;
            });
        }
    });
    (port, hits)
}

/// Ein Dienst über Wegwerf-Pfaden, dessen Updater den Fake fragt.
fn server(dir: &Path, port: u16, exe: PathBuf, root: PathBuf, config_toml: &str) -> IpcServer {
    let config = Config::default();
    let paths = humanitl_config::Paths::new(humanitl_config::Env::from_pairs([
        ("HOME", dir.display().to_string()),
        ("XDG_CONFIG_HOME", dir.join("config").display().to_string()),
        ("XDG_DATA_HOME", dir.join("data").display().to_string()),
        ("XDG_RUNTIME_DIR", dir.join("run").display().to_string()),
    ]));
    std::fs::create_dir_all(dir.join("config/humanitl")).unwrap();
    std::fs::write(dir.join("config/humanitl/config.toml"), config_toml).unwrap();
    IpcServer::over_the_recording(&config, Some(SessionId::new()))
        .with_paths(paths)
        .with_update_source(
            &config,
            Url::parse(&format!("http://127.0.0.1:{port}/latest")).unwrap(),
            format!("http://127.0.0.1:{port}/releases/download/"),
            [1_u8; 32],
            exe,
            root,
        )
        .unwrap()
}

async fn check(server: &IpcServer, force: bool) -> Result<v1::UpdateStatus, tonic::Status> {
    let request = v1::UpdateRequest {
        op: Some(v1::update_request::Op::Check(v1::update_request::Check {
            force,
            appimage: String::new(),
        })),
    };
    let mut stream = server.update(Request::new(request)).await?.into_inner();
    let event = stream.next().await.unwrap()?;
    match event.event {
        Some(v1::update_event::Event::Status(status)) => Ok(status),
        other => panic!("expected a status, got {other:?}"),
    }
}

fn code(status: &tonic::Status) -> String {
    humanitl_ipc::server_stub::diagnostic_from_status(status)
        .expect("the status carries a diagnostic")
        .code
}

#[tokio::test]
async fn without_force_and_with_update_check_off_nothing_is_asked() {
    let dir = tempfile::tempdir().unwrap();
    let (port, hits) = fake_github().await;
    let exe = dir.path().join("dev/target/debug/humanitld");
    let server = server(
        dir.path(),
        port,
        exe,
        dir.path().join("root"),
        "[update]\ncheck = false\n",
    );
    let status = check(&server, false).await.unwrap();
    assert!(!status.available);
    assert!(status.latest_version.is_empty());
    assert_eq!(hits.load(Ordering::SeqCst), 0, "the network was used");
}

#[tokio::test]
async fn a_forced_check_with_a_bad_signature_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let (port, hits) = fake_github().await;
    let exe = dir.path().join("dev/target/debug/humanitld");
    let server = server(
        dir.path(),
        port,
        exe,
        dir.path().join("root"),
        "[update]\ncheck = false\n",
    );
    let refused = check(&server, true).await.unwrap_err();
    assert_eq!(code(&refused), "UPDATE_002");
    assert!(
        hits.load(Ordering::SeqCst) >= 3,
        "latest, manifest and signature"
    );
}

#[tokio::test]
async fn the_daily_check_fails_quietly_and_waits_a_day() {
    let dir = tempfile::tempdir().unwrap();
    let (port, hits) = fake_github().await;
    let exe = dir.path().join("dev/target/debug/humanitld");
    let server = server(dir.path(), port, exe, dir.path().join("root"), "");
    // Die erste Frage geht hinaus und scheitert an der Signatur, ohne Fehler.
    let status = check(&server, false).await.unwrap();
    assert!(!status.available);
    let after_first = hits.load(Ordering::SeqCst);
    assert!(after_first >= 3);
    // Die zweite am selben Tag fragt nicht.
    check(&server, false).await.unwrap();
    assert_eq!(hits.load(Ordering::SeqCst), after_first);
}

#[tokio::test]
async fn apply_refuses_where_the_daemon_cannot_install() {
    let dir = tempfile::tempdir().unwrap();
    let (port, hits) = fake_github().await;
    // Ein Build-Baum: kein Weg aus dem Daemon.
    let exe = dir.path().join("dev/target/debug/humanitld");
    let server = server(dir.path(), port, exe, dir.path().join("root"), "");
    let request = v1::UpdateRequest {
        op: Some(v1::update_request::Op::Apply(v1::update_request::Apply {
            version: "0.0.999".to_owned(),
            appimage: String::new(),
        })),
    };
    let Err(refused) = server.update(Request::new(request)).await else {
        panic!("apply must refuse");
    };
    assert_eq!(code(&refused), "UPDATE_004");
    assert_eq!(
        hits.load(Ordering::SeqCst),
        0,
        "nothing is fetched before the refusal"
    );
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn apply_on_an_apt_package_names_the_upgrade_command() {
    let dir = tempfile::tempdir().unwrap();
    let (port, _hits) = fake_github().await;
    let root = dir.path().join("root");
    std::fs::create_dir_all(root.join("var/lib/dpkg/info")).unwrap();
    std::fs::write(root.join("var/lib/dpkg/info/humanitl.list"), "").unwrap();
    std::fs::create_dir_all(root.join("etc/apt/sources.list.d")).unwrap();
    std::fs::write(
        root.join("etc/apt/sources.list.d/nurkert.list"),
        "deb [signed-by=/usr/share/keyrings/nurkert-archive-keyring.gpg] https://apt.nurkert.de stable main
",
    )
    .unwrap();
    std::fs::create_dir_all(root.join("usr/bin")).unwrap();
    std::fs::write(root.join("usr/bin/pkexec"), "").unwrap();
    let server = server(
        dir.path(),
        port,
        PathBuf::from("/usr/lib/humanitl/bin/humanitld"),
        root,
        "[update]\ncheck = false\n",
    );
    let status = check(&server, false).await.unwrap();
    assert_eq!(status.method, i32::from(v1::UpdateMethod::Apt));
    assert_eq!(
        status.upgrade_command,
        [
            "pkexec",
            "apt-get",
            "install",
            "--only-upgrade",
            "-y",
            "humanitl"
        ]
    );
}

/// Ein Tausch, der mittendrin stehen blieb (Ziel fehlt, `.previous` da),
/// wird vor jeder Anfrage ins Netz zurückgeholt, auch wenn GitHub danach
/// nichts Brauchbares liefert (Review zu HUM-382).
#[tokio::test]
async fn apply_recovers_an_interrupted_swap_before_asking_the_network() {
    let dir = tempfile::tempdir().unwrap();
    let (port, _hits) = fake_github().await;
    let (exe, target) = if cfg!(target_os = "macos") {
        let bundle = dir.path().join("apps/Humanitl.app");
        (bundle.join("Contents/MacOS/bin/humanitld"), bundle)
    } else {
        let root = dir.path().join("apps/humanitl-0.0.5-linux-x86_64");
        (root.join("bin/humanitld"), root)
    };
    let previous = target.with_file_name(format!(
        "{}.previous",
        target.file_name().unwrap().to_string_lossy()
    ));
    std::fs::create_dir_all(previous.join("bin")).unwrap();
    std::fs::write(previous.join("bin/humanitld"), "old").unwrap();
    let server = server(dir.path(), port, exe, dir.path().join("root"), "");
    let request = v1::UpdateRequest {
        op: Some(v1::update_request::Op::Apply(v1::update_request::Apply {
            version: "0.0.999".to_owned(),
            appimage: String::new(),
        })),
    };
    // Die Signatur des Fakes ist falsch: Apply lehnt ab, aber erst nachdem
    // die alte Fassung wieder an ihrem Platz liegt.
    let Err(_refused) = server.update(Request::new(request)).await else {
        panic!("apply must refuse the bad signature");
    };
    assert_eq!(
        std::fs::read_to_string(target.join("bin/humanitld")).unwrap(),
        "old"
    );
    assert!(!previous.exists());
}

/// Ein AppImage-Pfad aus der Anfrage, den der Daemon nicht vermerkt hat,
/// wird abgelehnt, bevor sich im Dateisystem etwas ändert; `Check` holt über
/// ihn nichts zurück (Review zu HUM-382).
#[tokio::test]
async fn an_unrelated_appimage_path_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (port, hits) = fake_github().await;
    let victim = dir.path().join("other/Victim.AppImage");
    let previous = dir.path().join("other/Victim.AppImage.previous");
    std::fs::create_dir_all(previous.parent().unwrap()).unwrap();
    std::fs::write(&previous, "someone else's file").unwrap();
    let exe = dir
        .path()
        .join("home/.local/lib/humanitl/current/bin/humanitld");
    let server = server(
        dir.path(),
        port,
        exe,
        dir.path().join("root"),
        "[update]\ncheck = false\n",
    );
    let check = v1::UpdateRequest {
        op: Some(v1::update_request::Op::Check(v1::update_request::Check {
            force: false,
            appimage: victim.display().to_string(),
        })),
    };
    let mut stream = server
        .update(Request::new(check))
        .await
        .unwrap()
        .into_inner();
    let _ = stream.next().await;
    let apply = v1::UpdateRequest {
        op: Some(v1::update_request::Op::Apply(v1::update_request::Apply {
            version: "0.0.999".to_owned(),
            appimage: victim.display().to_string(),
        })),
    };
    let Err(refused) = server.update(Request::new(apply)).await else {
        panic!("apply must refuse an unrecorded AppImage");
    };
    assert_eq!(code(&refused), "UPDATE_004");
    assert!(!victim.exists());
    assert_eq!(
        std::fs::read_to_string(&previous).unwrap(),
        "someone else's file"
    );
    assert_eq!(hits.load(Ordering::SeqCst), 0);
}
