#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cc=${CC:-cc}
out=$(mktemp "${TMPDIR:-/tmp}/android-runtime-frame-validate.XXXXXX")
trap 'rm -f "$out"' EXIT HUP INT TERM
"$cc" -O2 -Wall -Wextra -Wpedantic -Werror -std=c11 \
  -I "$ROOT/display" "$ROOT/display/frame-validate-test.c" -o "$out"
"$out"
