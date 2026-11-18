#!/usr/bin/env python3
"""Der CI-Schritt von guard in release.yml gegen ein nachgestelltes Tor (HUM-424).

Der Schritt "The commit is on main and its CI run is green" wird aus
.github/workflows/release.yml gelesen und in einem Wegwerf-Repository mit
eigenem origin ausgefuehrt. Statt tools/ci-gate.sh antwortet ein Skript, das
seinen Aufruf mitschreibt und je Ereignis (push, workflow_dispatch) einen
Exit-Code aus einer Datei liefert. Geprueft wird vor allem, was nie geschehen
darf: ein strenger guard (Tag, release-dry/0.Y.Z), der einen Push-Lauf mit
ausgelassenen Jobs ohne vollen Lauf von Hand durchlaesst.

Aufruf: python3 tools/tests/release_guard_test.py [release.yml]
Mutationsbeweis: eine veraenderte Kopie von release.yml als Argument.
"""

import os
import re
import subprocess
import sys
import tempfile

try:
    import yaml
except ImportError:
    sys.exit("release_guard_test: PyYAML fehlt (apt install python3-yaml); ohne es prueft dieser Test nichts")

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
path = sys.argv[1] if len(sys.argv) > 1 else os.path.join(root, ".github", "workflows", "release.yml")
STEP = "The commit is on main and its CI run is green"

with open(path, encoding="utf-8") as f:
    wf = yaml.safe_load(f)
steps = [s for s in wf["jobs"]["guard"]["steps"] if s.get("name") == STEP]
if len(steps) != 1:
    sys.exit("release_guard_test: guard has no single step %r" % STEP)
step = steps[0]

GATE = """#!/usr/bin/env bash
echo "event=${HUMANITL_CI_EVENT:-} every=${HUMANITL_CI_EVERY_JOB:-} branch=${HUMANITL_CI_BRANCH:-} workflow=${HUMANITL_CI_WORKFLOW:-} skip=${HUMANITL_CI_MAY_SKIP:-} sha=$1 limit=$2" >> "$GATE_DIR/calls"
exit "$(cat "$GATE_DIR/rc-${HUMANITL_CI_EVENT:-none}" 2> /dev/null || echo 0)"
"""

git_env = dict(os.environ, GIT_AUTHOR_NAME="Test", GIT_AUTHOR_EMAIL="test@example.invalid",
               GIT_COMMITTER_NAME="Test", GIT_COMMITTER_EMAIL="test@example.invalid",
               GIT_CONFIG_GLOBAL="/dev/null", GIT_CONFIG_NOSYSTEM="1")


def git(cwd, *args):
    return subprocess.run(["git", "-C", cwd] + list(args), env=git_env, check=True,
                          capture_output=True, text=True).stdout.strip()


fails = 0


def case(name, strict, rcs, want_rc, want_events, on_main=True, want_text=None):
    global fails
    with tempfile.TemporaryDirectory(prefix="release-guard-test.") as tmp:
        origin = os.path.join(tmp, "origin.git")
        work = os.path.join(tmp, "work")
        subprocess.run(["git", "init", "-q", "--bare", "-b", "main", origin], env=git_env, check=True)
        subprocess.run(["git", "clone", "-q", origin, work], env=git_env, check=True, capture_output=True)
        os.makedirs(os.path.join(work, "tools"))
        gate = os.path.join(work, "tools", "ci-gate.sh")
        with open(gate, "w") as f:
            f.write(GATE)
        os.chmod(gate, 0o755)
        git(work, "add", "-A")
        git(work, "commit", "-q", "-m", "base")
        git(work, "push", "-q", "origin", "main")
        if not on_main:
            git(work, "commit", "-q", "--allow-empty", "-m", "not on main")
        sha = git(work, "rev-parse", "HEAD")
        for event, rc in rcs.items():
            with open(os.path.join(tmp, "rc-" + event), "w") as f:
                f.write(str(rc))
        env = dict(git_env, GATE_DIR=tmp)
        values = {"github.token": "token", "github.sha": sha, "steps.version.outputs.strict": strict,
                  "github.repository": "owner/name"}
        for key, value in (step.get("env") or {}).items():
            value = str(value)
            for expr in re.findall(r"\$\{\{\s*([^}]+?)\s*\}\}", value):
                value = value.replace("${{ %s }}" % expr, values[expr])
            env[key] = value
        r = subprocess.run(["bash", "--noprofile", "--norc", "-eo", "pipefail", "-c", step["run"]],
                           cwd=work, env=env, capture_output=True, text=True)
        calls = open(os.path.join(tmp, "calls")).read().splitlines() if os.path.exists(os.path.join(tmp, "calls")) else []
        events = [re.search(r"event=(\S*)", c).group(1) for c in calls]
        problems = []
        if r.returncode != want_rc:
            problems.append("exit %d, expected %d" % (r.returncode, want_rc))
        if events != want_events:
            problems.append("gate calls %s, expected %s" % (events, want_events))
        for c in calls:
            for need in ("every=1", "branch=main", "workflow=ci.yml", "skip= ", "sha=" + sha):
                if need not in c:
                    problems.append("gate call without %r: %s" % (need, c))
        if want_text and want_text not in r.stdout + r.stderr:
            problems.append("output without %r" % want_text)
        if problems:
            fails += 1
            print("FAIL %s: %s" % (name, "; ".join(problems)))
            print("    " + (r.stdout + r.stderr).strip().replace("\n", "\n    "))
        else:
            print("ok   %s" % name)


# Streng: Tag und release-dry/0.Y.Z.
case("strict_full_push_run", "true", {"push": 0}, 0, ["push"])
case("strict_partial_then_full_dispatch", "true", {"push": 4, "workflow_dispatch": 0}, 0,
     ["push", "workflow_dispatch"])
case("strict_partial_without_full_run", "true", {"push": 4, "workflow_dispatch": 3}, 1,
     ["push", "workflow_dispatch"], want_text="full=true")
case("strict_partial_dispatch_partial", "true", {"push": 4, "workflow_dispatch": 4}, 1,
     ["push", "workflow_dispatch"])
case("strict_partial_dispatch_red", "true", {"push": 4, "workflow_dispatch": 1}, 1,
     ["push", "workflow_dispatch"])
case("strict_red_push_never_asks_dispatch", "true", {"push": 1}, 1, ["push"])
case("strict_no_verdict", "true", {"push": 3}, 1, ["push"])
case("strict_gate_broken", "true", {"push": 2}, 1, ["push"])
case("strict_not_on_main", "true", {}, 1, [], on_main=False)
# Nicht streng: Probelauf eines Issue-Zweigs, eine Warnung genuegt.
case("lenient_partial_warns", "false", {"push": 4, "workflow_dispatch": 3}, 0,
     ["push", "workflow_dispatch"], want_text="::warning::")
case("lenient_not_on_main_skips_gate", "false", {}, 0, [], on_main=False, want_text="::warning::")


# HUM-531: Der Schalter des Probelaufs. Der Schritt "Version from the tag, ..."
# setzt macos_main=false nur fuer release-dry/hum-NNN-macos-fallback; ein Tag
# und release-dry/0.Y.Z kennen ihn nicht, und ein Tag mit der Endung bleibt
# ungueltig.
VERSION_STEP = [s for s in wf["jobs"]["guard"]["steps"] if s.get("id") == "version"]
if len(VERSION_STEP) != 1:
    sys.exit("release_guard_test: guard has no single step with id version")
VERSION_STEP = VERSION_STEP[0]


def switch_case(name, event, ref_type, ref_name, want_rc, want_outputs):
    global fails
    with tempfile.TemporaryDirectory(prefix="release-guard-test.") as tmp:
        out = os.path.join(tmp, "output")
        open(out, "w").close()
        env = dict(os.environ, EVENT=event, REF_TYPE=ref_type, REF_NAME=ref_name, INPUT_VERSION="",
                   SHA="0123456789abcdef0123456789abcdef01234567", LC_ALL="C", GITHUB_OUTPUT=out)
        r = subprocess.run(["bash", "--noprofile", "--norc", "-eo", "pipefail", "-c", VERSION_STEP["run"]],
                           cwd=tmp, env=env, capture_output=True, text=True)
        got = dict(line.split("=", 1) for line in open(out).read().splitlines() if "=" in line)
        problems = []
        if r.returncode != want_rc:
            problems.append("exit %d, expected %d" % (r.returncode, want_rc))
        for key, value in want_outputs.items():
            if got.get(key) != value:
                problems.append("%s=%r, expected %r" % (key, got.get(key), value))
        if problems:
            fails += 1
            print("FAIL %s: %s" % (name, "; ".join(problems)))
            print("    " + (r.stdout + r.stderr).strip().replace("\n", "\n    "))
        else:
            print("ok   %s" % name)


switch_case("switch_tag_runs_main_path", "push", "tag", "v0.4.1", 0,
            {"macos_main": "true", "publish": "true", "version": "0.4.1"})
switch_case("switch_release_dry_runs_main_path", "push", "branch", "release-dry/0.4.1", 0,
            {"macos_main": "true", "publish": "false"})
switch_case("switch_issue_dry_runs_main_path", "push", "branch", "release-dry/hum-531", 0,
            {"macos_main": "true", "version": "0.0.531"})
switch_case("switch_issue_dry_fallback_skips_main_path", "push", "branch", "release-dry/hum-531-macos-fallback", 0,
            {"macos_main": "false", "publish": "false", "version": "0.0.531"})
switch_case("switch_on_release_dry_version_refused", "push", "branch", "release-dry/0.4.1-macos-fallback", 1, {})
switch_case("switch_on_tag_refused", "push", "tag", "v0.4.1-macos-fallback", 1, {})
switch_case("switch_other_suffix_refused", "push", "branch", "release-dry/hum-531-macos", 1, {})

if fails:
    print("release_guard_test: %d Fall/Faelle rot" % fails, file=sys.stderr)
    sys.exit(1)
print("release_guard_test: alle Faelle gruen")
