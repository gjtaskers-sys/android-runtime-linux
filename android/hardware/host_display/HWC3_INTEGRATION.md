# HWC3 integration point

Android 17 uses the AIDL `android.hardware.graphics.composer3` HAL. The AOSP Ranchu implementation already separates the binder-facing `ComposerClient` from a `FrameComposer` backend. We intentionally follow that architecture instead of implementing another large binder service here.

References:

- AOSP AIDL HWC documentation: https://source.android.com/docs/core/graphics/aidl-hwc
- AOSP Ranchu HWC3 tree: https://android.googlesource.com/device/generic/goldfish-opengl/+/refs/heads/main/system/hwc3/

## Intended wiring

```text
SurfaceFlinger
    -> Composer3 / ComposerClient
        -> executeCommands()
            -> setClientTarget()
            -> validateDisplay()
            -> presentDisplay()
        -> FrameComposer backend
            -> HostDisplayPresenter
                -> HostDisplayTransport
                    -> SCM_RIGHTS DMA-BUF
                    -> Linux host display socket
```

For the first product image, the Ranchu/Cuttlefish resource and gralloc path is the reference implementation. Cuttlefish currently uses `gralloc.minigbm`, and its graphics configuration includes the Ranchu Composer3 service. We will adapt the `HostFrameComposer` seam rather than fork the whole service.

## Client-target policy

The first implementation forces all application layers to `Composition::CLIENT`. SurfaceFlinger therefore produces one final client-target buffer. `presentDisplay()` forwards that buffer through `HostDisplayPresenter` and returns an already-supported present fence.

This deliberately trades a little composition efficiency for a much simpler v0.1 host boundary. After the end-to-end path is validated, later work can add direct DMA-BUF EGL import on X11 and, eventually, per-layer/seamless-window composition.

## Buffer assumptions

The first Android image uses minigbm. `HostDisplayTransport` therefore treats file descriptor 0 in the native gralloc handle as the DMA-BUF and rejects handles with no FD. This assumption is **not** claimed to be generic across vendor grallocs; it is part of the planned runtime-image contract.

The transport also waits for the client-target acquire fence before exporting the FD to the host. That prevents the X11 consumer from racing SurfaceFlinger's GPU composition.


## CPU-access coherency

`HostDisplayTransport` sets protocol-v4's `ANDROID_RUNTIME_FRAME_FLAG_DMA_BUF_CPU_SYNC`
for the client-target DMA-BUF. The Linux host maps it read-only and brackets the
pixel copy with `DMA_BUF_IOCTL_SYNC` START/READ and END/READ. The producer first
waits for the Composer3 client-target acquire fence; these two steps are distinct
and both are required. The host then waits for the XShm completion event before
ACKing, so the Android producer is not told that the frame is consumed before
the host copy and X server read are complete.

This CPU-copy path is a conservative bring-up path, not zero-copy. Actual
minigbm format/stride/modifier and ioctl behavior must be observed from the
contributor-built image before declaring Android-to-X11 presentation verified.
