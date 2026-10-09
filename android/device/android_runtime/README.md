# Android Runtime AOSP product

This directory is a small product/device overlay for an AOSP checkout. It is
not a fork of Waydroid or an Android source tree.

The product inherits `aosp_x86_64`, then adds:

- Ranchu HWC3 + minigbm graphics with Intel i915 targeting,
- container-specific Android properties,
- a minimal fstab for the LXC environment,
- ADB over the private container network for development,
- the host-display property/socket contract.

AOSP itself is intentionally not vendored into this repository.
