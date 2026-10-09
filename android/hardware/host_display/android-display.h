#ifndef ANDROID_RUNTIME_DISPLAY_WIRE_H
#define ANDROID_RUNTIME_DISPLAY_WIRE_H

#include <stdint.h>

#define ANDROID_RUNTIME_DISPLAY_MAGIC "ANDRFRM1"
#define ANDROID_RUNTIME_DISPLAY_ACK_MAGIC "ANDRACK1"
#define ANDROID_RUNTIME_DISPLAY_VERSION 4u
#define ANDROID_RUNTIME_FRAME_FLAG_CLIENT_TARGET (1u << 0)
/* Receiver must bracket CPU reads of this DMA-BUF with DMA_BUF_IOCTL_SYNC. */
#define ANDROID_RUNTIME_FRAME_FLAG_DMA_BUF_CPU_SYNC (1u << 1)

struct android_runtime_frame_header {
    char magic[8];
    uint32_t version;
    uint32_t width;
    uint32_t height;
    uint32_t stride;
    uint32_t drm_fourcc;
    uint32_t modifier_lo;
    uint32_t modifier_hi;
    uint32_t flags;
    uint64_t sequence;
};

struct android_runtime_frame_ack {
    char magic[8];
    uint32_t version;
    uint32_t status;
    uint64_t sequence;
};

#endif
