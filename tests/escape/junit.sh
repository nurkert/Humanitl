#!/usr/bin/env sh
# Turn the RESULT lines of the escape harness into JUnit XML (HUM-006).
#
#   sh tests/escape/junit.sh target/escape/results.txt > target/escape/escape.xml
#
# One <testsuite name="escape"> with one <testcase classname="esc-N"> per probe.
# A failed probe carries <failure>, a probe the sprint cannot decide carries
# <skipped>, and a suite whose sandbox never started carries <error>. The
# evidence of a passing probe is kept in <system-out>, because "it passed" is
# not an answer to "how do you know".
#
# POSIX awk only: this runs on the host, before anything else is built.
set -eu

if [ "$#" -lt 1 ]; then
    echo "usage: junit.sh RESULTS-FILE" >&2
    exit 2
fi
if [ ! -f "$1" ]; then
    echo "junit.sh: no such results file: $1" >&2
    exit 2
fi

awk '
function esc(s) {
    gsub(/&/, "\\&amp;", s)
    gsub(/</, "\\&lt;", s)
    gsub(/>/, "\\&gt;", s)
    gsub(/"/, "\\&quot;", s)
    gsub(/\047/, "\\&apos;", s)
    gsub(/[[:cntrl:]]/, "?", s)
    return s
}
$1 == "RESULT" {
    n++
    suite[n] = $2
    name[n] = $3
    status[n] = $4
    d = ""
    for (i = 5; i <= NF; i++) d = d (i > 5 ? " " : "") $i
    detail[n] = d
    if ($4 == "fail") failures++
    else if ($4 == "skip") skipped++
    else if ($4 == "error") errors++
}
END {
    printf "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n"
    printf "<testsuite name=\"escape\" tests=\"%d\" failures=\"%d\" errors=\"%d\" skipped=\"%d\">\n",
        n, failures + 0, errors + 0, skipped + 0
    for (i = 1; i <= n; i++) {
        printf "  <testcase classname=\"%s\" name=\"%s\">\n", esc(suite[i]), esc(name[i])
        if (status[i] == "fail")
            printf "    <failure message=\"%s\"/>\n", esc(detail[i])
        else if (status[i] == "error")
            printf "    <error message=\"%s\"/>\n", esc(detail[i])
        else if (status[i] == "skip")
            printf "    <skipped message=\"%s\"/>\n", esc(detail[i])
        else
            printf "    <system-out>%s</system-out>\n", esc(detail[i])
        printf "  </testcase>\n"
    }
    printf "</testsuite>\n"
}
' "$1"
