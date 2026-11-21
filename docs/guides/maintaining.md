# Maintaining the repository

Applies to: Humanitl 0.1

For maintainers. How issues, labels and pull requests work on GitHub, and the repository settings
that auto-merge depends on. The working rules for each issue are in `CLAUDE.md` and
`CONTRIBUTING.md`; this page covers the GitHub side.

## Issues

GitHub issues hold the specification of every open issue from sprint 8 on. Older sprints stay in
`backlog/sprint-N.md` as history, and an issue without a GitHub number still lands with
`tools/commit-issue.sh --ci`.

A new issue can only be opened through a form under `.github/ISSUE_TEMPLATE/`; blank issues are
off, and the contact link sends vulnerabilities to private reporting (`SECURITY.md`):

| Form | For |
|---|---|
| `bug.yml` | something that does not work as documented; asks for `humanitl doctor`, version and OS |
| `feature.yml` | a problem, the proposed behaviour and the affected area |
| `docs.yml` | a page that is wrong, missing or unclear |
| `task.yml` | the maintainer specification: Context, Specification, Tests with mutation proofs, Pitfalls, Acceptance criteria, size and dependencies |

Issues and pull requests are written in English and never name private references.

`tools/issue-spec.sh N -o spec.md` writes title, labels and body of issue N to a Markdown file.
Agents and reviewers that run without network, Codex among them, read the specification from that
file. The file is a snapshot and does not belong in the repository.

## Labels

`.github/labels.yml` is the list of labels: name, colour, description and former names
(`aliases`). The workflow `.github/workflows/labels.yml` applies it on every push to `main` that
changes it, and by hand under Actions. It runs `tools/sync_labels.py`, which creates missing labels,
updates colour and description, renames a label that still carries a former name, and never deletes
a label that is missing from the file; Dependabot and GitHub add their own.

| Group | Labels |
|---|---|
| `type:` | `feature`, `bug`, `docs`, `chore`, `security` |
| `area:` | `daemon`, `proxy`, `sandbox`, `cli`, `app`, `packaging`, `ci`, `docs`, `website` |
| `size:` | `S`, `M`, `L` |
| `status:` | `blocked`, `needs-decision`, `needs-info`, `owner-walkthrough` |
| `sprint:` | one per sprint, `sprint:8` first |

The type is a label because GitHub issue types need an organisation. To add or change a label, edit
the file; `python3 tools/sync_labels.py --check` validates it, `--dry-run` prints the `gh` calls
without making them.

## Landing a change: one issue, one pull request

```sh
tools/issue-spec.sh 42 -o /tmp/issue-42.md
# implement, run the targeted checks, get both reviews
tools/commit-issue.sh --pr 42 short-slug "feat(area): subject" path/one path/two <<'EOF'
What was built, in two to four sentences.

Review: Codex 2 findings (1 taken, 1 not taken: reason), Antigravity 1 finding (taken).
EOF
```

`tools/commit-issue.sh --pr` builds the one commit in its own index on top of `origin/main`, exactly
as `--ci` does, and refuses when one of the paths changed on `main` since the working tree branched.
Before anything is built it refuses a message without the `Review:` paragraph and a message that
closes an issue itself (`Fixes #7` and the like; one issue per pull request). Before anything is
pushed it checks with `gh api repos/{owner}/{repo}/rules/branches/main` that a rule on `main`
requires the check `ci-ok` on an up-to-date branch, and stops if none does.

It pushes the branch `issue-42-short-slug` and opens a pull request: the title is the commit subject,
the body is `.github/pull_request_template.md` with `Closes #42`, the commit message under "What was
built" and the `Review:` paragraph under "Review". The gate is the `pull_request` run of `ci.yml`
for exactly the pushed head; `issue-*` branches get no push run, so every job runs once.
`tools/ci-gate.sh` waits for that run (`HUMANITL_CI_EVENT=pull_request`). Only when every gate of
that commit is green does the script request auto-merge with a merge commit for exactly that commit
(`gh pr merge --auto --merge --match-head-commit <sha>`). Then it waits until GitHub has merged.
GitHub writes the merge commit and closes the issue.

- **Red CI**: the pull request stays open without auto-merge. Fix, amend the commit on the branch,
  switch the working tree to another branch, then `tools/commit-issue.sh --land issue-42-short-slug`.
  It reuses the open pull request and rewrites its title and body from the current commit.
- **Auto-merge already on**: before every push to the branch of a pull request whose auto-merge is
  on, the script turns it off. If that fails, it stops before the push, so GitHub can never merge a
  head whose gates have not run.
- **`main` moved**: the ruleset requires an up-to-date branch, so GitHub does not merge. The script
  turns auto-merge off, rebases the one commit onto the new `main`, pushes, waits for the new run
  and requests auto-merge for the new commit.
- **Green, but no merge** within `HUMANITL_PR_MERGE_TIMEOUT` seconds (default 1800): the script
  says so and stops. It never merges around auto-merge; look at the pull request (required check,
  ruleset, the auto-merge setting) and run `--land` again.
- **Release paths**: a commit that touches a path from `tools/release-dry-paths.txt` is also pushed
  as `release-dry/hum-42` (`release.yml` knows only that pattern; the number is the GitHub issue).
  Auto-merge waits for that dry run as well, because the ruleset does not know it. If
  `release-dry/hum-42` already exists on `origin` at a commit that is not this issue's (an old
  `HUM-042`), the script stops instead of overwriting it.

The required check `ci-ok` has `if: always()` and no condition on the event, so it is never skipped
on a pull request; a skipped check would count as passed for the ruleset. `ci.yml` says so where it
defines the triggers and the job.

`gh` must be logged in (`gh auth status`). `HUMANITL_GH` replaces the command; the test
`tools/tests/commit_issue_pr_test.sh` uses that for a stub.

## Repository settings for auto-merge

Auto-merge without a required check merges at once. The settings below must exist before the first
`--pr`. They are made with `gh api` by a repository admin; the commands are the record.

Allow auto-merge and merge commits, and delete the head branch after a merge:

```sh
gh api -X PATCH repos/nurkert/Humanitl \
  -F allow_auto_merge=true \
  -F allow_merge_commit=true \
  -F delete_branch_on_merge=true
```

A ruleset on the default branch requires the check `ci-ok` from GitHub Actions (app id 15368) on an
up-to-date branch. It has no review requirement. The repository admin role (actor id 5) may bypass
it, so the admin can still push directly: `tools/commit-issue.sh --ci` merges into `main` itself,
and `tools/release.sh` pushes its dry-run branch and the tag.

```sh
gh api -X POST repos/nurkert/Humanitl/rulesets --input - <<'EOF'
{
  "name": "main: ci-ok",
  "target": "branch",
  "enforcement": "active",
  "conditions": { "ref_name": { "include": ["~DEFAULT_BRANCH"], "exclude": [] } },
  "bypass_actors": [
    { "actor_id": 5, "actor_type": "RepositoryRole", "bypass_mode": "always" }
  ],
  "rules": [
    {
      "type": "required_status_checks",
      "parameters": {
        "strict_required_status_checks_policy": true,
        "do_not_enforce_on_create": false,
        "required_status_checks": [ { "context": "ci-ok", "integration_id": 15368 } ]
      }
    }
  ]
}
EOF
```

Check both:

```sh
gh api repos/nurkert/Humanitl --jq '{allow_auto_merge, allow_merge_commit, delete_branch_on_merge}'
gh api repos/nurkert/Humanitl/rulesets --jq '.[] | {id, name, enforcement}'
gh api repos/nurkert/Humanitl/rulesets/<id> --jq '.rules, .bypass_actors'
```

To change the ruleset later, send the same JSON with `gh api -X PUT repos/nurkert/Humanitl/rulesets/<id> --input -`.
