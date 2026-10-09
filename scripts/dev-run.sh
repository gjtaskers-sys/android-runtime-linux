#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"

if ! command -v cargo >/dev/null 2>&1; then
    echo "cargo is required. On Debian/Ubuntu/Mint: sudo apt install cargo rustc" >&2
    exit 1
fi

cargo build --workspace

if command -v cc >/dev/null 2>&1; then
    ./scripts/build-x11-display.sh
fi

echo
./target/debug/android-runtime doctor

echo
echo "Host-side runtime is built."
echo "For the current X11 smoke test, run in another terminal:"
echo "  ./android-runtime-display-x11"
echo "  ./android-runtime-frame-smoke"
echo "Both the X11 helper and frame smoke use the same XDG_RUNTIME_DIR socket; set ANDROID_RUNTIME_DISPLAY_SOCKET to override it."
echo
echo "The production Android image is not connected yet. For development APK testing, start the SDK AVD with:"
echo "  ./scripts/android/run-dev-android.sh"
echo "Then list devices and install an APK with:"
echo "  ./target/debug/android-runtime devices"
echo "  ./target/debug/android-runtime install /path/to/app.apk"
