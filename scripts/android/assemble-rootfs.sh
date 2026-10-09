#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd -P)
REPLACE=0
if [ "${1:-}" = "--replace" ]; then
    REPLACE=1
    shift
fi

if [ "$#" -eq 1 ] && { [ "$1" = "-h" ] || [ "$1" = "--help" ]; }; then
    echo "usage: $0 [--replace] AOSP_PRODUCT_OUT ROOTFS"
    echo "Refuses missing partition images and existing output unless --replace is explicit."
    exit 0
fi
if [ "$#" -ne 2 ]; then
    echo "usage: $0 [--replace] AOSP_PRODUCT_OUT ROOTFS" >&2
    exit 2
fi

PRODUCT_OUT=$(CDPATH= cd -- "$1" 2>/dev/null && pwd -P) || {
    echo "AOSP product output directory does not exist: $1" >&2
    exit 2
}
ROOTFS_ARG=$2
[ -n "$ROOTFS_ARG" ] || { echo "ROOTFS path must not be empty" >&2; exit 2; }
case "$ROOTFS_ARG" in
    /*) ;;
    *) ROOTFS_ARG="$PWD/$ROOTFS_ARG" ;;
esac
while [ "$ROOTFS_ARG" != "/" ] && [ "${ROOTFS_ARG%/}" != "$ROOTFS_ARG" ]; do
    ROOTFS_ARG=${ROOTFS_ARG%/}
done
[ "$ROOTFS_ARG" != "/" ] || { echo "refusing to use / as ROOTFS" >&2; exit 2; }
# Catch obvious lexical overlaps before mkdir can create anything in PRODUCT_OUT.
case "$ROOTFS_ARG/" in
    "$PRODUCT_OUT/"*) echo "ROOTFS must not be inside AOSP PRODUCT_OUT" >&2; exit 2 ;;
esac
case "$PRODUCT_OUT/" in
    "$ROOTFS_ARG/"*) echo "ROOTFS must not contain AOSP PRODUCT_OUT" >&2; exit 2 ;;
esac
ROOTFS_PARENT_ARG=$(dirname -- "$ROOTFS_ARG")
ROOTFS_BASE=$(basename -- "$ROOTFS_ARG")
case "$ROOTFS_BASE" in
    ''|.|..) echo "ROOTFS must name a dedicated output directory, not '$ROOTFS_BASE'" >&2; exit 2 ;;
esac
mkdir -p "$ROOTFS_PARENT_ARG"
ROOTFS_PARENT=$(CDPATH= cd -- "$ROOTFS_PARENT_ARG" && pwd -P)
ROOTFS="$ROOTFS_PARENT/$ROOTFS_BASE"

[ "$ROOTFS_BASE" = "rootfs" ] || { echo "ROOTFS destination must be a dedicated directory named 'rootfs'" >&2; exit 2; }
[ "$ROOTFS_PARENT" != "/" ] || { echo "refusing to place ROOTFS directly in the filesystem root" >&2; exit 2; }
[ "$ROOTFS" != "/" ] || { echo "refusing to use / as ROOTFS" >&2; exit 2; }
[ ! -L "$ROOTFS" ] || { echo "refusing to replace a symlink at ROOTFS: $ROOTFS" >&2; exit 2; }

# Never assemble over or inside the AOSP output tree (or replace a parent that
# contains it). This catches common path mistakes before any mount or deletion.
case "$ROOTFS/" in
    "$PRODUCT_OUT/"*) echo "ROOTFS must not be inside AOSP PRODUCT_OUT" >&2; exit 2 ;;
esac
case "$PRODUCT_OUT/" in
    "$ROOTFS/"*) echo "ROOTFS must not contain AOSP PRODUCT_OUT" >&2; exit 2 ;;
esac

if [ -e "$ROOTFS" ]; then
    [ -d "$ROOTFS" ] || { echo "existing ROOTFS is not a directory: $ROOTFS" >&2; exit 2; }
    if [ "$REPLACE" -ne 1 ]; then
        echo "ROOTFS already exists: $ROOTFS" >&2
        echo "Move it aside or pass --replace to replace it after a new image validates." >&2
        exit 2
    fi
fi

# Fail early if the contributor's image build did not produce the full set of
# partitions expected by this product. Missing partitions must not be silently
# converted into an incomplete Android rootfs.
[ -d "$PRODUCT_OUT/root" ] || { echo "missing AOSP root staging directory: $PRODUCT_OUT/root" >&2; exit 1; }
for image in system.img system_ext.img product.img vendor.img; do
    [ -s "$PRODUCT_OUT/$image" ] || { echo "required product image is missing or empty: $PRODUCT_OUT/$image" >&2; exit 1; }
done

for cmd in rsync mount umount simg2img; do
    command -v "$cmd" >/dev/null 2>&1 || { echo "$cmd is required" >&2; exit 1; }
done
if [ "$(id -u)" -ne 0 ]; then
    command -v sudo >/dev/null 2>&1 || { echo "sudo is required to mount images and preserve rootfs ownership/xattrs" >&2; exit 1; }
fi

as_root() {
    if [ "$(id -u)" -eq 0 ]; then
        "$@"
    else
        sudo "$@"
    fi
}

TMP=$(mktemp -d)
STAGING="$ROOTFS.staging.$$"
BACKUP="$ROOTFS.backup.$$"
[ ! -e "$STAGING" ] && [ ! -L "$STAGING" ] || { rm -rf -- "$TMP"; echo "staging path already exists: $STAGING" >&2; exit 1; }
[ ! -e "$BACKUP" ] && [ ! -L "$BACKUP" ] || { rm -rf -- "$TMP"; echo "backup path already exists: $BACKUP" >&2; exit 1; }
COMMITTED=0

unmount_path() {
    path=$1
    as_root umount "$path" >/dev/null 2>&1 || true
}

cleanup() {
    set +e
    for m in "$TMP"/*/mnt; do
        [ -d "$m" ] || continue
        unmount_path "$m"
    done
    rm -rf -- "$TMP"
    if [ -d "$STAGING" ]; then
        as_root rm -rf -- "$STAGING"
    fi
    if [ "$COMMITTED" -eq 0 ] && [ -d "$BACKUP" ] && [ ! -e "$ROOTFS" ]; then
        as_root mv -- "$BACKUP" "$ROOTFS"
    fi
}
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

as_root mkdir -p "$STAGING"
as_root rsync -aHAX "$PRODUCT_OUT/root/" "$STAGING/"

mount_image() {
    image=$1
    name=$2
    dest=$3
    dir="$TMP/$name"
    mkdir -p "$dir/mnt"
    sparse="$dir/image.raw"
    simg2img "$image" "$sparse"
    as_root mount -o loop,ro "$sparse" "$dir/mnt"
    as_root mkdir -p "$STAGING/$dest"
    as_root rsync -aHAX "$dir/mnt/" "$STAGING/$dest/"
}

mount_image "$PRODUCT_OUT/system.img" system system
mount_image "$PRODUCT_OUT/system_ext.img" system_ext system_ext
mount_image "$PRODUCT_OUT/product.img" product product
mount_image "$PRODUCT_OUT/vendor.img" vendor vendor
if [ -s "$PRODUCT_OUT/odm.img" ]; then
    mount_image "$PRODUCT_OUT/odm.img" odm odm
fi

as_root mkdir -p "$STAGING/data" "$STAGING/run/android-runtime"
as_root chmod 0755 "$STAGING/data" "$STAGING/run" "$STAGING/run/android-runtime"

if [ ! -e "$STAGING/init" ] && [ -x "$STAGING/system/bin/init" ]; then
    as_root ln -s /system/bin/init "$STAGING/init"
fi

"$ROOT/scripts/android/validate-rootfs.sh" "$STAGING"

if [ -e "$ROOTFS" ]; then
    as_root mv -- "$ROOTFS" "$BACKUP"
fi
if ! as_root mv -- "$STAGING" "$ROOTFS"; then
    if [ -d "$BACKUP" ] && [ ! -e "$ROOTFS" ]; then
        as_root mv -- "$BACKUP" "$ROOTFS"
    fi
    echo "failed to publish validated rootfs at $ROOTFS" >&2
    exit 1
fi
COMMITTED=1
if [ -d "$BACKUP" ]; then
    as_root rm -rf -- "$BACKUP"
fi

echo "assembled and validated Android container rootfs at $ROOTFS"
