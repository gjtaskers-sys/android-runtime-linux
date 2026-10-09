# Architecture overview

## Status

This is the intended architecture, partially implemented. The Rust host CLI, APK/ADB development-emulator workflow, and synthetic-frame X11 bridge have been tested independently. Booting the project's Android userspace in LXC and presenting real Android-generated frames in X11 are not yet proven.

## Goal

Run Android applications as Linux desktop applications, with APK installation as the primary workflow and native X11 support as the first display backend. Linux Mint XFCE/X11 is the reference environment; Debian and Ubuntu are intended targets, not yet a tested support matrix.

## Execution model

The current architecture hypothesis is an Android userspace in an LXC container sharing the host kernel. A virtual machine remains a possible compatibility fallback if the container path cannot meet compatibility or isolation requirements. Neither execution model should be treated as complete until it passes its boot, security, and application tests.

### Container path

`Android userspace -> Linux kernel -> host graphics/device interfaces -> host display bridge`

The project uses LXC rather than implementing a new container runtime. Android-specific configuration and desktop integration belong in this project; namespace, cgroup, and core container primitives should remain with the established container stack. See the [LXC project](https://linuxcontainers.org/lxc/).

### Foreign-architecture applications

A future Native Bridge integration could allow Android to load foreign-ISA native libraries in the container. AOSP defines the interface but does not itself supply a translator. Any implementation requires independent ABI testing, source-provenance review, and licensing/redistribution review. An optional VM is also under consideration as a compatibility fallback.

## Android image

The initial image target is AOSP Android 17 x86_64. The image is intended to behave as an Android appliance rather than a general-purpose phone ROM, with desktop-oriented defaults and a controlled management interface. A built image and its service behavior have not yet been verified.

The normal host development workflow should not require syncing or building the full Android platform. The project expects an image builder or release process to provide prebuilt candidate images with pinned source metadata, validation results, and checksums.

## Display architecture

### Initial mode: one composed Android screen per X11 window

Android should compose application and system layers through SurfaceFlinger and Composer3. The initial host boundary is one final client-target buffer per frame, rather than one window per Android layer or app. This keeps the first display integration small; seamless per-app windows are a later design problem.

The host protocol transfers frame metadata and a buffer file descriptor over a private Unix socket. The current native helper uses a CPU-copy/XShm path so host-side presentation can be tested independently from EGL/DMA-BUF import. The protocol and synthetic-frame path have been tested, but actual Android buffer export and presentation remain unverified.

The current host-side frame contract includes dimensions, row stride, format flags, sequence acknowledgements, acquire-fence waiting on the Android side, CPU-access synchronization for DMA-BUF reads, and an XShm completion barrier before a frame is acknowledged as consumed. These testable contracts do not establish that a real Android allocator produces a compatible buffer.

Useful references:

- [Android graphics architecture](https://source.android.com/docs/core/graphics/architecture)
- [Android AIDL Hardware Composer](https://source.android.com/docs/core/graphics/aidl-hwc)
- [DMA-BUF sharing and synchronization](https://docs.kernel.org/driver-api/dma-buf.html)

## Performance strategy

The first real-frame milestone should use a known supported linear CPU-readable format and a copy fallback. Direct DMA-BUF/EGL import can be added only after real buffer metadata, fences, and lifetime are correct. Performance claims should include measurements from named hardware and software versions.

## Security model

Android is a system-like workload with access needs that may include Binder, DRM render nodes, networking, audio, and the host display socket. Production confinement requires an explicit LXC privilege profile, narrow mounts and device access, AppArmor/seccomp policy, ADB configuration, APK threat model, and crash/recovery behavior. Unprivileged-container support is a goal, not a verified capability.

## Intended host scope

The first intended distribution family is Debian-based Linux, with Linux Mint XFCE/X11 as the reference desktop. Initial targets include Ubuntu LTS, Debian stable, and Linux Mint releases; exact supported versions and kernel combinations must be established through testing before an alpha release.

## Non-goals for the first usable milestone

- seamless per-Android-app windows;
- Google Play Services dependency;
- camera or Bluetooth emulation;
- multiple Android instances;
- arbitrary Linux distribution support;
- root access by default.

The first meaningful end-to-end milestone is: start Android on demand, install an APK, and display the app in a native Linux window on a supported host.
