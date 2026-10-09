#!/bin/sh
set -eu

usage() {
    echo "usage: $0 ROOTFS" >&2
    exit 2
}

[ "$#" -eq 1 ] || usage
[ -d "$1" ] || { echo "FAIL: rootfs directory does not exist: $1" >&2; exit 1; }
ROOTFS=$(CDPATH= cd -- "$1" && pwd -P)

errors=0
passes=0

pass() {
    printf 'PASS: %s\n' "$1"
    passes=$((passes + 1))
}

fail() {
    printf 'FAIL: %s\n' "$1" >&2
    errors=$((errors + 1))
}

require_dir() {
    if [ -d "$ROOTFS/$1" ]; then
        pass "/$1 directory present"
    else
        fail "missing /$1 directory"
    fi
}

require_file() {
    if [ -f "$ROOTFS/$1" ]; then
        pass "/$1 file present"
    else
        fail "missing /$1 file"
    fi
}

require_executable() {
    if [ -f "$ROOTFS/$1" ] && [ -x "$ROOTFS/$1" ]; then
        pass "/$1 executable present"
    else
        fail "missing or non-executable /$1"
    fi
}

# This validates the artifact contract for the current AOSP x86_64 product. It
# does not boot Android, inspect kernel compatibility, or prove HWC operation.
# Absolute symlinks inside a container rootfs resolve against the container's
# root, not the host's /. Check the common AOSP /init -> /system/bin/init form
# inside ROOTFS instead of following it against the host filesystem.
if [ -L "$ROOTFS/init" ]; then
    init_target=$(readlink "$ROOTFS/init")
    case "$init_target" in
        /system/bin/init|system/bin/init)
            if [ -f "$ROOTFS/system/bin/init" ] && [ -x "$ROOTFS/system/bin/init" ]; then
                pass "/init symlink resolves to executable /system/bin/init inside rootfs"
            else
                fail "/init points to system/bin/init, but that target is missing or non-executable"
            fi
            ;;
        *) fail "unsupported /init symlink target: $init_target" ;;
    esac
elif [ -f "$ROOTFS/init" ] && [ -x "$ROOTFS/init" ]; then
    pass "/init executable present"
else
    fail "missing or non-executable /init"
fi
require_executable system/bin/init
require_executable system/bin/linker64
require_executable system/bin/app_process64
require_executable system/bin/servicemanager
require_executable system/bin/surfaceflinger

for dir in system system_ext product vendor data run/android-runtime; do
    require_dir "$dir"
done

for file in system/build.prop system_ext/build.prop product/build.prop vendor/build.prop vendor/etc/init/init.android-runtime.rc vendor/etc/fstab.android_runtime; do
    require_file "$file"
done

if [ -x "$ROOTFS/vendor/bin/hw/android.hardware.graphics.composer3-service.ranchu" ] || \
   [ -x "$ROOTFS/vendor/bin/hw/android.hardware.graphics.composer3-service" ] || \
   [ -x "$ROOTFS/system/bin/hw/android.hardware.graphics.composer3-service.ranchu" ]; then
    pass "Composer3 service candidate present"
else
    fail "missing Composer3 service binary at the expected vendor/system hw paths"
fi

printf '\nRootfs validation: %s passed, %s failed\n' "$passes" "$errors"
if [ "$errors" -ne 0 ]; then
    cat >&2 <<'MSG'
The image is not ready for an LXC boot attempt. Build the expected AOSP product
and assemble system, system_ext, product, and vendor into one rootfs. This check
is structural only; it cannot certify the image against the host kernel.
MSG
    exit 1
fi

printf '%s\n' "Rootfs structure matches the current Android Runtime artifact contract."
