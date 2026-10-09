# Android Runtime for Linux

**Status: experimental development preview — not production-ready.**

Android Runtime for Linux is an early-stage project exploring how Android applications could run in native Linux desktop windows. The current reference host is Linux Mint XFCE/X11; Debian and Ubuntu are intended targets, but broader compatibility has not yet been established.

The prototype combines a Rust host CLI, APK inspection and ADB management, a versioned display protocol, a native X11/XShm display helper, and tooling for a future LXC-based Android userspace. The custom Android container has not yet been demonstrated booting end to end, and real Android-rendered frames have not yet been verified in the X11 window.

> **Safety notice:** This is experimental software, not a production sandbox for untrusted APKs. Do not expose sensitive host data to Android applications through this prototype.

## Current status

### Verified functionality

The following results have been reproduced in the reference development environment:

- The Rust workspace builds; formatting and strict Clippy checks pass; all **27 workspace unit tests** pass.
- The CLI inspected an APK, installed it on the Android SDK development emulator through ADB, and launched 2048. The game was confirmed playable in that emulator.
- The native X11 helper receives synthetic frames over a Unix socket and displays a smooth RGB gradient. The test reads pixels back from the X11 window to verify color values and channel orientation.
- The native display smoke test sends eight changing frames and checks acknowledgement sequencing, timeouts, socket ownership, active-server protection, window close handling, and cleanup.
- Focused tests cover frame metadata, mocked DMA-BUF CPU-access synchronization, acquire-fence waiting, rootfs validation, safe assembly, AOSP builder preflight, and rootfs packaging.

These results validate host-side components and the development-emulator workflow. They **do not** establish that the custom Android userspace boots in LXC or that real Android graphics reach the X11 window.

### Not yet verified

- A complete AOSP product image built for this project.
- Android booting and reaching service readiness inside the project's LXC configuration.
- End-to-end presentation of real SurfaceFlinger/Composer3 client-target buffers in X11.
- Secure production isolation, installer lifecycle, recovery behavior, and a supported distribution/kernel matrix.
- Seamless per-app Linux windows, Wayland support, audio/camera integration, or a redistributable ARM native-translation path.
- A distributable Android system image or stable end-user release.

See the [roadmap](ROADMAP.md) for current priorities and ways to contribute, and [the architecture overview](docs/ARCHITECTURE.md) for the proposed design.

## Repository contents

- Rust workspace for APK inspection, ABI/package handling, device selection, runtime policy, CLI operations, and display-protocol definitions.
- Native C X11/XShm display helper with synthetic-frame, pixel-readback, and lifecycle tests.
- Android-side host-display integration scaffolding and an AOSP product overlay.
- Scripts for development-emulator setup, Android image/rootfs validation, safe assembly, contributor artifact packaging, and host-readiness diagnostics.
- CI for Rust checks, native X11 tests under Xvfb, and rootfs/preflight fixtures.

## Quick start: host checks

The project uses the Rust stable toolchain declared in `rust-toolchain.toml`. Install Rust with `rustup`, including `rustfmt` and `clippy`, and install the native test dependencies on Debian, Ubuntu, or Linux Mint:

```bash
sudo apt update
sudo apt install build-essential pkg-config libx11-dev libxext-dev xvfb xauth zstd
```

Run the Rust quality gates:

```bash
cargo fmt --all
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace
```

Run the focused native and packaging tests:

```bash
./scripts/test-frame-validation.sh
./scripts/test-dma-buf-sync.sh
./scripts/test-acquire-fence-wait.sh
./scripts/build-x11-display.sh
./scripts/test-x11-display.sh
./scripts/test-rootfs-validation.sh
./scripts/test-rootfs-assembly-safety.sh
./scripts/test-aosp-builder-preflight.sh
sudo ./scripts/test-rootfs-package.sh
./scripts/check-publication-hygiene.sh
```

`test-x11-display.sh` normally uses an active X11 session. For a headless host, run:

```bash
xvfb-run -a -s "-screen 0 1600x1400x24" ./scripts/test-x11-display.sh
```

The display test uses synthetic frames. A passing result verifies the tested host-side display path; it is not an Android graphics integration test.

## Development Android emulator

An Android SDK Virtual Device provides a temporary backend for APK-management work. This does not require building AOSP and is not the intended production execution path.

```bash
./scripts/android/bootstrap-dev-android.sh
./scripts/android/run-dev-android.sh
cargo build --workspace
./target/debug/android-runtime devices
./target/debug/android-runtime apk-info /path/to/app.apk
./target/debug/android-runtime install /path/to/app.apk --serial emulator-5554
```

Use the serial printed by `devices` if it differs. The inspect/install/launch workflow was verified with 2048 on the development emulator. See [development emulator instructions](docs/DEV_ANDROID.md) and the [APK compatibility test plan](docs/TEST_APKS.md).

## Android image work

Most contributors do **not** need to sync or build AOSP. The Android platform build is a separate workload for a machine with substantial RAM and free storage. Host-runtime improvements, Rust work, protocol tests, documentation, issue triage, and display-helper testing can all proceed without a full AOSP checkout.

Contributors with suitable build hardware can follow the [build guide](docs/BUILDING.md) and [image handoff guide](docs/CONTRIBUTOR_IMAGE_HANDOFF.md) to produce and document a candidate Android root filesystem. A packaged rootfs is only a candidate: structure validation does not prove that it boots successfully in LXC.

Host readiness checks are read-only and do not start a container or build AOSP:

```bash
./scripts/android/check-container-readiness.sh
./scripts/android/check-container-readiness.sh --rootfs /path/to/rootfs
```

## Security and licensing

Container isolation, mount policy, Binder/GPU access, APK threat modeling, and recovery behavior require further review. Report suspected vulnerabilities using the process in [`SECURITY.md`](SECURITY.md), not in a public issue with exploit details.

Project-authored source is licensed under Apache-2.0; see [`LICENSE`](LICENSE). The project does not bundle an AOSP checkout, Android system image, APK, Google Play image, firmware, or proprietary ARM translator. Third-party components and future image artifacts require their own provenance and redistribution review.

## Contributing

Contributions are welcome in testing, bug reproduction, documentation, architecture review, Rust/C development, Linux container behavior, Android graphics, and image validation. A full AOSP build is not required for ordinary host-runtime contributions.

Start with [`CONTRIBUTING.md`](CONTRIBUTING.md), review the [roadmap](ROADMAP.md), and choose a task that matches your environment. Please report only behavior that has actually been tested, and keep personal paths, serial numbers, credentials, and private logs out of public issues.

- [Roadmap](ROADMAP.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Design rationale and references](docs/DESIGN_RATIONALE.md)
- [Build instructions](docs/BUILDING.md)
- [Android image handoff](docs/CONTRIBUTOR_IMAGE_HANDOFF.md)
- [Security policy](SECURITY.md)
- [Support](SUPPORT.md)
