use crate::{Error, Result};
use btleplug::api::{Central, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::{Manager, Peripheral};
use std::time::Duration;
use tokio::time;

/// Scans for SpaceBuds/SpacePods BLE devices.
pub struct DeviceScanner;

impl DeviceScanner {
    /// Find the first SpaceBuds device by service UUID.
    pub async fn find_device(timeout: Duration) -> Result<Peripheral> {
        let manager = Manager::new().await?;
        let adapters = manager.adapters().await?;
        let adapter = adapters.into_iter().next().ok_or(Error::DeviceNotFound { retries: 0 })?;

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

        Err(Error::DeviceNotFound { retries: 0 })
    }

    /// Scan and return all SpaceBuds-compatible devices.
    pub async fn scan_devices(timeout: Duration) -> Result<Vec<Peripheral>> {
        let manager = Manager::new().await?;
        let adapters = manager.adapters().await?;
        let adapter = adapters.into_iter().next().ok_or(Error::DeviceNotFound { retries: 0 })?;

        adapter.start_scan(ScanFilter::default()).await?;
        time::sleep(timeout).await;

        let peripherals = adapter.peripherals().await?;
        let mut found = Vec::new();

        for peripheral in peripherals {
            if let Ok(Some(properties)) = peripheral.properties().await {
                for uuid in &properties.services {
                    let uuid_str = uuid.to_string().to_lowercase();
                    if uuid_str.contains("ff17") || uuid_str.contains("fe2c") {
                        found.push(peripheral);
                        break;
                    }
                }
            }
        }

        Ok(found)
    }

    /// Scan with retry logic.
    pub async fn find_device_with_retry(timeout: Duration, max_retries: usize) -> Result<Peripheral> {
        for attempt in 1..=max_retries {
            println!("Scan attempt {}/{}...", attempt, max_retries);
            match Self::find_device(timeout).await {
                Ok(device) => return Ok(device),
                Err(e) if attempt < max_retries => {
                    println!("Device not found, retrying...");
                    time::sleep(Duration::from_secs(2)).await;
                }
                Err(_) => return Err(Error::DeviceNotFound { retries: max_retries }),
            }
        }
        Err(Error::DeviceNotFound { retries: max_retries })
    }
}
