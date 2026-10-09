#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
AOSP=${AOSP_DIR:-}
JOBS=${JOBS:-$(nproc)}
OUT=${ANDROID_RUNTIME_OUT:-$ROOT/out/android-runtime}

usage() {
    cat <<USAGE
Usage: AOSP_DIR=/path/to/aosp $0 [--no-build] [--replace-rootfs]

Environment:
  AOSP_DIR   Existing AOSP checkout.
  JOBS       Parallel build jobs (default: nproc).
  ANDROID_RUNTIME_OUT  Image/rootfs output directory.
  --replace-rootfs     Replace an existing assembled rootfs only after the new one validates.

The host runtime does not require an AOSP checkout. This script is for
maintainers/build infrastructure because an Android 17 source build is large.
USAGE
}

NO_BUILD=0
REPLACE_ROOTFS=0
for arg in "$@"; do
    case "$arg" in
        --no-build) NO_BUILD=1 ;;
        --replace-rootfs) REPLACE_ROOTFS=1 ;;
        -h|--help) usage; exit 0 ;;
        *) echo "unknown argument: $arg" >&2; usage >&2; exit 2 ;;
    esac
done

[ -n "$AOSP" ] || { echo "AOSP_DIR is required" >&2; exit 2; }
AOSP=$(CDPATH= cd -- "$AOSP" && pwd -P) || { echo "AOSP_DIR does not exist" >&2; exit 2; }
[ -f "$AOSP/build/envsetup.sh" ] || { echo "$AOSP is not an AOSP checkout" >&2; exit 2; }
command -v repo >/dev/null 2>&1 || { echo "repo is required to pin the exact AOSP source manifest" >&2; exit 1; }
[ -d "$AOSP/.repo" ] || { echo "AOSP checkout is missing .repo metadata; refusing to build an unpinned image" >&2; exit 1; }

# Fail before patching or compiling if this output already contains an image and
# replacement was not explicitly requested. This saves a very expensive build.
mkdir -p "$OUT"
OUT=$(CDPATH= cd -- "$OUT" && pwd -P)
[ ! -L "$OUT/rootfs" ] || { echo "refusing to use a symlink as output rootfs: $OUT/rootfs" >&2; exit 2; }
if [ -e "$OUT/rootfs" ]; then
    [ -d "$OUT/rootfs" ] || { echo "existing output rootfs is not a directory: $OUT/rootfs" >&2; exit 2; }
    if [ "$REPLACE_ROOTFS" -ne 1 ]; then
        echo "output rootfs already exists: $OUT/rootfs" >&2
        echo "Move it aside or pass --replace-rootfs after reviewing the candidate build." >&2
        exit 2
    fi
fi

# Capture the exact checked-out revisions before overlaying project changes.
# Publish this manifest only after the candidate rootfs has assembled/validated.
TMP_MANIFEST=$(mktemp "$OUT/.repo-manifest-pinned.XXXXXX")
cleanup() { [ -z "${TMP_MANIFEST:-}" ] || rm -f -- "$TMP_MANIFEST"; }
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM
repo -C "$AOSP" manifest -r > "$TMP_MANIFEST"

python3 "$ROOT/android/patches/apply-aosp-overlay.py" "$AOSP"

if [ "$NO_BUILD" -eq 0 ]; then
    # shellcheck disable=SC1091
    . "$AOSP/build/envsetup.sh"
    lunch android_runtime_x86_64_minigbm-userdebug
    m -j"$JOBS" systemimage vendorimage productimage system_extimage
fi

PRODUCT=android_runtime_x86_64_minigbm
PRODUCT_OUT="$AOSP/out/target/product/$PRODUCT"
[ -d "$PRODUCT_OUT" ] || { echo "missing $PRODUCT_OUT; run without --no-build first" >&2; exit 1; }

if [ "$REPLACE_ROOTFS" -eq 1 ]; then
    "$ROOT/scripts/android/assemble-rootfs.sh" --replace "$PRODUCT_OUT" "$OUT/rootfs"
else
    "$ROOT/scripts/android/assemble-rootfs.sh" "$PRODUCT_OUT" "$OUT/rootfs"
fi
cp -f "$AOSP/build/soong/soong_ui.bash" "$OUT/soong-ui-reference" 2>/dev/null || true
printf '%s\n' "$PRODUCT_OUT" > "$OUT/source-product-out.txt"
printf '%s\n' "$PRODUCT" > "$OUT/product-name.txt"
mv -f -- "$TMP_MANIFEST" "$OUT/repo-manifest-pinned.xml"
TMP_MANIFEST=
echo "Android Runtime rootfs: $OUT/rootfs"
echo "Pinned source manifest: $OUT/repo-manifest-pinned.xml"
