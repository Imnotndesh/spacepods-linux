use crate::commands::BleCommand;
use crate::connection::ble::BleConnection;
use crate::connection::jieli::JieliConnection;
use crate::connection::scanner::DeviceScanner;
use crate::protocol::{ConnectionState, Packet, CMD_HANDSHAKE,
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

/// The command protocol a device speaks. Each family uses different GATT
/// characteristics and different command frames, so a command for one family must
/// never reach a device of the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChipFamily {
    /// Bluetrum protocol (most SpaceBuds): characteristics `ff17` (write) and `ff18` (notify).
    Bluetrum,
    /// JieLi RCSP protocol: characteristics `ae01` (write) and `ae02` (notify).
    Jieli,
}

/// Decide the protocol from the device's characteristic UUIDs.
///
/// A family is chosen only when its own write and notify characteristics are both
/// present. A device that matches neither, or matches both, is refused. There is no
/// default: an unknown device gets no commands.
pub fn classify_chip(uuids: &[uuid::Uuid]) -> std::result::Result<ChipFamily, &'static str> {
    use crate::protocol::{UUID_JIELI_NOTIFY, UUID_JIELI_WRITE, UUID_NOTIFY, UUID_WRITE};
    let has = |u: uuid::Uuid| uuids.contains(&u);
    let bluetrum = has(UUID_WRITE) && has(UUID_NOTIFY);
    let jieli = has(UUID_JIELI_WRITE) && has(UUID_JIELI_NOTIFY);
    match (bluetrum, jieli) {
        (true, false) => Ok(ChipFamily::Bluetrum),
        (false, true) => Ok(ChipFamily::Jieli),
        (true, true) => Err("device matches both chip protocols; refusing to guess"),
        (false, false) => Err("unrecognised device; no supported command protocol"),
    }
}

#[cfg(test)]
mod chip_tests {
    use super::*;
    use crate::protocol::{UUID_JIELI_NOTIFY, UUID_JIELI_WRITE, UUID_NOTIFY, UUID_WRITE};

    #[test]
    fn bluetrum_needs_both_its_characteristics() {
        assert_eq!(classify_chip(&[UUID_WRITE, UUID_NOTIFY]), Ok(ChipFamily::Bluetrum));
        assert!(classify_chip(&[UUID_WRITE]).is_err());
        assert!(classify_chip(&[UUID_NOTIFY]).is_err());
    }

    #[test]
    fn jieli_needs_both_its_characteristics() {
        assert_eq!(classify_chip(&[UUID_JIELI_WRITE, UUID_JIELI_NOTIFY]), Ok(ChipFamily::Jieli));
        assert!(classify_chip(&[UUID_JIELI_WRITE]).is_err());
    }

    #[test]
    fn unknown_device_is_refused_not_defaulted() {
        assert!(classify_chip(&[]).is_err());
        assert!(classify_chip(&[uuid::Uuid::nil()]).is_err());
    }

    #[test]
    fn device_matching_both_families_is_refused() {
        let both = [UUID_WRITE, UUID_NOTIFY, UUID_JIELI_WRITE, UUID_JIELI_NOTIFY];
        assert!(classify_chip(&both).is_err());
    }
}
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
    /// Serializes connection lifecycle operations so a manual `Connect` cannot
    /// race the background auto-connect loop (BlueZ answers the loser with
    /// `In Progress`).
    lifecycle: Arc<Mutex<()>>,
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
            lifecycle: Arc::new(Mutex::new(())),
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
        let _guard = self.lifecycle.lock().await;
        self.connect_inner(address).await
    }

    async fn connect_inner(&self, address: Option<&str>) -> Result<()> {
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
            self.disconnect_inner().await?;
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
            // A device BlueZ already knows (paired, or connected for audio) is used directly.
            let known = adapter.peripherals().await?;
            if let Some(p) = known.iter().find(|p| p.address() == bdaddr) {
                p.clone()
            } else {
                // Otherwise scan. The earbuds change their BLE address between sessions, but
                // the advertising beacon carries the classic address, which is the one the
                // person knows. Match on either.
                let found = crate::beacon::scanner::scan_for_devices(
                    self.config.scan_timeout.min(Duration::from_secs(6)),
                )
                .await?;
                let target = found.into_iter().find(|d| {
                    let by_ble = d.ble_address.as_deref().is_some_and(|a| a.eq_ignore_ascii_case(addr));
                    let by_classic = d
                        .beacon
                        .as_ref()
                        .and_then(|b| b.bt_address_string())
                        .is_some_and(|a| a.eq_ignore_ascii_case(addr));
                    by_ble || by_classic
                });
                match target {
                    Some(d) => adapter.peripheral(&d.peripheral_id).await?,
                    None => return Err(Error::DeviceNotFound { retries: 0 }),
                }
            }
        } else {
            DeviceScanner::find_device(self.config.scan_timeout).await?
        };

        self.set_state(ConnectionState::Connecting).await;

        // The control link can fail while audio holds the device. Then the audio link is
        // dropped once so the BLE link can come up, and audio is restored afterwards. This
        // runs only on connect, never on a settings change.
        // Note whether audio was up before this connect. The failed attempts can drop it, so it
        // is restored afterwards if it was.
        let peripheral_address = {
            use btleplug::api::Peripheral as _;
            peripheral.address().to_string()
        };
        let audio_was_up = match crate::bluez::audio_side_of(&peripheral_address).await {
            Some(audio) => crate::bluez::audio_connected(&audio).await,
            None => false,
        };
        let conn = match Self::open_connection_with_retries(peripheral.clone(), 2).await {
            Err(Error::ConnectionLost) => {
                let Some(audio) = crate::bluez::audio_side_of(&peripheral_address).await else {
                    crate::log::warn("BLE", &format!("no paired audio device for {peripheral_address}; cannot drop audio"));
                    return Err(Error::ConnectionLost);
                };
                crate::log::warn("BLE", "control link blocked while audio is up; dropping audio briefly");
                if let Err(e) = crate::bluez::disconnect_device(&audio).await {
                    crate::log::warn("BLE", &format!("audio disconnect failed: {e}"));
                }
                tokio::time::sleep(Duration::from_secs(2)).await;
                let result = Self::open_connection_with_retries(peripheral, 3).await;
                // Audio comes back whether or not the control link did.
                if audio_was_up && !crate::bluez::ensure_audio(&audio).await {
                    crate::log::warn("AUDIO", "audio did not come back after the control link");
                }
                result?
            }
            other => other?,
        };

        let mut inner = self.inner.write().await;
        inner.connection = Some(conn);
        inner.state = ConnectionState::Connected;
        let _ = self.state_tx.send(ConnectionState::Connected);

        Ok(())
    }

    pub async fn disconnect(&self) -> Result<()> {
        let _guard = self.lifecycle.lock().await;
        self.disconnect_inner().await
    }

    async fn disconnect_inner(&self) -> Result<()> {
        let mut inner = self.inner.write().await;
        if let Some(conn) = inner.connection.take() {
            conn.disconnect().await?;
        }
        inner.state = ConnectionState::Disconnected;
        let _ = self.state_tx.send(ConnectionState::Disconnected);
        Ok(())
    }

    /// Open the control link, retrying while it comes up without services. Other errors
    /// are returned at once.
    async fn open_connection_with_retries(
        peripheral: btleplug::platform::Peripheral,
        attempts: usize,
    ) -> Result<ChipConnection> {
        let mut attempt = 1;
        loop {
            match Self::open_connection(peripheral.clone()).await {
                Err(Error::ConnectionLost) if attempt < attempts => {
                    crate::log::warn(
                        "BLE",
                        &format!("control link attempt {attempt} of {attempts} failed; retrying"),
                    );
                    attempt += 1;
                    tokio::time::sleep(Duration::from_secs(3)).await;
                }
                other => return other,
            }
        }
    }

    /// Connect and pick the chip protocol from the discovered GATT services.
    async fn open_connection(
        peripheral: btleplug::platform::Peripheral,
    ) -> Result<ChipConnection> {
        use btleplug::api::Peripheral as _;

        // Start from a clean link: a previous failed attempt can leave the ACL
        // connection open, which makes the device skip the auth handshake.
        // Retry once — the first GATT connect can fail while BlueZ is still
        // settling after a scan.
        let mut last_err = None;
        for attempt in 1..=2 {
            if peripheral.is_connected().await.unwrap_or(false) {
                let _ = peripheral.disconnect().await;
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
            match peripheral.connect().await {
                Ok(()) => {
                    last_err = None;
                    break;
                }
                Err(e) => {
                    crate::log::warn(
                        "BLE",
                        &format!("GATT connect attempt {} failed: {}", attempt, e),
                    );
                    let _ = peripheral.disconnect().await;
                    last_err = Some(e);
                    tokio::time::sleep(Duration::from_millis(800)).await;
                }
            }
        }
        if let Some(e) = last_err {
            return Err(e.into());
        }
        peripheral.discover_services().await?;
        let mut uuids: Vec<uuid::Uuid> = peripheral.characteristics().iter().map(|c| c.uuid).collect();
        // Right after the link comes up, discovery can finish before every characteristic is
        // visible. Look once more before refusing, so a slow discovery is not mistaken for an
        // unknown device. A genuinely unknown device is still refused.
        if classify_chip(&uuids).is_err() {
            tokio::time::sleep(Duration::from_millis(1500)).await;
            peripheral.discover_services().await?;
            uuids = peripheral.characteristics().iter().map(|c| c.uuid).collect();
        }
        if uuids.is_empty() {
            // No services at all: the control link did not come up. This is not an unknown
            // device, so it is not refused as one. The caller can try again.
            let _ = peripheral.disconnect().await;
            crate::log::warn("BLE", "no GATT services after connect; the control link did not come up");
            return Err(Error::ConnectionLost);
        }
        match classify_chip(&uuids) {
            Ok(ChipFamily::Jieli) => {
                crate::log::info("BLE", "detected JieLi RCSP device");
                Ok(ChipConnection::Jieli(JieliConnection::new(peripheral).await?))
            }
            Ok(ChipFamily::Bluetrum) => {
                crate::log::info("BLE", "detected Bluetrum device");
                Ok(ChipConnection::Bluetrum(BleConnection::new(peripheral).await?))
            }
            Err(reason) => {
                // Refuse the connection. Nothing is written to a device we cannot identify:
                // the wrong command on the wrong chip can damage the earbuds.
                let seen: Vec<String> = uuids.iter().map(|u| u.to_string()).collect();
                crate::log::warn(
                    "BLE",
                    &format!("refusing to talk to this device: {} (characteristics seen: {})", reason, seen.join(" ")),
                );
                let _ = peripheral.disconnect().await;
                Err(Error::UnrecognisedDevice(reason))
            }
        }
    }

    pub async fn reconnect(&self) -> Result<()> {
        let _guard = self.lifecycle.lock().await;
        self.reconnect_inner().await
    }

    async fn reconnect_inner(&self) -> Result<()> {
        self.set_state(ConnectionState::Reconnecting).await;

        // Try normal reconnect first
        if let Ok(()) = self.connect_inner(None).await {
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
    /// Battery level over GATT, for Bluetrum devices. Read only. `None` for other chips
    /// or when the device does not report it.
    pub async fn read_battery_level(&self) -> Option<u8> {
        let inner = self.inner.read().await;
        match inner.connection.as_ref() {
            Some(ChipConnection::Bluetrum(c)) => c.read_battery_level().await,
            _ => None,
        }
    }

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

    pub async fn jieli_get_anc_level(&self) -> Result<(u8, u8)> {
        self.jieli_conn().await?.get_anc_level().await
    }

    pub async fn jieli_get_eq_value(&self) -> Result<Option<(u8, Vec<i8>)>> {
        self.jieli_conn().await?.get_eq_value().await
    }

    pub async fn jieli_get_eq_presets(&self) -> Result<(Vec<u16>, Vec<(u8, Vec<i8>)>)> {
        self.jieli_conn().await?.get_eq_presets().await
    }

    pub async fn jieli_set_eq_value(&self, mode: u8, gains: &[i8]) -> Result<()> {
        self.jieli_conn().await?.set_eq_value(mode, gains).await
    }

    pub async fn jieli_set_eq_preset(&self, mode: u8) -> Result<()> {
        self.jieli_conn().await?.set_eq_preset(mode).await
    }

    /// Capability bitfield reported by a JieLi device (custom command 33).
    pub async fn jieli_get_capabilities(&self) -> Option<Vec<u8>> {
        match self.jieli_conn().await {
            Ok(c) => c.get_capabilities().await,
            Err(_) => None,
        }
    }

    pub async fn jieli_rename(&self, name: &str) -> Result<()> {
        self.jieli_conn().await?.rename(name).await
    }

    pub async fn jieli_set_adv_setting(&self, ltv_type: u8, data: &[u8]) -> Result<()> {
        self.jieli_conn().await?.set_adv_setting(ltv_type, data).await
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
            lifecycle: self.lifecycle.clone(),
        }
    }
}
