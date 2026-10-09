#!/bin/sh
set -eu

DEST=${1:-$HOME/android-runtime-aosp}
JOBS=${JOBS:-$(nproc)}

if ! command -v repo >/dev/null 2>&1; then
    echo "The Android Repo tool is required but was not found in PATH." >&2
    exit 1
fi

if [ ! -d "$DEST/.repo" ]; then
    echo "AOSP checkout is not initialized at: $DEST" >&2
    echo "Run scripts/android/bootstrap-aosp.sh first." >&2
    exit 1
fi

case "$JOBS" in
    ''|*[!0-9]*) echo "JOBS must be a positive integer." >&2; exit 2 ;;
esac

if [ "$JOBS" -lt 1 ]; then
    echo "JOBS must be at least 1." >&2
    exit 2
fi

cd "$DEST"
echo "WARNING: full AOSP sync can require hundreds of GB of disk space." >&2
echo "This should normally be run on a dedicated build machine, not the target laptop." >&2
echo "Press Ctrl+C now to abort." >&2
printf 'Continuing in 5 seconds...\n' >&2
sleep 5

repo sync -c -j"$JOBS" --no-tags --optimized-fetch
printf '%s\n' "AOSP source sync completed at $DEST"
