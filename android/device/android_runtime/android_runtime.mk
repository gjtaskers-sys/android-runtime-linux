# SPDX-License-Identifier: Apache-2.0
# Shared product customization for the Android Runtime container.

PRODUCT_PACKAGES += \
    android.hardware.graphics.composer3-service.ranchu

PRODUCT_SYSTEM_DEFAULT_PROPERTIES += \
    ro.debuggable=1 \
    ro.adb.secure=0 \
    persist.sys.usb.config=none \
    service.adb.tcp.port=5555 \
    sys.usb.config=none \
    ro.android_runtime.host_display=1 \
    ro.android_runtime.gpu_backend=software \
    ro.android_runtime.display_socket=/run/android-runtime/display.sock

PRODUCT_VENDOR_PROPERTIES += \
    ro.hardware=android_runtime \
    ro.hardware.gralloc=minigbm \
    ro.vendor.hwcomposer.mode=noop \
    ro.vendor.hwcomposer.display_finder_mode=noop

PRODUCT_COPY_FILES += \
    $(LOCAL_PATH)/rootdir/init.android-runtime.rc:$(TARGET_COPY_OUT_VENDOR)/etc/init/init.android-runtime.rc \
    $(LOCAL_PATH)/rootdir/fstab.android_runtime:$(TARGET_COPY_OUT_VENDOR)/etc/fstab.android_runtime

# We are building a container userspace, not a bootable phone image.
PRODUCT_BUILD_BOOT_IMAGE := false
PRODUCT_BUILD_VENDOR_BOOT_IMAGE := false
PRODUCT_BUILD_RECOVERY_IMAGE := false
PRODUCT_BUILD_INIT_BOOT_IMAGE := false
PRODUCT_BUILD_RAMDISK_IMAGE := true
PRODUCT_BUILD_SYSTEM_IMAGE := true
PRODUCT_BUILD_VENDOR_IMAGE := true
PRODUCT_BUILD_PRODUCT_IMAGE := true
PRODUCT_BUILD_SYSTEM_EXT_IMAGE := true

# Keep both 64-bit and 32-bit app plumbing available where the generic x86_64
# baseline provides it. ARM32/ARM64 are handled by Native Bridge later.
TARGET_SUPPORTS_64_BIT_APPS := true
TARGET_SUPPORTS_32_BIT_APPS := true

# No physical phone radios/cameras are required by the base appliance.
PRODUCT_OTA_ENFORCE_VINTF_KERNEL_REQUIREMENTS := false
