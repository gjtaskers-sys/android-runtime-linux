#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd -- "$(dirname -- "$0")/../.." && pwd -P)
SOURCE_MANIFEST=
ROOTFS_ARG=
OUTPUT_ARG=

show_usage() {
    cat <<USAGE
Usage: sudo $0 [--source-manifest PINNED_MANIFEST.xml] ROOTFS OUTPUT.tar.zst

Validates and packages a clean Android Runtime rootfs for handoff from an AOSP
builder to normal runtime contributors. It preserves numeric ownership, ACLs,
extended attributes, and SELinux labels and emits .sha256 and .info sidecars.
It does not sync or build AOSP.
USAGE
}
usage() { show_usage >&2; exit 2; }

while [[ $# -gt 0 ]]; do
    case "$1" in
        --source-manifest)
            [[ $# -ge 2 && -z "$SOURCE_MANIFEST" && -n "$2" ]] || usage
            SOURCE_MANIFEST=$2
            shift 2
            ;;
        -h|--help) show_usage; exit 0 ;;
        --*) echo "unknown option: $1" >&2; usage ;;
        *)
            if [[ -z "$ROOTFS_ARG" ]]; then ROOTFS_ARG=$1
            elif [[ -z "$OUTPUT_ARG" ]]; then OUTPUT_ARG=$1
            else usage
            fi
            shift
            ;;
    esac
done
[[ -n "$ROOTFS_ARG" && -n "$OUTPUT_ARG" ]] || usage

if [[ $EUID -ne 0 ]]; then
    echo "run this packaging step as root so tar can preserve file ownership, ACLs, and security xattrs" >&2
    echo "rerun the same command with sudo." >&2
    exit 1
fi

[[ -d "$ROOTFS_ARG" && ! -L "$ROOTFS_ARG" ]] || { echo "ROOTFS must be an existing real directory: $ROOTFS_ARG" >&2; exit 1; }
ROOTFS=$(cd -- "$ROOTFS_ARG" && pwd -P)
ROOTFS_BASE=$(basename -- "$ROOTFS")
[[ "$ROOTFS_BASE" == "rootfs" ]] || { echo "rootfs directory must be named 'rootfs' so every artifact has a stable archive layout" >&2; exit 1; }
"$ROOT/scripts/android/validate-rootfs.sh" "$ROOTFS"

if [[ -n "$SOURCE_MANIFEST" && ! -f "$SOURCE_MANIFEST" ]]; then
    echo "pinned source manifest does not exist: $SOURCE_MANIFEST" >&2
    exit 1
fi

# A distributable base image must not capture an app's userdata or a running
# instance's private state. Runtime userdata is stored outside the rootfs.
DATA_CONTENT=$(find "$ROOTFS/data" -mindepth 1 -print -quit)
[[ -z "$DATA_CONTENT" ]] || { echo "refusing to package non-empty rootfs /data; use a clean assembled rootfs, not runtime userdata" >&2; exit 1; }

for cmd in tar zstd sha256sum awk mktemp; do
    command -v "$cmd" >/dev/null 2>&1 || { echo "$cmd is required" >&2; exit 1; }
done

case "$OUTPUT_ARG" in
    /*) ;;
    *) OUTPUT_ARG="$PWD/$OUTPUT_ARG" ;;
esac
while [[ "$OUTPUT_ARG" != "/" && "$OUTPUT_ARG" == */ ]]; do OUTPUT_ARG=${OUTPUT_ARG%/}; done
OUT_BASE=$(basename -- "$OUTPUT_ARG")
[[ "$OUT_BASE" != "." && "$OUT_BASE" != ".." && "$OUT_BASE" != "/" ]] || { echo "OUTPUT must name an archive file" >&2; exit 2; }
OUT_PARENT_ARG=$(dirname -- "$OUTPUT_ARG")
mkdir -p "$OUT_PARENT_ARG"
OUT_PARENT=$(cd -- "$OUT_PARENT_ARG" && pwd -P)
OUTPUT="$OUT_PARENT/$OUT_BASE"
INFO="$OUTPUT.info"
CHECKSUM="$OUTPUT.sha256"

case "$OUTPUT" in
    "$ROOTFS"/*) echo "output archive must be outside the rootfs" >&2; exit 2 ;;
esac
for target in "$OUTPUT" "$INFO" "$CHECKSUM"; do
    [[ ! -e "$target" && ! -L "$target" ]] || { echo "refusing to overwrite existing artifact: $target" >&2; exit 1; }
done

TMP_ARCHIVE=$(mktemp "$OUT_PARENT/.${OUT_BASE}.tmp.XXXXXX")
TMP_INFO="$TMP_ARCHIVE.info"
TMP_CHECKSUM="$TMP_ARCHIVE.sha256"
cleanup() { rm -f -- "$TMP_ARCHIVE" "$TMP_INFO" "$TMP_CHECKSUM"; }
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

ROOTFS_PARENT=$(dirname -- "$ROOTFS")
# Normalize timestamps and member order while preserving Android uid/gid, mode,
# hard links, ACLs and xattrs. Delete volatile PAX atime/ctime fields as well:
# otherwise equivalent trees can receive different archive hashes.
tar --sort=name --mtime=@0 --numeric-owner \
    --pax-option=delete=atime,delete=ctime \
    --xattrs --xattrs-include='*' --acls --selinux \
    -C "$ROOTFS_PARENT" -cf - "$ROOTFS_BASE" \
    | zstd -f -T0 -19 -q -o "$TMP_ARCHIVE"

ARCHIVE_SHA=$(sha256sum "$TMP_ARCHIVE" | awk '{print $1}')
SOURCE_SHA=not-provided
if [[ -n "$SOURCE_MANIFEST" ]]; then
    SOURCE_SHA=$(sha256sum "$SOURCE_MANIFEST" | awk '{print $1}')
fi
printf '%s  %s\n' "$ARCHIVE_SHA" "$OUT_BASE" > "$TMP_CHECKSUM"
cat > "$TMP_INFO" <<INFO
format_version=1
product=android_runtime_x86_64_minigbm
architecture=x86_64
archive=$(basename -- "$OUTPUT")
archive_sha256=$ARCHIVE_SHA
source_manifest_sha256=$SOURCE_SHA
rootfs_validation=passed
rootfs_data=empty
INFO

mv -- "$TMP_ARCHIVE" "$OUTPUT"
mv -- "$TMP_CHECKSUM" "$CHECKSUM"
mv -- "$TMP_INFO" "$INFO"
printf 'Packaged: %s\nSHA-256: %s\nMetadata: %s\n' "$OUTPUT" "$ARCHIVE_SHA" "$INFO"
