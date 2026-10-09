#include "HostDisplayPresenter.h"

#include <android-base/properties.h>
#include <log/log.h>

#include <algorithm>

#include "Display.h"

namespace android_runtime {
namespace {

constexpr const char* kDefaultDisplaySocket = "/run/android-runtime/display.sock";

}  // namespace

HostDisplayPresenter::HostDisplayPresenter(const char* socket_path)
    : mTransport(socket_path ? socket_path : kDefaultDisplaySocket) {}

bool HostDisplayPresenter::init() {
    if (mTransport.connect()) return true;
    ALOGE("Android Runtime host display: failed to connect to %s", mTransport.socket_path().c_str());
    return false;
}

bool HostDisplayPresenter::presentClientTarget(
        aidl::android::hardware::graphics::composer3::impl::Display* display,
        uint64_t sequence) {
    if (display == nullptr) return false;
    if (!mTransport.connected() && !mTransport.connect()) return false;

    buffer_handle_t buffer = display->getClientTarget().getBuffer();
    if (buffer == nullptr) return false;

    auto imported = mGralloc.Import(buffer);
    if (!imported) {
        ALOGE("Android Runtime host display: failed to import client target");
        return false;
    }

    const auto width = imported->GetWidth();
    const auto height = imported->GetHeight();
    const auto stride = imported->GetMonoPlanarStrideBytes();
    const auto fourcc = imported->GetDrmFormat();
    if (!width || !height || !stride || !fourcc) {
        ALOGE("Android Runtime host display: incomplete gralloc metadata");
        return false;
    }

    // v0.1 deliberately supports a single-plane 32-bit client target.
    if (*stride < *width * 4u || *fourcc == 0) {
        ALOGE("Android Runtime host display: unsupported client target geometry");
        return false;
    }

    auto fence = display->getClientTarget().getFence();
    HostDisplayBuffer frame{
        buffer,
        *width,
        *height,
        *stride,
        *fourcc,
        0,
    };

    return mTransport.present(frame, sequence, fence.get());
}

}  // namespace android_runtime
