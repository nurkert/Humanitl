//! Der Zugangsschlüssel des LLM-Endpunkts (HUM-140).
//!
//! In `config.toml` steht **nie der Schlüssel**, sondern ein Verweis darauf
//! ([`KeyRef`], `llm.api_key_ref`). Der Schlüssel selbst liegt an einem von zwei
//! Orten, und dieses Modul ist der einzige Weg zu beiden:
//!
//! - **Im Schlüsselbund des Systems** über den Secret Service (`gnome-keyring`,
//!   `kwallet`). Gesprochen wird mit `secret-tool` aus `libsecret-tools`, dem
//!   Werkzeug der Bibliothek selbst: keine eigene Kryptographie, kein eigener
//!   D-Bus-Client. Der Schlüssel reist dabei über `stdin` hinein und über
//!   `stdout` heraus, nie über die Kommandozeile, die jeder Prozess desselben
//!   Nutzers in `/proc/<pid>/cmdline` lesen kann.
//! - **In einer Datei mit `0600`** unter `$XDG_DATA_HOME/humanitl/keys`, wenn
//!   der Secret Service auf dieser Maschine nicht antwortet (kein Desktop, kein
//!   Sitzungsbus, kein `secret-tool`). Das ist der Rückfall, und er wird
//!   gemeldet (`CONFIG_019`), nicht verschwiegen.
//!
//! Der Schlüssel im Speicher ist ein [`ApiKey`]: Er wird beim Fallenlassen
//! überschrieben, und seine `Debug`-Form ist `••••`. Ein `tracing`-Feld oder ein
//! Befund, der ihn aus Versehen formatiert, zeigt also nichts.
//!
//! Was mit dem Schlüssel danach geschieht — als Datei in die Sandbox, als
//! `Authorization` an die Probe, maskiert in der Aufzeichnung —, steht in
//! `docs/SECURITY.md`, Abschnitt „Der Zugangsschlüssel des Modells".

use std::ffi::OsString;
use std::fmt;
use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::{self, Read, Write as _};
use std::os::unix::fs::{DirBuilderExt as _, MetadataExt as _, OpenOptionsExt as _};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use humanitl_core::diagnostics::codes::{CONFIG_019, CONFIG_020, LLM_009};
use humanitl_core::shell::shell_path;
use humanitl_core::{Diagnostic, FixAction, Severity};
use zeroize::Zeroizing;

use crate::paths::{DIR_MODE, FILE_MODE, Paths};
use crate::private_dir::process_uid;

/// Das Schema eines Verweises in den Schlüsselbund.
pub const SECRET_SERVICE_SCHEME: &str = "secret-service";

/// Das Schema eines Verweises auf die Datei des Rückfalls.
pub const FILE_SCHEME: &str = "file";

/// Der Name, unter dem Humanitl seine Einträge im Schlüsselbund ablegt
/// (Attribut `service`).
pub const SECRET_SERVICE_APP: &str = "humanitl";

/// Der Name des einen Schlüssels, den das Formular setzt.
pub const DEFAULT_KEY_NAME: &str = "llm";

/// Was an der Stelle des Schlüssels steht, wo er sonst sichtbar würde.
pub const MASK: &str = "••••";

/// Die größte Länge eines Schlüssels in Bytes. Echte Schlüssel sind weit
/// kürzer; die Grenze hält nur einen Client davon ab, Megabytes in den
/// Schlüsselbund zu schreiben.
pub const MAX_KEY_BYTES: usize = 4096;

/// Die kleinste Länge eines Schlüssels, in Zeichen.
///
/// Ein kürzerer Wert lässt sich weder sicher maskieren noch sicher finden:
/// `abc` stünde als Teil jedes dritten Worts in jeder Kopfzeile, und die
/// Aufzeichnung könnte ihn nur als ganzes Wort ersetzen (`?key=abc` bliebe
/// stehen). Ein Schlüssel, der sich nicht schützen lässt, wird deshalb gar
/// nicht erst gespeichert (HUM-140, Review). Jeder bekannte Anbieter liegt
/// deutlich darüber; ein lokaler Server nimmt einen selbst gewählten.
pub const MIN_KEY_CHARS: usize = 16;

/// Die längste Zeit, die `secret-tool` für eine Antwort bekommt.
///
/// Ein gesperrter Schlüsselbund fragt den Menschen nach seinem Passwort, und
/// das darf dauern; ein Sitzungsbus, der nie antwortet, darf den Start einer
/// Sitzung aber nicht für immer anhalten.
pub const TOOL_TIMEOUT: Duration = Duration::from_secs(20);

/// So viele Zeichen der Fehlerausgabe von `secret-tool` stehen im Befund.
const SHOWN_STDERR_CHARS: usize = 200;

/// Wie oft [`run_tool`] nachsieht, ob `secret-tool` fertig ist.
const POLL: Duration = Duration::from_millis(20);

/// Zähler für die Namen der Nebendateien beim Schreiben.
static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Ein Zugangsschlüssel im Speicher.
///
/// Er lässt sich nur über [`ApiKey::expose`] lesen, wird beim Fallenlassen
/// überschrieben und zeigt sich in `Debug` als [`MASK`]. `Display` gibt es mit
/// Absicht nicht: Wer ihn formatieren will, muss `expose` schreiben, und das
/// fällt in jedem Review auf.
#[derive(Clone, PartialEq, Eq)]
pub struct ApiKey(Zeroizing<String>);

impl ApiKey {
    /// Ein Schlüssel aus dem Text, den ein Mensch eingegeben hat.
    ///
    /// # Errors
    ///
    /// `CONFIG_020`, wenn der Text leer ist, kürzer als [`MIN_KEY_CHARS`],
    /// länger als [`MAX_KEY_BYTES`], oder Leerraum oder Steuerzeichen trägt. Ein solcher Text wäre in einer
    /// Kopfzeile `Authorization` kein gültiger Wert, und ein Leerzeichen am
    /// Ende stammt meist aus der Zwischenablage, nicht vom Anbieter.
    pub fn new(text: &str) -> Result<Self, Diagnostic> {
        let refuse = |why: &str| {
            Diagnostic::builder(CONFIG_020, Severity::Error)
                .why(format!("the access key was not stored: {why}"))
                .build()
        };
        if text.is_empty() {
            return Err(refuse("it is empty"));
        }
        let chars = text.chars().count();
        if chars < MIN_KEY_CHARS {
            return Err(Diagnostic::builder(CONFIG_020, Severity::Error)
                .why(format!(
                    "the access key was not stored: it has {chars} characters, and a key needs at \
                     least {MIN_KEY_CHARS}; a shorter value cannot be kept out of the recording \
                     reliably"
                ))
                .fix(FixAction::CopyCommand("openssl rand -hex 24".to_owned()))
                .build());
        }
        if text.len() > MAX_KEY_BYTES {
            return Err(refuse(&format!(
                "it is {} bytes long, and a key has at most {MAX_KEY_BYTES}",
                text.len()
            )));
        }
        if text.chars().any(|c| c.is_whitespace() || c.is_control()) {
            return Err(refuse(
                "it contains white space or a control character between its characters; blanks \
                 and quotes around a pasted key are removed, but a key is one word, so check \
                 that only the key was copied",
            ));
        }
        Ok(Self(Zeroizing::new(text.to_owned())))
    }

    /// Ein Schlüssel aus dem Text, den ein Mensch eingefügt hat (HUM-283).
    ///
    /// Erst fallen Leerraum außen und je ein Anführungszeichen am Anfang und
    /// am Ende weg ([`crate::pasted::strip_paste_debris`]): `"sk-…"` aus einer
    /// Konfigurationsdatei ist derselbe Schlüssel wie `sk-…`, und mit den
    /// Anführungszeichen antwortete der Server `401`. Was danach bleibt, prüft
    /// [`ApiKey::new`]; Leerraum im Inneren lehnt es ab, ohne den Schlüssel zu
    /// wiederholen.
    ///
    /// Diesen Weg nehmen die Stellen, an denen ein Mensch einen Schlüssel
    /// übergibt (`LlmKey` beim Setzen, der Schlüssel einer Probe), und auch
    /// das Lesen aus dem Speicher: Ein Schlüssel, den eine ältere Fassung
    /// samt Anführungszeichen abgelegt hat, wirkt so ohne neues Setzen.
    ///
    /// # Errors
    ///
    /// Wie [`ApiKey::new`].
    pub fn from_pasted(text: &str) -> Result<Self, Diagnostic> {
        Self::new(crate::pasted::strip_paste_debris(text))
    }

    /// Der Schlüssel als Text, für die eine Stelle, die ihn braucht.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ApiKey({MASK})")
    }
}

/// Wo ein Zugangsschlüssel liegt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreKind {
    /// Im Schlüsselbund des Systems.
    SecretService,
    /// In der Datei des Rückfalls.
    File,
}

impl StoreKind {
    /// Der Name, wie er in Verweisen und Antworten steht.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SecretService => SECRET_SERVICE_SCHEME,
            Self::File => FILE_SCHEME,
        }
    }
}

/// Ein Verweis auf einen Zugangsschlüssel, wie er in `llm.api_key_ref` steht.
///
/// Zwei Formen: `secret-service:humanitl/<name>` und `file:<name>`. Der Name
/// besteht aus 1 bis 32 Zeichen `a-z`, `0-9`, `-` und `_`; damit kann ein
/// Verweis weder aus dem Verzeichnis der Schlüssel herausführen noch einen
/// fremden Eintrag im Schlüsselbund benennen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyRef {
    store: StoreKind,
    name: String,
}

impl KeyRef {
    /// Der Verweis auf den Eintrag `name` im Schlüsselbund.
    ///
    /// `None`, wenn `name` kein gültiger Name ist.
    #[must_use]
    pub fn secret_service(name: &str) -> Option<Self> {
        is_key_name(name).then(|| Self {
            store: StoreKind::SecretService,
            name: name.to_owned(),
        })
    }

    /// Der Verweis auf die Datei `name` im Verzeichnis der Schlüssel.
    ///
    /// `None`, wenn `name` kein gültiger Name ist.
    #[must_use]
    pub fn file(name: &str) -> Option<Self> {
        is_key_name(name).then(|| Self {
            store: StoreKind::File,
            name: name.to_owned(),
        })
    }

    /// Liest einen Verweis; `None`, wenn der Text keiner ist.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        if let Some(rest) = text.strip_prefix("secret-service:") {
            let name = rest.strip_prefix("humanitl/")?;
            return Self::secret_service(name);
        }
        Self::file(text.strip_prefix("file:")?)
    }

    /// Wo der Schlüssel liegt.
    #[must_use]
    pub const fn store(&self) -> StoreKind {
        self.store
    }

    /// Der Name des Schlüssels.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl fmt::Display for KeyRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.store {
            StoreKind::SecretService => {
                write!(
                    f,
                    "{SECRET_SERVICE_SCHEME}:{SECRET_SERVICE_APP}/{}",
                    self.name
                )
            }
            StoreKind::File => write!(f, "{FILE_SCHEME}:{}", self.name),
        }
    }
}

/// Ob `name` ein Name ist, den ein Verweis tragen darf.
fn is_key_name(name: &str) -> bool {
    (1..=32).contains(&name.len())
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

/// Was [`KeyStore::store`] getan hat.
#[derive(Debug)]
pub struct Stored {
    /// Der Verweis, der jetzt in `llm.api_key_ref` gehört.
    pub key_ref: KeyRef,
    /// Was dazu zu sagen ist (`CONFIG_019`): warum der Schlüssel in der Datei
    /// liegt und nicht im Schlüsselbund, oder dass nach dem Ablegen im
    /// Schlüsselbund eine alte Datei des Rückfalls stehen blieb. `None`, wenn
    /// alles aufgeräumt ist.
    pub note: Option<Diagnostic>,
}

/// Der Zugang zu beiden Orten eines Schlüssels.
///
/// Billig zu klonen. `secret_tool` ist das Programm, mit dem der Schlüsselbund
/// gesprochen wird; Tests setzen dort eine Attrappe ein
/// ([`KeyStore::with_secret_tool`]) und fassen den Schlüsselbund des Menschen
/// nie an.
#[derive(Debug, Clone)]
pub struct KeyStore {
    key_dir: PathBuf,
    secret_tool: Option<PathBuf>,
    timeout: Duration,
    uid: u32,
}

impl KeyStore {
    /// Der Speicher für diese Pfade: Dateien unter [`Paths::key_dir`], und
    /// `secret-tool`, wie es der Suchpfad aus [`Paths::env`] findet.
    #[must_use]
    pub fn for_paths(paths: &Paths) -> Self {
        let secret_tool = paths
            .env()
            .non_empty("PATH")
            .and_then(|search| find_program(search, "secret-tool"));
        Self {
            key_dir: paths.key_dir(),
            secret_tool,
            timeout: TOOL_TIMEOUT,
            uid: process_uid(),
        }
    }

    /// Derselbe Speicher mit diesem Programm für den Schlüsselbund, oder
    /// ohne Schlüsselbund (`None`).
    #[must_use]
    pub fn with_secret_tool(mut self, program: Option<PathBuf>) -> Self {
        self.secret_tool = program;
        self
    }

    /// Derselbe Speicher mit einer anderen Frist für `secret-tool`.
    #[must_use]
    pub const fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Das Verzeichnis der Dateien des Rückfalls.
    #[must_use]
    pub fn key_dir(&self) -> &Path {
        &self.key_dir
    }

    /// Die Datei des Rückfalls zu einem Namen.
    #[must_use]
    pub fn file_path(&self, name: &str) -> PathBuf {
        self.key_dir.join(format!("{name}.key"))
    }

    /// Holt den Schlüssel, auf den `key_ref` zeigt.
    ///
    /// # Errors
    ///
    /// `LLM_009`, wenn der Speicher keinen Schlüssel dazu herausgibt, mit dem
    /// Grund und dem Befehl, der ihn neu hinterlegt.
    pub fn lookup(&self, key_ref: &KeyRef) -> Result<ApiKey, Diagnostic> {
        match key_ref.store() {
            StoreKind::SecretService => self.lookup_secret_service(key_ref),
            StoreKind::File => self.lookup_file(key_ref),
        }
    }

    /// Legt `key` ab: zuerst im Schlüsselbund, sonst in der Datei.
    ///
    /// Der Eintrag am jeweils anderen Ort wird danach entfernt, damit kein
    /// alter Schlüssel liegen bleibt, den niemand mehr meint.
    ///
    /// # Errors
    ///
    /// `CONFIG_020`, wenn weder der Schlüsselbund noch die Datei ihn nehmen.
    pub fn store(&self, key: &ApiKey) -> Result<Stored, Diagnostic> {
        let reason = match self.store_secret_service(DEFAULT_KEY_NAME, key) {
            Ok(()) => {
                // Ein Rest aus einer Zeit ohne Schlüsselbund liegt sonst als
                // Klartext neben dem Eintrag, der jetzt gilt; bleibt er
                // stehen, sagt der Befund es, statt es zu verschweigen.
                let note = self.remove_file(DEFAULT_KEY_NAME).err().map(|why| {
                    Diagnostic::builder(CONFIG_019, Severity::Warning)
                        .why(format!(
                            "the access key is in the system keyring now, and the old fallback \
                             file could not be removed ({why}); it still holds an earlier key \
                             in plain text"
                        ))
                        .fix(FixAction::CopyCommand(format!(
                            "rm {}",
                            shell_path(&self.file_path(DEFAULT_KEY_NAME))
                        )))
                        .build()
                });
                return Ok(Stored {
                    key_ref: default_ref(StoreKind::SecretService),
                    note,
                });
            }
            Err(reason) => reason,
        };
        self.write_file(DEFAULT_KEY_NAME, key).map_err(|why| {
            Diagnostic::builder(CONFIG_020, Severity::Error)
                .why(format!(
                    "the access key was not stored: the system keyring did not take it \
                     ({reason}), and the fallback file could not be written ({why})"
                ))
                .build()
        })?;
        Ok(Stored {
            key_ref: default_ref(StoreKind::File),
            note: Some(
                Diagnostic::builder(CONFIG_019, Severity::Warning)
                    .why(format!(
                        "the system keyring did not take the access key ({reason}); it is \
                         stored in {} with mode 0600 instead, readable by this account only",
                        self.file_path(DEFAULT_KEY_NAME).display()
                    ))
                    .fix(FixAction::CopyCommand(
                        "sudo apt-get install libsecret-tools".to_owned(),
                    ))
                    .build(),
            ),
        })
    }

    /// Entfernt den Schlüssel, auf den `key_ref` zeigt. Fehlt er, ist das
    /// kein Fehler.
    ///
    /// # Errors
    ///
    /// `CONFIG_020`, wenn der Eintrag noch da ist und sich nicht löschen ließ.
    pub fn clear(&self, key_ref: &KeyRef) -> Result<(), Diagnostic> {
        let result = match key_ref.store() {
            StoreKind::SecretService => self.clear_secret_service(key_ref.name()),
            StoreKind::File => self.remove_file(key_ref.name()),
        };
        result.map_err(|why| {
            Diagnostic::builder(CONFIG_020, Severity::Error)
                .why(format!("the access key {key_ref} was not removed: {why}"))
                .build()
        })
    }

    fn lookup_secret_service(&self, key_ref: &KeyRef) -> Result<ApiKey, Diagnostic> {
        let Some(tool) = &self.secret_tool else {
            return Err(unavailable(
                key_ref,
                "secret-tool is not installed, so the system keyring cannot be asked",
                Some(FixAction::CopyCommand(
                    "sudo apt-get install libsecret-tools".to_owned(),
                )),
            ));
        };
        let args = attributes("lookup", key_ref.name());
        let output = run_tool(tool, &args, None, self.timeout)
            .map_err(|why| unavailable(key_ref, &why, Some(set_again())))?;
        if !output.status.success() {
            return Err(unavailable(
                key_ref,
                &format!(
                    "the system keyring holds no such entry ({})",
                    describe(&output)
                ),
                Some(set_again()),
            ));
        }
        // Gelesen wird an Ort und Stelle: `stdout` wird beim Fallenlassen
        // überschrieben, eine Kopie als `String` würde es nicht.
        let text = std::str::from_utf8(&output.stdout).map_err(|_not_text| {
            unavailable(key_ref, "the keyring entry is not text", Some(set_again()))
        })?;
        // `secret-tool` hängt einen Zeilenumbruch an, wenn es an ein Terminal
        // schreibt; ein Schlüssel trägt keinen Leerraum (`ApiKey::new`). Ein
        // Eintrag, den eine ältere Fassung samt Anführungszeichen abgelegt hat,
        // wirkt trotzdem: dieselbe Bereinigung wie beim Setzen (HUM-283).
        ApiKey::from_pasted(text).map_err(|_refused| {
            unavailable(
                key_ref,
                "the keyring entry is not a usable key",
                Some(set_again()),
            )
        })
    }

    fn store_secret_service(&self, name: &str, key: &ApiKey) -> Result<(), String> {
        let tool = self
            .secret_tool
            .as_ref()
            .ok_or_else(|| "secret-tool is not installed".to_owned())?;
        let mut args = vec![
            OsString::from("store"),
            OsString::from("--label"),
            OsString::from(format!("Humanitl access key ({name})")),
        ];
        args.extend(attributes("", name).into_iter().skip(1));
        let output = run_tool(tool, &args, Some(key.expose().as_bytes()), self.timeout)?;
        if output.status.success() {
            Ok(())
        } else {
            Err(describe(&output))
        }
    }

    fn clear_secret_service(&self, name: &str) -> Result<(), String> {
        let Some(tool) = &self.secret_tool else {
            return Err("secret-tool is not installed".to_owned());
        };
        let output = run_tool(tool, &attributes("clear", name), None, self.timeout)?;
        // `secret-tool clear` endet auch ohne passenden Eintrag mit 0; ein
        // anderer Code ist ein Fehler des Schlüsselbunds.
        if output.status.success() {
            Ok(())
        } else {
            Err(describe(&output))
        }
    }

    fn lookup_file(&self, key_ref: &KeyRef) -> Result<ApiKey, Diagnostic> {
        let path = self.file_path(key_ref.name());
        let mut file = OpenOptions::new()
            .read(true)
            .custom_flags(nofollow_flag())
            .open(&path)
            .map_err(|error| {
                unavailable(
                    key_ref,
                    &format!("{} cannot be opened ({error})", path.display()),
                    Some(set_again()),
                )
            })?;
        let meta = file.metadata().map_err(|error| {
            unavailable(
                key_ref,
                &format!("{} cannot be read ({error})", path.display()),
                Some(set_again()),
            )
        })?;
        if !meta.is_file() || meta.uid() != self.uid {
            return Err(unavailable(
                key_ref,
                &format!(
                    "{} is not a regular file of this account (uid {}); a key another account \
                     placed there is not used",
                    path.display(),
                    meta.uid()
                ),
                None,
            ));
        }
        let mode = meta.mode() & 0o777;
        if mode & 0o077 != 0 {
            return Err(unavailable(
                key_ref,
                &format!(
                    "{} is mode {mode:04o}, readable by others; the key is not used until the \
                     file is 0600",
                    path.display()
                ),
                Some(FixAction::CopyCommand(format!(
                    "chmod 600 {}",
                    shell_path(&path)
                ))),
            ));
        }
        let mut bytes = Zeroizing::new(Vec::new());
        Read::take(
            &mut file,
            u64::try_from(MAX_KEY_BYTES).unwrap_or(u64::MAX) + 1,
        )
        .read_to_end(&mut bytes)
        .map_err(|error| {
            unavailable(
                key_ref,
                &format!("{} cannot be read ({error})", path.display()),
                Some(set_again()),
            )
        })?;
        let text = std::str::from_utf8(&bytes).map_err(|_not_text| {
            unavailable(key_ref, "the key file is not text", Some(set_again()))
        })?;
        ApiKey::from_pasted(text).map_err(|_refused| {
            unavailable(
                key_ref,
                &format!("{} does not hold a usable key", path.display()),
                Some(set_again()),
            )
        })
    }

    fn write_file(&self, name: &str, key: &ApiKey) -> Result<(), String> {
        self.ensure_key_dir()?;
        let path = self.file_path(name);
        let tmp = self.key_dir.join(format!(
            ".{name}.key.tmp-{}-{}",
            std::process::id(),
            TMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let written = (|| -> io::Result<()> {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(FILE_MODE)
                .custom_flags(nofollow_flag())
                .open(&tmp)?;
            file.write_all(key.expose().as_bytes())?;
            file.sync_all()?;
            fs::rename(&tmp, &path)?;
            File::open(&self.key_dir)?.sync_all()
        })();
        written.map_err(|error| {
            let _ignored = fs::remove_file(&tmp);
            format!("{}: {error}", path.display())
        })
    }

    fn remove_file(&self, name: &str) -> Result<(), String> {
        let path = self.file_path(name);
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!("{}: {error}", path.display())),
        }
    }

    /// Legt das Verzeichnis der Schlüssel mit `0700` an, oder übernimmt ein
    /// vorhandenes nur, wenn es ein Verzeichnis dieses Kontos ist.
    fn ensure_key_dir(&self) -> Result<(), String> {
        DirBuilder::new()
            .recursive(true)
            .mode(DIR_MODE)
            .create(&self.key_dir)
            .map_err(|error| format!("{}: {error}", self.key_dir.display()))?;
        let meta = fs::symlink_metadata(&self.key_dir)
            .map_err(|error| format!("{}: {error}", self.key_dir.display()))?;
        if !meta.is_dir() || meta.uid() != self.uid {
            return Err(format!(
                "{} is not a directory of this account",
                self.key_dir.display()
            ));
        }
        if meta.mode() & 0o077 != 0 {
            fs::set_permissions(
                &self.key_dir,
                std::os::unix::fs::PermissionsExt::from_mode(DIR_MODE),
            )
            .map_err(|error| format!("{}: {error}", self.key_dir.display()))?;
        }
        Ok(())
    }
}

/// Der Verweis auf den einen Schlüssel des Formulars an diesem Ort.
fn default_ref(store: StoreKind) -> KeyRef {
    KeyRef {
        store,
        name: DEFAULT_KEY_NAME.to_owned(),
    }
}

/// Die Argumente für `secret-tool <command>` mit den Attributen des Eintrags.
fn attributes(command: &str, name: &str) -> Vec<OsString> {
    [command, "service", SECRET_SERVICE_APP, "account", name]
        .into_iter()
        .map(OsString::from)
        .collect()
}

/// `LLM_009` mit Grund und Vorschlag.
fn unavailable(key_ref: &KeyRef, why: &str, fix: Option<FixAction>) -> Diagnostic {
    let builder = Diagnostic::builder(LLM_009, Severity::Error).why(format!(
        "llm.api_key_ref names the access key {key_ref}, and it is not available: {why}. The \
         agent is not started without the key you entered."
    ));
    match fix {
        Some(fix) => builder.fix(fix).build(),
        None => builder.build(),
    }
}

/// Der Vorschlag, den Schlüssel neu zu hinterlegen.
fn set_again() -> FixAction {
    FixAction::CopyCommand("humanitl llm key set".to_owned())
}

/// `O_NOFOLLOW`: Ein Symlink an der Stelle der Datei wird nicht verfolgt.
fn nofollow_flag() -> i32 {
    i32::try_from(rustix::fs::OFlags::NOFOLLOW.bits()).unwrap_or(0)
}

/// Sucht `program` in einem Suchpfad.
fn find_program(search: &str, program: &str) -> Option<PathBuf> {
    search
        .split(':')
        .filter(|dir| !dir.is_empty())
        .map(|dir| Path::new(dir).join(program))
        .find(|candidate| {
            fs::metadata(candidate).is_ok_and(|meta| meta.is_file() && meta.mode() & 0o111 != 0)
        })
}

/// Was `secret-tool` geantwortet hat.
struct ToolOutput {
    status: ExitStatus,
    /// Kann den Schlüssel tragen und wird deshalb überschrieben.
    stdout: Zeroizing<Vec<u8>>,
    stderr: Vec<u8>,
}

/// Der Ausgang eines Aufrufs als kurzer Satz, ohne `stdout`.
fn describe(output: &ToolOutput) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr: String = stderr
        .chars()
        .filter(|c| !c.is_control())
        .take(SHOWN_STDERR_CHARS)
        .collect();
    let code = output
        .status
        .code()
        .map_or_else(|| "a signal".to_owned(), |code| code.to_string());
    if stderr.trim().is_empty() {
        format!("secret-tool ended with {code}")
    } else {
        format!("secret-tool ended with {code}: {}", stderr.trim())
    }
}

/// Startet `secret-tool` mit `args`, schreibt `input` auf `stdin` und wartet
/// höchstens `timeout` auf das Ende.
///
/// Blockierend; der Daemon ruft es auf einem eigenen Faden. Läuft die Frist ab,
/// wird das Programm beendet.
fn run_tool(
    program: &Path,
    args: &[OsString],
    input: Option<&[u8]>,
    timeout: Duration,
) -> Result<ToolOutput, String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("{} could not be started ({error})", program.display()))?;
    if let Some(mut stdin) = child.stdin.take() {
        // Ein Programm, das `stdin` nicht liest, schließt es; dann zählt allein
        // sein Ausgang. Der Schlüssel ist kürzer als der Puffer einer Pipe,
        // also blockiert das Schreiben nicht.
        let _ignored = stdin.write_all(input.unwrap_or_default());
    }
    let status = wait_with_timeout(&mut child, timeout)?;
    let mut stdout = Zeroizing::new(Vec::new());
    if let Some(mut out) = child.stdout.take() {
        let _ignored = out.read_to_end(&mut stdout);
    }
    let mut stderr = Vec::new();
    if let Some(mut err) = child.stderr.take() {
        let _ignored = Read::take(&mut err, 4096).read_to_end(&mut stderr);
    }
    Ok(ToolOutput {
        status,
        stdout,
        stderr,
    })
}

/// Wartet auf `child`, höchstens `timeout` lang, und beendet es danach.
fn wait_with_timeout(child: &mut Child, timeout: Duration) -> Result<ExitStatus, String> {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) if Instant::now() >= deadline => {
                let _ignored = child.kill();
                let _ignored = child.wait();
                return Err(format!(
                    "secret-tool did not answer within {} s; the session bus or the keyring \
                     does not respond",
                    timeout.as_secs()
                ));
            }
            Ok(None) => std::thread::sleep(POLL),
            Err(error) => return Err(format!("secret-tool could not be awaited ({error})")),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::os::unix::fs::PermissionsExt as _;

    use super::*;
    use crate::test_support::write_executable;

    /// Eine Attrappe von `secret-tool`, die ihre Einträge als Dateien in
    /// `dir/store` ablegt. Der Schlüsselbund des Menschen bleibt unberührt.
    fn fake_tool(dir: &Path) -> PathBuf {
        let store = dir.join("store");
        fs::create_dir_all(&store).unwrap();
        let script = dir.join("secret-tool");
        write_executable(
            &script,
            &format!(
                "#!/bin/sh\nstore='{}'\nfor last; do :; done\nprintf '%s\\n' \"$*\" >> '{}'\n\
                 case \"$1\" in\n  store) cat > \"$store/$last\" ;;\n  \
                 lookup) [ -f \"$store/$last\" ] || exit 1; cat \"$store/$last\" ;;\n  \
                 clear) rm -f \"$store/$last\" ;;\nesac\n",
                store.display(),
                dir.join("argv.log").display()
            ),
        );
        script
    }

    /// Eine Attrappe, deren Sitzungsbus nie antwortet: Sie scheitert immer.
    fn broken_tool(dir: &Path) -> PathBuf {
        let script = dir.join("secret-tool-broken");
        write_executable(
            &script,
            "#!/bin/sh\necho 'Cannot autolaunch D-Bus without X11 $DISPLAY' >&2\nexit 1\n",
        );
        script
    }

    fn store_in(dir: &Path, tool: Option<PathBuf>) -> KeyStore {
        KeyStore {
            key_dir: dir.join("keys"),
            secret_tool: tool,
            timeout: Duration::from_secs(5),
            uid: process_uid(),
        }
    }

    const SECRET: &str = "sk-test-0123456789abcdef";

    #[test]
    fn a_reference_has_two_forms_and_nothing_else() {
        assert_eq!(
            KeyRef::parse("secret-service:humanitl/llm").unwrap(),
            KeyRef::secret_service("llm").unwrap()
        );
        assert_eq!(
            KeyRef::parse("file:llm").unwrap(),
            KeyRef::file("llm").unwrap()
        );
        for refused in [
            "",
            "llm",
            "secret-service:llm",
            "secret-service:other/llm",
            "file:",
            "file:../audit",
            "file:a/b",
            "file:LLM",
            "keyring:llm",
        ] {
            assert!(KeyRef::parse(refused).is_none(), "{refused:?}");
        }
        assert_eq!(
            KeyRef::secret_service("llm").unwrap().to_string(),
            "secret-service:humanitl/llm"
        );
        assert_eq!(KeyRef::file("llm").unwrap().to_string(), "file:llm");
    }

    #[test]
    fn the_key_never_shows_in_its_debug_form() {
        let key = ApiKey::new(SECRET).unwrap();
        let shown = format!("{key:?} {:?}", Some(&key));
        assert!(!shown.contains(SECRET), "{shown}");
        assert!(shown.contains(MASK), "{shown}");
    }

    #[test]
    fn a_key_with_white_space_or_nothing_is_config_020() {
        for refused in [
            "",
            "sk-0123456789abcdef 2",
            "sk-0123456789abcdef\n",
            "\tsk-0123456789abcdef",
        ] {
            let diagnostic = ApiKey::new(refused).unwrap_err();
            assert_eq!(diagnostic.code, CONFIG_020, "{refused:?}");
        }
        let long = "k".repeat(MAX_KEY_BYTES + 1);
        assert_eq!(ApiKey::new(&long).unwrap_err().code, CONFIG_020);
    }

    /// Ein eingefügter Schlüssel verliert Anführungszeichen und Leerraum
    /// außen und ist danach derselbe wie ohne; Leerraum im Inneren wird
    /// abgelehnt, und der Befund wiederholt den Schlüssel nicht (HUM-283).
    #[test]
    fn a_pasted_key_loses_its_quotes_and_inner_white_space_is_refused() {
        for pasted in [
            format!("\"{SECRET}\""),
            format!("'{SECRET}'"),
            format!("{SECRET}\""),
            format!("  {SECRET}\n"),
            format!("\" {SECRET} \"\r\n"),
        ] {
            let key = ApiKey::from_pasted(&pasted).unwrap();
            assert!(key.expose() == SECRET, "a pasted variant was not cleaned");
        }
        let inner = "sk-live-key 0123456789";
        let refused = ApiKey::from_pasted(&format!("\"{inner}\"")).unwrap_err();
        assert_eq!(refused.code, CONFIG_020);
        assert!(refused.why.contains("white space"), "{}", refused.why);
        assert!(
            !refused.why.contains("sk-live"),
            "the refusal repeats the key"
        );
        assert!(
            !refused.why.contains("0123456789"),
            "the refusal repeats the key"
        );
    }

    /// Ein Schlüssel unter 16 Zeichen ließe sich nicht verlässlich aus der
    /// Aufzeichnung halten (`?key=abc`) und wird abgelehnt, mit Grund und
    /// Ausweg; 16 Zeichen gehen (HUM-140, Review).
    #[test]
    fn a_key_too_short_to_mask_is_refused_with_why_and_fix() {
        let diagnostic = ApiKey::new("abc").unwrap_err();
        assert_eq!(diagnostic.code, CONFIG_020);
        assert!(diagnostic.why.contains("at least 16"), "{}", diagnostic.why);
        assert!(diagnostic.fix.is_some());
        let refused = ApiKey::new(&"k".repeat(MIN_KEY_CHARS - 1)).unwrap_err();
        assert_eq!(refused.code, CONFIG_020);
        assert!(ApiKey::new(&"k".repeat(MIN_KEY_CHARS)).is_ok());
    }

    #[test]
    fn the_keyring_takes_the_key_over_stdin_and_never_over_argv() {
        let dir = tempfile::tempdir().unwrap();
        let tool = fake_tool(dir.path());
        let store = store_in(dir.path(), Some(tool));
        let stored = store.store(&ApiKey::new(SECRET).unwrap()).unwrap();
        assert_eq!(stored.key_ref, KeyRef::secret_service("llm").unwrap());
        assert!(stored.note.is_none());

        let argv = fs::read_to_string(dir.path().join("argv.log")).unwrap();
        assert!(argv.contains("store --label"), "{argv}");
        assert!(!argv.contains(SECRET), "the key reached argv: {argv}");
        assert_eq!(
            store.lookup(&stored.key_ref).unwrap().expose(),
            SECRET,
            "the key comes back over stdout"
        );
        assert!(
            !store.file_path("llm").exists(),
            "no plain file next to the keyring"
        );
    }

    #[test]
    fn without_a_keyring_the_key_lands_in_a_0600_file_and_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(dir.path(), Some(broken_tool(dir.path())));
        let stored = store.store(&ApiKey::new(SECRET).unwrap()).unwrap();
        assert_eq!(stored.key_ref, KeyRef::file("llm").unwrap());
        let fallback = stored.note.expect("the fallback is reported");
        assert_eq!(fallback.code, CONFIG_019);
        assert!(
            fallback.why.contains("Cannot autolaunch D-Bus"),
            "{}",
            fallback.why
        );

        let path = store.file_path("llm");
        let mode = fs::metadata(&path).unwrap().mode() & 0o777;
        assert_eq!(mode, 0o600);
        let dir_mode = fs::metadata(store.key_dir()).unwrap().mode() & 0o777;
        assert_eq!(dir_mode, 0o700);
        assert_eq!(store.lookup(&stored.key_ref).unwrap().expose(), SECRET);
    }

    /// Ein Schlüssel, den eine ältere Fassung samt Anführungszeichen und
    /// Zeilenumbruch abgelegt hat, wirkt beim Lesen, aus dem Schlüsselbund
    /// wie aus der Datei (HUM-283).
    #[test]
    fn a_key_stored_with_quotes_by_an_older_version_still_works() {
        let quoted = format!("\"{SECRET}\"\n");

        let dir = tempfile::tempdir().unwrap();
        let store = store_in(dir.path(), Some(fake_tool(dir.path())));
        let stored = store.store(&ApiKey::new(SECRET).unwrap()).unwrap();
        fs::write(dir.path().join("store").join("llm"), &quoted).unwrap();
        let key = store.lookup(&stored.key_ref).unwrap();
        assert!(key.expose() == SECRET, "the keyring entry was not cleaned");

        let dir = tempfile::tempdir().unwrap();
        let store = store_in(dir.path(), Some(broken_tool(dir.path())));
        let stored = store.store(&ApiKey::new(SECRET).unwrap()).unwrap();
        fs::write(store.file_path("llm"), &quoted).unwrap();
        let key = store.lookup(&stored.key_ref).unwrap();
        assert!(key.expose() == SECRET, "the key file was not cleaned");
    }

    /// Bleibt nach dem Ablegen im Schlüsselbund eine alte Datei des
    /// Rückfalls stehen, sagt der Befund es (HUM-140, Review).
    #[test]
    fn an_old_fallback_file_that_stays_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(dir.path(), Some(fake_tool(dir.path())));
        // Ein Verzeichnis an der Stelle der Datei: `remove_file` scheitert.
        fs::create_dir_all(store.file_path("llm").join("blocker")).unwrap();
        let stored = store.store(&ApiKey::new(SECRET).unwrap()).unwrap();
        assert_eq!(stored.key_ref, KeyRef::secret_service("llm").unwrap());
        let note = stored.note.expect("the leftover is reported");
        assert_eq!(note.code, CONFIG_019);
        assert!(note.why.contains("could not be removed"), "{}", note.why);
        assert!(!note.why.contains(SECRET));
    }

    #[test]
    fn a_missing_key_is_llm_009_with_a_fix() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(dir.path(), Some(fake_tool(dir.path())));
        for key_ref in [
            KeyRef::secret_service("llm").unwrap(),
            KeyRef::file("llm").unwrap(),
        ] {
            let diagnostic = store.lookup(&key_ref).unwrap_err();
            assert_eq!(diagnostic.code, LLM_009, "{key_ref}");
            assert_eq!(
                diagnostic.fix,
                Some(FixAction::CopyCommand("humanitl llm key set".to_owned())),
                "{key_ref}"
            );
        }
        let without_tool = store_in(dir.path(), None);
        let diagnostic = without_tool
            .lookup(&KeyRef::secret_service("llm").unwrap())
            .unwrap_err();
        assert_eq!(diagnostic.code, LLM_009);
        assert!(diagnostic.fix.is_some());
    }

    #[test]
    fn an_open_key_file_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(dir.path(), None);
        store.store(&ApiKey::new(SECRET).unwrap()).unwrap();
        let path = store.file_path("llm");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        let diagnostic = store.lookup(&KeyRef::file("llm").unwrap()).unwrap_err();
        assert_eq!(diagnostic.code, LLM_009);
        assert!(!diagnostic.why.contains(SECRET));
        assert!(
            matches!(diagnostic.fix, Some(FixAction::CopyCommand(ref c)) if c.starts_with("chmod 600"))
        );
    }

    #[test]
    fn clearing_removes_the_entry_and_tolerates_a_missing_one() {
        let dir = tempfile::tempdir().unwrap();
        let store = store_in(dir.path(), Some(fake_tool(dir.path())));
        let stored = store.store(&ApiKey::new(SECRET).unwrap()).unwrap();
        store.clear(&stored.key_ref).unwrap();
        assert_eq!(store.lookup(&stored.key_ref).unwrap_err().code, LLM_009);
        store.clear(&stored.key_ref).unwrap();
        store.clear(&KeyRef::file("llm").unwrap()).unwrap();
    }

    #[test]
    fn a_tool_that_never_answers_runs_into_the_deadline() {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("secret-tool-hangs");
        write_executable(&script, "#!/bin/sh\nexec sleep 30\n");
        let store = store_in(dir.path(), Some(script)).with_timeout(Duration::from_millis(200));
        let started = Instant::now();
        let diagnostic = store
            .lookup(&KeyRef::secret_service("llm").unwrap())
            .unwrap_err();
        assert!(started.elapsed() < Duration::from_secs(10));
        assert_eq!(diagnostic.code, LLM_009);
        assert!(
            diagnostic.why.contains("did not answer"),
            "{}",
            diagnostic.why
        );
    }
}
