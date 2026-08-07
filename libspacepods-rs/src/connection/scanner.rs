use crate::ipc::protocol::ScannedDevice;
use crate::{Error, Result};
use btleplug::api::{Central, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::{Manager, Peripheral};
use std::time::Duration;
use tokio::time;

/// Scans for SpaceBuds/SpacePods BLE devices.
pub struct DeviceScanner;

impl DeviceScanner {
    const GAP_DEVICE_NAME_UUID: uuid::Uuid =
        uuid::Uuid::from_u128(0x00002a00_0000_1000_8000_00805f9b34fb);

    /// Check if a peripheral advertises a SpaceBuds-compatible service UUID.
    fn is_spacebuds(props: &btleplug::api::PeripheralProperties) -> bool {
        props.services.iter().any(|uuid| {
            let s = uuid.to_string().to_lowercase();
            s.contains("ff17") || s.contains("fe2c")
        })
    }

    /// Get the first BLE adapter.
    async fn first_adapter() -> Result<btleplug::platform::Adapter> {
        let manager = Manager::new().await?;
        let mut adapters = manager.adapters().await?;
        adapters.pop().ok_or(Error::DeviceNotFound { retries: 0 })
    }

    /// Resolve a peripheral's name — tries local_name first, then reads GAP name.
    async fn resolve_device_name(peripheral: &Peripheral) -> String {
        // Strategy 1: local name from advertisement
        if let Ok(Some(props)) = peripheral.properties().await {
            if let Some(name) = props.local_name {
                if !name.is_empty() {
                    return name;
                }
            }
        }

        // Strategy 2: brief connect + read GAP Device Name characteristic
        let was_connected = peripheral.is_connected().await.unwrap_or(false);
        if !was_connected {
            let _ = peripheral.connect().await;
            let _ = peripheral.discover_services().await;
            time::sleep(Duration::from_millis(300)).await;
        }

        let name = {
            let mut result = peripheral.address().to_string();
            for char in peripheral.characteristics() {
                if char.uuid == Self::GAP_DEVICE_NAME_UUID {
                    if let Ok(value) = peripheral.read(&char).await {
                        if let Ok(s) = String::from_utf8(value) {
                            let trimmed = s.trim_matches('\0').to_string();
                            if !trimmed.is_empty() {
                                result = trimmed;
                            }
                        }
                    }
                    break;
                }
            }
            result
        };

        if !was_connected {
            let _ = peripheral.disconnect().await;
        }

        name
    }

    /// Find the first SpaceBuds device by service UUID.
    pub async fn find_device(timeout: Duration) -> Result<Peripheral> {
        let adapter = Self::first_adapter().await?;

        adapter.start_scan(ScanFilter::default()).await?;
        time::sleep(timeout).await;

        let peripherals = adapter.peripherals().await?;

        for peripheral in peripherals {
            if let Ok(Some(props)) = peripheral.properties().await {
                if Self::is_spacebuds(&props) {
                    return Ok(peripheral);
                }
            }
        }

        Err(Error::DeviceNotFound { retries: 0 })
    }

    /// Scan and return all SpaceBuds-compatible devices with resolved names.
    pub async fn scan_devices(timeout: Duration) -> Result<Vec<ScannedDevice>> {
        let adapter = Self::first_adapter().await?;

        adapter.start_scan(ScanFilter::default()).await?;
        time::sleep(timeout).await;

        let peripherals = adapter.peripherals().await?;

        // Collect compatible peripherals
        let mut found: Vec<Peripheral> = Vec::new();
        for peripheral in peripherals {
            if let Ok(Some(props)) = peripheral.properties().await {
                if Self::is_spacebuds(&props) {
                    found.push(peripheral);
                }
            }
        }

        // Give a moment for late-arriving advertisement names
        // (Check resolved names asynchronously instead of in a sync closure)
        let mut needs_wait = false;
        for p in &found {
            if let Ok(Some(props)) = p.properties().await {
                if props.local_name.map_or(true, |n| n.is_empty()) {
                    needs_wait = true;
                    break;
                }
            }
        }
        if needs_wait {
            time::sleep(Duration::from_millis(500)).await;
        }

        let mut devices = Vec::with_capacity(found.len());
        for p in found {
            let name = Self::resolve_device_name(&p).await;
            let address = p.address().to_string();
            devices.push(ScannedDevice {
                name,
                address,
                product_name: None,
                product_id: None,
                real_mac: None,
                beacon_version: None,
                battery_left: None,
                battery_right: None,
                battery_case: None,
                already_connected: false,
                rssi: None,
            });
        }

        Ok(devices)
    }

    /// Scan with retry logic, returning the first found peripheral.
    pub async fn find_device_with_retry(
        timeout: Duration,
        max_retries: usize,
    ) -> Result<Peripheral> {
        for attempt in 1..=max_retries {
            eprintln!("Scan attempt {}/{}...", attempt, max_retries);
            match Self::find_device(timeout).await {
                Ok(device) => return Ok(device),
                Err(_) if attempt < max_retries => {
                    eprintln!("Device not found, retrying...");
                    time::sleep(Duration::from_secs(2)).await;
                }
                Err(_) => return Err(Error::DeviceNotFound { retries: max_retries }),
            }
        }
        Err(Error::DeviceNotFound { retries: max_retries })
    }
}
