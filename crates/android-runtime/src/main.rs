use android_apk_inspect::inspect;
use android_device::{validate_package_name, AndroidDeviceClient};
use android_runtime_core::lxc::{prepare_instance, LxcInstance, LxcSpec};
use android_runtime_core::{choose_mode, choose_performance, probe_host};
use std::env;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Duration;

fn usage() {
    eprintln!(
        "Usage:\n  android-runtime doctor\n  android-runtime devices\n  android-runtime apk-info <file.apk>\n  android-runtime plan <file.apk>\n  android-runtime install <file.apk> [--serial SERIAL] [--no-launch]\n  android-runtime launch <package> [--serial SERIAL]\n  android-runtime stop-app <package> [--serial SERIAL]\n  android-runtime uninstall <package> [--serial SERIAL]\n  android-runtime runtime prepare --rootfs ROOTFS [options]\n  android-runtime runtime start --rootfs ROOTFS [options]\n  android-runtime runtime stop [--name NAME] [--state-dir DIR]\n  android-runtime runtime status [--name NAME] [--state-dir DIR]\n\nOptions:\n  --serial SERIAL          Select an Android device (required if several are online)\n  --name NAME              Instance name (default: android-runtime)\n  --bridge BRIDGE          Linux bridge (default: lxcbr0)\n  --display-socket PATH    Host display socket\n  --state-dir DIR          LXC state directory\n"
    );
}

fn doctor() -> ExitCode {
    let h = probe_host();
    println!("Android Runtime host diagnostics");
    println!("  architecture:      {}", h.arch);
    println!("  logical CPUs:       {}", h.logical_cpus);
    println!("  RAM:                {} MiB", h.ram_mb);
    println!("  /dev/kvm:           {}", yes_no(h.kvm));
    println!("  LXC:                {}", yes_no(h.lxc));
    println!("  X11:                {}", yes_no(h.x11));
    println!("  Wayland:            {}", yes_no(h.wayland));
    println!(
        "  DRM render nodes:   {}",
        if h.render_nodes.is_empty() {
            "none".to_string()
        } else {
            h.render_nodes.join(", ")
        }
    );
    println!();
    if h.x11 {
        println!("  desktop target:     native X11 (primary target)");
    } else if h.wayland {
        println!("  desktop target:     Wayland (secondary target)");
    } else {
        println!("  desktop target:     unavailable");
    }
    let profile = choose_performance(&h);
    let (w, hgt) = profile.display_size();
    println!(
        "  performance profile: {} ({}x{} @ {} fps)",
        profile.as_str(),
        w,
        hgt,
        profile.target_fps()
    );
    ExitCode::SUCCESS
}

fn default_state_dir() -> PathBuf {
    if let Some(dir) = env::var_os("XDG_STATE_HOME") {
        return PathBuf::from(dir).join("android-runtime/lxc");
    }
    if let Some(home) = env::var_os("HOME") {
        return PathBuf::from(home).join(".local/state/android-runtime/lxc");
    }
    PathBuf::from(".android-runtime/lxc")
}

fn default_display_socket() -> PathBuf {
    if let Some(dir) = env::var_os("XDG_RUNTIME_DIR") {
        return PathBuf::from(dir).join("android-runtime/display.sock");
    }
    PathBuf::from("/tmp/android-runtime-display.sock")
}

fn detect_binder_nodes() -> Vec<PathBuf> {
    ["/dev/binder", "/dev/hwbinder", "/dev/vndbinder"]
        .into_iter()
        .map(PathBuf::from)
        .filter(|p| p.exists())
        .collect()
}

fn valid_instance_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

fn parse_runtime_options(
    mut args: impl Iterator<Item = String>,
) -> Result<(String, PathBuf, String, PathBuf, PathBuf), String> {
    let mut name = "android-runtime".to_string();
    let mut rootfs = None;
    let mut bridge = "lxcbr0".to_string();
    let mut display_socket = default_display_socket();
    let mut state_dir = default_state_dir();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--rootfs" => rootfs = Some(PathBuf::from(args.next().ok_or("--rootfs needs a path")?)),
            "--name" => name = args.next().ok_or("--name needs a value")?,
            "--bridge" => bridge = args.next().ok_or("--bridge needs a value")?,
            "--display-socket" => {
                display_socket = PathBuf::from(args.next().ok_or("--display-socket needs a path")?)
            }
            "--state-dir" => {
                state_dir = PathBuf::from(args.next().ok_or("--state-dir needs a path")?)
            }
            other => return Err(format!("unknown runtime option: {other}")),
        }
    }

    if !valid_instance_name(&name) {
        return Err("invalid instance name; use letters, digits, '.', '_' or '-'".to_string());
    }
    let rootfs = rootfs.ok_or("runtime command requires --rootfs ROOTFS")?;
    if !rootfs.is_dir() {
        return Err(format!(
            "rootfs does not exist or is not a directory: {}",
            rootfs.display()
        ));
    }

    Ok((name, rootfs, bridge, display_socket, state_dir))
}

fn runtime_command(mut args: impl Iterator<Item = String>) -> Result<ExitCode, String> {
    let Some(action) = args.next() else {
        return Err("missing runtime action".into());
    };
    let mut remaining = args.peekable();
    match action.as_str() {
        "prepare" | "start" => {
            let (name, rootfs, bridge, display_socket, state_dir) =
                parse_runtime_options(remaining)?;
            let userdata = state_dir.join(&name).join("userdata");
            let spec = LxcSpec {
                name: name.clone(),
                rootfs,
                render_nodes: probe_host()
                    .render_nodes
                    .into_iter()
                    .map(PathBuf::from)
                    .collect(),
                binder_nodes: detect_binder_nodes(),
                display_socket,
                userdata,
                work_dir: state_dir.clone(),
                veth_name: bridge,
            };
            let instance =
                prepare_instance(&spec).map_err(|e| format!("cannot prepare LXC instance: {e}"))?;
            println!("LXC config: {}", instance.config_path().display());
            if action == "start" {
                let status = instance
                    .start()
                    .map_err(|e| format!("failed to execute lxc-start: {e}"))?;
                if !status.success() {
                    return Err(format!("lxc-start failed with status {status}"));
                }
                println!("started: {}", instance.name);
            }
            Ok(ExitCode::SUCCESS)
        }
        "stop" | "status" => {
            let mut name = "android-runtime".to_string();
            let mut state_dir = default_state_dir();
            while let Some(arg) = remaining.next() {
                match arg.as_str() {
                    "--name" => name = remaining.next().ok_or("--name needs a value")?,
                    "--state-dir" => {
                        state_dir =
                            PathBuf::from(remaining.next().ok_or("--state-dir needs a path")?)
                    }
                    other => return Err(format!("unknown runtime option: {other}")),
                }
            }
            if !valid_instance_name(&name) {
                return Err("invalid instance name".to_string());
            }
            let instance = LxcInstance {
                name,
                lxc_path: state_dir,
            };
            if action == "stop" {
                let status = instance
                    .stop()
                    .map_err(|e| format!("failed to execute lxc-stop: {e}"))?;
                if !status.success() {
                    return Err(format!("lxc-stop failed with status {status}"));
                }
                println!("stopped: {}", instance.name);
            } else {
                println!(
                    "{}",
                    instance
                        .state()
                        .map_err(|e| format!("failed to query LXC: {e}"))?
                );
            }
            Ok(ExitCode::SUCCESS)
        }
        other => Err(format!("unknown runtime action: {other}")),
    }
}

fn apk_info(path: &Path) -> Result<ExitCode, String> {
    let info = inspect(path).map_err(|e| format!("cannot inspect {}: {e}", path.display()))?;
    println!("APK: {}", info.path.display());
    println!("  native code: {}", yes_no(info.has_native_code));
    println!(
        "  ABIs:         {}",
        if info.abis.is_empty() {
            "none".to_string()
        } else {
            info.abis
                .iter()
                .map(|a| a.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        }
    );
    println!("  split-like:   {}", yes_no(info.is_split_like));
    println!(
        "  package:      {}",
        info.package_name.as_deref().unwrap_or("unknown")
    );
    Ok(ExitCode::SUCCESS)
}

fn plan(path: &Path) -> Result<ExitCode, String> {
    let info = inspect(path).map_err(|e| format!("cannot inspect {}: {e}", path.display()))?;
    let host = probe_host();
    let mode = choose_mode(&info, &host);
    println!("Execution plan for {}", path.display());
    println!("  host ISA:      {}", host.arch);
    println!("  selected mode: {}", mode.as_str());
    if mode.as_str() == "arm-translated-container" {
        println!("  reason:        APK has native ARM code; use Android Native Bridge in the lite runtime");
    }
    if mode.as_str() == "compatibility-vm" {
        println!("  reason:        the APK requires an ARM system path that the fast container cannot provide");
        println!("  fallback:      ARM64/foreign-ISA VM backend (optional component)");
    }
    Ok(ExitCode::SUCCESS)
}

fn choose_device(client: &AndroidDeviceClient, serial: Option<&str>) -> Result<String, String> {
    client
        .wait_for_device(serial, Duration::from_secs(15))
        .map_err(|e| format!("device selection failed: {e}"))
}

fn read_serial_option(
    args: &mut impl Iterator<Item = String>,
    serial: &mut Option<String>,
) -> Result<(), String> {
    if serial.is_some() {
        return Err("--serial may only be specified once".to_string());
    }
    let value = args
        .next()
        .ok_or_else(|| "--serial requires a device serial".to_string())?;
    if value.trim().is_empty() || value.starts_with("--") {
        return Err("--serial requires a non-empty device serial".to_string());
    }
    *serial = Some(value);
    Ok(())
}

fn invalid_arguments(error: String) -> ExitCode {
    eprintln!("error: {error}");
    ExitCode::from(2)
}

fn install(path: &Path, serial: Option<&str>, launch: bool) -> Result<ExitCode, String> {
    if !path.is_file() {
        return Err(format!("APK does not exist: {}", path.display()));
    }
    let info = inspect(path).map_err(|e| format!("cannot inspect {}: {e}", path.display()))?;
    let package = info
        .package_name
        .clone()
        .ok_or("could not read AndroidManifest.xml package name")?;
    if launch {
        validate_package_name(&package)
            .map_err(|e| format!("APK contains an invalid package name: {e}"))?;
    }
    let client = AndroidDeviceClient::discover().map_err(|e| e.to_string())?;
    let chosen = choose_device(&client, serial)?;
    client
        .install(path, Some(&chosen))
        .map_err(|e| format!("install failed: {e}"))?;
    println!("installed {package} on {chosen}");
    if launch {
        client
            .launch_package(Some(&chosen), &package)
            .map_err(|e| format!("launch failed: {e}"))?;
        println!("launched {package}");
    }
    Ok(ExitCode::SUCCESS)
}

fn devices() -> Result<ExitCode, String> {
    let client = AndroidDeviceClient::discover().map_err(|e| e.to_string())?;
    let found = client.devices().map_err(|e| e.to_string())?;
    if found.is_empty() {
        println!("No Android devices detected.");
        println!("  ADB executable: {}", client.adb_path().display());
        println!();
        println!(
            "If the development emulator is not running, start it from the project directory:"
        );
        println!("  ./scripts/android/run-dev-android.sh");
        println!();
        println!("Then check the device list directly:");
        println!("  {} devices -l", client.adb_path().display());
        println!();
        println!("If the emulator is already open but missing, wait for Android to finish booting and run this command again.");
        return Ok(ExitCode::SUCCESS);
    }
    println!("Android devices (ADB: {})", client.adb_path().display());
    for d in found {
        println!(
            "{}\t{}{}",
            d.serial,
            d.state,
            if d.transport.is_empty() {
                String::new()
            } else {
                format!("\ttransport:{}", d.transport)
            }
        );
    }
    Ok(ExitCode::SUCCESS)
}

fn launch(package: &str, serial: Option<&str>) -> Result<ExitCode, String> {
    let client = AndroidDeviceClient::discover().map_err(|e| e.to_string())?;
    let chosen = choose_device(&client, serial)?;
    client
        .launch_package(Some(&chosen), package)
        .map_err(|e| format!("launch failed: {e}"))?;
    println!("launched {package} on {chosen}");
    Ok(ExitCode::SUCCESS)
}

fn stop_app(package: &str, serial: Option<&str>) -> Result<ExitCode, String> {
    let client = AndroidDeviceClient::discover().map_err(|e| e.to_string())?;
    let chosen = choose_device(&client, serial)?;
    client
        .stop_package(Some(&chosen), package)
        .map_err(|e| format!("stop failed: {e}"))?;
    println!("stopped {package} on {chosen}");
    Ok(ExitCode::SUCCESS)
}

fn uninstall(package: &str, serial: Option<&str>) -> Result<ExitCode, String> {
    validate_package_name(package).map_err(|error| format!("invalid package name: {error}"))?;
    let client = AndroidDeviceClient::discover().map_err(|error| error.to_string())?;
    let chosen = choose_device(&client, serial)?;
    client
        .uninstall_package(Some(&chosen), package)
        .map_err(|error| format!("uninstall failed: {error}"))?;
    println!("uninstalled {package} from {chosen}");
    Ok(ExitCode::SUCCESS)
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("doctor") => doctor(),
        Some("apk-info") => match args.next() {
            Some(path) => apk_info(Path::new(&path)).unwrap_or_else(|e| {
                eprintln!("error: {e}");
                ExitCode::from(2)
            }),
            None => {
                usage();
                ExitCode::from(2)
            }
        },
        Some("plan") => match args.next() {
            Some(path) => plan(Path::new(&path)).unwrap_or_else(|e| {
                eprintln!("error: {e}");
                ExitCode::from(2)
            }),
            None => {
                usage();
                ExitCode::from(2)
            }
        },
        Some("runtime") => match runtime_command(args) {
            Ok(code) => code,
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::from(2)
            }
        },
        Some("install") => {
            let Some(path) = args.next() else {
                usage();
                return ExitCode::from(2);
            };
            let mut serial = None;
            let mut launch = true;
            while let Some(arg) = args.next() {
                match arg.as_str() {
                    "--serial" => {
                        if let Err(error) = read_serial_option(&mut args, &mut serial) {
                            return invalid_arguments(error);
                        }
                    }
                    "--no-launch" => launch = false,
                    _ => return invalid_arguments(format!("unknown argument: {arg}")),
                }
            }
            install(Path::new(&path), serial.as_deref(), launch).unwrap_or_else(|e| {
                eprintln!("error: {e}");
                ExitCode::from(2)
            })
        }
        Some("devices") => devices().unwrap_or_else(|e| {
            eprintln!("error: {e}");
            ExitCode::from(2)
        }),
        Some("launch") => {
            let Some(package) = args.next() else {
                usage();
                return ExitCode::from(2);
            };
            let mut serial = None;
            while let Some(arg) = args.next() {
                if arg == "--serial" {
                    if let Err(error) = read_serial_option(&mut args, &mut serial) {
                        return invalid_arguments(error);
                    }
                } else {
                    return invalid_arguments(format!("unknown argument: {arg}"));
                }
            }
            launch(&package, serial.as_deref()).unwrap_or_else(|e| {
                eprintln!("error: {e}");
                ExitCode::from(2)
            })
        }
        Some("stop-app") => {
            let Some(package) = args.next() else {
                usage();
                return ExitCode::from(2);
            };
            let mut serial = None;
            while let Some(arg) = args.next() {
                if arg == "--serial" {
                    if let Err(error) = read_serial_option(&mut args, &mut serial) {
                        return invalid_arguments(error);
                    }
                } else {
                    return invalid_arguments(format!("unknown argument: {arg}"));
                }
            }
            stop_app(&package, serial.as_deref()).unwrap_or_else(|e| {
                eprintln!("error: {e}");
                ExitCode::from(2)
            })
        }
        Some("uninstall") => {
            let Some(package) = args.next() else {
                usage();
                return ExitCode::from(2);
            };
            let mut serial = None;
            while let Some(arg) = args.next() {
                if arg == "--serial" {
                    if let Err(error) = read_serial_option(&mut args, &mut serial) {
                        return invalid_arguments(error);
                    }
                } else {
                    return invalid_arguments(format!("unknown argument: {arg}"));
                }
            }
            uninstall(&package, serial.as_deref()).unwrap_or_else(|e| {
                eprintln!("error: {e}");
                ExitCode::from(2)
            })
        }
        _ => {
            usage();
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod cli_tests {
    use super::*;

    #[test]
    fn serial_option_requires_a_value() {
        let mut args = Vec::<String>::new().into_iter();
        let mut serial = None;
        assert!(read_serial_option(&mut args, &mut serial).is_err());
        assert_eq!(serial, None);
    }

    #[test]
    fn serial_option_rejects_duplicates() {
        let mut args = vec!["emulator-5554".to_string()].into_iter();
        let mut serial = Some("usb-123".to_string());
        assert!(read_serial_option(&mut args, &mut serial).is_err());
        assert_eq!(serial.as_deref(), Some("usb-123"));
    }

    #[test]
    fn serial_option_accepts_a_named_device() {
        let mut args = vec!["emulator-5554".to_string()].into_iter();
        let mut serial = None;
        read_serial_option(&mut args, &mut serial).unwrap();
        assert_eq!(serial.as_deref(), Some("emulator-5554"));
    }
}
