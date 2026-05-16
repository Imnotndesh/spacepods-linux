use crate::{SpaceBudsError, Result};
use crate::service::{ServiceCommand, ServiceResponse, DeviceStatus, DEFAULT_SOCKET_PATH};
use serde_json;
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::broadcast;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct SpacePodsClient {
    reader: Arc<Mutex<BufReader<tokio::net::unix::OwnedReadHalf>>>,
    writer: Arc<Mutex<tokio::net::unix::OwnedWriteHalf>>,
    subscribed: bool,
}

impl SpacePodsClient {
    pub async fn connect(socket_path: Option<PathBuf>) -> Result<Self> {
        let path = socket_path.unwrap_or_else(|| PathBuf::from(DEFAULT_SOCKET_PATH));
        let stream = UnixStream::connect(path).await?;
        let (reader, writer) = stream.into_split();

        Ok(Self {
            reader: Arc::new(Mutex::new(BufReader::new(reader))),
            writer: Arc::new(Mutex::new(writer)),
            subscribed: false,
        })
    }
    pub async fn scan(&mut self, timeout_secs: u64) -> Result<Vec<(String, String)>> {
        match self.send_command(ServiceCommand::Scan { timeout_secs }).await? {
            ServiceResponse::ScanResults { devices } => {
                Ok(devices.into_iter().map(|d| (d.name, d.address)).collect())
            }
            _ => Err(SpaceBudsError::Ipc("Unexpected response to scan".to_string())),
        }
    }

    pub async fn connect_device(&mut self, address: String) -> Result<()> {
        self.send_command(ServiceCommand::Connect { address }).await?;
        Ok(())
    }
    pub async fn send_command(&mut self, cmd: ServiceCommand) -> Result<ServiceResponse> {
        let cmd_json = serde_json::to_string(&cmd).unwrap() + "\n";

        {
            let mut writer = self.writer.lock().await;
            writer.write_all(cmd_json.as_bytes()).await?;
            writer.flush().await?;
        }

        let mut line = String::new();
        let mut reader = self.reader.lock().await;
        match reader.read_line(&mut line).await {
            Ok(0) => Err(SpaceBudsError::Ipc("Service closed connection".to_string())),
            Ok(_) => {
                let response: ServiceResponse = serde_json::from_str(line.trim())?;
                match &response {
                    ServiceResponse::Error { message } => Err(SpaceBudsError::Ipc(message.clone())),
                    _ => Ok(response),
                }
            }
            Err(e) => Err(SpaceBudsError::Io(e)),
        }
    }

    pub async fn ping(&mut self) -> Result<bool> {
        match self.send_command(ServiceCommand::Ping).await {
            Ok(ServiceResponse::Success { message, .. }) => Ok(message == Some("pong".to_string())),
            _ => Ok(false),
        }
    }

    pub async fn get_battery(&mut self) -> Result<(Option<u8>, Option<u8>, Option<u8>)> {
        match self.send_command(ServiceCommand::GetBattery).await? {
            ServiceResponse::Success { data: Some(data), .. } => {
                let left  = data["battery_left"].as_u64().map(|v| v as u8);
                let right = data["battery_right"].as_u64().map(|v| v as u8);
                let case_ = data["battery_case"].as_u64().map(|v| v as u8);
                Ok((left, right, case_))
            }
            _ => Ok((None, None, None)),
        }
    }

    pub async fn get_status(&mut self) -> Result<DeviceStatus> {
        match self.send_command(ServiceCommand::GetStatus).await {
            Ok(ServiceResponse::Success { data, .. }) => {
                if let Some(data) = data {
                    Ok(serde_json::from_value(data)?)
                } else {
                    Err(SpaceBudsError::Ipc("No status data received".to_string()))
                }
            }
            Ok(ServiceResponse::StatusUpdate { status }) => Ok(status),
            Ok(_) => Err(SpaceBudsError::Ipc("Unexpected response".to_string())),
            Err(e) => Err(e),
        }
    }

    pub async fn set_anc_mode(&mut self, mode: &str) -> Result<()> {
        self.send_command(ServiceCommand::SetAncMode { mode: mode.to_string() }).await?;
        Ok(())
    }

    pub async fn set_level(&mut self, level: u8) -> Result<()> {
        self.send_command(ServiceCommand::SetLevel { level }).await?;
        Ok(())
    }

    pub async fn set_eq_preset(&mut self, preset: u8) -> Result<()> {
        self.send_command(ServiceCommand::SetEqPreset { preset }).await?;
        Ok(())
    }

    pub async fn set_adaptive_anc(&mut self, enabled: bool) -> Result<()> {
        self.send_command(ServiceCommand::SetAdaptiveAnc { enabled }).await?;
        Ok(())
    }

    pub async fn set_dual_device(&mut self, enabled: bool) -> Result<()> {
        self.send_command(ServiceCommand::SetDualDevice { enabled }).await?;
        Ok(())
    }

    pub async fn subscribe(&mut self) -> Result<broadcast::Receiver<DeviceStatus>> {
        let (tx, rx) = broadcast::channel(32);

        self.send_command(ServiceCommand::Subscribe).await?;
        self.subscribed = true;

        let reader = Arc::clone(&self.reader);
        let tx_clone = tx.clone();

        tokio::spawn(async move {
            let mut reader = reader.lock().await;
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => break,
                    Ok(_) => {
                        if let Ok(ServiceResponse::StatusUpdate { status }) =
                            serde_json::from_str(line.trim())
                        {
                            if tx_clone.send(status).is_err() {
                                break;
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        Ok(rx)
    }

    pub async fn unsubscribe(&mut self) -> Result<()> {
        if self.subscribed {
            self.send_command(ServiceCommand::Unsubscribe).await?;
            self.subscribed = false;
        }
        Ok(())
    }
}