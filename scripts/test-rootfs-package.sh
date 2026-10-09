#!/bin/bash
set -euo pipefail
ROOT=$(cd -- "$(dirname -- "$0")/.." && pwd -P)
if [[ $EUID -ne 0 ]]; then
    echo "rootfs package test must run as root to exercise metadata-preserving packaging" >&2
    exit 1
fi
for cmd in zstd tar sha256sum; do command -v "$cmd" >/dev/null 2>&1 || { echo "$cmd is required" >&2; exit 1; }; done
TMP=$(mktemp -d)
cleanup() { rm -rf -- "$TMP"; }
trap cleanup EXIT INT TERM

ROOTFS="$TMP/rootfs"
OUT="$TMP/artifacts"
MANIFEST="$TMP/repo-manifest-pinned.xml"
mkdir -p "$ROOTFS/system/bin" "$ROOTFS/system_ext" "$ROOTFS/product" \
    "$ROOTFS/vendor/etc/init" "$ROOTFS/vendor/bin/hw" "$ROOTFS/data" \
    "$ROOTFS/run/android-runtime" "$OUT"
for file in init system/bin/init system/bin/linker64 system/bin/app_process64 \
    system/bin/servicemanager system/bin/surfaceflinger \
    vendor/bin/hw/android.hardware.graphics.composer3-service.ranchu; do
    : > "$ROOTFS/$file"
    chmod 0755 "$ROOTFS/$file"
done
: > "$ROOTFS/system/build.prop"
: > "$ROOTFS/system_ext/build.prop"
: > "$ROOTFS/product/build.prop"
: > "$ROOTFS/vendor/build.prop"
: > "$ROOTFS/vendor/etc/init/init.android-runtime.rc"
: > "$ROOTFS/vendor/etc/fstab.android_runtime"
printf '%s\n' '<manifest revision="fixture" />' > "$MANIFEST"

ARCHIVE="$OUT/android-runtime-x86_64-rootfs.tar.zst"
"$ROOT/scripts/android/package-rootfs.sh" --source-manifest "$MANIFEST" "$ROOTFS" "$ARCHIVE" > "$TMP/package.log"
zstd -t "$ARCHIVE" >/dev/null
zstd -dc "$ARCHIVE" | tar -tf - | grep -q 'rootfs/system/bin/surfaceflinger'
(cd "$OUT" && sha256sum -c "$(basename -- "$ARCHIVE").sha256")
grep -q 'rootfs_validation=passed' "$ARCHIVE.info"
grep -q "source_manifest_sha256=$(sha256sum "$MANIFEST" | awk '{print $1}')" "$ARCHIVE.info"

# The normalized tar stream should be byte-identical when repackaging the same tree.
SECOND_PARENT="$TMP/rebuild"
SECOND_OUT="$TMP/rebuild-artifacts"
mkdir -p "$SECOND_PARENT" "$SECOND_OUT"
cp -a "$ROOTFS" "$SECOND_PARENT/rootfs"
SECOND_ARCHIVE="$SECOND_OUT/rebuilt-rootfs.tar.zst"
"$ROOT/scripts/android/package-rootfs.sh" "$SECOND_PARENT/rootfs" "$SECOND_ARCHIVE" > "$TMP/rebuild.log"
FIRST_SHA=$(sha256sum "$ARCHIVE" | awk '{print $1}')
SECOND_SHA=$(sha256sum "$SECOND_ARCHIVE" | awk '{print $1}')
[[ "$FIRST_SHA" == "$SECOND_SHA" ]] || {
    echo "repackaging produced different hashes for the same rootfs: $FIRST_SHA != $SECOND_SHA" >&2
    exit 1
}

# Refuse to export an image after runtime state has appeared under /data.
mkdir -p "$ROOTFS/data/app"
if "$ROOT/scripts/android/package-rootfs.sh" "$ROOTFS" "$OUT/dirty.tar.zst" > "$TMP/dirty.log" 2>&1; then
    echo "package script unexpectedly accepted non-empty /data" >&2
    exit 1
fi
grep -q 'refusing to package non-empty rootfs /data' "$TMP/dirty.log"
[[ ! -e "$OUT/dirty.tar.zst" ]]

echo "rootfs package tests passed (metadata-preserving and hash-stable archive, checksum/manifest sidecars, runtime-data rejection)"
