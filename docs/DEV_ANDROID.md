# Development Android emulator

The Android SDK Emulator is a temporary backend for developing and testing the host CLI's APK and device-management workflow. It is not the intended production execution path and does not validate the custom LXC Android image or real Android-to-X11 graphics integration.

## Start the development device

```bash
./scripts/android/bootstrap-dev-android.sh
./scripts/android/run-dev-android.sh
```

Build the host runtime before using the CLI. `cargo test --workspace` builds test executables but may not create the standalone binary at `target/debug/android-runtime`.

```bash
cargo build --workspace
./target/debug/android-runtime devices
```

Use the device serial printed by `devices` for commands below.

## Inspect, plan, install, and launch an APK

```bash
./target/debug/android-runtime apk-info /path/to/app.apk
./target/debug/android-runtime plan /path/to/app.apk
./target/debug/android-runtime install /path/to/app.apk --serial emulator-5554
```

The CLI extracts the package name from the binary Android manifest, selects an online device, installs the APK using the available Android CLI with an ADB fallback, and launches the package. A convenience script is also available:

```bash
./scripts/android/install-dev-apk.sh /path/to/app.apk
```

Other lifecycle commands include:

```bash
./target/debug/android-runtime launch com.example.app --serial emulator-5554
./target/debug/android-runtime stop-app com.example.app --serial emulator-5554
./target/debug/android-runtime uninstall com.example.app --serial emulator-5554
```

**Caution:** uninstalling an app removes its local app data.

## What has been validated

The inspect/install/launch workflow has been exercised with the 2048 APK on an API 36 development AVD, and the game was confirmed playable. This validates the CLI-to-ADB/PackageManager workflow against a development Android system. It does not validate the custom Android image, LXC boot, or native X11 presentation of Android-generated frames.

The intended future flow is:

```text
APK -> manifest inspection -> execution policy -> device discovery
    -> install -> package launch
```

Keeping these commands independent of the backend is a design goal; replacing the emulator with the LXC runtime remains future work.
