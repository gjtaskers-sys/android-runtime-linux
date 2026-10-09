#!/bin/sh
set -eu

if [ "$(id -u)" -ne 0 ]; then
    exec sudo "$0" "$@"
fi

modprobe binder_linux devices="binder,hwbinder,vndbinder" 2>/dev/null || true

missing=0
for node in /dev/binder /dev/hwbinder /dev/vndbinder; do
    if [ ! -e "$node" ]; then
        echo "missing $node" >&2
        missing=1
    else
        chmod 0660 "$node"
    fi
done

if [ "$missing" -ne 0 ]; then
    echo "Binder device nodes were not created by binder_linux." >&2
    echo "Your kernel may be using binderfs; binderfs support will be added to the" >&2
    echo "host helper after the standard-node path is validated." >&2
    exit 1
fi

echo "Android Runtime binder devices are ready."
