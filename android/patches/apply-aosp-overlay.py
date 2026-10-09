#!/usr/bin/env python3
"""Apply the Android Runtime overlay to a clean AOSP checkout.

The overlay deliberately uses AOSP Ranchu's existing NoOpFrameComposer as the
lightweight presentation backend. We force all application layers to CLIENT,
so SurfaceFlinger produces one client-target buffer which we export to Linux.
"""
from __future__ import annotations

import argparse
import re
import shutil
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
ANDROID = ROOT / "android"
HOST = ANDROID / "hardware" / "host_display"
DEVICE = ANDROID / "device" / "android_runtime"
HWC_FILES = [
    "HostDisplayPresenter.cpp",
    "HostDisplayPresenter.h",
    "HostDisplayTransport.cpp",
    "HostDisplayTransport.h",
    "android-display.h",
]


def fail(msg: str) -> None:
    raise SystemExit(f"error: {msg}")


def replace_once(text: str, old: str, new: str, label: str) -> str:
    if old not in text:
        fail(f"could not find {label}")
    return text.replace(old, new, 1)


def copy_product_tree(aosp: Path) -> None:
    dst = aosp / "device" / "android_runtime"
    if dst.exists():
        shutil.rmtree(dst)
    shutil.copytree(DEVICE, dst)


def copy_host_files(hwc3: Path) -> None:
    for name in HWC_FILES:
        shutil.copy2(HOST / name, hwc3 / name)


def patch_hwc3_bp(hwc3: Path) -> None:
    bp = hwc3 / "Android.bp"
    text = bp.read_text()
    if '"HostDisplayPresenter.cpp",' not in text:
        text = replace_once(
            text,
            '"NoOpFrameComposer.cpp",\n',
            '"NoOpFrameComposer.cpp",\n        "HostDisplayPresenter.cpp",\n        "HostDisplayTransport.cpp",\n',
            "HWC3 source list",
        )
    bp.write_text(text)


def patch_noop_header(hwc3: Path) -> None:
    path = hwc3 / "NoOpFrameComposer.h"
    text = path.read_text()
    if '#include "HostDisplayPresenter.h"' not in text:
        text = replace_once(
            text,
            '#include "FrameComposer.h"\n',
            '#include "FrameComposer.h"\n#include "HostDisplayPresenter.h"\n',
            "NoOpFrameComposer includes",
        )
    if "mHostDisplayPresenter" not in text:
        text = text.replace(
            '  public:\n',
            '''  public:\n''',
            1,
        )
        marker = '  HWC3::Error onActiveConfigChange(Display* /*display*/) override;\n'
        member = marker + '''\n  std::unique_ptr<android_runtime::HostDisplayPresenter> mHostDisplayPresenter;\n  uint64_t mHostDisplaySequence = 0;\n'''
        text = replace_once(text, marker, member, "NoOpFrameComposer member anchor")
    if "#include <memory>" not in text:
        text = replace_once(text, '#include "Common.h"\n', '#include <cstdint>\n#include <memory>\n\n#include "Common.h"\n', "NoOpFrameComposer standard includes")
    path.write_text(text)


def patch_noop_source(hwc3: Path) -> None:
    path = hwc3 / "NoOpFrameComposer.cpp"
    text = path.read_text()
    if '"android-base/properties.h"' not in text:
        text = replace_once(
            text,
            '#include "NoOpFrameComposer.h"\n',
            '#include "NoOpFrameComposer.h"\n\n#include <android-base/properties.h>\n',
            "NoOpFrameComposer property include",
        )

    text = replace_once(
        text,
'''HWC3::Error NoOpFrameComposer::init() {\n  DEBUG_LOG("%s", __FUNCTION__);\n\n  return HWC3::Error::None;\n}\n''',
'''HWC3::Error NoOpFrameComposer::init() {\n  DEBUG_LOG("%s", __FUNCTION__);\n\n  const std::string socket = ::android::base::GetProperty(\n          "ro.android_runtime.display_socket", "/run/android-runtime/display.sock");\n  mHostDisplayPresenter = std::make_unique<android_runtime::HostDisplayPresenter>(socket.c_str());\n\n  return HWC3::Error::None;\n}\n''',
        "NoOpFrameComposer::init",
    )

    old_validate='''HWC3::Error NoOpFrameComposer::validateDisplay(Display*, DisplayChanges*) {\n  DEBUG_LOG("%s", __FUNCTION__);\n\n  return HWC3::Error::None;\n}\n'''
    new_validate='''HWC3::Error NoOpFrameComposer::validateDisplay(Display* display, DisplayChanges* outChanges) {\n  DEBUG_LOG("%s", __FUNCTION__);\n\n  if (display == nullptr || outChanges == nullptr) {\n    return HWC3::Error::BadParameter;\n  }\n\n  // We intentionally make SurfaceFlinger do the complete composition. This\n  // leaves the host-side container with one client-target buffer to present.\n  for (Layer* layer : display->getOrderedLayers()) {\n    if (layer != nullptr && layer->getCompositionType() != Composition::CLIENT) {\n      outChanges->addLayerCompositionChange(display->getId(), layer->getId(), Composition::CLIENT);\n    }\n  }\n\n  return HWC3::Error::None;\n}\n'''
    text = replace_once(text, old_validate, new_validate, "NoOpFrameComposer::validateDisplay")

    old_present='''HWC3::Error NoOpFrameComposer::presentDisplay(\n    Display*, ::android::base::unique_fd*,\n    std::unordered_map<int64_t, ::android::base::unique_fd>* /*outLayerFences*/) {\n  DEBUG_LOG("%s", __FUNCTION__);\n\n  return HWC3::Error::None;\n}\n'''
    new_present='''HWC3::Error NoOpFrameComposer::presentDisplay(\n    Display* display, ::android::base::unique_fd*,\n    std::unordered_map<int64_t, ::android::base::unique_fd>* /*outLayerFences*/) {\n  DEBUG_LOG("%s", __FUNCTION__);\n\n  if (display == nullptr || !mHostDisplayPresenter) {\n    return HWC3::Error::NoResources;\n  }\n\n  if (!mHostDisplayPresenter->presentClientTarget(display, ++mHostDisplaySequence)) {\n    return HWC3::Error::NoResources;\n  }\n\n  return HWC3::Error::None;\n}\n'''
    text = replace_once(text, old_present, new_present, "NoOpFrameComposer::presentDisplay")
    path.write_text(text)


def patch_display_finder(hwc3: Path) -> None:
    path = hwc3 / "DisplayFinder.cpp"
    text = path.read_text()
    old='''  outDisplays->push_back(DisplayMultiConfigs{\n      .displayId = 0,\n      .activeConfigId = 0,\n      .configs = {DisplayConfig(0,\n                                 /*width=*/720, //\n                                 /*heighth=*/1280, //\n                                 /*dpiXh=*/320, //\n                                 /*dpiYh=*/320, //\n                                 /*vsyncPeriod=*/HertzToPeriodNanos(30) //\n                                 )},\n  });\n'''
    new='''  const uint32_t width = static_cast<uint32_t>(::android::base::GetIntProperty(\n          "ro.android_runtime.display_width", 1600));\n  const uint32_t height = static_cast<uint32_t>(::android::base::GetIntProperty(\n          "ro.android_runtime.display_height", 900));\n  const uint32_t dpi = static_cast<uint32_t>(::android::base::GetIntProperty(\n          "ro.android_runtime.display_dpi", 160));\n  const uint32_t refresh = static_cast<uint32_t>(::android::base::GetIntProperty(\n          "ro.android_runtime.display_refresh", 60));\n\n  outDisplays->push_back(DisplayMultiConfigs{\n      .displayId = 0,\n      .activeConfigId = 0,\n      .configs = {DisplayConfig(0, width, height, dpi, dpi, HertzToPeriodNanos(refresh))},\n  });\n'''
    text = replace_once(text, old, new, "NoOp display configuration")
    path.write_text(text)


def patch_properties(aosp: Path) -> None:
    path = aosp / "device" / "android_runtime" / "android_runtime.mk"
    text = path.read_text()
    marker = '    ro.android_runtime.display_socket=/run/android-runtime/display.sock'
    addition = marker + ' \\\n    ro.android_runtime.display_width=1600 \\\n    ro.android_runtime.display_height=900 \\\n    ro.android_runtime.display_dpi=160 \\\n    ro.android_runtime.display_refresh=60'
    if 'ro.android_runtime.display_width=1600' not in text:
        text = text.replace(marker, addition, 1)
    path.write_text(text)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("aosp", type=Path)
    args = ap.parse_args()
    aosp = args.aosp.resolve()
    if not (aosp / "build" / "envsetup.sh").exists():
        fail(f"{aosp} does not look like an AOSP checkout")

    hwc3 = aosp / "device" / "generic" / "goldfish-opengl" / "system" / "hwc3"
    if not hwc3.exists():
        fail("AOSP Ranchu HWC3 directory is missing")
    for name in ["NoOpFrameComposer.cpp", "NoOpFrameComposer.h", "DisplayFinder.cpp", "Android.bp"]:
        if not (hwc3 / name).exists():
            fail(f"current Ranchu HWC3 is missing {name}; overlay must be reviewed")

    copy_product_tree(aosp)
    copy_host_files(hwc3)
    patch_hwc3_bp(hwc3)
    patch_noop_header(hwc3)
    patch_noop_source(hwc3)
    patch_display_finder(hwc3)
    patch_properties(aosp)

    print("Android Runtime AOSP overlay applied")
    print(f"  product: {aosp / 'device/android_runtime'}")
    print(f"  HWC3:    {hwc3}")
    print("  mode:    NoOp HWC3 -> client-target -> host display socket")


if __name__ == "__main__":
    main()
