use crate::commands::BleCommand;
use crate::connection::ble::BleConnection;
use crate::connection::scanner::DeviceScanner;
use crate::protocol::{ConnectionState, Packet, CMD_HANDSHAKE};
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
    connection: Option<BleConnection>,
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

    pub async fn connect(&self) -> Result<()> {
        let already_connected = {
            let inner = self.inner.read().await;
            if let Some(ref conn) = inner.connection {
                if conn.is_connected().await {
                    return Ok(());
                }
            }
            inner.connection.is_some()
        };

        // If we had an old connection, disconnect first
        if already_connected {
            self.disconnect().await?;
        }

        self.set_state(ConnectionState::Scanning).await;

        let peripheral = DeviceScanner::find_device(self.config.scan_timeout).await?;

        self.set_state(ConnectionState::Connecting).await;

        let conn = BleConnection::new(peripheral).await?;

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

    pub async fn reconnect(&self) -> Result<()> {
        self.set_state(ConnectionState::Reconnecting).await;

        // Try normal reconnect first
        if let Ok(()) = self.connect().await {
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
        let conn = inner.connection.as_ref().ok_or(Error::NotConnected)?;

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
