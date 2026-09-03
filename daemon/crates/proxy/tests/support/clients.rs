//! Echte Clients für die Konformitäts-Matrix (HUM-017).
//!
//! Die Matrix fährt `curl`, `websocat`, `grpcurl`, `python3`, `node` und `git`
//! durch den Proxy. Kein Test darf fehlschlagen, nur weil eines dieser
//! Werkzeuge auf der Maschine fehlt: [`require`] sucht es im `PATH` und
//! liefert `None`, wenn es nicht da ist; der Test meldet sich dann über
//! [`skip`] ab und endet grün. Im CI sind alle Werkzeuge installiert, dort
//! überspringt nichts.
//!
//! Dazu kommt [`env_kit`]: dieselben Umgebungsvariablen wie das Env-Kit der
//! Sandbox ([`humanitl_proxy::ca::ENV_KIT`]), nur mit der Proxy-Adresse und
//! dem CA-Pfad dieses Tests statt der Pfade in der Sandbox. So sehen die
//! Clients genau die Umgebung, die ein Agent später sieht.

use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use humanitl_proxy::ca::{ENV_KIT, SANDBOX_CA_PATH, SANDBOX_PROXY_URL};
use tokio::process::Command;

/// So lange darf ein externer Client höchstens laufen.
pub const CLIENT_TIMEOUT: Duration = Duration::from_secs(60);

/// Ein gefundenes Werkzeug.
#[derive(Debug, Clone)]
pub struct Tool {
    /// Der Name, unter dem gesucht wurde.
    pub name: String,
    /// Der volle Pfad zur ausführbaren Datei.
    pub path: PathBuf,
}

impl Tool {
    /// Ein Kommando, das dieses Werkzeug startet.
    #[must_use]
    pub fn command(&self) -> Command {
        Command::new(&self.path)
    }
}

/// Sucht ein Werkzeug im `PATH`.
///
/// Liefert `None`, wenn es fehlt oder nicht ausführbar ist.
#[must_use]
pub fn require(name: &str) -> Option<Tool> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| is_executable(candidate))
        .map(|path| Tool {
            name: name.to_owned(),
            path,
        })
}

fn is_executable(path: &Path) -> bool {
    std::fs::metadata(path)
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

/// Meldet einen übersprungenen Test mit Grund auf stderr.
///
/// `cargo test` kennt kein „skipped"; die Zeile ist das Signal an den
/// Menschen, der das Protokoll liest, und an das CI-Log.
pub fn skip(test: &str, why: &str) {
    eprintln!("SKIP {test}: {why}");
}

/// Meldet einen Test ab, weil ein Werkzeug fehlt.
pub fn skip_missing(test: &str, tool: &str) {
    skip(
        test,
        &format!("`{tool}` is not installed; install it to run this row of the matrix"),
    );
}

/// Was ein gelaufenes Kommando hinterlassen hat.
#[derive(Debug)]
pub struct Run {
    /// Der Exit-Code, `None` bei Abbruch durch ein Signal.
    pub code: Option<i32>,
    /// Alles auf stdout, roh (Antwortkörper können binär sein).
    pub stdout: Vec<u8>,
    /// Alles auf stderr.
    pub stderr: Vec<u8>,
}

impl Run {
    /// Wahr, wenn das Kommando mit 0 endete.
    #[must_use]
    pub fn success(&self) -> bool {
        self.code == Some(0)
    }

    /// stdout als Text, verlustbehaftet.
    #[must_use]
    pub fn out(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }

    /// stderr als Text, verlustbehaftet.
    #[must_use]
    pub fn err(&self) -> String {
        String::from_utf8_lossy(&self.stderr).into_owned()
    }

    /// Beides zusammen, für Fehlermeldungen im Test.
    #[must_use]
    pub fn report(&self) -> String {
        format!(
            "exit {:?}\n--- stdout ---\n{}\n--- stderr ---\n{}",
            self.code,
            self.out(),
            self.err()
        )
    }
}

/// Startet ein Kommando und wartet höchstens `limit` auf sein Ende.
///
/// stdin ist geschlossen, stdout und stderr werden gesammelt. Läuft das
/// Kommando in die Frist, endet der Test mit einer Meldung, die es nennt.
pub async fn run(mut cmd: Command, limit: Duration) -> Run {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let child = cmd.spawn().expect("the client starts");
    let output = tokio::time::timeout(limit, child.wait_with_output())
        .await
        .unwrap_or_else(|_| panic!("the client did not finish within {limit:?}"))
        .expect("the client is waitable");
    Run {
        code: output.status.code(),
        stdout: output.stdout,
        stderr: output.stderr,
    }
}

/// Das Env-Kit dieses Tests: [`ENV_KIT`] mit `proxy_url` statt der
/// Sandbox-Adresse und `ca_pem` statt des Sandbox-Pfades.
#[must_use]
pub fn env_kit(proxy_url: &str, ca_pem: &Path) -> Vec<(String, String)> {
    let ca = ca_pem.display().to_string();
    ENV_KIT
        .iter()
        .map(|(key, value)| {
            let value = match *value {
                SANDBOX_PROXY_URL => proxy_url.to_owned(),
                SANDBOX_CA_PATH => ca.clone(),
                other => other.to_owned(),
            };
            ((*key).to_owned(), value)
        })
        .collect()
}

/// Setzt ein Env-Kit auf einem Kommando.
pub fn apply(cmd: &mut Command, kit: &[(String, String)]) {
    for (key, value) in kit {
        cmd.env(key, value);
    }
}

/// Wahr, wenn dieser Python-Interpreter das Modul importieren kann.
pub async fn python_has_module(python: &Tool, module: &str) -> bool {
    let mut cmd = python.command();
    cmd.arg("-c").arg(format!("import {module}"));
    run(cmd, Duration::from_secs(30)).await.success()
}
