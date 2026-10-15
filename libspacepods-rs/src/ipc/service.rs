use crate::ipc::protocol::*;
use crate::protocol::{AncMode, ConnectionState,
};
use crate::{Error, ErrorCode, Result, SpaceBuds};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use btleplug::api::Peripheral;
use crate::ipc::frame::{self, FrameError};
use crate::ipc::paths;
use tokio::io::{AsyncWriteExt, BufReader};
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
    /// Set when the user takes manual control (Connect/Disconnect) so the
    /// background loop stops auto-connecting to a random device.
    manual: Arc<AtomicBool>,
    /// Set while a manual `Connect` is running so the auto-connect loop does not
    /// race it (BlueZ answers the loser with `In Progress`).
    connecting: Arc<AtomicBool>,
    /// True only once this process has bound the socket. `Drop` removes the socket
    /// file only in that case, so a refused second instance cannot delete the live one.
    owns_socket: bool,
}

impl SpacePodsService {
    pub async fn new(socket_path: Option<PathBuf>) -> Self {
        let buds = SpaceBuds::new_disconnected();
        let (status_tx, _) = broadcast::channel(32);
        let status = Arc::new(RwLock::new(DeviceStatus::default_disconnected()));
        let socket_path = socket_path.unwrap_or_else(paths::default_socket_path);
        Self {
            buds,
            status,
            status_tx,
            socket_path,
            running: Arc::new(Mutex::new(false)),
            manual: Arc::new(AtomicBool::new(false)),
            connecting: Arc::new(AtomicBool::new(false)),
            owns_socket: false,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        // Bind first. If another daemon already owns the socket we must fail before
        // starting any BLE work or status loop.
        let listener = self.bind_socket()?;
        *self.running.lock().await = true;

        // Spawn the background status updater
        let buds = self.buds.clone();
        let status = self.status.clone();
        let status_tx = self.status_tx.clone();
        let running = self.running.clone();
        let manual = self.manual.clone();
        let connecting = self.connecting.clone();

        tokio::spawn(async move {
            Self::status_updater_loop(buds, status, status_tx, running, manual, connecting).await;
        });

        // Session-bus interface. A missing session bus (for example when run as a
        // system service) is not fatal: the socket keeps working on its own.
        // `SPACEPODS_NO_DBUS` turns the bus interface off (used by tests that must not
        // claim the real session's name).
        // Battery is read in a supervised task that lives with the daemon.
        crate::battery::spawn_supervised(self.buds.clone(), self.status.clone(), self.status_tx.clone());

        let _dbus = if std::env::var_os("SPACEPODS_NO_DBUS").is_some() {
            None
        } else {
            match crate::ipc::dbus::serve(crate::ipc::dbus::DaemonState {
                buds: self.buds.clone(),
                status: self.status.clone(),
                status_tx: self.status_tx.clone(),
                manual: self.manual.clone(),
                connecting: self.connecting.clone(),
            })
            .await
            {
                Ok(conn) => {
                    crate::log::info("DBUS", &format!("serving {} on the session bus", crate::ipc::dbus::BUS_NAME));
                    Some(conn)
                }
                Err(e) => {
                    crate::log::warn("DBUS", &format!("session bus unavailable, socket only: {}", e));
                    None
                }
            }
        };

        println!("SpacePods service listening on {}", self.socket_path.display());
        crate::ipc::notify::ready();

        // Keep systemd's watchdog fed from the accept loop, so a wedged loop is
        // restarted instead of looking healthy.
        let watchdog = crate::ipc::notify::watchdog_interval();
        let mut ticker = time::interval(watchdog.map(|d| d / 2).unwrap_or(Duration::from_secs(3600)));

        // Accept loop
        while *self.running.lock().await {
            let accepted = if watchdog.is_some() {
                tokio::select! {
                    result = listener.accept() => Some(result),
                    _ = ticker.tick() => {
                        crate::ipc::notify::watchdog_ping();
                        None
                    }
                }
            } else {
                Some(listener.accept().await)
            };
            let Some(accepted) = accepted else { continue };
            match accepted {
                Ok((stream, _)) => {
                    if !Self::peer_is_allowed(&stream) {
                        continue;
                    }
                    let buds = self.buds.clone();
                    let status = self.status.clone();
                    let status_tx = self.status_tx.clone();
                    let manual = self.manual.clone();
                    let connecting = self.connecting.clone();

                    tokio::spawn(async move {
                        Self::handle_client(stream, buds, status, status_tx, manual, connecting).await;
                    });
                }
                Err(e) => {
                    crate::log::warn("IPC", &format!("accept failed: {}", e));
                }
            }
        }

        Ok(())
    }

    /// Prepare the socket directory, refuse to replace a live daemon, and bind.
    ///
    /// Only a stale socket owned by the current user is removed. A socket that
    /// answers a connect means another daemon is running.
    fn bind_socket(&mut self) -> Result<UnixListener> {
        let dir = self
            .socket_path
            .parent()
            .map(|p| p.to_path_buf())
            .ok_or_else(|| Error::UnsafeSocketDir {
                path: self.socket_path.clone(),
                reason: "socket path has no parent directory".into(),
            })?;
        paths::ensure_runtime_dir(&dir)?;

        // sockaddr_un holds at most 108 bytes including the terminating NUL.
        if self.socket_path.as_os_str().len() >= 108 {
            return Err(Error::UnsafeSocketDir {
                path: self.socket_path.clone(),
                reason: "socket path is longer than the 107-byte Unix socket limit".into(),
            });
        }

        if self.socket_path.exists() {
            match std::os::unix::net::UnixStream::connect(&self.socket_path) {
                Ok(_) => {
                    return Err(Error::AlreadyRunning {
                        path: self.socket_path.clone(),
                    })
                }
                Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                    return Err(Error::UnsafeSocketDir {
                        path: self.socket_path.clone(),
                        reason: "socket is in use by another user".into(),
                    })
                }
                Err(_) => {
                    // Nothing is listening: a stale socket. Remove it only if it is ours.
                    if !paths::is_owned_by_current_user(&self.socket_path)? {
                        return Err(Error::UnsafeSocketDir {
                            path: self.socket_path.clone(),
                            reason: "stale socket owned by another user".into(),
                        });
                    }
                    std::fs::remove_file(&self.socket_path)?;
                }
            }
        }

        let listener = UnixListener::bind(&self.socket_path)?;
        paths::secure_socket_file(&self.socket_path)?;
        self.owns_socket = true;
        Ok(listener)
    }

    /// Reject clients whose uid differs from the daemon's uid.
    ///
    /// The directory and socket modes already block other users; this is a second
    /// check in case the modes are changed by hand or by a packaging mistake.
    fn peer_is_allowed(stream: &UnixStream) -> bool {
        match stream.peer_cred() {
            Ok(cred) if cred.uid() == paths::current_uid() => true,
            Ok(cred) => {
                crate::log::warn(
                    "IPC",
                    &format!("rejected client with uid {} (daemon uid {})", cred.uid(), paths::current_uid()),
                );
                false
            }
            Err(e) => {
                crate::log::warn("IPC", &format!("rejected client: cannot read peer credentials: {}", e));
                false
            }
        }
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
        manual: Arc<AtomicBool>,
        connecting: Arc<AtomicBool>,
    ) {
        let mut reconnect_delay = Duration::from_secs(1);
        // Audio is connected once per control-link session, not on every pass.
        let mut audio_attempted = false;

        // Try to connect on startup after a short delay (let BlueZ settle)
        tokio::time::sleep(Duration::from_secs(2)).await;
        if !buds.is_connected().await && !manual.load(Ordering::Relaxed) && !crate::bluez::is_off().await {
            crate::log::info("IPC", "Not connected — attempting initial auto-connect");
            match buds.connect().await {
                Ok(_) => crate::log::info("IPC", "Initial auto-connect succeeded"),
                Err(e) => {
                    crate::log::warn("IPC", &format!("Initial auto-connect: {}", e));
                    stop_auto_connect_if_refused(&e, &manual);
                }
            }
        }

        // Initial refresh
        Self::refresh_full_status(&buds, &status, &status_tx).await;

        while *running.lock().await {
            let connected = buds.is_connected().await;

            if !connected {
                audio_attempted = false;
            } else if !audio_attempted {
                audio_attempted = true;
                // Bring up the audio link too, but only if it is not already up. This runs
                // in the daemon, so it keeps going when the UI is closed.
                if let Some(ble_address) = buds.address().await {
                    let status = status.clone();
                    let status_tx = status_tx.clone();
                    tokio::spawn(async move {
                        // Audio lives on the classic address, not the BLE one.
                        let up = match crate::bluez::audio_address_for(&ble_address).await {
                            Some(audio) => crate::bluez::ensure_audio(&audio).await,
                            None => {
                                crate::log::info("AUDIO", "no paired audio device matches this earbud");
                                false
                            }
                        };
                        let snapshot = {
                            let mut s = status.write().await;
                            s.connection.audio = up;
                            s.clone()
                        };
                        let _ = status_tx.send(snapshot);
                    });
                }
            }

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

                // Try to reconnect, unless the user manually disconnected or a
                // manual connect is in flight.
                // While Bluetooth is off there is nothing to reconnect to. Skip the attempt
                // (and its log noise) and check again on the next pass.
                if !manual.load(Ordering::Relaxed)
                    && !connecting.load(Ordering::Relaxed)
                    && !crate::bluez::is_off().await
                {
                    if let Err(e) = buds.reconnect().await {
                        stop_auto_connect_if_refused(&e, &manual);
                    }
                }
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
        if let Some(caps) = buds.manager.jieli_get_capabilities().await {
            let features = crate::device_profile::features_from_capabilities(&caps);
            crate::log::info(
                "IPC",
                &format!("device capabilities → {} features", features.len()),
            );
            status_lock.capabilities = Some(features.into_iter().collect());
        }
        match buds.features().get_key_settings().await {
            Ok(map) => { if !map.is_empty() { status_lock.key_settings = Some(map); } }
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
        manual: Arc<AtomicBool>,
        connecting: Arc<AtomicBool>,
    ) {
        let (reader, mut writer) = stream.split();
        let mut reader = BufReader::new(reader);
        let mut line: Vec<u8> = Vec::new();
        let mut subscribed = false;
        let mut status_rx = status_tx.subscribe();

        loop {
            tokio::select! {
                read_result = frame::read_frame(&mut reader, &mut line) => {
                    match read_result {
                        Ok(false) => break,
                        Err(FrameError::TooLarge) => {
                            let reply = IpcResult::error(
                                ErrorCode::PayloadTooLarge,
                                format!("payload too large (limit {} bytes)", frame::MAX_FRAME_BYTES),
                            );
                            if let Ok(json) = serde_json::to_string(&reply) {
                                let _ = writer.write_all(format!("{}\n", json).as_bytes()).await;
                            }
                            break;
                        }
                        Err(_) => break,
                        Ok(true) => {
                            let text = match frame::frame_as_str(&line) {
                                Ok(t) => t,
                                Err(_) => break,
                            };
                            // Parse incoming message
                            let response = match serde_json::from_str::<ServiceCommand>(text.trim()) {
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
                                        Self::execute_command(cmd, &buds, &status, &status_tx, &manual, &connecting).await
                                    }
                                }
                                Err(e) => IpcResult::error(ErrorCode::InvalidArgument, format!("Invalid command: {}", e)),
                            };

                            // Send response
                            let Ok(response_json) = serde_json::to_string(&response) else { break };
                            let response_json = response_json + "\n";
                            if writer.write_all(response_json.as_bytes()).await.is_err() {
                                break;
                            }
                            if writer.flush().await.is_err() {
                                break;
                            }
                        }
                    }
                }

                // Push status updates to subscribed clients
                status_update = status_rx.recv() => {
                    if subscribed {
                        if let Ok(new_status) = status_update {
                            let response = IpcResult::StatusUpdate { status: new_status };
                            let Ok(response_json) = serde_json::to_string(&response) else { break };
                            let response_json = response_json + "\n";
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

    pub(crate) async fn execute_command(
        cmd: ServiceCommand,
        buds: &SpaceBuds,
        status: &Arc<RwLock<DeviceStatus>>,
        status_tx: &broadcast::Sender<DeviceStatus>,
        manual: &Arc<AtomicBool>,
        connecting: &Arc<AtomicBool>,
    ) -> IpcResult {
        match cmd {
            ServiceCommand::Ping => IpcResult::Success {
                message: Some("pong".to_string()),
                // Lets clients show the Bluetooth state without a separate request.
                data: Some(serde_json::json!({
                    "bluetooth_powered": crate::bluez::adapter_powered().await,
                })),
            },

            ServiceCommand::GetStatus => {
                // Refresh before answering so query-based state (e.g. the
                // JieLi feature toggles) is current.
                Self::refresh_full_status(buds, status, status_tx).await;
                let status = status.read().await.clone();
                let data = serde_json::to_value(status).unwrap_or(serde_json::Value::Null);
                IpcResult::Success {
                    message: None,
                    data: Some(data),
                }
            }

            ServiceCommand::Scan { timeout_secs } => {
                if crate::bluez::is_off().await {
                    return IpcResult::failed("Scan failed", &Error::BluetoothOff);
                }
                match crate::beacon::scanner::scan_for_devices(Duration::from_secs(timeout_secs)).await {
                    Ok(discovered) => {
                        let devices: Vec<ScannedDevice> = discovered
                            .into_iter()
                            .map(|d| {
                                let product_name = d
                                    .beacon
                                    .as_ref()
                                    .map(|b| b.product_name().to_string());
                                let name = d
                                    .local_name
                                    .clone()
                                    .or_else(|| product_name.clone())
                                    .unwrap_or_else(|| "Oraimo device".to_string());
                                let address = d
                                    .ble_address
                                    .clone()
                                    .unwrap_or_else(|| "unknown".to_string());
                                ScannedDevice {
                                    name,
                                    address,
                                    product_name,
                                    product_id: d.beacon.as_ref().map(|b| b.product_id),
                                    real_mac: d.beacon.as_ref().and_then(|b| b.bt_address_string()),
                                    beacon_version: d.beacon.as_ref().map(|b| b.version),
                                    battery_left: d.beacon.as_ref().and_then(|b| b.left_battery),
                                    battery_right: d.beacon.as_ref().and_then(|b| b.right_battery),
                                    battery_case: d.beacon.as_ref().and_then(|b| b.case_battery),
                                    already_connected: d
                                        .beacon
                                        .as_ref()
                                        .map(|b| b.connected)
                                        .unwrap_or(false),
                                    rssi: d.rssi,
                                }
                            })
                            .collect();
                        IpcResult::ScanResults { devices }
                    }
                    Err(e) => IpcResult::failed("Scan failed", &e),
                }
            }


            ServiceCommand::Connect { address } => {
                // The user is taking control; stop the auto-connect loop.
                manual.store(false, Ordering::Relaxed);
                if crate::bluez::is_off().await {
                    return IpcResult::failed("Connection failed", &Error::BluetoothOff);
                }
                // Allow switching devices: only short-circuit when we are
                // already connected to the requested address.
                if buds.is_connected().await && buds.address().await.as_deref() == Some(address.as_str()) {
                    return IpcResult::Success {
                        message: Some("Already connected".to_string()),
                        data: None,
                    };
                }
                if buds.is_connected().await {
                    crate::log::warn("IPC", &format!("switching device: disconnecting to connect {}", address));
                    let _ = buds.disconnect().await;
                    time::sleep(Duration::from_millis(400)).await;
                }
                // Block the background auto-connect loop while we drive the
                // link ourselves; otherwise BlueZ rejects the loser with
                // `In Progress`.
                connecting.store(true, Ordering::Relaxed);
                let result = buds.connect_to(&address).await;
                connecting.store(false, Ordering::Relaxed);
                match result {
                    Ok(_) => {
                        // Reset cached device info so the new device is
                        // re-detected (product id, features, battery).
                        {
                            let mut s = status.write().await;
                            s.product_id = None;
                            s.key_settings = None;
                        }
                        Self::refresh_full_status(buds, status, status_tx).await;
                        let pid = status.read().await.product_id;
                        crate::log::warn("IPC", &format!("Connected to {} (pid {:?})", address, pid));
                        IpcResult::Success {
                            message: Some("Connected".to_string()),
                            data: None,
                        }
                    }
                    Err(e) => IpcResult::failed("Connection failed", &e),
                }
            }

            ServiceCommand::SetAncMode { mode } => {
                let mode_val = match mode.as_str() {
                    "off" | "0" => AncMode::Off,
                    "on" | "1" | "anc" => AncMode::Active,
                    "transparency" | "2" => AncMode::Transparency,
                    _ => {
                        return IpcResult::error(
                            ErrorCode::InvalidArgument,
                            format!("Invalid ANC mode: {}", mode),
                        );
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
                    Err(e) => IpcResult::failed("Failed to set ANC mode", &e),
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
                    Ok(false) => IpcResult::error(
                        ErrorCode::InvalidState,
                        "Cannot set level when ANC is off",
                    ),
                    Err(e) => IpcResult::failed("Failed to set level", &e),
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
                    Err(e) => IpcResult::failed("Failed to set EQ preset", &e),
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
                    Err(e) => IpcResult::failed("Failed to set adaptive ANC", &e),
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
                    Err(e) => IpcResult::failed("Failed to set dual device", &e),
                }
            }

            // ── Disconnect ──
            ServiceCommand::Disconnect => {
                // Manual disconnect: suppress the auto-reconnect loop.
                manual.store(true, Ordering::Relaxed);
                // Remember the address first: the BLE layer forgets it on disconnect.
                let address = buds.address().await;
                let ble = buds.disconnect().await;
                // Force: also drop the audio link, which the BLE layer does not own. This
                // runs even if the BLE disconnect failed.
                let bluez = match &address {
                    Some(addr) => crate::bluez::disconnect_device(addr).await,
                    None => Ok(()),
                };
                let mut s = status.write().await;
                *s = DeviceStatus::default_disconnected();
                match (ble, bluez) {
                    (Ok(_), Ok(_)) => IpcResult::Success {
                        message: Some("Disconnected".to_string()),
                        data: None,
                    },
                    (Err(e), _) => IpcResult::failed("Disconnect failed", &e),
                    (Ok(_), Err(e)) => IpcResult::error(
                        crate::ErrorCode::BluetoothUnavailable,
                        format!("Disconnect failed: {}", e),
                    ),
                }
            }

            // ── Custom raw BLE command ──
            ServiceCommand::Custom { command_id, payload } => {
                match buds.send_raw(command_id, payload).await {
                    Ok(_) => IpcResult::Success {
                        message: Some("Custom command sent".to_string()),
                        data: None,
                    },
                    Err(e) => IpcResult::failed("Custom command failed", &e),
                }
            }

            // ── JieLi RCSP custom command ──
            ServiceCommand::JieliCustom { payload } => {
                match buds.jieli_send_custom(payload).await {
                    Ok(Some(resp)) => IpcResult::Success {
                        message: Some("JieLi custom command sent".to_string()),
                        data: Some(
                            serde_json::to_value(resp.param)
                                .unwrap_or(serde_json::Value::Null),
                        ),
                    },
                    Ok(None) => IpcResult::Success {
                        message: Some("JieLi custom command sent (no response)".to_string()),
                        data: None,
                    },
                    Err(e) => IpcResult::failed("JieLi custom command failed", &e),
                }
            }

            // ── Chip-agnostic feature toggles ──
            ServiceCommand::SetFeature { feature, enabled } => {
                let _ = buds.manager.ensure_connected().await;
                if !buds.is_connected().await {
                    let e = if crate::bluez::is_off().await { Error::BluetoothOff } else { Error::NotConnected };
                    return IpcResult::failed(&format!("Couldn't change {}", feature), &e);
                }
                if buds.is_jieli().await {
                    match crate::protocol::OraFeature::from_name(&feature) {
                        Some(f) => match buds.manager.jieli_set_feature(f, enabled).await {
                            Ok(_) => IpcResult::Success {
                                message: Some(format!("{} set to {}", feature, enabled)),
                                data: None,
                            },
                            Err(e) => IpcResult::failed(&format!("Failed to set {}", feature), &e),
                        },
                        None => IpcResult::error(ErrorCode::UnsupportedFeature, format!("Unknown feature: {}", feature)),
                    }
                } else {
                    match crate::commands::features::bluetrum_feature_command(&feature, enabled) {
                        Some((cmd_id, payload)) => match buds.send_raw(cmd_id, payload).await {
                            Ok(_) => IpcResult::Success {
                                message: Some(format!("{} set to {}", feature, enabled)),
                                data: None,
                            },
                            Err(e) => IpcResult::failed(&format!("Failed to set {}", feature), &e),
                        },
                        None => IpcResult::error(ErrorCode::UnsupportedFeature, format!("Unknown feature: {}", feature)),
                    }
                }
            }

            ServiceCommand::GetFeature { feature } => {
                let _ = buds.manager.ensure_connected().await;
                if !buds.is_connected().await {
                    let e = if crate::bluez::is_off().await { Error::BluetoothOff } else { Error::NotConnected };
                    return IpcResult::failed(&format!("Couldn't read {}", feature), &e);
                }
                if buds.is_jieli().await {
                    if let Some(f) = crate::protocol::OraFeature::from_name(&feature) {
                        match buds.manager.jieli_get_feature(f).await {
                            Ok(v) => {
                                return IpcResult::Success {
                                    message: None,
                                    data: Some(
                                        serde_json::to_value(v)
                                            .unwrap_or(serde_json::Value::Null),
                                    ),
                                };
                            }
                            Err(e) => {
                                return IpcResult::failed(&format!("Failed to get {}", feature), &e);
                            }
                        }
                    }
                } else {
                    let value = match feature.as_str() {
                        "adaptive_anc" => buds.features().get_adaptive_anc().await.ok(),
                        "dual_device" => buds.features().get_dual_device().await.ok(),
                        "area_tap" => buds.features().get_area_tap().await.ok(),
                        "space_audio" | "3d_audio" => buds.features().get_space_audio().await.ok(),
                        _ => None,
                    };
                    if let Some(v) = value {
                        return IpcResult::Success {
                            message: None,
                            data: Some(serde_json::json!(v)),
                        };
                    }
                }
                IpcResult::error(ErrorCode::UnsupportedFeature, format!("Unknown feature: {}", feature))
            }

            // ── Rename ──
            ServiceCommand::Rename { name } => {
                let _ = buds.manager.ensure_connected().await;
                let result = if buds.is_jieli().await {
                    buds.manager.jieli_rename(&name).await
                } else {
                    buds.send_raw(0x2D, name.bytes().collect()).await
                };
                match result {
                    Ok(_) => IpcResult::Success {
                        message: Some("Renamed".to_string()),
                        data: None,
                    },
                    Err(e) => IpcResult::failed("Rename failed", &e),
                }
            }

            // ── Tone / voice-prompt volume ──
            ServiceCommand::SetToneVolume { level } => {
                let _ = buds.manager.ensure_connected().await;
                let result = if buds.is_jieli().await {
                    buds.jieli_send_custom(vec![crate::protocol::jieli::ora::TONE_VOLUME, level])
                        .await
                        .map(|_| ())
                } else {
                    buds.send_raw(0x46, vec![level]).await
                };
                match result {
                    Ok(_) => IpcResult::Success {
                        message: Some(format!("Tone volume set to {}", level)),
                        data: None,
                    },
                    Err(e) => IpcResult::failed("Failed to set tone volume", &e),
                }
            }

            // ── Gesture key remap ──
            ServiceCommand::SetKeySetting { key_type, function } => {
                let _ = buds.manager.ensure_connected().await;
                let result = if buds.is_jieli().await {
                    buds.manager
                        .jieli_set_adv_setting(
                            crate::protocol::jieli::ADV_TYPE_KEY_SETTINGS,
                            &[key_type, 0x01, function],
                        )
                        .await
                } else {
                    buds.send_raw(0x22, vec![key_type, 0x01, function]).await
                };
                match result {
                    Ok(_) => IpcResult::Success {
                        message: Some("Gesture updated".to_string()),
                        data: None,
                    },
                    Err(e) => IpcResult::failed("Failed to update gesture", &e),
                }
            }

            _ => IpcResult::error(ErrorCode::Internal, "Command not handled"),
        }
    }
}

impl Drop for SpacePodsService {
    fn drop(&mut self) {
        if self.owns_socket {
            let _ = std::fs::remove_file(&self.socket_path);
        }
    }
}

/// A device that cannot be identified is not retried in the background. Each retry
/// opens and closes a link to the earbuds, which is what makes them drop and
/// reconnect. The person must press Connect to try again; a manual connect clears
/// the flag (see `ServiceCommand::Connect`).
fn stop_auto_connect_if_refused(err: &Error, manual: &AtomicBool) {
    if err.code() == crate::ErrorCode::UnsupportedFeature {
        crate::log::warn(
            "IPC",
            "device not supported; automatic connection stopped. Press Connect to try again.",
        );
        manual.store(true, Ordering::Relaxed);
    }
}
