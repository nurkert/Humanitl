//! Die Wegwerf-Sandbox: das Projekt als Overlay, mit Diff, Übernahme und
//! Verwerfen (HUM-350).
//!
//! Mit `sandbox.mode = "disposable"` hängt der Launcher das Projektverzeichnis
//! nur lesbar als untere Schicht eines Overlays an `/work`. Jede Änderung des
//! Agenten landet in der oberen Schicht, einem Verzeichnis, das der Daemon
//! vor dem Start im Laufzeitverzeichnis anlegt
//! (`$XDG_RUNTIME_DIR/humanitl/disposable/<id>/upper`, Rechte `0700`, auf
//! tmpfs und damit im Arbeitsspeicher). Nach dem Ende des Laufs liest der
//! Daemon die obere Schicht, zeigt den Unterschied, und der Mensch übernimmt
//! alles, einzelne Dateien oder nichts.
//!
//! # Warum die obere Schicht auf dem Host liegt (Variante a)
//!
//! `--tmp-overlay` legt die obere Schicht in ein tmpfs, das nur im
//! Mount-Namensraum der Sandbox existiert; nach dem Ende ist sie weg, bevor
//! jemand sie lesen konnte. Die Alternative, den Shim als PID 1 vor seinem
//! Ende ein Archiv bauen zu lassen, brächte Code in den Prozess, der die
//! Sandbox zusammenhält, und einen neuen Kanal hinaus, über den der Daemon
//! vom Agenten beeinflusste Bytes parsen müsste. Hier dagegen liest der
//! Daemon nach dem Ende ein Verzeichnis, in das niemand mehr schreibt, mit
//! denselben Mitteln wie die Zusammenfassung (`openat2` mit
//! `RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS`, nie ein Symlink verfolgt). In der
//! Sandbox selbst ist der Pfad der oberen Schicht nicht zu sehen: `/run` kommt
//! nie vom Host, und `bwrap` öffnet die Verzeichnisse vor dem Einhängen über
//! Deskriptoren.
//!
//! # Was die Grenze ist
//!
//! - **Größe.** Die obere Schicht liegt auf dem tmpfs des
//!   Laufzeitverzeichnisses; dessen Größe ist die harte Grenze (logind
//!   setzt sie, Vorgabe 10 % des Arbeitsspeichers). Darunter hält
//!   `SizeGuard` eine eigene Grenze ([`Budget`]): Er misst das Wachstum des
//!   Dateisystems und beendet die Sandbox, sobald es die Grenze überschreitet
//!   (`SANDBOX_032`).
//! - **Lebensdauer.** Gelesen und übernommen wird nur, wenn die Sandbox
//!   beendet ist (`ensure_ended`); verworfen wird mit `Layer::discard`,
//!   das auch Verzeichnisse ohne Rechte und beliebig tiefe Bäume entfernt.
//! - **Übernahme.** `apply` schreibt nie über einen Symlink im Projekt
//!   hinaus, schreibt keinen Symlink, der das Projekt verlässt, kein Gerät,
//!   keine FIFO, keinen Socket, keine setuid-, setgid- oder Sticky-Bits, und
//!   keine Datei, die der Mensch seit dem Start geändert hat, ohne dass er es
//!   ausdrücklich für diese Datei sagt (`ApplyOptions::may_overwrite`).

#[cfg(target_os = "linux")]
mod apply;
#[cfg(target_os = "linux")]
mod layer;
#[cfg(target_os = "linux")]
mod scan;
mod text_diff;

#[cfg(not(target_os = "linux"))]
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use humanitl_config::SandboxMode;
use humanitl_core::diagnostics::codes::{SANDBOX_030, SANDBOX_033};
use humanitl_core::{Diagnostic, FixAction, Severity};
use serde::{Deserialize, Serialize};

use crate::bwrap::Version;

#[cfg(target_os = "linux")]
pub use crate::disposable::apply::{ApplyOptions, ApplyReport, apply, select};
#[cfg(all(test, target_os = "linux"))]
use crate::disposable::layer::is_tmpfs;
#[cfg(target_os = "linux")]
pub use crate::disposable::layer::{
    Layer, SizeGuard, Usage, baseline, baseline_at, baseline_limits, ensure_ended, probe_layer_dir,
    process_lives, require_tmpfs, start_ticks, sweep,
};
#[cfg(target_os = "linux")]
pub(crate) use crate::disposable::layer::{chmod_nofollow, open_dir_forcing};
#[cfg(target_os = "linux")]
pub use crate::disposable::scan::{ChangeSet, LayerChange, scan};
pub use crate::disposable::text_diff::{DIFF_MAX_BYTES, DIFF_MAX_OUTPUT, unified};

/// Ab dieser `bwrap`-Fassung gibt es `--overlay-src` und `--overlay`
/// (bubblewrap 0.11.0).
pub const MIN_OVERLAY_VERSION: Version = Version(0, 11, 0);

/// Ab dieser Kernel-Fassung darf ein Nutzer ohne Rechte overlayfs in seinem
/// Namensraum einhängen (Linux 5.11).
pub const MIN_OVERLAY_KERNEL: (u32, u32) = (5, 11);

/// Ob der Kernel mit dieser Fassung (`/proc/sys/kernel/osrelease`, etwa
/// `6.1.0-13-amd64`) overlayfs ohne Rechte einhängt (HUM-357). Eine Fassung,
/// die sich nicht lesen lässt, zählt als zu alt: Die Probe sagt dann lieber
/// `direct` mit Grund als einen Start, der beim Einhängen scheitert.
#[must_use]
pub fn kernel_allows_overlay(release: &str) -> bool {
    let mut parts = release
        .trim()
        .split(|c: char| !c.is_ascii_digit())
        .map(str::parse::<u32>);
    match (parts.next(), parts.next()) {
        (Some(Ok(major)), Some(Ok(minor))) => (major, minor) >= MIN_OVERLAY_KERNEL,
        _ => false,
    }
}

/// Das Verzeichnis unter dem Laufzeitverzeichnis, in dem die Schichten liegen.
pub const DISPOSABLE_DIR: &str = "disposable";

/// So viele Bytes darf die obere Schicht höchstens wachsen, wenn das
/// Dateisystem darunter genug Platz hat; sonst die Hälfte des freien Platzes.
pub const MAX_LAYER_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// So viele Einträge darf die obere Schicht höchstens anlegen.
pub const MAX_LAYER_INODES: u64 = 200_000;

/// So oft misst `SizeGuard` das Wachstum.
pub const GUARD_INTERVAL: Duration = Duration::from_millis(250);

/// So tief steigen Diff und Verwerfen höchstens rekursiv; darunter wird der
/// Baum beim Verwerfen flach umgehängt und beim Diff als abgeschnitten
/// gemeldet.
pub const MAX_DEPTH: usize = 64;

/// Die Verzeichnisse, die `bwrap` für das Overlay bekommt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayDirs {
    /// Die obere Schicht (`--overlay RWSRC`).
    pub upper: PathBuf,
    /// Das Arbeitsverzeichnis des Overlays (`--overlay … WORKDIR`), auf
    /// demselben Dateisystem wie die obere Schicht.
    pub work: PathBuf,
}

/// Ob `Layer::create_with` verlangt, dass die Schicht im Arbeitsspeicher
/// liegt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TmpfsCheck {
    /// Nur auf tmpfs; der Start.
    Require,
    /// Auf jedem Dateisystem; nur für Tests, deren temporäres Verzeichnis
    /// auf der Platte liegt.
    Skip,
}

/// Die Grenze der oberen Schicht.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Budget {
    /// So viele Bytes darf das Dateisystem wachsen.
    pub bytes: u64,
    /// So viele Einträge darf es dazubekommen.
    pub inodes: u64,
}

impl Budget {
    /// Die Grenze für ein Dateisystem mit diesem freien Platz:
    /// [`MAX_LAYER_BYTES`] und [`MAX_LAYER_INODES`], höchstens die Hälfte
    /// dessen, was frei ist.
    #[must_use]
    pub fn for_free(free_bytes: u64, free_inodes: u64) -> Self {
        Self {
            bytes: MAX_LAYER_BYTES.min(free_bytes / 2),
            inodes: MAX_LAYER_INODES.min(free_inodes / 2),
        }
    }
}

/// Was in der Aufzeichnung steht, wenn der Mensch entschieden hat.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DisposableRecord {
    /// `applied`, `discarded` oder `no_changes` (ein Lauf ohne Änderungen, HUM-357).
    pub outcome: String,
    /// Die übernommenen Pfade, gesäubert wie in der Zusammenfassung.
    pub applied: Vec<String>,
    /// Die verworfenen Pfade, gesäubert.
    pub discarded: Vec<String>,
}

/// Der Befund unter einem System ohne das Overlay von bubblewrap: Die
/// Wegwerf-Sandbox gibt es vorerst nur unter Linux (HUM-350).
#[must_use]
pub fn unsupported() -> Diagnostic {
    Diagnostic::builder(SANDBOX_030, Severity::Blocking)
        .why(
            "the disposable sandbox needs the overlay of bubblewrap, which exists only on \
             Linux; on this system the agent works in the project folder or not at all",
        )
        .fix(FixAction::ChangeSetting {
            key: "sandbox.mode".to_owned(),
            value: "direct".to_owned(),
        })
        .build()
}

/// Der Modus, mit dem ein Start läuft, nachdem `sandbox.mode` aufgelöst ist
/// (HUM-357).
#[derive(Debug, Clone, PartialEq)]
pub struct ModeChoice {
    /// `direct` oder `disposable`, nie `auto`.
    pub mode: SandboxMode,
    /// Der Hinweis `SANDBOX_033`, wenn die Vorgabe `auto` auf `direct`
    /// zurückgefallen ist; sonst keiner.
    pub notice: Option<Diagnostic>,
}

impl ModeChoice {
    /// Ob der Start eine Wegwerf-Sandbox ist.
    #[must_use]
    pub fn is_disposable(&self) -> bool {
        self.mode == SandboxMode::Disposable
    }

    fn direct(notice: Option<Diagnostic>) -> Self {
        Self {
            mode: SandboxMode::Direct,
            notice,
        }
    }
}

/// Löst den gewünschten Modus gegen das auf, was das System kann (HUM-357).
///
/// `support` prüft die Bedingungen einer Wegwerf-Sandbox (bubblewrap mit
/// `--overlay`, Laufzeitverzeichnis auf tmpfs, Projekt schreibbar) und wird
/// nur gefragt, wenn der Wunsch nicht `direct` ist.
///
/// - `direct` bleibt `direct`.
/// - `disposable` ist eine Wahl des Menschen und fällt nie zurück: Fehlt eine
///   Bedingung, endet der Start mit deren `SANDBOX_030`.
/// - `auto` ist die Vorgabe: `disposable`, wenn alles da ist, sonst `direct`
///   mit dem Hinweis `SANDBOX_033`, der den Grund nennt. Unter einem System
///   ohne Overlay (macOS) gibt es den Modus gar nicht; dort ist `auto` still
///   `direct`, denn ein Hinweis bei jedem Start sagte nur, was immer gilt.
///
/// # Errors
///
/// Der Befund von `support`, wenn `disposable` ausdrücklich gewünscht ist und
/// nicht geht.
pub fn resolve_mode(
    wanted: SandboxMode,
    support: impl FnOnce() -> Result<(), Diagnostic>,
) -> Result<ModeChoice, Diagnostic> {
    match wanted {
        SandboxMode::Direct => Ok(ModeChoice::direct(None)),
        SandboxMode::Disposable => support().map(|()| ModeChoice {
            mode: SandboxMode::Disposable,
            notice: None,
        }),
        SandboxMode::Auto if !cfg!(target_os = "linux") => Ok(ModeChoice::direct(None)),
        SandboxMode::Auto => Ok(match support() {
            Ok(()) => ModeChoice {
                mode: SandboxMode::Disposable,
                notice: None,
            },
            Err(reason) => ModeChoice::direct(Some(fallback_notice(&reason))),
        }),
    }
}

/// Der Hinweis, dass die Vorgabe auf `direct` zurückgefallen ist
/// (`SANDBOX_033`, HUM-357): der Grund aus `reason`, dessen Fix, und wie der
/// Mensch den Hinweis abstellt.
#[must_use]
pub fn fallback_notice(reason: &Diagnostic) -> Diagnostic {
    let builder = Diagnostic::builder(SANDBOX_033, Severity::Warning).why(format!(
        "{}; sandbox.mode is auto, so this run writes to the project folder directly. Fix \
         the cause for a disposable sandbox, or set sandbox.mode = \"direct\" to silence this",
        reason.why.trim_end_matches('.')
    ));
    match &reason.fix {
        Some(fix) => builder.fix(fix.clone()),
        None => builder.fix(FixAction::ChangeSetting {
            key: "sandbox.mode".to_owned(),
            value: "direct".to_owned(),
        }),
    }
    .build()
}

/// Räumt Schichten auf, die ein früherer Daemon liegen ließ. Ohne Overlay
/// gibt es keine; nichts zu tun.
#[cfg(not(target_os = "linux"))]
#[must_use]
pub fn sweep(_runtime_dir: &Path) -> usize {
    0
}

#[cfg(all(test, target_os = "linux"))]
mod tests;
