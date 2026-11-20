//! The watch over a session's credential file (HUM-520, ADR-0023 point 4).
//!
//! A session with a depot agent gets a writable copy of the configuration
//! ([`super::Scratch`]). The daemon looks at the one credential file in it
//! every [`WRITEBACK_POLL`] and once more when the sandbox ends, right before
//! the copy is removed ([`crate::SandboxHandle::before_cleanup`]); each look
//! that finds a changed file hands it to the guard ([`take_back`]). A look
//! that finds the file as the last look left it does nothing, so an unchanged
//! file that was refused once is not reported again every ten seconds.
//!
//! Every outcome that changed or refused something goes to the caller's
//! [`Report`]: the daemon writes it into the audit log and the status.

use std::os::unix::fs::MetadataExt as _;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use humanitl_core::ids::SessionId;

use super::credential::{
    CredentialFormat, Origin, Refused, TakeBack, Taken, WRITEBACK_POLL, take_back,
};
use super::seal::CredentialKeys;
use super::store::Depot;

/// Where an outcome goes: the daemon's audit log and status.
pub type Report = Arc<dyn Fn(&Outcome) + Send + Sync>;

/// One outcome of the guard in a session.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    /// The agent.
    pub agent: String,
    /// The session.
    pub session: SessionId,
    /// Stored, or refused.
    pub result: Result<Taken, Refused>,
}

/// What the watch of one session needs.
pub struct Writeback {
    depot: Depot,
    file: Option<&'static str>,
    format: CredentialFormat,
    session: SessionId,
    report: Report,
    /// The key of a sealed depot (HUM-523); every look decides from the
    /// depot itself whether to seal.
    keys: Option<CredentialKeys>,
}

impl std::fmt::Debug for Writeback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Writeback")
            .field("agent", &self.depot.agent())
            .field("file", &self.file)
            .field("session", &self.session)
            .finish_non_exhaustive()
    }
}

impl Writeback {
    /// The watch for `agent` in `session`, or `None` when its recipe names
    /// no credential file or no format: then nothing goes back.
    #[must_use]
    pub fn for_agent(
        depot: Depot,
        recipe: &super::DepotRecipe,
        session: SessionId,
        report: Report,
        keys: Option<CredentialKeys>,
    ) -> Option<Self> {
        Some(Self {
            depot,
            file: Some(recipe.credential_file?),
            format: recipe.credential_format?,
            session,
            report,
            keys,
        })
    }

    /// The credential file's name.
    #[must_use]
    pub fn file(&self) -> Option<&'static str> {
        self.file
    }
}

/// The state of one watch, shared by its thread and the hook at the end.
struct Watch {
    copy: PathBuf,
    writeback: Writeback,
    /// The file as the last look saw it, and the base of the next write-back.
    state: Mutex<Seen>,
    stop: AtomicBool,
}

#[derive(Default)]
struct Seen {
    /// Inode, size and modification time of the last look.
    stamp: Option<(u64, u64, i64, i64)>,
    /// The SHA-256 of the file the session works on: what it got, then what
    /// it stored.
    base: Option<String>,
    /// Whether the last look ended without a refusal; the look at the end
    /// tries a refused or failed file again even when it did not change.
    settled: bool,
}

/// Starts the watch of `copy` for the sandbox of `handle`.
pub(crate) fn watch(
    handle: &crate::SandboxHandle,
    copy: PathBuf,
    base: Option<String>,
    writeback: Writeback,
) {
    let watch = Arc::new(Watch {
        copy,
        writeback,
        state: Mutex::new(Seen {
            stamp: None,
            base,
            settled: true,
        }),
        stop: AtomicBool::new(false),
    });
    let polled = Arc::clone(&watch);
    let spawned = std::thread::Builder::new()
        .name("credential-writeback".to_owned())
        .spawn(move || polled.poll());
    let ended = Arc::clone(&watch);
    handle.before_cleanup(Box::new(move || {
        ended.stop.store(true, Ordering::SeqCst);
        ended.look_at_end();
    }));
    // Without a thread the look at the end still comes.
    drop(spawned);
}

impl Watch {
    fn poll(&self) {
        let mut next = Instant::now() + WRITEBACK_POLL;
        while !self.stop.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(250));
            if Instant::now() >= next {
                if self.stop.load(Ordering::SeqCst) {
                    break;
                }
                self.look();
                next = Instant::now() + WRITEBACK_POLL;
            }
        }
    }

    /// One look: the guard runs when the file changed since the last one.
    fn look(&self) {
        self.look_with(false);
    }

    /// The last look, before the copy goes: a file the last look refused or
    /// could not take is tried again even when it did not change.
    fn look_at_end(&self) {
        self.look_with(true);
    }

    fn look_with(&self, at_end: bool) {
        let Some(file) = self.writeback.file else {
            return;
        };
        let mut seen = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let stamp = std::fs::symlink_metadata(self.copy.join(file))
            .ok()
            .map(|meta| (meta.ino(), meta.size(), meta.mtime(), meta.mtime_nsec()));
        if stamp.is_none() || (stamp == seen.stamp && (seen.settled || !at_end)) {
            return;
        }
        seen.stamp = stamp;
        let request = TakeBack {
            depot: &self.writeback.depot,
            file,
            format: &self.writeback.format,
            session: self.writeback.session,
            base: seen.base.as_deref(),
            origin: Origin::Session,
            keys: self.writeback.keys.as_ref(),
        };
        let result = take_back(&self.copy, &request);
        seen.settled = result.is_ok();
        if let Ok(Taken::Stored { new, .. }) = &result {
            seen.base = Some(new.clone());
        }
        if matches!(result, Ok(Taken::Nothing)) {
            return;
        }
        (self.writeback.report)(&Outcome {
            agent: self.writeback.depot.agent().to_owned(),
            session: self.writeback.session,
            result,
        });
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::agent::depot::credential::{CredentialField, Expiry, TimeUnit, ValueForm};
    use crate::agent::depot::store::tests::filled;

    const FORMAT: CredentialFormat = CredentialFormat {
        fields: &[
            CredentialField {
                pointer: "/token",
                form: ValueForm::Token,
                required: true,
            },
            CredentialField {
                pointer: "/expires",
                form: ValueForm::Count,
                required: true,
            },
        ],
        expiry: Some(Expiry {
            pointer: "/expires",
            unit: TimeUnit::Seconds,
        }),
        account: None,
        one_of: &[],
        requires: &[],
        fixed: &[],
        evidence: "stand-in of the tests",
    };

    fn watch_of(depot: Depot, copy: PathBuf, seen: Arc<Mutex<Vec<Outcome>>>) -> Watch {
        Watch {
            copy,
            writeback: Writeback {
                depot,
                file: Some("cred.json"),
                format: FORMAT,
                session: SessionId::new(),
                report: Arc::new(move |outcome| seen.lock().unwrap().push(outcome.clone())),
                keys: None,
            },
            state: Mutex::new(Seen::default()),
            stop: AtomicBool::new(false),
        }
    }

    fn paths(dir: &std::path::Path) -> humanitl_config::Paths {
        humanitl_config::Paths::new(humanitl_config::Env::from_pairs([
            ("HOME", dir.join("home").to_str().unwrap()),
            ("XDG_RUNTIME_DIR", dir.join("run").to_str().unwrap()),
            ("XDG_DATA_HOME", dir.join("data").to_str().unwrap()),
        ]))
    }

    /// A session renews its credential file and ends: after the end the new
    /// file is in the depot, `0600`, and the session's copy is gone. Without
    /// the watch (`credential_writeback = false`) nothing goes back.
    #[test]
    fn a_renewal_is_in_the_depot_after_the_session_ends() {
        use std::os::unix::fs::PermissionsExt as _;
        use std::os::unix::process::ExitStatusExt as _;

        for enabled in [true, false] {
            let base = tempfile::tempdir().unwrap();
            let depot = filled(base.path(), "standin", "standin");
            let stored = depot.credentials_dir().join("cred.json");
            std::fs::write(&stored, r#"{"expires":1,"token":"a"}"#).unwrap();
            std::fs::set_permissions(&stored, std::fs::Permissions::from_mode(0o600)).unwrap();
            let session = SessionId::new();
            let scratch = crate::agent::depot::scratch::create(
                &paths(base.path()),
                session,
                &depot,
                None,
                None,
            )
            .unwrap();
            let copy = scratch.path().to_path_buf();
            let shared = Arc::new(crate::handle::Shared::default());
            let handle = crate::SandboxHandle::new(
                humanitl_core::ids::SandboxId::nil(),
                0,
                String::new(),
                Arc::clone(&shared),
            );
            let seen = Arc::new(Mutex::new(Vec::new()));
            let report_to = Arc::clone(&seen);
            let writeback = enabled.then(|| Writeback {
                depot: depot.clone(),
                file: Some("cred.json"),
                format: FORMAT,
                session,
                report: Arc::new(move |outcome: &Outcome| {
                    report_to.lock().unwrap().push(outcome.clone());
                }),
                keys: None,
            });
            scratch.hand_to_with(&handle, writeback);
            std::fs::write(copy.join("cred.json"), r#"{"token":"b","expires":2}"#).unwrap();
            std::fs::write(copy.join("settings.json"), r#"{"hooks":{}}"#).unwrap();
            shared.set_exit(std::process::ExitStatus::from_raw(0));
            assert!(!copy.exists(), "the session's copy goes with the sandbox");
            let now: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&stored).unwrap()).unwrap();
            assert_eq!(
                std::fs::read(depot.config_dir().join("settings.json")).unwrap(),
                b"{}\n"
            );
            if enabled {
                assert_eq!(now, serde_json::json!({"token": "b", "expires": 2}));
                let mode = std::fs::metadata(&stored).unwrap().permissions().mode();
                assert_eq!(mode & 0o777, 0o600);
                assert_eq!(seen.lock().unwrap().len(), 1);
            } else {
                assert_eq!(now, serde_json::json!({"token": "a", "expires": 1}));
                assert!(seen.lock().unwrap().is_empty());
            }
        }
    }

    /// A login stored by hand, `0600`.
    fn stored_by_hand(depot: &Depot, text: &str) {
        use std::os::unix::fs::PermissionsExt as _;
        let stored = depot.credentials_dir().join("cred.json");
        std::fs::write(&stored, text).unwrap();
        std::fs::set_permissions(&stored, std::fs::Permissions::from_mode(0o600)).unwrap();
    }

    /// A file the last look could not take (the depot's lock was held) is
    /// tried again at the end, although it did not change since.
    #[test]
    fn the_last_look_retries_a_file_that_failed() {
        let base = tempfile::tempdir().unwrap();
        let depot = filled(base.path(), "standin", "standin");
        stored_by_hand(&depot, r#"{"token":"z","expires":0}"#);
        let copy = base.path().join("copy");
        std::fs::create_dir(&copy).unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let watch = watch_of(depot.clone(), copy.clone(), Arc::clone(&seen));
        std::fs::write(copy.join("cred.json"), r#"{"token":"a","expires":1}"#).unwrap();
        let held = depot.lock_named("credentials").unwrap();
        watch.look();
        drop(held);
        assert!(seen.lock().unwrap()[0].result.is_err());
        // The same file, unchanged: a poll leaves it, the end takes it.
        watch.look();
        assert_eq!(seen.lock().unwrap().len(), 1);
        watch.look_at_end();
        let outcomes = seen.lock().unwrap();
        assert_eq!(outcomes.len(), 2, "{outcomes:?}");
        assert!(matches!(outcomes[1].result, Ok(Taken::Stored { .. })));
    }

    /// A changed file is taken back once; the same file again is not
    /// reported a second time, and a session renews twice in a row.
    #[test]
    fn each_change_is_looked_at_once() {
        let base = tempfile::tempdir().unwrap();
        let depot = filled(base.path(), "standin", "standin");
        let copy = base.path().join("copy");
        std::fs::create_dir(&copy).unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        stored_by_hand(&depot, r#"{"token":"z","expires":0}"#);
        let watch = watch_of(depot.clone(), copy.clone(), Arc::clone(&seen));
        watch.look();
        assert!(seen.lock().unwrap().is_empty(), "no file, nothing to say");
        std::fs::write(copy.join("cred.json"), r#"{"token":"a","expires":1}"#).unwrap();
        watch.look();
        watch.look();
        assert_eq!(seen.lock().unwrap().len(), 1);
        std::fs::write(copy.join("cred.json"), r#"{"token":"b","expires":2,"x":1}"#).unwrap();
        watch.look();
        watch.look();
        std::fs::write(copy.join("cred.json"), r#"{"token":"c","expires":3}"#).unwrap();
        watch.look();
        let outcomes = seen.lock().unwrap();
        assert_eq!(outcomes.len(), 3, "{outcomes:?}");
        assert!(matches!(outcomes[0].result, Ok(Taken::Stored { .. })));
        assert!(outcomes[1].result.is_err());
        assert!(matches!(outcomes[2].result, Ok(Taken::Stored { .. })));
        let stored = std::fs::read_to_string(depot.credentials_dir().join("cred.json")).unwrap();
        assert!(stored.contains("\"c\""), "{stored}");
    }
}
