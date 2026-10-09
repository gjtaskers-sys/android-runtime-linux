# Android image contribution and handoff

## Scope

Most runtime contributions do not require an AOSP sync or build. The AOSP-derived Android userspace is a separate build workload that can be handled by contributors with suitable build hardware. Keeping image production separate avoids making a large platform build a prerequisite for ordinary host development.

A rootfs archive is a candidate artifact, not proof of compatibility. It must pass the structure validator and then boot on a documented host/kernel before it can be described as usable.

## Builder workflow

From a clean checkout on a dedicated Android build machine:

1. Initialize and sync the AOSP checkout explicitly with `scripts/android/bootstrap-aosp.sh` and `scripts/android/sync-aosp.sh`.
2. Build the selected product and assemble the rootfs:

   ```bash
   AOSP_DIR=/path/to/aosp \
   ANDROID_RUNTIME_OUT="$PWD/out/android-runtime" \
   JOBS="$(nproc)" \
   ./scripts/android/build-aosp-image.sh
   ```

   A repeated assembly refuses to overwrite an existing rootfs by default. To replace it, use `--replace-rootfs`; the existing rootfs is retained until the new result has been assembled and validated.
3. Review the pinned AOSP manifest produced at `out/android-runtime/repo-manifest-pinned.xml`. Include that manifest with the artifact so the source revisions can be identified.
4. Validate and package the rootfs:

   ```bash
   sudo ./scripts/android/package-rootfs.sh \
     --source-manifest out/android-runtime/repo-manifest-pinned.xml \
     out/android-runtime/rootfs \
     out/android-runtime/android-runtime-x86_64-rootfs.tar.zst
   ```

   This generates the archive plus `.sha256` and `.info` sidecars. Root privileges are used to preserve numeric ownership, ACLs, extended attributes, and SELinux labels. The script rejects a rootfs containing files under `/data` to avoid including app or user data from a runtime instance.

## Required handoff files

Provide these items together:

- `android-runtime-x86_64-rootfs.tar.zst`;
- its `.sha256` checksum file;
- its `.info` metadata file;
- `repo-manifest-pinned.xml` from the matching AOSP output directory;
- build logs and rootfs validation output, distributed separately from the image.

The archive is sorted by path and normalizes volatile timestamps while preserving Android numeric UID/GID, mode bits, hard links, ACLs, xattrs, and SELinux contexts. This improves hash stability but does not, by itself, prove the complete AOSP build is reproducible.

Before an artifact is redistributed, review provenance and license terms for AOSP-derived files and all included components. Confirm that it contains no machine-specific secrets, no populated `/data`, and no component without the required redistribution rights. Do not commit large generated images to the source repository; use a reviewed artifact channel with metadata and checksums.

## Validate on a host

Verify the checksum before extracting:

```bash
sha256sum -c android-runtime-x86_64-rootfs.tar.zst.sha256
sudo mkdir -p /var/lib/android-runtime/images/candidate
sudo tar --numeric-owner --xattrs --acls --selinux \
  --zstd -xf android-runtime-x86_64-rootfs.tar.zst \
  -C /var/lib/android-runtime/images/candidate
```

The archive contains a top-level `rootfs/` directory. Validate the candidate without building AOSP:

```bash
./scripts/android/check-container-readiness.sh \
  --rootfs /var/lib/android-runtime/images/candidate/rootfs
```

For a builder or CI machine that only needs structural validation:

```bash
./scripts/android/check-container-readiness.sh \
  --rootfs-only --rootfs /path/to/rootfs
```

The host preflight is diagnostic only. It does not load kernel modules, change permissions, start LXC, or assert that Android boots. Missing required host devices or tools are reported explicitly; missing desktop display/render resources can be warnings for a boot-only test.

## Boot evidence required

For every claimed supported host/image combination, record:

- distribution/version, kernel release, and LXC version;
- rootfs archive checksum and AOSP source manifest;
- Binder device configuration, cgroup mode, and container privilege profile;
- container start/stop logs and readiness of `init`, `servicemanager`, `zygote`, `system_server`, and PackageManager;
- `/data` persistence across stop/start;
- APK install/launch test result;
- graphics status: synthetic host X11 only, Android boot UI delivered, or real app frames delivered.

An archive passing structural validation or `lxc-start` returning success is not sufficient. The exit criterion is a repeatable Android userspace boot with service readiness and a successful APK launch test on the target host.
