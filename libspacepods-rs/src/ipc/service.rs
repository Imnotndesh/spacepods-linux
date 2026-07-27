use crate::commands::AncController;
use crate::connection::{DeviceScanner, ConnectionManager};
use crate::ipc::protocol::*;
use crate::protocol::{AncMode, ConnectionState, TlvParser,
    ID_DEVICE_POWER,
    ID_ANC_MODE, ID_ANC_GAIN, ID_ANC_MAX, ID_TRANS_GAIN, ID_TRANS_MAX,
    ID_EQ_SETTING, ID_ENV_ADAPTIVE, ID_DUAL_DEVICE, ID_KEY_SETTINGS,
    ID_WORK_MODE, ID_IN_EAR_STATUS, ID_AUTO_ANSWER,
    ID_TONE_VOLUME, ID_HEARING_CARE, ID_VOICE_PROMPT,
};
use crate::{Error, Result, SpaceBuds};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use btleplug::api::Peripheral;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{broadcast, Mutex, RwLock};
use tokio::time;

/// Unix socket daemon that exposes SpaceBuds controls via JSON IPC.
///
/// Architecture:
/// - Single background task polls device status periodically
/// - Each client connection is handled in a spawn'd task
/// - Status updates are pushed to subscribed clients via the notification channel
/// - Battery tracking has been removed — the OS Bluetooth stack handles this.
pub struct SpacePodsService {
    buds: SpaceBuds,
    status: Arc<RwLock<DeviceStatus>>,
    status_tx: broadcast::Sender<DeviceStatus>,
    socket_path: PathBuf,
    running: Arc<Mutex<bool>>,
}

impl SpacePodsService {
    pub async fn new(socket_path: Option<PathBuf>) -> Self {
        let buds = SpaceBuds::new_disconnected();
        let (status_tx, _) = broadcast::channel(32);
        let status = Arc::new(RwLock::new(DeviceStatus::default_disconnected()));
        let socket_path = socket_path.unwrap_or_else(|| PathBuf::from(DEFAULT_SOCKET_PATH));
        Self {
            buds,
            status,
            status_tx,
            socket_path,
            running: Arc::new(Mutex::new(false)),
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        *self.running.lock().await = true;

        // Spawn the background status updater
        let buds = self.buds.clone();
        let status = self.status.clone();
        let status_tx = self.status_tx.clone();
        let running = self.running.clone();

        tokio::spawn(async move {
            Self::status_updater_loop(buds, status, status_tx, running).await;
        });

        // Set up Unix socket listener
        #[cfg(unix)]
        {
            if self.socket_path.exists() {
                std::fs::remove_file(&self.socket_path).ok();
            }
        }

        let listener = UnixListener::bind(&self.socket_path)
            .map_err(|e| Error::Ipc(format!("Failed to bind socket: {}", e)))?;

        #[cfg(unix)]
        {
            use std::fs::Permissions;
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.socket_path, Permissions::from_mode(0o666)).ok();
        }

        println!("SpacePods service listening on {}", self.socket_path.display());

        // Accept loop
        while *self.running.lock().await {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let buds = self.buds.clone();
                    let status = self.status.clone();
                    let status_tx = self.status_tx.clone();

                    tokio::spawn(async move {
                        Self::handle_client(stream, buds, status, status_tx).await;
                    });
                }
                Err(e) => {
                    eprintln!("Error accepting connection: {}", e);
                }
            }
        }

        Ok(())
    }

    pub async fn stop(&self) {
        let mut running = self.running.lock().await;
        *running = false;
    }

    // ── Background Status Loop ──

    async fn status_updater_loop(
        buds: SpaceBuds,
        status: Arc<RwLock<DeviceStatus>>,
        status_tx: broadcast::Sender<DeviceStatus>,
        running: Arc<Mutex<bool>>,
    ) {
        let mut reconnect_delay = Duration::from_secs(1);

        // Initial refresh
        Self::refresh_full_status(&buds, &status, &status_tx).await;

        while *running.lock().await {
            let connected = buds.is_connected().await;

            if connected {
                reconnect_delay = Duration::from_secs(1);

                tokio::select! {
                    // Periodic full status refresh (no battery polling — OS handles it)
                    _ = time::sleep(Duration::from_secs(30)) => {
                        Self::refresh_full_status(&buds, &status, &status_tx).await;
                    }
                    _ = tokio::signal::ctrl_c() => {
                        break;
                    }
                }
            } else {
                // Mark as disconnected
                {
                    let mut status_lock = status.write().await;
                    status_lock.connection.connected = false;
                    status_lock.connection.state = ConnectionState::Disconnected;
                }
                let current_status = status.read().await.clone();
                let _ = status_tx.send(current_status);

                // Try to reconnect
                let _ = buds.reconnect().await;
                time::sleep(reconnect_delay).await;
                reconnect_delay = (reconnect_delay * 2).min(Duration::from_secs(30));
            }
        }
    }

    async fn refresh_full_status(
        buds: &SpaceBuds,
        status: &Arc<RwLock<DeviceStatus>>,
        status_tx: &broadcast::Sender<DeviceStatus>,
    ) {
        if !buds.is_connected().await {
            return;
        }

        let mut status_lock = status.write().await;
        let is_first = status_lock.product_id.is_none();

        // First refresh: bulk query all device info
        if is_first {
            drop(status_lock);
            match buds.query_device_info(&[
                ID_DEVICE_POWER,
                ID_ANC_MODE, ID_ANC_GAIN, ID_ANC_MAX, ID_TRANS_GAIN, ID_TRANS_MAX,
                ID_EQ_SETTING, ID_ENV_ADAPTIVE, ID_DUAL_DEVICE, ID_KEY_SETTINGS,
                ID_WORK_MODE, ID_IN_EAR_STATUS, ID_AUTO_ANSWER,
                ID_TONE_VOLUME, ID_HEARING_CARE, ID_VOICE_PROMPT,
            ]).await {
                Ok(payload) => {
                    let mut parser = TlvParser::new(&payload);
                    crate::log::info("IPC", "Bulk device info query OK");
                    let mut status_lock = status.write().await;
                    status_lock.connection.connected = true;
                    status_lock.connection.address = buds.address().await;
                    status_lock.connection.state = ConnectionState::Connected;

                    // Battery: [left, right, case] each byte bit7=charging, bits0-6=percentage
                    if let Some(data) = parser.get_bytes(ID_DEVICE_POWER) {
                        crate::log::full("IPC", &format!("Battery raw: {:02x?}", data));
                        status_lock.battery.left = data.first().map(|&b| b & 0x7F);
                        status_lock.battery.right = data.get(1).map(|&b| b & 0x7F);
                        status_lock.battery.case = data.get(2).map(|&b| b & 0x7F);
                    }

                    // ANC
                    let anc_mode = parser.get_int(ID_ANC_MODE)
                        .and_then(|v| AncMode::from_u8(v as u8))
                        .unwrap_or(AncMode::Off);
                    status_lock.anc.mode = anc_mode;
                    status_lock.anc.level = parser.get_int(ID_ANC_GAIN).unwrap_or(0) as u8;
                    status_lock.anc.max_level = parser.get_int(ID_ANC_MAX).unwrap_or(0) as u8;

                    if let Some(data) = parser.get_bytes(ID_EQ_SETTING) {
                        if data.len() >= 2 {
                            status_lock.eq = Some(EqInfo { mode: data[0], name: String::new(), description: String::new(), gains: vec![] });
                        }
                    }
                    status_lock.features.adaptive_anc = parser.get_int(ID_ENV_ADAPTIVE).map(|v| v == 1);
                    status_lock.features.dual_device = parser.get_int(ID_DUAL_DEVICE).map(|v| v == 1);

                    if let Some(data) = parser.get_bytes(ID_KEY_SETTINGS) {
                        let mut map = std::collections::HashMap::new();
                        let mut i = 0;
                        while i + 2 < data.len() {
                            if data[i + 1] == 1 && i + 3 <= data.len() {
                                map.insert(data[i], data[i + 2]);
                            }
                            i += 2 + data[i + 1] as usize;
                        }
                        if !map.is_empty() { status_lock.key_settings = Some(map); }
                    }
                    if status_lock.product_id.is_none() {
                        if let Some(pid) = buds.detect_product_id().await {
                            crate::log::info("IPC", &format!("product_id={} ({})", pid, crate::device_profile::profile_for_product(pid).name));
                            status_lock.product_id = Some(pid);
                        }
                    }
                    let new_status = status_lock.clone();
                    let _ = status_tx.send(new_status);
                    return;
                }
                Err(_) => { /* fall through */ }
            }
        }

        // Individual queries
        let anc_mode = buds.anc().get_mode().await.unwrap_or(AncMode::Off);
        let (level, max_level) = buds.anc().get_level().await.unwrap_or((0, 0));
        let eq_state = buds.eq().get_state().await.unwrap_or(None);
        let adaptive = buds.features().get_adaptive_anc().await.ok();
        let dual = buds.features().get_dual_device().await.ok();

        let mut status_lock = status.write().await;
        status_lock.connection.connected = true;
        status_lock.connection.address = buds.address().await;
        status_lock.connection.state = ConnectionState::Connected;
        status_lock.anc.mode = anc_mode;
        status_lock.anc.level = level;
        status_lock.anc.max_level = max_level;
        if let Some(eq) = eq_state {
            status_lock.eq = Some(EqInfo { mode: eq.mode, name: eq.name, description: eq.description, gains: eq.gains });
        }
        status_lock.features.adaptive_anc = adaptive;
        status_lock.features.dual_device = dual;
        if status_lock.product_id.is_none() {
            if let Some(pid) = buds.detect_product_id().await {
                crate::log::info("IPC", &format!("product_id={} ({})", pid, crate::device_profile::profile_for_product(pid).name));
                status_lock.product_id = Some(pid);
            }
        }
        match buds.features().get_key_settings().await {
            Ok(map) => { status_lock.key_settings = Some(map); }
            Err(_) => {}
        }

        let new_status = status_lock.clone();
        let _ = status_tx.send(new_status);
    }

    // ── Client Handling ──

    async fn handle_client(
        mut stream: UnixStream,
        buds: SpaceBuds,
        status: Arc<RwLock<DeviceStatus>>,
        status_tx: broadcast::Sender<DeviceStatus>,
    ) {
        let (reader, mut writer) = stream.split();
        let mut reader = BufReader::new(reader);
        let mut line = String::new();
        let mut subscribed = false;
        let mut status_rx = status_tx.subscribe();

        loop {
            tokio::select! {
                read_result = reader.read_line(&mut line) => {
                    match read_result {
                        Ok(0) => break,
                        Ok(_) => {
                            // Parse incoming message
                            let response = match serde_json::from_str::<ServiceCommand>(line.trim()) {
                                Ok(cmd) => {
                                    if matches!(cmd, ServiceCommand::Subscribe) {
                                        subscribed = true;
                                        IpcResult::Success {
                                            message: Some("Subscribed to status updates".to_string()),
                                            data: None,
                                        }
                                    } else if matches!(cmd, ServiceCommand::Unsubscribe) {
                                        subscribed = false;
                                        IpcResult::Success {
                                            message: Some("Unsubscribed from status updates".to_string()),
                                            data: None,
                                        }
                                    } else {
                                        Self::execute_command(cmd, &buds, &status).await
                                    }
                                }
                                Err(e) => IpcResult::Error {
                                    message: format!("Invalid command: {}", e),
                                },
                            };

                            // Send response
                            let response_json = serde_json::to_string(&response).unwrap() + "\n";
                            if writer.write_all(response_json.as_bytes()).await.is_err() {
                                break;
                            }
                            if writer.flush().await.is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                    line.clear();
                }

                // Push status updates to subscribed clients
                status_update = status_rx.recv() => {
                    if subscribed {
                        if let Ok(new_status) = status_update {
                            let response = IpcResult::StatusUpdate { status: new_status };
                            let response_json = serde_json::to_string(&response).unwrap() + "\n";
                            if writer.write_all(response_json.as_bytes()).await.is_err() {
                                break;
                            }
                            if writer.flush().await.is_err() {
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    // ── Command Execution ──

    async fn execute_command(
        cmd: ServiceCommand,
        buds: &SpaceBuds,
        status: &Arc<RwLock<DeviceStatus>>,
    ) -> IpcResult {
        match cmd {
            ServiceCommand::Ping => IpcResult::Success {
                message: Some("pong".to_string()),
                data: None,
            },

            ServiceCommand::GetStatus => {
                let status = status.read().await.clone();
                let data = serde_json::to_value(status).unwrap_or(serde_json::Value::Null);
                IpcResult::Success {
                    message: None,
                    data: Some(data),
                }
            }

            ServiceCommand::Scan { timeout_secs } => {
                match DeviceScanner::scan_devices(Duration::from_secs(timeout_secs)).await {
                    Ok(devices) => IpcResult::ScanResults { devices },
                    Err(e) => IpcResult::Error {
                        message: format!("Scan failed: {}", e),
                    },
                }
            }


            ServiceCommand::Connect { .. } => {
                if buds.is_connected().await {
                    return IpcResult::Success {
                        message: Some("Already connected".to_string()),
                        data: None,
                    };
                }
                match buds.connect().await {
                    Ok(_) => IpcResult::Success {
                        message: Some("Connected".to_string()),
                        data: None,
                    },
                    Err(e) => IpcResult::Error {
                        message: format!("Connection failed: {}", e),
                    },
                }
            }

            ServiceCommand::SetAncMode { mode } => {
                let mode_val = match mode.as_str() {
                    "off" | "0" => AncMode::Off,
                    "on" | "1" | "anc" => AncMode::Active,
                    "transparency" | "2" => AncMode::Transparency,
                    _ => {
                        return IpcResult::Error {
                            message: format!("Invalid ANC mode: {}", mode),
                        };
                    }
                };

                match buds.anc().set_mode(mode_val).await {
                    Ok(_) => {
                        // Refresh status after a short delay
                        let buds = buds.clone();
                        let status = status.clone();
                        tokio::spawn(async move {
                            time::sleep(Duration::from_millis(200)).await;
                            if let Ok(mode) = buds.anc().get_mode().await {
                                let mut s = status.write().await;
                                s.anc.mode = mode;
                            }
                        });

                        IpcResult::Success {
                            message: Some(format!("ANC mode set to {}", mode)),
                            data: None,
                        }
                    }
                    Err(e) => IpcResult::Error {
                        message: format!("Failed to set ANC mode: {}", e),
                    },
                }
            }

            ServiceCommand::SetLevel { level } => {
                match buds.anc().set_level(level).await {
                    Ok(true) => {
                        let buds = buds.clone();
                        let status = status.clone();
                        tokio::spawn(async move {
                            time::sleep(Duration::from_millis(200)).await;
                            let (lvl, max) = buds.anc().get_level().await.unwrap_or((0, 0));
                            let mut s = status.write().await;
                            s.anc.level = lvl;
                            s.anc.max_level = max;
                        });

                        IpcResult::Success {
                            message: Some(format!("Level set to {}", level)),
                            data: None,
                        }
                    }
                    Ok(false) => IpcResult::Error {
                        message: "Cannot set level when ANC is off".to_string(),
                    },
                    Err(e) => IpcResult::Error {
                        message: format!("Failed to set level: {}", e),
                    },
                }
            }

            ServiceCommand::SetEqPreset { preset } => {
                match buds.eq().set_preset(preset).await {
                    Ok(_) => {
                        let buds = buds.clone();
                        let status = status.clone();
                        tokio::spawn(async move {
                            time::sleep(Duration::from_millis(500)).await;
                            if let Ok(Some(eq)) = buds.eq().get_state().await {
                                let mut s = status.write().await;
                                s.eq = Some(EqInfo {
                                    mode: eq.mode,
                                    name: eq.name,
                                    description: eq.description,
                                    gains: eq.gains,
                                });
                            }
                        });

                        IpcResult::Success {
                            message: Some(format!("EQ preset set to {}", preset)),
                            data: None,
                        }
                    }
                    Err(e) => IpcResult::Error {
                        message: format!("Failed to set EQ preset: {}", e),
                    },
                }
            }

            ServiceCommand::SetAdaptiveAnc { enabled } => {
                match buds.features().set_adaptive_anc(enabled).await {
                    Ok(_) => {
                        let buds = buds.clone();
                        let status = status.clone();
                        tokio::spawn(async move {
                            time::sleep(Duration::from_millis(200)).await;
                            if let Ok(adaptive) = buds.features().get_adaptive_anc().await {
                                let mut s = status.write().await;
                                s.features.adaptive_anc = Some(adaptive);
                            }
                        });

                        IpcResult::Success {
                            message: Some(format!(
                                "Adaptive ANC {}",
                                if enabled { "enabled" } else { "disabled" }
                            )),
                            data: None,
                        }
                    }
                    Err(e) => IpcResult::Error {
                        message: format!("Failed to set adaptive ANC: {}", e),
                    },
                }
            }

            ServiceCommand::SetDualDevice { enabled } => {
                match buds.features().set_dual_device(enabled).await {
                    Ok(_) => {
                        let buds = buds.clone();
                        let status = status.clone();
                        tokio::spawn(async move {
                            time::sleep(Duration::from_millis(200)).await;
                            if let Ok(dual) = buds.features().get_dual_device().await {
                                let mut s = status.write().await;
                                s.features.dual_device = Some(dual);
                            }
                        });

                        IpcResult::Success {
                            message: Some(format!(
                                "Dual device {}",
                                if enabled { "enabled" } else { "disabled" }
                            )),
                            data: None,
                        }
                    }
                    Err(e) => IpcResult::Error {
                        message: format!("Failed to set dual device: {}", e),
                    },
                }
            }

            // ── Custom raw BLE command ──
            ServiceCommand::Custom { command_id, payload } => {
                match buds.send_raw(command_id, payload).await {
                    Ok(_) => IpcResult::Success {
                        message: Some("Custom command sent".to_string()),
                        data: None,
                    },
                    Err(e) => IpcResult::Error {
                        message: format!("Custom command failed: {}", e),
                    },
                }
            }

            _ => IpcResult::Error {
                message: "Command not handled".to_string(),
            },
        }
    }
}

impl Drop for SpacePodsService {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            let _ = std::fs::remove_file(&self.socket_path);
        }
    }
}
