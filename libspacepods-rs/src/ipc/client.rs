use crate::ipc::protocol::*;
use crate::{Error, Result};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::{broadcast, Mutex};

/// Client for communicating with the SpacePods daemon over Unix socket.
///
/// Provides typed methods for all service commands.
/// Supports subscribing to real-time status updates.
pub struct SpacePodsClient {
    reader: Arc<Mutex<BufReader<tokio::net::unix::OwnedReadHalf>>>,
    writer: Arc<Mutex<tokio::net::unix::OwnedWriteHalf>>,
    subscribed: bool,
    next_id: Arc<Mutex<u64>>,
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
            next_id: Arc::new(Mutex::new(1)),
        })
    }

    /// Send a command and wait for the response.
    async fn send_command_raw(&mut self, cmd: ServiceCommand) -> Result<IpcResult> {
        let mut next_id = self.next_id.lock().await;
        let id = *next_id;
        *next_id = id.wrapping_add(1);
        drop(next_id);

        let msg = IpcMessage::Request {
            id,
            command: cmd,
        };
        let cmd_json = serde_json::to_string(&msg).unwrap() + "\n";

        {
            let mut writer = self.writer.lock().await;
            writer.write_all(cmd_json.as_bytes()).await?;
            writer.flush().await?;
        }

        let mut line = String::new();
        let mut reader = self.reader.lock().await;
        match reader.read_line(&mut line).await {
            Ok(0) => Err(Error::Ipc("Service closed connection".to_string())),
            Ok(_) => {
                let response: IpcResult = serde_json::from_str(line.trim())?;
                match &response {
                    IpcResult::Error { message } => Err(Error::Ipc(message.clone())),
                    _ => Ok(response),
                }
            }
            Err(e) => Err(Error::Io(e)),
        }
    }

    // ── Public API ──

    pub async fn ping(&mut self) -> Result<bool> {
        match self.send_command_raw(ServiceCommand::Ping).await {
            Ok(IpcResult::Success { message, .. }) => Ok(message == Some("pong".to_string())),
            _ => Ok(false),
        }
    }

    pub async fn get_status(&mut self) -> Result<DeviceStatus> {
        match self.send_command_raw(ServiceCommand::GetStatus).await {
            Ok(IpcResult::Success { data, .. }) => {
                if let Some(data) = data {
                    Ok(serde_json::from_value(data)?)
                } else {
                    Err(Error::Ipc("No status data received".to_string()))
                }
            }
            Ok(IpcResult::StatusUpdate { status }) => Ok(status),
            Ok(_) => Err(Error::Ipc("Unexpected response".to_string())),
            Err(e) => Err(e),
        }
    }

    pub async fn scan(&mut self, timeout_secs: u64) -> Result<Vec<ScannedDevice>> {
        match self.send_command_raw(ServiceCommand::Scan { timeout_secs }).await {
            Ok(IpcResult::ScanResults { devices }) => Ok(devices),
            Ok(_) => Err(Error::Ipc("Unexpected response to scan".to_string())),
            Err(e) => Err(e),
        }
    }

    pub async fn connect_device(&mut self, address: String) -> Result<()> {
        self.send_command_raw(ServiceCommand::Connect { address }).await?;
        Ok(())
    }

    pub async fn set_anc_mode(&mut self, mode: &str) -> Result<()> {
        self.send_command_raw(ServiceCommand::SetAncMode { mode: mode.to_string() }).await?;
        Ok(())
    }

    pub async fn set_level(&mut self, level: u8) -> Result<()> {
        self.send_command_raw(ServiceCommand::SetLevel { level }).await?;
        Ok(())
    }

    pub async fn set_eq_preset(&mut self, preset: u8) -> Result<()> {
        self.send_command_raw(ServiceCommand::SetEqPreset { preset }).await?;
        Ok(())
    }

    pub async fn set_adaptive_anc(&mut self, enabled: bool) -> Result<()> {
        self.send_command_raw(ServiceCommand::SetAdaptiveAnc { enabled }).await?;
        Ok(())
    }

    pub async fn set_dual_device(&mut self, enabled: bool) -> Result<()> {
        self.send_command_raw(ServiceCommand::SetDualDevice { enabled }).await?;
        Ok(())
    }

    /// Subscribe to real-time status updates.
    /// Returns a broadcast receiver that yields `DeviceStatus` values.
    pub async fn subscribe(&mut self) -> Result<broadcast::Receiver<DeviceStatus>> {
        let (tx, rx) = broadcast::channel(32);

        self.send_command_raw(ServiceCommand::Subscribe).await?;
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
                        // Try to parse as a status update response
                        if let Ok(IpcResult::StatusUpdate { status }) =
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
            self.send_command_raw(ServiceCommand::Unsubscribe).await?;
            self.subscribed = false;
        }
        Ok(())
    }
}
