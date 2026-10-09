# Android product overlay and graphics integration

This directory contains the project's AOSP product/device overlay and Android-side host-display integration scaffolding. A complete image has not yet been built and validated end to end; the diagrams below describe the intended integration and current source-level assumptions, not a supported Android release.

## Intended graphics flow

```text
SurfaceFlinger
  -> Composer3 / HWC3 integration
     -> host-frame composer
        -> validate: request client composition for app layers
        -> present: forward the final client-target buffer
           -> wait for acquire fence
           -> transfer buffer FD and metadata over SCM_RIGHTS
           -> host X11 display bridge
```

The goal is to avoid physical DRM/KMS presentation from inside the guest. Android should compose a single final client-target buffer, and the host bridge should present that frame. Actual AOSP integration, boot behavior, and buffer export still require validation using a built image.

## Display properties

The product overlay defines the following configurable display properties:

- `ro.android_runtime.display_width`
- `ro.android_runtime.display_height`
- `ro.android_runtime.display_dpi`
- `ro.android_runtime.display_refresh`

These are configuration inputs; a running, supported image and its behavior have not yet been demonstrated.

## Graphics bring-up strategy

The first bring-up target is a conservative CPU-readable buffer format with a copy into the host X11 image. Hardware acceleration and direct DMA-BUF/EGL import are later optimizations. Intel/i915 and minigbm are design targets, not currently verified end-to-end capabilities.

## DMA-BUF CPU-access synchronization (protocol v4)

The Android-side source marks exported client-target frames with `ANDROID_RUNTIME_FRAME_FLAG_DMA_BUF_CPU_SYNC`. After the producer waits for the client-target acquire fence, the host brackets its CPU read/copy with `DMA_BUF_IOCTL_SYNC(DMA_BUF_SYNC_START | DMA_BUF_SYNC_READ)` and `DMA_BUF_IOCTL_SYNC(DMA_BUF_SYNC_END | DMA_BUF_SYNC_READ)`. These ioctls address CPU cache coherency; they do not replace the acquire-fence wait or the host-side XShm completion acknowledgement.

Synthetic smoke frames use ordinary memfds and omit the DMA-BUF synchronization flag, so host tests remain independent of a real DMA-BUF exporter. The protocol tests and mock-ioctl tests validate parts of this contract, but only a built Android image can establish the actual minigbm handle layout, format, stride, modifier, and synchronization behavior.
