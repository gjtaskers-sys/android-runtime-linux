# Building and testing

## Host development dependencies

For the current host-side work on Ubuntu 24.04, Debian 13, or Linux Mint 22.x, install:

```bash
sudo apt update
sudo apt install build-essential pkg-config libx11-dev libxext-dev xvfb xauth lxc adb repo git python3 curl zstd
```

The project uses Rust for orchestration and a small C helper for the initial X11/XShm display backend. Install the stable Rust toolchain specified by `rust-toolchain.toml`, including `rustfmt` and `clippy`.

## Build the host workspace

```bash
cargo fmt --all
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace
```

## Build and test the X11 bridge

```bash
./scripts/build-x11-display.sh
./scripts/test-x11-display.sh
```

The smoke test uses an active X11 session. On a headless test host, run:

```bash
xvfb-run -a -s "-screen 0 1600x1400x24" ./scripts/test-x11-display.sh
```

The test sends synthetic frames and verifies displayed pixel samples. It does not test real Android graphics buffers.

## Development Android backend

The development-emulator harness supports APK and device-management work without an AOSP checkout:

```bash
./scripts/android/bootstrap-dev-android.sh
./scripts/android/run-dev-android.sh
```

See [`DEV_ANDROID.md`](DEV_ANDROID.md) for CLI commands and the scope of the emulator test.

## Android platform image

Building the proposed Android product from AOSP is a separate task requiring substantially more RAM and disk space than host development. Most contributors should use the host test suite. A contributor with suitable build hardware can follow [`CONTRIBUTOR_IMAGE_HANDOFF.md`](CONTRIBUTOR_IMAGE_HANDOFF.md) to create a candidate image with pinned source metadata and validation results.

There is currently no Android system image that has been demonstrated booting end to end in the project's LXC configuration. Do not treat successful rootfs assembly or validation as proof of a working Android runtime.
