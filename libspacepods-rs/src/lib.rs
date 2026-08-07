pub mod protocol;
pub mod errors;
pub mod connection;
pub mod commands;
pub mod ipc;
pub mod beacon;
pub mod cli;
pub mod device_profile;
pub mod log;

pub use errors::{Error, Result};
pub use protocol::*;
pub use connection::*;
pub use commands::*;
pub use ipc::*;

/// Current version — set at build time from Cargo.toml or GIT_TAG env.
pub const VERSION: &str = env!("SPACEPODS_VERSION");

use std::time::Duration;

/// Top-level handle to a pair of SpaceBuds.
///
/// Provides access to controllers for ANC, EQ, and features.
/// All BLE operations go through the internal ConnectionManager.
///
/// # Example
///
/// ```no_run
/// use spacepods::SpaceBuds;
///
/// # async fn example() -> Result<(), spacepods::Error> {
/// let buds = SpaceBuds::builder()
///     .scan_timeout(Duration::from_secs(5))
///     .max_retries(3)
///     .auto_connect(true)
///     .build()
///     .await?;
///
/// let mode = buds.anc().get_mode().await?;
/// println!("Current ANC mode: {}", mode);
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct SpaceBuds {
    pub(crate) manager: connection::ConnectionManager,
}

impl SpaceBuds {
    /// Create a new `SpaceBuds` instance with default settings.
    /// Automatically connects to the nearest device.
    pub async fn new() -> Result<Self> {
        Self::builder().auto_connect(true).build().await
    }

    /// Create a disconnected instance (lazy connection).
    pub fn new_disconnected() -> Self {
        Self {
            manager: connection::ConnectionManager::new(
                connection::ConnectionConfig::default(),
            ),
        }
    }

    /// Get a builder for fine-grained configuration.
    pub fn builder() -> SpaceBudsBuilder {
        SpaceBudsBuilder::new()
    }

    /// Connect to the device.
    pub async fn connect(&self) -> Result<()> {
        self.manager.connect(None).await
    }

    /// Connect to a specific device by BLE address.
    pub async fn connect_to(&self, address: &str) -> Result<()> {
        self.manager.connect(Some(address)).await
    }

    /// Disconnect from the device.
    pub async fn disconnect(&self) -> Result<()> {
        self.manager.disconnect().await
    }

    /// Reconnect to the device.
    pub async fn reconnect(&self) -> Result<()> {
        self.manager.reconnect().await
    }

    /// Check if connected.
    pub async fn is_connected(&self) -> bool {
        self.manager.is_connected().await
    }

    /// Get the device address.
    pub async fn address(&self) -> Option<String> {
        self.manager.address().await
    }

    /// Try to detect the product ID from BLE advertisement manufacturer data.
    pub async fn detect_product_id(&self) -> Option<u16> {
        self.manager.detect_product_id().await
    }

    /// Query multiple device info IDs at once.
    pub async fn query_device_info(&self, info_ids: &[u8]) -> Result<Vec<u8>> {
        self.manager.query_device_info(info_ids).await
    }

    /// Send a raw BLE command by command ID and payload.
    /// Used for features not yet wrapped in typed controllers.
    pub async fn send_raw(&self, cmd_id: u8, payload: Vec<u8>) -> Result<()> {
        use crate::commands::BleCommand;
        use crate::protocol::constants::*;

        struct Raw {
            cmd_id: u8,
            payload: Vec<u8>,
        }
        impl BleCommand for Raw {
            type Response = ();
            fn cmd_id(&self) -> u8 { self.cmd_id }
            fn encode(&self) -> Vec<u8> { self.payload.clone() }
            fn decode(&self, _payload: &[u8]) -> Result<Self::Response, crate::Error> { Ok(()) }
        }

        self.manager.send(&Raw { cmd_id, payload }).await?;
        Ok(())
    }

    // ── Controllers (borrowing, not cloning) ──

    /// Access ANC controls.
    pub fn anc(&self) -> commands::AncController<'_> {
        commands::AncController { buds: self }
    }

    /// Access EQ controls.
    pub fn eq(&self) -> commands::EqController<'_> {
        commands::EqController { buds: self }
    }

    /// Access feature controls (adaptive ANC, dual device).
    pub fn features(&self) -> commands::FeatureController<'_> {
        commands::FeatureController { buds: self }
    }

    /// Subscribe to connection state changes.
    pub fn subscribe_state(&self) -> tokio::sync::broadcast::Receiver<protocol::ConnectionState> {
        self.manager.subscribe_state()
    }

    /// Get current connection state.
    pub fn state(&self) -> protocol::ConnectionState {
        self.manager.state()
    }
}

// ── Builder ──

/// Builder for configuring and constructing a `SpaceBuds` instance.
pub struct SpaceBudsBuilder {
    config: connection::ConnectionConfig,
    auto_connect: bool,
}

impl SpaceBudsBuilder {
    pub fn new() -> Self {
        Self {
            config: connection::ConnectionConfig::default(),
            auto_connect: false,
        }
    }

    /// Set the BLE scan timeout.
    pub fn scan_timeout(mut self, timeout: Duration) -> Self {
        self.config.scan_timeout = timeout;
        self
    }

    /// Set the maximum number of reconnection retries.
    pub fn max_retries(mut self, retries: usize) -> Self {
        self.config.max_retries = retries;
        self
    }

    /// Set the delay between reconnection attempts.
    pub fn reconnect_delay(mut self, delay: Duration) -> Self {
        self.config.reconnect_delay = delay;
        self
    }

    /// Whether to automatically connect on build.
    pub fn auto_connect(mut self, enabled: bool) -> Self {
        self.auto_connect = enabled;
        self
    }

    /// Build the `SpaceBuds` instance.
    /// If `auto_connect` is true, also connects to the device.
    pub async fn build(self) -> Result<SpaceBuds> {
        let buds = SpaceBuds {
            manager: connection::ConnectionManager::new(self.config),
        };

        if self.auto_connect {
            buds.manager.connect(None).await?;
        }

        Ok(buds)
    }
}

impl Default for SpaceBudsBuilder {
    fn default() -> Self {
        Self::new()
    }
}
