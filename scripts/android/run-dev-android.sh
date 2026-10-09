#!/bin/sh
set -eu
SDK_ROOT=${ANDROID_SDK_ROOT:-$HOME/android-sdk}
AVD_NAME=${ANDROID_RUNTIME_AVD_NAME:-medium_phone}
ANDROID_CLI="$SDK_ROOT/cmdline-tools/latest/bin/android"

[ -x "$ANDROID_CLI" ] || {
    echo "Android CLI is not installed. Run scripts/android/bootstrap-dev-android.sh first." >&2
    exit 1
}

exec "$ANDROID_CLI" --no-metrics --sdk="$SDK_ROOT" emulator start "$AVD_NAME"
