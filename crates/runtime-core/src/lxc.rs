use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

#[derive(Debug, Clone)]
pub struct LxcSpec {
    pub name: String,
    pub rootfs: PathBuf,
    pub render_nodes: Vec<PathBuf>,
    pub binder_nodes: Vec<PathBuf>,
    pub display_socket: PathBuf,
    pub userdata: PathBuf,
    pub work_dir: PathBuf,
    pub veth_name: String,
}

/// Generate only the Android-specific part of an LXC configuration.
///
/// The product deliberately does not copy Waydroid's broad device list. The
/// first runtime should expose the minimum hardware surface: one DRM render
/// node, one display socket, virtual networking, and binderfs devices created
/// by the host setup layer.
pub fn write_config(spec: &LxcSpec, path: impl AsRef<Path>) -> io::Result<()> {
    let mut cfg = String::new();
    cfg.push_str("lxc.init.cmd = /init\n");
    cfg.push_str("lxc.autodev = 1\n");
    cfg.push_str("lxc.mount.auto = proc:rw sys:ro cgroup:ro\n");
    cfg.push_str("lxc.pty.max = 0\n");
    cfg.push_str("lxc.mount.entry = tmpfs run/android-runtime tmpfs nosuid,nodev,mode=0755 0 0\n");
    cfg.push_str("lxc.console.path = none\n");
    cfg.push_str("lxc.uts.name = android-runtime\n");
    cfg.push_str("lxc.apparmor.allow_nesting = 0\n");
    cfg.push_str("lxc.no_new_privs = 1\n");
    cfg.push_str("lxc.cap.drop = SYS_MODULE SYS_RAWIO SYS_BOOT SYS_TIME SYS_PTRACE\n");
    cfg.push_str("lxc.rootfs.path = ");
    cfg.push_str(&spec.rootfs.display().to_string());
    cfg.push('\n');

    for node in &spec.render_nodes {
        if node.exists() {
            if let Some(name) = node.file_name().and_then(|s| s.to_str()) {
                cfg.push_str(&format!(
                    "lxc.mount.entry = {} dev/dri/{} none bind,create=file,rw 0 0\n",
                    node.display(),
                    name
                ));
            }
        }
    }

    for node in &spec.binder_nodes {
        if node.exists() {
            if let Some(name) = node.file_name().and_then(|s| s.to_str()) {
                cfg.push_str(&format!(
                    "lxc.mount.entry = {} dev/{} none bind,create=file,rw 0 0\n",
                    node.display(),
                    name
                ));
            }
        }
    }

    fs::create_dir_all(&spec.userdata)?;
    cfg.push_str(&format!(
        "lxc.mount.entry = {} data none bind,create=dir,rw 0 0\n",
        spec.userdata.display()
    ));

    cfg.push_str(&format!(
        "lxc.mount.entry = {} run/android-runtime/display.sock none bind,create=file,rw 0 0\n",
        spec.display_socket.display()
    ));

    cfg.push_str("lxc.net.0.type = veth\n");
    cfg.push_str(&format!("lxc.net.0.link = {}\n", spec.veth_name));
    cfg.push_str("lxc.net.0.flags = up\n");
    cfg.push_str("lxc.net.0.name = eth0\n");
    cfg.push_str("lxc.net.0.hwaddr = 02:41:52:54:00:01\n");

    fs::create_dir_all(spec.work_dir.parent().unwrap_or(Path::new(".")))?;
    fs::write(path, cfg)
}

#[derive(Debug, Clone)]
pub struct LxcInstance {
    pub name: String,
    pub lxc_path: PathBuf,
}

impl LxcInstance {
    pub fn config_path(&self) -> PathBuf {
        self.lxc_path.join(&self.name).join("config")
    }

    pub fn start(&self) -> io::Result<ExitStatus> {
        Command::new("lxc-start")
            .args([
                "-P",
                self.lxc_path.to_string_lossy().as_ref(),
                "-n",
                &self.name,
                "-d",
            ])
            .status()
    }

    pub fn stop(&self) -> io::Result<ExitStatus> {
        Command::new("lxc-stop")
            .args([
                "-P",
                self.lxc_path.to_string_lossy().as_ref(),
                "-n",
                &self.name,
                "-k",
            ])
            .status()
    }

    pub fn state(&self) -> io::Result<String> {
        let out = Command::new("lxc-info")
            .args([
                "-P",
                self.lxc_path.to_string_lossy().as_ref(),
                "-n",
                &self.name,
                "-s",
            ])
            .output()?;
        if !out.status.success() {
            return Ok("UNKNOWN".to_string());
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }
}

pub fn prepare_instance(spec: &LxcSpec) -> io::Result<LxcInstance> {
    let instance = LxcInstance {
        name: spec.name.clone(),
        lxc_path: spec.work_dir.clone(),
    };
    let dir = instance.lxc_path.join(&instance.name);
    fs::create_dir_all(dir)?;
    write_config(spec, instance.config_path())?;
    Ok(instance)
}
