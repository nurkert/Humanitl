//! Was eine Sitzung unter Seatbelt täte, ohne etwas anzulegen (HUM-376).
//!
//! Die Vorschau zeigt dieselbe Kommandozeile und dasselbe Profil wie
//! [`SeatbeltBackend::plan`](crate::SandboxBackend::plan), baut sie aber nur
//! aus Lesezugriffen: kein Sitzungsverzeichnis, kein gebundener Port, kein
//! Deskriptor. Was erst der Start festlegt, steht als Platzhalter da
//! ([`PREVIEW_SESSION_PORT`], [`PREVIEW_AGENT_PORT`], [`PREVIEW_STATUS_FD`],
//! [`PREVIEW_REPORT_FD`]);
//! `the_preview_is_the_plan_but_for_the_ports` hält beide Wege zusammen.
//!
//! Das Profil ist der Beleg. Die Tabelle ([`SeatbeltPreview::rows`]) entsteht
//! aus derselben [`SeatbeltPolicy`] und nennt jeden Pfad bei dem Namen, den er
//! wirklich hat: Unter Seatbelt gibt es keinen eigenen Mount-Namensraum, der
//! Agent sieht die echten Pfade. Die Namen aus der Profildatei (`/work`,
//! `/tmp`) stehen daneben.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::{
    PRIVATE_TREES, SeatbeltBackend, SeatbeltPolicy, SessionLayout, git_applies, prepend_to_path,
    session_env,
};
use crate::launcher::{
    DisposableLayer, Door, PlanPreview, PlanRequest, PolicyMode, PolicyOrigin, PolicyRow,
    ResourceLimits,
};
use crate::profile::{SandboxProfile, SessionContext};

/// Steht in der Vorschau für den Loopback-Port der Sitzung.
pub const PREVIEW_SESSION_PORT: &str = "SESSION_PORT";

/// Steht in der Vorschau für den zweiten Port der Sitzung (HUM-365).
pub const PREVIEW_AGENT_PORT: &str = "AGENT_PORT";

/// Steht in der Vorschau für den Deskriptor der Status-Pipe.
pub const PREVIEW_STATUS_FD: &str = "STATUS_FD";
/// Steht in der Umgebung für den Deskriptor des Shim-Berichts.
pub const PREVIEW_REPORT_FD: &str = "REPORT_FD";

/// Die Portnummern, mit denen das Profil gerendert wird, bevor die
/// Platzhalter sie ersetzen. Beide sind als echter Sitzungsport unmöglich:
/// [`super::session_port`] nimmt nur freie Ports vom System.
const RENDER_PORTS: (u16, u16) = (0, 1);

/// Kommandozeile, Profil und Tabelle einer Sitzung, die nicht gestartet wird.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeatbeltPreview {
    /// Die Kommandozeile, `argv[0]` ist `sandbox-exec`.
    pub argv: Vec<OsString>,
    /// Die Beschreibung, aus der Text und Tabelle stammen.
    pub policy: SeatbeltPolicy,
    /// Der Text von `sandbox.sb`.
    pub policy_text: String,
    /// Die Tabelle aus [`SeatbeltPreview::policy`]: Ziel und Quelle jeder
    /// Zeile sind derselbe echte Pfad (HUM-429).
    pub rows: Vec<PolicyRow>,
    /// Die Umgebung des Agenten, sortiert, auf echte Pfade abgebildet.
    pub env: Vec<(String, String)>,
    /// Die Schlüssel aus [`SeatbeltPreview::env`], die allein das Backend
    /// setzt und die keine der Quellen des Profils kennt.
    pub backend_env: Vec<String>,
}

impl SeatbeltBackend {
    /// Was eine Sitzung täte, ohne Seiteneffekt.
    ///
    /// Anders als der Plan scheitert die Vorschau nicht: Sie läuft auch, wenn
    /// Proxy-Socket, Shim oder Projektordner fehlen, denn wer nachsehen will,
    /// braucht nichts davon. Fehlende Pfade behalten ihren Namen, ihr schon
    /// vorhandener Teil wird aufgelöst. Eine Wegwerf-Schicht, die kein Klon
    /// ist, zeigt sie nicht; der Plan lehnt sie ab.
    ///
    /// Die ausführliche Form von [`crate::SandboxBackend::preview`], mit der
    /// Beschreibung, aus der Profiltext und Tabelle stammen.
    #[must_use]
    pub fn seatbelt_preview(
        &self,
        request: &PlanRequest<'_>,
        controlling_terminal: bool,
    ) -> SeatbeltPreview {
        let PlanRequest {
            profile,
            session,
            options,
        } = *request;
        let project = resolved(&session.work_src);
        let work = match &options.disposable {
            Some(DisposableLayer::Clone(clone)) => resolved(clone),
            Some(DisposableLayer::Overlay(_)) | None => project.clone(),
        };
        let layout = SessionLayout {
            root: resolved(&self.sessions_dir()).join(session.session.to_string()),
        };
        let door = resolved(&session.proxy_socket_src);
        let shim = resolved(&session.shim_src);
        let map = Self::session_map(options, &layout, &work, &door, &shim);

        let mut files = vec![shim.clone()];
        files.extend(
            session
                .agent_program
                .iter()
                .map(|agent| resolved(&agent.host)),
        );
        let mut policy = Self::policy(
            request,
            &project,
            &work,
            &layout,
            files,
            &door,
            RENDER_PORTS,
        );
        // Wie im Plan: die Dateien des Adapters unter `home` bleiben, wo sie
        // liegen, und das `git` der Sitzung steht vorn auf dem Suchpfad.
        let home = layout.home();
        let protected: Vec<PathBuf> = session
            .files
            .iter()
            .filter_map(|file| map.map_path(&file.dst))
            .filter(|target| target.starts_with(&home))
            .collect();
        policy.protect(&protected, &home);
        let git_bin = git_applies(&work, &policy.masked).then(|| layout.git_bin());
        let policy_text = placeholders_in_rules(&policy.render());

        let mut argv: Vec<OsString> = vec![
            self.program.clone().into_os_string(),
            "-f".into(),
            layout.profile().into_os_string(),
            shim.into_os_string(),
            "--proxy-port".into(),
            PREVIEW_SESSION_PORT.into(),
            "--status-fd".into(),
            PREVIEW_STATUS_FD.into(),
            "--chdir".into(),
            work.clone().into_os_string(),
        ];
        if controlling_terminal {
            argv.push("--controlling-terminal".into());
        }
        argv.push("--".into());
        argv.extend(
            session
                .command
                .iter()
                .map(|arg| OsString::from(map.map_text(&arg.to_string_lossy()))),
        );

        let (mut env, backend_env) = preview_env(profile, session, &map, &layout, &door);
        if let Some(bin) = git_bin {
            prepend_to_path(&mut env, &bin);
        }
        let rows = rows_of(&policy, &map.entries);
        SeatbeltPreview {
            argv,
            policy,
            policy_text,
            rows,
            env,
            backend_env,
        }
    }
}

impl SeatbeltPreview {
    /// Die Vorschau in der Form des Ports (HUM-429). Die Umgebung steht
    /// nicht in der Zeile, es gibt dort also keinen Wert zurückzuhalten; die
    /// Tür ist der Port der Sitzung, den erst der Start vergibt.
    pub(crate) fn into_plan_preview(self, proxy_socket: &Path) -> PlanPreview {
        PlanPreview {
            argv: self.argv,
            policy: self.rows,
            env: self.env,
            backend_env: self.backend_env,
            argv_env: Vec::new(),
            policy_text: Some(self.policy_text),
            door: Door::LoopbackPort {
                host: resolved(proxy_socket),
                port: RENDER_PORTS.0,
            },
            limits: ResourceLimits::default(),
        }
    }
}

/// Die Umgebung der Vorschau und die Schlüssel, die allein das Backend setzt.
fn preview_env(
    profile: &SandboxProfile,
    session: &SessionContext,
    map: &super::PathMap,
    layout: &SessionLayout,
    door: &Path,
) -> (Vec<(String, String)>, Vec<String>) {
    let own_door = format!("127.0.0.1:{}", RENDER_PORTS.0);
    let shown_door = format!("127.0.0.1:{PREVIEW_SESSION_PORT}");
    let mut env: Vec<(String, String)> =
        session_env(profile, session, map, layout, door, RENDER_PORTS.0, &0)
            .into_iter()
            .map(|(key, value)| {
                // Die Nummer vergibt erst der Start; die 0 dieses Aufrufs
                // wäre ein falscher Wert auf dem Bildschirm.
                if key == crate::bridge_env::ENV_REPORT_FD {
                    (key, PREVIEW_REPORT_FD.to_owned())
                } else {
                    (key, value.replace(&own_door, &shown_door))
                }
            })
            .collect();
    env.push((
        crate::agent::ENV_AGENT_PORT.to_owned(),
        PREVIEW_AGENT_PORT.to_owned(),
    ));
    env.sort();
    let known: Vec<String> = profile
        .effective_env(session, Some(0))
        .into_iter()
        .map(|(key, _)| key)
        .collect();
    let backend_env = env
        .iter()
        .map(|(key, _)| key)
        .filter(|key| !known.contains(key))
        .cloned()
        .collect();
    (env, backend_env)
}

/// Setzt die Platzhalter in die Netzwerkregeln des gerenderten Profils.
///
/// Nur in die Regeln selbst: Ein Pfad, in dem zufällig `localhost:0` steht,
/// bleibt unberührt.
fn placeholders_in_rules(text: &str) -> String {
    let mut out = text.to_owned();
    for (port, name) in [
        (RENDER_PORTS.0, PREVIEW_SESSION_PORT),
        (RENDER_PORTS.1, PREVIEW_AGENT_PORT),
    ] {
        for kind in ["remote", "local"] {
            out = out.replace(
                &format!("({kind} ip \"localhost:{port}\")"),
                &format!("({kind} ip \"localhost:{name}\")"),
            );
        }
    }
    out
}

/// Die Tabelle aus dem, was auch das Profil schreibt.
///
/// Reihenfolge wie im Profil: erst das Lesen des Systems, dann die Sperren,
/// dann das, was die Sitzung wieder öffnet. Die Sperren einzelner Ordner
/// zwischen Projekt und Maske (`locked`) fehlen hier: Sie gelten nur für den
/// Eintrag selbst, und „nur lesbar" behauptete mehr, als sie schützen. Sie
/// stehen im Profiltext.
fn rows_of(policy: &SeatbeltPolicy, map: &[(String, PathBuf)]) -> Vec<PolicyRow> {
    // Ohne eigenen Mount-Namensraum ist der Pfad in der Sandbox der auf dem
    // Rechner: Ziel und Quelle sind derselbe.
    let row = |path: &Path, mode, origin| {
        PolicyRow::new(mode, Some(path.to_path_buf()), path.to_path_buf(), origin)
    };
    let work_access = if policy.work_writable {
        PolicyMode::ReadWrite
    } else {
        PolicyMode::ReadOnly
    };
    let mut rows = vec![row(
        Path::new("/"),
        PolicyMode::ReadOnly,
        PolicyOrigin::Profile,
    )];
    rows.extend(
        PRIVATE_TREES
            .iter()
            .map(|tree| row(Path::new(tree), PolicyMode::Denied, PolicyOrigin::Profile)),
    );
    rows.push(row(&policy.work, work_access, PolicyOrigin::Session));
    rows.push(row(
        &policy.session,
        PolicyMode::ReadOnly,
        PolicyOrigin::Session,
    ));
    rows.extend(
        policy
            .session_writable
            .iter()
            .map(|dir| row(dir, PolicyMode::ReadWrite, PolicyOrigin::Session)),
    );
    rows.extend(
        policy
            .read_only
            .iter()
            .map(|dir| row(dir, PolicyMode::ReadOnly, PolicyOrigin::User)),
    );
    rows.extend(
        policy
            .writable
            .iter()
            .map(|dir| row(dir, PolicyMode::ReadWrite, PolicyOrigin::User)),
    );
    rows.extend(
        policy
            .files
            .iter()
            .chain(std::iter::once(&policy.door))
            .map(|file| row(file, PolicyMode::ReadOnly, PolicyOrigin::Session)),
    );
    rows.extend(
        policy
            .masked
            .iter()
            .map(|path| row(path, PolicyMode::Denied, PolicyOrigin::Profile)),
    );
    let mut merged: Vec<PolicyRow> = Vec::with_capacity(rows.len());
    for entry in rows {
        match merged.iter_mut().find(|known| known.target == entry.target) {
            Some(known) => merge_row(known, entry),
            None => merged.push(entry),
        }
    }
    let mut rows = merged;

    for (name, path) in map {
        if let Some(entry) = rows.iter_mut().find(|entry| entry.target == *path) {
            entry.names.push(name.clone());
        } else {
            let mut entry = row(path, PolicyMode::ReadOnly, PolicyOrigin::Session);
            entry.names.push(name.clone());
            rows.push(entry);
        }
    }
    for entry in &mut rows {
        entry.names.sort();
    }
    rows
}

/// Führt zwei Zeilen desselben Pfads zur Wirkung des Profils zusammen.
///
/// Die Zeilen kommen in der Reihenfolge des Profils. Eine Maske sperrt zuletzt
/// und gewinnt, ein Schreibrecht übersteht ein späteres Leserecht, denn SBPL
/// nimmt mit `allow file-read*` kein `allow file-write*` zurück, und ein
/// Leserecht öffnet wieder, was ein privater Baum davor gesperrt hatte.
fn merge_row(known: &mut PolicyRow, entry: PolicyRow) {
    // Die Tabelle unter Seatbelt kennt nur diese drei; was anderes käme,
    // gewinnt wie eine Sperre, damit sie nie mehr behauptet als das Profil.
    let wins = match entry.mode {
        PolicyMode::ReadOnly => known.mode == PolicyMode::Denied,
        _ => true,
    };
    if wins {
        known.mode = entry.mode;
        known.origin = entry.origin;
    }
    known.names.extend(entry.names);
}

/// Der Pfad mit aufgelöstem vorhandenem Teil; was fehlt, bleibt dahinter
/// stehen. Seatbelt vergleicht aufgelöste Pfade, und unter macOS liegen
/// `/tmp` und `/var` hinter Verweisen.
fn resolved(path: &Path) -> PathBuf {
    for base in path.ancestors() {
        if let Ok(real) = base.canonicalize() {
            let rest = path.strip_prefix(base).unwrap_or(Path::new(""));
            return if rest.as_os_str().is_empty() {
                real
            } else {
                real.join(rest)
            };
        }
    }
    path.to_path_buf()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::os::unix::fs::PermissionsExt as _;
    use std::os::unix::net::UnixListener;

    use humanitl_config::{Env, Paths, WorkMode};
    use humanitl_core::ids::SessionId;

    use super::*;
    use crate::{LaunchOptions, SandboxBackend as _, SandboxProfile};

    /// Alles, was ein echter Plan verlangt, in einem eigenen Verzeichnis.
    struct Fixture {
        _dir: tempfile::TempDir,
        _listener: UnixListener,
        backend: SeatbeltBackend,
        session: SessionContext,
        profile: SandboxProfile,
        options: LaunchOptions,
    }

    impl Fixture {
        /// Profil und Sitzung des Fixtures mit den Vorgaben eines Starts.
        fn request(&self) -> PlanRequest<'_> {
            PlanRequest::new(&self.profile, &self.session, &self.options)
        }
    }

    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = dir.path().canonicalize().expect("resolved");
        let paths = Paths::new(
            Env::from_process().with("XDG_RUNTIME_DIR", state.join("run").to_string_lossy()),
        );
        let work = state.join("work");
        std::fs::create_dir_all(work.join(".git/hooks")).expect("hooks");
        let socket = paths.proxy_socket();
        let parent = socket.parent().expect("a parent");
        std::fs::create_dir_all(parent).expect("socket dir");
        std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700)).expect("mode");
        let listener = UnixListener::bind(&socket).expect("socket");
        std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600)).expect("mode");
        let ca = state.join("ca.crt");
        std::fs::write(&ca, "ca").expect("ca");
        let bundle = state.join("bundle.crt");
        std::fs::write(&bundle, "bundle").expect("bundle");
        let shim = state.join("shim");
        std::fs::write(&shim, "#!/bin/sh\n").expect("shim");
        std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755)).expect("mode");
        let session = SessionContext {
            session: SessionId::nil(),
            work_src: work,
            work_mode: WorkMode::Rw,
            proxy_socket_src: socket,
            ca_cert_src: ca,
            ca_bundle_src: bundle,
            shim_src: shim,
            session_env: Vec::new(),
            command: vec!["/work/run.sh".into(), "--flag".into()],
            files: Vec::new(),
            agent_program: None,
        };
        let text = include_str!("../../../../../profiles/sandbox/default.toml");
        let profile = SandboxProfile::parse(text, Path::new("default.toml")).expect("the profile");
        Fixture {
            _dir: dir,
            _listener: listener,
            backend: SeatbeltBackend::unchecked("/usr/bin/sandbox-exec", paths),
            session,
            profile,
            options: LaunchOptions::default(),
        }
    }

    /// Die Vorschau sagt, was der Plan tut. Zieht jemand am einen Weg, ohne
    /// den anderen mitzunehmen, zeigt die Oberfläche wieder etwas, das nie
    /// läuft.
    #[test]
    fn the_preview_is_the_plan_but_for_the_ports() {
        let fx = fixture();
        let preview = fx.backend.seatbelt_preview(&fx.request(), false);
        let sessions = fx.backend.sessions_dir();
        assert!(!sessions.exists(), "the preview left {sessions:?} behind");

        let plan = fx.backend.plan(&fx.request()).expect("the plan");
        let port = plan.env_value("HTTPS_PROXY");
        let port = port.rsplit(':').next().expect("a port").to_owned();
        let agent_port = plan.env_value(crate::agent::ENV_AGENT_PORT);

        let path = sessions
            .join(fx.session.session.to_string())
            .join("sandbox.sb");
        let real = std::fs::read_to_string(path).expect("the profile of the plan");
        let shown = preview
            .policy_text
            .replace(PREVIEW_SESSION_PORT, &port)
            .replace(PREVIEW_AGENT_PORT, &agent_port);
        assert_eq!(shown, real);

        let argv: Vec<String> = preview
            .argv
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        let planned: Vec<String> = plan
            .argv
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(argv.len(), planned.len());
        for (index, (shown, real)) in argv.iter().zip(&planned).enumerate() {
            match shown.as_str() {
                PREVIEW_SESSION_PORT => assert_eq!(real, &port),
                PREVIEW_STATUS_FD => assert!(real.parse::<i32>().is_ok(), "{real}"),
                _ => assert_eq!(shown, real, "argument {index}"),
            }
        }

        let keys = |env: &[(String, String)]| {
            let mut keys: Vec<String> = env.iter().map(|(key, _)| key.clone()).collect();
            keys.sort();
            keys
        };
        assert_eq!(keys(&preview.env), keys(&plan.env));
        for (key, shown) in &preview.env {
            if key == crate::bridge_env::ENV_REPORT_FD {
                assert_eq!(shown, PREVIEW_REPORT_FD);
                let real = plan.env_value(key);
                assert!(real.parse::<i32>().is_ok(), "{real}");
                continue;
            }
            let value = shown
                .replace(PREVIEW_SESSION_PORT, &port)
                .replace(PREVIEW_AGENT_PORT, &agent_port);
            assert_eq!(value, plan.env_value(key), "{key}");
        }
    }

    /// Steht ein Ordner in `extra_ro` und `extra_rw`, darf der Agent schreiben;
    /// die Tabelle darf nicht weniger behaupten als das Profil erlaubt.
    #[test]
    fn a_path_both_readable_and_writable_shows_the_write() {
        let mut fx = fixture();
        let both = fx.session.work_src.parent().expect("a parent").join("both");
        std::fs::create_dir_all(&both).expect("the folder");
        fx.profile.mounts.extra_ro.push(both.clone());
        fx.profile.mounts.extra_rw.push(both.clone());

        let preview = fx.backend.seatbelt_preview(&fx.request(), false);
        let rows: Vec<&PolicyRow> = preview.rows.iter().filter(|r| r.target == both).collect();
        assert_eq!(rows.len(), 1, "one line per path");
        assert_eq!(rows[0].mode, PolicyMode::ReadWrite);
        assert_eq!(rows[0].origin, PolicyOrigin::User);
    }

    /// Die Maske sperrt im Profil zuletzt, und der Name der Profildatei bleibt.
    #[test]
    fn merging_keeps_the_mask_and_the_profile_names() {
        let path = PathBuf::from("/x");
        let line = |mode, names: &[&str]| PolicyRow {
            names: names.iter().map(|name| (*name).to_owned()).collect(),
            ..PolicyRow::new(
                mode,
                Some(path.clone()),
                path.clone(),
                PolicyOrigin::Session,
            )
        };
        let mut known = line(PolicyMode::ReadWrite, &["/work"]);
        merge_row(&mut known, line(PolicyMode::ReadOnly, &["/a"]));
        assert_eq!(known.mode, PolicyMode::ReadWrite);
        merge_row(&mut known, line(PolicyMode::Denied, &[]));
        assert_eq!(known.mode, PolicyMode::Denied);
        assert_eq!(known.names, vec!["/work".to_owned(), "/a".to_owned()]);

        let mut private = line(PolicyMode::Denied, &[]);
        merge_row(&mut private, line(PolicyMode::ReadOnly, &[]));
        assert_eq!(private.mode, PolicyMode::ReadOnly);
    }

    #[test]
    fn the_table_names_the_real_paths_and_where_the_profile_has_them() {
        let mut fx = fixture();
        fx.session.work_mode = WorkMode::Ro;
        std::fs::write(fx.session.work_src.join(".envrc"), "x").expect("a mask");
        let preview = fx.backend.seatbelt_preview(&fx.request(), true);
        let row = |path: &Path| {
            preview
                .rows
                .iter()
                .find(|row| row.target == path)
                .unwrap_or_else(|| panic!("no row for {path:?}"))
        };

        let work = row(&fx.session.work_src);
        assert_eq!(work.mode, PolicyMode::ReadOnly);
        assert_eq!(work.names, vec![WORK_NAME.to_owned()]);
        assert_eq!(work.source.as_deref(), Some(work.target.as_path()));
        assert_eq!(row(Path::new("/")).mode, PolicyMode::ReadOnly);
        assert_eq!(row(Path::new("/Users")).mode, PolicyMode::Denied);
        assert_eq!(
            row(&fx.session.work_src.join(".envrc")).mode,
            PolicyMode::Denied
        );
        let tmp = preview
            .rows
            .iter()
            .find(|row| row.names.iter().any(|name| name == "/tmp"))
            .expect("the session has a /tmp");
        assert_eq!(tmp.mode, PolicyMode::ReadWrite);
        assert_eq!(tmp.names, vec!["/tmp".to_owned(), "/var/tmp".to_owned()]);
        assert!(tmp.target.starts_with(&preview.policy.session));

        assert!(
            preview
                .argv
                .iter()
                .any(|arg| arg == "--controlling-terminal")
        );
        assert!(preview.policy_text.contains("localhost:SESSION_PORT"));
        assert!(preview.policy_text.contains("localhost:AGENT_PORT"));
        assert!(!preview.policy_text.contains("localhost:0"));
    }

    /// Wer nachsehen will, braucht weder Proxy noch Shim noch Projekt.
    #[test]
    fn a_preview_needs_nothing_to_exist() {
        let fx = fixture();
        let mut session = fx.session.clone();
        session.work_src = PathBuf::from("/nowhere/proj");
        session.proxy_socket_src = PathBuf::from("/nowhere/proxy.sock");
        session.shim_src = PathBuf::from("/nowhere/shim");
        let preview = fx
            .backend
            .seatbelt_preview(&PlanRequest::new(&fx.profile, &session, &fx.options), false);
        assert!(
            preview
                .rows
                .iter()
                .any(|row| row.target.as_path() == Path::new("/nowhere/proj"))
        );
        assert!(preview.env.iter().any(|(key, _)| key == "TMPDIR"));
        assert!(preview.backend_env.iter().any(|key| key == "TMPDIR"));
    }

    /// Jede Zeile der Tabelle steht im Profil, und jeder Baum, den das Profil
    /// öffnet oder sperrt, hat seine Zeile (HUM-429). Unter Seatbelt ist das
    /// Profil, was die Zeile unter bwrap ist: Die Tabelle kommt aus derselben
    /// Beschreibung ([`SeatbeltPolicy`]) und nicht aus dem Text, und dieser
    /// Vergleich in beide Richtungen hält beide zusammen. Die Ordner über
    /// einem Pfad (`file-read-metadata`) und die Sperren einzelner Ordner
    /// (`locked`) haben mit Absicht keine Zeile; siehe [`rows_of`].
    #[test]
    fn every_row_is_in_the_profile_and_every_tree_of_the_profile_a_row() {
        let mut fx = fixture();
        std::fs::write(fx.session.work_src.join(".envrc"), "x").expect("a mask");
        let extra = fx.session.work_src.parent().expect("a parent").join("data");
        std::fs::create_dir_all(&extra).expect("the folder");
        fx.profile.mounts.extra_rw.push(extra);
        fx.options.stdio = crate::StdioMode::Pty { cols: 80, rows: 24 };
        let preview = crate::SandboxBackend::preview(&fx.backend, &fx.request());
        let text = preview.policy_text.expect("Seatbelt has a profile");
        assert!(text.contains("(allow file-read*)\n"), "{text}");
        let trees: Vec<PathBuf> = text
            .split("(subpath \"")
            .skip(1)
            .map(|part| PathBuf::from(part.split('"').next().expect("a closing quote")))
            .collect();
        // Eine Zeile steht selbst im Profil, oder sie liegt in einem Baum, den
        // das Profil nennt: die Dateien der Sitzung im Sitzungsverzeichnis.
        for row in &preview.policy {
            if row.target == Path::new("/") {
                continue;
            }
            let quoted = format!("\"{}\"", row.target.display());
            assert!(
                text.contains(&quoted) || trees.iter().any(|tree| row.target.starts_with(tree)),
                "{row:?} is not in the profile"
            );
            assert_eq!(row.source.as_ref(), Some(&row.target), "{row:?}");
        }
        for tree in &trees {
            assert!(
                preview.policy.iter().any(|row| row.target == *tree),
                "the profile names {tree:?}, the table does not"
            );
        }
        assert!(trees.len() > PRIVATE_TREES.len(), "{text}");
        assert!(
            preview
                .argv
                .iter()
                .any(|arg| arg == "--controlling-terminal"),
            "a start at the own terminal shows its line"
        );
        assert_eq!(
            preview.door.describe(),
            format!("loopback_port 127.0.0.1:{PREVIEW_SESSION_PORT}")
        );
        assert!(preview.argv_env.is_empty() && preview.limits.is_empty());
    }

    const WORK_NAME: &str = "/work";

    trait EnvValue {
        fn env_value(&self, key: &str) -> String;
    }

    impl EnvValue for crate::LaunchPlan {
        fn env_value(&self, key: &str) -> String {
            self.env.iter().find(|(name, _)| name == key).map_or_else(
                || panic!("{key} is not in the plan"),
                |(_, value)| value.clone(),
            )
        }
    }
}
