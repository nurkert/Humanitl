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
//!   Unter macOS ist das der Anmelde-Schlüsselbund (Keychain), und das Werkzeug
//!   ist das Systemprogramm `/usr/bin/security` (HUM-349). Auch hier keine
//!   Client-Bibliothek: Die App ist nur ad-hoc signiert, ein Eintrag, dessen
//!   Zugriffsliste unserem Programm vertraut, fragte nach jedem Update neu.
//!   Einträge, die `security` anlegt, vertrauen dem von Apple signierten
//!   Werkzeug. Der Schlüssel geht über `stdin` an `security -i`, heraus über
//!   `stdout`, nie über die Kommandozeile.
//! - **In einer Datei mit `0600`** unter `$XDG_DATA_HOME/humanitl/keys`, wenn
//!   der Schlüsselbund auf dieser Maschine nicht antwortet (kein Desktop, kein
//!   Sitzungsbus, kein `secret-tool`; unter macOS etwa ein gesperrter
//!   Schlüsselbund in einer Sitzung ohne Bildschirm). Das ist der Rückfall, und
//!   er wird gemeldet (`CONFIG_019`), nicht verschwiegen.
//!
//! ## Was `security -i` nicht von selbst sagt
//!
//! Gemessen an einem Wegwerf-Schlüsselbund unter macOS 27 (HUM-349):
//!
//! - Eine Zeile über gut 4096 Bytes wird zerschnitten, und der Rest gilt als
//!   neuer Befehl. Dann steht das Ende des Schlüssels in `unknown command`
//!   auf `stderr`, oder ein gekürzter Schlüssel wird gespeichert. Deshalb
//!   erreicht eine zu lange Zeile das Programm gar nicht
//!   ([`MAX_KEYCHAIN_LINE`]), und von `stderr` kommt nur die Meldung des
//!   Schlüsselbunds in den Befund.
//! - Eine Zeile, die sich nicht verarbeiten lässt, endet auch mit `0` und
//!   ohne Ausgabe. Der Ausgang allein beweist also nichts: Nach dem Ablegen
//!   wird der Eintrag zurückgelesen und verglichen.
//! - `find-generic-password -w` gibt einen Wert mit Bytes außerhalb von ASCII
//!   als Hexadezimaltext aus, ununterscheidbar von einem Schlüssel, der
//!   wirklich so aussieht. Solche Schlüssel bleiben deshalb in der Datei.
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

/// Das Schema eines Verweises in den Schlüsselbund von macOS (HUM-349).
///
/// Ein eigenes Schema und nicht `secret-service:`, weil ein Verweis sagt, wo
/// der Schlüssel liegt: Eine `config.toml`, die zwischen Linux und macOS
/// wandert, nennt einen Ort, den die andere Maschine nicht hat, und das
/// meldet [`KeyStore::lookup`] so, statt dort den falschen Speicher zu fragen.
pub const KEYCHAIN_SCHEME: &str = "keychain";

/// Das Schema eines Verweises auf die Datei des Rückfalls.
pub const FILE_SCHEME: &str = "file";

/// Der Name, unter dem Humanitl seine Einträge im Schlüsselbund ablegt
/// (Attribut `service`).
pub const SECRET_SERVICE_APP: &str = "humanitl";

/// Das Werkzeug für den Schlüsselbund von macOS. Ein fester Pfad und keine
/// Suche: Dieses Programm ist es, dem der Eintrag vertraut.
pub const KEYCHAIN_TOOL: &str = "/usr/bin/security";

/// Der Fehlercode von `security`, wenn es keinen passenden Eintrag gibt
/// (`errSecItemNotFound`, abgeschnitten auf ein Byte).
const KEYCHAIN_NOT_FOUND: i32 = 44;

/// Die längste Zeile in Bytes, die an `security -i` geht.
///
/// Gemessen schneidet das Programm kurz über 4096 Bytes; die Grenze hält
/// Abstand dazu. Sie liegt auf der Zeile und nicht auf dem Schlüssel, weil
/// Anführungszeichen und Backslashes beim Maskieren doppelt zählen.
pub const MAX_KEYCHAIN_LINE: usize = 4000;

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

/// Die längste Zeit, die das Werkzeug des Schlüsselbunds für eine Antwort
/// bekommt.
///
/// Ein gesperrter Schlüsselbund fragt den Menschen nach seinem Passwort, und
/// das darf dauern; ein Sitzungsbus, der nie antwortet, darf den Start einer
/// Sitzung aber nicht für immer anhalten.
pub const TOOL_TIMEOUT: Duration = Duration::from_secs(20);

/// So viele Zeichen der Fehlerausgabe des Werkzeugs stehen im Befund.
const SHOWN_STDERR_CHARS: usize = 200;

/// Wie oft [`run_tool`] nachsieht, ob das Werkzeug fertig ist.
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
    /// Im Schlüsselbund des Systems über den Secret Service.
    SecretService,
    /// Im Schlüsselbund von macOS über `/usr/bin/security`.
    Keychain,
    /// In der Datei des Rückfalls.
    File,
}

impl StoreKind {
    /// Der Name, wie er in Verweisen und Antworten steht.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SecretService => SECRET_SERVICE_SCHEME,
            Self::Keychain => KEYCHAIN_SCHEME,
            Self::File => FILE_SCHEME,
        }
    }

    /// Wie Befunde diesen Schlüsselbund nennen.
    const fn keyring_label(self) -> &'static str {
        match self {
            Self::Keychain => "the macOS Keychain",
            Self::SecretService | Self::File => "the system keyring",
        }
    }
}

/// Ein Verweis auf einen Zugangsschlüssel, wie er in `llm.api_key_ref` steht.
///
/// Drei Formen: `secret-service:humanitl/<name>`,
/// `keychain:humanitl/<name>` und `file:<name>`. Der Name
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

    /// Der Verweis auf den Eintrag `name` im Schlüsselbund von macOS.
    ///
    /// `None`, wenn `name` kein gültiger Name ist.
    #[must_use]
    pub fn keychain(name: &str) -> Option<Self> {
        is_key_name(name).then(|| Self {
            store: StoreKind::Keychain,
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
        if let Some(rest) = text.strip_prefix("keychain:") {
            let name = rest.strip_prefix("humanitl/")?;
            return Self::keychain(name);
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
            StoreKind::Keychain => {
                write!(f, "{KEYCHAIN_SCHEME}:{SECRET_SERVICE_APP}/{}", self.name)
            }
            StoreKind::File => write!(f, "{FILE_SCHEME}:{}", self.name),
        }
    }
}

/// Ob `name` ein Name ist, den ein Verweis tragen darf.
pub(crate) fn is_key_name(name: &str) -> bool {
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
/// Billig zu klonen. `system` ist der Schlüsselbund, in dem diese Maschine
/// ablegt; die Entscheidung fällt zur Laufzeit, damit die Tests beider
/// Systeme auf beiden Systemen laufen. `secret_tool` und `keychain_tool` sind
/// die Programme, mit denen gesprochen wird; Tests setzen dort Attrappen ein
/// ([`KeyStore::with_secret_tool`], [`KeyStore::with_keychain_tool`]) und
/// fassen den Schlüsselbund des Menschen nie an.
#[derive(Debug, Clone)]
pub struct KeyStore {
    key_dir: PathBuf,
    system: StoreKind,
    secret_tool: Option<PathBuf>,
    keychain_tool: Option<PathBuf>,
    timeout: Duration,
    uid: u32,
}

impl KeyStore {
    /// Der Speicher für diese Pfade: Dateien unter [`Paths::key_dir`], und
    /// der Schlüsselbund dieses Systems.
    #[must_use]
    pub fn for_paths(paths: &Paths) -> Self {
        let system = if cfg!(target_os = "macos") {
            StoreKind::Keychain
        } else {
            StoreKind::SecretService
        };
        Self::for_system(paths, system)
    }

    /// Der Speicher für `system`: unter Linux `secret-tool`, wie es der
    /// Suchpfad aus [`Paths::env`] findet, unter macOS allein
    /// [`KEYCHAIN_TOOL`]. Ein `secret-tool`, das dort zufällig im Suchpfad
    /// liegt, wird nie gefragt: Ein Verweis von Linux meldet den fremden Ort.
    ///
    /// Auch der feste Pfad gilt nur, wenn der Suchpfad `/usr/bin` nennt. So
    /// bleibt die Regel beider Systeme gleich, dass Pfade ohne Suchpfad keinen
    /// Schlüsselbund haben, und ein Test mit Wegwerf-Pfaden erreicht den
    /// Schlüsselbund des Menschen nicht.
    fn for_system(paths: &Paths, system: StoreKind) -> Self {
        let search = paths.env().non_empty("PATH");
        let secret_tool = search
            .filter(|_search| system == StoreKind::SecretService)
            .and_then(|search| find_program(search, "secret-tool"));
        let keychain_tool = search
            .filter(|search| {
                system == StoreKind::Keychain
                    && search
                        .split(':')
                        .any(|dir| matches!(dir, "/usr/bin" | "/usr/bin/"))
            })
            .map(|_search| PathBuf::from(KEYCHAIN_TOOL));
        Self {
            key_dir: paths.key_dir(),
            system,
            secret_tool,
            keychain_tool,
            timeout: TOOL_TIMEOUT,
            uid: process_uid(),
        }
    }

    /// Derselbe Speicher mit diesem Programm für den Secret Service, oder
    /// ohne Schlüsselbund (`None`), auf jedem System.
    #[must_use]
    pub fn with_secret_tool(mut self, program: Option<PathBuf>) -> Self {
        self.system = StoreKind::SecretService;
        self.secret_tool = program;
        self.keychain_tool = None;
        self
    }

    /// Derselbe Speicher mit diesem Programm an der Stelle von
    /// [`KEYCHAIN_TOOL`], auf jedem System.
    #[must_use]
    pub fn with_keychain_tool(mut self, program: Option<PathBuf>) -> Self {
        self.system = StoreKind::Keychain;
        self.secret_tool = None;
        self.keychain_tool = program;
        self
    }

    /// Derselbe Speicher mit einer anderen Frist für das Werkzeug.
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
            StoreKind::Keychain => self.lookup_keychain(key_ref),
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
        self.store_named(DEFAULT_KEY_NAME, key)
    }

    /// Wie [`KeyStore::store`], unter dem Namen `name`: der Eintrag
    /// `humanitl/<name>` im Schlüsselbund oder die Datei `<name>.key`. Jeder
    /// gespeicherte Modellserver hat so seinen eigenen Eintrag (HUM-289).
    ///
    /// # Errors
    ///
    /// `CONFIG_020`, wenn `name` kein gültiger Name ist oder weder der
    /// Schlüsselbund noch die Datei den Schlüssel nehmen.
    pub fn store_named(&self, name: &str, key: &ApiKey) -> Result<Stored, Diagnostic> {
        if !is_key_name(name) {
            return Err(Diagnostic::builder(CONFIG_020, Severity::Error)
                .why(format!(
                    "the access key was not stored: {name:?} is not a key name (1 to 32 \
                     characters a-z, 0-9, - and _)"
                ))
                .build());
        }
        let keyring = self.system.keyring_label();
        let stored = match self.system {
            StoreKind::Keychain => self.store_keychain(name, key),
            StoreKind::SecretService | StoreKind::File => self.store_secret_service(name, key),
        };
        let reason = match stored {
            Ok(()) => {
                // Ein Rest aus einer Zeit ohne Schlüsselbund liegt sonst als
                // Klartext neben dem Eintrag, der jetzt gilt; bleibt er
                // stehen, sagt der Befund es, statt es zu verschweigen.
                let note = self.remove_file(name).err().map(|why| {
                    Diagnostic::builder(CONFIG_019, Severity::Warning)
                        .why(format!(
                            "the access key is in {keyring} now, and the old fallback file could \
                             not be removed ({why}); it still holds an earlier key in plain text"
                        ))
                        .fix(FixAction::CopyCommand(format!(
                            "rm {}",
                            shell_path(&self.file_path(name))
                        )))
                        .build()
                });
                return Ok(Stored {
                    key_ref: named_ref(self.system, name),
                    note,
                });
            }
            Err(reason) => reason,
        };
        self.write_file(name, key).map_err(|why| {
            Diagnostic::builder(CONFIG_020, Severity::Error)
                .why(format!(
                    "the access key was not stored: {keyring} did not take it ({reason}), and \
                     the fallback file could not be written ({why})"
                ))
                .build()
        })?;
        let builder = Diagnostic::builder(CONFIG_019, Severity::Warning).why(format!(
            "{keyring} did not take the access key ({reason}); it is stored in {} with mode 0600 \
             instead, readable by this account only",
            self.file_path(name).display()
        ));
        let note = match self.fallback_fix(name, key) {
            Some(fix) => builder.fix(fix).build(),
            None => builder.build(),
        };
        Ok(Stored {
            key_ref: named_ref(StoreKind::File, name),
            note: Some(note),
        })
    }

    /// Entfernt den Schlüssel, auf den `key_ref` zeigt. Fehlt er, ist das
    /// kein Fehler.
    ///
    /// Ein Verweis auf den Schlüsselbund des anderen Systems zählt als schon
    /// weg: Hier gibt es den Eintrag nicht, und nur so lässt sich der Verweis
    /// mit `humanitl llm key remove` aus der Konfiguration nehmen.
    ///
    /// # Errors
    ///
    /// `CONFIG_020`, wenn der Eintrag noch da ist und sich nicht löschen ließ.
    pub fn clear(&self, key_ref: &KeyRef) -> Result<(), Diagnostic> {
        if self.is_foreign(key_ref) {
            return Ok(());
        }
        let result = match key_ref.store() {
            StoreKind::SecretService => self.clear_secret_service(key_ref.name()),
            StoreKind::Keychain => self.clear_keychain(key_ref.name()),
            StoreKind::File => self.remove_file(key_ref.name()),
        };
        result.map_err(|why| {
            Diagnostic::builder(CONFIG_020, Severity::Error)
                .why(format!("the access key {key_ref} was not removed: {why}"))
                .build()
        })
    }

    /// Der Befehl, mit dem ein Mensch den Schlüssel selbst entfernt, wenn
    /// [`KeyStore::clear`] ihn nicht wegbekam. Für einen Verweis auf das andere
    /// System gibt es keinen: Dort scheitert `clear` nie.
    #[must_use]
    pub fn manual_clear(&self, key_ref: &KeyRef) -> Option<FixAction> {
        if self.is_foreign(key_ref) {
            return None;
        }
        let name = key_ref.name();
        Some(FixAction::CopyCommand(match key_ref.store() {
            StoreKind::SecretService => {
                format!("secret-tool clear service {SECRET_SERVICE_APP} account {name}")
            }
            StoreKind::Keychain => {
                format!("security delete-generic-password -s {SECRET_SERVICE_APP} -a {name}")
            }
            StoreKind::File => format!("rm {}", shell_path(&self.file_path(name))),
        }))
    }

    /// Ob `key_ref` in den Schlüsselbund des anderen Systems zeigt.
    fn is_foreign(&self, key_ref: &KeyRef) -> bool {
        key_ref.store() != StoreKind::File && key_ref.store() != self.system
    }

    /// Was gegen den Rückfall hilft. Unter macOS gibt es nichts zu
    /// installieren; hilft das Entsperren nicht, weil `security` einen solchen
    /// Schlüssel gar nicht tragen kann, schlägt der Befund nichts vor.
    fn fallback_fix(&self, name: &str, key: &ApiKey) -> Option<FixAction> {
        match self.system {
            StoreKind::Keychain => (self.keychain_tool.is_some()
                && keychain_add_line(name, key).is_ok())
            .then(|| FixAction::CopyCommand("security unlock-keychain".to_owned())),
            StoreKind::SecretService | StoreKind::File => Some(FixAction::CopyCommand(
                "sudo apt-get install libsecret-tools".to_owned(),
            )),
        }
    }

    fn lookup_secret_service(&self, key_ref: &KeyRef) -> Result<ApiKey, Diagnostic> {
        let Some(tool) = &self.secret_tool else {
            // Unter macOS gibt es nichts zu installieren: Der Verweis stammt
            // von einer Linux-Maschine, und das Setzen legt ihn neu an.
            if self.system == StoreKind::Keychain {
                return Err(unavailable(
                    key_ref,
                    "this reference points to the Secret Service of a Linux desktop, which \
                     this system does not have",
                    Some(set_again()),
                ));
            }
            return Err(unavailable(
                key_ref,
                "secret-tool is not installed, so the system keyring cannot be asked",
                Some(FixAction::CopyCommand(
                    "sudo apt-get install libsecret-tools".to_owned(),
                )),
            ));
        };
        let args = attributes("lookup", key_ref.name());
        let output = run_tool(tool, &args, None, self.timeout, &SECRET_TOOL)
            .map_err(|why| unavailable(key_ref, &why, Some(set_again())))?;
        if !output.status.success() {
            return Err(unavailable(
                key_ref,
                &format!(
                    "the system keyring holds no such entry ({})",
                    describe(&output, &SECRET_TOOL)
                ),
                Some(set_again()),
            ));
        }
        key_from_output(key_ref, &output)
    }

    fn lookup_keychain(&self, key_ref: &KeyRef) -> Result<ApiKey, Diagnostic> {
        if self.keychain_tool.is_none() {
            let why = if self.system == StoreKind::Keychain {
                "/usr/bin/security cannot be used here, so the macOS Keychain cannot be asked"
            } else {
                "this reference points to the macOS Keychain, which this system does not have"
            };
            return Err(unavailable(key_ref, why, Some(set_again())));
        }
        let output = self
            .find_in_keychain(key_ref.name())
            .map_err(|why| unavailable(key_ref, &why, Some(set_again())))?;
        if output.status.code() == Some(KEYCHAIN_NOT_FOUND) {
            return Err(unavailable(
                key_ref,
                "the macOS Keychain holds no such entry",
                Some(set_again()),
            ));
        }
        if !output.status.success() {
            return Err(unavailable(
                key_ref,
                &format!(
                    "the macOS Keychain did not hand out the entry ({})",
                    describe(&output, &SECURITY_TOOL)
                ),
                Some(set_again()),
            ));
        }
        key_from_output(key_ref, &output)
    }

    /// `security find-generic-password` für den Eintrag `name`. Die Antwort
    /// kann den Schlüssel tragen.
    fn find_in_keychain(&self, name: &str) -> Result<ToolOutput, String> {
        let tool = self.keychain_program()?;
        let mut args = keychain_item("find-generic-password", name);
        args.push(OsString::from("-w"));
        run_tool(tool, &args, None, self.timeout, &SECURITY_TOOL)
    }

    fn keychain_program(&self) -> Result<&PathBuf, String> {
        self.keychain_tool
            .as_ref()
            .ok_or_else(|| format!("{KEYCHAIN_TOOL} cannot be used here"))
    }

    /// Legt den Schlüssel über `security -i` ab: Der Befehl samt Schlüssel
    /// steht auf `stdin`, `argv` trägt nur `-i`. Das `add-generic-password`
    /// nennt weder `-A` noch `-T`, also vertraut der Eintrag allein dem
    /// Werkzeug selbst.
    fn store_keychain(&self, name: &str, key: &ApiKey) -> Result<(), String> {
        let tool = self.keychain_program()?;
        let line = keychain_add_line(name, key)?;
        let args = [OsString::from("-i")];
        let output = run_tool(
            tool,
            &args,
            Some(line.as_bytes()),
            self.timeout,
            &SECURITY_TOOL,
        )?;
        if !output.status.success() {
            return Err(describe_interactive(&output));
        }
        // Der Ausgang von `security -i` beweist nichts (Kopf dieser Datei):
        // Abgelegt ist der Schlüssel erst, wenn genau er zurückkommt.
        let back = self.find_in_keychain(name)?;
        let text = back.stdout.strip_suffix(b"\n").unwrap_or(&back.stdout);
        if back.status.success() && text == key.expose().as_bytes() {
            Ok(())
        } else {
            Err(format!(
                "{KEYCHAIN_TOOL} reported success, and the entry does not read back as the key"
            ))
        }
    }

    fn clear_keychain(&self, name: &str) -> Result<(), String> {
        let tool = self.keychain_program()?;
        let args = keychain_item("delete-generic-password", name);
        let output = run_tool(tool, &args, None, self.timeout, &SECURITY_TOOL)?;
        // Anders als `secret-tool clear` meldet `security` einen fehlenden
        // Eintrag als Fehler; hier heißt das nur: schon weg.
        if output.status.success() || output.status.code() == Some(KEYCHAIN_NOT_FOUND) {
            Ok(())
        } else {
            Err(describe(&output, &SECURITY_TOOL))
        }
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
        let output = run_tool(
            tool,
            &args,
            Some(key.expose().as_bytes()),
            self.timeout,
            &SECRET_TOOL,
        )?;
        if output.status.success() {
            Ok(())
        } else {
            Err(describe(&output, &SECRET_TOOL))
        }
    }

    fn clear_secret_service(&self, name: &str) -> Result<(), String> {
        let Some(tool) = &self.secret_tool else {
            return Err("secret-tool is not installed".to_owned());
        };
        let output = run_tool(
            tool,
            &attributes("clear", name),
            None,
            self.timeout,
            &SECRET_TOOL,
        )?;
        // `secret-tool clear` endet auch ohne passenden Eintrag mit 0; ein
        // anderer Code ist ein Fehler des Schlüsselbunds.
        if output.status.success() {
            Ok(())
        } else {
            Err(describe(&output, &SECRET_TOOL))
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

/// Der Verweis auf den Schlüssel `name` an diesem Ort.
fn named_ref(store: StoreKind, name: &str) -> KeyRef {
    KeyRef {
        store,
        name: name.to_owned(),
    }
}

/// Die Argumente für `secret-tool <command>` mit den Attributen des Eintrags.
fn attributes(command: &str, name: &str) -> Vec<OsString> {
    [command, "service", SECRET_SERVICE_APP, "account", name]
        .into_iter()
        .map(OsString::from)
        .collect()
}

/// Die Argumente von `security <command>` für den Eintrag `name`.
fn keychain_item(command: &str, name: &str) -> Vec<OsString> {
    [command, "-s", SECRET_SERVICE_APP, "-a", name]
        .into_iter()
        .map(OsString::from)
        .collect()
}

/// Die eine Zeile für `security -i`, die `key` unter `name` ablegt oder den
/// Eintrag ersetzt (`-U`).
///
/// `security -i` kennt doppelte Anführungszeichen, in denen `\` das nächste
/// Zeichen schützt; gemessen kommt damit jedes druckbare ASCII-Zeichen exakt
/// zurück. `-X` mit Hexadezimaltext bräuchte kein Maskieren, verdoppelt aber
/// jede Länge und stößt schon bei 2000 Bytes an die Zeilengrenze.
///
/// # Errors
///
/// Ein Grund ohne den Schlüssel, wenn `security` diesen Schlüssel nicht
/// unverändert trüge: Bytes außerhalb von druckbarem ASCII kämen als
/// Hexadezimaltext zurück, und eine Zeile über [`MAX_KEYCHAIN_LINE`] würde
/// zerschnitten.
fn keychain_add_line(name: &str, key: &ApiKey) -> Result<Zeroizing<String>, String> {
    let text = key.expose();
    if !text.bytes().all(|byte| (0x21..=0x7e).contains(&byte)) {
        return Err(
            "the key has characters outside printable ASCII, and /usr/bin/security would hand \
             them back in another form"
                .to_owned(),
        );
    }
    let head = format!(
        "add-generic-password -U -s {SECRET_SERVICE_APP} -a {name} -l \"Humanitl access key \
         ({name})\" -w \""
    );
    let escapes = text.bytes().filter(|byte| matches!(byte, b'"' | b'\\'));
    let length = head.len() + text.len() + escapes.count() + "\"\n".len();
    if length > MAX_KEYCHAIN_LINE {
        return Err(format!(
            "the key makes a command of {length} bytes, and /usr/bin/security takes \
             {MAX_KEYCHAIN_LINE}"
        ));
    }
    // Die volle Länge von Anfang an: Wüchse der Text, bliebe eine Kopie des
    // Schlüssels im alten Puffer stehen, die niemand überschreibt.
    let mut line = Zeroizing::new(String::with_capacity(length));
    line.push_str(&head);
    for character in text.chars() {
        if matches!(character, '"' | '\\') {
            line.push('\\');
        }
        line.push(character);
    }
    line.push_str("\"\n");
    Ok(line)
}

/// Der Schlüssel aus der Antwort eines Werkzeugs.
fn key_from_output(key_ref: &KeyRef, output: &ToolOutput) -> Result<ApiKey, Diagnostic> {
    // Gelesen wird an Ort und Stelle: `stdout` wird beim Fallenlassen
    // überschrieben, eine Kopie als `String` würde es nicht.
    let text = std::str::from_utf8(&output.stdout).map_err(|_not_text| {
        unavailable(key_ref, "the keyring entry is not text", Some(set_again()))
    })?;
    // `secret-tool` hängt einen Zeilenumbruch an, wenn es an ein Terminal
    // schreibt, `security -w` immer; ein Schlüssel trägt keinen Leerraum
    // (`ApiKey::new`). Ein Eintrag, den eine ältere Fassung samt
    // Anführungszeichen abgelegt hat, wirkt trotzdem: dieselbe Bereinigung
    // wie beim Setzen (HUM-283).
    ApiKey::from_pasted(text).map_err(|_refused| {
        unavailable(
            key_ref,
            "the keyring entry is not a usable key",
            Some(set_again()),
        )
    })
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

/// Wie Befunde ein Werkzeug nennen.
struct ToolName {
    /// Der Name des Programms.
    name: &'static str,
    /// Was es heißt, wenn es nicht antwortet.
    stalled: &'static str,
}

const SECRET_TOOL: ToolName = ToolName {
    name: "secret-tool",
    stalled: "the session bus or the keyring does not respond",
};

const SECURITY_TOOL: ToolName = ToolName {
    name: "security",
    stalled: "the macOS Keychain does not respond, perhaps it waits for its password",
};

/// Was ein Werkzeug geantwortet hat.
struct ToolOutput {
    status: ExitStatus,
    /// Kann den Schlüssel tragen und wird deshalb überschrieben.
    stdout: Zeroizing<Vec<u8>>,
    stderr: Vec<u8>,
}

/// Der Ausgang eines Aufrufs als kurzer Satz, ohne `stdout`.
fn describe(output: &ToolOutput, tool: &ToolName) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    sentence(output, tool, &stderr)
}

/// [`describe`] für `security -i`, dessen `stderr` nicht als Ganzes taugt:
/// Eine Eingabezeile, die es nicht versteht, wiederholt es dort wörtlich, und
/// die Zeile trägt den Schlüssel. In den Befund kommen deshalb nur die
/// Meldungen des Schlüsselbunds selbst.
fn describe_interactive(output: &ToolOutput) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let keychain = stderr
        .lines()
        .find(|line| line.starts_with("security: SecKeychain"))
        .unwrap_or_default();
    sentence(output, &SECURITY_TOOL, keychain)
}

/// Der Satz zu einem Ausgang und einem Ausschnitt von `stderr`.
fn sentence(output: &ToolOutput, tool: &ToolName, stderr: &str) -> String {
    let name = tool.name;
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
        format!("{name} ended with {code}")
    } else {
        format!("{name} ended with {code}: {}", stderr.trim())
    }
}

/// Startet `program` mit `args`, schreibt `input` auf `stdin` und wartet
/// höchstens `timeout` auf das Ende.
///
/// Blockierend; der Daemon ruft es auf einem eigenen Faden. Läuft die Frist ab,
/// wird das Programm beendet.
fn run_tool(
    program: &Path,
    args: &[OsString],
    input: Option<&[u8]>,
    timeout: Duration,
    tool: &ToolName,
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
    let status = wait_with_timeout(&mut child, timeout, tool)?;
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
fn wait_with_timeout(
    child: &mut Child,
    timeout: Duration,
    tool: &ToolName,
) -> Result<ExitStatus, String> {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) if Instant::now() >= deadline => {
                let _ignored = child.kill();
                let _ignored = child.wait();
                return Err(format!(
                    "{} did not answer within {} s; {}",
                    tool.name,
                    timeout.as_secs(),
                    tool.stalled
                ));
            }
            Ok(None) => std::thread::sleep(POLL),
            Err(error) => return Err(format!("{} could not be awaited ({error})", tool.name)),
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
            system: StoreKind::SecretService,
            secret_tool: tool,
            keychain_tool: None,
            timeout: Duration::from_secs(5),
            uid: process_uid(),
        }
    }

    const SECRET: &str = "sk-test-0123456789abcdef";

    #[test]
    fn a_reference_has_three_forms_and_nothing_else() {
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
            "keychain:llm",
            "keychain:other/llm",
            "keychain:humanitl/LLM",
        ] {
            assert!(KeyRef::parse(refused).is_none(), "{refused:?}");
        }
        assert_eq!(
            KeyRef::secret_service("llm").unwrap().to_string(),
            "secret-service:humanitl/llm"
        );
        assert_eq!(KeyRef::file("llm").unwrap().to_string(), "file:llm");
        let keychain = KeyRef::parse("keychain:humanitl/llm").unwrap();
        assert_eq!(keychain, KeyRef::keychain("llm").unwrap());
        assert_eq!(keychain.store().as_str(), "keychain");
        assert_eq!(keychain.to_string(), "keychain:humanitl/llm");
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

    /// Eine Attrappe von `/usr/bin/security`, die sich verhält wie gemessen:
    /// `-i` liest den Befehl von `stdin`, `find -w` hängt einen Zeilenumbruch
    /// an, und ein fehlender Eintrag endet mit 44. Sie legt ihre Einträge als
    /// Dateien in `dir/keychain` ab; die Argumente landen in `argv.log`.
    fn fake_security(dir: &Path) -> PathBuf {
        let store = dir.join("keychain");
        fs::create_dir_all(&store).unwrap();
        let script = dir.join("security");
        write_executable(
            &script,
            &format!(
                r#"#!/bin/sh
store='{store}'
printf '%s\n' "$*" >> '{log}'
account=
if [ "$1" = -i ]; then
  line=$(cat)
  case "$line" in
    "add-generic-password -U -s humanitl -a "*) ;;
    *) echo "security: unknown command" >&2; exit 1 ;;
  esac
  account=$(printf '%s\n' "$line" | sed -e 's/.* -a \([a-z0-9_-]*\) .*/\1/')
  printf '%s' "$line" | sed -e 's/.* -w "//' -e 's/"$//' -e 's/\\\(.\)/\1/g' \
    > "$store/$account"
  exit 0
fi
command=$1
shift
while [ $# -gt 0 ]; do
  [ "$1" = -a ] && account=$2
  shift
done
[ -f "$store/$account" ] || {{
  echo 'security: SecKeychainSearchCopyNext: The specified item could not be found.' >&2
  exit 44
}}
case "$command" in
  find-generic-password) cat "$store/$account"; echo ;;
  delete-generic-password) rm -f "$store/$account" ;;
esac
"#,
                store = store.display(),
                log = dir.join("argv.log").display()
            ),
        );
        script
    }

    /// Eine Attrappe, die mit `body` antwortet, ohne etwas abzulegen.
    fn scripted_security(dir: &Path, body: &str) -> PathBuf {
        let script = dir.join("security-scripted");
        write_executable(&script, &format!("#!/bin/sh\ncat > /dev/null\n{body}\n"));
        script
    }

    fn keychain_in(dir: &Path, tool: Option<PathBuf>) -> KeyStore {
        store_in(dir, None).with_keychain_tool(tool)
    }

    fn paths_with(search: Option<&str>, dir: &Path) -> Paths {
        let mut pairs = vec![("HOME", dir.display().to_string())];
        if let Some(search) = search {
            pairs.push(("PATH", search.to_owned()));
        }
        Paths::new(crate::Env::from_pairs(pairs))
    }

    #[test]
    fn the_keychain_takes_the_key_over_stdin_and_never_over_argv() {
        let dir = tempfile::tempdir().unwrap();
        let store = keychain_in(dir.path(), Some(fake_security(dir.path())));
        let stored = store.store(&ApiKey::new(SECRET).unwrap()).unwrap();
        assert_eq!(stored.key_ref, KeyRef::keychain("llm").unwrap());
        assert!(stored.note.is_none());

        let argv = fs::read_to_string(dir.path().join("argv.log")).unwrap();
        assert!(argv.starts_with("-i\n"), "{argv}");
        assert!(!argv.contains(SECRET), "the key reached argv: {argv}");
        assert_eq!(
            store.lookup(&stored.key_ref).unwrap().expose(),
            SECRET,
            "the key comes back over stdout, without the line break"
        );
        assert!(
            !store.file_path("llm").exists(),
            "no plain file next to the keychain"
        );
    }

    /// Der Befehl vertraut nur dem Werkzeug selbst: `-A` öffnete den Eintrag
    /// für jedes Programm, `-T` nähme ein weiteres auf.
    #[test]
    fn the_add_command_replaces_the_item_and_opens_it_to_nobody() {
        let key = ApiKey::new(SECRET).unwrap();
        let line = keychain_add_line("cloud", &key).unwrap();
        assert_eq!(
            line.as_str(),
            "add-generic-password -U -s humanitl -a cloud -l \
             \"Humanitl access key (cloud)\" -w \"sk-test-0123456789abcdef\"\n"
        );
        assert!(!line.contains(" -A"));
        assert!(!line.contains(" -T"));
    }

    /// Anführungszeichen und Backslash sind maskiert, und über die Attrappe,
    /// die so liest wie `security -i`, kommt der Schlüssel unverändert zurück.
    #[test]
    fn a_key_with_quotes_and_backslashes_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let store = keychain_in(dir.path(), Some(fake_security(dir.path())));
        for text in [
            r#"sk-"quoted"-0123456789"#,
            r"sk-back\slash-0123456789",
            r"sk-trailing-0123456789\",
            r#"sk-both-\"-0123456789"#,
            "sk-$HOME-`id`-'x'-0123456789",
        ] {
            let key = ApiKey::new(text).unwrap();
            let stored = store.store(&key).unwrap();
            assert_eq!(stored.key_ref, KeyRef::keychain("llm").unwrap(), "{text}");
            assert!(stored.note.is_none(), "{text}");
            let raw = fs::read(dir.path().join("keychain/llm")).unwrap();
            assert_eq!(raw, text.as_bytes(), "the item is not the key");
        }
        let line = keychain_add_line("llm", &ApiKey::new(r#"sk-a"b\c-0123456789"#).unwrap());
        assert!(
            line.unwrap()
                .ends_with("-w \"sk-a\\\"b\\\\c-0123456789\"\n"),
            "the line is not quoted"
        );
    }

    /// Ein scheiternder Befehl: `security -i` endet ungleich 0, der Schlüssel
    /// liegt in der Datei, und der Befund nennt das richtige Werkzeug und den
    /// richtigen Ausweg.
    #[test]
    fn a_failed_security_i_falls_back_to_the_file_and_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let tool = scripted_security(
            dir.path(),
            "echo 'security: SecKeychainItemCreateFromContent (<default>): User interaction is \
             not allowed.' >&2\necho 'add-generic-password: returned -25308' >&2\nexit 36",
        );
        let store = keychain_in(dir.path(), Some(tool));
        let stored = store.store(&ApiKey::new(SECRET).unwrap()).unwrap();
        assert_eq!(stored.key_ref, KeyRef::file("llm").unwrap());
        let note = stored.note.expect("the fallback is reported");
        assert_eq!(note.code, CONFIG_019);
        assert_eq!(note.severity, Severity::Warning);
        assert!(
            note.why.starts_with("the macOS Keychain did not take"),
            "{}",
            note.why
        );
        assert!(
            note.why.contains("User interaction is not allowed"),
            "{}",
            note.why
        );
        assert!(note.why.contains("mode 0600"), "{}", note.why);
        assert!(!note.why.contains("secret-tool"), "{}", note.why);
        assert_eq!(
            note.fix,
            Some(FixAction::CopyCommand(
                "security unlock-keychain".to_owned()
            ))
        );
        assert_eq!(store.lookup(&stored.key_ref).unwrap().expose(), SECRET);
    }

    /// `security -i` meldet manche Fehler mit 0 und ohne Ausgabe. Ohne das
    /// Zurücklesen stünde dann ein Verweis auf einen Eintrag in der
    /// Konfiguration, den es nicht gibt, und der Schlüssel wäre verloren.
    #[test]
    fn a_silent_success_is_caught_by_reading_back() {
        let dir = tempfile::tempdir().unwrap();
        let tool = scripted_security(
            dir.path(),
            "case \"$1\" in -i) exit 0 ;; *) echo 'not-the-key'; exit 0 ;; esac",
        );
        let store = keychain_in(dir.path(), Some(tool));
        let stored = store.store(&ApiKey::new(SECRET).unwrap()).unwrap();
        assert_eq!(stored.key_ref, KeyRef::file("llm").unwrap());
        let note = stored.note.expect("the fallback is reported");
        assert!(note.why.contains("does not read back"), "{}", note.why);
        assert!(!note.why.contains("not-the-key"), "{}", note.why);
        assert_eq!(store.lookup(&stored.key_ref).unwrap().expose(), SECRET);
    }

    /// `security -i` wiederholt eine Zeile, die es nicht versteht, wörtlich
    /// auf `stderr`. Davon erreicht nichts den Befund.
    #[test]
    fn the_stderr_of_security_i_never_carries_the_key_into_a_finding() {
        let dir = tempfile::tempdir().unwrap();
        let tool = scripted_security(
            dir.path(),
            &format!("echo 'security: unknown command \"{SECRET}\"' >&2\nexit 1"),
        );
        let store = keychain_in(dir.path(), Some(tool));
        let stored = store.store(&ApiKey::new(SECRET).unwrap()).unwrap();
        let note = stored.note.expect("the fallback is reported");
        assert!(!note.why.contains(SECRET), "the key reached the finding");
        assert!(note.why.contains("security ended with 1"), "{}", note.why);

        // Auch wenn die Datei scheitert und der Fehler `CONFIG_020` wird.
        let blocked = keychain_in(dir.path(), store.keychain_tool.clone());
        fs::remove_dir_all(blocked.key_dir()).unwrap();
        fs::write(blocked.key_dir(), "a file where the directory belongs").unwrap();
        let refused = blocked.store(&ApiKey::new(SECRET).unwrap()).unwrap_err();
        assert_eq!(refused.code, CONFIG_020);
        assert!(
            refused.why.contains("the macOS Keychain did not take it"),
            "{}",
            refused.why
        );
        assert!(!refused.why.contains(SECRET), "the key reached the finding");
    }

    /// Was `security` nicht unverändert trüge, erreicht es gar nicht: Eine
    /// zu lange Zeile würde zerschnitten, Bytes außerhalb von ASCII kämen als
    /// Hexadezimaltext zurück. Beides landet in der Datei, ohne Vorschlag,
    /// der nicht hülfe.
    #[test]
    fn a_key_security_cannot_carry_stays_in_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let store = keychain_in(dir.path(), Some(fake_security(dir.path())));
        for text in ["k".repeat(MAX_KEY_BYTES), format!("sk-schlüssel-{SECRET}")] {
            let key = ApiKey::new(&text).unwrap();
            let stored = store.store(&key).unwrap();
            assert_eq!(stored.key_ref, KeyRef::file("llm").unwrap());
            let note = stored.note.expect("the fallback is reported");
            assert_eq!(note.code, CONFIG_019);
            assert!(note.fix.is_none(), "unlocking does not help here");
            assert!(!note.why.contains(&text), "the key reached the finding");
            assert_eq!(store.lookup(&stored.key_ref).unwrap(), key);
        }
        assert!(
            !dir.path().join("argv.log").exists(),
            "the tool was started"
        );
    }

    /// Die Grenze zählt die Maskierung mit, und bis zu ihr geht es durch.
    #[test]
    fn the_line_limit_counts_the_escapes() {
        let head = keychain_add_line("llm", &ApiKey::new(SECRET).unwrap())
            .unwrap()
            .len()
            - SECRET.len();
        let fits = "k".repeat(MAX_KEYCHAIN_LINE - head);
        let line = keychain_add_line("llm", &ApiKey::new(&fits).unwrap()).unwrap();
        assert_eq!(line.len(), MAX_KEYCHAIN_LINE);
        let doubled = "\"".repeat(MAX_KEYCHAIN_LINE - head);
        assert!(keychain_add_line("llm", &ApiKey::new(&doubled).unwrap()).is_err());
    }

    /// Ein Schlüssel, den der Rückfall als Datei abgelegt hat, wirkt weiter
    /// über seinen Verweis, und das nächste Setzen zieht ihn um.
    #[test]
    fn a_file_key_keeps_working_and_setting_moves_it_to_the_keychain() {
        let dir = tempfile::tempdir().unwrap();
        let old = keychain_in(dir.path(), None)
            .store(&ApiKey::new(SECRET).unwrap())
            .unwrap();
        assert_eq!(old.key_ref, KeyRef::file("llm").unwrap());
        let note = old.note.expect("the fallback is reported");
        assert!(note.why.starts_with("the macOS Keychain"), "{}", note.why);

        let store = keychain_in(dir.path(), Some(fake_security(dir.path())));
        assert_eq!(store.lookup(&old.key_ref).unwrap().expose(), SECRET);

        let moved = store.store(&ApiKey::new(SECRET).unwrap()).unwrap();
        assert_eq!(moved.key_ref, KeyRef::keychain("llm").unwrap());
        assert!(moved.note.is_none());
        assert!(!store.file_path("llm").exists(), "the file stayed");
        assert_eq!(store.lookup(&moved.key_ref).unwrap().expose(), SECRET);
    }

    /// Bleibt die alte Datei stehen, sagt der Befund es mit dem richtigen
    /// Namen des Schlüsselbunds.
    #[test]
    fn an_old_fallback_file_that_stays_names_the_keychain() {
        let dir = tempfile::tempdir().unwrap();
        let store = keychain_in(dir.path(), Some(fake_security(dir.path())));
        fs::create_dir_all(store.file_path("llm").join("blocker")).unwrap();
        let stored = store.store(&ApiKey::new(SECRET).unwrap()).unwrap();
        assert_eq!(stored.key_ref, KeyRef::keychain("llm").unwrap());
        let note = stored.note.expect("the leftover is reported");
        assert!(
            note.why
                .starts_with("the access key is in the macOS Keychain now")
        );
    }

    #[test]
    fn clearing_the_keychain_tolerates_a_missing_entry_and_reports_a_failure() {
        let dir = tempfile::tempdir().unwrap();
        let store = keychain_in(dir.path(), Some(fake_security(dir.path())));
        let stored = store.store(&ApiKey::new(SECRET).unwrap()).unwrap();
        store.clear(&stored.key_ref).unwrap();
        assert_eq!(store.lookup(&stored.key_ref).unwrap_err().code, LLM_009);
        store.clear(&stored.key_ref).unwrap();

        let tool = scripted_security(dir.path(), "echo 'security: refused' >&2\nexit 51");
        let refused = keychain_in(dir.path(), Some(tool))
            .clear(&stored.key_ref)
            .unwrap_err();
        assert_eq!(refused.code, CONFIG_020);
        assert!(
            refused.why.contains("security ended with 51"),
            "{}",
            refused.why
        );
    }

    #[test]
    fn a_missing_keychain_entry_and_a_refusing_keychain_read_differently() {
        let dir = tempfile::tempdir().unwrap();
        let key_ref = KeyRef::keychain("llm").unwrap();
        let store = keychain_in(dir.path(), Some(fake_security(dir.path())));
        let missing = store.lookup(&key_ref).unwrap_err();
        assert_eq!(missing.code, LLM_009);
        assert!(
            missing.why.contains("holds no such entry"),
            "{}",
            missing.why
        );

        let tool = scripted_security(dir.path(), "echo 'security: locked' >&2\nexit 36");
        let locked = keychain_in(dir.path(), Some(tool))
            .lookup(&key_ref)
            .unwrap_err();
        assert_eq!(locked.code, LLM_009);
        assert!(locked.why.contains("did not hand out"), "{}", locked.why);
        assert!(
            locked.why.contains("security ended with 36"),
            "{}",
            locked.why
        );
    }

    /// Die Frist und ihr Text gelten auch für `security`.
    #[test]
    fn a_keychain_that_never_answers_runs_into_the_deadline() {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("security-hangs");
        write_executable(&script, "#!/bin/sh\nexec sleep 30\n");
        let store = keychain_in(dir.path(), Some(script)).with_timeout(Duration::from_millis(200));
        let stored = store.store(&ApiKey::new(SECRET).unwrap()).unwrap();
        let note = stored.note.expect("the fallback is reported");
        assert!(note.why.contains("security did not answer"), "{}", note.why);
        assert!(!note.why.contains("secret-tool"), "{}", note.why);
    }

    /// Eine Konfiguration, die zwischen den Systemen wandert, nennt einen Ort,
    /// den es hier nicht gibt. Der Befund sagt das, und auf macOS schlägt er
    /// kein Paket von Linux vor.
    #[test]
    fn a_reference_of_the_other_system_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let on_macos = keychain_in(dir.path(), Some(fake_security(dir.path())));
        let foreign = on_macos
            .lookup(&KeyRef::secret_service("llm").unwrap())
            .unwrap_err();
        assert_eq!(foreign.code, LLM_009);
        assert!(foreign.why.contains("Linux desktop"), "{}", foreign.why);
        assert_eq!(foreign.fix, Some(set_again()));

        let on_linux = store_in(dir.path(), None);
        let foreign = on_linux
            .lookup(&KeyRef::keychain("llm").unwrap())
            .unwrap_err();
        assert_eq!(foreign.code, LLM_009);
        assert!(foreign.why.contains("macOS Keychain"), "{}", foreign.why);
        assert_eq!(foreign.fix, Some(set_again()));
    }

    /// Die Texte unter Linux bleiben, wie sie waren.
    #[test]
    fn the_linux_fallback_keeps_its_words() {
        let dir = tempfile::tempdir().unwrap();
        let stored = store_in(dir.path(), None)
            .store(&ApiKey::new(SECRET).unwrap())
            .unwrap();
        let note = stored.note.expect("the fallback is reported");
        assert_eq!(
            note.why,
            format!(
                "the system keyring did not take the access key (secret-tool is not installed); \
                 it is stored in {} with mode 0600 instead, readable by this account only",
                dir.path().join("keys/llm.key").display()
            )
        );
        assert_eq!(
            note.fix,
            Some(FixAction::CopyCommand(
                "sudo apt-get install libsecret-tools".to_owned()
            ))
        );
    }

    /// Nur `/usr/bin` im Suchpfad öffnet den Schlüsselbund von macOS. Ohne
    /// das erreichten die Tests mit Wegwerf-Pfaden den des Menschen.
    #[test]
    fn the_keychain_tool_needs_usr_bin_on_the_search_path() {
        let dir = tempfile::tempdir().unwrap();
        for (search, expected) in [
            (None, None),
            (Some("/does/not/exist"), None),
            (Some("/opt/x:/usr/bin:/bin"), Some(KEYCHAIN_TOOL)),
            (Some("/opt/x:/usr/bin/:/bin"), Some(KEYCHAIN_TOOL)),
        ] {
            let paths = paths_with(search, dir.path());
            let store = KeyStore::for_system(&paths, StoreKind::Keychain);
            assert_eq!(store.system, StoreKind::Keychain);
            assert_eq!(store.keychain_tool.as_deref(), expected.map(Path::new));
        }
        let paths = paths_with(Some("/usr/bin"), dir.path());
        let linux = KeyStore::for_system(&paths, StoreKind::SecretService);
        assert!(linux.keychain_tool.is_none());
    }

    /// Ein `secret-tool` im Suchpfad von macOS wird nicht gefunden, also
    /// meldet ein kopierter Verweis den fremden Ort, statt es zu fragen.
    #[test]
    fn a_secret_tool_on_the_search_path_of_macos_is_never_asked() {
        let dir = tempfile::tempdir().unwrap();
        let tool = fake_tool(dir.path());
        let search = dir.path().display().to_string();
        let paths = paths_with(Some(&search), dir.path());
        assert_eq!(
            KeyStore::for_system(&paths, StoreKind::SecretService).secret_tool,
            Some(tool)
        );
        let on_macos = KeyStore::for_system(&paths, StoreKind::Keychain);
        assert_eq!(on_macos.secret_tool, None);
        let foreign = on_macos
            .lookup(&KeyRef::secret_service("llm").unwrap())
            .unwrap_err();
        assert!(foreign.why.contains("Linux desktop"), "{}", foreign.why);
        assert!(
            !dir.path().join("argv.log").exists(),
            "the tool was started"
        );
    }

    /// Ein Verweis des anderen Systems lässt sich entfernen, ohne ein
    /// Werkzeug zu starten; der eigene Schlüsselbund scheitert weiter.
    #[test]
    fn clearing_a_reference_of_the_other_system_counts_as_gone() {
        let dir = tempfile::tempdir().unwrap();
        let broken = scripted_security(dir.path(), "exit 51");
        let on_macos = keychain_in(dir.path(), Some(broken));
        on_macos
            .clear(&KeyRef::secret_service("llm").unwrap())
            .unwrap();
        assert!(on_macos.clear(&KeyRef::keychain("llm").unwrap()).is_err());

        let on_linux = store_in(dir.path(), Some(broken_tool(dir.path())));
        on_linux.clear(&KeyRef::keychain("llm").unwrap()).unwrap();
        // Die Attrappe endet bei `clear` mit 1: Der eigene Ort wird gefragt.
        assert!(
            on_linux
                .clear(&KeyRef::secret_service("llm").unwrap())
                .is_err()
        );
    }

    /// Der Befehl zum Entfernen von Hand gehört zu Ort und System.
    #[test]
    fn the_manual_clear_names_the_tool_of_this_system() {
        let dir = tempfile::tempdir().unwrap();
        let command = |text: &str| Some(FixAction::CopyCommand(text.to_owned()));
        let on_linux = store_in(dir.path(), None);
        assert_eq!(
            on_linux.manual_clear(&KeyRef::secret_service("cloud").unwrap()),
            command("secret-tool clear service humanitl account cloud")
        );
        assert_eq!(
            on_linux.manual_clear(&KeyRef::keychain("cloud").unwrap()),
            None
        );

        let on_macos = keychain_in(dir.path(), None);
        assert_eq!(
            on_macos.manual_clear(&KeyRef::keychain("cloud").unwrap()),
            command("security delete-generic-password -s humanitl -a cloud")
        );
        assert_eq!(
            on_macos.manual_clear(&KeyRef::secret_service("cloud").unwrap()),
            None
        );

        let expected = format!("rm {}", shell_path(&on_macos.file_path("cloud")));
        assert_eq!(
            on_macos.manual_clear(&KeyRef::file("cloud").unwrap()),
            command(&expected)
        );
    }

    /// `with_secret_tool(None)` heißt für die Tests: gar kein Schlüsselbund,
    /// auch dort, wo der Speicher sonst den von macOS nähme.
    #[test]
    fn no_secret_tool_means_no_keyring_on_every_system() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_with(Some("/usr/bin"), dir.path());
        let store = KeyStore::for_system(&paths, StoreKind::Keychain).with_secret_tool(None);
        assert_eq!(store.system, StoreKind::SecretService);
        assert!(store.keychain_tool.is_none());
    }
}
