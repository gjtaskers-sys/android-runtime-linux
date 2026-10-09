#pragma once

#include <cerrno>
#include <poll.h>

namespace android_runtime {

using PollFunction = int (*)(pollfd*, nfds_t, int);

/*
 * A fence is ready only when poll reports readability. Treat poll errors,
 * hangups, and invalid descriptors as failures rather than mistaking any
 * positive poll result for a signaled acquire fence. The injected poll function
 * keeps error/retry behavior testable without an Android graphics stack.
 */
inline bool waitForFenceReadableWith(int fd, PollFunction poll_function) {
    if (fd < 0 || poll_function == nullptr) return false;

    pollfd descriptor{};
    descriptor.fd = fd;
    descriptor.events = POLLIN;
    for (;;) {
        descriptor.revents = 0;
        const int result = poll_function(&descriptor, 1, -1);
        if (result < 0 && errno == EINTR) continue;
        if (result <= 0) return false;
        if ((descriptor.revents & POLLIN) == 0) return false;
        if ((descriptor.revents & (POLLERR | POLLHUP | POLLNVAL)) != 0) return false;
        return true;
    }
}

inline bool waitForFenceReadable(int fd) {
    return waitForFenceReadableWith(fd, poll);
}

}  // namespace android_runtime
