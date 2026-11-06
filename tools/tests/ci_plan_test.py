#!/usr/bin/env python3
"""Die Pfadregel von ci.yml passt zu tools/ci-changes.sh (HUM-424).

Ein Job darf nur dann `skipped` enden, wenn tools/ci-gate.sh das als Pfadregel
erkennt: Der Job `changes` hat fuer ihn eine Ausgabe und einen Schritt
`skip <job>` mit der umgekehrten Bedingung, und der Job selbst laeuft genau
unter `needs.changes.outputs.<job> == 'true'`. Stimmen die drei Stellen nicht
ueberein, ueberspringt die CI einen Job, ohne dass das Tor es merkt, oder das
Tor faerbt jeden Lauf rot. Dieser Test liest ci.yml und die Jobliste, die
tools/ci-changes.sh ausgibt, und vergleicht.

Dazu der Sammeljob `ci-ok`, den die Branch Protection verlangt: Er haengt von
jedem anderen Job ab, laeuft immer, und sein Schritt wird hier gegen
nachgestellte `needs`-Kontexte ausgefuehrt. Gruen nur, wenn changes und lint
gruen sind und jeder andere Job gruen ist oder nach dem Plan uebersprungen.

Aufruf: python3 tools/tests/ci_plan_test.py [ci.yml]
Mutationsbeweis: eine veraenderte Kopie von ci.yml als Argument.
"""

import json
import os
import subprocess
import sys

try:
    import yaml
except ImportError:
    sys.exit("ci_plan_test: PyYAML fehlt (apt install python3-yaml); ohne es prueft dieser Test nichts")

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
path = sys.argv[1] if len(sys.argv) > 1 else os.path.join(root, ".github", "workflows", "ci.yml")
fails = []
AGGREGATE = "ci-ok"


def fail(msg):
    fails.append(msg)


with open(path, encoding="utf-8") as f:
    wf = yaml.safe_load(f)
jobs = wf.get("jobs") or {}

out = subprocess.run(
    [os.path.join(root, "tools", "ci-changes.sh"), "--full", "HEAD"],
    cwd=root, capture_output=True, text=True, check=True,
).stdout
planned = []
for line in out.splitlines():
    key, _, value = line.partition("=")
    if key != "full" and not key.startswith("area-"):
        planned.append(key)
        if value != "true":
            fail("ci-changes.sh --full says %s=%s" % (key, value))

# YAML 1.1 liest den Schluessel `on` als True.
triggers = wf.get("on", wf.get(True)) or {}
full_input = ((triggers.get("workflow_dispatch") or {}).get("inputs") or {}).get("full") or {}
if full_input.get("type") != "boolean" or full_input.get("default") is not True:
    fail("workflow_dispatch needs an input full (boolean, default true)")

changes = jobs.get("changes")
if not changes:
    fail("no job changes")
    changes = {}
if changes.get("name") != "changes" or "if" in changes or "needs" in changes:
    fail("job changes must be named changes and run unconditionally")

outputs = changes.get("outputs") or {}
steps = changes.get("steps") or []
areas = [s for s in steps if s.get("id") == "areas"]
if len(areas) != 1 or "tools/ci-changes.sh" not in (areas[0].get("run") or ""):
    fail("job changes needs one step with id areas that runs tools/ci-changes.sh")
skip_steps = {}
for s in steps:
    name = s.get("name") or ""
    if name.startswith("skip "):
        if name in skip_steps:
            fail("step %r twice in changes" % name)
        skip_steps[name] = s

for job_id, job in jobs.items():
    if job.get("name", job_id) != job_id:
        fail("job %s has name %r; tools/ci-gate.sh matches by name" % (job_id, job.get("name")))
    if job_id in ("changes", AGGREGATE):
        continue
    needs = job.get("needs") or []
    needs = [needs] if isinstance(needs, str) else needs
    if "changes" not in needs:
        if "if" in job:
            fail("job %s has an if: but no needs: changes; the gate would see an unexplained skip" % job_id)
        if job_id in planned:
            fail("job %s is planned by ci-changes.sh but has no needs: changes" % job_id)
        continue
    if job_id not in planned:
        fail("job %s needs changes but ci-changes.sh has no line for it" % job_id)
    want_if = "needs.changes.outputs.%s == 'true'" % job_id
    if job.get("if") != want_if:
        fail("job %s: if must be %r, is %r" % (job_id, want_if, job.get("if")))
    want_out = "${{ steps.areas.outputs.%s }}" % job_id
    if outputs.get(job_id) != want_out:
        fail("changes.outputs.%s must be %r, is %r" % (job_id, want_out, outputs.get(job_id)))
    step = skip_steps.pop("skip " + job_id, None)
    want_step_if = "steps.areas.outputs.%s != 'true'" % job_id
    if step is None:
        fail("changes has no step 'skip %s'" % job_id)
    elif step.get("if") != want_step_if:
        fail("step 'skip %s': if must be %r, is %r" % (job_id, want_step_if, step.get("if")))

for job_id in planned:
    if job_id not in jobs:
        fail("ci-changes.sh plans %s, ci.yml has no such job" % job_id)
for name in skip_steps:
    fail("changes has %r, but no job uses it" % name)
for key in outputs:
    if key not in planned:
        fail("changes.outputs.%s is not planned by ci-changes.sh" % key)

# Der Sammeljob: immer, nach allen anderen, mit der Regel des Tors.
agg = jobs.get(AGGREGATE)
if not agg:
    fail("no job %s for branch protection" % AGGREGATE)
else:
    if agg.get("if") != "always()":
        fail("job %s must run with if: always()" % AGGREGATE)
    agg_needs = agg.get("needs") or []
    agg_needs = [agg_needs] if isinstance(agg_needs, str) else agg_needs
    others = set(jobs) - {AGGREGATE}
    if set(agg_needs) != others or len(agg_needs) != len(others):
        fail("job %s must need every other job exactly once; missing %s, extra %s"
             % (AGGREGATE, sorted(others - set(agg_needs)), sorted(set(agg_needs) - others)))
    agg_steps = agg.get("steps") or []
    if len(agg_steps) != 1 or "run" not in agg_steps[0]:
        fail("job %s needs exactly one run step" % AGGREGATE)
    else:
        heavy = [j for j in agg_needs if j not in ("changes", "lint")]

        def needs_ctx(changes="success", lint="success", plan=None, results=None):
            plan = {j: "false" for j in heavy} if plan is None else plan
            results = results or {}
            ctx = {"changes": {"result": changes, "outputs": plan},
                   "lint": {"result": lint, "outputs": {}}}
            for j in heavy:
                default = "success" if plan.get(j) == "true" else "skipped"
                ctx[j] = {"result": results.get(j, default), "outputs": {}}
            return ctx

        first = heavy[0] if heavy else "none"
        cases = [
            ("website only, all heavy skipped by plan", 0, needs_ctx()),
            ("every job ran", 0, needs_ctx(plan={j: "true" for j in heavy})),
            ("changes failed before its outputs", 1, needs_ctx(changes="failure", plan={})),
            # Ein spaeterer Schritt von changes scheitert, die Ausgaben stehen schon.
            ("changes failed after its outputs", 1, needs_ctx(changes="failure")),
            ("lint failed", 1, needs_ctx(lint="failure")),
            ("lint skipped", 1, needs_ctx(lint="skipped")),
            ("skipped although planned", 1,
             needs_ctx(plan=dict({j: "false" for j in heavy}, **{first: "true"}), results={first: "skipped"})),
            ("heavy job failed", 1, needs_ctx(plan={j: "true" for j in heavy}, results={first: "failure"})),
            ("heavy job cancelled", 1, needs_ctx(plan={j: "true" for j in heavy}, results={first: "cancelled"})),
        ]
        for name, want, ctx in cases:
            r = subprocess.run(["bash", "-e", "-c", agg_steps[0]["run"]],
                               env=dict(os.environ, NEEDS=json.dumps(ctx)), capture_output=True, text=True)
            if r.returncode != want:
                fail("job %s, case %r: exit %d, expected %d (%s)"
                     % (AGGREGATE, name, r.returncode, want, (r.stdout + r.stderr).strip()[-200:]))

if fails:
    for msg in fails:
        print("FAIL " + msg)
    print("ci_plan_test: %d Befund(e) in %s" % (len(fails), path), file=sys.stderr)
    sys.exit(1)
print("ci_plan_test: %d Jobs mit Pfadregel, ci.yml und tools/ci-changes.sh passen zusammen" % len(planned))
