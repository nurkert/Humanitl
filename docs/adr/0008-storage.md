# ADR-0008 · Storage: SQLite, blob store, audit chain, separate pseudonym mapping

Applies to: Humanitl 0.1

Status: Accepted
Date: 2026-09-02

## Context

Humanitl records what goes through the proxy: metadata of every flow, headers,
bodies of request and response, findings, decisions, rules. This data is at
the same time the means of evidence for the GDPR obligations of the audience
("what went to whom?") and the most sensitive data collection on the machine:
by construction it contains everything the agent has ever sent, including
customer data.

Three separate requirements follow from this. First, a queryable database for
metadata (history screen with filter, sorting and cursor, server-side).
Second, a storage place for bodies that does not bloat the database file with
a 40 MB upload. Third, evidence that the recording was not altered
afterwards — and with it an honest statement about what this evidence is
worth.

Constraints: a desktop program without a server. No database service the user
has to install. One user, one process, concurrent writes from the proxy and
reads from the UI.

## Decision

**Metadata in SQLite** in WAL mode, with versioned migrations
(`refinery`, embedded from `daemon/crates/recorder/migrations/`), at
`$XDG_DATA_HOME/humanitl/humanitl.db`. A dedicated writer thread writes,
reads run on a separate read-only connection; filtering, sorting and cursor
paging of the history are executed server-side in SQL. Stored are sessions,
flows, messages (headers and body per direction), findings and a snapshot of
deleted rules, so that the history can still explain a decision even when the
rule no longer stands in `rules.yaml` — `rules.yaml` remains the source of
truth for rules.

The binding schema does not stand in this file but in the migrations:
`V1__init.sql` and `V2__rules_snapshot.sql` in HUM-026
(`backlog/sprint-2.md`) as well as the pseudonym migration in HUM-048
(`backlog/sprint-4.md`). The short version in `BACKLOG.md` 3.4 is an early
sketch; where it deviates from the migrations, the migrations apply.

**Bodies** up to 256 KB (`recorder.inline_max_bytes`) lie inline as a BLOB in
`messages`. Larger bodies go content-addressed into the blob store at
`$XDG_DATA_HOME/humanitl/blobs/<hex[0..2]>/<sha256-hex>`; the database holds
only the reference.

**Audit log** as append-only JSONL at
`$XDG_DATA_HOME/humanitl/audit/audit.jsonl`, with a hash chain. Every entry
carries `seq`, `ts`, `prev_hash` and `hash` over the canonical JSON form of
the entry, plus an HMAC with an installation key from the system keyring.
The head hash is anchored periodically: display in the UI, storage in a second
location and additionally via `logger` in the systemd journal.

**What the chain proves is spelled out, not hinted at.** It protects against
subsequent editing by third parties with file access. It does **not** protect
against an attacker who runs as the same user and has both the key and the
file — they can rewrite the chain. The protection against this case would be
an external anchor (timestamping service, foreign machine) and is explicitly
post-MVP.

**Pseudonymisation mapping separate.** The mapping from pseudonym to original
value lives in a table of its own, encrypted with a keyring key, exclusively
on the host side. It never goes into the sandbox and never into a request.
Secrets are stored only as a hash plus a display prefix, never in plain text.
An export is encrypted.

## Rationale

SQLite is the obvious and right choice for this case: embedded, no service,
one file, transactional, and in WAL mode the writing recorder and the reading
UI run side by side without blocking each other. Migrations are there from day
one, because a tool that stores evidence must not lose its old data on update.

The limit at 256 KB separates two access patterns. Small bodies (JSON
payloads, LLM prompts) are wanted together with the metadata in one query;
large bodies are wanted neither in the database file nor in memory when
listing the history. Content addressing via SHA-256 deduplicates as a side
effect: an agent that uploads the same file ten times occupies the space once.
The two-level path (`<hex[0..2]>/<sha256>`) keeps the directories small enough
for any file system.

JSONL for the audit log instead of another table, because append-only is
exactly the property a hash chain needs, and because a text file stays
readable without Humanitl and verifiable with standard tools. The canonical
JSON form is the precondition for the hash being reproducible. The HMAC
prevents someone without the key from recomputing a plausible chain; anchoring
the head hash in several places makes cutting off the end (truncation)
conspicuous.

Honesty about the limits of the chain is part of the decision, not an
addition. A security promise that claims more than it keeps is worse than
none, because the user adjusts their behaviour to it. Principle 3
("honest about limits") applies here literally.

The pseudonym mapping is the only data set that allows restoring the original
data from pseudonymised recordings. It is therefore encrypted, kept separate
and never visible in the sandbox — otherwise pseudonymisation would be a
ritual instead of a measure.

## Rejected alternatives

- **PostgreSQL.** Would need a service the user installs, configures and
  updates. For a single-seat application an unjustified hurdle.
  Earmarked as a possible second `FlowStore` adapter for a team mode.
- **Everything in files (one directory per flow).** Simple to write, but
  every filter in the history screen would be a scan over all flows.
  Server-side filtering, sorting and cursor paging is a requirement, not an
  extra.
- **Bodies always inline in the database.** A database file that grows with
  40 MB uploads becomes a problem for `VACUUM` and for backups, and a
  `SELECT *` in the UI pulls hundreds of megabytes.
- **Bodies always in the blob store.** Two IO operations for every small
  JSON object and a lot of tiny files. The threshold at 256 KB takes the useful
  part of both paths.
- **Audit log in the same SQLite database.** Convenient, but a database is
  editable; append-only is a convention for it, not a property. And an auditor
  should be able to recompute the chain without Humanitl.
- **Signature instead of HMAC (asymmetric).** Would be stronger if the private
  key lived elsewhere — which is exactly not the case in the MVP. As long as
  the key lives on the same machine, the asymmetry buys nothing except
  complexity. With an external anchor that changes.
- **Blockchain anchor or external timestamping service in the MVP.** The only
  real protection against the attacker with key and file, but it presupposes a
  network connection and a service. Post-MVP, and until then clearly named as
  what is missing.
- **Pseudonym mapping unencrypted next to the flows.** Would have devalued
  pseudonymisation: whoever has access to the recording would also have the
  resolution.

## Consequences

- The port `FlowStore` (adapter: SQLite) and the port `AuditSink` (adapter:
  JSONL hash chain) are separate. A later remote anchor is a second
  `AuditSink`, not a rebuild (ADR-0015).
- Migrations are mandatory from HUM-026 onwards, even for the first version of
  the schema.
- `recorder.retention_days` controls deletion; deletion is documented,
  because it creates a gap in the chain that the verification must know about.
- The blob store needs a clean-up routine that removes orphaned blobs as soon
  as the associated flows are deleted.
- `humanitl audit verify` checks the chain and is available without the UI as
  well; the tamper tests from `BACKLOG.md` 4.5 (delete an entry, cut the end)
  are part of acceptance. The truncation case stays red until head anchoring
  exists — this too is documented as such.
- M1 does not yet record persistently. The statement "everything is recorded"
  applies only from HUM-026 onwards (`backlog/CONVENTIONS.md` 4.10) and is not
  claimed until then.

## Affected issues

`HUM-026` (recorder: SQLite, WAL, migrations, blob store, `ListFlows`),
`HUM-050` (audit hash chain and `verify`), `HUM-051` (audit screen, export,
retention), `HUM-048` (pseudonym mapping, encrypted, keyring),
`HUM-032` (history screen and export).
