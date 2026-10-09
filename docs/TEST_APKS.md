# APK compatibility test plan

This document tracks representative applications for validating the runtime as functionality becomes available. Results must identify the backend used: a development emulator, a custom LXC Android image, or a synthetic host-display test. These are different levels of evidence and must not be conflated.

## Validated with the development emulator

### 2048 Open Fun Game

Package: `org.andstatus.game2048`

The CLI's APK inspection, installation, and launch workflow has been tested with 2048 on the Android SDK development emulator, and gameplay was confirmed. This is an APK/ADB workflow test only; it does not show the app running inside the custom LXC container or sending real Android frames to the native X11 window.

Project page: https://f-droid.org/en/packages/org.andstatus.game2048/

## Candidate applications for future testing

### K-9 Mail

Useful after container boot, storage, networking, and lifecycle support are in place. Record the exact release, APK ABI, Android version, and backend for every result.

Project page: https://f-droid.org/en/packages/com.fsck.k9/

### Termux

Useful for architecture-specific native-code testing. Use a specific ABI APK when testing translation or ABI selection; a universal APK alone does not establish that each ABI works.

Project releases: https://github.com/termux/termux-app/releases

### VLC

A later stress test for video, audio, GPU/media integration, and filesystem access. It should not be used as an early bring-up requirement.

Project page: https://f-droid.org/en/packages/org.videolan.vlc/

## Reporting a compatibility result

For each test, record:

- application name, version, package, APK source, and checksum (do not upload the APK unless redistribution is permitted);
- host distribution, kernel, architecture, and display session;
- Android backend/image revision and source manifest when applicable;
- APK ABI/split information;
- install, launch, graphics, input, storage, networking, and lifecycle outcome;
- sanitized logs and exact reproduction commands.

Keep emulator-only results separate from LXC results. After changing the manifest parser, run the focused test suite:

```bash
cargo test -p android-apk-inspect
```
