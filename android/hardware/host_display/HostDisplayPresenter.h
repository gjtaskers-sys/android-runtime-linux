#pragma once

#include "HostDisplayTransport.h"

#include <cstdint>

#include "Gralloc.h"

namespace aidl::android::hardware::graphics::composer3::impl {
class Display;
}

namespace android_runtime {

class HostDisplayPresenter {
  public:
    explicit HostDisplayPresenter(const char* socket_path);

    bool init();
    bool presentClientTarget(
        aidl::android::hardware::graphics::composer3::impl::Display* display,
        uint64_t sequence);
    bool ready() const { return mTransport.connected(); }

  private:
    HostDisplayTransport mTransport;
    aidl::android::hardware::graphics::composer3::impl::Gralloc mGralloc;
};

}  // namespace android_runtime
