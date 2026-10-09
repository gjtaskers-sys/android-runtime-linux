# Contributing

Thanks for considering a contribution. Android Runtime is an experimental
project: focused, reproducible changes are especially helpful while the Android
container and real-frame graphics paths are still being proven.

## Good places to start

- Run and improve the Rust, protocol, X11, and rootfs fixture tests.
- Improve diagnostics, documentation, and reproducibility.
- Review Android graphics-buffer formats, ownership, fences, and sync contracts.
- Help validate or build an AOSP image on suitable hardware and submit the
  source manifest, build notes, validation report, and artifact checksums.

**A full AOSP sync/build is not required for ordinary host-runtime work.** Do
not start one on a limited machine just to run the host test suite.

## Before opening an issue

Search existing issues first. Include the project revision, Linux distribution
and version, display session (X11/Wayland), command executed, exact error, and
relevant sanitized logs. Do not attach APKs, system images, rootfs archives,
private keys, access tokens, or personal data. For possible vulnerabilities,
follow [`SECURITY.md`](SECURITY.md) instead of filing a public issue.

## Development checks

Use the stable Rust toolchain in `rust-toolchain.toml` and install the native
packages described in the README. Before submitting a change, run the relevant
checks; for changes to Rust code, run all of:

```bash
cargo fmt --all
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace
```

For native display, protocol, or image-handoff changes, also run the focused
scripts from the README. Changes to CI or publication files should run:

```bash
./scripts/check-publication-hygiene.sh
```

Do not weaken validation or suppress a lint just to make a check pass. Add a
regression test when changing behavior or fixing a bug. Tests that require a
GPU, Android image, or AOSP checkout should identify that dependency explicitly
and retain a fixture/unit test when possible.

## Pull requests

- Keep changes focused and explain the problem they solve.
- Describe user-visible changes and any architecture/protocol implications.
- Identify tests run and clearly list tests that could not be run.
- Update README/ROADMAP/docs when a capability's verified status changes.
- Never claim real Android-to-X11 frame delivery or LXC boot based only on a
  synthetic frame test or rootfs structure check.
- Do not commit generated Android images, SDK/AVD downloads, APKs, build output,
  logs, or secrets.
- Preserve license and attribution notices. Do not add third-party source,
  firmware, APKs, or binary translators without recording provenance and
  reviewing redistribution terms.

By intentionally submitting a contribution for inclusion, you agree that it
will be distributed under this repository's Apache-2.0 license unless you and
the project maintainers agree otherwise in writing. Submit only work you have
the right to contribute.
