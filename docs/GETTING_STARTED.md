# Getting started

This guide is for people who want to try the currently testable pieces or help with the project without building Android from source.

## What can I try?

At present there are two separate test paths:

- **Host display test:** opens a native X11 window and shows a generated color gradient. It tests the host display bridge, not Android graphics.
- **APK workflow:** uses the Android SDK development emulator to inspect, install, and launch an APK through the CLI. The 2048 workflow has been tested on the emulator.

The custom Android userspace has not yet been demonstrated booting in LXC. Neither path should be described as a finished Android desktop runtime.

## Host display test

Use Linux Mint XFCE/X11, Ubuntu, or Debian with the X11 development tools installed. Install the host packages:

```bash
sudo apt update
sudo apt install build-essential pkg-config libx11-dev libxext-dev xvfb xauth
```

Install Rust with [rustup](https://rustup.rs/) and use the toolchain in `rust-toolchain.toml`.

From the project root:

```bash
cargo build --workspace
./scripts/build-x11-display.sh
./scripts/test-x11-display.sh
```

The smoke test opens a window, sends eight changing synthetic frames, checks a few pixels, and exercises acknowledgement and window/socket cleanup. For a headless system, run:

```bash
xvfb-run -a -s "-screen 0 1600x1400x24" ./scripts/test-x11-display.sh
```

## APK test with the Android SDK emulator

This optional test needs more disk space because it downloads Android SDK components and an emulator image. It does not download or build AOSP.

```bash
./scripts/android/bootstrap-dev-android.sh
./scripts/android/run-dev-android.sh
cargo build --workspace
./target/debug/android-runtime devices
```

Find the path to an APK you are allowed to use and trust. Inspect it first:

```bash
./target/debug/android-runtime apk-info /path/to/app.apk
```

Install it, replacing `emulator-5554` if `devices` printed a different serial:

```bash
./target/debug/android-runtime install /path/to/app.apk --serial emulator-5554
```

Only use APKs from sources you trust. The prototype does not yet provide a production-grade sandbox for malicious apps.

## Full test suite

```bash
cargo fmt --all
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo build --workspace --locked

./scripts/test-frame-validation.sh
./scripts/test-dma-buf-sync.sh
./scripts/test-acquire-fence-wait.sh
./scripts/test-rootfs-validation.sh
./scripts/test-rootfs-assembly-safety.sh
./scripts/test-aosp-builder-preflight.sh
./scripts/build-x11-display.sh
./scripts/test-x11-display.sh
```

Rootfs packaging tests additionally require `zstd` and root privileges. See [`BUILDING.md`](BUILDING.md) for host requirements.

## Need help?

Open a [bug report](https://github.com/gjtaskers-sys/android-runtime-linux/issues/new/choose) with your distribution/version, display session (X11 or Wayland), Rust version, command, and sanitized error output. Never post access tokens, private keys, APKs, system images, or personal paths in public issues. Security concerns belong in [`SECURITY.md`](../SECURITY.md).
