#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"

if [ -z "${DISPLAY:-}" ]; then
    echo "DISPLAY is not set; run this from an X11 session or under xvfb-run." >&2
    exit 2
fi

./scripts/build-x11-display.sh

tmpdir=$(mktemp -d "${TMPDIR:-/tmp}/android-runtime-x11-smoke.XXXXXX")
socket_path="$tmpdir/display.sock"
server_log="$tmpdir/server.log"
server_pid=
status=0

cleanup() {
    status=$?
    if [ -n "$server_pid" ]; then
        kill "$server_pid" 2>/dev/null || true
        wait "$server_pid" 2>/dev/null || true
        server_pid=
    fi
    if [ "$status" -ne 0 ] && [ -f "$server_log" ]; then
        echo "--- X11 display helper log ---" >&2
        cat "$server_log" >&2
    fi
    rm -rf "$tmpdir"
    exit "$status"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

# The smoke sender should reject invalid frame counts before connecting.
if ./android-runtime-frame-smoke "$socket_path" 0 >"$tmpdir/invalid-count.log" 2>&1; then
    echo "frame smoke unexpectedly accepted a zero frame count" >&2
    exit 1
else
    invalid_count_status=$?
    if [ "$invalid_count_status" -ne 2 ]; then
        cat "$tmpdir/invalid-count.log" >&2
        echo "frame smoke returned $invalid_count_status for an invalid frame count; expected 2" >&2
        exit 1
    fi
fi

# A regular file at the chosen path must not be unlinked or overwritten.
printf '%s\n' 'preserve-this-file' > "$socket_path"
if ./android-runtime-display-x11 "$socket_path" >"$tmpdir/regular-path.log" 2>&1; then
    echo "display helper unexpectedly replaced a regular file at its socket path" >&2
    exit 1
fi
if [ ! -f "$socket_path" ] || [ -L "$socket_path" ] ||
   ! grep -qx 'preserve-this-file' "$socket_path"; then
    echo "display helper modified a non-socket path" >&2
    exit 1
fi
rm -f "$socket_path"

./android-runtime-display-x11 "$socket_path" >"$server_log" 2>&1 &
server_pid=$!
tries=0
while [ "$tries" -lt 50 ]; do
    if [ -S "$socket_path" ]; then
        break
    fi
    if ! kill -0 "$server_pid" 2>/dev/null; then
        echo "display helper exited before creating its socket" >&2
        exit 1
    fi
    sleep 0.1
    tries=$((tries + 1))
done
if [ ! -S "$socket_path" ]; then
    echo "timed out waiting for display socket" >&2
    exit 1
fi

# A second helper must refuse to steal the active helper's socket path.
if ./android-runtime-display-x11 "$socket_path" >"$tmpdir/duplicate-path.log" 2>&1; then
    echo "second display helper unexpectedly bound an active socket" >&2
    exit 1
fi
if [ ! -S "$socket_path" ] || ! kill -0 "$server_pid" 2>/dev/null; then
    echo "second display helper disrupted the active helper" >&2
    exit 1
fi

./android-runtime-frame-smoke "$socket_path" 8
./android-runtime-frame-verify "Android Runtime" 8
hold_seconds=${ANDROID_RUNTIME_SMOKE_HOLD_SECONDS:-2}
case "$hold_seconds" in
    ''|*[!0-9]*) echo "ANDROID_RUNTIME_SMOKE_HOLD_SECONDS must be a non-negative integer" >&2; exit 2 ;;
esac
sleep "$hold_seconds"

# A running helper must not unlink a replacement path during shutdown. This
# simulates an external actor replacing the pathname while the listener FD is
# still alive, then requests a graceful WM close without using the socket.
rm -f "$socket_path"
printf '%s\n' 'preserve-after-exit' > "$socket_path"
./android-runtime-window-close "Android Runtime"
tries=0
while kill -0 "$server_pid" 2>/dev/null; do
    if [ "$tries" -ge 50 ]; then
        echo "display helper did not exit after WM_DELETE_WINDOW" >&2
        exit 1
    fi
    sleep 0.1
    tries=$((tries + 1))
done
if wait "$server_pid"; then
    server_pid=
else
    echo "display helper failed during graceful shutdown" >&2
    server_pid=
    exit 1
fi
if [ ! -f "$socket_path" ] || [ -L "$socket_path" ] ||
   ! grep -qx 'preserve-after-exit' "$socket_path"; then
    echo "display helper removed or modified a replacement socket path on exit" >&2
    exit 1
fi
echo "X11 display smoke tests passed (socket safety, active-server protection, 8 frame-completion ACKs, WM close, and safe cleanup)."
