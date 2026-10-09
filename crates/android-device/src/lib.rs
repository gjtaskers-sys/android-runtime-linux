use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant};

/// Validate an Android application ID before passing it to `adb shell`.
///
/// Android package IDs are dot-separated identifiers; constraining them here
/// prevents APK-controlled text from being interpreted as remote-shell syntax.
pub fn validate_package_name(package: &str) -> io::Result<()> {
    let mut segments = package.split('.');
    let has_multiple_segments = segments.next().is_some() && segments.next().is_some();
    let valid = has_multiple_segments
        && package.split('.').all(|segment| {
            !segment.is_empty()
                && segment.as_bytes()[0].is_ascii_alphabetic()
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        });

    if valid {
        Ok(())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "invalid Android package name {package:?}; expected dot-separated identifiers starting with a letter"
            ),
        ))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    pub serial: String,
    pub state: String,
    pub transport: String,
}

pub struct AndroidDeviceClient {
    adb: PathBuf,
    android_cli: Option<PathBuf>,
}

impl AndroidDeviceClient {
    pub fn discover() -> io::Result<Self> {
        let adb = find_tool("adb").ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "adb not found; run scripts/android/bootstrap-dev-android.sh or install Android platform-tools"))?;
        let android_cli = std::env::var_os("ANDROID_CLI")
            .map(PathBuf::from)
            .or_else(|| find_tool("android"));
        Ok(Self { adb, android_cli })
    }

    pub fn from_paths(adb: PathBuf, android_cli: Option<PathBuf>) -> Self {
        Self { adb, android_cli }
    }

    /// The exact ADB executable selected by discovery. Useful for diagnostics.
    pub fn adb_path(&self) -> &Path {
        &self.adb
    }

    fn adb_output(&self, args: &[&str]) -> io::Result<Output> {
        Command::new(&self.adb).args(args).output()
    }

    pub fn devices(&self) -> io::Result<Vec<Device>> {
        let out = self.adb_output(&["devices", "-l"])?;
        if !out.status.success() {
            return Err(io::Error::other(
                String::from_utf8_lossy(&out.stderr).trim().to_string(),
            ));
        }
        let text = String::from_utf8_lossy(&out.stdout);
        let mut result = Vec::new();
        for line in text.lines().skip(1) {
            let mut fields = line.split_whitespace();
            let Some(serial) = fields.next() else {
                continue;
            };
            let Some(state) = fields.next() else { continue };
            if state == "no permissions" || serial.is_empty() {
                continue;
            }
            let transport = fields
                .find_map(|f| f.strip_prefix("transport_id:"))
                .unwrap_or("")
                .to_string();
            result.push(Device {
                serial: serial.to_string(),
                state: state.to_string(),
                transport,
            });
        }
        Ok(result)
    }

    pub fn wait_for_device(&self, serial: Option<&str>, timeout: Duration) -> io::Result<String> {
        if let Some(value) = serial {
            if value.trim().is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "device serial must not be empty",
                ));
            }
        }
        let start = Instant::now();
        loop {
            let devices = self.devices()?;
            if let Some(chosen) = choose_online_device(&devices, serial)? {
                return Ok(chosen);
            }
            if start.elapsed() >= timeout {
                let target = serial
                    .map(|s| format!(" with serial {s}"))
                    .unwrap_or_default();
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!("timed out waiting for an online Android device{target}"),
                ));
            }
            std::thread::sleep(Duration::from_millis(500));
        }
    }

    pub fn install(&self, apk: &Path, serial: Option<&str>) -> io::Result<()> {
        if !apk.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("APK not found: {}", apk.display()),
            ));
        }

        let mut android_cli_failure = None;
        if let Some(android) = &self.android_cli {
            let mut cmd = Command::new(android);
            cmd.args(["--no-metrics", "install", "--no-delta"]);
            if let Some(s) = serial {
                cmd.arg(format!("--device={s}"));
            }
            cmd.arg(format!("--apks={}", apk.display()));
            let out = cmd.output()?;
            if out.status.success() {
                return Ok(());
            }
            android_cli_failure = Some(format!(
                "Android CLI ({}): {}",
                out.status,
                summarize_output(&out),
            ));
        }

        let args0 = ["install", "-r", "-d"];
        let mut args: Vec<&str> = Vec::new();
        if let Some(s) = serial {
            args.extend(["-s", s]);
        }
        args.extend(args0);
        let mut cmd = Command::new(&self.adb);
        cmd.args(&args).arg(apk);
        let out = cmd.output()?;
        if out.status.success() {
            Ok(())
        } else {
            let target = serial.unwrap_or("<automatic selection>");
            let mut message = format!(
                "adb install failed for device {target} ({}): {}",
                out.status,
                summarize_output(&out),
            );
            if let Some(cli_error) = android_cli_failure {
                message.push_str("; ");
                message.push_str(&cli_error);
            }
            Err(io::Error::other(message))
        }
    }

    /// Uninstall a package from a selected online device using ADB's argument-safe command form.
    pub fn uninstall_package(&self, serial: Option<&str>, package: &str) -> io::Result<()> {
        validate_package_name(package)?;

        let mut command = Command::new(&self.adb);
        if let Some(serial) = serial {
            command.args(["-s", serial]);
        }
        let output = command.args(["uninstall", package]).output()?;
        if output.status.success() {
            Ok(())
        } else {
            let target = serial.unwrap_or("<automatic selection>");
            Err(io::Error::other(format!(
                "adb uninstall failed for package {package} on device {target} ({}): {}",
                output.status,
                summarize_output(&output),
            )))
        }
    }

    pub fn shell(&self, serial: Option<&str>, command: &str, args: &[&str]) -> io::Result<String> {
        let mut adb_args: Vec<&str> = Vec::new();
        if let Some(s) = serial {
            adb_args.extend(["-s", s]);
        }
        adb_args.push("shell");
        adb_args.push(command);
        adb_args.extend_from_slice(args);
        let out = self.adb_output(&adb_args)?;
        if !out.status.success() {
            return Err(io::Error::other(
                String::from_utf8_lossy(&out.stderr).trim().to_string(),
            ));
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    pub fn launch_package(&self, serial: Option<&str>, package: &str) -> io::Result<()> {
        validate_package_name(package)?;
        let out = self.shell(
            serial,
            "monkey",
            &[
                "-p",
                package,
                "-c",
                "android.intent.category.LAUNCHER",
                "--dbg-no-events",
                "1",
            ],
        )?;
        if out.is_empty() || out.contains("No activities found") || out.contains("monkey aborted") {
            return Err(io::Error::other(format!(
                "could not launch {package}: {out}"
            )));
        }
        Ok(())
    }

    pub fn stop_package(&self, serial: Option<&str>, package: &str) -> io::Result<()> {
        validate_package_name(package)?;
        self.shell(serial, "am", &["force-stop", package])
            .map(|_| ())
    }
}

fn choose_online_device(
    devices: &[Device],
    requested_serial: Option<&str>,
) -> io::Result<Option<String>> {
    let online: Vec<&Device> = devices
        .iter()
        .filter(|device| device.state == "device")
        .collect();
    if let Some(serial) = requested_serial {
        return Ok(online
            .into_iter()
            .find(|device| device.serial.as_str() == serial)
            .map(|device| device.serial.clone()));
    }

    match online.as_slice() {
        [] => Ok(None),
        [device] => Ok(Some(device.serial.clone())),
        _ => {
            let serials = online
                .iter()
                .map(|device| device.serial.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("multiple Android devices are online ({serials}); specify one with --serial SERIAL"),
            ))
        }
    }
}

fn summarize_output(output: &Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    match (stdout.is_empty(), stderr.is_empty()) {
        (true, true) => "the command returned no diagnostic output".to_string(),
        (false, true) => format!("stdout: {stdout}"),
        (true, false) => format!("stderr: {stderr}"),
        (false, false) => format!("stdout: {stdout}; stderr: {stderr}"),
    }
}

fn find_tool(name: &str) -> Option<PathBuf> {
    // Prefer the SDK installed by our bootstrap script. In particular, an
    // older /usr/bin/adb can otherwise win even when the SDK's platform-tools
    // is present in ~/android-sdk.
    let mut roots: Vec<PathBuf> = Vec::new();
    for value in ["ANDROID_SDK_ROOT", "ANDROID_HOME"] {
        if let Some(root) = std::env::var_os(value).map(PathBuf::from) {
            if !roots.contains(&root) {
                roots.push(root);
            }
        }
    }
    if let Some(home) = std::env::var_os("HOME") {
        let root = PathBuf::from(home).join("android-sdk");
        if !roots.contains(&root) {
            roots.push(root);
        }
    }

    for root in roots {
        let candidate = match name {
            "adb" => root.join("platform-tools/adb"),
            "emulator" => root.join("emulator/emulator"),
            "android" => root.join("cmdline-tools/latest/bin/android"),
            other => root.join("cmdline-tools/latest/bin").join(other),
        };
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let p = dir.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_adb_device_listing_with_transport_id() {
        let text = "List of devices attached\nemulator-5554 device product:sdk_gphone64_x86_64 transport_id:1\n";
        let mut result = Vec::new();
        for line in text.lines().skip(1) {
            let mut fields = line.split_whitespace();
            let Some(serial) = fields.next() else {
                continue;
            };
            let Some(state) = fields.next() else { continue };
            if state == "no permissions" || serial.is_empty() {
                continue;
            }
            let transport = fields
                .find_map(|f| f.strip_prefix("transport_id:"))
                .unwrap_or("")
                .to_string();
            result.push(Device {
                serial: serial.to_string(),
                state: state.to_string(),
                transport,
            });
        }
        assert_eq!(
            result,
            vec![Device {
                serial: "emulator-5554".to_string(),
                state: "device".to_string(),
                transport: "1".to_string(),
            }]
        );
    }

    #[test]
    fn validates_normal_android_package_names() {
        for package in ["org.andstatus.game2048", "com.example_app.app2", "A.B_C"] {
            assert!(validate_package_name(package).is_ok(), "{package:?}");
        }
    }

    #[test]
    fn rejects_invalid_or_shell_like_package_names() {
        for package in [
            "",
            "org",
            ".org.app",
            "org..app",
            "org.app.",
            "org.123app",
            "org.app;id",
            "org.app $(id)",
            "org.app\necho",
        ] {
            assert!(validate_package_name(package).is_err(), "{package:?}");
        }
    }

    #[test]
    fn uninstall_rejects_invalid_package_before_running_adb() {
        let client =
            AndroidDeviceClient::from_paths(PathBuf::from("/definitely/not/a/real/adb"), None);
        let error = client.uninstall_package(None, "org.app;id").unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("invalid Android package name"));
    }

    #[test]
    fn empty_requested_serial_is_rejected() {
        let client =
            AndroidDeviceClient::from_paths(PathBuf::from("/definitely/not/a/real/adb"), None);
        let error = client
            .wait_for_device(Some("  "), Duration::from_millis(1))
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("serial"));
    }

    #[test]
    fn automatic_selection_requires_exactly_one_online_device() {
        let one = vec![Device {
            serial: "emulator-5554".into(),
            state: "device".into(),
            transport: "1".into(),
        }];
        assert_eq!(
            choose_online_device(&one, None).unwrap(),
            Some("emulator-5554".into())
        );

        let two = vec![
            Device {
                serial: "emulator-5554".into(),
                state: "device".into(),
                transport: "1".into(),
            },
            Device {
                serial: "usb-123".into(),
                state: "device".into(),
                transport: "2".into(),
            },
        ];
        let error = choose_online_device(&two, None).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(error.to_string().contains("--serial SERIAL"));
        assert_eq!(
            choose_online_device(&two, Some("usb-123")).unwrap(),
            Some("usb-123".into())
        );
        assert_eq!(
            choose_online_device(&two, Some("missing-device")).unwrap(),
            None
        );
    }
}
