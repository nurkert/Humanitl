# shellcheck shell=sh
# Sourced by run.sh after the suites in the sandbox (HUM-435): the case
# esc-2/engine_hook_not_run.
#
# A container engine reads much more than its command line: containers.conf in
# every layer, mounts.conf, OCI hooks. A process of the same user can change
# all of them and so add a mount, a volume or a hook to every container
# (THREAT-MODEL K-27). The Podman adapter starts the engine with a
# containers.conf of its own and an empty hook directory (HUM-433); this case
# and two probes in esc-2 show that the user's configuration stays out.
#
# The CI job escape-tests-podman plants such a configuration before the run
# with tests/escape/plant-engine-config.sh: a mounts.conf entry for
# /run/secrets, a volume, and a hook that creates a marker file. The script
# first proves with a plain container that the engine does apply all three, so
# that "not in the sandbox" means "kept out" and not "never read". It leaves
# the directory in HUMANITL_ESCAPE_ENGINE_CANARY_DIR. Inside the sandbox,
# esc-2 looks for the secret and the volume (no_engine_secrets,
# no_engine_volume); here, on the host, the marker of the hook must still be
# missing after every sandbox of the run.
#
# bwrap and the kernel level (qemu, HUM-445) read no engine configuration, so
# without a planted one there is no case for them. For the container engine
# the case without a planted configuration is a skip that says so, never a
# pass. A developer machine runs it that way: the planting writes into
# ~/.config/containers and belongs on a runner.
#
# Needs from run.sh: ESCAPE_BACKEND and record_case.

engine_config_case() {
    engine_dir="${HUMANITL_ESCAPE_ENGINE_CANARY_DIR:-}"
    if [ -z "$engine_dir" ]; then
        if [ "$ESCAPE_BACKEND" != bwrap ] && [ "$ESCAPE_BACKEND" != qemu ]; then
            record_case esc-2 engine_hook_not_run skip \
                "no planted engine configuration; tests/escape/plant-engine-config.sh plants one in the CI job escape-tests-podman"
        fi
        return
    fi
    if [ ! -f "$engine_dir/planted" ]; then
        record_case esc-2 engine_hook_not_run fail \
            "HUMANITL_ESCAPE_ENGINE_CANARY_DIR=$engine_dir holds no proof that a configuration was planted and took effect"
        return
    fi
    if [ -e "$engine_dir/hook-ran" ]; then
        record_case esc-2 engine_hook_not_run fail \
            "LEAK: the hook of the user's engine configuration ran for a sandbox ($engine_dir/hook-ran exists)"
    else
        record_case esc-2 engine_hook_not_run pass \
            "the hook of the user's engine configuration, proven to run for a plain container, did not run for any sandbox"
    fi
}
