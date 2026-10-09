# Host display transport

This directory contains the Android-side v0.1 host display bridge.

```text
SurfaceFlinger / HWC3
        |
        v
 HostDisplayPresenter
        |
        v
 HostDisplayTransport
        |
        +-- AF_UNIX SOCK_SEQPACKET
        +-- frame metadata
        +-- SCM_RIGHTS buffer FD
        +-- synchronous frame ACK
        v
Linux desktop display helper
```

The first Android implementation uses the final client-target buffer so the
host receives one already-composited frame. `SCM_RIGHTS` transfers the backing
FD without copying on the Android side; the current X11 helper then copies into
XShm. That copy is deliberate for v0.1 reliability and will be replaced by a
DMA-BUF/EGL import path after the Android image is booting reliably.

The transport assumes the Android Runtime appliance uses minigbm-style native
handles where FD 0 is the first plane. This is an appliance-specific contract,
not a promise about arbitrary Android/vendor gralloc handles.
