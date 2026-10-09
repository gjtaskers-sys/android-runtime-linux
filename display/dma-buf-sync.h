#ifndef ANDROID_RUNTIME_DMA_BUF_SYNC_H
#define ANDROID_RUNTIME_DMA_BUF_SYNC_H

#include <errno.h>
#include <linux/dma-buf.h>
#include <stdint.h>
#include <sys/ioctl.h>

typedef int (*android_runtime_ioctl_fn)(int fd, unsigned long request, void *argument);

static inline int android_runtime_real_ioctl(int fd, unsigned long request, void *argument) {
    return ioctl(fd, request, argument);
}

/*
 * Bracket CPU access to a DMA-BUF. The Linux DMA-BUF API requires these
 * coherency markers around mmap-based CPU reads/writes; they do not replace
 * waiting on the producer's acquire fence.
 *
 * Keep the callback seam so retry and error behavior is testable without a
 * graphics driver or a real DMA-BUF exporter.
 */
static inline int android_runtime_dma_buf_sync_with(int fd, uint64_t flags,
                                                     android_runtime_ioctl_fn ioctl_call) {
    if (fd < 0 || ioctl_call == 0) {
        errno = EINVAL;
        return -1;
    }

    struct dma_buf_sync sync = {.flags = flags};
    for (;;) {
        if (ioctl_call(fd, DMA_BUF_IOCTL_SYNC, &sync) == 0) return 0;
        if (errno == EINTR || errno == EAGAIN) continue;
        return -1;
    }
}

static inline int android_runtime_dma_buf_sync(int fd, uint64_t flags) {
    return android_runtime_dma_buf_sync_with(fd, flags, android_runtime_real_ioctl);
}

#endif
