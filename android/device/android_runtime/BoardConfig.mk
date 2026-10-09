# SPDX-License-Identifier: Apache-2.0
# The product inherits the generic x86_64 board from AOSP. This file is kept
# deliberately small so future board-specific tuning is isolated here.

TARGET_ARCH := x86_64
TARGET_ARCH_VARIANT := x86-64
TARGET_CPU_ABI := x86_64
TARGET_CPU_ABI2 :=

# Graphics are intentionally conservative for the first boot milestone.
# The host runtime starts with the HWC3 client-target transport and software
# rendering; Intel minigbm/DRM acceleration is enabled in a later milestone.
BOARD_GPU_DRIVERS := i915
BOARD_USES_MINIGBM := true
