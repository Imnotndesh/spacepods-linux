use crate::TlvParser;
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
    pub(crate) response_tx: broadcast::Sender<Packet>,
    battery_tx: broadcast::Sender<(Option<u8>, Option<u8>, Option<u8>)>,
}

impl BleConnection {
    pub async fn new(peripheral: Peripheral) -> Result<Self> {
        let peripheral = Arc::new(peripheral);
        if !peripheral.is_connected().await.unwrap_or(false) {
            peripheral.connect().await?;
        }
        if peripheral.services().is_empty() {
            peripheral.discover_services().await?;
            time::sleep(Duration::from_millis(500)).await;
        }
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

    pub(crate) async fn next_seq(&self) -> u8 {
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
        // flush stale messages
        while response_rx.try_recv().is_ok() {}
        self.write(&packet).await?;

        tokio::select! {
            result = async {
                loop {
                    match response_rx.recv().await {
                        Ok(p) => if let Some(v) = parser(&p) { return Some(v); },
                        Err(_) => return None,
                    }
                }
            } => Ok(result),
            _ = time::sleep(timeout) => {
                eprintln!("[BLE] Query timeout for cmd 0x{:02x}", cmd_id);
                Ok(None)
            },
        }
    }

    pub fn subscribe_battery(&self) -> broadcast::Receiver<(Option<u8>, Option<u8>, Option<u8>)> {
        self.battery_tx.subscribe()
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
        if let Some(notify_char) = chars.iter().find(|c| c.uuid == UUID_NOTIFY) {
            self.peripheral.subscribe(notify_char).await?;
        }
        if let Some(bat_char) = chars.iter().find(|c| c.uuid == UUID_BATTERY_LEVEL) {
            if bat_char.properties.contains(btleplug::api::CharPropFlags::NOTIFY)
                || bat_char.properties.contains(btleplug::api::CharPropFlags::INDICATE)
            {
                let _ = self.peripheral.subscribe(bat_char).await;
            }
        }
        Ok(())
    }

    pub async fn get_battery_level(&self) -> Result<(Option<u8>, Option<u8>, Option<u8>)> {
        use crate::protocol::ID_BATTERY;
        let result = self.query(
            CMD_HANDSHAKE,
            vec![0xFF, 0x00, ID_BATTERY, 0x00],
            |packet| {
                if packet.cmd_id == CMD_HANDSHAKE {
                    let mut parser = TlvParser::new(&packet.payload);
                    if let Some(data) = parser.get_bytes(ID_BATTERY) {
                        let left  = data.get(0).map(|&b| b.min(100));
                        let right = data.get(1).map(|&b| b.min(100));
                        let case  = data.get(2).map(|&b| b.min(100));
                        return Some((left, right, case));
                    }
                }
                None
            },
            Duration::from_secs(3),
        ).await?;
        Ok(result.unwrap_or((None, None, None)))
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
    pub async fn find_specific_device(timeout: Duration, target_address: &str) -> Result<Peripheral> {
        if let Ok(p) = Self::find_already_connected(Some(target_address)).await {
            println!("[BLE] Reusing existing OS connection for {}", target_address);
            return Ok(p);
        }

        let peripherals = Self::scan_devices(timeout).await?;
        for peripheral in peripherals {
            if peripheral.address().to_string().to_lowercase() == target_address.to_lowercase() {
                return Ok(peripheral);
            }
        }
        Err(SpaceBudsError::DeviceNotFound)
    }
    pub async fn find_already_connected(target_address: Option<&str>) -> Result<Peripheral> {
        let manager = Manager::new().await?;
        let adapters = manager.adapters().await?;
        let adapter = adapters.into_iter().next().ok_or(SpaceBudsError::DeviceNotFound)?;
        let peripherals = adapter.peripherals().await?;

        for peripheral in peripherals {
            if !peripheral.is_connected().await.unwrap_or(false) {
                continue;
            }

            let address = peripheral.address().to_string();

            if let Some(target) = target_address {
                if address.to_lowercase() != target.to_lowercase() {
                    continue;
                }
            }

            if peripheral.services().is_empty() {
                peripheral.discover_services().await?;
            }

            let chars = peripheral.characteristics();
            let has_write = chars.iter().any(|c| c.uuid == UUID_WRITE);
            let has_notify = chars.iter().any(|c| c.uuid == UUID_NOTIFY);

            if has_write && has_notify {
                return Ok(peripheral);
            }

            if let Ok(Some(props)) = peripheral.properties().await {
                let uuids: Vec<String> = props.services.iter()
                    .map(|u| u.to_string().to_lowercase())
                    .collect();
                if uuids.iter().any(|u| u.contains("ff17") || u.contains("fe2c")) {
                    return Ok(peripheral);
                }
            }
        }

        Err(SpaceBudsError::DeviceNotFound)
    }

    pub async fn scan_devices(timeout: Duration) -> Result<Vec<Peripheral>> {
        use btleplug::api::CentralEvent;
        use futures::StreamExt;

        let manager = Manager::new().await?;
        let adapters = manager.adapters().await?;
        let adapter = adapters.into_iter().next()
            .ok_or(SpaceBudsError::DeviceNotFound)?;

        let mut events = adapter.events().await?;

        adapter.start_scan(ScanFilter::default()).await?;

        let mut found_ids = std::collections::HashSet::new();
        let mut found = Vec::new();
        let deadline = tokio::time::Instant::now() + timeout;

        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                break;
            }

            match tokio::time::timeout(remaining, events.next()).await {
                Ok(Some(CentralEvent::DeviceDiscovered(id)))
                | Ok(Some(CentralEvent::DeviceUpdated(id))) => {
                    if found_ids.contains(&id) {
                        continue;
                    }

                    if let Ok(peripheral) = adapter.peripheral(&id).await {
                        if let Ok(Some(props)) = peripheral.properties().await {
                            let uuids: Vec<String> = props.services.iter()
                                .map(|u| u.to_string().to_lowercase())
                                .collect();

                            if uuids.iter().any(|u| u.contains("ff17") || u.contains("fe2c")) {
                                println!("[BLE] Discovered SpacePods: {} [{}]",
                                         props.local_name.as_deref().unwrap_or("Unknown"),
                                         peripheral.address()
                                );
                                found_ids.insert(id);
                                found.push(peripheral);
                            }
                        }
                    }
                }
                Ok(Some(_)) => continue,
                Ok(None) => break,
                Err(_) => break,
            }
        }

        adapter.stop_scan().await?;
        let cached = adapter.peripherals().await.unwrap_or_default();
        for peripheral in cached {
            let id = peripheral.id();
            if found_ids.contains(&id) {
                continue;
            }
            if peripheral.is_connected().await.unwrap_or(false) {
                if let Ok(Some(props)) = peripheral.properties().await {
                    let uuids: Vec<String> = props.services.iter()
                        .map(|u| u.to_string().to_lowercase())
                        .collect();
                    if uuids.iter().any(|u| u.contains("ff17") || u.contains("fe2c")) {
                        found_ids.insert(id);
                        found.push(peripheral);
                    }
                }
            }
        }

        Ok(found)
    }

    pub async fn find_device_with_retry(timeout: Duration, max_retries: usize) -> Result<Peripheral> {
        for attempt in 1..=max_retries {
            println!("Scan attempt {}/{}...", attempt, max_retries);
            match Self::find_device(timeout).await {
                Ok(device) => return Ok(device),
                Err(e) if attempt < max_retries => {
                    println!("Device not found, retrying...");
                    time::sleep(Duration::from_secs(1)).await;
                }
                Err(e) => return Err(e),
            }
        }
        Err(SpaceBudsError::DeviceNotFound)
    }
}