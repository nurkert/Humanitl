//! `humanitl sandbox plan`: was ein Start in diesem Projekt bekäme, ohne zu
//! starten (HUM-400).
//!
//! Ein dünner Client (ADR-018): Der Daemon löst den Plan auf
//! (`SandboxRequest.Plan`), mit dem Projekt-Profil des Projektordners und
//! `Plan.llm_server`; hier wird nur gezeigt. Vor allem zeigt der Befehl,
//! welcher gespeicherte Modellserver gilt und woher die Wahl kommt: Ein
//! Projekt kann einen anderen Server wählen als den aktiven, und das soll
//! vor dem Start zu sehen sein.

use humanitl_core::diagnostics::codes;
use humanitl_core::{Diagnostic, FixAction, Severity};
use humanitl_ipc::v1;
use serde_json::json;

use crate::cmd::{Context, EXIT_OK, Failure, ProfileMeaning, from_proto, status_diagnostic};

/// Die erste Minor des Vertrags, die die Wahl des Servers je Lauf, Projekt
/// und Profil kennt (HUM-400).
pub const SERVER_CHOICE_MINOR: u32 = 28;

/// Die erste Minor des Vertrags, die `Plan.cli_overrides` kennt (HUM-559):
/// Erst ab ihr sieht die Vorschau die Werte der Sitzung, vor allem die
/// Untergrenze der Kommandozeile.
pub const PLAN_OVERRIDES_MINOR: u32 = 55;

/// Die erste Minor des Vertrags, die `Plan.backend` kennt (HUM-440).
pub const PLAN_BACKEND_MINOR: u32 = 38;

/// Ob eine Momentaufnahme die Werte der Sitzung dieses Aufrufs trägt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SessionValues {
    /// `humanitl sandbox plan`: die Vorschau eines Starts, mit denselben
    /// Werten wie der Start (HUM-559).
    Preview,
    /// `humanitl sessions`: nur die Moduszeile, ohne Werte der Sitzung.
    Without,
}

/// `sandbox plan`.
///
/// # Errors
///
/// `DAEMON_001` ohne Daemon, die Befunde des Plans, und `CONFIG_021`, wenn
/// der gewählte Server nicht gespeichert ist: Der Start lehnte dann ab.
pub async fn run(ctx: &Context) -> Result<u8, Failure> {
    let (status, minor) = snapshot_with(ctx, SessionValues::Preview).await?;
    if ctx.render.is_json() {
        ctx.render.value(&plan_json(&status));
    } else {
        for line in plan_lines(&status, minor) {
            ctx.render.line(&line);
        }
    }
    // Ein Name ohne gespeicherten Server: Der Plan zeigt ihn, der Start
    // lehnte ab, und das sagt der Exit-Code.
    if let Some(refusal) = status
        .llm_server
        .as_ref()
        .and_then(|choice| choice.refusal.as_ref())
        .and_then(from_proto)
    {
        return Err(Failure::new(refusal));
    }
    Ok(EXIT_OK)
}

/// Die Momentaufnahme des Plans und die Vertrags-Minor des Daemons.
///
/// Für `humanitl sessions` (HUM-512), das aus der Antwort die Moduszeile
/// zeigt. Ohne die Werte der Sitzung dieses Aufrufs: `sessions` ist keine
/// Vorschau eines Starts, und eine Untergrenze aus der Umgebung ließe es
/// sonst an einem älteren Daemon mit `CLI_004` enden (HUM-559).
///
/// # Errors
///
/// Wie [`run`], ohne den Befund eines nicht gespeicherten Servers.
pub async fn snapshot(ctx: &Context) -> Result<(v1::sandbox_event::Status, u32), Failure> {
    snapshot_with(ctx, SessionValues::Without).await
}

/// [`snapshot`], mit oder ohne die Werte der Sitzung dieses Aufrufs.
async fn snapshot_with(
    ctx: &Context,
    values: SessionValues,
) -> Result<(v1::sandbox_event::Status, u32), Failure> {
    let resolved = ctx.config()?;
    let (work_dir, work_dir_source) = ctx.work_dir(&resolved);
    let mut client = ctx.connect().await?;
    let info = client
        .get_info(())
        .await
        .map_err(|status| Failure::new(status_diagnostic(&status, "GetInfo")))?
        .into_inner();
    crate::cmd::daemon::check_proto(&info)?;
    let llm_server = llm_server_pair(ctx).unwrap_or_default();
    // Ein älterer Daemon überginge Server und Profil der Sitzung still und
    // zeigte einen anderen Plan als den verlangten.
    let wants = !llm_server.is_empty()
        || (ctx.profile_means == ProfileMeaning::Session && ctx.profile_flag().is_some());
    if wants && info.proto_minor < SERVER_CHOICE_MINOR {
        return Err(Failure::new(old_daemon(info.proto_minor)));
    }
    let overrides = session_values(ctx, &resolved, values);
    let plan = plan_request(
        ctx,
        &work_dir,
        &work_dir_source,
        llm_server,
        overrides,
        info.proto_minor,
    )?;
    let mut stream = client
        .sandbox(v1::SandboxRequest {
            op: Some(v1::sandbox_request::Op::Plan(plan)),
        })
        .await
        .map_err(|status| Failure::new(status_diagnostic(&status, "Sandbox")))?
        .into_inner();
    let minor = info.proto_minor;
    collect(ctx, &mut stream)
        .await
        .map(|status| (status, minor))
}

/// Die Werte der Sitzung, die eine Momentaufnahme mitschickt: für die
/// Vorschau die des Starts, für `humanitl sessions` keine (HUM-559).
fn session_values(
    ctx: &Context,
    resolved: &humanitl_config::Resolved,
    values: SessionValues,
) -> Vec<v1::sandbox_request::CliOverride> {
    match values {
        SessionValues::Preview => crate::cmd::run::client_overrides(ctx, resolved),
        SessionValues::Without => Vec::new(),
    }
}

/// Die Anfrage des Plans, mit den Flags wie unter `humanitl run` (HUM-400):
/// `--profile` ist das Profil der Sitzung, `--sandbox-profile` das Profil
/// des Sandbox-Backends, `--llm NAME` der Server des Laufs.
///
/// Dazu `overrides`, für die Vorschau dieselben Werte der Sitzung wie im
/// Start (HUM-559, [`crate::cmd::run::client_overrides`]): Flags, Umgebung
/// und die Datei aus `--config`, vor allem die Untergrenze der Isolation.
/// `minor` ist die Vertrags-Minor des Daemons; was er davon nicht sieht,
/// prüft [`preview_values`].
///
/// # Errors
///
/// `CLI_004` aus [`preview_values`].
fn plan_request(
    ctx: &Context,
    work_dir: &std::path::Path,
    work_dir_source: &humanitl_sandbox::WorkDirSource,
    llm_server: String,
    overrides: Vec<v1::sandbox_request::CliOverride>,
    minor: u32,
) -> Result<v1::sandbox_request::Plan, Failure> {
    let (backend, cli_overrides) = preview_values(overrides, minor)?;
    let (profile, session_profile) = match ctx.profile_means {
        ProfileMeaning::Sandbox => (
            ctx.profile_flag().unwrap_or_default().to_owned(),
            String::new(),
        ),
        ProfileMeaning::Session => (
            ctx.cli_pairs()
                .into_iter()
                .find(|(path, _)| path == "sandbox.profile")
                .map(|(_, name)| name)
                .unwrap_or_default(),
            ctx.profile_flag().unwrap_or_default().to_owned(),
        ),
    };
    Ok(v1::sandbox_request::Plan {
        profile,
        work_dir: work_dir.display().to_string(),
        work_mode: String::new(),
        work_dir_source: Some(humanitl_ipc::work_dir::source_to_proto(work_dir_source)),
        llm_server,
        session_profile,
        backend,
        cli_overrides,
    })
}

/// `Plan.backend` und `Plan.cli_overrides` zu diesen Werten der Sitzung, für
/// einen Daemon der Minor `minor` (HUM-559).
///
/// Das Backend dieses Aufrufs steht in beiden Feldern: `Plan.backend` kennt
/// ein Daemon seit 1.38, `Plan.cli_overrides` erst seit 1.55, und ab dort
/// gilt bei beiden dasselbe. Leer heißt: `sandbox.backend` der
/// Konfiguration.
///
/// # Errors
///
/// `CLI_004`, wenn der Daemon einen der Werte nicht sähe: vor 1.55 jeden
/// außer dem Backend, vor 1.38 auch das. Er überginge ihn still und zeigte
/// einen Plan, der vom Start abweicht, etwa ohne die Untergrenze, an der der
/// Start dann scheitert. Der Befund nennt die höchste Minor, die einer der
/// abgelehnten Werte braucht.
fn preview_values(
    overrides: Vec<v1::sandbox_request::CliOverride>,
    minor: u32,
) -> Result<(String, Vec<v1::sandbox_request::CliOverride>), Failure> {
    let backend = overrides
        .iter()
        .find(|pair| pair.path == "sandbox.backend")
        .map(|pair| pair.value.clone())
        .unwrap_or_default();
    // Je Wert die Minor, ab der ein Daemon ihn im Plan sieht.
    let needs = |pair: &v1::sandbox_request::CliOverride| {
        if pair.path == "sandbox.backend" {
            PLAN_BACKEND_MINOR
        } else {
            PLAN_OVERRIDES_MINOR
        }
    };
    let unseen: Vec<&v1::sandbox_request::CliOverride> = overrides
        .iter()
        .filter(|pair| minor < needs(pair))
        .collect();
    if let Some(needed) = unseen.iter().map(|pair| needs(pair)).max() {
        let unseen: Vec<String> = unseen
            .iter()
            .map(|pair| format!("{} = {:?}", pair.path, pair.value))
            .collect();
        return Err(Failure::new(
            Diagnostic::builder(codes::CLI_004, Severity::Error)
                .why(format!(
                    "the daemon speaks contract 1.{minor}, and a preview with {} needs \
                         1.{needed}; it would show a plan without them that the start does not \
                         get. Restart the daemon, or preview without these values",
                    unseen.join(", ")
                ))
                .fix(FixAction::CopyCommand(
                    "systemctl --user restart humanitld.service".to_owned(),
                ))
                .build(),
        ));
    }
    Ok((backend, overrides))
}

/// Liest die Antwort des Plans; Warnungen gehen gleich hinaus.
async fn collect(
    ctx: &Context,
    stream: &mut tonic::Streaming<v1::SandboxEvent>,
) -> Result<v1::sandbox_event::Status, Failure> {
    let mut status = None;
    while let Some(event) = stream
        .message()
        .await
        .map_err(|status| Failure::new(status_diagnostic(&status, "Sandbox")))?
    {
        match event.event {
            Some(v1::sandbox_event::Event::Diagnostic(wire)) => {
                if let Some(diagnostic) = from_proto(&wire) {
                    if diagnostic.severity >= Severity::Error {
                        return Err(Failure::new(diagnostic));
                    }
                    ctx.render.diagnostic(&diagnostic);
                }
            }
            Some(v1::sandbox_event::Event::Status(snapshot)) => status = Some(snapshot),
            _ => {}
        }
    }
    status.ok_or_else(|| {
        Failure::new(
            Diagnostic::builder(codes::CLI_004, Severity::Error)
                .why("the daemon answered the plan without a snapshot".to_owned())
                .fix(FixAction::CopyCommand("humanitl daemon status".to_owned()))
                .build(),
        )
    })
}

/// Die erste Minor des Vertrags, die `Status.ask_mode` kennt (HUM-512).
pub const ASK_MODE_MINOR: u32 = 42;

/// Wer eine Anfrage ohne Regel entscheidet, als eine Zeile (HUM-512).
///
/// Die Moduszeile der Oberfläche in Textform: Mit `upstream` nennt sie den
/// Entscheidungs-Proxy aus `Status.egress`, wer auflöst und was daraus
/// folgt. Vor 1.42 kennt der Daemon das Feld nicht; ein leeres heißt dann
/// „unbekannt" und nicht „in der App".
#[must_use]
pub fn decided_by_line(status: &v1::sandbox_event::Status, minor: u32) -> String {
    let route = status.egress.clone().unwrap_or_default();
    let text = match status.ask_mode.as_str() {
        "ui" => "asked in the app".to_owned(),
        "terminal" => "asked in the terminal of the run".to_owned(),
        "none" => "refused at once".to_owned(),
        "upstream" if route.proxy.is_empty() => {
            // Der Start lehnt das ab (`CONFIG_003`); die Zeile sagt es,
            // statt einen Proxy zu nennen, den es nicht gibt.
            "the decision proxy, but this daemon goes direct: the start refuses".to_owned()
        }
        "upstream" if route.resolve == "humanitl" => format!(
            "decided by {}; Humanitl resolves and checks every address first, so the name \
             leaves as a DNS query before the decision there",
            route.proxy
        ),
        "upstream" => format!(
            "decided by {}; it resolves the names, so a public name that points into your \
             network is its to refuse",
            route.proxy
        ),
        _ if minor < ASK_MODE_MINOR => format!("unknown (daemon 1.{minor})"),
        other => format!("unknown ({other})"),
    };
    format!("without a rule: {text}")
}

/// Der Server aus `--llm NAME` oder `--llm-server NAME`, wenn einer dasteht.
pub fn llm_server_pair(ctx: &Context) -> Option<String> {
    ctx.cli_pairs()
        .into_iter()
        .find(|(path, _)| path == humanitl_config::servers::SERVER)
        .map(|(_, value)| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// Woher die Wahl kommt, als Satzteil: `this run`, `the project profile
/// PATH`, `the profile NAME` oder `llm.active`.
#[must_use]
pub fn source_text(choice: &v1::LlmServerChoice) -> String {
    use v1::llm_server_choice::Source;
    match Source::try_from(choice.source).unwrap_or(Source::Unspecified) {
        Source::Run => "this run".to_owned(),
        Source::Project => format!("the project profile {}", choice.detail),
        Source::Profile => format!("the profile {}", choice.detail),
        // `llm.server` in `config.toml` travels as the default with this
        // detail (HUM-400); without it the default is `llm.active`.
        Source::Default if choice.detail == "config.toml" => "llm.server in config.toml".to_owned(),
        Source::Default => "llm.active".to_owned(),
        Source::Unspecified => "an unknown place".to_owned(),
    }
}

/// Die Zeile über den Server: Name und Herkunft, oder dass keiner gilt.
///
/// `minor` ist die Vertrags-Minor des Daemons: Vor 1.28 kennt er das Feld
/// nicht, und ein fehlendes heißt dann „unbekannt", nicht „keiner".
#[must_use]
pub fn server_line(choice: Option<&v1::LlmServerChoice>, minor: u32) -> String {
    if choice.is_none() && minor < SERVER_CHOICE_MINOR {
        return format!("llm server: unknown (daemon 1.{minor})");
    }
    match choice {
        Some(choice) if choice.refusal.is_some() => format!(
            "llm server: {} (chosen by {}, not saved: the start refuses)",
            choice.name,
            source_text(choice)
        ),
        Some(choice) => format!(
            "llm server: {} (chosen by {})",
            choice.name,
            source_text(choice)
        ),
        None => "llm server: none saved applies".to_owned(),
    }
}

/// Die Zeilen des Plans für Menschen.
fn plan_lines(status: &v1::sandbox_event::Status, minor: u32) -> Vec<String> {
    let mut lines = vec![
        format!("sandbox profile: {}", status.profile),
        format!("work dir: {} ({})", status.work_dir, status.work_mode),
    ];
    if !status.sandbox_mode.is_empty() {
        lines.push(format!("mode: {}", status.sandbox_mode));
    }
    lines.push(server_line(status.llm_server.as_ref(), minor));
    lines.push(decided_by_line(status, minor));
    lines.push(format!(
        "llm endpoint: {}",
        if status.llm_endpoint.is_empty() {
            "-"
        } else {
            status.llm_endpoint.as_str()
        }
    ));
    lines
}

/// Der Plan als JSON.
fn plan_json(status: &v1::sandbox_event::Status) -> serde_json::Value {
    let server = status.llm_server.as_ref().map(|choice| {
        use v1::llm_server_choice::Source;
        let source = match Source::try_from(choice.source).unwrap_or(Source::Unspecified) {
            Source::Run => "run",
            Source::Project => "project",
            Source::Profile => "profile",
            Source::Default => "default",
            Source::Unspecified => "",
        };
        json!({
            "name": choice.name,
            "source": source,
            "detail": choice.detail,
            "saved": choice.refusal.is_none(),
        })
    });
    json!({
        "profile": status.profile,
        "work_dir": status.work_dir,
        "work_mode": status.work_mode,
        "sandbox_mode": status.sandbox_mode,
        "llm_server": server,
        "llm_endpoint": status.llm_endpoint,
        "ask_mode": status.ask_mode,
        "egress": status.egress.as_ref().map(|route| json!({
            "proxy": route.proxy,
            "resolve": route.resolve,
        })),
    })
}

/// `CLI_004` für einen Daemon, der die Wahl je Lauf nicht kennt.
fn old_daemon(minor: u32) -> Diagnostic {
    Diagnostic::builder(codes::CLI_004, Severity::Error)
        .why(format!(
            "the daemon speaks contract 1.{minor}, and choosing a saved server for one run needs \
             1.{SERVER_CHOICE_MINOR}"
        ))
        .fix(FixAction::CopyCommand(
            "systemctl --user restart humanitld.service".to_owned(),
        ))
        .build()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    fn choice(source: v1::llm_server_choice::Source, detail: &str) -> v1::LlmServerChoice {
        v1::LlmServerChoice {
            name: "work".to_owned(),
            source: source.into(),
            detail: detail.to_owned(),
            refusal: None,
            unconfirmed: None,
            project_dir: String::new(),
            project_path: String::new(),
        }
    }

    /// Die Moduszeile in Textform (HUM-512): der Proxy und wer auflöst mit
    /// `upstream`, die übrigen Modi kurz, ein älterer Daemon unbekannt.
    #[test]
    fn the_mode_line_names_the_decision_proxy_and_who_resolves() {
        let mut status = v1::sandbox_event::Status {
            ask_mode: "upstream".to_owned(),
            egress: Some(v1::EgressRoute {
                proxy: "127.0.0.1:8080".to_owned(),
                resolve: "proxy".to_owned(),
            }),
            ..v1::sandbox_event::Status::default()
        };
        let line = super::decided_by_line(&status, 42);
        assert!(
            line.starts_with("without a rule: decided by 127.0.0.1:8080"),
            "{line}"
        );
        assert!(line.contains("its to refuse"), "{line}");
        status.egress = Some(v1::EgressRoute {
            proxy: "127.0.0.1:8080".to_owned(),
            resolve: "humanitl".to_owned(),
        });
        assert!(super::decided_by_line(&status, 42).contains("DNS query before the decision"));
        status.egress = None;
        assert!(super::decided_by_line(&status, 42).contains("the start refuses"));
        status.ask_mode = "ui".to_owned();
        assert_eq!(
            super::decided_by_line(&status, 42),
            "without a rule: asked in the app"
        );
        status.ask_mode = String::new();
        assert_eq!(
            super::decided_by_line(&status, 41),
            "without a rule: unknown (daemon 1.41)"
        );
    }

    #[test]
    fn the_server_line_names_server_and_source() {
        use v1::llm_server_choice::Source;
        assert_eq!(
            server_line(
                Some(&choice(Source::Project, "/p/.humanitl/profile.toml")),
                28
            ),
            "llm server: work (chosen by the project profile /p/.humanitl/profile.toml)"
        );
        assert_eq!(
            server_line(Some(&choice(Source::Profile, "team")), 28),
            "llm server: work (chosen by the profile team)"
        );
        assert_eq!(
            server_line(Some(&choice(Source::Run, "")), 28),
            "llm server: work (chosen by this run)"
        );
        assert_eq!(
            server_line(Some(&choice(Source::Default, "")), 28),
            "llm server: work (chosen by llm.active)"
        );
        assert_eq!(server_line(None, 28), "llm server: none saved applies");
    }

    /// Ein Daemon vor 1.28 sagt nichts über den Server; das heißt nicht,
    /// dass keiner gilt.
    #[test]
    fn an_older_daemon_reads_as_unknown_and_not_as_none() {
        use v1::llm_server_choice::Source;
        assert_eq!(
            server_line(Some(&choice(Source::Default, "config.toml")), 28),
            "llm server: work (chosen by llm.server in config.toml)"
        );
        assert_eq!(server_line(None, 27), "llm server: unknown (daemon 1.27)");
        assert_eq!(server_line(None, 28), "llm server: none saved applies");
    }

    #[test]
    fn an_unsaved_server_says_the_start_refuses() {
        let mut refused = choice(v1::llm_server_choice::Source::Project, "/p");
        refused.refusal = Some(v1::Diagnostic::default());
        assert!(server_line(Some(&refused), 28).contains("the start refuses"));
    }

    /// The context of `humanitl sandbox plan ARGS` in `home`, with these
    /// extra variables in its environment.
    fn plan_context(
        home: &std::path::Path,
        args: &[&str],
        extra: &[(&str, &str)],
    ) -> crate::cmd::Context {
        let matches = crate::cli::command()
            .try_get_matches_from(["humanitl", "sandbox", "plan"].iter().chain(args))
            .expect("the command line parses");
        let mut pairs = vec![
            ("HOME".to_owned(), home.display().to_string()),
            (
                "XDG_CONFIG_HOME".to_owned(),
                home.join("cfg").display().to_string(),
            ),
        ];
        pairs.extend(
            extra
                .iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned())),
        );
        let env = humanitl_config::Env::from_pairs(pairs);
        crate::cmd::Context {
            paths: humanitl_config::Paths::new(env.clone()),
            env,
            cwd: home.to_path_buf(),
            cli_config: crate::cli::config_pairs(&matches),
            config_file: None,
            profile: None,
            profile_means: crate::cmd::ProfileMeaning::Session,
            render: crate::render::Renderer::new(false, 0, true),
            source_lock: humanitl_config::agents::SourceLock::none(),
        }
    }

    /// The plan this command sends to a daemon of contract 1.`minor`.
    fn plan_sent(
        ctx: &crate::cmd::Context,
        minor: u32,
    ) -> Result<v1::sandbox_request::Plan, Failure> {
        snapshot_sent(ctx, SessionValues::Preview, minor)
    }

    /// The plan a snapshot with or without the session values sends.
    fn snapshot_sent(
        ctx: &crate::cmd::Context,
        values: SessionValues,
        minor: u32,
    ) -> Result<v1::sandbox_request::Plan, Failure> {
        let resolved = ctx.config().expect("the client resolves");
        let (work_dir, source) = ctx.work_dir(&resolved);
        let overrides = session_values(ctx, &resolved, values);
        plan_request(ctx, &work_dir, &source, String::new(), overrides, minor)
    }

    /// The daemon's resolution of this plan: its session values as
    /// `Sandbox(Plan)` mixes them (`SessionResolver::resolve_plan`), none of
    /// the client's variables, `config.toml` read afresh.
    fn plan_resolved(
        home: &std::path::Path,
        plan: &v1::sandbox_request::Plan,
    ) -> humanitl_config::Resolved {
        use humanitl_config::{Env, ProfileSelection};
        use humanitl_ipc::session::SessionResolver;
        let env = Env::from_pairs([
            ("HOME".to_owned(), home.display().to_string()),
            (
                "XDG_CONFIG_HOME".to_owned(),
                home.join("cfg").display().to_string(),
            ),
        ]);
        let daemon = SessionResolver::new(
            humanitl_config::Paths::new(env.clone()),
            humanitl_config::resolve(&ProfileSelection::any(), None, &env, &[])
                .expect("the daemon resolves"),
        );
        let overrides: Vec<(String, String)> = plan
            .cli_overrides
            .iter()
            .map(|pair| (pair.path.clone(), pair.value.clone()))
            .collect();
        daemon
            .resolve_plan(
                None,
                None,
                None,
                (!plan.backend.is_empty()).then(|| plan.backend.clone()),
                &overrides,
                None,
            )
            .expect("the plan resolves")
    }

    /// `humanitl sandbox plan --sandbox-min-isolation kernel` previews the
    /// start that `humanitl run` with the same flag gets (HUM-559): the floor
    /// travels in `Plan.cli_overrides`, the daemon's plan resolution raises
    /// its floor, and `auto` is refused with `SANDBOX_035` as at the start.
    /// The variable `HUMANITL_SANDBOX__MIN_ISOLATION` travels the same way.
    ///
    /// Mutation proof: with `cli_overrides` left empty in `plan_request`, or
    /// with the overrides not mixed in `resolve_plan`, the plan resolves with
    /// the daemon's weaker floor and `auto` is chosen.
    #[test]
    fn the_preview_raises_the_floor_like_the_start() {
        let home = tempfile::tempdir().expect("tempdir");
        let home = home.path();
        let kernel_env = [("HUMANITL_SANDBOX__MIN_ISOLATION", "kernel")];
        for (args, extra) in [
            (&["--sandbox-min-isolation", "kernel"][..], &[][..]),
            (&[][..], &kernel_env[..]),
        ] {
            let ctx = plan_context(home, args, extra);
            let plan = plan_sent(&ctx, PLAN_OVERRIDES_MINOR).expect("a new daemon takes it");
            let start = crate::cmd::run::client_overrides(&ctx, &ctx.config().expect("resolves"));
            assert_eq!(plan.cli_overrides, start, "{args:?} {extra:?}");
            let resolved = plan_resolved(home, &plan);
            assert_eq!(
                resolved.config.sandbox.min_isolation,
                humanitl_config::MinIsolation::Kernel,
                "{args:?} {extra:?}"
            );
            let refused = humanitl_sandbox::choose_backend(
                &resolved.config.sandbox,
                humanitl_sandbox::AUTO_BACKEND,
            )
            .expect_err("auto never reaches the kernel level");
            assert_eq!(refused.code.as_str(), "SANDBOX_035", "{}", refused.why);
        }
        // Gegenprobe: ohne Untergrenze der Kommandozeile reist keine mit, und
        // der Plan bleibt bei der Untergrenze des Daemons.
        let ctx = plan_context(home, &[], &[]);
        let plan = plan_sent(&ctx, PLAN_OVERRIDES_MINOR).expect("nothing to refuse");
        assert!(plan.cli_overrides.is_empty(), "{:?}", plan.cli_overrides);
        assert_ne!(
            plan_resolved(home, &plan).config.sandbox.min_isolation,
            humanitl_config::MinIsolation::Kernel
        );
    }

    /// A daemon before 1.55 does not know `Plan.cli_overrides` and would show
    /// a plan without the floor that the start then refuses; the preview says
    /// `CLI_004` instead, and only when a floor travels (HUM-559).
    ///
    /// Mutation proof: with `SESSION_FLOOR_MINOR` (1.49) as the bar in
    /// `preview_values`, a daemon 1.54 gets a plan without the floor.
    #[test]
    fn an_old_daemon_gets_no_preview_without_the_floor() {
        let home = tempfile::tempdir().expect("tempdir");
        let home = home.path();
        let ctx = plan_context(home, &["--sandbox-min-isolation", "kernel"], &[]);
        let refused = plan_sent(&ctx, PLAN_OVERRIDES_MINOR - 1).expect_err("too old for the floor");
        assert_eq!(refused.diagnostic.code.as_str(), "CLI_004");
        assert!(
            refused.diagnostic.why.contains("1.55"),
            "{}",
            refused.diagnostic.why
        );
        let ctx = plan_context(home, &[], &[]);
        assert!(plan_sent(&ctx, PLAN_OVERRIDES_MINOR - 1).is_ok());
    }

    /// `humanitl sessions` is no preview of a start (HUM-559): it sends no
    /// session values, so a floor from the environment neither changes its
    /// mode line nor ends it with `CLI_004` against a daemon before 1.55.
    ///
    /// Mutation proof: with `SessionValues::Preview` in the arm of
    /// `Without` in `session_values`, the plan carries the floor and the
    /// daemon 1.54 is refused.
    #[test]
    fn the_sessions_snapshot_sends_no_session_values() {
        let home = tempfile::tempdir().expect("tempdir");
        let home = home.path();
        let ctx = plan_context(home, &[], &[("HUMANITL_SANDBOX__MIN_ISOLATION", "kernel")]);
        let plan = snapshot_sent(&ctx, SessionValues::Without, PLAN_OVERRIDES_MINOR - 1)
            .expect("sessions works against a daemon 1.54");
        assert!(plan.cli_overrides.is_empty(), "{:?}", plan.cli_overrides);
        assert!(plan.backend.is_empty());
        assert!(snapshot_sent(&ctx, SessionValues::Preview, PLAN_OVERRIDES_MINOR - 1).is_err());
    }

    /// Before 1.55 a daemon sees no `Plan.cli_overrides` (HUM-559): the
    /// backend of the call travels in `Plan.backend` as well (known since
    /// 1.38), so a configured `kernel` floor with `--sandbox-backend` naming
    /// the VM backend previews that backend like the start; every other value is `CLI_004`, and
    /// before 1.38 the backend, too.
    ///
    /// Mutation proof: with `Plan.backend` left empty the plan of the daemon
    /// 1.54 previews `auto` below the floor; without the gate a daemon 1.54
    /// gets `llm.endpoint` in a field it does not know.
    #[test]
    fn an_older_daemon_previews_the_backend_and_refuses_what_it_cannot_see() {
        let home = tempfile::tempdir().expect("tempdir");
        let home = home.path();
        let config = plan_context(home, &[], &[]).paths.config_path();
        std::fs::create_dir_all(config.parent().expect("a directory")).expect("mkdir");
        std::fs::write(&config, "[sandbox]\nmin_isolation = \"kernel\"\n").expect("write");

        let ctx = plan_context(
            home,
            &["--sandbox-backend", humanitl_sandbox::VM_BACKEND],
            &[],
        );
        let plan = plan_sent(&ctx, PLAN_OVERRIDES_MINOR - 1).expect("1.54 knows Plan.backend");
        assert_eq!(plan.backend, humanitl_sandbox::VM_BACKEND);
        let resolved = plan_resolved(
            home,
            &v1::sandbox_request::Plan {
                cli_overrides: Vec::new(),
                ..plan.clone()
            },
        );
        assert_eq!(
            humanitl_sandbox::backend_name(resolved.config.sandbox.backend),
            humanitl_sandbox::VM_BACKEND
        );
        let refused =
            plan_sent(&ctx, PLAN_BACKEND_MINOR - 1).expect_err("1.37 knows no Plan.backend");
        assert_eq!(refused.diagnostic.code.as_str(), "CLI_004");
        // Die Minor, ab der das Backend ankäme, nicht die der übrigen Werte.
        assert!(
            refused.diagnostic.why.contains("needs 1.38"),
            "{}",
            refused.diagnostic.why
        );

        let ctx = plan_context(home, &["--llm", "http://ollama.lan:11434"], &[]);
        let refused = plan_sent(&ctx, PLAN_OVERRIDES_MINOR - 1).expect_err("unseen value");
        assert_eq!(refused.diagnostic.code.as_str(), "CLI_004");
        assert!(
            refused.diagnostic.why.contains("llm.endpoint"),
            "{}",
            refused.diagnostic.why
        );
        assert!(plan_sent(&ctx, PLAN_OVERRIDES_MINOR).is_ok());
    }

    #[test]
    fn the_json_carries_name_source_and_whether_it_is_saved() {
        let status = v1::sandbox_event::Status {
            llm_server: Some(choice(v1::llm_server_choice::Source::Project, "/p")),
            llm_endpoint: "https://llm.work.example/v1".to_owned(),
            ..Default::default()
        };
        let value = plan_json(&status);
        assert_eq!(value["llm_server"]["name"], "work");
        assert_eq!(value["llm_server"]["source"], "project");
        assert_eq!(value["llm_server"]["saved"], true);
        assert_eq!(value["llm_endpoint"], "https://llm.work.example/v1");
    }
}
