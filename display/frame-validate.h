#ifndef ANDROID_RUNTIME_FRAME_VALIDATE_H
#define ANDROID_RUNTIME_FRAME_VALIDATE_H

#include "android-display.h"

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

/* Validate untrusted frame metadata before casting sizes to size_t or mapping. */
static inline bool android_runtime_valid_frame_geometry(
    const struct android_runtime_frame_header *hdr, size_t *map_size) {
    if (hdr == NULL || map_size == NULL || hdr->width == 0 || hdr->height == 0 ||
        hdr->width > ANDROID_RUNTIME_MAX_FRAME_DIMENSION ||
        hdr->height > ANDROID_RUNTIME_MAX_FRAME_DIMENSION ||
        hdr->stride < hdr->width * 4u ||
        (hdr->flags & ~ANDROID_RUNTIME_FRAME_KNOWN_FLAGS) != 0) {
        return false;
    }

    const uint64_t bytes = (uint64_t)hdr->stride * (uint64_t)hdr->height;
    if (bytes == 0 || bytes > ANDROID_RUNTIME_MAX_FRAME_BYTES) {
        return false;
    }

    /* The protocol's 256 MiB cap also fits a 32-bit Linux size_t. */
    *map_size = (size_t)bytes;
    return true;
}

#endif
