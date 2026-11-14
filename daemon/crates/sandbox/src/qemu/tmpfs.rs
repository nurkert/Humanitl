//! Die beschreibbaren Orte des Gasts (HUM-466).
//!
//! Unter bwrap ist jede `--tmpfs`-Zeile außerhalb von `/work` ein leeres,
//! beschreibbares Dateisystem: `/tmp`, `/var/tmp`, das Heimatverzeichnis des
//! Agenten. In der nur lesbaren Wurzel des Gasts sind diese Zeilen leere
//! Verzeichnisse; der Shim hängt im Gast über jedes ein tmpfs, mit Rechten
//! wie unter bwrap (0755, dem Agenten gehörend, `nosuid`, `nodev`, die
//! Vorgabegröße des Kerns), bevor der Agent startet (`guest_tmpfs.rs` des
//! Shims).
//!
//! bwrap hängt seine tmpfs vor den Binds ein, die in der Zeile danach kommen:
//! Eine Datei des Adapters unter `~/.config/<agent>/` liegt obenauf. Im Gast
//! kommt das tmpfs zuletzt und verdeckte sie. Deshalb nennt der Plan, was
//! unter einem tmpfs liegt und obenauf bleiben muss (`tmpfs_keep`): Ein Bind
//! wird als nur lesbare Kopie der Einhängung der Wurzel auf das tmpfs
//! verschoben, ein Verweis neu angelegt, ein Verzeichnis (etwa ein
//! Einhängepunkt) neu erzeugt.
//!
//! `/dev/shm` hängt der Shim immer ein (HUM-445); es steht deshalb nicht in
//! der Liste. Was die Wurzel unter `/dev` trägt, verdeckt im Gast das eigene
//! `devtmpfs` des Kerns, bevor der Shim es greifen könnte; ein solcher
//! Eintrag unter einem tmpfs wird abgelehnt statt still zu fehlen.
//!
//! `/run` ist unter bwrap beschreibbar, ohne dass das Profil es nennt: bwrap
//! legt das Verzeichnis auf seiner eigenen, beschreibbaren Wurzel an (für
//! `/run/humanitl`). Im Gast bekommt `/run` deshalb immer ein tmpfs, ganz
//! unten wie die Wurzel von bwrap, mit denselben Rechten wie jedes andere
//! tmpfs des Gasts (0755, dem Agenten gehörend; unter bwrap entsteht das
//! Verzeichnis mit 0755 bei der üblichen umask 022). Was die Wurzel unter
//! `/run` trägt, bleibt obenauf, je Verzeichnis direkt unter `/run` als eine
//! nur lesbare Kopie: `/run/humanitl` mit Shim, Gastplan und Modulen bleibt
//! so sichtbar und nur lesbar.

use std::path::{Path, PathBuf};

use humanitl_core::diagnostics::codes::SANDBOX_006;
use humanitl_core::{Diagnostic, Severity};

use super::lines::RootEntry;

/// Das tmpfs, das der Shim im Gast immer selbst einhängt (HUM-445).
pub(crate) const DEV_SHM: &str = "/dev/shm";

/// Das Gerätedateisystem des Gastkerns: Was die Wurzel darunter trägt, sieht
/// der Gast nie.
const DEV: &str = "/dev";

/// Unter bwrap beschreibbar, ohne Zeile im Profil; im Gast immer ein tmpfs.
pub(crate) const RUN: &str = "/run";

/// Die tmpfs des Gasts und was obenauf bleibt, in der Reihenfolge der Zeile.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct GuestTmpfs {
    /// Wohin ein tmpfs kommt.
    pub(crate) points: Vec<PathBuf>,
    /// Was obenauf bleibt, als `bind:`, `link:` oder `dir:` mit dem Pfad
    /// (die Form des Gastplans).
    pub(crate) keep: Vec<String>,
    /// Die Ziele der Binds unter `keep`: Jedes wird im Gast eine eigene,
    /// nur lesbare Einhängung, die die Mounttabelle des Plans nennt.
    pub(crate) binds: Vec<PathBuf>,
}

impl GuestTmpfs {
    /// Nimmt `path` mit der Art von `entry` in `keep` auf; ein Bind wird
    /// auch in `binds` vermerkt.
    fn keep_entry(&mut self, entry: &RootEntry, path: &Path) {
        let kind = match entry {
            RootEntry::Bind { .. } => "bind",
            RootEntry::Symlink { .. } => "link",
            RootEntry::Dir(_) | RootEntry::Tmpfs(_) => "dir",
        };
        self.keep_as(kind, path);
    }

    fn keep_as(&mut self, kind: &str, path: &Path) {
        if kind == "bind" {
            self.binds.push(path.to_path_buf());
        }
        self.keep.push(format!("{kind}:{}", path.display()));
    }
}

/// Das Ziel eines Eintrags der Wurzel.
fn target(entry: &RootEntry) -> &Path {
    match entry {
        RootEntry::Bind { target, .. }
        | RootEntry::Symlink { target, .. }
        | RootEntry::Tmpfs(target)
        | RootEntry::Dir(target) => target,
    }
}

/// `SANDBOX_006`: eine Folge von Einhängungen, die der Gast nicht in der
/// Reihenfolge von bwrap nachbauen kann.
fn unreproducible(why: String) -> Diagnostic {
    Diagnostic::builder(SANDBOX_006, Severity::Blocking)
        .why(why)
        .build()
}

/// Liest die tmpfs des Gasts aus den Einträgen der Wurzel, in deren
/// Reihenfolge (der von bwrap).
///
/// Ein Eintrag bleibt obenauf, wenn er nach einem tmpfs kommt und auf oder
/// unter dessen Pfad liegt. Liegt er schon unter einem Bind, der obenauf
/// bleibt, bringt diese Kopie ihn mit: Die Wurzel des Gasts zeigt ihn dort
/// bereits, und ein eigener Einhängepunkt in der nur lesbaren Kopie ließe
/// sich nicht anlegen. Unter `/run` (ohne eigene Zeile im Profil) bleibt je
/// Verzeichnis direkt unter `/run` eine Kopie obenauf.
///
/// # Errors
///
/// `SANDBOX_006`, wenn ein tmpfs auf oder unter einem Bind liegt, der
/// obenauf bleibt, gleich ob es davor oder danach kommt (der Shim
/// verschiebt jede Kopie nach allen tmpfs und verdeckte es), oder wenn ein Eintrag unter einem tmpfs in
/// `/dev` liegt, wo der Gast ihn nicht greifen kann.
pub(crate) fn guest_tmpfs(entries: &[RootEntry]) -> Result<GuestTmpfs, Diagnostic> {
    let run = Path::new(RUN);
    let run_listed = entries
        .iter()
        .any(|entry| matches!(entry, RootEntry::Tmpfs(point) if point == run));
    let mut tmpfs = GuestTmpfs::default();
    if !run_listed {
        tmpfs.points.push(run.to_path_buf());
    }
    // Was ein tmpfs verdeckt: die Punkte der Liste und `/dev/shm`.
    let mut covering: Vec<PathBuf> = Vec::new();
    for entry in entries {
        let path = target(entry);
        if let RootEntry::Tmpfs(point) = entry {
            if point != Path::new(DEV_SHM) {
                tmpfs.points.push(point.clone());
            }
            covering.push(point.clone());
            continue;
        }
        if tmpfs.binds.iter().any(|bind| path.starts_with(bind)) {
            continue;
        }
        if covering.iter().any(|point| path.starts_with(point)) {
            if path.starts_with(DEV) {
                return Err(unreproducible(format!(
                    "{} lies under a tmpfs in /dev, where the guest kernel's own device file \
                     system hides what the guest root carries",
                    path.display()
                )));
            }
            tmpfs.keep_entry(entry, path);
        } else if !run_listed
            && let Ok(below) = path.strip_prefix(run)
            && let Some(first) = below.components().next()
        {
            let top = run.join(first);
            if top == path {
                tmpfs.keep_entry(entry, path);
            } else {
                tmpfs.keep_as("bind", &top);
            }
        }
    }
    // Gleich in welcher Reihenfolge: Der Shim verschiebt jede Kopie erst
    // nach allen tmpfs, ein tmpfs auf oder unter ihr läge also verdeckt.
    for point in tmpfs.points.iter().chain(&covering) {
        if let Some(bind) = tmpfs.binds.iter().find(|bind| point.starts_with(bind)) {
            return Err(unreproducible(format!(
                "the tmpfs at {} lies on or under the read-only copy of {}, which the kernel \
                 level moves on top after every tmpfs; the tmpfs would be hidden",
                point.display(),
                bind.display()
            )));
        }
    }
    Ok(tmpfs)
}

/// Die Wurzeln der Socket-Suche des Gasts: `/work`, `/dev` (mit
/// `/dev/shm`) und jedes tmpfs, das nicht schon unter einer davon liegt.
/// In der nur lesbaren Wurzel und den nur lesbaren Kopien führt kein Socket
/// irgendwohin (HUM-444).
pub(crate) fn socket_walk(work: &Path, tmpfs: &GuestTmpfs) -> Vec<PathBuf> {
    let mut roots = vec![work.to_path_buf(), PathBuf::from("/dev")];
    for point in &tmpfs.points {
        if !roots.iter().any(|root| point.starts_with(root)) {
            roots.push(point.clone());
        }
    }
    roots
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    fn bind(target: &str) -> RootEntry {
        RootEntry::Bind {
            source: PathBuf::from("/src"),
            target: PathBuf::from(target),
        }
    }

    fn tmpfs(point: &str) -> RootEntry {
        RootEntry::Tmpfs(PathBuf::from(point))
    }

    /// Was vor einem tmpfs kommt, verdeckt es wie unter bwrap; was danach
    /// darunter liegt, bleibt obenauf; `/dev/shm` hängt der Shim selbst ein;
    /// `/run` bekommt immer ein tmpfs, ganz vorn.
    #[test]
    fn what_follows_a_tmpfs_beneath_it_stays_on_top() {
        let entries = [
            bind("/usr"),
            RootEntry::Symlink {
                link: PathBuf::from("usr/bin"),
                target: PathBuf::from("/bin"),
            },
            bind("/tmp/before"),
            tmpfs("/tmp"),
            tmpfs("/dev/shm"),
            tmpfs("/home/agent"),
            bind("/etc/passwd"),
            bind("/home/agent/.config/agent/config.json"),
            RootEntry::Symlink {
                link: PathBuf::from("/x"),
                target: PathBuf::from("/home/agent/link"),
            },
            RootEntry::Dir(PathBuf::from("/home/agent/proj")),
            bind("/home/agent/tree"),
            bind("/home/agent/tree/inner.json"),
            bind("/tmpx/y"),
        ];
        let found = guest_tmpfs(&entries).unwrap();
        assert_eq!(
            found.points,
            ["/run", "/tmp", "/home/agent"].map(PathBuf::from)
        );
        assert_eq!(
            found.keep,
            [
                "bind:/home/agent/.config/agent/config.json",
                "link:/home/agent/link",
                "dir:/home/agent/proj",
                "bind:/home/agent/tree",
            ]
        );
        assert_eq!(
            found.binds,
            ["/home/agent/.config/agent/config.json", "/home/agent/tree",].map(PathBuf::from)
        );
    }

    /// Unter `/run` bleibt je Verzeichnis direkt darunter eine nur lesbare
    /// Kopie obenauf, gleich in welcher Reihenfolge; eine Datei oder ein
    /// Verweis direkt unter `/run` bleibt selbst. Nennt das Profil `/run`,
    /// gilt die Reihenfolge der Zeile wie für jedes tmpfs.
    #[test]
    fn run_is_a_tmpfs_with_its_subtrees_on_top() {
        let entries = [
            bind("/run/humanitl/humanitl-shim"),
            tmpfs("/tmp"),
            bind("/run/humanitl/modules"),
            bind("/run/humanitl/guest-plan.json"),
            bind("/run/file"),
            RootEntry::Symlink {
                link: PathBuf::from("/x"),
                target: PathBuf::from("/run/link"),
            },
        ];
        let found = guest_tmpfs(&entries).unwrap();
        assert_eq!(found.points, ["/run", "/tmp"].map(PathBuf::from));
        assert_eq!(
            found.keep,
            ["bind:/run/humanitl", "bind:/run/file", "link:/run/link"]
        );
        assert_eq!(
            found.binds,
            ["/run/humanitl", "/run/file"].map(PathBuf::from)
        );

        let listed = guest_tmpfs(&[
            bind("/run/humanitl/humanitl-shim"),
            tmpfs("/run"),
            bind("/run/humanitl/modules"),
        ])
        .unwrap();
        assert_eq!(listed.points, [PathBuf::from("/run")]);
        assert_eq!(listed.keep, ["bind:/run/humanitl/modules"]);
    }

    /// Ein tmpfs auf oder unter einem Bind, der obenauf bleibt, ließe sich im
    /// Gast nur in anderer Reihenfolge als unter bwrap einhängen: abgelehnt.
    #[test]
    fn a_tmpfs_on_a_kept_bind_is_refused() {
        for nested in ["/home/agent/tree/cache", "/home/agent/tree"] {
            let refused = guest_tmpfs(&[
                tmpfs("/home/agent"),
                bind("/home/agent/tree"),
                tmpfs(nested),
            ])
            .unwrap_err();
            assert_eq!(refused.code.as_str(), "SANDBOX_006", "{nested}");
            assert!(refused.why.contains(nested), "{}", refused.why);
        }
        let below_run =
            guest_tmpfs(&[bind("/run/humanitl/x"), tmpfs("/run/humanitl/t")]).unwrap_err();
        assert_eq!(below_run.code.as_str(), "SANDBOX_006");
        // Auch davor: ein tmpfs unter `/run/humanitl` aus dem Profil, und
        // allgemein ein tmpfs, nach dem ein Bind obenauf bleibt, der es enthält.
        let profile_run = guest_tmpfs(&[
            tmpfs("/tmp"),
            tmpfs("/run/humanitl/cache"),
            bind("/run/humanitl/humanitl-shim"),
        ])
        .unwrap_err();
        assert_eq!(profile_run.code.as_str(), "SANDBOX_006");
        assert!(
            profile_run.why.contains("/run/humanitl/cache"),
            "{}",
            profile_run.why
        );
        let before = guest_tmpfs(&[
            tmpfs("/home/agent"),
            tmpfs("/home/agent/tree/cache"),
            bind("/home/agent/tree"),
        ])
        .unwrap_err();
        assert_eq!(before.code.as_str(), "SANDBOX_006");
        assert!(
            before.why.contains("/home/agent/tree/cache"),
            "{}",
            before.why
        );
    }

    /// Was nach dem tmpfs von `/dev/shm` darunter liegt, verdeckt im Gast das
    /// `devtmpfs` des Kerns: abgelehnt statt still fehlend.
    #[test]
    fn an_entry_under_dev_shm_is_refused() {
        let refused = guest_tmpfs(&[tmpfs("/dev/shm"), bind("/dev/shm/x")]).unwrap_err();
        assert_eq!(refused.code.as_str(), "SANDBOX_006");
        assert!(refused.why.contains("/dev/shm/x"), "{}", refused.why);
        assert!(guest_tmpfs(&[bind("/dev/shm/x"), tmpfs("/dev/shm")]).is_ok());
    }

    #[test]
    fn the_socket_walk_covers_every_tmpfs_once() {
        let found = guest_tmpfs(&[
            tmpfs("/tmp"),
            tmpfs("/tmp/x"),
            tmpfs("/dev/foo"),
            tmpfs("/home/agent"),
        ])
        .unwrap();
        assert_eq!(
            socket_walk(Path::new("/work"), &found),
            ["/work", "/dev", "/run", "/tmp", "/home/agent"].map(PathBuf::from)
        );
    }
}
