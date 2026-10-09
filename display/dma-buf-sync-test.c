#include "dma-buf-sync.h"

#include <assert.h>
#include <stdio.h>
#include <string.h>

struct mock_state {
    int calls;
    int result[4];
    int error[4];
    int expected_fd;
    uint64_t expected_flags;
};

static struct mock_state *active_mock;

static int mock_ioctl(int fd, unsigned long request, void *argument) {
    assert(active_mock != NULL);
    assert(fd == active_mock->expected_fd);
    assert(request == DMA_BUF_IOCTL_SYNC);
    struct dma_buf_sync *sync = argument;
    assert(sync->flags == active_mock->expected_flags);

    int index = active_mock->calls++;
    assert(index < 4);
    if (active_mock->result[index] == 0) return 0;
    errno = active_mock->error[index];
    return -1;
}

static void test_start_and_end_flags_are_forwarded(void) {
    struct mock_state mock = {
        .expected_fd = 42,
        .expected_flags = DMA_BUF_SYNC_START | DMA_BUF_SYNC_READ,
        .result = {0},
    };
    active_mock = &mock;
    assert(android_runtime_dma_buf_sync_with(42, DMA_BUF_SYNC_START | DMA_BUF_SYNC_READ,
                                             mock_ioctl) == 0);
    assert(mock.calls == 1);

    memset(&mock, 0, sizeof(mock));
    mock.expected_fd = 42;
    mock.expected_flags = DMA_BUF_SYNC_END | DMA_BUF_SYNC_READ;
    active_mock = &mock;
    assert(android_runtime_dma_buf_sync_with(42, DMA_BUF_SYNC_END | DMA_BUF_SYNC_READ,
                                             mock_ioctl) == 0);
    assert(mock.calls == 1);
}

static void test_transient_errors_are_retried(void) {
    struct mock_state mock = {
        .calls = 0,
        .result = {-1, -1, 0},
        .error = {EINTR, EAGAIN, 0},
        .expected_fd = 9,
        .expected_flags = DMA_BUF_SYNC_START | DMA_BUF_SYNC_READ,
    };
    active_mock = &mock;
    assert(android_runtime_dma_buf_sync_with(9, mock.expected_flags, mock_ioctl) == 0);
    assert(mock.calls == 3);
}

static void test_permanent_error_is_returned(void) {
    struct mock_state mock = {
        .result = {-1},
        .error = {ENOTTY},
        .expected_fd = 7,
        .expected_flags = DMA_BUF_SYNC_END | DMA_BUF_SYNC_READ,
    };
    active_mock = &mock;
    errno = 0;
    assert(android_runtime_dma_buf_sync_with(7, mock.expected_flags, mock_ioctl) == -1);
    assert(errno == ENOTTY);
    assert(mock.calls == 1);
}

int main(void) {
    test_start_and_end_flags_are_forwarded();
    test_transient_errors_are_retried();
    test_permanent_error_is_returned();
    puts("DMA-BUF CPU synchronization tests passed (START/END flags, EINTR/EAGAIN retry, permanent errors)");
    return 0;
}
