#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cxx=${CXX:-c++}
out=$(mktemp "${TMPDIR:-/tmp}/android-runtime-acquire-fence.XXXXXX")
trap 'rm -f "$out"' EXIT HUP INT TERM
"$cxx" -O2 -Wall -Wextra -Wpedantic -Werror -std=c++17 \
  -I "$ROOT/android/hardware/host_display" \
  "$ROOT/android/hardware/host_display/AcquireFenceWaitTest.cpp" -o "$out"
"$out"
