#!/bin/sh
set -eu
PROJECT_ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
APK=${1:-}
[ -n "$APK" ] || { echo "usage: $0 /path/to/app.apk" >&2; exit 2; }
[ -f "$APK" ] || { echo "APK not found: $APK" >&2; exit 1; }

# Prefer our real runtime CLI so this exercises the same APK inspection,
# package identification, device selection and launch behavior as the product.
if [ -x "$PROJECT_ROOT/target/debug/android-runtime" ]; then
  exec "$PROJECT_ROOT/target/debug/android-runtime" install "$APK"
fi

echo "Project binary is not built; run ./scripts/dev-run.sh first." >&2
exit 1
