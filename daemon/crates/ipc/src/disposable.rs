//! Die Wegwerf-Sandbox im Dienst (HUM-350).
//!
//! [`crate::SandboxService`] legt beim Start einer Wegwerf-Sandbox eine
//! obere Schicht an (`humanitl_sandbox::Layer`) und nimmt die Grundlage
//! des Projekts; nach dem Ende wartet beides hier als `Waiting`, bis ein
//! Client `changes`, `apply_changes` oder `discard_changes` schickt. Es wartet
//! höchstens eine Schicht: Ein neuer Start mit wartenden Änderungen wird
//! abgelehnt (`SANDBOX_030`), damit nichts, was der Mensch noch nicht
//! gesehen hat, stillschweigend verschwindet.
//!
//! Die Fachlogik (Diff, Prüfung, Schreiben) liegt in
//! `humanitl_sandbox::disposable`; hier stehen nur der Zustand, die
//! Wire-Form und die Aufzeichnung.

#[cfg(target_os = "linux")]
use std::os::fd::AsFd as _;
#[cfg(target_os = "linux")]
use std::path::PathBuf;
#[cfg(target_os = "linux")]
use std::sync::Arc;

use humanitl_config::SandboxMode;
use humanitl_core::diagnostics::codes;
#[cfg(target_os = "linux")]
use humanitl_core::ids::{SandboxId, SessionId};
use humanitl_core::{Diagnostic, FixAction, Severity};
#[cfg(target_os = "linux")]
use humanitl_sandbox::disposable::{self, ChangeSet, DisposableRecord, LayerChange};
#[cfg(target_os = "linux")]
use humanitl_sandbox::summary::{ChangeKind, SessionSummary, display_path};
#[cfg(target_os = "linux")]
use humanitl_sandbox::worktree::{Kind, TreeSnapshot, open_root};
#[cfg(target_os = "linux")]
use humanitl_sandbox::{ApplyOptions, Layer, SandboxHandle, SizeGuard};

#[cfg(target_os = "linux")]
use crate::v1;

/// Eine obere Schicht, deren Änderungen auf den Menschen warten.
#[cfg(target_os = "linux")]
#[derive(Debug)]
pub(crate) struct Waiting {
    /// Der Lauf.
    pub(crate) sandbox: SandboxId,
    /// Die Schicht.
    pub(crate) layer: Layer,
    /// Das Projektverzeichnis auf dem Host, zur Anzeige.
    pub(crate) work_dir: PathBuf,
    /// Der Projektordner, geöffnet vor dem Start; Diff und Übernahme
    /// arbeiten nur an ihm, nie am Pfad (C1).
    pub(crate) lower: std::os::fd::OwnedFd,
    /// Der Stand des Projekts vor dem Start.
    pub(crate) baseline: TreeSnapshot,
    /// Die Sandbox, damit vor jedem Lesen feststeht, dass sie beendet ist.
    pub(crate) handle: Arc<SandboxHandle>,
    /// Die Startzeit ihres ersten Prozesses (Feld 22 von `/proc/<pid>/stat`).
    pub(crate) child_start: Option<u64>,
    /// Die Grenze der Schicht, solange die Sandbox läuft.
    pub(crate) guard: Option<SizeGuard>,
}

/// Ohne das Overlay von bubblewrap entsteht nie eine Schicht; der Typ hat
/// keinen Wert (HUM-350).
#[cfg(not(target_os = "linux"))]
#[derive(Debug)]
pub(crate) enum Waiting {}

/// Was eine Übernahme ergab.
#[cfg(target_os = "linux")]
pub(crate) struct Applied {
    /// Die Antwort an den Client.
    pub(crate) outcome: v1::DisposableOutcome,
    /// Der Befund, wenn das Schreiben mittendrin abbrach; die Schicht wartet
    /// dann weiter.
    pub(crate) failed: Option<Diagnostic>,
    /// Was in die Aufzeichnung gehört, wenn die Schicht danach weg ist.
    pub(crate) record: Option<DisposableRecord>,
}

/// Der Befund, wenn keine Änderungen warten.
#[must_use]
pub fn nothing_pending() -> Diagnostic {
    Diagnostic::builder(codes::SANDBOX_031, Severity::Warning)
        .why(
            "no disposable sandbox has changes waiting; changes wait after a run with \
             sandbox.mode = \"disposable\" until they are applied or discarded",
        )
        .fix(FixAction::CopyCommand(
            "humanitl run --disposable".to_owned(),
        ))
        .build()
}

/// Der Befund, wenn ein Start auf wartende Änderungen trifft.
#[cfg(target_os = "linux")]
#[must_use]
pub(crate) fn pending_blocks_start(sandbox: SandboxId) -> Diagnostic {
    Diagnostic::builder(codes::SANDBOX_030, Severity::Blocking)
        .why(format!(
            "the changes of the disposable sandbox {sandbox} still wait for apply or discard; \
             a new disposable run would put them out of reach"
        ))
        .fix(FixAction::CopyCommand(
            "humanitl sandbox changes".to_owned(),
        ))
        .build()
}

/// Liest `Start.sandbox_mode`; leer heißt: die Konfiguration.
///
/// # Errors
///
/// `CONFIG_003` für ein anderes Wort als `direct` oder `disposable`.
pub(crate) fn parse_mode(text: &str) -> Result<Option<SandboxMode>, Diagnostic> {
    if text.trim().is_empty() {
        return Ok(None);
    }
    SandboxMode::parse(text).map(Some).ok_or_else(|| {
        Diagnostic::builder(codes::CONFIG_003, Severity::Blocking)
            .why(format!(
                "sandbox_mode {:?} is neither \"direct\" nor \"disposable\"",
                text.trim()
            ))
            .fix(FixAction::ChangeSetting {
                key: "sandbox.mode".to_owned(),
                value: "disposable".to_owned(),
            })
            .build()
    })
}

/// Ob eine Anfrage diesen Lauf meint: leer meint den wartenden.
///
/// # Errors
///
/// `SANDBOX_031`, wenn sie einen anderen nennt.
#[cfg(target_os = "linux")]
pub(crate) fn check_id(waiting: &Waiting, asked: &str) -> Result<(), Diagnostic> {
    let asked = asked.trim();
    if asked.is_empty() || asked == waiting.sandbox.to_string() {
        return Ok(());
    }
    Err(Diagnostic::builder(codes::SANDBOX_031, Severity::Warning)
        .why(format!(
            "the changes of sandbox {asked} do not wait here; the changes of sandbox {} do",
            waiting.sandbox
        ))
        .fix(FixAction::CopyCommand(
            "humanitl sandbox changes".to_owned(),
        ))
        .build())
}

#[cfg(target_os = "linux")]
impl Waiting {
    /// Liest die obere Schicht und vergleicht sie mit dem Projekt.
    ///
    /// # Errors
    ///
    /// `SANDBOX_031`, solange die Sandbox läuft; `SANDBOX_011`, wenn eine der
    /// beiden Wurzeln nicht zu öffnen ist.
    fn scan(&self) -> Result<ChangeSet, Diagnostic> {
        disposable::ensure_ended(&self.handle, self.child_start)?;
        let upper = open_root(&self.layer.upper())?;
        Ok(disposable::scan(
            upper.as_fd(),
            self.lower.as_fd(),
            &self.baseline,
        ))
    }

    /// Die wartenden Änderungen in ihrer Wire-Form, mit Text-Diffs.
    ///
    /// # Errors
    ///
    /// Wie [`Waiting::scan`].
    pub(crate) fn changes(&self) -> Result<v1::DisposableChanges, Diagnostic> {
        let set = self.scan()?;
        let upper = open_root(&self.layer.upper())?;
        let lower = self.lower.as_fd();
        let changes = set
            .changes
            .iter()
            .map(|change| {
                let diff = change.text_diff(upper.as_fd(), lower);
                change_to_proto(change, diff)
            })
            .collect();
        Ok(v1::DisposableChanges {
            sandbox_id: self.sandbox.to_string(),
            work_dir: self.work_dir.to_string_lossy().into_owned(),
            changes,
            truncated: set.truncated,
        })
    }

    /// Übernimmt die gewählten Änderungen.
    ///
    /// # Errors
    ///
    /// `SANDBOX_031`, wenn die Auswahl oder eine Prüfung vor dem ersten
    /// Schreiben scheitert; dann ist nichts geschrieben.
    pub(crate) fn apply(
        &self,
        request: &v1::sandbox_request::ApplyChanges,
    ) -> Result<Applied, Diagnostic> {
        check_id(self, &request.sandbox_id)?;
        let set = self.scan()?;
        let chosen = disposable::select(&set, &request.ids)?;
        let upper = open_root(&self.layer.upper())?;
        let report = disposable::apply(
            upper.as_fd(),
            self.lower.as_fd(),
            &self.baseline,
            &chosen,
            &options_of(request),
        )?;
        let applied: Vec<String> = report.applied.iter().map(|rel| display_path(rel)).collect();
        if let Some((_, failed)) = report.failed {
            return Ok(Applied {
                outcome: outcome(self.sandbox, "partial", applied, Vec::new()),
                failed: Some(failed),
                record: None,
            });
        }
        let discarded: Vec<String> = set
            .changes
            .iter()
            .filter(|change| !report.applied.contains(&change.rel))
            .map(LayerChange::display)
            .collect();
        let record = DisposableRecord {
            outcome: "applied".to_owned(),
            applied: applied.clone(),
            discarded: discarded.clone(),
        };
        Ok(Applied {
            outcome: outcome(self.sandbox, "applied", applied, discarded),
            failed: None,
            record: Some(record),
        })
    }

    /// Die Pfade, die mit einem Verwerfen verloren gehen, für die
    /// Aufzeichnung; leer, wenn die Schicht nicht zu lesen ist.
    pub(crate) fn discarded_paths(&self) -> Vec<String> {
        self.scan()
            .map(|set| set.changes.iter().map(LayerChange::display).collect())
            .unwrap_or_default()
    }

    /// Die Zusammenfassung des Laufs mit dem, was der Mensch entschieden hat.
    pub(crate) fn summary(&self, session: SessionId, record: DisposableRecord) -> SessionSummary {
        let mut summary = SessionSummary::new(session, self.sandbox, &self.work_dir);
        summary.disposable = Some(record);
        summary
    }
}

/// Die Antwort auf `apply_changes` und `discard_changes`.
#[cfg(target_os = "linux")]
pub(crate) fn outcome(
    sandbox: SandboxId,
    outcome: &str,
    applied: Vec<String>,
    discarded: Vec<String>,
) -> v1::DisposableOutcome {
    v1::DisposableOutcome {
        sandbox_id: sandbox.to_string(),
        outcome: outcome.to_owned(),
        applied,
        discarded,
    }
}

#[cfg(target_os = "linux")]
fn change_to_proto(change: &LayerChange, diff: Option<String>) -> v1::DisposableChange {
    let kind = match change.kind {
        ChangeKind::Added => v1::FileChangeKind::Added,
        ChangeKind::Modified => v1::FileChangeKind::Modified,
        ChangeKind::Removed => v1::FileChangeKind::Removed,
        ChangeKind::SymlinkAdded => v1::FileChangeKind::SymlinkAdded,
        ChangeKind::ModeChanged => v1::FileChangeKind::ModeChanged,
    };
    let (entry, target) = match &change.what {
        Kind::File => ("file", String::new()),
        Kind::Dir => ("dir", String::new()),
        Kind::Symlink { target } => ("symlink", display_path(target)),
        Kind::Other => ("other", String::new()),
    };
    let display = change.display();
    let is_file = change.what == Kind::File && change.kind != ChangeKind::ModeChanged;
    v1::DisposableChange {
        id: change.path_hash(),
        mangled: display.as_bytes() != change.rel.as_os_str().as_encoded_bytes(),
        path: display,
        kind: kind.into(),
        entry: entry.to_owned(),
        size: change.size,
        mode: change.mode,
        sha256: change.sha256.map(hex::encode).unwrap_or_default(),
        symlink_target: target,
        binary: is_file && diff.is_none(),
        diff: diff.unwrap_or_default(),
        conflict: change.conflict.clone().unwrap_or_default(),
        refused: change.refused.clone().unwrap_or_default(),
        stripped_bits: change.stripped,
        hardlinked: change.hardlinked,
    }
}

/// Wie eine Anfrage übernehmen will (HUM-351): Überschrieben wird je
/// Kennung aus `overwrite_ids`; `overwrite` heißt weiter „alle gewählten",
/// für `humanitl sandbox apply --overwrite`. Eine Bestätigung für eine Datei
/// wird nie zu einer für alle.
#[cfg(target_os = "linux")]
fn options_of(request: &v1::sandbox_request::ApplyChanges) -> ApplyOptions {
    ApplyOptions {
        overwrite: request.overwrite,
        overwrite_ids: request
            .overwrite_ids
            .iter()
            .map(|id| id.trim().to_owned())
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    /// Eine einzeln bestätigte Datei erlaubt nur ihr Überschreiben, nie das
    /// einer anderen mit Konflikt (HUM-351).
    #[cfg(target_os = "linux")]
    #[test]
    fn overwrite_ids_name_only_the_confirmed_changes() {
        let change = |rel: &str| LayerChange {
            rel: PathBuf::from(rel),
            kind: ChangeKind::Modified,
            what: Kind::File,
            size: 3,
            mode: 0o644,
            stripped: 0,
            sha256: None,
            hardlinked: false,
            conflict: Some("changed in the project after the start".to_owned()),
            refused: None,
        };
        let confirmed = change("a.txt");
        let other = change("b.txt");
        let options = options_of(&v1::sandbox_request::ApplyChanges {
            sandbox_id: String::new(),
            ids: vec![confirmed.path_hash(), other.path_hash()],
            overwrite: false,
            overwrite_ids: vec![format!(" {} ", confirmed.path_hash())],
        });
        assert!(!options.overwrite);
        assert!(options.may_overwrite(&confirmed));
        assert!(!options.may_overwrite(&other));
        // Der alte Schalter der Kommandozeile gilt für jede gewählte.
        let all = options_of(&v1::sandbox_request::ApplyChanges {
            sandbox_id: String::new(),
            ids: Vec::new(),
            overwrite: true,
            overwrite_ids: Vec::new(),
        });
        assert!(all.may_overwrite(&other));
    }

    #[test]
    fn the_mode_of_a_start_is_read_strictly() {
        assert_eq!(parse_mode("").unwrap(), None);
        assert_eq!(
            parse_mode(" Disposable ").unwrap(),
            Some(SandboxMode::Disposable)
        );
        assert_eq!(parse_mode("direct").unwrap(), Some(SandboxMode::Direct));
        let err = parse_mode("throwaway").unwrap_err();
        assert_eq!(err.code.as_str(), "CONFIG_003");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_change_travels_with_its_id_and_sanitized_path() {
        let change = LayerChange {
            rel: PathBuf::from("a\u{1b}[2Jb.txt"),
            kind: ChangeKind::Added,
            what: Kind::File,
            size: 3,
            mode: 0o644,
            stripped: 0o4000,
            sha256: Some([0xab; 32]),
            hardlinked: false,
            conflict: None,
            refused: None,
        };
        let wire = change_to_proto(&change, None);
        assert_eq!(wire.id.len(), 16);
        assert!(!wire.path.contains('\u{1b}'), "{}", wire.path);
        assert!(wire.mangled);
        assert!(
            wire.binary,
            "a file without a text diff is shown by size and hash"
        );
        assert_eq!(wire.sha256.len(), 64);
        assert_eq!(wire.stripped_bits, 0o4000);
    }
}
