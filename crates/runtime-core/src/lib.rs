//! Host-side orchestration and compatibility policy.

use android_apk_inspect::ApkInfo;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    LiteContainer,
    ArmTranslatedContainer,
    CompatibilityVm,
}

impl ExecutionMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LiteContainer => "lite-container",
            Self::ArmTranslatedContainer => "arm-translated-container",
            Self::CompatibilityVm => "compatibility-vm",
        }
    }
}

#[derive(Debug, Clone)]
pub struct HardwareProfile {
    pub arch: String,
    pub logical_cpus: usize,
    pub ram_mb: u64,
    pub kvm: bool,
    pub x11: bool,
    pub wayland: bool,
    pub render_nodes: Vec<String>,
    pub lxc: bool,
}

pub fn probe_host() -> HardwareProfile {
    let logical_cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    let ram_mb = fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|line| line.starts_with("MemTotal:"))
                .and_then(|line| line.split_whitespace().nth(1))
                .and_then(|v| v.parse::<u64>().ok())
        })
        .map(|kb| kb / 1024)
        .unwrap_or(0);
    let render_nodes = fs::read_dir("/dev/dri")
        .ok()
        .into_iter()
        .flat_map(|it| it.flatten())
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with("renderD") {
                Some(format!("/dev/dri/{name}"))
            } else {
                None
            }
        })
        .collect();
    HardwareProfile {
        arch: std::env::consts::ARCH.to_string(),
        logical_cpus,
        ram_mb,
        kvm: Path::new("/dev/kvm").exists(),
        x11: std::env::var_os("DISPLAY").is_some(),
        wayland: std::env::var_os("WAYLAND_DISPLAY").is_some(),
        render_nodes,
        lxc: command_exists("lxc-start"),
    }
}

pub mod lxc;

pub fn choose_mode(apk: &ApkInfo, host: &HardwareProfile) -> ExecutionMode {
    if apk.abis.is_empty() || apk.supports_host_isa(&host.arch) {
        return ExecutionMode::LiteContainer;
    }

    if host.arch == "x86_64" && apk.needs_arm_translation_on_x86() {
        // The presence of a bridge is an Android-image capability; this
        // host-side policy only decides which capability the image must expose.
        return ExecutionMode::ArmTranslatedContainer;
    }

    // The compatibility backend is intentionally generic: ARM64 is the first
    // implementation target, while QEMU can later cover other foreign ISAs.
    ExecutionMode::CompatibilityVm
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerformanceProfile {
    Lite,
    Balanced,
    High,
}

impl PerformanceProfile {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lite => "lite",
            Self::Balanced => "balanced",
            Self::High => "high",
        }
    }

    pub fn display_size(self) -> (u32, u32) {
        match self {
            Self::Lite => (1280, 720),
            Self::Balanced => (1600, 900),
            Self::High => (1920, 1080),
        }
    }

    pub fn target_fps(self) -> u32 {
        match self {
            Self::Lite => 30,
            Self::Balanced | Self::High => 60,
        }
    }
}

pub fn choose_performance(host: &HardwareProfile) -> PerformanceProfile {
    if host.ram_mb <= 8192 || host.logical_cpus <= 2 {
        PerformanceProfile::Lite
    } else if host.ram_mb <= 16384 || host.logical_cpus <= 4 {
        PerformanceProfile::Balanced
    } else {
        PerformanceProfile::High
    }
}

fn command_exists(name: &str) -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|dir| dir.join(name).is_file())
}

#[cfg(test)]
mod tests {
    use super::*;
    use android_apk_inspect::{Abi, ApkInfo};
    use std::path::PathBuf;

    fn host(arch: &str, cpus: usize, ram_mb: u64) -> HardwareProfile {
        HardwareProfile {
            arch: arch.to_string(),
            logical_cpus: cpus,
            ram_mb,
            kvm: false,
            x11: true,
            wayland: false,
            render_nodes: vec!["/dev/dri/renderD128".to_string()],
            lxc: true,
        }
    }

    #[test]
    fn x86_apk_uses_lite_container() {
        let apk = ApkInfo {
            path: PathBuf::from("x.apk"),
            abis: vec![Abi::X86_64],
            has_native_code: true,
            is_split_like: false,
            package_name: None,
        };
        assert_eq!(
            choose_mode(&apk, &host("x86_64", 4, 16384)),
            ExecutionMode::LiteContainer
        );
    }

    #[test]
    fn arm_only_apk_uses_translation_path() {
        let apk = ApkInfo {
            path: PathBuf::from("x.apk"),
            abis: vec![Abi::Arm64],
            has_native_code: true,
            is_split_like: false,
            package_name: None,
        };
        assert_eq!(
            choose_mode(&apk, &host("x86_64", 4, 16384)),
            ExecutionMode::ArmTranslatedContainer
        );
    }

    #[test]
    fn low_end_hardware_gets_lite_profile() {
        let p = choose_performance(&host("x86_64", 4, 8192));
        assert_eq!(p, PerformanceProfile::Lite);
        assert_eq!(p.display_size(), (1280, 720));
        assert_eq!(p.target_fps(), 30);
    }
}
