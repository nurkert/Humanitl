#!/bin/sh
# ESC-3 — egress: nothing leaves except through the proxy, and there it waits.
# Runs INSIDE the sandbox: humanitl sandbox run --profile test -- /tests/escape/esc-3-egress.sh
#
# RED IS THE CORRECT STATE UNTIL SPRINT 1 CLOSES, and here in two different
# ways, which is the whole reason this suite exists so early:
#
#   * The direct probes (no proxy, no DNS, no UDP) are already green today. They
#     have to be: the network namespace has nothing but lo, so the absence of a
#     route does the work long before seccomp is written. A red line here would
#     mean the sandbox is not a sandbox.
#   * The proxy probes are red until HUM-013/HUM-015 put a proxy behind
#     /run/humanitl/proxy.sock. They currently fail with "connection refused",
#     not with the 403 body, and that difference is visible in the evidence.
#
# The claim under test (SECURITY.md, ADR-006, THREAT-MODEL K-10, K-13): the only
# route out is the proxy; everything that goes through it is decided by a human;
# and no name is resolved before that decision.
#
# run.sh sets HUMANITL_HOLD__TIMEOUT_SECS=2 for this suite, so a held request
# turns into a timeout-block quickly and curl gets to see the 403 body instead
# of sitting in the queue for five minutes.

set -u
ESC_LIB="${ESC_LIB:-$(dirname "$0")/lib.sh}"
# shellcheck source=tests/escape/lib.sh
. "$ESC_LIB"

esc_begin esc-3

# --- no route out without the proxy -------------------------------------------
#
# --noproxy '*' is not optional: the profile puts HTTP_PROXY into the
# environment, so a plain curl would measure the proxy instead of the absence of
# a route, and would report a leak as a pass.
probe direct_http  curl -s --max-time 3 --noproxy '*' http://example.com/
probe direct_https curl -s --max-time 3 --noproxy '*' https://example.com/
probe direct_ip    curl -s --max-time 3 --noproxy '*' http://93.184.216.34/
probe dns_lookup   sh -c 'getent hosts example.com'
probe dns_udp_53   python3 -c '
import socket, sys
handle = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
handle.settimeout(2)
handle.sendto(b"\x00\x00", ("1.1.1.1", 53))
print("a UDP datagram left the sandbox")
sys.exit(0)
'
probe quic_udp python3 -c '
import socket, sys
handle = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
handle.settimeout(2)
handle.sendto(b"x", ("1.1.1.1", 443))
print("a UDP datagram left the sandbox")
sys.exit(0)
'

# --- through the proxy, everything is decided ---------------------------------
#
# Not `probe`: reaching the proxy is allowed and expected. What must hold is the
# ANSWER — the 403 body from CONVENTIONS.md 3.5, which the proxy sends for every
# BlockReason. A connection error reads as fail here, and that is the state
# until HUM-013 and HUM-015 land.
expect_output via_proxy_held \
    'Blocked by Humanitl' curl -s --max-time 10 http://blocked.example/
expect_output via_proxy_private_held \
    'Blocked by Humanitl' curl -s --max-time 10 http://10.0.0.1/
expect_output via_proxy_metadata_held \
    'Blocked by Humanitl' curl -s --max-time 10 http://169.254.169.254/
expect_output via_proxy_idn_held \
    'Blocked by Humanitl' curl -s --max-time 10 http://xn--80ak6aa92e.com/
expect_output via_proxy_reason_line \
    '^reason: ' curl -s --max-time 10 http://blocked.example/
expect_output host_mismatch_blocked \
    'authority_mismatch' curl -sk --max-time 10 -H 'Host: evil.io' https://github.com/

# --- the meta endpoint, the one channel back to the human ---------------------
#
# ADR-014 and HUM-073: the proxy answers the reserved host `humanitl.internal`
# itself, without DNS and without an upstream. From inside the sandbox this is
# the only way for the agent to read the rules or to ask the human for
# something, and it has to work in a namespace that has no name service at all.
#
# --noproxy is deliberately NOT passed here: the request is meant to go to the
# proxy, in absolute form, so curl never resolves the name either.
expect_output meta_status \
    '^humanitl session=' curl -s --max-time 10 http://humanitl.internal/
expect_output meta_status_lists_rules \
    'rules \(first match wins\):' curl -s --max-time 10 http://humanitl.internal/
expect_output meta_ask_queued \
    '^queued$' curl -s --max-time 10 --data 'please allow https://pypi.org/' \
    http://humanitl.internal/ask
expect_output meta_unknown_path_404 \
    '^404$' curl -s -o /dev/null -w '%{http_code}' --max-time 10 \
    http://humanitl.internal/secrets
expect_output meta_other_method_405 \
    '^405$' curl -s -o /dev/null -w '%{http_code}' --max-time 10 --data x \
    http://humanitl.internal/
# A name that only looks like the reserved one is an ordinary host and lands in
# the queue like everything else. Without this line the case above would also
# be green if the proxy answered every `*.internal` name itself.
expect_output meta_look_alike_is_held \
    'Blocked by Humanitl' curl -s --max-time 10 http://evil-humanitl.internal/

# --- the host-side observation ------------------------------------------------
#
# ADR-006 says a name is resolved only after the decision. Proving it needs a
# host-side watcher (resolvectl statistics, or tcpdump port 53) around the
# sandbox, and something inside the proxy that could resolve at all. Both arrive
# with HUM-024; recording the case as skipped keeps the claim in the report
# rather than letting it fall out of sight.
skip dns_not_before_decision \
    "needs the resolver after the decision (HUM-024) and a host-side DNS watcher in run.sh"
# The same gap, for the reserved name: that no lookup happens for
# `humanitl.internal` is proven inside the daemon by the counting resolver mock
# (daemon/crates/proxy/tests/meta.rs, `status_over_plain_http`). Proving it from
# out here needs the same host-side watcher.
skip meta_no_dns_lookup \
    "proven by the counting resolver in daemon/crates/proxy/tests/meta.rs; from the host it needs the DNS watcher of dns_not_before_decision"

esc_end
