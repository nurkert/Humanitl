# ADR-0011 · One configuration source, three visibility tiers

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-09-02

## Context

Humanitl is configurable down into the sandbox: LLM endpoint,
agent adapter, sandbox profile, mounts, environment variables, timeouts, caps,
detectors, catalogue sources, rules. At the same time the standard path should need three
decisions: LLM server, project folder, start (principle 8, "have to do little,
be able to do a lot").

These two requirements collide in two ways. First, the classic
double maintenance looms: a value exists in a TOML file, as a CLI flag and
as a checkbox in the settings dialogue, with three different names, three
descriptions and a default that differs in two places. Second, overload
looms: a settings dialogue with eighty fields helps nobody,
even if every single field is necessary.

On top of that comes a third requirement from principle 10: the UI must never be the only
way. What can be set in the UI must also be settable in a file and on the
command line — otherwise headless operation (ADR-0013) is a
subset instead of an alternative.

## Decision

**There is exactly one source for every setting: a Rust type.** All
settings are `serde` types with a `schemars` derivation in the crate
`humanitl-config`, with `Config` as the root. From it arise — generated, not
maintained in parallel:

- the TOML schema and its validation,
- the CLI flags (`clap`, with the same names),
- the settings interface in the UI (from the JSON schema, with description,
  default and reset),
- the documentation.

**No setting exists only in the UI.**

**Three visibility tiers.** Every field carries an attribute
`#[humanitl(tier = "basic" | "advanced" | "expert")]`:

| Tier | Place | Behaviour |
|---|---|---|
| `basic` | setup checklist | the three decisions of the standard path |
| `advanced` | settings screen | visible by default |
| `expert` | settings screen | collapsed, with a warning where security-relevant |

Search always covers all tiers; nothing is unfindable, only
collapsed by default.

**Precedence, from low to high:** built-in defaults → global
`config.toml` → global profile → project profile → environment variables
`HUMANITL_*` (path separator `__`, for example `HUMANITL_HOLD__TIMEOUT_SECS`) →
CLI flag. Every resolution records the **origin** (`Origin`) per field, so that
the UI can show "this value comes from the project profile".

**Profiles bundle** sandbox profile, rule set, agent adapter, LLM endpoint,
timeout, mounts and environment into one file. They live globally under
`$XDG_CONFIG_HOME/humanitl/profiles/` or per project under
`.humanitl/profile.toml`; the project wins. `humanitl run --profile llm-only`
and the UI use the same profile files.

**Rules have two storage places** (ADR-0007): saved in `rules.yaml`
(survives the restart) and temporary with `expires: session` (only in the daemon's
memory). The UI shows both in separate tabs; temporary rules have a
remaining lifetime and an action "make permanent".

## Rationale

Generating from one type instead of maintaining three representations is the
only way that does not drift apart over time. A new field appears
automatically in the schema, in the CLI, in the UI and in the documentation — and if the
description is missing, that shows up during generation, not with the user. The Definition
of Done demands tier, description and default for every new field; that makes
completeness checkable instead of hoped for.

The three tiers resolve the overload without hiding anything. Progressive
disclosure here means: the standard path shows three fields, the
settings screen shows the useful ones, and the dangerous ones are one
click away and carry a warning. Because search covers all
tiers, nobody has to guess whether a setting exists.

The origin per field is the answer to the most common confusion with
layered configuration: "But I did set that, why does it not
apply?" When the UI says that the value comes from the project profile and overrides the
global one, the question answers itself.

Profiles are the unit in which people actually think: not "timeout 300
seconds and mount `/work` as `ro`", but "my setup for client projects".
That a profile is a file makes it shareable, versionable and, in case of an
error, readable.

The separation of saved and temporary rules maps the two real
kinds of trust: "this should always apply" and "this applies right now because I
am working here". Without this separation a `rules.yaml` collects, within two
weeks, thirty entries that nobody can keep track of any more — and each of them is a
permanently open door.

## Rejected alternatives

- **Configuration only in the UI, file as an implementation detail.** The most convenient
  way for UI development and a break with principle 10: headless operation,
  scripting and versioning of the configuration would be ruled out.
- **Configuration only in files, no settings screen.** Unreasonable for the
  target audience (no security experts, no forced terminal).
- **Hand-written CLI flags beside a hand-written UI.** The normal case in
  many projects and the reason why defaults drift apart there. A
  generator costs effort once and nothing afterwards.
- **JSON or YAML instead of TOML for the main configuration.** JSON has no
  comments, which is disqualifying for a hand-edited file. YAML has
  known pitfalls with implicit types. TOML is the clearest choice for flat to medium
  nesting. Rules stay YAML because they are an ordered
  list with nesting (ADR-0007).
- **Only two tiers (simple/advanced).** Would have mixed the security-relevant
  settings with the merely rarely used ones. The third tier
  exists so that a warning has a meaning.
- **A flat settings list without profiles.** Would have reduced the most common
  use case ("different project, different rules") to manually changing
  six values.
- **Persistent rules only.** Would be simpler, but every ad hoc approval would be
  permanent. That is exactly the creeping loss of control that the
  tool is meant to prevent.

## Consequences

- `humanitl-config` belongs to the core: `serde` and `schemars`, no IO in the type
  itself, table-driven tests for the precedence.
- Every field needs `#[schemars(description = "…")]` and a tier attribute. Without
  both, a field is incomplete and is objected to in review.
- `humanitl config schema` prints the JSON schema. The settings screen
  is generated from it; it contains no field list in the Dart code.
- The group `limits` is the home of all caps and timeouts
  (`backlog/CONVENTIONS.md` 4.4); older keys such as `hold.body_cap_bytes`
  are kept as aliases so that existing files keep working.
- `humanitl config set` validates against the schema and on an error returns
  a `Diagnostic` instead of an error message (ADR-0012).
- Live reload of the configuration file is possible because resolution is a pure
  function over the layers.
- The generated interface is good enough for eighty fields, but not for
  special cases. Where a field needs its own interaction (search for an LLM server,
  choose the project folder), there is a hand-built widget for that field — not
  for the whole screen.

## Affected issues

`HUM-062` (config crate with schema, tiers and precedence tests), `HUM-066`
(profiles, global and per project), `HUM-069` (settings screen with
progressive disclosure), `HUM-070` (`humanitl config get|set|schema|edit`),
`HUM-033` (rules screen with tabs for saved and temporary rules).
