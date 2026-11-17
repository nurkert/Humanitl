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
//!    configuration variable, self-update switch, credential file, install
//!    hosts for HUM-519, login flow for HUM-520). Without a recipe `depot`
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
    /// The hosts an install or update downloads from (HUM-519); exact names.
    pub install_hosts: &'static [&'static str],
    /// How a login works (HUM-520).
    pub login: LoginFlow,
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
            install_hosts: &["claude.ai", "downloads.claude.ai"],
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
            for host in recipe.install_hosts {
                assert!(
                    !host.contains(['*', '/', ':']) && host.contains('.'),
                    "{}: install host {host}",
                    agent.id
                );
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
