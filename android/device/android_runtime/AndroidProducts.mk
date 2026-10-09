# Android Runtime product definitions.
# This product intentionally inherits Google's x86_64 AOSP emulator baseline
# and replaces the desktop-facing pieces with our container integration.

PRODUCT_MAKEFILES := \
    $(LOCAL_DIR)/android_runtime_x86_64_minigbm.mk

COMMON_LUNCH_CHOICES := \
    android_runtime_x86_64_minigbm-userdebug
