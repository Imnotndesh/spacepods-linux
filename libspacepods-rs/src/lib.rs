// lib.rs
mod ble;
mod errors;
mod protocol;
mod commands;
pub mod service;
pub mod client;
pub mod cli;

pub use ble::{BleConnection, DeviceScanner};
pub use errors::{SpaceBudsError, Result};
pub use protocol::*;
pub use commands::*;

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time;

#[derive(Clone)]
pub struct SpaceBuds {
    conn: Arc<Mutex<Option<BleConnection>>>,
    address: Option<String>,
    max_retries: usize,
}

impl SpaceBuds {
    pub async fn new() -> Result<Self> {
        Self::with_address(None).await
    }

    pub async fn with_address(address: Option<String>) -> Result<Self> {
        let buds = Self {
            conn: Arc::new(Mutex::new(None)),
            address,
            max_retries: 3,
        };

        buds.connect().await?;
        Ok(buds)
    }

    pub async fn connect(&self) -> Result<()> {
        let mut conn_lock = self.conn.lock().await;

        // Check if already connected
        if let Some(conn) = conn_lock.as_ref() {
            if conn.is_connected().await {
                return Ok(());
            }
        }

        // Find and connect to device
        let peripheral = if let Some(_addr) = &self.address {
            // TODO: Connect to specific address
            DeviceScanner::find_device(Duration::from_secs(10)).await?
        } else {
            DeviceScanner::find_device(Duration::from_secs(10)).await?
        };

        let conn = BleConnection::new(peripheral).await?;
        *conn_lock = Some(conn.clone());

        Ok(())
    }
    pub async fn with_connection<F, Fut, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(BleConnection) -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        self.ensure_connected().await?;
        let conn_lock = self.conn.lock().await;
        let conn = conn_lock.as_ref().unwrap().clone();
        drop(conn_lock);
        f(conn).await
    }

    pub async fn ensure_connected(&self) -> Result<()> {
        let conn_lock = self.conn.lock().await;

        if let Some(conn) = conn_lock.as_ref() {
            if conn.is_connected().await {
                return Ok(());
            }
        }

        drop(conn_lock);
        self.connect().await
    }

    pub async fn disconnect(&self) -> Result<()> {
        let mut conn_lock = self.conn.lock().await;
        if let Some(conn) = conn_lock.take() {
            conn.disconnect().await?;
        }
        Ok(())
    }

    pub async fn reconnect(&self) -> Result<()> {
        self.disconnect().await?;
        time::sleep(Duration::from_secs(1)).await;
        match self.connect().await {
            Ok(_) => Ok(()),
            Err(_e) => {
                if let Some(conn) = self.conn.lock().await.as_ref() {
                    conn.reconnect_with_rediscovery().await?;
                }
                Ok(())
            }
        }
    }

    pub fn anc(&self) -> AncController {
        AncController::new(self.clone())
    }

    pub fn eq(&self) -> EqController {
        EqController::new(self.clone())
    }

    pub fn features(&self) -> FeatureController {
        FeatureController::new(self.clone())
    }

    pub async fn is_connected(&self) -> bool {
        let conn_lock = self.conn.lock().await;
        if let Some(conn) = conn_lock.as_ref() {
            conn.is_connected().await
        } else {
            false
        }
    }

    pub fn address(&self) -> Option<String> {
        self.address.clone()
    }
}