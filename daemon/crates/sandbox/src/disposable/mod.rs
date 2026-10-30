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
//!   ausdrücklich sagt (`ApplyOptions::overwrite`).

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

use humanitl_core::diagnostics::codes::SANDBOX_030;
use humanitl_core::{Diagnostic, FixAction, Severity};
use serde::{Deserialize, Serialize};

use crate::bwrap::Version;

#[cfg(target_os = "linux")]
pub use crate::disposable::apply::{ApplyOptions, ApplyReport, apply, select};
#[cfg(all(test, target_os = "linux"))]
use crate::disposable::layer::is_tmpfs;
#[cfg(target_os = "linux")]
pub use crate::disposable::layer::{
    Layer, SizeGuard, Usage, baseline, baseline_at, baseline_limits, ensure_ended, process_lives,
    start_ticks, sweep,
};
#[cfg(target_os = "linux")]
pub(crate) use crate::disposable::layer::{chmod_nofollow, open_dir_forcing};
#[cfg(target_os = "linux")]
pub use crate::disposable::scan::{ChangeSet, LayerChange, scan};
pub use crate::disposable::text_diff::{DIFF_MAX_BYTES, DIFF_MAX_OUTPUT, unified};

/// Ab dieser `bwrap`-Fassung gibt es `--overlay-src` und `--overlay`
/// (bubblewrap 0.11.0).
pub const MIN_OVERLAY_VERSION: Version = Version(0, 11, 0);

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
    /// `applied` oder `discarded`.
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

/// Räumt Schichten auf, die ein früherer Daemon liegen ließ. Ohne Overlay
/// gibt es keine; nichts zu tun.
#[cfg(not(target_os = "linux"))]
#[must_use]
pub fn sweep(_runtime_dir: &Path) -> usize {
    0
}

#[cfg(all(test, target_os = "linux"))]
mod tests;
