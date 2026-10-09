# Security policy

## Current security status

Android Runtime is experimental and is not yet a production security boundary
for untrusted APKs. Container confinement, privileged host operations, mount
policy, binder/GPU exposure, ADB configuration, and failure recovery remain
under development. Do not expose sensitive host data to Android apps through
this prototype.

## Reporting a vulnerability

Please do not post exploit details, proof-of-concept payloads, credentials, or
sensitive logs in a public issue.

Preferred channel: use GitHub's **Report a vulnerability** / private vulnerability
reporting feature for this repository if it is enabled. The repository owner
should enable that feature in GitHub's Security settings before inviting public
use. If it is not available, use the maintainer contact information on the
GitHub profile and ask for a private reporting channel; do not send secrets in
public discussions.

Include the affected revision, host distribution/kernel where relevant, impact,
reproduction steps that do not expose other users' data, and any mitigation you
know. Please allow maintainers reasonable time to investigate and coordinate a
fix before public disclosure.

## Supported versions

There are currently no stable releases. Security reports against the latest
public source preview are welcome, but no response-time or patch-time guarantee
is offered for this experimental project.
