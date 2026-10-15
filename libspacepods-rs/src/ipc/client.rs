use crate::ipc::dbus;
use crate::ipc::frame::{self, FrameError};
use crate::ipc::paths;
use crate::ipc::protocol::*;
use crate::{Error, Result};
use futures::StreamExt;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncWriteExt, BufReader};
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::UnixStream;
use tokio::sync::{broadcast, Mutex, OnceCell};

/// One process-wide session-bus connection. Creating a bus connection per call is
/// expensive, so every client shares this one. Failures are not cached.
static SESSION_BUS: OnceCell<zbus::Connection> = OnceCell::const_new();

async fn session_bus() -> Result<zbus::Connection> {
    SESSION_BUS
        .get_or_try_init(|| async { zbus::Connection::session().await })
        .await
        .cloned()
        .map_err(dbus_error)
}

fn dbus_error(e: zbus::Error) -> Error {
    Error::Dbus(e.to_string())
}

/// How requests reach the daemon.
enum Transport {
    /// Unix socket with newline-delimited JSON (used for the CLI, and as a fallback).
    Socket {
        reader: Arc<Mutex<BufReader<OwnedReadHalf>>>,
        writer: Arc<Mutex<OwnedWriteHalf>>,
    },
    /// Session-bus interface `com.github.imnotndesh.SpacePods.Daemon1`.
    Dbus { proxy: zbus::Proxy<'static> },
}

/// Client for the SpacePods daemon.
///
/// [`SpacePodsClient::connect`] prefers the session-bus interface and falls back to the
/// Unix socket when the daemon is not on the bus (for example when run without a
/// session bus). Both transports speak the same command schema, so every typed method
/// behaves the same either way. Supports subscribing to real-time status updates.
pub struct SpacePodsClient {
    transport: Transport,
    subscribed: bool,
    next_id: Arc<Mutex<u64>>,
}

impl SpacePodsClient {
    /// Connect to the daemon.
    ///
    /// With no explicit socket path, the D-Bus interface is tried first. An explicit
    /// path always uses the socket.
    pub async fn connect(socket_path: Option<PathBuf>) -> Result<Self> {
        if socket_path.is_none() {
            if let Ok(client) = Self::connect_dbus().await {
                return Ok(client);
            }
        }
        Self::connect_socket(socket_path).await
    }

    /// Connect over the session bus only. Fails if the daemon does not own the name.
    pub async fn connect_dbus() -> Result<Self> {
        let conn = session_bus().await?;
        let proxy = zbus::Proxy::new(&conn, dbus::BUS_NAME, dbus::OBJECT_PATH, dbus::BUS_NAME)
            .await
            .map_err(dbus_error)?;
        let mut client = Self {
            transport: Transport::Dbus { proxy },
            subscribed: false,
            next_id: Arc::new(Mutex::new(1)),
        };
        // Reaching the name is not enough: check that the daemon answers.
        client.send_command_raw(ServiceCommand::Ping).await?;
        Ok(client)
    }

    /// Connect over the Unix socket.
    pub async fn connect_socket(socket_path: Option<PathBuf>) -> Result<Self> {
        let path = socket_path.unwrap_or_else(paths::default_socket_path);
        let stream = UnixStream::connect(path).await?;
        let (reader, writer) = stream.into_split();

        Ok(Self {
            transport: Transport::Socket {
                reader: Arc::new(Mutex::new(BufReader::new(reader))),
                writer: Arc::new(Mutex::new(writer)),
            },
            subscribed: false,
            next_id: Arc::new(Mutex::new(1)),
        })
    }

    /// Which transport this client is using. Useful for diagnostics.
    pub fn transport_name(&self) -> &'static str {
        match self.transport {
            Transport::Socket { .. } => "socket",
            Transport::Dbus { .. } => "dbus",
        }
    }

    /// Send a command and wait for the response.
    pub async fn send_command_raw(&mut self, cmd: ServiceCommand) -> Result<IpcResult> {
        let result = match &self.transport {
            Transport::Socket { reader, writer } => {
                socket_request(reader, writer, &self.next_id, cmd).await?
            }
            Transport::Dbus { proxy } => dbus_request(proxy, cmd).await?,
        };
        match &result {
            IpcResult::Error { code, message, .. } => Err(Error::Service {
                code: *code,
                message: message.clone(),
            }),
            _ => Ok(result),
        }
    }

    // ── Public API ──

    /// Whether the Bluetooth adapter is on, as the daemon sees it. `None` if unknown.
    pub async fn bluetooth_powered(&mut self) -> Result<Option<bool>> {
        match self.send_command_raw(ServiceCommand::Ping).await? {
            IpcResult::Success { data: Some(data), .. } => {
                Ok(data.get("bluetooth_powered").and_then(|v| v.as_bool()))
            }
            _ => Ok(None),
        }
    }

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

    pub async fn disconnect_device(&mut self) -> Result<()> {
        self.send_command_raw(ServiceCommand::Disconnect).await?;
        Ok(())
    }

    /// Send a raw Oraimo/JieLi RCSP custom command.
    /// Returns the response parameter bytes, if the device replied with any.
    pub async fn jieli_custom(&mut self, payload: Vec<u8>) -> Result<Option<Vec<u8>>> {
        match self
            .send_command_raw(ServiceCommand::JieliCustom { payload })
            .await?
        {
            IpcResult::Success { data, .. } => Ok(data.and_then(|v| serde_json::from_value(v).ok())),
            _ => Ok(None),
        }
    }

    /// Set a boolean feature by chip-agnostic name (e.g. `"chat_mode"`).
    pub async fn set_feature(&mut self, feature: &str, enabled: bool) -> Result<()> {
        self.send_command_raw(ServiceCommand::SetFeature {
            feature: feature.to_string(),
            enabled,
        })
        .await?;
        Ok(())
    }

    /// Query a boolean feature by chip-agnostic name.
    pub async fn get_feature(&mut self, feature: &str) -> Result<Option<bool>> {
        match self
            .send_command_raw(ServiceCommand::GetFeature {
                feature: feature.to_string(),
            })
            .await?
        {
            IpcResult::Success { data, .. } => Ok(data.and_then(|v| serde_json::from_value(v).ok())),
            _ => Ok(None),
        }
    }

    /// Rename the connected device (chip-agnostic).
    pub async fn rename(&mut self, name: &str) -> Result<()> {
        self.send_command_raw(ServiceCommand::Rename {
            name: name.to_string(),
        })
        .await?;
        Ok(())
    }

    /// Set the tone / voice-prompt volume (0-10, chip-agnostic).
    pub async fn set_tone_volume(&mut self, level: u8) -> Result<()> {
        self.send_command_raw(ServiceCommand::SetToneVolume { level }).await?;
        Ok(())
    }

    /// Remap a gesture key to a function (chip-agnostic).
    pub async fn set_key_setting(&mut self, key_type: u8, function: u8) -> Result<()> {
        self.send_command_raw(ServiceCommand::SetKeySetting { key_type, function })
            .await?;
        Ok(())
    }

    /// Subscribe to real-time status updates.
    /// Returns a broadcast receiver that yields `DeviceStatus` values.
    ///
    /// Over D-Bus the daemon pushes `StatusChanged` signals with no request needed.
    /// Over the socket the client sends `subscribe` and reads pushed frames.
    pub async fn subscribe(&mut self) -> Result<broadcast::Receiver<DeviceStatus>> {
        let (tx, rx) = broadcast::channel(32);

        let socket_reader = match &self.transport {
            Transport::Dbus { proxy } => {
                let mut signals = proxy.receive_signal("StatusChanged").await.map_err(dbus_error)?;
                tokio::spawn(async move {
                    while let Some(msg) = signals.next().await {
                        let Ok(json) = msg.body().deserialize::<String>() else { continue };
                        let Ok(status) = serde_json::from_str::<DeviceStatus>(&json) else { continue };
                        if tx.send(status).is_err() {
                            break;
                        }
                    }
                });
                return Ok(rx);
            }
            Transport::Socket { reader, .. } => Arc::clone(reader),
        };

        self.send_command_raw(ServiceCommand::Subscribe).await?;
        self.subscribed = true;

        tokio::spawn(async move {
            let mut reader = socket_reader.lock().await;
            let mut line: Vec<u8> = Vec::new();
            loop {
                match frame::read_frame(&mut *reader, &mut line).await {
                    Ok(true) => {
                        let Ok(text) = frame::frame_as_str(&line) else { break };
                        // Try to parse as a status update response
                        if let Ok(IpcResult::StatusUpdate { status }) =
                            serde_json::from_str(text.trim())
                        {
                            if tx.send(status).is_err() {
                                break;
                            }
                        }
                    }
                    // Clean EOF, oversized frame, or I/O error: the stream is done.
                    Ok(false) | Err(_) => break,
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

/// One request/response over the socket. Frames are matched by `id` in the request,
/// but this client holds the reader lock for the whole exchange, so replies are in order.
async fn socket_request(
    reader: &Arc<Mutex<BufReader<OwnedReadHalf>>>,
    writer: &Arc<Mutex<OwnedWriteHalf>>,
    next_id: &Arc<Mutex<u64>>,
    cmd: ServiceCommand,
) -> Result<IpcResult> {
    let mut next = next_id.lock().await;
    let id = *next;
    *next = id.wrapping_add(1);
    drop(next);

    let msg = IpcMessage::Request { id, command: cmd };
    let cmd_json = serde_json::to_string(&msg)? + "\n";

    {
        let mut writer = writer.lock().await;
        writer.write_all(cmd_json.as_bytes()).await?;
        writer.flush().await?;
    }

    let mut line: Vec<u8> = Vec::new();
    let mut reader = reader.lock().await;
    match frame::read_frame(&mut *reader, &mut line).await {
        Ok(false) => Err(Error::Ipc("Service closed connection".to_string())),
        Ok(true) => {
            let text = frame::frame_as_str(&line).map_err(Error::Io)?;
            Ok(serde_json::from_str(text.trim())?)
        }
        Err(FrameError::TooLarge) => Err(Error::PayloadTooLarge {
            limit: frame::MAX_FRAME_BYTES,
        }),
        Err(FrameError::UnexpectedEof) => Err(Error::Ipc("Service closed connection mid-reply".to_string())),
        Err(FrameError::Io(e)) => Err(Error::Io(e)),
    }
}

/// One request/response over the session bus: the command goes in as JSON and the
/// reply comes back as JSON, the same schema the socket uses.
async fn dbus_request(proxy: &zbus::Proxy<'static>, cmd: ServiceCommand) -> Result<IpcResult> {
    let json = serde_json::to_string(&cmd)?;
    let reply = proxy
        .call_method("Request", &(json,))
        .await
        .map_err(dbus_error)?;
    let text: String = reply.body().deserialize().map_err(dbus_error)?;
    Ok(serde_json::from_str(&text)?)
}
