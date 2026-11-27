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

use super::credential::{
    CredentialField, CredentialFormat, EntryMap, NamedFields, Requires, ValueForm,
};

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
    /// The agent asks in the run's terminal which provider to log in to and
    /// then for its key, which the person pastes there (`OpenCode`'s
    /// `opencode auth login`, #110). No URL to open and no callback; a
    /// provider's own browser login reaches hosts the recipe does not name,
    /// and they are held like any request of the run.
    TerminalPrompt,
}

/// The declarations of one agent for running from the depot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DepotRecipe {
    /// The program's path inside a version directory, one plain file name,
    /// which is also its name on the sandbox's `PATH`: `claude`.
    pub program: &'static str,
    /// The variable that points the agent at its configuration directory:
    /// `CLAUDE_CONFIG_DIR`. The launcher sets it to the session's copy.
    ///
    /// `None` for an agent whose configuration directory no variable moves
    /// (`agy` keeps it under `~/.gemini`, HUM-517): it keeps its
    /// configuration in the session's own home as with the source `host`,
    /// the depot carries only its program, and a configuration run is
    /// refused (`AGENT_015`), because nothing it wrote would ever be read.
    pub config_env: Option<&'static str>,
    /// Variables that turn off self-update in the session; the read-only
    /// program directory makes an update fail even without them.
    pub update_off: &'static [(&'static str, &'static str)],
    /// The credential file, relative to the configuration directory; the
    /// one file HUM-520 takes back through its guard. A relative path inside
    /// the copy, never absolute and never with `..` (#110: `OpenCode`'s
    /// `opencode/auth.json` under `XDG_DATA_HOME`); the depot stores it under
    /// its file name ([`credential_name`]).
    pub credential_file: Option<&'static str>,
    /// How an install or update runs in a maintenance sandbox (HUM-519);
    /// `None` until every value is sourced, then `install` answers
    /// `AGENT_015`.
    pub install: Option<InstallRecipe>,
    /// Why a configuration run is refused although
    /// [`DepotRecipe::config_env`] points at a copy (#110: `OpenCode`'s copy is
    /// its data directory, which holds sessions and logs, not configuration);
    /// `agents config` then answers `AGENT_015` with it.
    pub config_refused: Option<&'static str>,
    /// Why there is no [`DepotRecipe::install`], with its evidence, for the
    /// `why` of `AGENT_015` (HUM-566): an agent whose vendor ships no install
    /// the depot can pin and check says so instead of a general sentence.
    pub install_refused: Option<&'static str>,
    /// How a login works (HUM-520).
    pub login: LoginFlow,
    /// The format of the credential file, which the guard of the write-back
    /// checks (HUM-520, [`super::credential`]). `None` until every key is
    /// sourced from the vendor or measured: then nothing is taken back, and
    /// `agents login` answers `AGENT_014`. The guard never guesses a format.
    pub credential_format: Option<CredentialFormat>,
    /// The login run (HUM-520); `None` until its command and hosts are
    /// sourced, then `agents login` answers `AGENT_014`.
    pub login_run: Option<LoginRecipe>,
    /// The keys of the agent's configuration files that run commands; the
    /// configuration run highlights a change of them (HUM-520 point 5).
    pub command_keys: &'static [&'static str],
}

/// A login inside a maintenance run (HUM-520 point 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoginRecipe {
    /// The words after the depot program: `["auth", "login"]`.
    pub args: &'static [&'static str],
    /// The vendor's login hosts with their evidence: `allow` rules of the
    /// run's session alone, and the only hosts whose URL a client may open
    /// on the host after a click or confirmation; every other URL is shown,
    /// never opened.
    pub hosts: &'static [InstallHost],
    /// Where the agent waits for the browser's callback on its own loopback,
    /// if it does (Codex: `localhost:1455/auth/callback`). The daemon
    /// delivers a pasted callback URL inside the run, never by forwarding a
    /// port; on a backend that shares the host's loopback only the device
    /// flow works ([`LoginFlow::DeviceCode`]).
    pub callback: Option<LoginCallback>,
    /// Single named keys of another file that hold the account
    /// ([`NamedFields`]); never the file.
    pub named: Option<NamedFields>,
    /// Where the person turns the device flow on at the vendor, for the
    /// `fix` of `AGENT_014` when only the device flow is possible and it
    /// failed.
    pub device_flow_setting: Option<&'static str>,
}

/// The callback of a browser login on the agent's loopback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoginCallback {
    /// The port, `1455`.
    pub port: u16,
    /// The path, `/auth/callback`.
    pub path: &'static str,
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

/// The vendor signs nothing, but publishes a list of SHA-256 checksums of its
/// release archives, and its release metadata names the SHA-256 of that list
/// (HUM-566; Codex: `release.json` with `assets[].digest` and
/// `codex-package_SHA256SUMS`).
///
/// The run downloads metadata, list and archive and unpacks the archive into
/// the program tree; the daemon reads its own copy of the three files after
/// the run and checks the chain itself (`super::checksums`): the metadata
/// names the version the run installed, the list has the SHA-256 the metadata
/// names, and the archive has the SHA-256 the list names for the platform.
/// What binds the chain is TLS to the vendor's host, the same trust the
/// vendor's own installer has; there is no key of the vendor's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChecksumList {
    /// The JSON pointer of the release's name in the metadata: `/tag_name`.
    pub version_pointer: &'static str,
    /// What precedes the version in that name: `rust-v`.
    pub version_prefix: &'static str,
    /// The name of the checksum list among the metadata's `/assets`, each an
    /// object with `name` and `digest` (`sha256:<hex>`).
    pub list: &'static str,
    /// The archive's name in the list, with `{platform}` for the vendor's
    /// platform name.
    pub archive: &'static str,
    /// Where the vendor documents metadata, list and archive.
    pub evidence: &'static str,
}

/// The vendor signs nothing and publishes no list of checksums, but releases
/// on GitHub as an immutable release (#110, ADR-0023 amendment 2026-11-27;
/// `OpenCode`). GitHub computes the SHA-256 of every asset at upload and names
/// it in the release metadata (`assets[].digest`, `sha256:<hex>`); an
/// immutable release keeps its assets and its tag from then on
/// (docs.github.com, "Immutable releases").
///
/// The run downloads metadata and archive and unpacks the archive into the
/// program tree; the daemon reads its own copy of both after the run and
/// checks them itself (`super::checksums::check_release_digest`): the
/// metadata names the version the run installed, says the release is
/// immutable, and names exactly one SHA-256 for the platform's archive, which
/// the archive has.
///
/// The anchor is weaker than a [`ChecksumList`]: the SHA-256 is GitHub's
/// statement about what was uploaded, not one of the vendor's, and what binds
/// it is TLS to GitHub. Anyone who can upload to the vendor's repository
/// stands behind the release as the vendor would.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReleaseDigest {
    /// The JSON pointer of the release's name in the metadata: `/tag_name`.
    pub version_pointer: &'static str,
    /// What precedes the version in that name: `v`.
    pub version_prefix: &'static str,
    /// The JSON pointer of the flag that must be `true`: `/immutable`. A
    /// release without it, or with it `false`, is refused.
    pub immutable_pointer: &'static str,
    /// The archive's name among the metadata's `/assets`, with `{platform}`
    /// for the vendor's platform name.
    pub archive: &'static str,
    /// Where GitHub documents the metadata and the flag.
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
///   as `signed/manifest`, `signed/signature` and `signed/key.asc`; for a
///   recipe with a [`ChecksumList`] the vendor's release metadata, the list
///   and the archive the tree came from as `checked/metadata`,
///   `checked/list` and `checked/archive`; for a recipe with a
///   [`ReleaseDigest`] the metadata and the archive alone.
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
    /// relative path; the program the session starts. When it is not the
    /// plain name [`DepotRecipe::program`] (Codex: `bin/codex`), the daemon
    /// puts a relative link of that name at the top of the version directory
    /// to it, as the vendor's installer does, so the session finds the
    /// program by its name and the program finds its helpers beside it.
    pub program: &'static str,
    /// Further files the tree must hold as regular files, relative paths
    /// (HUM-566): the helpers of a program that comes as a package of
    /// several files. A tree without one of them is refused.
    pub expected: &'static [&'static str],
    /// The vendor's names of the platforms it builds for.
    pub platforms: &'static [(HostPlatform, &'static str)],
    /// The vendor's signature, where it signs.
    pub signature: Option<SignedManifest>,
    /// The vendor's list of checksums, where it publishes one instead of a
    /// signature (HUM-566).
    pub checksums: Option<ChecksumList>,
    /// GitHub's digest of an immutable release, where the vendor publishes
    /// neither a signature nor a list (#110).
    pub release_digest: Option<ReleaseDigest>,
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
    // `~/.opencode/bin` (the host's home, never mounted; re-read 2026-11-27
    // in the vendor's `install` at tag v1.19.6, `INSTALL_DIR=$HOME/.opencode/
    // bin`); the npm package `opencode-ai` and Homebrew put it where the
    // sandbox's `/usr` reaches.
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
        recipe: Some(OPENCODE_RECIPE),
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
            config_env: Some("CLAUDE_CONFIG_DIR"),
            update_off: &[("DISABLE_UPDATES", "1")],
            credential_file: Some(".credentials.json"),
            install: Some(CLAUDE_INSTALL),
            install_refused: None,
            config_refused: None,
            login: LoginFlow::PastedCode,
            // The keys of `.credentials.json` and the fields of `.claude.json`
            // that a session needs to start logged in are not published by
            // the vendor; they are to be measured in the owner walkthrough
            // (HUM-553) before a format is declared here. Until then nothing
            // is taken back and `agents login claude` answers `AGENT_014`.
            // Re-read 2026-11-20 (HUM-514): code.claude.com/docs/en/
            // authentication names only place and mode (`~/.claude/
            // .credentials.json`, `0600`, under `CLAUDE_CONFIG_DIR` when set),
            // `claude auth login`, the prompt `Paste code here if prompted`
            // and the sign-in hosts (network-config: `claude.ai`,
            // `claude.com`, `platform.claude.com`, `sso.claude.com`), no key
            // of the file.
            credential_format: None,
            login_run: None,
            // code.claude.com/docs/en/settings-reference, settings index
            // (2026-11-20, HUM-514): every key of a settings file that runs
            // a command (`apiKeyHelper`, `awsAuthRefresh`,
            // `awsCredentialExport`, `gcpAuthRefresh`, `otelHeadersHelper`,
            // `fileSuggestion`, `statusLine`, `hooks`, `processWrapper`,
            // `policyHelper`, `sshHostCommands`), `env`, which changes what
            // runs, and `mcpServers` of `.claude.json`, which starts servers
            // (code.claude.com/docs/en/settings, "~/.claude.json").
            command_keys: &[
                "hooks",
                "apiKeyHelper",
                "statusLine",
                "mcpServers",
                "env",
                "awsAuthRefresh",
                "awsCredentialExport",
                "gcpAuthRefresh",
                "otelHeadersHelper",
                "fileSuggestion",
                "processWrapper",
                "policyHelper",
                "sshHostCommands",
            ],
        }),
    },
    // github.com/openai/codex README (2026-11-17, re-read 2026-11-20 for
    // HUM-515): `npm install -g @openai/codex`, `brew install --cask codex`,
    // or the standalone installer (`https://chatgpt.com/codex/install.sh`),
    // which puts `codex` under `~/.local/bin` and the package under
    // `CODEX_HOME/packages/standalone` (learn.chatgpt.com/docs/config-file/
    // environment-variables, "Installer variables"), both in the host's home,
    // never mounted. A global npm install lies under `{prefix}/lib/
    // node_modules` with its command in `{prefix}/bin`, and `prefix` defaults
    // to where node is installed, `/usr/local` or, for a distribution's node,
    // `/usr` (docs.npmjs.com/cli/v11/configuring-npm/folders, "prefix
    // Configuration", "Node Modules"); Homebrew's cask links
    // `/opt/homebrew/bin/codex`.
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
        recipe: Some(CODEX_RECIPE),
    },
    // antigravity.google/docs/getting-started (2026-11-17, re-read
    // 2026-11-20 for HUM-517): `curl -fsSL
    // https://antigravity.google/cli/install.sh | bash` puts `agy` at
    // `~/.local/bin/agy`, the host's home, never mounted, or with `--dir` in
    // any directory (antigravity.google/cli/install.sh); the vendor names no
    // npm package and no system package. The places below cover a copy or
    // `--dir` into the system's bin directories.
    KnownAgent {
        id: "agy",
        display_name: "Antigravity CLI",
        program: "agy",
        host_installs: &[
            "/usr/bin/agy",
            "/usr/local/bin/agy",
            "/opt/homebrew/bin/agy",
        ],
        recipe: Some(AGY_RECIPE),
    },
];

/// The vendor's repository at the tag every `OpenCode` source below was read
/// at (2026-11-27): `v1.19.6`, commit
/// `f4e363c8295fa90a03dddc451882fffe8fbce357`.
pub const OPENCODE_SOURCE: &str =
    "https://github.com/anomalyco/opencode/blob/f4e363c8295fa90a03dddc451882fffe8fbce357/";

/// `OpenCode` from the depot (#110). Sources, read 2026-11-27 at
/// [`OPENCODE_SOURCE`] (paths relative to it):
///
/// - **Data directory as the copy** ([`DepotRecipe::config_env`]
///   `XDG_DATA_HOME`): `OpenCode` keeps its provider logins in
///   `path.join(Global.Path.data, "auth.json")`
///   (`packages/opencode/src/auth/index.ts`), written with mode `0600`, and
///   `Global.Path.data` is `path.join(xdgData, "opencode")`
///   (`packages/core/src/global.ts`, `xdg-basedir`: `$XDG_DATA_HOME` or
///   `~/.local/share`). `OPENCODE_CONFIG_DIR` moves only the configuration
///   directory (same file, `Global.make`; `packages/opencode/src/config/
///   paths.ts` adds it to the directories read), never `auth.json`. So the
///   depot's copy is `OpenCode`'s data directory's parent, and the credential
///   file lies at `opencode/auth.json` inside it. The adapter's own
///   configuration (`XDG_CONFIG_HOME/opencode`, the managed configuration,
///   `OPENCODE_CONFIG`) is untouched by it (`super::super::opencode`).
/// - `OPENCODE_DISABLE_AUTOUPDATE` (`truthy`: `true` or `1`) turns the update
///   check off (`packages/core/src/flag/flag.ts`,
///   `packages/opencode/src/cli/upgrade.ts`); the adapter sets it as well.
/// - **Login in the run's terminal** ([`LoginFlow::TerminalPrompt`], lead's
///   decision on #110): `opencode auth login` (`providers` has the alias
///   `auth`, `login` its command, `packages/opencode/src/cli/cmd/
///   providers.ts`) asks for a provider and then its key or the provider's
///   own login, and writes `auth.json` (`Auth.set`). Only this run stores the
///   first file and adds a provider; a session renews what is stored, and a
///   `/connect` in a session that adds a provider is refused by the guard
///   (`AGENT_012`).
/// - **Credential format** ([`OPENCODE_AUTH_JSON`]): a map of entries.
/// - **No configuration run** ([`DepotRecipe::config_refused`]): the copy is
///   the data directory, with sessions, logs and the catalog's cache, not
///   configuration; the adapter writes `OpenCode`'s configuration itself.
/// - The install ([`OPENCODE_INSTALL`]) takes the release archive from
///   GitHub, checked against GitHub's digest of an immutable release.
pub const OPENCODE_RECIPE: DepotRecipe = DepotRecipe {
    program: "opencode",
    config_env: Some("XDG_DATA_HOME"),
    update_off: &[("OPENCODE_DISABLE_AUTOUPDATE", "true")],
    credential_file: Some("opencode/auth.json"),
    install: Some(OPENCODE_INSTALL),
    install_refused: None,
    config_refused: Some(
        "its copy in the depot is OpenCode's data directory (XDG_DATA_HOME), which holds \
         sessions, logs and the model catalog's cache, not configuration; Humanitl writes \
         OpenCode's configuration for every session itself",
    ),
    login: LoginFlow::TerminalPrompt,
    credential_format: Some(OPENCODE_AUTH_JSON),
    login_run: Some(LoginRecipe {
        args: &["auth", "login"],
        // The key itself needs no network. `login` refreshes the provider
        // catalog first (`modelsDev.refresh(true)`) from
        // `https://models.opencode.ai/api.json` with a timeout of 10 s and
        // falls back to the catalog built in (`packages/core/src/
        // models-dev.ts`); allowed, so the run does not wait on a hold.
        hosts: &[InstallHost {
            host: "models.opencode.ai",
            evidence: concat!(
                "https://github.com/anomalyco/opencode/blob/f4e363c8295fa90a03dddc451882fffe8fbce357/",
                "packages/core/src/models-dev.ts (source https://models.opencode.ai, ",
                "fetchApi <source>/api.json) and packages/opencode/src/cli/cmd/providers.ts ",
                "(modelsDev.refresh(true)), read 2026-11-27"
            ),
        }],
        callback: None,
        named: None,
        device_flow_setting: None,
    }),
    // No configuration run, so nothing to highlight.
    command_keys: &[],
};

/// One key of an `OpenCode` login entry.
const fn opencode_field(pointer: &'static str, form: ValueForm, required: bool) -> CredentialField {
    CredentialField {
        pointer,
        form,
        required,
    }
}

/// The format of `OpenCode`'s `auth.json`, from the schema that reads and
/// writes it (`packages/opencode/src/auth/index.ts` at [`OPENCODE_SOURCE`],
/// read 2026-11-27): an object from a provider's identifier (any string, a
/// URL for `wellknown`; `Auth.set` strips a trailing `/`) to `Info`, the
/// union of three classes picked by `type`:
///
/// - `oauth`: `refresh` and `access` (`String`), `expires`
///   (`NonNegativeInt`), optional `accountId` and `enterpriseUrl`;
/// - `api`: `key` (`String`), optional `metadata` (`Record<String,
///   String>`);
/// - `wellknown`: `key` (the name of the variable the token goes into) and
///   `token` (`String`).
///
/// `Auth.all` drops an entry that does not decode; the guard refuses the
/// whole file instead. No expiry: each `oauth` entry carries its own, so a
/// session writes back only when it started on the stored file. No account:
/// the file names none.
pub const OPENCODE_AUTH_JSON: CredentialFormat = CredentialFormat {
    fields: &[],
    expiry: None,
    account: None,
    one_of: &[],
    requires: &[],
    fixed: &[],
    entries: Some(EntryMap {
        tag: "type",
        kinds: &[
            (
                "oauth",
                &[
                    opencode_field("/type", ValueForm::OneOf(&["oauth"]), true),
                    opencode_field("/refresh", ValueForm::Token, true),
                    opencode_field("/access", ValueForm::Token, true),
                    opencode_field("/expires", ValueForm::Count, true),
                    opencode_field("/accountId", ValueForm::Text, false),
                    opencode_field("/enterpriseUrl", ValueForm::Text, false),
                ],
            ),
            (
                "api",
                &[
                    opencode_field("/type", ValueForm::OneOf(&["api"]), true),
                    opencode_field("/key", ValueForm::Token, true),
                    opencode_field("/metadata", ValueForm::TextMap, false),
                ],
            ),
            (
                "wellknown",
                &[
                    opencode_field("/type", ValueForm::OneOf(&["wellknown"]), true),
                    opencode_field("/key", ValueForm::Text, true),
                    opencode_field("/token", ValueForm::Token, true),
                ],
            ),
        ],
        max_entries: 64,
    }),
    evidence: concat!(
        "https://github.com/anomalyco/opencode/blob/f4e363c8295fa90a03dddc451882fffe8fbce357/",
        "packages/opencode/src/auth/index.ts (Oauth, Api, WellKnown, Info), read 2026-11-27"
    ),
};

/// `OpenCode` from its GitHub release, checked against GitHub's digest of the
/// immutable release (#110, ADR-0023 amendment 2026-11-27).
///
/// Sources, read 2026-11-27:
///
/// - The vendor's installer `install` at [`OPENCODE_SOURCE`]: archives at
///   `https://github.com/anomalyco/opencode/releases/download/v<version>/
///   opencode-<target><ext>`, `.tar.gz` on Linux, `.zip` on macOS; the
///   target is `<os>-<arch>`, then `-baseline` for an `x64` CPU without AVX2,
///   then `-musl` on a musl system; it unpacks and moves `opencode` from the
///   top of the archive. It installs `OpenCode` 1 only ("`OpenCode` 2 ships as
///   `@opencode/cli`"; a version from 2 on is refused) and verifies nothing.
///   Without a version the installer reads the newest `v1.x.y` `tag_name` of
///   `https://api.github.com/repos/anomalyco/opencode/releases?per_page=100`
///   (its fallback; npm's registry, its first choice, is left out: one source
///   of the version suffices). The depot reads `releases/latest` instead,
///   which GitHub defines as the newest release that is neither a draft nor
///   a prerelease (docs.github.com/en/rest/releases, "Get the latest
///   release"): a prerelease is never installed without `--version`. Once the
///   latest release is `OpenCode` 2, a run without `--version` is refused
///   below and names the version; an `OpenCode` 1 release is then pinned
///   with `--version`.
/// - The release `v1.19.6` (`https://api.github.com/repos/anomalyco/opencode/
///   releases/tags/v1.19.6`): `"immutable": true`, every asset with
///   `"digest": "sha256:<hex>"`; no list of checksums, no signature of the
///   CLI archives (`.github/workflows/publish.yml` signs only the macOS and
///   Windows binaries with the platforms' code signing).
/// - The download from `github.com` answers with a redirect to
///   `release-assets.githubusercontent.com` (measured with a `HEAD` request,
///   2026-11-27; nothing was downloaded).
/// - x64 takes the `baseline` build: the depot does not look at the CPU, and
///   the vendor builds it for every x64 CPU, as its installer chooses it
///   without AVX2.
pub const OPENCODE_INSTALL: InstallRecipe = InstallRecipe {
    script: r#"api=https://api.github.com/repos/anomalyco/opencode
download=https://github.com/anomalyco/opencode/releases/download
out=$HUMANITL_DEPOT_OUT
version=$HUMANITL_DEPOT_VERSION
if [ -z "$version" ]; then
    version=$(curl -fsSL "$api/releases/latest" | tr ',' '\n' \
        | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"v\([^"]*\)".*/\1/p' | head -n 1)
fi
case $version in
    *[!0-9A-Za-z.+-]*) echo "humanitl: the vendor named no version: $version" >&2; exit 1 ;;
    1.[0-9]*.[0-9]*) ;;
    *) echo "humanitl: no OpenCode 1 version: $version" >&2; exit 1 ;;
esac
asset="opencode-$HUMANITL_DEPOT_PLATFORM"
mkdir -p "$out/program" "$out/checked"
curl -fsSL -o "$out/checked/metadata" "$api/releases/tags/v$version"
# Each JSON object's own keys, without its children and without blanks outside
# strings: the release must say `"immutable":true` itself, and exactly one
# asset object names the archive and its digest, in whatever key order.
expected=$(tr -d '\n' < "$out/checked/metadata" | awk -v asset="$asset" '{
    depth = 0; quoted = 0; escaped = 0
    for (i = 1; i <= length($0); i++) {
        c = substr($0, i, 1)
        if (quoted) {
            own[depth] = own[depth] c
            if (escaped) escaped = 0
            else if (c == "\\") escaped = 1
            else if (c == "\"") quoted = 0
            continue
        }
        if (c == "\"") { quoted = 1; own[depth] = own[depth] c; continue }
        if (c == " " || c == "\t" || c == "\r") continue
        if (c == "{") { depth++; own[depth] = ""; continue }
        if (c == "}") {
            text = own[depth]
            if (depth == 1 && index(text, "\"immutable\":true") > 0) immutable = 1
            if (index(text, "\"name\":\"" asset "\"") > 0) {
                found++
                sum = ""
                if (match(text, /"digest":"sha256:[0-9a-fA-F]*"/)) {
                    sum = substr(text, RSTART + 17, RLENGTH - 18)
                }
            }
            depth--
            continue
        }
        own[depth] = own[depth] c
    }
} END {
    if (!immutable) { print "not-immutable"; exit }
    if (found == 1 && length(sum) == 64) print tolower(sum)
}')
if [ "$expected" = not-immutable ]; then
    echo "humanitl: release v$version is not immutable" >&2
    exit 1
fi
if [ -z "$expected" ]; then
    echo "humanitl: release v$version names no single digest for $asset" >&2
    exit 1
fi
curl -fsSL -o "$out/checked/archive" "$download/v$version/$asset"
actual=$(sha256sum "$out/checked/archive" | cut -d ' ' -f 1)
if [ "$expected" != "$actual" ]; then
    echo "humanitl: $asset does not match the digest of release v$version" >&2
    exit 1
fi
case $asset in
    *.tar.gz) tar -xzf "$out/checked/archive" -C "$out/program" --no-same-owner --no-same-permissions ;;
    *.zip) unzip -q "$out/checked/archive" -d "$out/program" ;;
    *) echo "humanitl: unknown archive $asset" >&2; exit 1 ;;
esac
printf '%s\n' "$version" > "$out/version"
"#,
    hosts: &[
        InstallHost {
            host: "api.github.com",
            evidence: concat!(
                "https://github.com/anomalyco/opencode/blob/f4e363c8295fa90a03dddc451882fffe8fbce357/",
                "install (releases?per_page=100), ",
                "https://docs.github.com/en/rest/releases/releases#get-the-latest-release (releases/latest) and ",
                "https://docs.github.com/en/rest/releases/releases (immutable, assets[].digest), ",
                "read 2026-11-27"
            ),
        },
        InstallHost {
            host: "github.com",
            evidence: concat!(
                "https://github.com/anomalyco/opencode/blob/f4e363c8295fa90a03dddc451882fffe8fbce357/",
                "install (releases/download/v<version>/<filename>), read 2026-11-27"
            ),
        },
        InstallHost {
            host: "release-assets.githubusercontent.com",
            evidence: concat!(
                "https://github.com/anomalyco/opencode/releases/download/v1.19.6/",
                "opencode-linux-x64.tar.gz answers 302 with this host, HEAD request 2026-11-27"
            ),
        },
    ],
    program: "opencode",
    expected: &[],
    platforms: &[
        (HostPlatform::LinuxX64, "linux-x64-baseline.tar.gz"),
        (HostPlatform::LinuxArm64, "linux-arm64.tar.gz"),
        (HostPlatform::LinuxX64Musl, "linux-x64-baseline-musl.tar.gz"),
        (HostPlatform::LinuxArm64Musl, "linux-arm64-musl.tar.gz"),
        (HostPlatform::MacX64, "darwin-x64-baseline.zip"),
        (HostPlatform::MacArm64, "darwin-arm64.zip"),
    ],
    signature: None,
    checksums: None,
    release_digest: Some(ReleaseDigest {
        version_pointer: "/tag_name",
        version_prefix: "v",
        immutable_pointer: "/immutable",
        archive: "opencode-{platform}",
        evidence: concat!(
            "https://docs.github.com/en/rest/releases/releases (immutable, assets[].digest) and ",
            "https://docs.github.com/en/code-security/supply-chain-security/",
            "understanding-your-software-supply-chain/immutable-releases, read 2026-11-27"
        ),
    }),
    source: "https://github.com/anomalyco/opencode/releases",
};

/// The Antigravity CLI from the depot (HUM-517). Sources, read 2026-11-20:
/// antigravity.google/docs (install, tab CLI; cli/troubleshooting; settings)
/// and the install script antigravity.google/cli/install.sh.
///
/// - **API key only** ([`LoginFlow::ApiKeyOnly`], ADR-0023 point 5): `agy`
///   keeps its login "in the OS-native keyring … Linux Secret Service/dbus"
///   (install page, tab CLI), and the page names no file fallback; the
///   host's session bus never enters a sandbox. A depot session takes the
///   key from Humanitl's keyring through the adapter's launcher
///   (`super::super::agy`), and `agents login agy` answers `AGENT_014`
///   with the command that stores a key.
/// - **No configuration variable** ([`DepotRecipe::config_env`] `None`):
///   settings, rules and skills lie under `~/.gemini`, and the documentation
///   names no variable that moves it. The session keeps them in its own
///   tmpfs home, where the adapter writes `modelProvider = "gemini"` before
///   every start; a configuration run is refused.
/// - `AGY_CLI_DISABLE_AUTO_UPDATE=true` turns the background self-updater
///   off (cli/troubleshooting).
/// - **No install recipe, decided in HUM-566** ([`AGY_INSTALL_REFUSED`]):
///   `agents install agy` answers `AGENT_015` with the reason.
pub const AGY_RECIPE: DepotRecipe = DepotRecipe {
    program: "agy",
    config_env: None,
    update_off: &[("AGY_CLI_DISABLE_AUTO_UPDATE", "true")],
    credential_file: None,
    install: None,
    install_refused: Some(AGY_INSTALL_REFUSED),
    config_refused: None,
    login: LoginFlow::ApiKeyOnly,
    credential_format: None,
    login_run: None,
    // No configuration run, so nothing to highlight.
    command_keys: &[],
};

/// Why the depot installs no `agy` (HUM-566, decision on point 3).
///
/// Read 2026-11-20: the install script `https://antigravity.google/cli/install.sh`
/// reads `<base>/manifests/<platform>.json` from
/// `antigravity-cli-auto-updater-974169037036.us-central1.run.app`, a JSON
/// object with `version`, `url` and `sha512`, downloads `url` and compares
/// its SHA-512 with the same object; it has no option for a version. The
/// manifest for `linux_amd64` named version `1.5.13` and the URL
/// `https://storage.googleapis.com/antigravity-public/antigravity-cli/
/// 1.5.13-5999302598852608/linux-x64/cli_linux_x64.tar.gz`.
///
/// A recipe with a host known only at run time would be admissible where the
/// host comes from a fixed list (the run holds every other host anyway). It
/// fails on three other points: the manifest names the current version and
/// no other, and nothing fetches a chosen one, so neither `--version` nor an
/// update to a chosen version is possible (ADR-0023 point 5); the program's host `storage.googleapis.com` serves
/// every bucket of Google Cloud Storage, so an `allow` rule for it lets the
/// run fetch from any bucket an unsigned manifest names; and the only check
/// is a SHA-512 in that same unsigned manifest, from the same answer as the
/// URL. The archive's layout is documented nowhere either.
pub const AGY_INSTALL_REFUSED: &str = "the vendor's installer (antigravity.google/cli/install.sh, \
     read 2026-11-20) fetches only the current version and offers no way to fetch a chosen one, \
     so `--version` and an update to a chosen version are impossible; it fetches \
     the program from storage.googleapis.com, a host of every Cloud Storage bucket, at a URL an \
     unsigned manifest names, and checks it only against a SHA-512 from that same manifest; \
     until the vendor publishes pinned, checkable releases nothing is installed";

/// Codex from the depot (HUM-515). Sources, read 2026-11-20: the vendor's
/// documentation at learn.chatgpt.com and the repository
/// `github.com/openai/codex` at commit
/// `a0a30c40e8dbcc44af52e2df64dcce64368d4422` (paths below relative to it).
///
/// - `CODEX_HOME` is the root of config, auth, logs and sessions
///   (learn.chatgpt.com/docs/config-file/environment-variables).
/// - No variable turns the update check off; `check_for_update_on_startup`
///   does, and under Linux the adapter enforces it through
///   `/etc/codex/requirements.toml`, which holds for the depot's copy too
///   (`super::super::codex`). Hence no [`DepotRecipe::update_off`].
/// - `auth.json` under `CODEX_HOME` with `cli_auth_credentials_store =
///   "file"` (learn.chatgpt.com/docs/auth, "Credential storage"), enforced
///   by the same requirements file.
/// - The install ([`CODEX_INSTALL`], HUM-566) takes the vendor's whole
///   package and starts `bin/codex` through a link `codex -> bin/codex`, as
///   the vendor's installer does.
pub const CODEX_RECIPE: DepotRecipe = DepotRecipe {
    program: "codex",
    config_env: Some("CODEX_HOME"),
    update_off: &[],
    credential_file: Some("auth.json"),
    install: Some(CODEX_INSTALL),
    install_refused: None,
    config_refused: None,
    login: LoginFlow::DeviceCode,
    credential_format: Some(CODEX_AUTH_JSON),
    login_run: Some(LoginRecipe {
        // learn.chatgpt.com/docs/auth, "Login on headless devices":
        // `codex login --device-auth`; "Open the link in your browser, sign
        // in, then enter the one-time code".
        args: &["login", "--device-auth"],
        hosts: &[InstallHost {
            host: "auth.openai.com",
            evidence: concat!(
                "https://github.com/openai/codex/blob/a0a30c40e8dbcc44af52e2df64dcce64368d4422/",
                "codex-rs/login/src/server.rs (DEFAULT_ISSUER https://auth.openai.com) and ",
                "codex-rs/login/src/device_code_auth.rs (deviceauth/usercode, deviceauth/token, ",
                "verification URL <issuer>/codex/device), read 2026-11-20"
            ),
        }],
        // The device flow opens no listener: the browser flow's callback
        // server (`codex-rs/login/src/server.rs`, port 1455, redirect
        // `http://127.0.0.1:<port>/auth/callback`) is not part of this run,
        // so there is nothing to deliver a pasted URL to.
        callback: None,
        named: None,
        device_flow_setting: Some(
            "turn on device code login in your ChatGPT security settings (personal account) \
             or in the ChatGPT workspace permissions (workspace admin); \
             https://learn.chatgpt.com/docs/auth",
        ),
    }),
    // learn.chatgpt.com/docs/config-file/config-reference (2026-11-20): keys
    // of `config.toml` that run a command (`notify`, `mcp_servers.<id>.
    // command` and `.http_headers_helper`, `model_providers.<id>.auth.
    // command`, `hooks`, `desktop.custom_file_handlers.<id>.command`),
    // `shell_environment_policy`, which changes what commands see, and
    // `profiles`, which can carry every one of them.
    command_keys: &[
        "notify",
        "mcp_servers",
        "model_providers",
        "hooks",
        "desktop",
        "shell_environment_policy",
        "profiles",
    ],
};

/// The values of `auth_mode` the format takes: the two modes Codex's own
/// logins store, `ApiKey` (`login_with_api_key`) and `Chatgpt`
/// (`persist_tokens_async`), as `AuthMode` serialises them
/// (`#[serde(rename_all = "lowercase")]`, `codex-rs/protocol/src/auth.rs`).
/// Every other mode (`chatgptAuthTokens`, `headers`, `agentIdentity`,
/// `personalAccessToken`, `bedrockApiKey`, `bedrockAccessKeys`) belongs to
/// another source of credentials, and `headers` the loader refuses outright
/// ("externally provided auth cannot be loaded from auth storage",
/// `codex-rs/login/src/auth/manager.rs`).
pub const CODEX_AUTH_MODES: &[&str] = &["apikey", "chatgpt"];

/// The format of Codex's `auth.json`, from the struct that writes it:
/// `AuthDotJson` in `codex-rs/login/src/auth/storage.rs` and `TokenData` in
/// `codex-rs/login/src/token_data.rs` (commit
/// `a0a30c40e8dbcc44af52e2df64dcce64368d4422`, read 2026-11-20).
///
/// - `OPENAI_API_KEY` is an `Option<String>` without `skip_serializing_if`:
///   always written, `null` after a `ChatGPT` login without an exchanged key;
///   `tokens.account_id` likewise.
/// - `tokens.id_token` is written as the raw JWT (`serialize_id_token`);
///   `access_token` is a JWT, `refresh_token` a string.
/// - `last_refresh` is an RFC 3339 time, not a count; the token's expiry
///   lies inside the JWT, so the format declares no [`Expiry`](super::credential::Expiry) and the guard
///   takes a file back only from a session that started on the stored one.
/// - The account is `tokens.account_id`, which the login copies from the
///   `chatgpt_account_id` claim of the `id_token`
///   (`codex-rs/login/src/server.rs`, `persist_tokens_async`); the JWT itself
///   is read by nobody here. The source leaves it `null` when the claim is
///   missing; the guard then refuses the file whenever it carries `tokens`
///   (`CredentialFormat::account`): tokens without a named account could be
///   any account's.
/// - At least one of `OPENAI_API_KEY` and `tokens` holds a value
///   ([`CredentialFormat::one_of`]): `{"OPENAI_API_KEY": null}` is no login.
/// - `auth_mode` is one of [`CODEX_AUTH_MODES`]; `chatgpt` needs `tokens`,
///   `apikey` needs `OPENAI_API_KEY` ([`CredentialFormat::requires`]), and a
///   later write-back keeps the stored mode ([`CredentialFormat::fixed`]):
///   a session that wrote `"headers"` would leave a file the loader refuses
///   (HUM-515, review).
/// - Left out on purpose, so refused: `agent_identity` (a string or an
///   object, `AgentIdentityStorage`), `personal_access_token`,
///   `bedrock_api_key`, `bedrock_access_keys`. They belong to other login
///   modes than the device flow of the login run.
pub const CODEX_AUTH_JSON: CredentialFormat = CredentialFormat {
    fields: &[
        CredentialField {
            pointer: "/auth_mode",
            form: ValueForm::OneOf(CODEX_AUTH_MODES),
            required: false,
        },
        CredentialField {
            pointer: "/OPENAI_API_KEY",
            form: ValueForm::TokenOrNull,
            required: true,
        },
        CredentialField {
            pointer: "/tokens",
            form: ValueForm::Object,
            required: false,
        },
        CredentialField {
            pointer: "/tokens/id_token",
            form: ValueForm::Token,
            required: true,
        },
        CredentialField {
            pointer: "/tokens/access_token",
            form: ValueForm::Token,
            required: true,
        },
        CredentialField {
            pointer: "/tokens/refresh_token",
            form: ValueForm::Token,
            required: true,
        },
        CredentialField {
            pointer: "/tokens/oauth_provenance",
            form: ValueForm::Object,
            required: false,
        },
        CredentialField {
            pointer: "/tokens/oauth_provenance/issuer",
            form: ValueForm::Text,
            required: true,
        },
        CredentialField {
            pointer: "/tokens/oauth_provenance/client_id",
            form: ValueForm::Text,
            required: true,
        },
        CredentialField {
            pointer: "/tokens/login_owner_id",
            form: ValueForm::Text,
            required: false,
        },
        CredentialField {
            pointer: "/tokens/account_id",
            form: ValueForm::TextOrNull,
            required: true,
        },
        CredentialField {
            pointer: "/last_refresh",
            form: ValueForm::Text,
            required: false,
        },
    ],
    expiry: None,
    account: Some("/tokens/account_id"),
    // An API-key login writes `OPENAI_API_KEY` without `tokens`, a ChatGPT
    // login `tokens` (and, when a key was exchanged, the key too): a file
    // with neither is no login (HUM-515, review).
    one_of: &["/OPENAI_API_KEY", "/tokens"],
    // `CodexAuth::from_auth_dot_json` (`codex-rs/login/src/auth/manager.rs`)
    // refuses an API-key mode without a key ("API key auth is missing a
    // key.") and builds a ChatGPT login from `tokens`.
    requires: &[
        Requires {
            pointer: "/auth_mode",
            equals: "chatgpt",
            present: &["/tokens"],
        },
        Requires {
            pointer: "/auth_mode",
            equals: "apikey",
            present: &["/OPENAI_API_KEY"],
        },
    ],
    // The refresh (`persist_tokens` in the same file) refuses any mode but
    // `chatgpt` and keeps `auth_mode`; only a login changes it.
    fixed: &["/auth_mode"],
    entries: None,
    evidence: concat!(
        "https://github.com/openai/codex/blob/a0a30c40e8dbcc44af52e2df64dcce64368d4422/",
        "codex-rs/login/src/auth/storage.rs (AuthDotJson) and codex-rs/login/src/token_data.rs ",
        "(TokenData), read 2026-11-20"
    ),
};

/// Codex from the vendor's release host, checked against its list of
/// checksums (HUM-566).
///
/// Sources, read 2026-11-20: the vendor's installer
/// `https://github.com/openai/codex/blob/a0a30c40e8dbcc44af52e2df64dcce64368d4422/scripts/install/install.sh`
/// and the release `0.171.0` on `releases.openai.com`.
///
/// - The installer reads from `RELEASES_BASE_URL =
///   https://releases.openai.com/codex` (GitHub's releases are its fallback,
///   left out here: one host suffices): the current version as `tag_name`
///   (`rust-v<version>`) of `<base>/channels/latest`, a pinned one's metadata
///   at `<base>/releases/<version>/release.json`, the assets at
///   `<base>/releases/<version>/<asset>`.
/// - Integrity: no signature. The metadata names the SHA-256 of
///   `codex-package_SHA256SUMS` (`assets[].digest`, `sha256:<hex>`), the list
///   one line `<sha256>  <asset>` per archive, and the installer compares the
///   archive with its line before it unpacks it. The run does the same before
///   `tar` sees the archive; the daemon checks the chain again on its own
///   copy after the run (`super::checksums`).
/// - The package `codex-package-<target>.tar.gz` (measured on
///   `x86_64-unknown-linux-musl`, 0.171.0, 189 MiB packed, 63 entries): `bin/codex`,
///   `bin/codex-code-mode-host`, `bin/codex-exec-server`, `codex-path/rg`,
///   `codex-resources/bwrap`, `codex-resources/voice/…` and
///   `codex-package.json` (`"entrypoint": "bin/codex"`). The installer
///   requires `codex-package.json`, `bin/codex`, `bin/codex-code-mode-host`
///   and `codex-path/rg` (and `bwrap` under Linux, which a session does not
///   use: Codex's own sandbox does not start in Humanitl's, HUM-515), and
///   links `codex -> bin/codex` in the release directory.
/// - Targets: Linux only as static musl builds, for glibc and musl alike.
/// - Whether `codex` runs from the depot beside its helpers is measured in
///   the owner walkthrough (HUM-553 point 4); the installer starts it the
///   same way, through the link.
pub const CODEX_INSTALL: InstallRecipe = InstallRecipe {
    script: r#"base=https://releases.openai.com/codex
out=$HUMANITL_DEPOT_OUT
version=$HUMANITL_DEPOT_VERSION
if [ -z "$version" ]; then
    version=$(curl -fsSL "$base/channels/latest" | tr ',' '\n' \
        | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"rust-v\([^"]*\)".*/\1/p' | head -n 1)
fi
case $version in
    *[!0-9A-Za-z.+-]*) echo "humanitl: the vendor named no version: $version" >&2; exit 1 ;;
    [0-9]*.[0-9]*.[0-9]*) ;;
    *) echo "humanitl: the vendor named no version: $version" >&2; exit 1 ;;
esac
asset="codex-package-$HUMANITL_DEPOT_PLATFORM.tar.gz"
mkdir -p "$out/program" "$out/checked"
curl -fsSL -o "$out/checked/metadata" "$base/releases/$version/release.json"
curl -fsSL -o "$out/checked/list" "$base/releases/$version/codex-package_SHA256SUMS"
curl -fsSL -o "$out/checked/archive" "$base/releases/$version/$asset"
expected=$(awk -v asset="$asset" '{ sub(/^\*/, "", $2) }
    $2 == asset { n++; sum = tolower($1) }
    END { if (n == 1 && length(sum) == 64) print sum }' "$out/checked/list")
actual=$(sha256sum "$out/checked/archive" | cut -d ' ' -f 1)
if [ -z "$expected" ] || [ "$expected" != "$actual" ]; then
    echo "humanitl: $asset does not match the vendor's checksum list" >&2
    exit 1
fi
tar -xzf "$out/checked/archive" -C "$out/program" --no-same-owner --no-same-permissions
printf '%s\n' "$version" > "$out/version"
"#,
    hosts: &[InstallHost {
        host: "releases.openai.com",
        evidence: concat!(
            "https://github.com/openai/codex/blob/a0a30c40e8dbcc44af52e2df64dcce64368d4422/",
            "scripts/install/install.sh (RELEASES_BASE_URL https://releases.openai.com/codex, ",
            "channels/latest, releases/<version>/release.json and assets), read 2026-11-20"
        ),
    }],
    program: "bin/codex",
    expected: &[
        "bin/codex-code-mode-host",
        "codex-path/rg",
        "codex-package.json",
    ],
    platforms: &[
        (HostPlatform::LinuxX64, "x86_64-unknown-linux-musl"),
        (HostPlatform::LinuxX64Musl, "x86_64-unknown-linux-musl"),
        (HostPlatform::LinuxArm64, "aarch64-unknown-linux-musl"),
        (HostPlatform::LinuxArm64Musl, "aarch64-unknown-linux-musl"),
        (HostPlatform::MacX64, "x86_64-apple-darwin"),
        (HostPlatform::MacArm64, "aarch64-apple-darwin"),
    ],
    signature: None,
    checksums: Some(ChecksumList {
        version_pointer: "/tag_name",
        version_prefix: "rust-v",
        list: "codex-package_SHA256SUMS",
        archive: "codex-package-{platform}.tar.gz",
        evidence: concat!(
            "https://github.com/openai/codex/blob/a0a30c40e8dbcc44af52e2df64dcce64368d4422/",
            "scripts/install/install.sh (checksum_asset, package_asset) and ",
            "https://releases.openai.com/codex/releases/0.171.0/release.json, read 2026-11-20"
        ),
    }),
    release_digest: None,
    source: "https://releases.openai.com/codex/releases",
};

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
    expected: &[],
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
    checksums: None,
    release_digest: None,
    source: "https://downloads.claude.ai/claude-code-releases",
};

/// Whether `path` is a credential path the guard takes: relative, inside the
/// copy, without `..`, `.` or an empty part, and not more than
/// [`CREDENTIAL_DEPTH`] parts deep (#110).
#[must_use]
pub fn credential_path_ok(path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').collect();
    !path.is_empty()
        && parts.len() <= CREDENTIAL_DEPTH
        && parts
            .iter()
            .all(|part| !part.is_empty() && *part != "." && *part != ".." && !part.contains('\0'))
}

/// The most parts a credential path has: a directory and the file
/// (`opencode/auth.json`).
pub const CREDENTIAL_DEPTH: usize = 2;

/// The name under which the depot stores the credential file `path`: its
/// last part. `credentials/` holds one flat file per agent.
#[must_use]
pub fn credential_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

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

    /// Codex's `auth.json` as `AuthDotJson` writes it after a device login
    /// (`codex-rs/login/src/server.rs`, `persist_tokens_async`) or an API-key
    /// login: the declared format takes both, and refuses tokens without a
    /// named account, a file with neither key nor tokens, keys of other login
    /// modes, a key the struct does not have, a missing token and a wrong
    /// type. The tokens are made up; no real file is read (HUM-520,
    /// "Fallstricke").
    ///
    /// Mutation proofs: `TokenOrNull` checked like `Token` turns the first
    /// assert red; `check_document` without the `one_of` rule turns the
    /// `{"OPENAI_API_KEY":null}` assert red; without the account rule the
    /// `null` account assert; `OneOf` that takes any string the `headers`
    /// assert; without the `requires` rule the `chatgpt` without tokens
    /// assert.
    #[test]
    fn the_codex_format_takes_the_file_its_login_writes() {
        use super::super::credential::check_document;

        let login = |api_key: &str, account: &str| {
            serde_json::from_str::<serde_json::Value>(&format!(
                r#"{{"auth_mode":"chatgpt","OPENAI_API_KEY":{api_key},"tokens":{{"id_token":"eyJh.eyJi.c2ln","access_token":"eyJh.eyJj.c2ln","refresh_token":"rt_0123456789","oauth_provenance":{{"issuer":"https://auth.openai.com","client_id":"app_test"}},"login_owner_id":"owner-1","account_id":{account}}},"last_refresh":"2026-11-20T03:00:00Z"}}"#
            ))
            .unwrap()
        };
        let format = &CODEX_AUTH_JSON;
        check_document(&login("null", r#""acc-1""#), format).unwrap();
        check_document(&login(r#""sk-exchanged""#, r#""acc-1""#), format).unwrap();
        let api_key: serde_json::Value =
            serde_json::from_str(r#"{"auth_mode":"apikey","OPENAI_API_KEY":"sk-test"}"#).unwrap();
        check_document(&api_key, format).unwrap();

        // Tokens without a named account could be any account's.
        assert!(check_document(&login("null", "null"), format).is_err());
        assert!(check_document(&login("null", r#""""#), format).is_err());

        let refused = |text: &str| {
            let value: serde_json::Value = serde_json::from_str(text).unwrap();
            assert!(check_document(&value, format).is_err(), "taken: {text}");
        };
        // Neither key nor tokens: no login.
        refused(r#"{"OPENAI_API_KEY":null}"#);
        refused(r#"{"auth_mode":"chatgpt","OPENAI_API_KEY":null,"last_refresh":"x"}"#);
        // Another login mode, a key the struct does not know.
        refused(r#"{"OPENAI_API_KEY":"sk-test","agent_identity":"eyJh.eyJi.c2ln"}"#);
        refused(r#"{"OPENAI_API_KEY":"sk-test","hooks":{}}"#);
        // The key Codex always writes is missing, a token is missing.
        refused(r#"{"auth_mode":"chatgpt"}"#);
        refused(
            r#"{"OPENAI_API_KEY":null,"tokens":{"id_token":"a.b.c","access_token":"a.b.c","account_id":"acc-1"}}"#,
        );
        // A wrong type.
        refused(r#"{"OPENAI_API_KEY":42}"#);
        // A mode the loader refuses or that is no login of Codex's own.
        refused(r#"{"auth_mode":"headers","OPENAI_API_KEY":"sk-test"}"#);
        refused(r#"{"auth_mode":"agentIdentity","OPENAI_API_KEY":"sk-test"}"#);
        // A mode without its material.
        refused(r#"{"auth_mode":"chatgpt","OPENAI_API_KEY":"sk-test"}"#);
        let mut no_key = login("null", r#""acc-1""#);
        no_key["auth_mode"] = serde_json::Value::from("apikey");
        assert!(
            check_document(&no_key, format).is_err(),
            "apikey without a key"
        );
        assert_eq!(CODEX_AUTH_JSON.account, Some("/tokens/account_id"));
        assert_eq!(CODEX_AUTH_JSON.expiry, None, "the expiry lies in the JWT");
    }

    /// `OpenCode`'s logins land in the depot's copy (#110): the copy is its
    /// `XDG_DATA_HOME`, and `auth.json` lies at `opencode/auth.json` inside it,
    /// where `packages/opencode/src/auth/index.ts` writes it
    /// (`Global.Path.data` = `$XDG_DATA_HOME/opencode`).
    ///
    /// Mutation proofs: the `OpenCode` recipe back to `None` turns the first
    /// assert red; a credential file outside the copy (`../auth.json`, an
    /// absolute path) the asserts on the path.
    #[test]
    fn the_opencode_credential_file_lies_inside_the_configuration_copy() {
        let recipe = known("opencode")
            .and_then(|agent| agent.recipe)
            .expect("OpenCode has a depot recipe");
        assert_eq!(recipe.config_env, Some("XDG_DATA_HOME"));
        let file = recipe.credential_file.expect("a credential file");
        assert_eq!(file, "opencode/auth.json");
        assert!(credential_path_ok(file));
        let copy = Path::new(super::super::DEPOT_CONFIG_DST);
        let in_session = copy.join(file);
        assert!(in_session.starts_with(copy));
        assert!(
            in_session
                .components()
                .all(|part| matches!(part, Component::RootDir | Component::Normal(_)))
        );
        // `$XDG_DATA_HOME/opencode/auth.json` with the copy as XDG_DATA_HOME.
        assert_eq!(
            in_session,
            Path::new("/run/humanitl/agent-config/opencode/auth.json")
        );
        assert_eq!(credential_name(file), "auth.json");
        assert_eq!(recipe.login, LoginFlow::TerminalPrompt);
        assert_eq!(
            recipe.login_run.map(|login| login.args),
            Some(&["auth", "login"][..])
        );
        assert_eq!(recipe.credential_format, Some(OPENCODE_AUTH_JSON));
        assert!(recipe.config_refused.is_some());
        assert!(
            recipe
                .update_off
                .contains(&("OPENCODE_DISABLE_AUTOUPDATE", "true"))
        );
        let install = recipe.install.expect("an install recipe");
        assert!(install.signature.is_none() && install.checksums.is_none());
        let digest = install.release_digest.expect("GitHub's release digest");
        assert_eq!(digest.immutable_pointer, "/immutable");
        let hosts: Vec<&str> = install.hosts.iter().map(|host| host.host).collect();
        assert_eq!(
            hosts,
            [
                "api.github.com",
                "github.com",
                "release-assets.githubusercontent.com"
            ]
        );
        for platform in [
            HostPlatform::LinuxX64,
            HostPlatform::LinuxArm64,
            HostPlatform::MacArm64,
        ] {
            assert!(
                install.platforms.iter().any(|(have, _)| *have == platform),
                "{platform:?}"
            );
        }
    }

    /// `OpenCode`'s format takes what `opencode auth login` writes for each
    /// kind (`packages/opencode/src/auth/index.ts`), and refuses an unknown
    /// `type`, an entry that is no object, a key a kind does not have, a
    /// missing required key and an entry name with a space. Made-up values.
    ///
    /// Mutation proof: `entry_fields` that takes any `type` turns the first
    /// refusal red.
    #[test]
    fn the_opencode_format_takes_the_entries_its_login_writes() {
        use super::super::credential::check_document;
        let format = &OPENCODE_AUTH_JSON;
        let taken: serde_json::Value = serde_json::from_str(
            r#"{
                "opencode": {"type": "api", "key": "sk-zen-0123"},
                "openrouter": {"type": "api", "key": "sk-or-1", "metadata": {"label": "work"}},
                "anthropic": {"type": "oauth", "refresh": "r-1", "access": "a-1", "expires": 1700000000000},
                "github-copilot": {"type": "oauth", "refresh": "r-2", "access": "a-2", "expires": 0, "enterpriseUrl": "https://example.invalid", "accountId": "acc"},
                "https://auth.example.invalid": {"type": "wellknown", "key": "EXAMPLE_TOKEN", "token": "t-1"}
            }"#,
        )
        .unwrap();
        check_document(&taken, format).unwrap();
        check_document(&serde_json::json!({}), format).unwrap();
        for text in [
            r#"{"x":{"type":"session","key":"k"}}"#,
            r#"{"x":{"key":"k"}}"#,
            r#"{"x":"sk-plain"}"#,
            r#"{"x":["api"]}"#,
            r#"{"x":{"type":"api","key":"k","hooks":"curl evil"}}"#,
            r#"{"x":{"type":"api"}}"#,
            r#"{"x":{"type":"oauth","refresh":"r","access":"a","expires":-1}}"#,
            r#"{"x":{"type":"api","key":"k","metadata":{"a":1}}}"#,
            r#"{"x y":{"type":"api","key":"k"}}"#,
            r#"["x"]"#,
        ] {
            let value: serde_json::Value = serde_json::from_str(text).unwrap();
            assert!(check_document(&value, format).is_err(), "taken: {text}");
        }
    }

    /// A credential path never leaves the copy: relative, without `..`, `.`
    /// or an empty part, at most a directory and a file.
    ///
    /// Mutation proof: `credential_path_ok` that lets `..` pass turns the
    /// first refusal red.
    #[test]
    fn a_credential_path_that_leaves_the_copy_is_refused() {
        for path in [
            "../auth.json",
            "opencode/../../auth.json",
            "/etc/passwd",
            "",
            "./auth.json",
            "opencode//auth.json",
            "opencode/",
            "a/b/auth.json",
            "..",
        ] {
            assert!(!credential_path_ok(path), "taken: {path:?}");
        }
        for path in ["auth.json", ".credentials.json", "opencode/auth.json"] {
            assert!(credential_path_ok(path), "refused: {path:?}");
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
                recipe.install.is_none() || recipe.install_refused.is_none(),
                "{}: an install recipe and a reason against one",
                agent.id
            );
            if let Some(config_env) = recipe.config_env {
                assert!(
                    config_env
                        .chars()
                        .all(|c| c.is_ascii_uppercase() || c == '_'),
                    "{}: config_env",
                    agent.id
                );
                assert!(
                    !crate::bridge_env::RESERVED_ENV.contains(&config_env),
                    "{}: config_env is reserved",
                    agent.id
                );
                // The launcher and a maintenance run set `HOME` themselves.
                assert_ne!(config_env, "HOME", "{}", agent.id);
            }
            if let Some(file) = recipe.credential_file {
                // Inside the configuration copy and never the program.
                assert!(
                    credential_path_ok(file),
                    "{}: credential file {file}",
                    agent.id
                );
                assert!(
                    super::super::adopt::relative_inside(file),
                    "{}: credential file {file}",
                    agent.id
                );
                assert!(
                    plain_name(credential_name(file)),
                    "{}: credential file {file}",
                    agent.id
                );
                assert_ne!(file, recipe.program, "{}", agent.id);
                assert_ne!(
                    credential_name(file),
                    super::super::store::MANIFEST_FILE,
                    "{}",
                    agent.id
                );
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
                // The session starts program/<version>/<program>: the file
                // itself or the daemon's link to it, which carries its name.
                assert_eq!(
                    Path::new(install.program)
                        .file_name()
                        .and_then(std::ffi::OsStr::to_str),
                    Some(recipe.program),
                    "{}: the session starts program/<version>/<program>",
                    agent.id
                );
                for path in install.expected {
                    assert!(
                        super::super::adopt::relative_inside(path) && *path != install.program,
                        "{}: expected {path}",
                        agent.id
                    );
                }
                assert!(
                    [
                        install.signature.is_some(),
                        install.checksums.is_some(),
                        install.release_digest.is_some()
                    ]
                    .iter()
                    .filter(|set| **set)
                    .count()
                        <= 1,
                    "{}: one way of checking",
                    agent.id
                );
                if let Some(digest) = install.release_digest {
                    assert!(digest.version_pointer.starts_with('/'), "{}", agent.id);
                    assert!(digest.immutable_pointer.starts_with('/'), "{}", agent.id);
                    assert!(digest.archive.contains("{platform}"), "{}", agent.id);
                    assert!(digest.evidence.starts_with("https://"), "{}", agent.id);
                }
                if let Some(list) = install.checksums {
                    assert!(list.version_pointer.starts_with('/'), "{}", agent.id);
                    assert!(list.archive.contains("{platform}"), "{}", agent.id);
                    assert!(
                        super::super::adopt::relative_inside(list.list) && !list.list.contains('/'),
                        "{}",
                        agent.id
                    );
                    assert!(list.evidence.starts_with("https://"), "{}", agent.id);
                }
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
