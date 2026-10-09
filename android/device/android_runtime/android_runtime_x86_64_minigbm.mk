# SPDX-License-Identifier: Apache-2.0
# Android Runtime x86_64 container product.

$(call inherit-product, $(SRC_TARGET_DIR)/product/aosp_x86_64.mk)
$(call inherit-product, $(LOCAL_PATH)/android_runtime.mk)

PRODUCT_NAME := android_runtime_x86_64_minigbm
PRODUCT_DEVICE := generic_x86_64
PRODUCT_BRAND := Android Runtime
PRODUCT_MODEL := Android Runtime x86_64
PRODUCT_MANUFACTURER := Android Runtime Project

PRODUCT_ENFORCE_ARTIFACT_PATH_REQUIREMENTS := relaxed
PRODUCT_SOONG_NAMESPACES += device/android_runtime

PRODUCT_SYSTEM_PROPERTIES += \
    ro.android_runtime.host_display=1 \
    ro.android_runtime.display_socket=/run/android-runtime/display.sock \
    ro.android_runtime.container=1
