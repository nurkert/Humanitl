#!/usr/bin/env sh
# Plants a hostile configuration of the user's container engine (HUM-435).
#
# THIS WRITES INTO ~/.config/containers. It is for a disposable CI runner, the
# job escape-tests-podman, and refuses to run anywhere else unless
# HUMANITL_ESCAPE_PLANT_ENGINE_CONFIG=1 says the machine is disposable too. It
# never overwrites a file that is already there.
#
# What it plants, the three ways a process of the same user could add
# something to every container (THREAT-MODEL K-27):
#
#   ~/.config/containers/mounts.conf     a directory with a canary file onto
#                                        /run/secrets
#   ~/.config/containers/containers.conf a volume with a canary file onto
#                                        /humanitl-engine-volume, and a hook
#                                        directory
#   ~/.config/containers/oci/hooks.d/    a prestart hook that creates the marker
#                                        file hook-ran
#
# Then it proves that the engine applies all three, with a plain container
# from a local image (busybox, imported, no registry): the canary is read from
# both mounts and the marker appears. Only then does it remove the marker,
# write the file `planted` and hand the directory to the run through
# HUMANITL_ESCAPE_ENGINE_CANARY_DIR in $GITHUB_ENV. Without that proof, the
# probes that find none of it in the sandbox would be green for a
# configuration nobody read (tests/escape/engine-config-proof.sh).
#
#   tests/escape/plant-engine-config.sh DIR
#
# DIR is created and holds the canary, the marker and the proof. Exit 0 when
# the configuration is planted and proven, 1 when the engine did not apply it,
# 2 for a usage error or a machine this must not run on.
set -eu

if [ "${GITHUB_ACTIONS:-}" != true ] && [ "${HUMANITL_ESCAPE_PLANT_ENGINE_CONFIG:-0}" != 1 ]; then
    echo "plant-engine-config: refusing to write into ~/.config/containers outside a CI runner" >&2
    echo "plant-engine-config: set HUMANITL_ESCAPE_PLANT_ENGINE_CONFIG=1 only on a disposable machine" >&2
    exit 2
fi
if [ $# -ne 1 ] || [ -z "$1" ]; then
    echo "usage: tests/escape/plant-engine-config.sh DIR" >&2
    exit 2
fi
case "$1" in
/*) DIR="$1" ;;
*) DIR="$(pwd)/$1" ;;
esac
CANARY=HUMANITL_ENGINE_CANARY
IMAGE=localhost/humanitl-escape-control:latest
CONF="$HOME/.config/containers"
HOOKS="$CONF/oci/hooks.d"

for file in "$CONF/containers.conf" "$CONF/mounts.conf" "$HOOKS/humanitl-escape.json"; do
    if [ -e "$file" ]; then
        echo "plant-engine-config: $file exists; this script never overwrites a configuration" >&2
        exit 2
    fi
done
BUSYBOX=""
for candidate in /bin/busybox /usr/bin/busybox; do
    if [ -x "$candidate" ] && ldd "$candidate" 2>&1 | grep -q "not a dynamic executable"; then
        BUSYBOX="$candidate"
        break
    fi
done
if [ -z "$BUSYBOX" ]; then
    echo "plant-engine-config: no static busybox for the control container (apt-get install busybox-static)" >&2
    exit 2
fi

mkdir -p "$DIR/secret" "$DIR/volume" "$DIR/rootfs/bin" "$HOOKS"
rm -f "$DIR/hook-ran" "$DIR/planted"
printf '%s\n' "$CANARY" > "$DIR/secret/canary"
printf '%s\n' "$CANARY" > "$DIR/volume/canary"

printf '%s:/run/secrets\n' "$DIR/secret" > "$CONF/mounts.conf"
cat > "$CONF/containers.conf" <<EOF
# Planted by tests/escape/plant-engine-config.sh (HUM-435). A sandbox must see
# none of it.
[containers]
default_mounts_file = "$CONF/mounts.conf"
volumes = ["$DIR/volume:/humanitl-engine-volume:ro"]

[engine]
hooks_dir = ["$HOOKS"]
EOF
cat > "$HOOKS/humanitl-escape.json" <<EOF
{"version": "1.0.0", "hook": {"path": "/usr/bin/touch", "args": ["touch", "$DIR/hook-ran"]}, "when": {"always": true}, "stages": ["prestart"]}
EOF
echo "plant-engine-config: planted under $CONF"

# The control: a plain container of the same engine, with the user's
# configuration as planted.
cp "$BUSYBOX" "$DIR/rootfs/bin/busybox"
tar -C "$DIR/rootfs" -cf - . | podman import - "$IMAGE" > /dev/null
set +e
control=$(podman run --rm --network none --pull never "$IMAGE" \
    /bin/busybox sh -c 'cat /run/secrets/canary /humanitl-engine-volume/canary' 2>&1)
control_code=$?
set -e
podman rmi "$IMAGE" > /dev/null 2>&1 || true
seen=$(printf '%s\n' "$control" | grep -cx "$CANARY" || true)
if [ "$control_code" -ne 0 ] || [ "$seen" != 2 ] || [ ! -e "$DIR/hook-ran" ]; then
    echo "plant-engine-config: the engine did not apply the planted configuration" >&2
    echo "plant-engine-config: exit $control_code, canary read $seen of 2 times, hook ran: $([ -e "$DIR/hook-ran" ] && echo yes || echo no)" >&2
    printf '%s\n' "$control" | sed 's/^/  /' >&2
    exit 1
fi
rm -f "$DIR/hook-ran"
printf 'mounts.conf, volume and hook applied to a plain container\n' > "$DIR/planted"
echo "plant-engine-config: a plain container sees the secret and the volume, and the hook ran"
if [ -n "${GITHUB_ENV:-}" ]; then
    printf 'HUMANITL_ESCAPE_ENGINE_CANARY_DIR=%s\n' "$DIR" >> "$GITHUB_ENV"
fi
