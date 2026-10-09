#include "HostDisplayTransport.h"
#include "AcquireFenceWait.h"
#include "android-display.h"

#include <errno.h>
#include <poll.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <unistd.h>

namespace android_runtime {
namespace {

int extract_dma_buf_fd(buffer_handle_t handle) {
    if (handle == nullptr) return -1;
    const native_handle_t* native = handle;
    if (native->numFds < 1) return -1;
    return native->data[0];
}

bool wait_acquire_fence(int fd) {
    if (fd < 0) return true;
    return waitForFenceReadable(fd);
}

}  // namespace

HostDisplayTransport::HostDisplayTransport(std::string socket_path)
    : mSocketPath(std::move(socket_path)) {}

HostDisplayTransport::~HostDisplayTransport() { close(); }

bool HostDisplayTransport::connect() {
    close();

    int fd = socket(AF_UNIX, SOCK_SEQPACKET | SOCK_CLOEXEC, 0);
    if (fd < 0) return false;

    sockaddr_un addr = {};
    addr.sun_family = AF_UNIX;
    if (mSocketPath.empty() || mSocketPath.size() >= sizeof(addr.sun_path)) {
        ::close(fd);
        return false;
    }
    memcpy(addr.sun_path, mSocketPath.c_str(), mSocketPath.size() + 1);

    if (::connect(fd, reinterpret_cast<const sockaddr*>(&addr), sizeof(addr)) < 0) {
        ::close(fd);
        return false;
    }

    mSocket.reset(fd);
    return true;
}

bool HostDisplayTransport::present(const HostDisplayBuffer& buffer, uint64_t sequence,
                                   int acquire_fence_fd) {
    if (!mSocket.ok() || buffer.handle == nullptr) return false;
    if (!wait_acquire_fence(acquire_fence_fd)) return false;

    const int dma_fd = extract_dma_buf_fd(buffer.handle);
    if (dma_fd < 0) return false;

    return send_frame(buffer, sequence, dma_fd);
}

bool HostDisplayTransport::send_frame(const HostDisplayBuffer& buffer, uint64_t sequence,
                                      int dma_buf_fd) {
    android_runtime_frame_header hdr = {};
    memcpy(hdr.magic, ANDROID_RUNTIME_DISPLAY_MAGIC, sizeof(hdr.magic));
    hdr.version = ANDROID_RUNTIME_DISPLAY_VERSION;
    hdr.width = buffer.width;
    hdr.height = buffer.height;
    hdr.stride = buffer.stride;
    hdr.drm_fourcc = buffer.drm_fourcc;
    hdr.modifier_lo = static_cast<uint32_t>(buffer.modifier & 0xffffffffu);
    hdr.modifier_hi = static_cast<uint32_t>(buffer.modifier >> 32);
    hdr.flags = ANDROID_RUNTIME_FRAME_FLAG_CLIENT_TARGET |
                ANDROID_RUNTIME_FRAME_FLAG_DMA_BUF_CPU_SYNC;
    hdr.sequence = sequence;

    iovec iov = {&hdr, sizeof(hdr)};
    char control[CMSG_SPACE(sizeof(int))] = {};
    msghdr msg = {};
    msg.msg_iov = &iov;
    msg.msg_iovlen = 1;
    msg.msg_control = control;
    msg.msg_controllen = sizeof(control);

    cmsghdr* cmsg = CMSG_FIRSTHDR(&msg);
    cmsg->cmsg_level = SOL_SOCKET;
    cmsg->cmsg_type = SCM_RIGHTS;
    cmsg->cmsg_len = CMSG_LEN(sizeof(int));
    memcpy(CMSG_DATA(cmsg), &dma_buf_fd, sizeof(int));

    if (sendmsg(mSocket.get(), &msg, MSG_NOSIGNAL) != static_cast<ssize_t>(sizeof(hdr))) {
        return false;
    }

    android_runtime_frame_ack ack = {};
    ssize_t n;
    do {
        n = recv(mSocket.get(), &ack, sizeof(ack), MSG_WAITALL);
    } while (n < 0 && errno == EINTR);
    if (n != static_cast<ssize_t>(sizeof(ack))) return false;

    return memcmp(ack.magic, ANDROID_RUNTIME_DISPLAY_ACK_MAGIC, sizeof(ack.magic)) == 0 &&
           ack.version == ANDROID_RUNTIME_DISPLAY_VERSION && ack.status == 0 &&
           ack.sequence == sequence;
}

void HostDisplayTransport::close() { mSocket.reset(); }

}  // namespace android_runtime
