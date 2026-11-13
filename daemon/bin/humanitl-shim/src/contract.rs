//! Werte, die der Shim und seine Integrationstests teilen (HUM-459).
//!
//! Ein Integrationstest sieht das Binary nicht als Bibliothek. Statt die
//! Werte dort abzuschreiben, binden die Tests genau diese Datei mit
//! `#[path = "../src/contract.rs"]` ein; ändert sich hier ein Wert, prüfen
//! sie ihn mit, statt still strenger oder lockerer zu werden.

use std::time::Duration;

/// How long the agent has after the lifeline ends before everything is
/// killed; the same five seconds as `humanitl_sandbox::KILL_GRACE`, written
/// out because the shim knows no workspace crate. The contract test
/// `the_lifeline_grace_is_the_launchers` in the sandbox crate holds both
/// together.
pub const KILL_GRACE: Duration = Duration::from_secs(5);

/// How many further shells may live at the same time.
pub const MAX_SHELLS: usize = 4;

/// The warning with which the shim gives up the shell door once the daemon
/// stops reading its replies.
pub const SHELL_REPLIES_LOST: &str =
    "humanitl-shim: warning: shell replies cannot reach the daemon; ending every further shell\n";
