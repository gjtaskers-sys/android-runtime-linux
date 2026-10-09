# Design rationale and references

This document summarizes the main architectural choices and the upstream material used to evaluate them. These references inform the design; they are not project dependencies unless a dependency is explicitly declared in the build files.

## Execution model

The current proposal is an Android userspace running in an LXC container that shares the host Linux kernel. This avoids introducing a guest kernel for the primary path, but it makes kernel configuration, Binder availability, namespaces/cgroups, device access, and confinement part of the compatibility problem. A virtual-machine backend remains a possible future fallback if container constraints prove incompatible with the required security or application behavior.

LXC documentation: https://linuxcontainers.org/lxc/

## Existing Android-on-Linux projects

- **Waydroid** is relevant prior art for container-based Android, host integration, and graphics composition. Its implementation is worth comparing during boot and graphics bring-up, but it is not a runtime dependency of this project. https://github.com/waydroid/waydroid
- **redroid** is relevant prior art for running Android userspace in Linux container environments and for image/build operations. Its main focus differs from a single-user desktop window workflow. https://github.com/remote-android/redroid-doc
- **Anbox** is historical prior art for the split between an Android container and a host session manager. Its architecture can inform design review, but its current maintenance status should be checked before considering reuse. https://github.com/anbox/anbox
- **Cuttlefish and Ranchu** provide reference implementations for Android virtual-device configurations and graphics/HWC integration. https://source.android.com/docs/devices/cuttlefish

Review each project's current license, implementation, maintenance status, and integration contract before copying or adapting code.

## Graphics boundary

Android's graphics stack already composes application and system layers through SurfaceFlinger and a Hardware Composer interface. The initial design exports one final client-target buffer rather than trying to turn each Android layer into an independent desktop window. This keeps the first host display boundary smaller, while leaving per-app window mapping as a later design problem.

- Android graphics architecture: https://source.android.com/docs/core/graphics/architecture
- Android AIDL Hardware Composer: https://source.android.com/docs/core/graphics/aidl-hwc
- AOSP Ranchu HWC3 reference: https://android.googlesource.com/device/generic/goldfish-opengl/+/refs/heads/main/system/hwc3/

The current host X11 helper first uses a CPU-copy/XShm path. Before real Android frames can be considered supported, the built image must verify actual buffer format, modifier, planes, file descriptors, offsets, strides, dimensions, fence semantics, and buffer lifetime. Protocol mocks and synthetic gradient tests cannot prove those properties for a real allocator.

## DMA-BUF CPU synchronization

Linux DMA-BUF supports sharing buffer file descriptors across subsystems and provides synchronization primitives for asynchronous access. When userspace accesses a DMA-BUF mapping from the CPU, it must bracket CPU access with the appropriate `DMA_BUF_IOCTL_SYNC` START/END operations; those operations provide cache-coherency hooks and do not replace waiting for device work to finish.

Kernel documentation: https://docs.kernel.org/driver-api/dma-buf.html

## Foreign-architecture native libraries

AOSP's Native Bridge interface allows Android runtime components to integrate an implementation for loading foreign-ISA native libraries. The interface is not itself a translator. Any translation implementation requires a separate review of ABI compatibility, source provenance, license terms, and redistribution rights.

AOSP Native Bridge source: https://android.googlesource.com/platform/art/+/refs/heads/main/libnativebridge/

## What remains a hypothesis

The intended Android version, container configuration, graphics allocation path, and desktop integration must be validated against a reproducible image and a documented host/kernel matrix. No design choice should be treated as complete until the corresponding boot, real-frame, lifecycle, and security tests have evidence.
