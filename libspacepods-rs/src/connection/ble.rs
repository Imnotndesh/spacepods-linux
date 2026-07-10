use crate::protocol::{Packet, UUID_NOTIFY, UUID_WRITE, CMD_HANDSHAKE};
use crate::{Error, Result};
use btleplug::api::{Peripheral as _, WriteType};
use btleplug::platform::Peripheral;
use futures::stream::StreamExt;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, Mutex};

/// Low-level BLE connection wrapper.
/// Manages a single peripheral and its notification streams.
/// Battery tracking has been removed — the OS Bluetooth stack handles this.
#[derive(Clone)]
pub struct BleConnection {
    peripheral: Arc<Peripheral>,
    write_char: btleplug::api::Characteristic,
    notify_char: btleplug::api::Characteristic,
    seq: Arc<Mutex<u8>>,
    response_tx: broadcast::Sender<Packet>,
}

impl BleConnection {
    pub async fn new(peripheral: Peripheral) -> Result<Self> {
        let peripheral = Arc::new(peripheral);

        peripheral.connect().await?;
        peripheral.discover_services().await?;
        tokio::time::sleep(Duration::from_millis(500)).await;

        let chars = peripheral.characteristics();

        let write_char = chars
            .iter()
            .find(|c| c.uuid == UUID_WRITE)
            .cloned()
            .ok_or(Error::WriteCharNotFound)?;

        let notify_char = chars
            .iter()
            .find(|c| c.uuid == UUID_NOTIFY)
            .cloned()
            .ok_or(Error::NotifyCharNotFound)?;

        peripheral.subscribe(&notify_char).await?;

        // Set up response notification channel (no battery — OS handles that)
        let (response_tx, _) = broadcast::channel(32);
        let response_tx_clone = response_tx.clone();

        // Spawn notification listener — only listens for SpaceBuds command responses
        let mut notification_stream = peripheral.notifications().await?;
        tokio::spawn(async move {
            while let Some(notification) = notification_stream.next().await {
                if notification.uuid == UUID_NOTIFY {
                    if let Some(packet) = Packet::from_bytes(&notification.value) {
                        let _ = response_tx_clone.send(packet);
                    }
                }
                // Battery notifications are intentionally ignored — the OS surfaces them
            }
        });

        let conn = Self {
            peripheral,
            write_char,
            notify_char,
            seq: Arc::new(Mutex::new(0)),
            response_tx,
        };

        // Perform initial handshake
        conn.handshake().await?;

        Ok(conn)
    }

    pub async fn handshake(&self) -> Result<()> {
        let packet = Packet::new_request(self.next_seq().await, CMD_HANDSHAKE, vec![0xFF, 0x00]);
        self.write(&packet).await?;
        tokio::time::sleep(Duration::from_millis(200)).await;
        Ok(())
    }

    pub async fn next_seq(&self) -> u8 {
        let mut seq = self.seq.lock().await;
        let current = *seq;
        *seq = current.wrapping_add(1);
        current
    }

    pub async fn write(&self, packet: &Packet) -> Result<()> {
        self.peripheral
            .write(&self.write_char, &packet.to_bytes(), WriteType::WithoutResponse)
            .await?;
        Ok(())
    }

    pub fn response_rx(&self) -> broadcast::Receiver<Packet> {
        self.response_tx.subscribe()
    }

    pub async fn is_connected(&self) -> bool {
        self.peripheral.is_connected().await.unwrap_or(false)
    }

    pub async fn disconnect(&self) -> Result<()> {
        self.peripheral.disconnect().await?;
        Ok(())
    }

    pub fn address(&self) -> String {
        self.peripheral.address().to_string()
    }

    pub async fn force_rediscover(&self) -> Result<()> {
        self.peripheral.discover_services().await?;
        tokio::time::sleep(Duration::from_millis(500)).await;
        Ok(())
    }

    pub async fn reconnect_with_rediscovery(&self) -> Result<()> {
        self.peripheral.disconnect().await?;
        tokio::time::sleep(Duration::from_millis(500)).await;

        self.peripheral.connect().await?;
        tokio::time::sleep(Duration::from_millis(500)).await;

        self.peripheral.discover_services().await?;
        tokio::time::sleep(Duration::from_millis(500)).await;

        let chars = self.peripheral.characteristics();

        if let Some(notify_char) = chars.iter().find(|c| c.uuid == UUID_NOTIFY) {
            self.peripheral.subscribe(notify_char).await?;
        }

        Ok(())
    }
}
