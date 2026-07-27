use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

/// Manages the SpacePods daemon process lifecycle.
/// The daemon handles all Bluetooth LE communication with the earbuds.
pub struct DaemonManager {
    process: Mutex<Option<Child>>,
    socket_path: String,
}

impl DaemonManager {
    pub fn new() -> Self {
        Self {
            process: Mutex::new(None),
            socket_path: "/tmp/spacepods.sock".to_string(),
        }
    }

    /// Start the daemon if it's not already running.
    /// Returns true if the daemon is (or became) available.
    pub fn ensure_running(&self) -> bool {
        if self.is_running() {
            return true;
        }

        eprintln!("Daemon not found, spawning...");
        let exe = std::env::current_exe()
            .unwrap_or_else(|_| "spacepods".into());

        match Command::new(&exe)
            .arg("service")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            Ok(child) => {
                let mut proc = self.process.lock().unwrap();
                *proc = Some(child);
                // Give daemon time to start listening
                std::thread::sleep(Duration::from_millis(1200));
                self.is_running()
            }
            Err(e) => {
                eprintln!("Failed to spawn daemon: {}", e);
                // Try as a separate binary name
                match Command::new("libspacepods")
                    .arg("service")
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                {
                    Ok(child) => {
                        let mut proc = self.process.lock().unwrap();
                        *proc = Some(child);
                        std::thread::sleep(Duration::from_millis(1200));
                        self.is_running()
                    }
                    Err(e2) => {
                        eprintln!("Also failed to spawn libspacepods: {}", e2);
                        false
                    }
                }
            }
        }
    }

    /// Check if the daemon socket is reachable.
    pub fn is_running(&self) -> bool {
        std::path::Path::new(&self.socket_path).exists()
            && std::fs::metadata(&self.socket_path).is_ok()
    }

    /// Stop the daemon process.
    pub fn stop(&self) {
        if let Ok(mut proc) = self.process.lock() {
            if let Some(ref mut child) = *proc {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
        // Also try pkill as fallback
        let _ = Command::new("pkill")
            .args(["-f", "spacepods service"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
    }

    /// Restart the daemon.
    pub fn restart(&self) -> bool {
        self.stop();
        std::thread::sleep(Duration::from_millis(300));
        self.ensure_running()
    }

    /// Check if the daemon is responsive by attempting a socket connection.
    pub async fn check_responsive() -> bool {
        libspacepods::client::SpacePodsClient::connect(None).await.is_ok()
    }
}

impl Drop for DaemonManager {
    fn drop(&mut self) {
        self.stop();
    }
}

// ── Legacy compatibility functions ──

/// Ensure the daemon is running. Kept for backward compatibility.
pub fn ensure_daemon_running() {
    let manager = DaemonManager::new();
    manager.ensure_running();
}

/// Check if the daemon is reachable.
pub async fn check_daemon_running() -> bool {
    DaemonManager::check_responsive().await
}

/// Write or remove the autostart .desktop entry.
pub fn write_autostart_entry(enable: bool) {
    let path = glib::user_config_dir()
        .join("autostart")
        .join("spacepods.desktop");
    if enable {
        let content = "[Desktop Entry]\n\
            Type=Application\n\
            Name=SpacePods\n\
            Exec=spacepods\n\
            Icon=audio-headset\n\
            Comment=SpacePods earbuds manager\n\
            X-GNOME-Autostart-enabled=true\n";
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&path, content);
    } else {
        let _ = std::fs::remove_file(&path);
    }
}

/// Spawn the daemon process (legacy function used by settings page).
pub fn spawn_daemon() {
    let exe = std::env::current_exe().unwrap_or_else(|_| "spacepods".into());
    let _ = Command::new(exe)
        .arg("service")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
}

/// Kill the daemon process.
pub fn kill_daemon() {
    let _ = Command::new("pkill")
        .args(["-f", "spacepods service"])
        .spawn();
}
