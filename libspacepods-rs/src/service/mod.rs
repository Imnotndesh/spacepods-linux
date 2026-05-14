use crate::{SpaceBuds, SpaceBudsError, Result};
use crate::protocol::{MODE_OFF, MODE_ANC, MODE_TRANSPARENCY};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::{broadcast, Mutex, RwLock};
use tokio::time;

#[cfg(unix)]
pub const DEFAULT_SOCKET_PATH: &str = "/tmp/spacepods.sock";

#[cfg(windows)]
pub const DEFAULT_SOCKET_PATH: &str = r"\\.\pipe\spacepods";

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "cmd", content = "args")]
pub enum ServiceCommand {
    #[serde(rename = "ping")]
    Ping,

    #[serde(rename = "status")]
    GetStatus,

    #[serde(rename = "anc")]
    SetAncMode { mode: String },

    #[serde(rename = "level")]
    SetLevel { level: u8 },

    #[serde(rename = "eq")]
    SetEqPreset { preset: u8 },

    #[serde(rename = "adaptive")]
    SetAdaptiveAnc { enabled: bool },

    #[serde(rename = "dual")]
    SetDualDevice { enabled: bool },

    #[serde(rename = "subscribe")]
    Subscribe,

    #[serde(rename = "unsubscribe")]
    Unsubscribe,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DeviceStatus {
    pub connected: bool,
    pub address: Option<String>,
    pub anc_mode: Option<u8>,
    pub anc_level: u8,
    pub anc_max: u8,
    pub eq_mode: Option<u8>,
    pub eq_name: Option<String>,
    pub adaptive_anc: Option<bool>,
    pub dual_device: Option<bool>,
    pub battery_left: Option<u8>,
    pub battery_right: Option<u8>,
    pub battery_case: Option<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ServiceResponse {
    #[serde(rename = "success")]
    Success {
        message: Option<String>,
        data: Option<serde_json::Value>,
    },

    #[serde(rename = "error")]
    Error { message: String },

    #[serde(rename = "status_update")]
    StatusUpdate { status: DeviceStatus },
}

pub struct SpacePodsService {
    buds: SpaceBuds,
    status: Arc<RwLock<DeviceStatus>>,
    status_tx: broadcast::Sender<DeviceStatus>,
    socket_path: PathBuf,
    running: Arc<Mutex<bool>>,
    subscribers: Arc<Mutex<Vec<tokio::sync::mpsc::UnboundedSender<DeviceStatus>>>>,
}

impl SpacePodsService {
    pub async fn new(socket_path: Option<PathBuf>) -> Result<Self> {
        let buds = SpaceBuds::new().await?;

        let (status_tx, _) = broadcast::channel(32);

        let status = Arc::new(RwLock::new(DeviceStatus {
            connected: false,
            address: None,
            anc_mode: None,
            anc_level: 0,
            anc_max: 0,
            eq_mode: None,
            eq_name: None,
            adaptive_anc: None,
            dual_device: None,
            battery_left: None,
            battery_right: None,
            battery_case: None,
        }));

        let socket_path = socket_path.unwrap_or_else(|| PathBuf::from(DEFAULT_SOCKET_PATH));

        Ok(Self {
            buds,
            status,
            status_tx,
            socket_path,
            running: Arc::new(Mutex::new(false)),
            subscribers: Arc::new(Mutex::new(Vec::new())),
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        *self.running.lock().await = true;

        let buds_clone = self.buds.clone();
        let status_clone = self.status.clone();
        let status_tx_clone = self.status_tx.clone();
        let running_clone = self.running.clone();
        let subscribers_clone = self.subscribers.clone();

        tokio::spawn(async move {
            Self::status_updater_loop(buds_clone, status_clone, status_tx_clone, running_clone, subscribers_clone).await;
        });

        #[cfg(unix)]
        {
            if self.socket_path.exists() {
                std::fs::remove_file(&self.socket_path).ok();
            }
        }

        let listener = UnixListener::bind(&self.socket_path)
            .map_err(|e| SpaceBudsError::Ipc(format!("Failed to bind socket: {}", e)))?;

        #[cfg(unix)]
        {
            use std::fs::Permissions;
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.socket_path, Permissions::from_mode(0o666)).ok();
        }

        println!("SpacePods service listening on {}", self.socket_path.display());

        while *self.running.lock().await {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let buds = self.buds.clone();
                    let status = self.status.clone();
                    let subscribers = self.subscribers.clone();

                    tokio::spawn(async move {
                        Self::handle_client(stream, buds, status, subscribers).await;
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

    async fn status_updater_loop(
        buds: SpaceBuds,
        status: Arc<RwLock<DeviceStatus>>,
        status_tx: broadcast::Sender<DeviceStatus>,
        running: Arc<Mutex<bool>>,
        subscribers: Arc<Mutex<Vec<tokio::sync::mpsc::UnboundedSender<DeviceStatus>>>>,
    ) {
        let mut reconnect_delay = time::Duration::from_secs(1);
        let mut battery_interval = time::interval(time::Duration::from_secs(60));

        Self::refresh_full_status(&buds, &status, &status_tx, &subscribers).await;

        while *running.lock().await {
            let connected = buds.is_connected().await;

            if connected {
                reconnect_delay = time::Duration::from_secs(1);

                tokio::select! {
                    _ = battery_interval.tick() => {
                        Self::refresh_battery_only(&buds, &status, &status_tx, &subscribers).await;
                    }
                    _ = tokio::signal::ctrl_c() => {
                        break;
                    }
                }
            } else {
                {
                    let mut status_lock = status.write().await;
                    status_lock.connected = false;
                }
                let current_status = status.read().await.clone();
                let _ = status_tx.send(current_status.clone());
                Self::broadcast_to_subscribers(&subscribers, current_status).await;

                let _ = buds.reconnect().await;
                time::sleep(reconnect_delay).await;
                reconnect_delay = (reconnect_delay * 2).min(time::Duration::from_secs(30));
            }
        }
    }

    async fn broadcast_to_subscribers(
        subscribers: &Arc<Mutex<Vec<tokio::sync::mpsc::UnboundedSender<DeviceStatus>>>>,
        status: DeviceStatus,
    ) {
        let mut subs = subscribers.lock().await;
        subs.retain(|sender| sender.send(status.clone()).is_ok());
    }

    async fn handle_client(
        mut stream: UnixStream,
        buds: SpaceBuds,
        status: Arc<RwLock<DeviceStatus>>,
        subscribers: Arc<Mutex<Vec<tokio::sync::mpsc::UnboundedSender<DeviceStatus>>>>,
    ) {
        let (reader, mut writer) = stream.split();
        let mut reader = BufReader::new(reader);
        let mut line = String::new();
        let (sub_tx, mut sub_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut subscribed = false;

        {
            let mut subs = subscribers.lock().await;
            subs.push(sub_tx);
        }

        loop {
            tokio::select! {
                read_result = reader.read_line(&mut line) => {
                    match read_result {
                        Ok(0) => break,
                        Ok(_) => {
                            let response = match serde_json::from_str::<ServiceCommand>(&line.trim()) {
                                Ok(cmd) => {
                                    if matches!(cmd, ServiceCommand::Subscribe) {
                                        subscribed = true;
                                        ServiceResponse::Success {
                                            message: Some("Subscribed to status updates".to_string()),
                                            data: None,
                                        }
                                    } else if matches!(cmd, ServiceCommand::Unsubscribe) {
                                        subscribed = false;
                                        ServiceResponse::Success {
                                            message: Some("Unsubscribed from status updates".to_string()),
                                            data: None,
                                        }
                                    } else {
                                        Self::execute_command(cmd, &buds, &status).await
                                    }
                                }
                                Err(e) => ServiceResponse::Error {
                                    message: format!("Invalid command: {}", e),
                                },
                            };

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

                status_update = sub_rx.recv() => {
                    if subscribed {
                        if let Some(new_status) = status_update {
                            let response = ServiceResponse::StatusUpdate { status: new_status };
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

        let mut subs = subscribers.lock().await;
        subs.retain(|sender| !sender.is_closed());
    }

    async fn refresh_full_status(
        buds: &SpaceBuds,
        status: &Arc<RwLock<DeviceStatus>>,
        status_tx: &broadcast::Sender<DeviceStatus>,
        subscribers: &Arc<Mutex<Vec<tokio::sync::mpsc::UnboundedSender<DeviceStatus>>>>,
    ) {
        if !buds.is_connected().await {
            return;
        }

        let anc_mode = buds.anc().get_mode().await.unwrap_or(None);
        let (level, max_level) = buds.anc().get_level().await.unwrap_or((0, 0));
        let eq_state = buds.eq().get_state().await.unwrap_or(None);
        let adaptive = buds.features().get_adaptive_anc().await.unwrap_or(None);
        let dual = buds.features().get_dual_device().await.unwrap_or(None);
        let (batt_left, batt_right, batt_case) = buds.with_connection(|conn| async move {
            conn.get_battery_level().await
        }).await.unwrap_or((None, None, None));

        let mut status_lock = status.write().await;
        status_lock.connected = true;
        status_lock.address = buds.address();
        status_lock.anc_mode = anc_mode;
        status_lock.anc_level = level;
        status_lock.anc_max = max_level;

        if let Some(eq) = eq_state {
            status_lock.eq_mode = Some(eq.mode);
            status_lock.eq_name = Some(eq.name);
        }

        status_lock.adaptive_anc = adaptive;
        status_lock.dual_device = dual;
        status_lock.battery_left = batt_left;
        status_lock.battery_right = batt_right;
        status_lock.battery_case = batt_case;

        let new_status = status_lock.clone();
        let _ = status_tx.send(new_status.clone());
        Self::broadcast_to_subscribers(subscribers, new_status).await;
    }

    async fn refresh_battery_only(
        buds: &SpaceBuds,
        status: &Arc<RwLock<DeviceStatus>>,
        status_tx: &broadcast::Sender<DeviceStatus>,
        subscribers: &Arc<Mutex<Vec<tokio::sync::mpsc::UnboundedSender<DeviceStatus>>>>,
    ) {
        if !buds.is_connected().await {
            return;
        }

        let (batt_left, batt_right, batt_case) = buds.with_connection(|conn| async move {
            conn.get_battery_level().await
        }).await.unwrap_or((None, None, None));

        let mut status_lock = status.write().await;
        status_lock.battery_left = batt_left;
        status_lock.battery_right = batt_right;
        status_lock.battery_case = batt_case;

        let new_status = status_lock.clone();
        let _ = status_tx.send(new_status.clone());
        Self::broadcast_to_subscribers(subscribers, new_status).await;
    }

    async fn execute_command(
        cmd: ServiceCommand,
        buds: &SpaceBuds,
        status: &Arc<RwLock<DeviceStatus>>,
    ) -> ServiceResponse {
        match cmd {
            ServiceCommand::Ping => {
                ServiceResponse::Success {
                    message: Some("pong".to_string()),
                    data: None,
                }
            }

            ServiceCommand::GetStatus => {
                let status = status.read().await.clone();
                let data = serde_json::to_value(status).unwrap_or(serde_json::Value::Null);
                ServiceResponse::Success {
                    message: None,
                    data: Some(data),
                }
            }

            ServiceCommand::SetAncMode { mode } => {
                let mode_val = match mode.as_str() {
                    "off" | "0" => MODE_OFF,
                    "on" | "1" | "anc" => MODE_ANC,
                    "transparency" | "2" => MODE_TRANSPARENCY,
                    _ => {
                        return ServiceResponse::Error {
                            message: format!("Invalid ANC mode: {}", mode),
                        };
                    }
                };

                match buds.anc().set_mode(mode_val).await {
                    Ok(_) => {
                        tokio::spawn({
                            let buds = buds.clone();
                            let status = status.clone();
                            async move {
                                time::sleep(time::Duration::from_millis(200)).await;
                                if let Ok(mode) = buds.anc().get_mode().await {
                                    let mut status = status.write().await;
                                    status.anc_mode = mode;
                                }
                            }
                        });

                        ServiceResponse::Success {
                            message: Some(format!("ANC mode set to {}", mode)),
                            data: None,
                        }
                    }
                    Err(e) => ServiceResponse::Error {
                        message: format!("Failed to set ANC mode: {}", e),
                    },
                }
            }

            ServiceCommand::SetLevel { level } => {
                match buds.anc().set_level(level).await {
                    Ok(true) => {
                        tokio::spawn({
                            let buds = buds.clone();
                            let status = status.clone();
                            async move {
                                time::sleep(time::Duration::from_millis(200)).await;
                                let (level, max) = buds.anc().get_level().await.unwrap_or((0, 0));
                                let mut status = status.write().await;
                                status.anc_level = level;
                                status.anc_max = max;
                            }
                        });

                        ServiceResponse::Success {
                            message: Some(format!("Level set to {}", level)),
                            data: None,
                        }
                    }
                    Ok(false) => ServiceResponse::Error {
                        message: "Cannot set level when ANC is off".to_string(),
                    },
                    Err(e) => ServiceResponse::Error {
                        message: format!("Failed to set level: {}", e),
                    },
                }
            }

            ServiceCommand::SetEqPreset { preset } => {
                match buds.eq().set_preset(preset).await {
                    Ok(_) => {
                        tokio::spawn({
                            let buds = buds.clone();
                            let status = status.clone();
                            async move {
                                time::sleep(time::Duration::from_millis(500)).await;
                                if let Some(eq) = buds.eq().get_state().await.unwrap_or(None) {
                                    let mut status_lock = status.write().await;
                                    status_lock.eq_mode = Some(eq.mode);
                                    status_lock.eq_name = Some(eq.name);
                                }
                            }
                        });

                        ServiceResponse::Success {
                            message: Some(format!("EQ preset set to {}", preset)),
                            data: None,
                        }
                    }
                    Err(e) => ServiceResponse::Error {
                        message: format!("Failed to set EQ preset: {}", e),
                    },
                }
            }

            ServiceCommand::SetAdaptiveAnc { enabled } => {
                match buds.features().set_adaptive_anc(enabled).await {
                    Ok(_) => {
                        tokio::spawn({
                            let buds = buds.clone();
                            let status = status.clone();
                            async move {
                                time::sleep(time::Duration::from_millis(200)).await;
                                if let Ok(adaptive) = buds.features().get_adaptive_anc().await {
                                    let mut status = status.write().await;
                                    status.adaptive_anc = adaptive;
                                }
                            }
                        });

                        ServiceResponse::Success {
                            message: Some(format!("Adaptive ANC {}", if enabled { "enabled" } else { "disabled" })),
                            data: None,
                        }
                    }
                    Err(e) => ServiceResponse::Error {
                        message: format!("Failed to set adaptive ANC: {}", e),
                    },
                }
            }

            ServiceCommand::SetDualDevice { enabled } => {
                match buds.features().set_dual_device(enabled).await {
                    Ok(_) => {
                        tokio::spawn({
                            let buds = buds.clone();
                            let status = status.clone();
                            async move {
                                time::sleep(time::Duration::from_millis(200)).await;
                                if let Ok(dual) = buds.features().get_dual_device().await {
                                    let mut status = status.write().await;
                                    status.dual_device = dual;
                                }
                            }
                        });

                        ServiceResponse::Success {
                            message: Some(format!("Dual device {}", if enabled { "enabled" } else { "disabled" })),
                            data: None,
                        }
                    }
                    Err(e) => ServiceResponse::Error {
                        message: format!("Failed to set dual device: {}", e),
                    },
                }
            }

            _ => ServiceResponse::Error {
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