#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
SDK_ROOT=${ANDROID_SDK_ROOT:-$HOME/android-sdk}
TOOLS_VERSION=${ANDROID_CMDLINE_TOOLS_VERSION:-15859902}
API=${ANDROID_DEV_API:-auto}
AVD_NAME=${ANDROID_RUNTIME_AVD_NAME:-medium_phone}

say() { printf '%s\n' "$*"; }
fail() { echo "error: $*" >&2; exit 1; }

# Android command-line tools are Java applications. Prefer the distro OpenJDK
# rather than downloading a second private JDK into the project. Android's
# current tooling is compatible with modern OpenJDK; use 17 as the Debian-family
# baseline for this project.
if ! command -v java >/dev/null 2>&1; then
    say "Java is required by the Android SDK command-line tools."
    if command -v apt-get >/dev/null 2>&1; then
        say "Install it with:"
        say "  sudo apt update && sudo apt install openjdk-17-jdk"
    else
        say "Install OpenJDK 17 using your distribution's package manager."
    fi
    exit 2
fi

JAVA_BIN=$(command -v java)
JAVA_HOME_DETECTED=$(dirname "$(dirname "$(readlink -f "$JAVA_BIN")")")
export JAVA_HOME="${JAVA_HOME:-$JAVA_HOME_DETECTED}"

JAVA_VERSION=$(java -version 2>&1 | sed -n 's/.*version "\([0-9][0-9]*\).*/\1/p' | head -1)
case "$JAVA_VERSION" in
    8|9|10|11|12|13|14|15|16)
        fail "Java $JAVA_VERSION detected. Install OpenJDK 17 or newer: sudo apt install openjdk-17-jdk"
        ;;
esac

mkdir -p "$SDK_ROOT"

if [ ! -x "$SDK_ROOT/cmdline-tools/latest/bin/android" ]; then
    tmp=$(mktemp -d)
    trap 'rm -rf "$tmp"' EXIT
    url="https://dl.google.com/android/repository/commandlinetools-linux-${TOOLS_VERSION}_latest.zip"
    say "Downloading Android command-line tools ${TOOLS_VERSION}..."
    curl -fL "$url" -o "$tmp/tools.zip"
    mkdir -p "$SDK_ROOT/cmdline-tools"
    rm -rf "$SDK_ROOT/cmdline-tools/latest"
    mkdir -p "$tmp/unpack"
    unzip -q "$tmp/tools.zip" -d "$tmp/unpack"
    mv "$tmp/unpack/cmdline-tools" "$SDK_ROOT/cmdline-tools/latest"
fi

ANDROID_CLI="$SDK_ROOT/cmdline-tools/latest/bin/android"

export ANDROID_SDK_ROOT="$SDK_ROOT"
PATH="$SDK_ROOT/platform-tools:$SDK_ROOT/emulator:$SDK_ROOT/cmdline-tools/latest/bin:$PATH"
export PATH

say "Installing Android Emulator and platform tools with Android CLI..."
[ -x "$ANDROID_CLI" ] || fail "Android CLI was not found at $ANDROID_CLI"

# The current Android CLI replaces sdkmanager/avdmanager. We intentionally
# keep the development harness pinned to API 36 for repeatability.
if [ "$API" = auto ]; then
    API=36
fi

# Install the emulator, platform tools, and the x86_64 system image.
"$ANDROID_CLI" --no-metrics --sdk="$SDK_ROOT" sdk install emulator platform-tools
IMAGE="system-images/android-${API}/default/x86_64"
say "Installing development system image: $IMAGE"
"$ANDROID_CLI" --no-metrics --sdk="$SDK_ROOT" sdk install "$IMAGE"

# Android CLI's emulator command creates the AVD from a built-in hardware
# profile and does not use the legacy avdmanager devices.xml database.
# The profile itself becomes the device name, so use medium_phone as the
# stable development harness name.
AVD_NAME=${ANDROID_RUNTIME_AVD_NAME:-medium_phone}
if ! "$ANDROID_CLI" --no-metrics --sdk="$SDK_ROOT" emulator list 2>/dev/null | grep -Fq "$AVD_NAME"; then
    say "Creating Android CLI AVD: $AVD_NAME"
    "$ANDROID_CLI" --no-metrics --sdk="$SDK_ROOT" emulator create "$AVD_NAME"
fi

cat <<EOF2

Android Runtime development Android environment is ready.

SDK: $SDK_ROOT
API: $API
AVD: $AVD_NAME

The development emulator is NOT the production runtime. It is a temporary
way to run real Android apps on this machine while the lightweight LXC image
is developed and the full AOSP build remains off-machine.

Start it with:
  $ROOT/scripts/android/run-dev-android.sh

Install an APK with:
  $ROOT/scripts/android/install-dev-apk.sh /path/to/app.apk
EOF2
