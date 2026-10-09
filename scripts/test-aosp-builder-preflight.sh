#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd -P)
TMP=$(mktemp -d)
cleanup() { rm -rf -- "$TMP"; }
trap cleanup EXIT HUP INT TERM

FAKEBIN="$TMP/bin"
AOSP="$TMP/aosp"
OUT="$TMP/out"
mkdir -p "$FAKEBIN" "$AOSP/build" "$AOSP/.repo" "$OUT/rootfs"
: > "$AOSP/build/envsetup.sh"
printf '%s\n' 'must be preserved' > "$OUT/rootfs/sentinel"
cat > "$FAKEBIN/repo" <<'REPO'
#!/bin/sh
printf '%s\n' "repo unexpectedly invoked: $*" >> "${REPO_CALL_LOG:?}"
exit 99
REPO
chmod +x "$FAKEBIN/repo"
export PATH="$FAKEBIN:$PATH"
export REPO_CALL_LOG="$TMP/repo-called.log"

# An existing output must stop before repo, overlay, or the expensive AOSP build.
if AOSP_DIR="$AOSP" ANDROID_RUNTIME_OUT="$OUT" \
    "$ROOT/scripts/android/build-aosp-image.sh" --no-build > "$TMP/existing.log" 2>&1; then
    echo "builder unexpectedly accepted an existing rootfs without --replace-rootfs" >&2
    exit 1
fi
grep -q 'output rootfs already exists' "$TMP/existing.log"
grep -q 'must be preserved' "$OUT/rootfs/sentinel"
[ ! -e "$REPO_CALL_LOG" ] || { echo "builder invoked repo before the existing-output guard" >&2; exit 1; }

# Missing repo metadata must also fail before the AOSP overlay or any build command.
rm -rf "$OUT/rootfs"
rm -rf "$AOSP/.repo"
if AOSP_DIR="$AOSP" ANDROID_RUNTIME_OUT="$OUT/missing-metadata" \
    "$ROOT/scripts/android/build-aosp-image.sh" --no-build > "$TMP/metadata.log" 2>&1; then
    echo "builder unexpectedly accepted an AOSP tree without .repo metadata" >&2
    exit 1
fi
grep -q 'missing .repo metadata' "$TMP/metadata.log"
[ ! -e "$REPO_CALL_LOG" ] || { echo "builder invoked repo despite missing .repo metadata" >&2; exit 1; }

echo "AOSP builder preflight tests passed (no unnecessary build and pinned metadata required)"
