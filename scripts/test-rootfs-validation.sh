#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd -P)
TMP=$(mktemp -d)
cleanup() { rm -rf "$TMP"; }
trap cleanup EXIT INT TERM

VALID="$TMP/valid-rootfs"
mkdir -p \
    "$VALID/system/bin" \
    "$VALID/system_ext" \
    "$VALID/product" \
    "$VALID/vendor/etc/init" \
    "$VALID/vendor/bin/hw" \
    "$VALID/data" \
    "$VALID/run/android-runtime"

for file in init system/bin/init system/bin/linker64 system/bin/app_process64 \
    system/bin/servicemanager system/bin/surfaceflinger \
    vendor/bin/hw/android.hardware.graphics.composer3-service.ranchu; do
    : > "$VALID/$file"
    chmod 0755 "$VALID/$file"
done
: > "$VALID/system/build.prop"
: > "$VALID/system_ext/build.prop"
: > "$VALID/product/build.prop"
: > "$VALID/vendor/build.prop"
: > "$VALID/vendor/etc/init/init.android-runtime.rc"
: > "$VALID/vendor/etc/fstab.android_runtime"

"$ROOT/scripts/android/validate-rootfs.sh" "$VALID" > "$TMP/valid.log"
grep -q 'Rootfs structure matches' "$TMP/valid.log"
# An absolute /init symlink must resolve inside the container rootfs, not host /.
rm "$VALID/init"
ln -s /system/bin/init "$VALID/init"
"$ROOT/scripts/android/validate-rootfs.sh" "$VALID" > "$TMP/symlink-init.log"
grep -q 'symlink resolves to executable' "$TMP/symlink-init.log"
"$ROOT/scripts/android/check-container-readiness.sh" --rootfs-only --rootfs "$VALID" > "$TMP/preflight.log"
grep -q 'Rootfs structure matches' "$TMP/preflight.log"
if "$ROOT/scripts/android/check-container-readiness.sh" --bridge 'lxcbr0;echo-bad' --rootfs-only --rootfs "$VALID" > "$TMP/bad-bridge.log" 2>&1; then
    echo "preflight unexpectedly accepted a shell-like bridge name" >&2
    exit 1
fi
grep -q 'invalid bridge name' "$TMP/bad-bridge.log"

# A missing required Android service must make validation fail.
rm "$VALID/system/bin/surfaceflinger"
if "$ROOT/scripts/android/validate-rootfs.sh" "$VALID" > "$TMP/invalid.log" 2>&1; then
    echo "validator incorrectly accepted a rootfs without surfaceflinger" >&2
    exit 1
fi
grep -q 'missing or non-executable /system/bin/surfaceflinger' "$TMP/invalid.log"
: > "$VALID/system/bin/surfaceflinger"
chmod 0755 "$VALID/system/bin/surfaceflinger"
rm "$VALID/vendor/bin/hw/android.hardware.graphics.composer3-service.ranchu"
if "$ROOT/scripts/android/validate-rootfs.sh" "$VALID" > "$TMP/no-composer.log" 2>&1; then
    echo "validator incorrectly accepted a rootfs without the Composer3 service" >&2
    exit 1
fi
grep -q 'missing Composer3 service binary' "$TMP/no-composer.log"

# An invalid path must fail without attempting any mount or mutation.
if "$ROOT/scripts/android/validate-rootfs.sh" "$TMP/not-a-rootfs" > "$TMP/missing.log" 2>&1; then
    echo "validator incorrectly accepted a missing rootfs" >&2
    exit 1
fi

echo "rootfs validation tests passed (valid fixture accepted; missing service and missing directory rejected)"
