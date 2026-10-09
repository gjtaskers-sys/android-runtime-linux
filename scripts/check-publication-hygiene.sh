#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

fail() { printf 'publication hygiene check failed: %s\n' "$*" >&2; exit 1; }

# The full Apache-2.0 text is about 11 KiB; catch accidentally abbreviated copies.
[[ -f LICENSE ]] || fail "LICENSE is missing"
license_bytes="$(wc -c < LICENSE | tr -d '[:space:]')"
(( license_bytes >= 10000 )) || fail "LICENSE is unexpectedly short (${license_bytes} bytes); include the full Apache-2.0 text"
grep -q 'END OF TERMS AND CONDITIONS' LICENSE || fail "LICENSE does not look like the complete Apache-2.0 text"

for required in README.md ROADMAP.md CONTRIBUTING.md CODE_OF_CONDUCT.md SECURITY.md SUPPORT.md Cargo.lock .github/workflows/ci.yml .github/PULL_REQUEST_TEMPLATE.md .github/ISSUE_TEMPLATE/bug_report.yml .github/ISSUE_TEMPLATE/feature_request.yml .github/ISSUE_TEMPLATE/compatibility_report.yml assets/demo/2048-android-sdk-emulator.mp4 assets/demo/2048-android-sdk-emulator-preview.gif assets/demo/x11-synthetic-gradient.mp4 assets/demo/x11-synthetic-gradient-preview.gif; do
  [[ -s "$required" ]] || fail "required public-project file is missing or empty: $required"
done

# Large or proprietary artifacts belong in a reviewed release channel, not git.
forbidden_artifacts="$(find . -type f \
  \( -iname '*.apk' -o -iname '*.apks' -o -iname '*.aab' \
     -o -iname '*.tar.zst' -o -iname '*.qcow2' -o -iname '*.vmdk' \
     -o -iname '*.img' -o -iname '*.iso' -o -iname '*.pem' \
     -o -iname '*.p12' -o -iname '*.jks' -o -iname '*.keystore' \) \
  -not -path './.git/*' -not -path './target/*' -print)"
[[ -z "$forbidden_artifacts" ]] || {
  printf 'Unexpected artifact/credential-like files found:\n%s\n' "$forbidden_artifacts" >&2
  fail "remove generated images, APKs, key stores, or other non-source artifacts before publishing"
}

large_files="$(find . -type f -size +50M -not -path './.git/*' -not -path './target/*' -print)"
[[ -z "$large_files" ]] || {
  printf 'Files larger than 50 MiB found:\n%s\n' "$large_files" >&2
  fail "review large files before publishing"
}

# Scan common private-key and token formats. Exclude this script because its
# own regex literals intentionally contain the token signatures being checked.
if grep -RIlE \
  --exclude='check-publication-hygiene.sh' \
  --exclude-dir=.git --exclude-dir=target --exclude-dir=__pycache__ \
  --binary-files=without-match -- \
  '-----BEGIN (RSA |EC |OPENSSH |DSA )?PRIVATE KEY-----|gh[pousr]_[A-Za-z0-9_]{20,}|AKIA[0-9A-Z]{16}' .; then
  fail "possible private key or credential token found; investigate before publishing"
else
  grep_status=$?
  [[ $grep_status -eq 1 ]] || fail "credential scan failed with grep exit status $grep_status"
fi

# Public-facing documentation must not contain local user paths or private
# handoff notes from the development process. Keep these checks focused on docs
# rather than source code, where explicit example paths may be useful.
public_doc_pattern='(/home/[[:alnum:]_.-]+/|~/Downloads/|next[0-9]+work|project owner|owner.s Linux Mint|not yet uploaded to GitHub|before the first push|the owner has verified)'
for public_doc in README.md ROADMAP.md CONTRIBUTING.md CODE_OF_CONDUCT.md SECURITY.md SUPPORT.md docs/*.md; do
  if grep -nIE "$public_doc_pattern" "$public_doc"; then
    fail "public documentation contains a local development path or private handoff wording"
  else
    grep_status=$?
    [[ $grep_status -eq 1 ]] || fail "documentation scan failed with grep exit status $grep_status"
  fi
done
[[ ! -e docs/PUBLICATION_CHECKLIST.md ]] || fail "remove the one-time first-publication checklist from the public package"
[[ ! -e docs/RECORDING_DEMOS.md ]] || fail "remove the creator-only recording tutorial; public demo media belongs in assets/demo/"
[[ ! -e docs/PROJECT_PAGE.md ]] || fail "remove the one-time project-page setup notes from the public package"

printf 'publication hygiene checks passed (license, artifacts, credentials, file sizes, and public-facing documentation)\n'
