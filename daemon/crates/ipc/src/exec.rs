//! Weitere Shells in der laufenden Sandbox: die RPC `Exec` (HUM-299).
//!
//! Wie `docker exec`: Neben dem Agenten öffnet der Mensch eine Shell mit
//! einem eigenen Terminal, ohne den Agenten anzuhalten und ohne sein Terminal
//! zu teilen. Gestartet wird sie vom Shim in der Sandbox
//! ([`humanitl_sandbox::SandboxHandle::open_shell`]); sie trägt dieselben
//! Grenzen wie der Agent. Dieses Modul gibt ihr ein Terminal wie das des
//! Agenten ([`TerminalHub::for_shell`]: ein Filter, ein Ring, ein Schreiber,
//! beliebig viele Leser), legt es unter seiner Kennung ab, damit
//! `Terminal` mit `Open.shell_id` es findet, und nimmt es wieder heraus,
//! sobald die Shell endet.
//!
//! Die Tür in die Sandbox öffnet nur diese RPC, und die erreicht nur, wer das
//! Token des Daemons hat; der gRPC-Socket wird nie in die Sandbox gebunden.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use humanitl_core::diagnostics::codes;
use humanitl_core::ids::SandboxId;
use humanitl_core::{Diagnostic, Severity};
use humanitl_sandbox::{SandboxHandle, Shell};

use crate::terminal::TerminalHub;
use crate::v1;

/// Das Kommando ohne Wunsch des Aufrufers: eine interaktive `bash`, wenn die
/// Sandbox eine findet, sonst `sh`. Über `/bin/sh`, denn das ist die Shell,
/// die `/etc/passwd` der Sandbox nennt (`SANDBOX_SHELL`).
///
/// Ohne Startdateien: Der Agent kann `~/.bashrc`, `~/.profile` oder eine
/// Datei, auf die `ENV` zeigt, in seinem Heimatverzeichnis anlegen, und eine
/// interaktive Shell führte sie aus, bevor der Mensch die erste Zeile tippt.
/// Deshalb `bash --norc --noprofile` und `sh` ohne `ENV`.
pub const DEFAULT_SHELL: [&str; 3] = [
    "/bin/sh",
    "-c",
    "if command -v bash >/dev/null 2>&1; then exec bash --norc --noprofile -i; fi; \
     unset ENV; exec sh -i",
];

/// Wie lange das Terminal einer schon beendeten Shell auf den ersten Client
/// wartet. Ein schnelles Kommando (`humanitl sandbox exec -- true`) endet,
/// bevor sich die Kommandozeile anhängt; ihre Ausgabe und ihr Exit-Code sollen
/// trotzdem ankommen.
pub const FINISHED_RETENTION: Duration = Duration::from_secs(30);

/// Die Größe eines Terminals, wenn der Aufrufer keine nennt.
const DEFAULT_SIZE: (u16, u16) = (80, 24);

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Ein Terminal in der Ablage, und ab wann es nicht mehr gilt.
#[derive(Debug)]
struct Entry {
    hub: TerminalHub,
    /// Gesetzt, sobald die Shell beendet ist: Bis dahin bekommt der erste
    /// Client noch Rückstand und Exit-Code, danach ist die Kennung fort.
    retire_at: Option<Instant>,
}

/// Die Terminals der weiteren Shells einer Sitzung, nach Kennung.
///
/// Billig zu klonen. Eine Shell steht darin, solange sie lebt. Endet sie,
/// bleibt ihr Terminal für genau einen Client stehen, höchstens
/// [`FINISHED_RETENTION`] lang; danach bekommt `Terminal` mit ihrer Kennung
/// `IPC_006`.
///
/// Die Kennung nach außen trägt die Sitzung (`<sitzung>-<n>`, die letzten
/// acht Hex-Zeichen der zufälligen Hälfte der Sandbox-Kennung): Die Nummern
/// des Shims beginnen je Sandbox bei 1, und ein `attach --shell 1` von
/// gestern träfe sonst die Shell einer anderen Sitzung.
#[derive(Debug, Clone)]
pub struct ShellRegistry {
    prefix: String,
    hubs: Arc<Mutex<BTreeMap<String, Entry>>>,
}

impl ShellRegistry {
    /// Die Ablage für die Shells dieser Sandbox.
    #[must_use]
    pub fn for_sandbox(sandbox: SandboxId) -> Self {
        let text = sandbox.to_string().replace('-', "");
        let prefix = text[text.len().saturating_sub(8)..].to_owned();
        Self {
            prefix,
            hubs: Arc::default(),
        }
    }

    /// Die Kennung nach außen für die Shell mit der Nummer `n` des Shims.
    #[must_use]
    pub fn external_id(&self, n: u32) -> String {
        format!("{}-{n}", self.prefix)
    }

    /// Das Terminal der Shell mit dieser Kennung: solange sie lebt, und
    /// danach einmal, für den ersten Client nach ihrem Ende.
    #[must_use]
    pub fn get(&self, shell_id: &str) -> Option<TerminalHub> {
        let mut hubs = lock(&self.hubs);
        let now = Instant::now();
        hubs.retain(|_, entry| entry.retire_at.is_none_or(|at| at > now));
        let id = shell_id.trim();
        match hubs.get(id)?.retire_at {
            None => hubs.get(id).map(|entry| entry.hub.clone()),
            Some(_) => hubs.remove(id).map(|entry| entry.hub),
        }
    }

    /// Die Kennungen der Shells, die leben oder auf ihren ersten Client
    /// warten.
    #[must_use]
    pub fn ids(&self) -> Vec<String> {
        let now = Instant::now();
        lock(&self.hubs)
            .iter()
            .filter(|(_, entry)| entry.retire_at.is_none_or(|at| at > now))
            .map(|(id, _)| id.clone())
            .collect()
    }

    fn insert(&self, id: &str, hub: TerminalHub) {
        lock(&self.hubs).insert(
            id.to_owned(),
            Entry {
                hub,
                retire_at: None,
            },
        );
    }

    /// Die Shell ist beendet: Ihr Terminal wartet noch `keep` auf einen
    /// Client.
    fn retire(&self, id: &str, keep: Duration) {
        if let Some(entry) = lock(&self.hubs).get_mut(id) {
            entry.retire_at = Some(Instant::now() + keep);
        }
    }
}

/// Eine geöffnete Shell, bevor ihr Leser läuft.
#[derive(Debug)]
pub struct Opened {
    /// Ihr Terminal; schon in der Ablage.
    pub hub: TerminalHub,
    /// Die Antwort an den Aufrufer.
    pub response: v1::ExecResponse,
    shell: Arc<Shell>,
    registry: ShellRegistry,
}

impl Opened {
    /// Liest die Ausgabe der Shell in ihr Terminal, bis sie endet, schickt
    /// den Exit-Code an jeden Client und lässt das Terminal noch für einen
    /// ersten Client stehen ([`FINISHED_RETENTION`]).
    ///
    /// Blockiert; gehört auf einen eigenen Faden. Danach fällt die
    /// Herrscherseite des Pseudoterminals, sobald kein Client sie mehr hält.
    #[must_use = "the exit code is the shell's own"]
    pub fn run(self) -> i32 {
        let Self {
            hub,
            response,
            shell,
            registry,
        } = self;
        let code = shell.pump(|chunk| hub.feed(chunk));
        hub.finish(code);
        registry.retire(&response.shell_id, FINISHED_RETENTION);
        code
    }
}

/// Öffnet eine weitere Shell in `handle` und legt ihr Terminal in
/// `registry` ab.
///
/// Blockiert, bis die Sandbox antwortet.
///
/// # Errors
///
/// `TERM_002` aus [`SandboxHandle::open_shell`]: keine Tür, beendete
/// Sandbox, Grenze erreicht, Ablehnung des Shims.
pub fn open(
    handle: &SandboxHandle,
    sandbox: SandboxId,
    registry: &ShellRegistry,
    request: &v1::ExecRequest,
    notices: bool,
) -> Result<Opened, Diagnostic> {
    let command: Vec<String> = if request.command.is_empty() {
        DEFAULT_SHELL
            .iter()
            .map(|part| (*part).to_owned())
            .collect()
    } else {
        request.command.clone()
    };
    let cols = size_or(request.cols, DEFAULT_SIZE.0);
    let rows = size_or(request.rows, DEFAULT_SIZE.1);
    let shell = Arc::new(handle.open_shell(&command, cols, rows)?);
    let shell_id = registry.external_id(shell.id());
    let hub = TerminalHub::for_shell(
        sandbox,
        Arc::clone(&shell),
        shell_id.clone(),
        cols,
        rows,
        notices,
    );
    registry.insert(&shell_id, hub.clone());
    Ok(Opened {
        hub,
        response: v1::ExecResponse {
            shell_id,
            command,
            sandbox_id: sandbox.to_string(),
        },
        shell,
        registry: registry.clone(),
    })
}

/// Eine Größe von der Leitung: `0` heißt die Vorgabe, alles über
/// [`u16::MAX`] ist keine Fenstergröße.
fn size_or(value: u32, default: u16) -> u16 {
    match value {
        0 => default,
        other => u16::try_from(other).unwrap_or(u16::MAX),
    }
}

/// Der Befund für eine Kennung, unter der keine Shell lebt.
#[must_use]
pub fn no_such_shell(shell_id: &str, sandbox: SandboxId, open: &[String]) -> Diagnostic {
    let known = if open.is_empty() {
        "none is open".to_owned()
    } else {
        format!("open are {}", open.join(", "))
    };
    Diagnostic::builder(codes::IPC_006, Severity::Error)
        .why(format!(
            "sandbox {sandbox} has no shell {shell_id:?}; it may have ended ({known})"
        ))
        .fix(humanitl_core::FixAction::CopyCommand(
            "humanitl sandbox exec".to_owned(),
        ))
        .build()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn a_size_of_zero_takes_the_default_and_a_huge_one_is_capped() {
        assert_eq!(size_or(0, 80), 80);
        assert_eq!(size_or(120, 80), 120);
        assert_eq!(size_or(u32::MAX, 80), u16::MAX);
    }

    #[test]
    fn the_default_shell_prefers_bash_and_runs_no_start_file() {
        assert_eq!(DEFAULT_SHELL[0], humanitl_sandbox::SANDBOX_SHELL);
        assert!(DEFAULT_SHELL[2].contains("exec bash --norc --noprofile -i"));
        assert!(DEFAULT_SHELL[2].ends_with("unset ENV; exec sh -i"));
    }

    #[test]
    fn an_external_id_carries_its_sandbox() {
        let first = ShellRegistry::for_sandbox(SandboxId::new());
        let second = ShellRegistry::for_sandbox(SandboxId::new());
        let id = first.external_id(1);
        assert!(id.ends_with("-1") && id.len() == 10, "{id}");
        assert_ne!(id, second.external_id(1), "another session, another id");
        assert_ne!(id, "1");
    }

    #[test]
    fn an_unknown_shell_names_the_open_ones() {
        let sandbox = SandboxId::new();
        let none = no_such_shell("7", sandbox, &[]);
        assert_eq!(none.code, codes::IPC_006);
        assert!(none.why.contains("none is open"), "{}", none.why);
        let some = no_such_shell("7", sandbox, &["1".to_owned(), "2".to_owned()]);
        assert!(some.why.contains("open are 1, 2"), "{}", some.why);
    }
}
