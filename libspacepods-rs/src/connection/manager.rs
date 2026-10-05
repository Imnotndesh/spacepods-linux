use crate::commands::BleCommand;
use crate::connection::ble::BleConnection;
use crate::connection::jieli::JieliConnection;
use crate::connection::scanner::DeviceScanner;
use crate::protocol::{ConnectionState, Packet, TlvParser, CMD_HANDSHAKE,
    ID_ANC_MODE, ID_ANC_GAIN, ID_ANC_MAX, ID_TRANS_GAIN, ID_TRANS_MAX,
    ID_EQ_SETTING, ID_ENV_ADAPTIVE, ID_DUAL_DEVICE, ID_KEY_SETTINGS,
    ID_WORK_MODE, ID_IN_EAR_STATUS, ID_AUTO_ANSWER,
    ID_TONE_VOLUME, ID_HEARING_CARE, ID_VOICE_PROMPT,
};
use crate::{Error, Result};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, Mutex, RwLock};

#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    pub scan_timeout: Duration,
    pub max_retries: usize,
    pub reconnect_delay: Duration,
}

impl Default for ConnectionConfig {
    fn default() -> Self {
        Self {
            scan_timeout: Duration::from_secs(10),
            max_retries: 3,
            reconnect_delay: Duration::from_secs(1),
        }
    }
}

struct Inner {
    state: ConnectionState,
    connection: Option<ChipConnection>,
}

/// A connected device speaking one of the supported chip protocols.
///
/// This keeps the two wire protocols side by side so a JieLi device (RCSP) and
/// a Bluetrum device (`ff17`/`ff18`) can share the same connection lifecycle.
pub enum ChipConnection {
    /// Bluetrum protocol (most SpaceBuds).
    Bluetrum(BleConnection),
    /// JieLi RCSP protocol (e.g. BoomPop Pro).
    Jieli(JieliConnection),
}

impl ChipConnection {
    pub fn is_jieli(&self) -> bool {
        matches!(self, ChipConnection::Jieli(_))
    }

    fn as_bluetrum(&self) -> Option<&BleConnection> {
        match self {
            ChipConnection::Bluetrum(c) => Some(c),
            ChipConnection::Jieli(_) => None,
        }
    }

    async fn is_connected(&self) -> bool {
        match self {
            ChipConnection::Bluetrum(c) => c.is_connected().await,
            ChipConnection::Jieli(c) => c.is_connected().await,
        }
    }

    async fn disconnect(&self) -> Result<()> {
        match self {
            ChipConnection::Bluetrum(c) => c.disconnect().await,
            ChipConnection::Jieli(c) => c.disconnect().await,
        }
    }

    async fn reconnect_with_rediscovery(&self) -> Result<()> {
        match self {
            ChipConnection::Bluetrum(c) => c.reconnect_with_rediscovery().await,
            ChipConnection::Jieli(c) => c.reconnect_with_rediscovery().await,
        }
    }

    fn address(&self) -> String {
        match self {
            ChipConnection::Bluetrum(c) => c.address(),
            ChipConnection::Jieli(c) => c.address(),
        }
    }

    async fn detect_product_id(&self) -> Option<u16> {
        match self {
            ChipConnection::Bluetrum(c) => c.detect_product_id().await,
            ChipConnection::Jieli(c) => c.detect_product_id().await,
        }
    }
}

/// Manages the BLE connection lifecycle with a state machine.
/// All access goes through `send()`.
/// Battery tracking has been removed — the OS Bluetooth stack handles this.
pub struct ConnectionManager {
    inner: Arc<RwLock<Inner>>,
    config: ConnectionConfig,
    state_tx: broadcast::Sender<ConnectionState>,
}

impl ConnectionManager {
    pub fn new(config: ConnectionConfig) -> Self {
        let (state_tx, _) = broadcast::channel(16);
        Self {
            inner: Arc::new(RwLock::new(Inner {
                state: ConnectionState::Disconnected,
                connection: None,
            })),
            config,
            state_tx,
        }
    }

    // ── State management ──

    pub fn state(&self) -> ConnectionState {
        self.inner.try_read().map(|i| i.state.clone()).unwrap_or(ConnectionState::Disconnected)
    }

    pub fn subscribe_state(&self) -> broadcast::Receiver<ConnectionState> {
        self.state_tx.subscribe()
    }

    async fn set_state(&self, new_state: ConnectionState) {
        let mut inner = self.inner.write().await;
        inner.state = new_state.clone();
        let _ = self.state_tx.send(new_state);
    }

    // ── Connection lifecycle ──

    pub async fn connect(&self, address: Option<&str>) -> Result<()> {
        let already_connected = {
            let inner = self.inner.read().await;
            if let Some(ref conn) = inner.connection {
                if conn.is_connected().await {
                    return Ok(());
                }
            }
            inner.connection.is_some()
        };

        if already_connected {
            self.disconnect().await?;
        }

        self.set_state(ConnectionState::Scanning).await;

        let peripheral = if let Some(addr) = address {
            // Quick targeted scan to find the peripheral by address
            use btleplug::platform::Manager;
            use btleplug::api::{Central, Manager as _, Peripheral as _, ScanFilter};
            use std::str::FromStr;
            let manager = Manager::new().await?;
            let adapters = manager.adapters().await?;
            let adapter = adapters.into_iter().next()
                .ok_or(Error::DeviceNotFound { retries: 0 })?;
            let bdaddr = btleplug::api::BDAddr::from_str(addr)
                .map_err(|_| Error::Ipc(format!("Invalid address: {}", addr)))?;
            // Scan briefly to discover the peripheral. Use an empty filter:
            // some Oraimo devices (e.g. BoomPop Pro) do not advertise the
            // ff17/fe2c service UUIDs.
            adapter.start_scan(ScanFilter::default()).await?;
            tokio::time::sleep(Duration::from_secs(3)).await;
            let peripherals = adapter.peripherals().await?;
            let _ = adapter.stop_scan().await;
            let mut found = None;
            for p in &peripherals {
                if p.address() == bdaddr {
                    found = Some(p.clone());
                    break;
                }
            }
            found.ok_or(Error::DeviceNotFound { retries: 0 })?
        } else {
            DeviceScanner::find_device(self.config.scan_timeout).await?
        };

        self.set_state(ConnectionState::Connecting).await;

        let conn = Self::open_connection(peripheral).await?;

        let mut inner = self.inner.write().await;
        inner.connection = Some(conn);
        inner.state = ConnectionState::Connected;
        let _ = self.state_tx.send(ConnectionState::Connected);

        Ok(())
    }

    pub async fn disconnect(&self) -> Result<()> {
        let mut inner = self.inner.write().await;
        if let Some(conn) = inner.connection.take() {
            conn.disconnect().await?;
        }
        inner.state = ConnectionState::Disconnected;
        let _ = self.state_tx.send(ConnectionState::Disconnected);
        Ok(())
    }

    /// Connect and pick the chip protocol from the discovered GATT services.
    async fn open_connection(
        peripheral: btleplug::platform::Peripheral,
    ) -> Result<ChipConnection> {
        use btleplug::api::Peripheral as _;

        // Start from a clean link: a previous failed attempt can leave the ACL
        // connection open, which makes the device skip the auth handshake.
        if peripheral.is_connected().await.unwrap_or(false) {
            let _ = peripheral.disconnect().await;
            tokio::time::sleep(Duration::from_millis(400)).await;
        }
        peripheral.connect().await?;
        peripheral.discover_services().await?;
        let is_jieli = peripheral
            .characteristics()
            .iter()
            .any(|c| c.uuid == crate::protocol::UUID_JIELI_WRITE);
        if is_jieli {
            crate::log::info("BLE", "detected JieLi RCSP device");
            Ok(ChipConnection::Jieli(JieliConnection::new(peripheral).await?))
        } else {
            Ok(ChipConnection::Bluetrum(BleConnection::new(peripheral).await?))
        }
    }

    pub async fn reconnect(&self) -> Result<()> {
        self.set_state(ConnectionState::Reconnecting).await;

        // Try normal reconnect first
        if let Ok(()) = self.connect(None).await {
            return Ok(());
        }

        // Fall back to rediscovery reconnect
        let inner = self.inner.read().await;
        if let Some(ref conn) = inner.connection {
            conn.reconnect_with_rediscovery().await?;
            return Ok(());
        }

        Err(Error::ConnectionLost)
    }

    /// Ensure we have an active connection, reconnecting if needed.
    pub async fn ensure_connected(&self) -> Result<()> {
        let is_ok = {
            let inner = self.inner.read().await;
            match inner.connection.as_ref() {
                Some(conn) => conn.is_connected().await,
                None => false,
            }
        };

        if is_ok {
            return Ok(());
        }

        self.reconnect().await
    }

    // ── Send commands ──

    /// Send a typed command and get back a typed response.
    ///
    /// For handshake (query) commands, this waits for a notification response.
    /// For direct commands, it fires and forgets with a small delay.
    pub async fn send<C: BleCommand>(&self, cmd: &C) -> Result<C::Response> {
        self.ensure_connected().await?;

        let inner = self.inner.read().await;
        let chip = inner.connection.as_ref().ok_or(Error::NotConnected)?;
        if chip.is_jieli() {
            return Err(Error::UnsupportedChip("JieLi RCSP (Bluetrum command)"));
        }
        let conn = chip.as_bluetrum().ok_or(Error::NotConnected)?;

        let seq = conn.next_seq().await;
        let packet = Packet::new_request(seq, cmd.cmd_id(), cmd.encode());
        conn.write(&packet).await?;

        // Handshake commands expect TLV response via notifications
        if cmd.cmd_id() == CMD_HANDSHAKE {
            let mut rx = conn.response_rx();
            tokio::select! {
                Ok(pkt) = rx.recv() => {
                    cmd.decode(&pkt.payload)
                }
                _ = tokio::time::sleep(Duration::from_secs(3)) => {
                    Err(Error::Timeout(Duration::from_secs(3)))
                }
            }
        } else {
            // Fire-and-forget: brief pause for device processing
            tokio::time::sleep(Duration::from_millis(200)).await;
            cmd.decode(&[])
        }
    }

    // ── Convenience ──

    /// Check if currently connected.
    pub async fn is_connected(&self) -> bool {
        let inner = self.inner.read().await;
        match inner.connection.as_ref() {
            Some(conn) => conn.is_connected().await,
            None => false,
        }
    }

    /// Get the device address.
    pub async fn address(&self) -> Option<String> {
        let inner = self.inner.read().await;
        inner.connection.as_ref().map(|c| c.address())
    }

    /// Try to detect the product ID from BLE advertisement manufacturer data.
    pub async fn detect_product_id(&self) -> Option<u16> {
        let inner = self.inner.read().await;
        if let Some(conn) = inner.connection.as_ref() {
            conn.detect_product_id().await
        } else {
            None
        }
    }

    /// Query multiple device info IDs at once and return raw TLV payload bytes.
    pub async fn query_device_info(&self, info_ids: &[u8]) -> Result<Vec<u8>> {
        // Build payload: [0xFF, 0x00, id1, 0x00, id2, 0x00, ...]
        let mut payload = vec![0xFF, 0x00];
        for &id in info_ids {
            payload.push(id);
            payload.push(0x00);
        }

        self.ensure_connected().await?;
        let inner = self.inner.read().await;
        let chip = inner.connection.as_ref().ok_or(Error::NotConnected)?;
        if chip.is_jieli() {
            return Err(Error::UnsupportedChip("JieLi RCSP (Bluetrum query)"));
        }
        let conn = chip.as_bluetrum().ok_or(Error::NotConnected)?;

        let seq = conn.next_seq().await;
        let packet = Packet::new_request(seq, CMD_HANDSHAKE, payload);
        conn.write(&packet).await?;

        let mut rx = conn.response_rx();
        tokio::select! {
            Ok(pkt) = rx.recv() => {
                Ok(pkt.payload)
            }
            _ = tokio::time::sleep(Duration::from_secs(5)) => {
                Err(Error::Timeout(Duration::from_secs(5)))
            }
        }
    }

    // ── JieLi RCSP ──

    /// Whether the connected device uses the JieLi RCSP protocol.
    pub async fn is_jieli(&self) -> bool {
        let inner = self.inner.read().await;
        inner
            .connection
            .as_ref()
            .map(|c| c.is_jieli())
            .unwrap_or(false)
    }

    async fn jieli_conn(&self) -> Result<JieliConnection> {
        let inner = self.inner.read().await;
        match inner.connection.as_ref() {
            Some(ChipConnection::Jieli(c)) => Ok(c.clone()),
            Some(_) => Err(Error::UnsupportedChip("not a JieLi device")),
            None => Err(Error::NotConnected),
        }
    }

    pub async fn jieli_set_anc_mode(&self, mode: u8) -> Result<()> {
        self.jieli_conn().await?.set_anc_mode(mode).await
    }

    pub async fn jieli_get_anc_mode(&self) -> Result<u8> {
        self.jieli_conn().await?.get_anc_mode().await
    }

    pub async fn jieli_set_anc_level(&self, mode: u8, level: u8) -> Result<()> {
        self.jieli_conn().await?.set_anc_level(mode, level).await
    }

    pub async fn jieli_set_dual_device(&self, enable: bool) -> Result<()> {
        self.jieli_conn().await?.set_dual_device(enable).await
    }

    pub async fn jieli_set_feature(
        &self,
        feature: crate::protocol::OraFeature,
        enabled: bool,
    ) -> Result<()> {
        self.jieli_conn().await?.set_feature(feature, enabled).await
    }

    pub async fn jieli_get_feature(
        &self,
        feature: crate::protocol::OraFeature,
    ) -> Result<Option<bool>> {
        self.jieli_conn().await?.get_feature(feature).await
    }

    pub async fn jieli_send_custom(
        &self,
        payload: Vec<u8>,
    ) -> Result<Option<crate::protocol::RcspPacket>> {
        self.jieli_conn().await?.send_custom(payload, true).await
    }
}

impl Clone for ConnectionManager {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            config: self.config.clone(),
            state_tx: self.state_tx.clone(),
        }
    }
}
