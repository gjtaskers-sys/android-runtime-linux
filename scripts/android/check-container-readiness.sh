#!/bin/sh
set -u

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd -P)
ROOTFS=
BRIDGE=lxcbr0
ROOTFS_ONLY=0

usage() {
    cat >&2 <<USAGE
Usage: $0 [--rootfs ROOTFS] [--bridge NAME] [--rootfs-only]

Checks the current host's LXC/binder/display prerequisites. If --rootfs is
provided, it also validates the assembled Android rootfs. It never loads kernel
modules, changes permissions, starts a container, or builds AOSP.

Modes:
  --rootfs ROOTFS   Assembled prebuilt rootfs to validate
  --bridge NAME     Host Linux bridge used for container networking (default: lxcbr0)
  --rootfs-only     Validate only the rootfs structure; suitable for image CI
USAGE
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --rootfs)
            [ "$#" -ge 2 ] || { usage; exit 2; }
            [ -n "$2" ] || { echo "--rootfs needs a non-empty path" >&2; exit 2; }
            [ -z "$ROOTFS" ] || { echo "--rootfs may only be specified once" >&2; exit 2; }
            ROOTFS=$2
            shift 2
            ;;
        --bridge)
            [ "$#" -ge 2 ] || { usage; exit 2; }
            [ -n "$2" ] || { echo "--bridge needs a non-empty name" >&2; exit 2; }
            BRIDGE=$2
            shift 2
            ;;
        --rootfs-only)
            ROOTFS_ONLY=1
            shift
            ;;
        -h|--help) usage; exit 0 ;;
        *) echo "unknown option: $1" >&2; usage; exit 2 ;;
    esac
done

case "$BRIDGE" in
    ''|*[!A-Za-z0-9_.-]*) echo "invalid bridge name: $BRIDGE" >&2; exit 2 ;;
esac

if [ "$ROOTFS_ONLY" -eq 1 ]; then
    [ -n "$ROOTFS" ] || { echo "--rootfs-only requires --rootfs ROOTFS" >&2; exit 2; }
    exec "$ROOT/scripts/android/validate-rootfs.sh" "$ROOTFS"
fi

errors=0
warnings=0
passes=0

pass() { printf 'PASS: %s\n' "$1"; passes=$((passes + 1)); }
fail() { printf 'FAIL: %s\n' "$1" >&2; errors=$((errors + 1)); }
warn() { printf 'WARN: %s\n' "$1" >&2; warnings=$((warnings + 1)); }

for cmd in lxc-start lxc-stop lxc-info; do
    if command -v "$cmd" >/dev/null 2>&1; then
        pass "required command '$cmd' is installed"
    else
        fail "required command '$cmd' is missing (install the distro's LXC package)"
    fi
done

binder_missing=0
for node in /dev/binder /dev/hwbinder /dev/vndbinder; do
    if [ -c "$node" ]; then
        pass "$node is a character device"
        if [ ! -r "$node" ] || [ ! -w "$node" ]; then
            fail "$node is not readable and writable by the current user; the current runtime has no privileged device-access helper"
        fi
    else
        fail "$node is missing; current prototype expects standard binder_linux device nodes"
        binder_missing=1
    fi
done
if [ "$binder_missing" -ne 0 ]; then
    cat >&2 <<'MSG'
Binder setup is incomplete. A manual diagnostic is:
  ls -l /dev/binder /dev/hwbinder /dev/vndbinder
  sudo modprobe binder_linux devices="binder,hwbinder,vndbinder"
Do not apply broad permissions to /dev. Binderfs-only hosts are not supported by
the current host-setup script and need an explicit implementation first.
MSG
fi

if [ -d "/sys/class/net/$BRIDGE/bridge" ]; then
    pass "Linux bridge '$BRIDGE' exists"
else
    fail "Linux bridge '$BRIDGE' is missing; the generated LXC config requires it (start/configure lxc-net or pass --bridge NAME)"
fi

if [ -n "${DISPLAY:-}" ]; then
    pass "DISPLAY is set for the native X11 target ($DISPLAY)"
else
    warn "DISPLAY is unset; Android may boot, but the native X11 window cannot be opened from this shell"
fi

found_render=0
for node in /dev/dri/renderD*; do
    [ -e "$node" ] || continue
    if [ -c "$node" ]; then
        pass "DRM render node found: $node"
        found_render=1
    fi
done
if [ "$found_render" -eq 0 ]; then
    warn "no DRM render node found; software rendering is the intended initial fallback"
fi

if [ -n "$ROOTFS" ]; then
    if "$ROOT/scripts/android/validate-rootfs.sh" "$ROOTFS"; then
        pass "Android rootfs structural validation passed"
    else
        fail "Android rootfs failed structural validation"
    fi
else
    warn "no --rootfs supplied; image contents were not checked. Contributors should validate their prebuilt artifact before handoff"
fi

printf '\nContainer readiness: %s passed, %s warning(s), %s failed\n' "$passes" "$warnings" "$errors"
if [ "$errors" -ne 0 ]; then
    printf '%s\n' "NOT READY: fix the failed requirements above before attempting LXC boot."
    exit 1
fi
printf '%s\n' "Host checks passed. Warnings above still need review; this check does not boot Android or prove kernel/userspace compatibility."
