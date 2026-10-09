#pragma once

#include <cstdint>
#include <string>

#include <android-base/unique_fd.h>
#include <hardware/hardware.h>

namespace android_runtime {

struct HostDisplayBuffer {
    buffer_handle_t handle = nullptr;
    uint32_t width = 0;
    uint32_t height = 0;
    uint32_t stride = 0;
    uint32_t drm_fourcc = 0;
    uint64_t modifier = 0;
};

class HostDisplayTransport {
  public:
    explicit HostDisplayTransport(std::string socket_path);
    ~HostDisplayTransport();

    HostDisplayTransport(const HostDisplayTransport&) = delete;
    HostDisplayTransport& operator=(const HostDisplayTransport&) = delete;

    bool connect();
    bool present(const HostDisplayBuffer& buffer, uint64_t sequence,
                 int acquire_fence_fd = -1);
    void close();

    bool connected() const { return mSocket.ok(); }
    const std::string& socket_path() const { return mSocketPath; }

  private:
    bool send_frame(const HostDisplayBuffer& buffer, uint64_t sequence, int dma_buf_fd);

    std::string mSocketPath;
    android::base::unique_fd mSocket;
};

}  // namespace android_runtime
