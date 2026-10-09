#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)
DEST=${1:-$HOME/android-runtime-aosp}
BRANCH=${ANDROID_RELEASE_BRANCH:-android-latest-release}

if ! command -v repo >/dev/null 2>&1; then
    cat >&2 <<EOF
The Android Repo tool is required but was not found in PATH.

On Debian/Ubuntu/Linux Mint, the simplest installation is:
  sudo apt update
  sudo apt install repo

The official git-repo project also supports a per-user install:
  mkdir -p "\$HOME/.bin"
  curl https://storage.googleapis.com/git-repo-downloads/repo -o "\$HOME/.bin/repo"
  chmod a+rx "\$HOME/.bin/repo"
  export PATH="\$HOME/.bin:\$PATH"

Then verify with:
  repo --version
EOF
    exit 1
fi

for cmd in git python3; do
    if ! command -v "$cmd" >/dev/null 2>&1; then
        echo "Required command '$cmd' was not found in PATH." >&2
        exit 1
    fi
done

if ! git config --get user.name >/dev/null 2>&1 || ! git config --get user.email >/dev/null 2>&1; then
    cat >&2 <<EOF
Git needs a user name and email before Repo can initialize the AOSP checkout.
This is required by Repo/Git even though you are not publishing an AOSP commit.

Set them once with:
  git config --global user.name "Your Name"
  git config --global user.email "you@example.com"

You can use a GitHub noreply address for the email if you prefer not to
associate your normal email address with Git commits.
EOF
    exit 1
fi

mkdir -p "$DEST"
cd "$DEST"

# repo init is intentionally idempotent here.  If an earlier invocation
# created .repo but failed during identity setup, rerunning init repairs the
# incomplete checkout instead of incorrectly skipping initialization.
repo init \
    -u https://android.googlesource.com/platform/manifest \
    -b "$BRANCH" \
    --partial-clone \
    --no-use-superproject \
    --clone-filter=blob:limit=10M

printf '%s\n' "AOSP manifest initialized at $DEST"
printf '%s\n' "Release branch: $BRANCH"
printf '%s\n' "No source sync was started. Run scripts/android/sync-aosp.sh explicitly on a build machine."
