use crate::errors::{Result, SpaceBudsError};
use crate::protocol::{Packet, UUID_NOTIFY, UUID_WRITE, UUID_BATTERY_LEVEL, CMD_HANDSHAKE};
use btleplug::api::{Central, Manager as _, Peripheral as _, ScanFilter, WriteType, Characteristic};
use btleplug::platform::{Manager, Peripheral};
use futures::stream::StreamExt;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, broadcast};
use tokio::time;

pub struct DeviceScanner;

#[derive(Clone)]
pub struct BleConnection {
    peripheral: Arc<Peripheral>,
    write_char: Characteristic,
    notify_char: Characteristic,
    seq: Arc<Mutex<u8>>,
    response_tx: broadcast::Sender<Packet>,
    battery_tx: broadcast::Sender<(Option<u8>, Option<u8>, Option<u8>)>,
}

impl BleConnection {
    pub async fn new(peripheral: Peripheral) -> Result<Self> {
        let peripheral = Arc::new(peripheral);

        peripheral.connect().await?;
        peripheral.discover_services().await?;
        time::sleep(Duration::from_millis(500)).await;

        let chars = peripheral.characteristics();

        let write_char = chars
            .iter()
            .find(|c| c.uuid == UUID_WRITE)
            .cloned()
            .ok_or(SpaceBudsError::WriteCharNotFound)?;

        let notify_char = chars
            .iter()
            .find(|c| c.uuid == UUID_NOTIFY)
            .cloned()
            .ok_or(SpaceBudsError::NotifyCharNotFound)?;

        peripheral.subscribe(&notify_char).await?;

        let battery_char = chars
            .iter()
            .find(|c| c.uuid == UUID_BATTERY_LEVEL)
            .cloned();

        if let Some(ref bat_char) = battery_char {
            // Only subscribe if the characteristic supports notifications/indications
            if bat_char.properties.contains(btleplug::api::CharPropFlags::NOTIFY)
                || bat_char.properties.contains(btleplug::api::CharPropFlags::INDICATE)
            {
                let _ = peripheral.subscribe(bat_char).await;
            }
        }

        let (response_tx, _) = broadcast::channel(32);
        let response_tx_clone = response_tx.clone();
        let (battery_tx, _) = broadcast::channel(16);
        let battery_tx_clone = battery_tx.clone();
        let mut notification_stream = peripheral.notifications().await?;
        let peripheral_clone = Arc::clone(&peripheral);

        tokio::spawn(async move {
            while let Some(notification) = notification_stream.next().await {
                if notification.uuid == UUID_BATTERY_LEVEL {
                    if !notification.value.is_empty() {
                        let level = notification.value[0].min(100);
                        let _ = battery_tx_clone.send((Some(level), Some(level), None));
                    }
                } else if notification.uuid == UUID_NOTIFY {
                    // Main protocol packets
                    if let Some(packet) = Packet::from_bytes(&notification.value) {
                        let _ = response_tx_clone.send(packet);
                    }
                }
            }
        });

        let conn = Self {
            peripheral,
            write_char,
            notify_char,
            seq: Arc::new(Mutex::new(0)),
            response_tx,
            battery_tx,
        };

        conn.handshake().await?;

        Ok(conn)
    }

    pub async fn handshake(&self) -> Result<()> {
        let packet = Packet::new_request(self.next_seq().await, CMD_HANDSHAKE, vec![0xFF, 0x00]);
        self.write(&packet).await?;
        tokio::time::sleep(Duration::from_millis(200)).await;
        Ok(())
    }

    async fn next_seq(&self) -> u8 {
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

    pub async fn command(&self, cmd_id: u8, payload: Vec<u8>) -> Result<()> {
        let packet = Packet::new_request(self.next_seq().await, cmd_id, payload);
        self.write(&packet).await
    }

    pub async fn query<T, F>(&self, cmd_id: u8, payload: Vec<u8>, parser: F, timeout: Duration) -> Result<Option<T>>
    where
        F: Fn(&Packet) -> Option<T> + Send + 'static,
        T: Send + 'static,
    {
        let packet = Packet::new_request(self.next_seq().await, cmd_id, payload);
        let mut response_rx = self.response_tx.subscribe();
        self.write(&packet).await?;

        tokio::select! {
            Ok(packet) = response_rx.recv() => {
                Ok(parser(&packet))
            }
            _ = time::sleep(timeout) => {
                Ok(None)
            }
        }
    }
    pub fn subscribe_battery(&self) -> broadcast::Receiver<(Option<u8>, Option<u8>, Option<u8>)> {
        self.battery_tx.subscribe()
    }

    pub async fn get_battery_level(&self) -> Result<(Option<u8>, Option<u8>, Option<u8>)> {
        let chars = self.peripheral.characteristics();

        for char in chars.iter() {
            if char.uuid == UUID_BATTERY_LEVEL {
                if let Ok(value) = self.peripheral.read(char).await {
                    if !value.is_empty() {
                        let level = value[0].min(100);
                        return Ok((Some(level), Some(level), None));
                    }
                }
            }
        }

        if let Ok(Some(props)) = self.peripheral.properties().await {
            for (_, data) in &props.manufacturer_data {
                if data.len() >= 4 && (data[0] == 0x06 || data[0] == 0x07) {
                    let left = Some(data[1].min(100));
                    let right = Some(data[2].min(100));
                    let case = if data.len() >= 4 { Some(data[3].min(100)) } else { None };
                    return Ok((left, right, case));
                }
            }
        }
        Ok((None, None, None))
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
        time::sleep(Duration::from_millis(500)).await;
        Ok(())
    }

    pub async fn reconnect_with_rediscovery(&self) -> Result<()> {
        self.peripheral.disconnect().await?;
        time::sleep(Duration::from_millis(500)).await;

        self.peripheral.connect().await?;
        time::sleep(Duration::from_millis(500)).await;

        self.peripheral.discover_services().await?;
        time::sleep(Duration::from_millis(500)).await;

        let chars = self.peripheral.characteristics();

        // Re-subscribe to main notify characteristic
        if let Some(notify_char) = chars.iter().find(|c| c.uuid == UUID_NOTIFY) {
            self.peripheral.subscribe(notify_char).await?;
        }

        // Re-subscribe to battery characteristic if present and supports notify
        if let Some(bat_char) = chars.iter().find(|c| c.uuid == UUID_BATTERY_LEVEL) {
            if bat_char.properties.contains(btleplug::api::CharPropFlags::NOTIFY)
                || bat_char.properties.contains(btleplug::api::CharPropFlags::INDICATE)
            {
                let _ = self.peripheral.subscribe(bat_char).await;
            }
        }

        Ok(())
    }
}

impl DeviceScanner {
    pub async fn find_device(timeout: Duration) -> Result<Peripheral> {
        let manager = Manager::new().await?;
        let adapters = manager.adapters().await?;
        let adapter = adapters.into_iter().next().ok_or(SpaceBudsError::DeviceNotFound)?;

        adapter.start_scan(ScanFilter::default()).await?;
        time::sleep(timeout).await;

        let peripherals = adapter.peripherals().await?;

        for peripheral in peripherals {
            let properties = peripheral.properties().await?.unwrap();
            for uuid in &properties.services {
                let uuid_str = uuid.to_string().to_lowercase();
                if uuid_str.contains("ff17") || uuid_str.contains("fe2c") {
                    return Ok(peripheral);
                }
            }
        }

        Err(SpaceBudsError::DeviceNotFound)
    }

    pub async fn find_device_with_retry(timeout: Duration, max_retries: usize) -> Result<Peripheral> {
        for attempt in 1..=max_retries {
            println!("Scan attempt {}/{}...", attempt, max_retries);
            match Self::find_device(timeout).await {
                Ok(device) => return Ok(device),
                Err(e) if attempt < max_retries => {
                    println!("Device not found, retrying...");
                    time::sleep(Duration::from_secs(2)).await;
                }
                Err(e) => return Err(e),
            }
        }
        Err(SpaceBudsError::DeviceNotFound)
    }
}