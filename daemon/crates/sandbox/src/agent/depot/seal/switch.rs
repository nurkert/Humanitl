//! The switch of the encryption as a transaction (HUM-523, review).
//!
//! One switch holds the lock `agents/credential-seal.switch.lock` from its
//! first read to its last write, `config.toml` included, and the credential
//! lock of every depot. Before anything changes it writes the journal
//! [`JOURNAL`]: the target and every file that is not yet in the target's
//! form. Then, in this order:
//!
//! 1. switching off: `agents.encrypt_credentials = false` is written into
//!    `config.toml` first, durably, before any plain file lies in the depot;
//! 2. switching on: [`super::SEAL_STATE`] is written;
//! 3. each listed file is replaced atomically by its other form (temporary
//!    file beside the depot's directories, `rename`, `fsync`); switching on
//!    writes only sealed bytes, so no plain text is staged in the depot;
//! 4. switching off: the state goes;
//! 5. switching on: `agents.encrypt_credentials = true` is written last,
//!    after every file is sealed;
//! 6. the journal goes.
//!
//! A crash at any point leaves the journal. Until it is finished, a reader
//! takes a listed file in either form ([`pending`]), and no writer stores
//! anything ([`super::for_disk`]). [`recover`] finishes it forward: every
//! step is idempotent, so the same steps run again until the journal goes.
//! The daemon calls it before a status answer, a session start and a login
//! or configuration run. A step that fails without a crash (a full disk, a
//! `config.toml` that cannot be written) turns the switch around: a journal
//! with the old target replaces the first, and its steps run; if that fails
//! too, the journal stays for [`recover`].

use std::path::Path;

use humanitl_config::Paths;
use humanitl_core::Diagnostic;
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use super::super::credential::{self, CredentialLock};
use super::super::recipe::KNOWN;
use super::super::store::Depot;
use super::{
    CredentialKeys, LOCK_ID, SealKey, VERSION, agents_dir, counted, credential_files, is_sealed,
    open, remove_state, seal, state_present, unavailable, unopened, write_state,
};

/// The journal of a switch in progress, under `agents/`.
pub const JOURNAL: &str = "credential-seal.journal";

/// What [`JOURNAL`] holds: no secret, the target and the files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Journal {
    version: u8,
    /// Sealed (`true`) or plain.
    target: bool,
    /// `(agent, file)` of every file to bring into the target's form.
    files: Vec<(String, String)>,
}

/// What [`switch`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Switched {
    /// Whether the files are sealed now.
    pub enabled: bool,
    /// The files converted, as `agent/file`.
    pub converted: Vec<String>,
}

/// A crash point, for the tests: the switch stops there as a crash would,
/// without turning around.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Fault {
    /// After the journal is written.
    Journal,
    /// After the state is set (switching on).
    State,
    /// After the first file is replaced.
    FirstFile,
    /// After the last file, and the state gone (switching off).
    Files,
    /// After `config.toml` is written: first when switching off, last when
    /// switching on.
    Config,
}

/// The locks of a switch: its own, then every depot's credential lock.
struct Held {
    _switching: crate::agent::depot::store::Reservation,
    _depots: Vec<CredentialLock>,
}

/// Takes the switch lock, then every existing depot's credential lock.
fn hold(paths: &Paths) -> Result<Held, Diagnostic> {
    let switching = Depot::of(paths, LOCK_ID)
        .lock_named("switch")
        .map_err(|found| {
            unavailable(
                &format!(
                    "another switch of the encryption holds the depot ({})",
                    found.why
                ),
                None,
            )
        })?;
    let depots = existing(paths)
        .iter()
        .map(credential::lock)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Held {
        _switching: switching,
        _depots: depots,
    })
}

fn existing(paths: &Paths) -> Vec<Depot> {
    KNOWN
        .iter()
        .map(|agent| Depot::of(paths, agent.id))
        .filter(Depot::exists)
        .collect()
}

/// Seals (`on`) or opens every credential file of every depot, sets or
/// removes the state and writes `agents.encrypt_credentials`, as one
/// transaction (module docs). An interrupted switch is finished first.
///
/// Switching on asks the keyring every time and refuses an entry that is
/// gone, locked or another key than the one read before; a key is created
/// only when the keyring answers that it holds none and no file is sealed.
///
/// # Errors
///
/// `AGENT_020` without a keyring that answers, for a file that does not
/// open, for a plain file in a sealed depot, or when a step fails (then the
/// switch is turned around, or the journal waits for [`recover`]);
/// `AGENT_012` while a write-back holds a depot; `AGENT_013` for a depot that
/// fails its checks.
pub fn switch(paths: &Paths, keys: &CredentialKeys, on: bool) -> Result<Switched, Diagnostic> {
    switch_with(paths, keys, on, None)
}

pub(crate) fn switch_with(
    paths: &Paths,
    keys: &CredentialKeys,
    on: bool,
    fault: Option<Fault>,
) -> Result<Switched, Diagnostic> {
    let _held = hold(paths)?;
    recover_held(paths, keys)?;
    switch_held(paths, keys, on, fault)
}

/// Finishes an interrupted switch, if the journal `agents/credential-seal.journal` names one; `false` when
/// there was none.
///
/// # Errors
///
/// As [`switch`]; the journal then stays.
pub fn recover(paths: &Paths, keys: &CredentialKeys) -> Result<bool, Diagnostic> {
    if !journal_present(&agents_dir(paths)) {
        return Ok(false);
    }
    let _held = hold(paths)?;
    recover_held(paths, keys)
}

/// Brings the depots to `agents.encrypt_credentials` as `config.toml` says
/// it now, read under the switch lock: an interrupted switch is finished,
/// then a state that differs (a hand edit) is switched. Without a setting
/// (`CredentialKeys::with_setting`) nothing happens.
///
/// # Errors
///
/// As [`switch`]; `AGENT_020` also while another switch holds the lock.
pub fn reconcile(paths: &Paths, keys: &CredentialKeys) -> Result<Option<Switched>, Diagnostic> {
    if keys.setting.is_none() {
        return Ok(None);
    }
    let _held = hold(paths)?;
    recover_held(paths, keys)?;
    let wanted = keys.configured()?.unwrap_or(false);
    let settled = state_present(&agents_dir(paths)) == wanted
        && KNOWN.iter().all(|agent| {
            let (sealed, plain) = counted(&Depot::of(paths, agent.id));
            if wanted { plain == 0 } else { sealed == 0 }
        });
    if settled {
        return Ok(None);
    }
    switch_held(paths, keys, wanted, None).map(Some)
}

fn recover_held(paths: &Paths, keys: &CredentialKeys) -> Result<bool, Diagnostic> {
    let agents = agents_dir(paths);
    let Some(journal) = read_journal(&agents)? else {
        return Ok(false);
    };
    sweep_temps(paths)?;
    let key = journal_key(keys, &journal, paths)?;
    run(paths, keys, &journal, key.as_ref(), None)?;
    Ok(true)
}

/// The key a journal needs: to seal its files, or to open the sealed ones.
fn journal_key(
    keys: &CredentialKeys,
    journal: &Journal,
    paths: &Paths,
) -> Result<Option<SealKey>, Diagnostic> {
    let sealed_listed = journal.files.iter().any(|(agent, file)| {
        stored(&Depot::of(paths, agent), file)
            .ok()
            .flatten()
            .is_some_and(|bytes| is_sealed(&bytes))
    });
    if journal.target || sealed_listed {
        keys.key().map(Some)
    } else {
        Ok(None)
    }
}

fn switch_held(
    paths: &Paths,
    keys: &CredentialKeys,
    on: bool,
    fault: Option<Fault>,
) -> Result<Switched, Diagnostic> {
    let agents = agents_dir(paths);
    let state = state_present(&agents);
    let mut found = Vec::new();
    for depot in existing(paths) {
        depot.check()?;
        for (file, bytes) in credential_files(&depot)? {
            // In a sealed depot every file is sealed; one without the header
            // is damaged or of an unknown format, never plain text to seal.
            if state && !is_sealed(&bytes) {
                return Err(unopened(
                    depot.agent(),
                    &file,
                    "is not sealed, and the depot is (a damaged header or a format this daemon \
                     does not know)",
                ));
            }
            found.push((depot.clone(), file, bytes));
        }
    }
    let any_sealed = found.iter().any(|(_, _, bytes)| is_sealed(bytes));
    let key = if on {
        Some(keys.key_for_enabling(!any_sealed)?)
    } else if any_sealed {
        Some(keys.key()?)
    } else {
        None
    };
    // The state names the key it was set with: another key in the keyring
    // is refused, also when no file would show it.
    if let (true, Some(key)) = (state, &key) {
        check_key_id(&agents, key)?;
    }
    // A write-back temporary file a crash left at a depot's root holds a
    // login, in plain text before a switch on: it goes before anything
    // else, and the setting never says true while one lies there.
    sweep_temps(paths)?;
    // Every sealed file must open before anything changes.
    if let Some(key) = &key {
        for (depot, file, bytes) in &found {
            if is_sealed(bytes) {
                open(key, depot.agent(), file, bytes)
                    .map_err(|why| unopened(depot.agent(), file, &why))?;
            }
        }
    }
    let journal = Journal {
        version: VERSION,
        target: on,
        files: found
            .iter()
            .filter(|(_, _, bytes)| is_sealed(bytes) != on)
            .map(|(depot, file, _)| (depot.agent().to_owned(), file.clone()))
            .collect(),
    };
    write_journal(&agents, &journal).map_err(|why| unavailable(&why, None))?;
    if fault == Some(Fault::Journal) {
        return Err(crashed());
    }
    match run(paths, keys, &journal, key.as_ref(), fault) {
        Ok(switched) => Ok(switched),
        Err(failed) if fault.is_some() => Err(failed),
        Err(failed) => Err(turn_around(paths, keys, &journal, key.as_ref(), &failed)),
    }
}

/// After a step failed: the same files back to the old target, under a new
/// journal. Answers the finding to report.
fn turn_around(
    paths: &Paths,
    keys: &CredentialKeys,
    journal: &Journal,
    key: Option<&SealKey>,
    failed: &Diagnostic,
) -> Diagnostic {
    let back = Journal {
        target: !journal.target,
        ..journal.clone()
    };
    let reverted = write_journal(&agents_dir(paths), &back)
        .map_err(|why| unavailable(&why, None))
        .and_then(|()| run(paths, keys, &back, key, None));
    let mut found = failed.clone();
    found.why = match reverted {
        Ok(_) => format!(
            "{}; the switch was turned back, and the depot and config.toml are as before",
            found.why
        ),
        Err(again) => format!(
            "{}; turning the switch back failed too ({}), and it is finished at the next status \
             or start",
            found.why, again.why
        ),
    };
    found
}

/// The steps of a journal, each idempotent; the journal goes last.
fn run(
    paths: &Paths,
    keys: &CredentialKeys,
    journal: &Journal,
    key: Option<&SealKey>,
    fault: Option<Fault>,
) -> Result<Switched, Diagnostic> {
    let agents = agents_dir(paths);
    // Switching off says `false` first, durably, before a single plain file
    // lies in the depot: a crash later never leaves `true` with plain text.
    // Switching on says `true` last, after every file is sealed.
    if !journal.target {
        keys.write_setting(false)?;
        if fault == Some(Fault::Config) {
            return Err(crashed());
        }
    }
    if journal.target && !state_present(&agents) {
        let key = key.ok_or_else(|| unavailable("no key was read for the switch", None))?;
        write_state(&agents, key).map_err(|why| unavailable(&why, None))?;
    }
    if fault == Some(Fault::State) {
        return Err(crashed());
    }
    let mut converted = Vec::new();
    for (agent, file) in &journal.files {
        let depot = Depot::of(paths, agent);
        let Some(bytes) = stored(&depot, file)? else {
            continue;
        };
        if is_sealed(&bytes) != journal.target {
            let after = convert(&depot, file, &bytes, journal.target, key)?;
            credential::put(&depot, file, after.as_slice()).map_err(|why| {
                unavailable(
                    &format!("{agent}/{file} could not be replaced ({why})"),
                    None,
                )
            })?;
        }
        converted.push(format!("{agent}/{file}"));
        if fault == Some(Fault::FirstFile) {
            return Err(crashed());
        }
    }
    if !journal.target {
        remove_state(&agents).map_err(|why| unavailable(&why, None))?;
    }
    if fault == Some(Fault::Files) {
        return Err(crashed());
    }
    if journal.target {
        keys.write_setting(true)?;
        if fault == Some(Fault::Config) {
            return Err(crashed());
        }
    }
    remove_small(&agents, JOURNAL).map_err(|why| unavailable(&why, None))?;
    Ok(Switched {
        enabled: journal.target,
        converted,
    })
}

/// One file in the other form: sealed bytes, or the opened plain text.
fn convert(
    depot: &Depot,
    file: &str,
    bytes: &[u8],
    target: bool,
    key: Option<&SealKey>,
) -> Result<Zeroizing<Vec<u8>>, Diagnostic> {
    let key = key.ok_or_else(|| unavailable("no key was read for the switch", None))?;
    if target {
        seal(key, depot.agent(), file, bytes)
            .map(Zeroizing::new)
            .map_err(|why| unavailable(&why, None))
    } else {
        open(key, depot.agent(), file, bytes).map_err(|why| unopened(depot.agent(), file, &why))
    }
}

/// The stored bytes of one credential file; `None` without one.
fn stored(depot: &Depot, file: &str) -> Result<Option<Zeroizing<Vec<u8>>>, Diagnostic> {
    Ok(credential_files(depot)?
        .into_iter()
        .find(|(name, _)| name == file)
        .map(|(_, bytes)| bytes))
}

fn crashed() -> Diagnostic {
    unavailable("the switch stopped here, as a crash would", None)
}

/// Whether a journal lies under `agents/`.
pub(crate) fn journal_present(agents: &Path) -> bool {
    std::fs::symlink_metadata(agents.join(JOURNAL)).is_ok_and(|meta| meta.is_file())
}

/// Whether the journal of a switch in progress lists `file` of `depot`.
pub(crate) fn pending(depot: &Depot, file: &str) -> bool {
    depot
        .root()
        .parent()
        .and_then(|agents| read_journal(agents).ok().flatten())
        .is_some_and(|journal| {
            journal
                .files
                .iter()
                .any(|(agent, name)| agent == depot.agent() && name == file)
        })
}

fn read_journal(agents: &Path) -> Result<Option<Journal>, Diagnostic> {
    match std::fs::read(agents.join(JOURNAL)) {
        Ok(bytes) => {
            let journal: Journal = serde_json::from_slice(&bytes).map_err(|err| {
                unavailable(
                    &format!("the journal of an interrupted switch cannot be read ({err})"),
                    None,
                )
            })?;
            validate(agents, &journal)?;
            Ok(Some(journal))
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(unavailable(
            &format!("the journal of an interrupted switch cannot be read ({err})"),
            None,
        )),
    }
}

/// A journal names only what a switch of this daemon would: its own format
/// version, registered agents, and for each a plain file name of its
/// credential directory (the adapter's credential file or one that lies
/// there), inside `agents/`. Anything else is refused, never followed.
fn validate(agents: &Path, journal: &Journal) -> Result<(), Diagnostic> {
    let refuse = |what: String| {
        unavailable(
            &format!(
                "the journal {} of an interrupted switch is refused: {what}; it is not followed \
                 and stays for a look",
                agents.join(JOURNAL).display()
            ),
            None,
        )
    };
    if journal.version != VERSION {
        return Err(refuse(format!(
            "it has the format version {}, and this daemon writes {VERSION}",
            journal.version
        )));
    }
    for (agent, file) in &journal.files {
        let Some(known) = KNOWN.iter().find(|known| known.id == agent) else {
            return Err(refuse(format!("{agent:?} is no registered agent")));
        };
        let plain_name = super::super::adopt::relative_inside(file)
            && Path::new(file).components().count() == 1
            && !matches!(file.as_str(), "" | "." | "..");
        let dir = agents.join(known.id).join("credentials");
        let listed = known
            .recipe
            .and_then(|recipe| recipe.credential_file)
            .is_some_and(|name| super::super::recipe::credential_name(name) == file)
            || std::fs::symlink_metadata(dir.join(file)).is_ok_and(|meta| meta.is_file());
        if !plain_name || !listed || !dir.join(file).starts_with(agents) {
            return Err(refuse(format!("{file:?} is no credential file of {agent}")));
        }
    }
    Ok(())
}

/// Removes every write-back temporary file at a depot's root
/// (`.<file>.writeback`, [`credential::put`]); the caller holds every
/// credential lock, so none of them is being written.
fn sweep_temps(paths: &Paths) -> Result<(), Diagnostic> {
    for depot in existing(paths) {
        let Ok(entries) = std::fs::read_dir(depot.root()) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            if !(name.starts_with('.') && name.ends_with(".writeback")) {
                continue;
            }
            if entry.file_type().is_ok_and(|kind| kind.is_file()) {
                std::fs::remove_file(entry.path()).map_err(|err| {
                    unavailable(
                        &format!(
                            "the left-over write-back file {} cannot be removed ({err})",
                            entry.path().display()
                        ),
                        None,
                    )
                })?;
            }
        }
        let _ = super::super::adopt::sync_dir(depot.root());
    }
    Ok(())
}

/// Refuses a key whose id differs from the one the state recorded.
fn check_key_id(agents: &Path, key: &SealKey) -> Result<(), Diagnostic> {
    match super::state_key_id(agents) {
        Some(recorded) if recorded != key.id() => Err(unavailable(
            "the keyring entry humanitl/agents-depot holds another key than the one the depot \
             was encrypted with",
            None,
        )),
        _ => Ok(()),
    }
}

fn write_journal(agents: &Path, journal: &Journal) -> Result<(), String> {
    let bytes = serde_json::to_vec(journal).map_err(|err| err.to_string())?;
    write_small(agents, JOURNAL, &bytes)
}

/// A small file of the switch under `agents/`: `0600`, temporary file,
/// `rename`, `fsync` of the directory.
pub(super) fn write_small(agents: &Path, name: &str, bytes: &[u8]) -> Result<(), String> {
    super::super::adopt::create_synced(agents)
        .map_err(|err| format!("{} cannot be created ({err})", agents.display()))?;
    let temp = agents.join(format!(".{name}.tmp"));
    let _ = std::fs::remove_file(&temp);
    credential::write_new(&temp, bytes, None)
        .and_then(|()| std::fs::rename(&temp, agents.join(name)).map_err(|err| err.to_string()))
        .and_then(|()| super::super::adopt::sync_dir(agents).map_err(|err| err.to_string()))
        .inspect_err(|_| {
            let _ = std::fs::remove_file(&temp);
        })
}

/// Removes a small file of the switch; gone already is no error.
pub(super) fn remove_small(agents: &Path, name: &str) -> Result<(), String> {
    match std::fs::remove_file(agents.join(name)) {
        Ok(()) => super::super::adopt::sync_dir(agents).map_err(|err| err.to_string()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(format!("{name} cannot be removed ({err})")),
    }
}
