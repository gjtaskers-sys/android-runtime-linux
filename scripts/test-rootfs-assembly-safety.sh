#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd -P)
TMP=$(mktemp -d)
cleanup() { rm -rf "$TMP"; }
trap cleanup EXIT INT TERM
PRODUCT="$TMP/product-out"
ROOTFS="$TMP/rootfs"
mkdir -p "$PRODUCT/root" "$ROOTFS"
printf '%s\n' 'keep-me' > "$ROOTFS/sentinel.txt"

# Existing output is protected unless replacement is explicit.
if "$ROOT/scripts/android/assemble-rootfs.sh" "$PRODUCT" "$ROOTFS" > "$TMP/existing.log" 2>&1; then
    echo "assembler unexpectedly replaced an existing rootfs without --replace" >&2
    exit 1
fi
grep -q 'ROOTFS already exists' "$TMP/existing.log"
grep -q 'keep-me' "$ROOTFS/sentinel.txt"

# Even with --replace, a missing AOSP partition must fail before touching output.
if "$ROOT/scripts/android/assemble-rootfs.sh" --replace "$PRODUCT" "$ROOTFS" > "$TMP/missing-image.log" 2>&1; then
    echo "assembler unexpectedly accepted missing AOSP partition images" >&2
    exit 1
fi
grep -q 'required product image is missing or empty' "$TMP/missing-image.log"
grep -q 'keep-me' "$ROOTFS/sentinel.txt"

# A destination that would contain the source build tree must be refused.
if "$ROOT/scripts/android/assemble-rootfs.sh" --replace "$PRODUCT" "$TMP" > "$TMP/overlap.log" 2>&1; then
    echo "assembler unexpectedly accepted overlapping source and destination paths" >&2
    exit 1
fi
grep -q 'ROOTFS must not contain AOSP PRODUCT_OUT' "$TMP/overlap.log"
grep -q 'keep-me' "$ROOTFS/sentinel.txt"

if "$ROOT/scripts/android/assemble-rootfs.sh" --replace "$PRODUCT" / > "$TMP/root-target.log" 2>&1; then
    echo "assembler unexpectedly accepted / as ROOTFS" >&2
    exit 1
fi
grep -q 'refusing to use / as ROOTFS' "$TMP/root-target.log"
grep -q 'keep-me' "$ROOTFS/sentinel.txt"

echo "rootfs assembly safety tests passed (existing output preserved; missing partitions and overlapping paths rejected)"
