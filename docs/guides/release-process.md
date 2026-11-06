# Release process

Applies to: Humanitl 0.1

For maintainers. A release is cut with one command, `tools/release.sh`, and with nothing else: no
hand-made tags, no tag pushed from a terminal, no tag ever moved. Users check what it produces with
[Verifying a download](verify-download.md).

## Versions

Versions follow SemVer. Before 1.0.0, a minor version (0.1.0 to 0.2.0) may change the
configuration, the protocol or the rule format; a patch version (0.1.0 to 0.1.1) never does. The
pre-releases 0.0.N came before 0.1.0.

`tools/release.sh` accepts exactly one next version: after the highest tag `v0.Y.Z` on `origin`,
only `0.Y.(Z+1)` or `0.(Y+1).0`. It refuses a gap, a repeat and 1.0.0. Releases before 1.0.0 are
not signed; they carry `SHA256SUMS` only. Signing is a precondition for 1.0.0, and until it exists
the scripts refuse that number.

The tag is the only source of the version. The repository keeps `0.0.0` in `daemon/Cargo.toml`,
`daemon/Cargo.lock` and `app/pubspec.yaml`; the release run writes the version into its own
checkout (`packaging/release/stamp-version.sh`), and `packaging/release/check-version.sh` fails the
run unless `humanitl --version`, `humanitld --version` and the app all report it. The package
formats take it from the same run. There is no `VERSION` file.

## Changelog

[`CHANGELOG.md`](https://github.com/nurkert/Humanitl/blob/main/CHANGELOG.md) follows Keep a
Changelog 1.1.0. User-visible changes go under `## [Unreleased]` in the categories `Added`,
`Changed`, `Deprecated`, `Removed`, `Fixed` and `Security`. The release notes of a version from
0.1.0 on start with its section (`packaging/release/changelog-section.sh`), followed by the install
instructions and the list of merge commits since the previous tag.

A section only counts with a date: `## [0.1.0] - 2026-11-20`. A draft such as
`## [0.1.0] - unreleased` stops `tools/release.sh` before anything is pushed, and the `guard` job
of the release run stops on it as well.

## Before the release

1. **Private vulnerability reporting is on.** In the security section of the repository settings,
   "Private vulnerability reporting" must be enabled; `SECURITY.md` sends reporters there. Open
   <https://github.com/nurkert/Humanitl/security/advisories/new> once to confirm the form appears.
2. **`main` is green.** The `ci.yml` run of the tip of `main` has passed, and the last
   `fuzz-nightly` run found nothing new.
3. **The changelog is final.** Move the entries from `## [Unreleased]` into the version's section,
   set today's date in its heading, leave an empty `## [Unreleased]` above it, and land that as an
   ordinary issue commit (`chore(release): 0.1.0`). Its CI run on `main` has to be green too.
4. **The documents agree.** The status table in `README.md` and the version tables in
   `docs/SECURITY.md` describe what is in the release.

## Cutting the release

```sh
tools/release.sh 0.1.0
```

The script, in order:

1. checks that the version is the next one after the highest `v0.*` tag on `origin`;
2. takes the tip of `origin/main` (a second argument may only name exactly that commit) and, from
   0.1.0 on, checks that `CHANGELOG.md` in that commit has a dated section for the version;
3. waits for the `ci.yml` run of that commit on `main` to be green with every job run
   (`tools/ci-gate.sh`, at most one hour, `HUMANITL_CI_TIMEOUT`). If that run gives no verdict in
   time, the script exits with 3 and has pushed nothing; run the same command again once the run
   finished. The path rules of `ci.yml` skip jobs a push does not touch (HUM-424), so the push run
   of a commit that changed only `docs/` is green without the Rust, e2e and macOS jobs (a change to
   `CHANGELOG.md` always runs everything). The script then asks for a full run: in the Actions tab, workflow `ci`, "Run workflow" on
   `main` with `full` checked (or `gh workflow run ci.yml --ref main -f full=true`). It waits for
   that run of the same commit and goes on when every job in it is green;
4. pushes the commit as branch `release-dry/0.1.0`, which starts the dry run of
   `.github/workflows/release.yml`: every build and check job with exactly this version, without
   the job `release`. It waits until every job is green;
5. only then sets the annotated tag `v0.1.0` ("Humanitl 0.1.0") on that same commit, pushes it and
   deletes the dry-run branch;
6. waits for the run of the tag and prints the address of the release.

The CI run takes 10 to 20 minutes and the release run about as long; run the script where it can
wait, for example in a second terminal.

Exit codes: 0 published; 1 refused or red; 2 wrong arguments; 3 no verdict in time (run it again,
or watch the run in the Actions tab).

## When something is red

- **Refused before anything was pushed** (wrong number, not the tip of `main`, no dated changelog
  section, `ci.yml` red): fix the cause on `main` and run the same command again.
- **A tag pushed by hand** instead of through the script: the `guard` job of its run looks for a
  green dry run of the same commit on `release-dry/<version>`, finds none and stops before anything
  is published. Do not move or delete that tag; release the next patch version with the script.
- **The dry run is red.** There is no tag. If it was a fluke, re-run the failed jobs of that dry run
  in the Actions tab and run the same command again; it evaluates the new attempt. If it was a
  defect, fix it on `main` and run the same command again; it checks the new tip.
- **The run of the tag is red although its dry run was green.** The script names that run. Open it
  and choose "Re-run failed jobs". The tag stays where it is.
- **A tag run failed for good.** Never move, delete or re-push a tag. Fix the defect on `main` and
  release the next patch version; the failed number stays without a release.

## After the release

1. Download the `.deb` and `SHA256SUMS` from the release page on another machine and run the check
   from [Verifying a download](verify-download.md).
2. Install the `.deb` in a fresh virtual machine, run `humanitl sandbox check` and hold one request.
3. Publish the AUR package `humanitl-bin` by hand from the files the run rendered
   (`packaging/aur/README.md`).
