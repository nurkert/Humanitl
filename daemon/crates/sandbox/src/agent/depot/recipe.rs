//! The registered agents of the depot: one declarative recipe each
//! (ADR-0023, HUM-518).
//!
//! The depot core, the guard (`AGENT_017`), the masks of host installs, the
//! `Agents` RPC and the CLI iterate over [`KNOWN`] and never name a single
//! agent. Everything that differs between agents is data in its
//! [`KnownAgent`] entry.
//!
//! # How to add an agent to the depot
//!
//! Two places, nothing else:
//!
//! 1. one [`KnownAgent`] in [`KNOWN`] here: identifier, display name,
//!    program name, the host install locations to mask (always, recipe or
//!    not), and, once every value is sourced from the vendor's documentation,
//!    its [`DepotRecipe`] (program inside the version directory,
//!    configuration variable, self-update switch, credential file, the
//!    [`InstallRecipe`] of HUM-519 (script, hosts with evidence, program
//!    path, platforms, signature), login flow for HUM-520). Without a recipe `depot`
//!    answers `AGENT_015`;
//! 2. one line in the macro `known_agents!` in
//!    `daemon/crates/config/src/agents.rs`, which gives the identifier, the
//!    field `agents.<id>` and its branch of `AgentsConfig::settings`.
//!
//! The schema snapshot, `docs/CONFIG.md` and the reader register then name
//! the new keys; their tests say so.
//!
//! The tests below walk every entry and hold the invariants (absolute install
//! locations, a program name that is one plain file name, a credential file
//! inside the configuration copy, the config crate's lists in step), so a new
//! entry is checked without a new test.

/// How an agent logs in inside a maintenance run (HUM-520 builds the run).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginFlow {
    /// No stored login; the agent takes an API key from Humanitl's keyring.
    ApiKeyOnly,
    /// The agent prints a URL; the browser shows a code the person pastes
    /// into the terminal (Claude Code).
    PastedCode,
    /// A device flow that needs no callback into the sandbox (Codex
    /// `--device-auth`).
    DeviceCode,
}

/// The declarations of one agent for running from the depot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DepotRecipe {
    /// The program's path inside a version directory, one plain file name,
    /// which is also its name on the sandbox's `PATH`: `claude`.
    pub program: &'static str,
    /// The variable that points the agent at its configuration directory:
    /// `CLAUDE_CONFIG_DIR`. The launcher sets it to the session's copy.
    pub config_env: &'static str,
    /// Variables that turn off self-update in the session; the read-only
    /// program directory makes an update fail even without them.
    pub update_off: &'static [(&'static str, &'static str)],
    /// The credential file, relative to the configuration directory; the
    /// one file HUM-520 takes back through its guard.
    pub credential_file: Option<&'static str>,
    /// How an install or update runs in a maintenance sandbox (HUM-519);
    /// `None` until every value is sourced, then `install` answers
    /// `AGENT_015`.
    pub install: Option<InstallRecipe>,
    /// How a login works (HUM-520).
    pub login: LoginFlow,
}

/// One host an install or update reaches, with the place of the vendor's
/// documentation that names it (ADR-0023 point 5). Exact names only: the
/// maintenance run allows exactly these, everything else is held.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstallHost {
    /// The host, as the rule names it: `downloads.claude.ai`.
    pub host: &'static str,
    /// Where the vendor documents it, a URL with the date it was read.
    pub evidence: &'static str,
}

/// A platform the daemon can run on, as the daemon names it; the recipe
/// maps it to the vendor's name ([`InstallRecipe::platforms`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostPlatform {
    /// Linux on `x86_64` with glibc.
    LinuxX64,
    /// Linux on `aarch64` with glibc.
    LinuxArm64,
    /// Linux on `x86_64` with musl.
    LinuxX64Musl,
    /// Linux on `aarch64` with musl.
    LinuxArm64Musl,
    /// macOS on Intel.
    MacX64,
    /// macOS on Apple silicon.
    MacArm64,
}

/// The vendor signs a manifest of checksums (Claude Code: `manifest.json`
/// with a detached `OpenPGP` signature). The run only downloads manifest,
/// signature and the vendor's key; the daemon verifies its own copy of them
/// with the host's `gpg` against [`SignedManifest::fingerprint`] and compares
/// the program's SHA-256 with the verified manifest (`super::verify`). No
/// cryptography of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignedManifest {
    /// The fingerprint of the vendor's primary key, 40 hex digits in capitals
    /// without spaces.
    pub fingerprint: &'static str,
    /// The JSON pointer of the program's SHA-256 in the manifest, with
    /// `{platform}` for the vendor's platform name.
    pub checksum_pointer: &'static str,
    /// The JSON pointer of the version in the manifest. The version of a
    /// signed install comes from here, not from the run: a run that names
    /// another version than the signed manifest is refused.
    pub version_pointer: &'static str,
    /// Where the vendor documents key and manifest.
    pub evidence: &'static str,
}

/// What an install or update runs in its maintenance sandbox (HUM-519).
///
/// The daemon builds the run around [`InstallRecipe::script`]: it sets the
/// variables of the contract and runs the script under `/bin/sh -eu`. The
/// script only downloads; where [`InstallRecipe::signature`] is set, the
/// daemon verifies the download after the run, never the run itself.
/// The contract of the script:
///
/// - in: `HUMANITL_DEPOT_VERSION` (the version asked for, empty for the
///   vendor's current one), `HUMANITL_DEPOT_PLATFORM` (the vendor's name of
///   the platform), `HUMANITL_DEPOT_OUT` (the result directory), `HOME`;
/// - out: `$HUMANITL_DEPOT_OUT/version` with the version installed (one
///   line), the program tree under `$HUMANITL_DEPOT_OUT/program/`, and for a
///   signed recipe the manifest, its signature and the vendor's public key
///   as `signed/manifest`, `signed/signature` and `signed/key.asc`.
///
/// Only the program tree is taken over; a launcher or anything else the
/// installer leaves in `HOME` is discarded with the run.
///
/// What binds the version: for a signed recipe the verified manifest
/// ([`SignedManifest::version_pointer`]), and the run's `version` file must
/// say the same. For an unsigned recipe nothing does: the version is the
/// run's word, checked only for its form and against the version asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InstallRecipe {
    /// Shell commands for `/bin/sh`, run in the sandbox.
    pub script: &'static str,
    /// The hosts the run reaches; each becomes an `allow` rule of this run's
    /// session and of no other.
    pub hosts: &'static [InstallHost],
    /// Where the program lies inside the program tree after the run, a
    /// relative path; the program the session starts.
    pub program: &'static str,
    /// The vendor's names of the platforms it builds for.
    pub platforms: &'static [(HostPlatform, &'static str)],
    /// The vendor's signature, where it signs.
    pub signature: Option<SignedManifest>,
    /// The source the manifest names: where the program comes from.
    pub source: &'static str,
}

/// One agent `agents.<id>` knows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KnownAgent {
    /// The identifier, as in `agents.<id>`.
    pub id: &'static str,
    /// The name status, CLI and app show.
    pub display_name: &'static str,
    /// The program's file name on the host, which the guard recognises.
    pub program: &'static str,
    /// Where a host install of the agent lives, masked under the depot. Only
    /// what exists on the host is masked.
    pub host_installs: &'static [&'static str],
    /// The recipe for the depot, once every value is sourced.
    pub recipe: Option<DepotRecipe>,
}

/// Every agent `agents.<id>` knows, in the order of
/// `humanitl_config::agents::KNOWN_AGENTS`.
pub const KNOWN: &[KnownAgent] = &[
    // opencode.ai/docs (2026-11-17): the installer puts the binary under
    // `~/.opencode/bin` (the host's home, never mounted); the npm package
    // `opencode-ai` and Homebrew put it where the sandbox's `/usr` reaches.
    KnownAgent {
        id: "opencode",
        display_name: "OpenCode",
        program: "opencode",
        host_installs: &[
            "/usr/bin/opencode",
            "/usr/local/bin/opencode",
            "/usr/lib/node_modules/opencode-ai",
            "/usr/local/lib/node_modules/opencode-ai",
            "/opt/homebrew/bin/opencode",
        ],
        recipe: None,
    },
    // The adapter's install command (`pi.rs`, `INSTALL_COMMAND`): a global npm
    // install of `@earendil-works/pi-coding-agent`.
    KnownAgent {
        id: "pi",
        display_name: "pi",
        program: "pi",
        host_installs: &[
            "/usr/bin/pi",
            "/usr/local/bin/pi",
            "/usr/lib/node_modules/@earendil-works/pi-coding-agent",
            "/usr/local/lib/node_modules/@earendil-works/pi-coding-agent",
            "/opt/homebrew/bin/pi",
        ],
        recipe: None,
    },
    // code.claude.com/docs/en/setup and /settings, 2026-11-16: the native
    // installer (launcher under `~/.local/bin`, the host's home, never
    // mounted) or a global npm install of `@anthropic-ai/claude-code`.
    KnownAgent {
        id: "claude",
        display_name: "Claude Code",
        program: "claude",
        host_installs: &[
            "/usr/bin/claude",
            "/usr/local/bin/claude",
            "/usr/lib/node_modules/@anthropic-ai/claude-code",
            "/usr/local/lib/node_modules/@anthropic-ai/claude-code",
            "/opt/homebrew/bin/claude",
        ],
        recipe: Some(DepotRecipe {
            program: "claude",
            config_env: "CLAUDE_CONFIG_DIR",
            update_off: &[("DISABLE_UPDATES", "1")],
            credential_file: Some(".credentials.json"),
            install: Some(CLAUDE_INSTALL),
            login: LoginFlow::PastedCode,
        }),
    },
    // github.com/openai/codex README (2026-11-17): `npm install -g
    // @openai/codex`, `brew install --cask codex`, or the standalone
    // installer and release binaries renamed to `codex`. HUM-515 sources the
    // recipe.
    KnownAgent {
        id: "codex",
        display_name: "Codex",
        program: "codex",
        host_installs: &[
            "/usr/bin/codex",
            "/usr/local/bin/codex",
            "/usr/lib/node_modules/@openai/codex",
            "/usr/local/lib/node_modules/@openai/codex",
            "/opt/homebrew/bin/codex",
        ],
        recipe: None,
    },
    // antigravity.google/docs/getting-started (2026-11-17): `curl -fsSL
    // https://antigravity.google/cli/install.sh | bash` puts `agy` at
    // `~/.local/bin/agy`, the host's home, never mounted; the places below
    // cover a copy into the system's bin directories. HUM-517 sources the
    // recipe.
    KnownAgent {
        id: "agy",
        display_name: "Antigravity CLI",
        program: "agy",
        host_installs: &[
            "/usr/bin/agy",
            "/usr/local/bin/agy",
            "/opt/homebrew/bin/agy",
        ],
        recipe: None,
    },
];

/// Claude Code from the vendor's release bucket, verified against its signed
/// manifest (HUM-519).
///
/// Sources, read 2026-11-19:
///
/// - code.claude.com/docs/en/setup, section "Binary integrity and code
///   signing": every release publishes `manifest.json` with the SHA-256 of
///   every platform binary under `platforms.<platform>.checksum`, signed with
///   the key `31DD DE24 DDFA B679 F42D 7BD2 BAA9 29FF 1A7E CACE`, which lies at
///   `https://downloads.claude.ai/keys/claude-code.asc`; manifest and
///   signature at `https://downloads.claude.ai/claude-code-releases/<version>/
///   manifest.json` and `manifest.json.sig`, signed from 2.1.89 on.
/// - `https://claude.ai/install.sh` answers with a redirect to
///   `https://downloads.claude.ai/claude-code-releases/bootstrap.sh`, which
///   reads the current version from `<base>/latest`, the binary from
///   `<base>/<version>/<platform>/claude`, and contacts no other host.
///   It then runs `claude install`, which writes a launcher under
///   `~/.local/bin` and the version under `~/.local/share/claude/versions/`.
///   The recipe takes the same files without running the vendor's program:
///   nothing downloaded executes during the run, and only the one binary is
///   taken over, never a launcher.
/// - Platform names (same page, "Install with npm"): `linux-x64`,
///   `linux-arm64`, `linux-x64-musl`, `linux-arm64-musl`, `darwin-x64`,
///   `darwin-arm64`.
pub const CLAUDE_INSTALL: InstallRecipe = InstallRecipe {
    script: r#"base=https://downloads.claude.ai/claude-code-releases
out=$HUMANITL_DEPOT_OUT
version=$HUMANITL_DEPOT_VERSION
if [ -z "$version" ]; then
    version=$(curl -fsSL "$base/latest")
fi
case $version in
    [0-9]*.[0-9]*.[0-9]*) ;;
    *) echo "humanitl: the vendor named no version: $version" >&2; exit 1 ;;
esac
mkdir -p "$out/program" "$out/signed"
curl -fsSL -o "$out/signed/manifest" "$base/$version/manifest.json"
curl -fsSL -o "$out/signed/signature" "$base/$version/manifest.json.sig"
curl -fsSL -o "$out/signed/key.asc" https://downloads.claude.ai/keys/claude-code.asc
curl -fsSL -o "$out/program/claude" "$base/$version/$HUMANITL_DEPOT_PLATFORM/claude"
chmod 0700 "$out/program/claude"
printf '%s
' "$version" > "$out/version"
"#,
    hosts: &[InstallHost {
        host: "downloads.claude.ai",
        evidence: concat!(
            "https://code.claude.com/docs/en/setup#binary-integrity-and-code-signing ",
            "(2026-11-19); https://claude.ai/install.sh redirects to ",
            "https://downloads.claude.ai/claude-code-releases/bootstrap.sh, ",
            "which contacts only this host"
        ),
    }],
    program: "claude",
    platforms: &[
        (HostPlatform::LinuxX64, "linux-x64"),
        (HostPlatform::LinuxArm64, "linux-arm64"),
        (HostPlatform::LinuxX64Musl, "linux-x64-musl"),
        (HostPlatform::LinuxArm64Musl, "linux-arm64-musl"),
        (HostPlatform::MacX64, "darwin-x64"),
        (HostPlatform::MacArm64, "darwin-arm64"),
    ],
    signature: Some(SignedManifest {
        fingerprint: "31DDDE24DDFAB679F42D7BD2BAA929FF1A7ECACE",
        checksum_pointer: "/platforms/{platform}/checksum",
        // `https://downloads.claude.ai/claude-code-releases/2.1.89/manifest.json`
        // (read 2026-11-19): top-level `version` ("2.1.89"), `buildDate` and
        // `platforms.<platform>.{binary,checksum,size}`.
        version_pointer: "/version",
        evidence: concat!(
            "https://code.claude.com/docs/en/setup#verify-the-manifest-signature ",
            "(2026-11-19)"
        ),
    }),
    source: "https://downloads.claude.ai/claude-code-releases",
};

/// The known agent with this identifier.
#[must_use]
pub fn known(id: &str) -> Option<&'static KnownAgent> {
    KNOWN.iter().find(|agent| agent.id == id)
}

/// The known agent whose program has this file name.
#[must_use]
pub fn by_program(program: &std::ffi::OsStr) -> Option<&'static KnownAgent> {
    KNOWN.iter().find(|agent| program == agent.program)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::path::{Component, Path};

    use super::*;

    fn plain_name(name: &str) -> bool {
        let mut parts = Path::new(name).components();
        matches!(parts.next(), Some(Component::Normal(_))) && parts.next().is_none()
    }

    #[test]
    fn the_table_follows_the_known_agents_of_the_config() {
        let ids: Vec<&str> = KNOWN.iter().map(|agent| agent.id).collect();
        assert_eq!(ids, humanitl_config::agents::KNOWN_AGENTS);
        for agent in KNOWN {
            assert!(
                humanitl_config::Config::default()
                    .agents
                    .settings(agent.id)
                    .is_some(),
                "{} has no agents.<id> in the configuration",
                agent.id
            );
        }
    }

    /// The notice at load follows the table: `depot` for an agent without a
    /// recipe is a warning, for one with a recipe none.
    #[test]
    fn the_load_notice_follows_the_recipes() {
        for agent in KNOWN {
            let text = format!("[{}]\nsource = \"depot\"\n", agent.id);
            let config = humanitl_config::Config {
                agents: toml::from_str(&text).unwrap(),
                ..humanitl_config::Config::default()
            };
            let notices = super::super::load_notices(&config);
            assert_eq!(notices.is_empty(), agent.recipe.is_some(), "{}", agent.id);
        }
    }

    /// Every registered recipe, present and future, keeps these.
    #[test]
    #[allow(clippy::too_many_lines)]
    fn every_registered_agent_keeps_the_invariants() {
        let mut programs = std::collections::BTreeSet::new();
        for agent in KNOWN {
            assert!(!agent.display_name.is_empty(), "{}", agent.id);
            assert!(
                !agent.host_installs.is_empty(),
                "{}: every agent names its host install locations, recipe or not",
                agent.id
            );
            assert!(plain_name(agent.program), "{}: program", agent.id);
            assert!(
                programs.insert(agent.program),
                "{}: program twice",
                agent.id
            );
            for place in agent.host_installs {
                let path = Path::new(place);
                assert!(path.is_absolute(), "{}: {place}", agent.id);
                assert!(
                    !path.components().any(|part| part == Component::ParentDir),
                    "{}: {place}",
                    agent.id
                );
                // A mask over the sandbox's own tree or over a whole system
                // directory would hide more than an agent.
                assert!(!path.starts_with("/run/humanitl"), "{}: {place}", agent.id);
                assert!(path.components().count() >= 3, "{}: {place}", agent.id);
            }
            let Some(recipe) = agent.recipe else {
                continue;
            };
            assert!(plain_name(recipe.program), "{}: recipe program", agent.id);
            assert!(
                recipe
                    .config_env
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c == '_'),
                "{}: config_env",
                agent.id
            );
            assert!(
                !crate::bridge_env::RESERVED_ENV.contains(&recipe.config_env),
                "{}: config_env is reserved",
                agent.id
            );
            if let Some(file) = recipe.credential_file {
                // Inside the configuration copy and never the program.
                assert!(plain_name(file), "{}: credential file {file}", agent.id);
                assert_ne!(file, recipe.program, "{}", agent.id);
                assert_ne!(file, super::super::store::MANIFEST_FILE, "{}", agent.id);
            }
            if let Some(install) = recipe.install {
                assert!(!install.hosts.is_empty(), "{}: install hosts", agent.id);
                for host in install.hosts {
                    assert!(
                        !host.host.contains(['*', '/', ':']) && host.host.contains('.'),
                        "{}: install host {}",
                        agent.id,
                        host.host
                    );
                    assert!(
                        humanitl_core::HostName::parse(host.host).is_ok(),
                        "{}: install host {}",
                        agent.id,
                        host.host
                    );
                    assert!(
                        host.evidence.starts_with("https://"),
                        "{}: every install host names its evidence",
                        agent.id
                    );
                }
                assert!(
                    super::super::adopt::relative_inside(install.program),
                    "{}: install program {}",
                    agent.id,
                    install.program
                );
                assert_eq!(
                    install.program, recipe.program,
                    "{}: the session starts program/<version>/<program>",
                    agent.id
                );
                assert!(!install.platforms.is_empty(), "{}", agent.id);
                assert!(install.source.starts_with("https://"), "{}", agent.id);
                if let Some(signature) = install.signature {
                    assert_eq!(signature.fingerprint.len(), 40, "{}", agent.id);
                    assert!(
                        signature
                            .fingerprint
                            .chars()
                            .all(|c| c.is_ascii_digit() || c.is_ascii_uppercase()),
                        "{}",
                        agent.id
                    );
                    assert!(
                        signature.checksum_pointer.starts_with('/')
                            && signature.checksum_pointer.contains("{platform}")
                            && signature.version_pointer.starts_with('/'),
                        "{}",
                        agent.id
                    );
                }
            }
            for (key, _) in recipe.update_off {
                assert!(
                    !crate::bridge_env::RESERVED_ENV.contains(key),
                    "{}",
                    agent.id
                );
            }
        }
    }
}
