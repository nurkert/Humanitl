//! Ob `humanitl config set` und `SetConfig` des Daemons einen Wert schreiben
//! dürfen, obwohl die Konfiguration danach noch nicht lädt (HUM-070,
//! HUM-562).
//!
//! Beide Schreiber laden die Nebendatei so, wie der nächste Start sie lädt,
//! und vergleichen die Befunde mit denen vor dem Schreiben. Was dabei aus dem
//! Text eines Befunds gelesen wird (welche Schlüssel er nennt) und wann ein
//! alter Befund das Schreiben trotzdem sperrt (er steht in der Datei und
//! verhindert den Start), steht hier, damit beide Wege dieselbe Regel haben.

use std::path::{Path, PathBuf};

use humanitl_core::diagnostics::codes::{CONFIG_002, CONFIG_015};
use humanitl_core::{Diagnostic, FixAction, Severity};

use crate::{Env, PROFILE_SECTION, ProfileSelection, alias, schema};

/// Die Schlüssel, die ein Befund nennt: der früheste und alle.
///
/// Gelesen wird der Text, weil der Befund keinen Schlüssel als Feld trägt;
/// deshalb gelten drei Regeln, die ihn davor schützen, etwas Falsches zu
/// lesen. `CONFIG_002` ist ein Schlüssel, den das Schema nicht kennt: Der
/// Blattpfad in seinem Text ist ein Vorschlag (`did you mean …?`) und nicht der
/// Schlüssel, also hat er keinen. Ein alter Name zählt als sein heutiger. Bei
/// gleichem Anfang gewinnt der längere Pfad. Gesucht wird nur, wo die
/// bekannten Sätze der Ladung einen Schlüssel hinschreiben (`key_regions`);
/// ein Wert, der zufällig wie ein Schlüssel aussieht, nennt keinen.
#[must_use]
pub fn keys_named_in(diagnostic: &Diagnostic) -> (Option<String>, Vec<String>) {
    if diagnostic.code == CONFIG_002 {
        return (None, Vec::new());
    }
    let why = key_regions(&diagnostic.why).join("\n");
    let why = why.as_str();
    let mut hits: Vec<(usize, usize, &'static str)> = schema::leaf_paths()
        .into_iter()
        .flat_map(|path| {
            why.match_indices(path)
                .map(move |(at, _)| (at, path.len(), path))
        })
        .chain(alias::ALIASES.iter().flat_map(|entry| {
            why.match_indices(entry.old)
                .map(move |(at, _)| (at, entry.old.len(), entry.canonical))
        }))
        .collect();
    // Früher zuerst, bei gleichem Anfang der längere; ein Pfad, der nur der
    // Anfang eines längeren an derselben Stelle ist, zählt nicht.
    hits.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
    let mut involved: Vec<String> = Vec::new();
    let mut covered_until = 0;
    for (at, len, path) in hits {
        if at < covered_until {
            continue;
        }
        covered_until = at + len;
        if !involved.iter().any(|known| known == path) {
            involved.push(path.to_owned());
        }
    }
    (involved.first().cloned(), involved)
}

/// Die Teile eines Befundtextes, in denen ein Schlüssel stehen kann.
///
/// Zwei Sätze der Ladung tragen neben dem Schlüssel einen Wert aus der Datei
/// oder einen Pfad, und der kann aussehen wie ein Schlüssel
/// (`[ui] theme = "hold.timeout_secs"`):
///
/// - `<key> = <found> is out of range; allowed is <allowed> (default …)`
///   (`validate::out_of_range`): der Schlüssel vorn und die Grenze, die einen
///   zweiten nennen kann (`at least limits.hold_body_cap_bytes (…)`).
/// - `<key> (from <origin>) expects <expected>, found <found>`
///   (`load::bad_value`): nur der Schlüssel vorn.
///
/// Jeder andere Text zählt als Ganzes. Das liest im Zweifel einen Schlüssel
/// zu viel, und ein Schlüssel zu viel verhindert ein Schreiben, statt eines
/// zu erlauben.
fn key_regions(why: &str) -> Vec<&str> {
    const OUT_OF_RANGE: &str = " is out of range; allowed is ";
    if let Some((subject, rest)) = why.split_once(" = ")
        && let Some((_, allowed)) = rest.rsplit_once(OUT_OF_RANGE)
    {
        let allowed = allowed
            .rsplit_once(" (default ")
            .map_or(allowed, |(allowed, _)| allowed);
        return vec![subject, allowed];
    }
    if let Some((subject, rest)) = why.split_once(" (from ")
        && rest.contains(" expects ")
    {
        return vec![subject];
    }
    vec![why]
}

/// Der TOML-Text ohne den Schlüssel `key` und ohne jeden seiner alten Namen,
/// jeweils unter dem Präfix `prefix`.
#[must_use]
pub fn without_everywhere(text: &str, key: &str, prefix: &[String]) -> String {
    std::iter::once(key)
        .chain(alias::old_names(key))
        .fold(text.to_owned(), |text, name| {
            let mut segments = prefix.to_vec();
            segments.extend(name.split('.').map(ToOwned::to_owned));
            without_key(&text, &segments)
        })
}

/// Der TOML-Text ohne den Schlüssel unter `segments`; unverändert, wenn er
/// dort nicht steht oder der Text kein TOML ist.
#[must_use]
pub fn without_key(text: &str, segments: &[String]) -> String {
    let Ok(mut document) = text.parse::<toml_edit::DocumentMut>() else {
        return text.to_owned();
    };
    let Some((last, groups)) = segments.split_last() else {
        return text.to_owned();
    };
    let mut cursor: &mut dyn toml_edit::TableLike = document.as_table_mut();
    for group in groups {
        match cursor
            .get_mut(group)
            .and_then(toml_edit::Item::as_table_like_mut)
        {
            Some(next) => cursor = next,
            None => return text.to_owned(),
        }
    }
    cursor.remove(last);
    document.to_string()
}

/// Der Schlüssel eines Befunds, der den Start verhindert und aus
/// `file_text` kommt, sonst keiner (HUM-562).
///
/// Ein solcher Befund sperrt jedes Schreiben, das ihn nicht repariert: Ein
/// Erfolg für einen anderen Schlüssel hinterließe eine Datei, mit der der
/// Daemon nicht startet. Aus der Datei kommt er, wenn sein Schlüssel unter
/// `prefix` darin steht und der Befund ohne diesen Eintrag verschwindet;
/// `persists_without` lädt dafür die Konfiguration mit dem Text ohne den
/// Schlüssel und sagt, ob derselbe Befund bleibt. Ein Befund aus einer
/// anderen Ebene (die Umgebung, ein Profil, das den Wert der Datei
/// überschreibt) sperrt nicht: Der Schreiber könnte ihn nicht beheben. Ob das
/// Schreiben repariert (den Schlüssel des Befunds setzt oder Befunde
/// wegnimmt), entscheidet der Aufrufer, denn nur er kennt die Befunde vorher.
pub fn blocking_key(
    finding: &Diagnostic,
    file_text: &str,
    prefix: &[String],
    persists_without: impl Fn(&str) -> bool,
) -> Option<String> {
    if !matches!(finding.severity, Severity::Error | Severity::Blocking) {
        return None;
    }
    // Jeder Schlüssel, den der Befund nennt, nicht nur der erste: Bei einem
    // Paar kann der erste aus der Umgebung kommen und der zweite in der Datei
    // stehen; ohne ihn verschwände der Befund.
    let (_, involved) = keys_named_in(finding);
    involved.into_iter().find(|key| {
        let without = without_everywhere(file_text, key, prefix);
        without != file_text && !persists_without(&without)
    })
}

/// Die Ablehnung eines Schreibens von `set_key`, weil `finding` schon in der
/// Datei steht und den Start verhindert (HUM-562): derselbe Code, derselbe
/// Text und derselbe Vorschlag, dazu der Satz, warum nicht geschrieben wird.
#[must_use]
pub fn blocked_write(set_key: &str, finding: &Diagnostic) -> Diagnostic {
    Diagnostic::builder(finding.code, finding.severity)
        .why(format!(
            "{}; the daemon does not start with this file, so {set_key} is not written until \
             that is repaired",
            finding.why
        ))
        .maybe_fix(finding.fix.clone())
        .build()
}

/// Was die Prüfung über die übrigen Ebenen wissen muss: dieselben Eingaben,
/// mit denen der nächste Start lädt.
#[derive(Debug, Clone)]
pub struct ProbeContext {
    /// Die Profilauswahl (die Kommandozeile oder keine).
    pub selection: ProfileSelection,
    /// Das Verzeichnis, nach dessen Projekt-Profil gesucht wird; `None` für
    /// den Daemon, der kein Arbeitsverzeichnis eines Projekts hat.
    pub cwd: Option<PathBuf>,
    /// Die Umgebung.
    pub env: Env,
    /// Die wirkliche globale `config.toml`.
    pub global: PathBuf,
}

/// Wohin `config set` oder `SetConfig` schreibt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// `config.toml` des Menschen.
    Global,
    /// `<projekt>/.humanitl/profile.toml`, unter `[config]`.
    Project,
}

impl Scope {
    /// Das Wort für die Ausgabe.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Project => "project",
        }
    }
}

/// Lädt die Konfiguration so, wie der nächste Start sie lädt — mit der
/// Nebendatei an der Stelle der wirklichen — und entscheidet, ob der Wert
/// für `key` geschrieben werden darf; dieselbe Entscheidung für
/// `humanitl config set` und `SetConfig` des Daemons (HUM-562).
///
/// Die Quellen werden dafür **mit** der Nebendatei neu bestimmt und nicht an
/// einem vorab bestimmten Satz ausgetauscht: Welches Projekt-Profil gilt,
/// hängt an `sandbox.work_dir`, und `config set sandbox.work_dir` ändert
/// genau das.
///
/// Geschrieben wird, wenn die Konfiguration mit dem neuen Wert lädt, **oder**
/// wenn jeder Befund nachher genau so schon vorher dastand: derselbe Code am
/// selben Schlüssel mit demselben Text, und keiner nennt den gesetzten
/// Schlüssel. Ein Wert, der selbst falsch ist, wird nicht geschrieben, auch
/// wenn derselbe falsche Wert schon dastand. Ein Befund, der sich durch den neuen
/// Wert nur im Text ändert — ein Paar wie `limits.hold_max_bytes` und
/// `limits.hold_body_cap_bytes`, dessen Grenze der neue Wert verschiebt —, ist
/// ein neuer Befund. Ein Befund ohne Schlüssel verhindert das Schreiben immer.
/// Die Ladung meldet nur ihren ersten Befund; alle Befunde einer Ebenenfolge
/// sammelt deshalb [`findings`]. So verdeckt ein älterer Fehler keinen neuen,
/// und eine Konfiguration mit zwei falschen Werten lässt sich trotzdem
/// Schlüssel für Schlüssel reparieren.
///
/// Ein Befund, der schon vorher dastand und den Start verhindert, sperrt
/// trotzdem jedes Schreiben, das ihn nicht repariert ([`blocking_key`]).
///
/// `Ok(Some(why))`: geschrieben, obwohl die Konfiguration noch nicht lädt;
/// `why` ist der älteste Befund, den der Aufrufer dazu nennen kann.
///
/// # Errors
///
/// `CONFIG_015`, wenn die Nebendatei nicht lesbar ist, sonst der Befund, mit
/// dem das Schreiben abgelehnt wird.
pub fn check_write(
    probe: &ProbeContext,
    key: &str,
    scope: Scope,
    file: &Path,
    candidate: &Path,
) -> Result<Option<String>, Diagnostic> {
    let candidate_text = std::fs::read_to_string(candidate).map_err(|error| {
        Diagnostic::builder(CONFIG_015, Severity::Error)
            .why(format!(
                "{} could not be read back for the check: {error}",
                candidate.display()
            ))
            .build()
    })?;
    let after = findings(probe, scope, Some(&candidate_text), file);
    if after.is_empty() {
        return Ok(None);
    }
    let current_text = std::fs::read_to_string(file).ok();
    let before = findings(probe, scope, current_text.as_deref(), file);
    let new = after.iter().find(|finding| {
        finding.key.is_none()
            || finding.involved.iter().any(|named| named == key)
            || !before.iter().any(|old| old.same_as(finding))
    });
    if let Some(finding) = new {
        return Err(refusal(key, finding, !before.is_empty(), file, candidate));
    }
    // Alles, was jetzt noch scheitert, scheiterte schon vorher genau so; der
    // Wert bringt nichts Neues mit. Geschrieben wird trotzdem nur, wenn er der
    // Reparatur dient (HUM-562): Steht ein Befund, der den Start verhindert,
    // in der Datei, die geschrieben wird, und der Wert ändert weder dessen
    // Schlüssel noch verringert er die Befunde, meldete der Schreiber Erfolg
    // für eine Datei, mit der der Daemon nicht startet.
    if let Some(blocking) =
        still_blocking(probe, file, key, scope, &candidate_text, &before, &after)
    {
        let mut diagnostic = blocking.diagnostic.clone();
        diagnostic.why = diagnostic.why.replace(
            &candidate.display().to_string(),
            &file.display().to_string(),
        );
        return Err(blocked_write(key, &diagnostic));
    }
    Ok(before.first().map(|first| first.diagnostic.why.clone()))
}

/// Der Befund, der den Start auch nach dem Schreiben verhindert und aus der
/// geschriebenen Datei kommt, sofern das Schreiben nichts repariert
/// (HUM-562).
///
/// Repariert wird, wenn der gesetzte Schlüssel einer der Schlüssel eines
/// Befunds von vorher ist (`resolver.test_ca` auf einen absoluten Pfad oder
/// `-`, der Reihe nach bei zwei falschen Werten) oder wenn es nachher weniger
/// Befunde gibt.
fn still_blocking<'a>(
    probe: &ProbeContext,
    file: &Path,
    key: &str,
    scope: Scope,
    candidate_text: &str,
    before: &[Finding],
    after: &'a [Finding],
) -> Option<&'a Finding> {
    let repairs = before
        .iter()
        .any(|old| old.involved.iter().any(|named| named == key))
        || after.len() < before.len();
    if repairs {
        return None;
    }
    let prefix = match scope {
        Scope::Global => Vec::new(),
        Scope::Project => vec![PROFILE_SECTION.to_owned()],
    };
    after.iter().find(|finding| {
        blocking_key(&finding.diagnostic, candidate_text, &prefix, |without| {
            findings(probe, scope, Some(without), file)
                .iter()
                .any(|other| other.same_as(finding))
        })
        .is_some()
    })
}

/// Der Befund, mit dem `config set` ablehnt.
fn refusal(
    key: &str,
    finding: &Finding,
    broken_before: bool,
    file: &Path,
    candidate: &Path,
) -> Diagnostic {
    // Die Ladung nennt die Nebendatei; der Mensch kennt nur seine Datei.
    let why = finding.diagnostic.why.replace(
        &candidate.display().to_string(),
        &file.display().to_string(),
    );
    let names_the_key = finding.involved.iter().any(|named| named == key);
    // Nennt der Befund den gesetzten Schlüssel, gilt sein eigener Vorschlag;
    // sonst ist die Datei als Ganzes zu richten. Lud die Konfiguration schon
    // vorher nicht, sagt der Text das in jedem Fall.
    let fix = if finding.key.is_none() {
        // Ein Befund ohne Schlüssel kann aus einer Ebene kommen, die der
        // Mensch hier nicht schreibt, etwa aus dem Projekt-Profil, das
        // `sandbox.work_dir` gerade wählt. Dessen Vorschlag trüge einen Wert
        // aus dem Projekt in die globale Datei; die Grenze zwischen beiden
        // bleibt, und der Vorschlag ist die Datei als Ganzes.
        Some(FixAction::CopyCommand("humanitl config edit".to_owned()))
    } else if names_the_key {
        finding
            .diagnostic
            .fix
            .clone()
            .or_else(|| schema::field(key).and_then(crate::value::default_fix))
    } else if broken_before {
        Some(FixAction::CopyCommand("humanitl config edit".to_owned()))
    } else {
        finding.diagnostic.fix.clone()
    };
    let why = if broken_before {
        format!(
            "{why}; the configuration did not load before this change either, and this \
             finding is new or cannot be told apart from the older ones"
        )
    } else {
        why
    };
    let mut builder =
        Diagnostic::builder(finding.diagnostic.code, finding.diagnostic.severity).why(why);
    if let Some(fix) = fix {
        builder = builder.fix(fix);
    }
    builder.build()
}

/// Ein Befund der Ladung und die Schlüssel, die er nennt.
#[derive(Debug)]
pub struct Finding {
    /// Der Befund, wie die Ladung ihn meldet.
    pub diagnostic: Diagnostic,
    /// Der Schlüssel, um den es geht: der erste, den der Text nennt. `None`,
    /// wenn der Text keinen nennt oder der Befund keinen haben kann
    /// (`CONFIG_002`, ein Schlüssel, den das Schema nicht kennt).
    pub key: Option<String>,
    /// Alle Schlüssel, die der Text nennt, in der Reihenfolge ihres
    /// Vorkommens; bei einem Befund über ein Paar beide.
    pub involved: Vec<String>,
}

impl Finding {
    /// Derselbe Befund: derselbe Code am selben Schlüssel mit demselben Text.
    ///
    /// Der Text zählt mit: Ein Befund über ein Paar behält seinen Schlüssel,
    /// wenn der neue Wert die Grenze des anderen verschiebt, aber nicht seinen
    /// Text. Der Pfad der Nebendatei wird dafür vorher ersetzt
    /// ([`findings`]).
    #[must_use]
    pub fn same_as(&self, other: &Self) -> bool {
        self.key.is_some()
            && self.key == other.key
            && self.diagnostic.code == other.diagnostic.code
            && self.diagnostic.why == other.diagnostic.why
    }
}

/// Wie oft [`findings`] höchstens einen Schlüssel herausnimmt und neu lädt.
const MAX_FINDINGS: usize = 16;

/// Alle Befunde einer Ebenenfolge, in der Reihenfolge, in der die Ladung sie
/// meldet.
///
/// `file_text` ist der Text der Datei, die `config set` schreibt (oder `None`,
/// wenn es sie nicht gibt); die übrigen Ebenen kommen von der Platte und aus
/// der Umgebung. Nach jedem Befund wird sein Schlüssel — samt seinen alten
/// Namen — aus der globalen Datei, aus dem Projekt-Profil und aus der Umgebung
/// genommen und neu geladen, bis die Ladung gelingt. Ein Befund ohne
/// Schlüssel, einer, der sich unverändert wiederholt (der Fehler steckt in
/// einer Ebene, die hier nicht verändert wird), und das Ende der Runden
/// beenden das Zählen mit einem Befund ohne Schlüssel: Was danach käme, ist
/// nicht gezählt, und ein unvollständiges Bild darf nie zum Schreiben führen.
#[must_use]
pub fn findings(
    probe: &ProbeContext,
    scope: Scope,
    file_text: Option<&str>,
    file: &Path,
) -> Vec<Finding> {
    let Ok(scratch) = tempfile::tempdir() else {
        return vec![unknown_finding(
            file,
            "no temporary directory for the check",
        )];
    };
    findings_in(
        probe,
        scope,
        file_text,
        file,
        scratch.path(),
        |path, text| std::fs::write(path, text),
    )
}

/// Schreibt eine Nebendatei; im Betrieb `std::fs::write`, im Test einer, der
/// scheitert.
pub type ScratchWriter = fn(&Path, &str) -> std::io::Result<()>;

/// [`findings`] mit den Nebendateien in `scratch`, geschrieben von `write`.
#[must_use]
pub fn findings_in(
    probe: &ProbeContext,
    scope: Scope,
    file_text: Option<&str>,
    file: &Path,
    scratch: &Path,
    write: ScratchWriter,
) -> Vec<Finding> {
    let mut layers = Layers::new(probe, scope, file_text, file, write);
    let mut out: Vec<Finding> = Vec::new();

    for round in 0..MAX_FINDINGS {
        let Err(diagnostic) = layers.load(probe, scratch, round) else {
            return out;
        };
        let (key, involved) = keys_named_in(&diagnostic);
        let repeated = out.last().is_some_and(|last| {
            last.diagnostic.code == diagnostic.code
                && last.key == key
                && last.diagnostic.why == diagnostic.why
        });
        if repeated {
            out.push(unknown_finding(
                file,
                "a finding repeats after its key was taken out of every layer, so the rest \
                 cannot be counted",
            ));
            return out;
        }
        let peel = key.clone();
        out.push(Finding {
            diagnostic,
            key,
            involved,
        });
        let Some(peel) = peel else {
            return out;
        };
        layers.peel(&peel);
    }
    out.push(unknown_finding(
        file,
        "the check stopped after its last round, so not every finding is counted",
    ));
    out
}

/// Die Ebenen, die [`findings`] Runde um Runde lädt und aus denen es
/// Schlüssel herausnimmt.
struct Layers {
    /// Der Text der globalen Datei, `None`, wenn es keine gibt.
    global: Option<String>,
    /// Die globale Datei, für die die Nebendatei im Text eines Befunds steht.
    global_real: PathBuf,
    /// Der Text des Projekt-Profils, sobald er festgehalten ist.
    project: Option<String>,
    /// Das Projekt-Profil, für das die Nebendatei im Text steht.
    project_real: Option<PathBuf>,
    /// Ob das Projekt-Profil schon gelesen und festgehalten ist.
    project_pinned: bool,
    /// Die Umgebung ohne die schon herausgenommenen Variablen.
    env: Env,
    /// Die schon herausgenommenen Schlüssel, in ihrer Reihenfolge.
    peeled: Vec<String>,
    /// Was die Nebendateien schreibt.
    write: ScratchWriter,
}

impl Layers {
    /// Die Ebenen am Anfang: die Datei, die `config set` schreibt, an ihrer
    /// Stelle, alles andere von der Platte und aus der Umgebung.
    fn new(
        probe: &ProbeContext,
        scope: Scope,
        file_text: Option<&str>,
        file: &Path,
        write: ScratchWriter,
    ) -> Self {
        let (global, project, project_real) = match scope {
            Scope::Global => (file_text.map(ToOwned::to_owned), None, None),
            Scope::Project => (
                std::fs::read_to_string(&probe.global).ok(),
                file_text.map(ToOwned::to_owned),
                Some(file.to_path_buf()),
            ),
        };
        Self {
            global,
            global_real: probe.global.clone(),
            project,
            project_real,
            project_pinned: scope == Scope::Project,
            env: probe.env.clone(),
            peeled: Vec::new(),
            write,
        }
    }

    /// Lädt die Ebenen dieser Runde aus Nebendateien in `scratch`.
    ///
    /// Im Text eines Befunds steht danach die Datei, für die eine Nebendatei
    /// steht: Der Mensch kennt nur seine, und ein Vergleich mit der anderen
    /// Folge darf nicht an einem Namen einer Nebendatei scheitern.
    fn load(
        &mut self,
        probe: &ProbeContext,
        scratch: &Path,
        round: usize,
    ) -> Result<(), Diagnostic> {
        let write = self.write;
        let global_path = self
            .global
            .as_ref()
            .map(|text| write_scratch(write, &scratch.join(format!("global-{round}.toml")), text))
            .transpose()?;
        let mut written: Vec<(PathBuf, PathBuf)> = global_path
            .iter()
            .map(|path| (path.clone(), self.global_real.clone()))
            .collect();
        let result = crate::sources_with_global(
            &probe.selection,
            probe.cwd.as_deref(),
            &self.env,
            &[],
            global_path,
        )
        .and_then(|mut sources| {
            if !self.project_pinned {
                // Das Projekt-Profil der wirklichen Folge wird einmal gelesen
                // und danach wie die globale Datei behandelt; was schon
                // herausgenommen ist, fehlt auch darin.
                self.project_real.clone_from(&sources.profile_project);
                self.project = sources
                    .profile_project
                    .as_ref()
                    .and_then(|path| std::fs::read_to_string(path).ok())
                    .map(|text| {
                        self.peeled
                            .iter()
                            .fold(text, |text, key| without_in_profile(&text, key))
                    });
                self.project_pinned = true;
            }
            sources.profile_project = self
                .project
                .as_ref()
                .map(|text| {
                    write_scratch(write, &scratch.join(format!("project-{round}.toml")), text)
                })
                .transpose()?;
            if let (Some(temp), Some(real)) = (&sources.profile_project, &self.project_real) {
                written.push((temp.clone(), real.clone()));
            }
            crate::load(&sources).map(|_| ())
        });
        result.map_err(|mut diagnostic| {
            for (temp, real) in &written {
                diagnostic.why = diagnostic
                    .why
                    .replace(&temp.display().to_string(), &real.display().to_string());
            }
            diagnostic
        })
    }

    /// Nimmt einen Schlüssel aus allen Ebenen: aus der globalen Datei, aus dem
    /// Projekt-Profil und aus der Umgebung, jeweils samt seinen alten Namen.
    fn peel(&mut self, key: &str) {
        self.global = self
            .global
            .take()
            .map(|text| without_everywhere(&text, key, &[]));
        self.project = self
            .project
            .take()
            .map(|text| without_in_profile(&text, key));
        self.env = without_variable(&self.env, key);
        self.peeled.push(key.to_owned());
    }
}

/// Schreibt eine Nebendatei für [`Layers::load`].
///
/// Ein Schreibfehler ist ein Befund ohne Schlüssel und verhindert damit das
/// Schreiben. Still übergangen bliebe eine leere oder halbe Nebendatei zurück
/// (etwa bei vollem `TMPDIR`), die als Vorgabe lädt, und die Prüfung meldete
/// eine Konfiguration als gut, die sie nie gesehen hat. Den Vorschlag setzt
/// [`refusal`], wie für jeden Befund ohne Schlüssel.
fn write_scratch(write: ScratchWriter, path: &Path, text: &str) -> Result<PathBuf, Diagnostic> {
    write(path, text).map_err(|error| {
        Diagnostic::builder(CONFIG_015, Severity::Error)
            .why(format!(
                "{}: the check could not write its scratch copy: {error}",
                path.display()
            ))
            .build()
    })?;
    Ok(path.to_path_buf())
}

/// Ein Befund, der keinen Schlüssel hat, weil die Prüfung selbst nicht zu
/// Ende kam. Er verhindert das Schreiben.
fn unknown_finding(file: &Path, why: &str) -> Finding {
    Finding {
        diagnostic: Diagnostic::builder(CONFIG_015, Severity::Error)
            .why(format!("{}: {why}", file.display()))
            .build(),
        key: None,
        involved: Vec::new(),
    }
}

/// Das Projekt-Profil ohne den Schlüssel unter `[config]`.
fn without_in_profile(text: &str, key: &str) -> String {
    without_everywhere(text, key, &[PROFILE_SECTION.to_owned()])
}

/// Die Umgebung ohne die Variablen, die diesen Schlüssel setzen, unter
/// seinem heutigen und unter jedem alten Namen.
fn without_variable(env: &Env, key: &str) -> Env {
    let head = format!("{}_", crate::DEFAULT_ENV_PREFIX);
    Env::from_pairs(
        env.iter()
            .filter(|(name, _)| {
                let Some(rest) = name.strip_prefix(&head) else {
                    return true;
                };
                let path = rest
                    .to_lowercase()
                    .split(crate::ENV_SEPARATOR)
                    .collect::<Vec<_>>()
                    .join(".");
                let canonical = alias::canonical(&path).unwrap_or(path.as_str());
                canonical != key
            })
            .map(|(name, value)| (name.clone(), value.clone())),
    )
}
