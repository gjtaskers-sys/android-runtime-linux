#include "frame-validate.h"

#include <assert.h>
#include <stdio.h>

_Static_assert(sizeof(struct android_runtime_frame_header) == 48, "frame header wire size changed");
_Static_assert(sizeof(struct android_runtime_frame_ack) == 24, "frame ACK wire size changed");

static struct android_runtime_frame_header frame(uint32_t width, uint32_t height,
                                                  uint32_t stride, uint32_t flags) {
    struct android_runtime_frame_header hdr = {0};
    hdr.width = width;
    hdr.height = height;
    hdr.stride = stride;
    hdr.flags = flags;
    return hdr;
}

int main(void) {
    size_t map_size = 0;
    struct android_runtime_frame_header good =
        frame(1280, 720, 1280u * 4u, ANDROID_RUNTIME_FRAME_FLAG_CLIENT_TARGET);
    assert(android_runtime_valid_frame_geometry(&good, &map_size));
    assert(map_size == (size_t)1280 * 4u * 720u);

    struct android_runtime_frame_header dma_buf =
        frame(1280, 720, 1280u * 4u,
              ANDROID_RUNTIME_FRAME_FLAG_CLIENT_TARGET |
                  ANDROID_RUNTIME_FRAME_FLAG_DMA_BUF_CPU_SYNC);
    assert(android_runtime_valid_frame_geometry(&dma_buf, &map_size));

    struct android_runtime_frame_header zero_width = frame(0, 720, 0, 0);
    assert(!android_runtime_valid_frame_geometry(&zero_width, &map_size));

    struct android_runtime_frame_header short_stride = frame(1280, 720, 1280u * 4u - 1u, 0);
    assert(!android_runtime_valid_frame_geometry(&short_stride, &map_size));

    struct android_runtime_frame_header huge_dim =
        frame(ANDROID_RUNTIME_MAX_FRAME_DIMENSION + 1u, 1u,
              (ANDROID_RUNTIME_MAX_FRAME_DIMENSION + 1u) * 4u, 0);
    assert(!android_runtime_valid_frame_geometry(&huge_dim, &map_size));

    struct android_runtime_frame_header huge_buffer = frame(8193u, 8192u, 8193u * 4u, 0);
    assert(!android_runtime_valid_frame_geometry(&huge_buffer, &map_size));

    struct android_runtime_frame_header unknown_flags = frame(1280, 720, 1280u * 4u, 1u << 31);
    assert(!android_runtime_valid_frame_geometry(&unknown_flags, &map_size));

    puts("frame metadata validation tests passed");
    return 0;
}
