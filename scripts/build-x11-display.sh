#!/bin/sh
set -eu
cc=${CC:-cc}
$cc -O2 -Wall -Wextra -Wpedantic -std=c11 \
  -I display \
  display/x11-display.c \
  -lX11 -lXext \
  -o android-runtime-display-x11

$cc -O2 -Wall -Wextra -Wpedantic -std=c11 \
  -I display \
  display/frame-smoke.c \
  -o android-runtime-frame-smoke

$cc -O2 -Wall -Wextra -Wpedantic -std=c11 \
  display/window-close.c \
  -lX11 \
  -o android-runtime-window-close

$cc -O2 -Wall -Wextra -Wpedantic -Werror -std=c11 \
  display/frame-pixel-verify.c \
  -lX11 \
  -o android-runtime-frame-verify
