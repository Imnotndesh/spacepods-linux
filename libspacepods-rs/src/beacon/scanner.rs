//! Event-driven BLE scanner with DeviceBeacon manufacturer data filtering.
//!
//! On Linux/BlueZ, `ScanFilter` service UUIDs are merged across D-Bus clients,
//! so we post-filter using `DeviceBeacon`. Only genuine Oraimo/SpaceBuds devices
//! produce the proprietary beacon format in their manufacturer data.

use crate::{Error, Result};
use crate::beacon::DeviceBeacon;
use btleplug::api::{Central, CentralEvent, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::{Manager, PeripheralId};
use futures::stream::StreamExt;
use std::collections::HashMap;
use std::time::Duration;
use tokio::time;

/// A discovered SpaceBuds device.
#[derive(Debug, Clone)]
pub struct DiscoveredDevice {
    pub beacon: DeviceBeacon,
    pub local_name: Option<String>,
    pub ble_address: Option<String>,
    pub rssi: Option<i16>,
    pub peripheral_id: PeripheralId,
}

/// Scan for SpaceBuds using event-driven discovery + DeviceBeacon post-filtering.
///
/// Returns all devices whose manufacturer data parses as a valid DeviceBeacon.
pub async fn scan_for_devices(duration: Duration) -> Result<Vec<DiscoveredDevice>> {
    let manager = Manager::new().await?;
    let adapters = manager.adapters().await?;
    let adapter = adapters
        .into_iter()
        .next()
        .ok_or(Error::DeviceNotFound { retries: 0 })?;

    let filter = ScanFilter {
        services: vec![
            crate::protocol::UUID_FF17,
            crate::protocol::UUID_FE2C,
        ],
    };

    adapter.start_scan(filter).await?;
    let mut events = adapter.events().await?;
    let mut found: HashMap<PeripheralId, DiscoveredDevice> = HashMap::new();
    let deadline = time::sleep(duration);
    tokio::pin!(deadline);

    loop {
        tokio::select! {
            event = events.next() => {
                match event {
                    Some(CentralEvent::ManufacturerDataAdvertisement { id, manufacturer_data }) => {
                        if !found.contains_key(&id) {
                            for (_mfg_id, data) in &manufacturer_data {
                                if let Some(beacon) = DeviceBeacon::from_manufacturer_data(data) {
                                    let device = DiscoveredDevice {
                                        beacon,
                                        local_name: None,
                                        ble_address: None,
                                        rssi: None,
                                        peripheral_id: id.clone(),
                                    };
                                    found.insert(id, device);
                                    break;
                                }
                            }
                        }
                    }
                    None => break,
                    _ => {}
                }
            }
            _ = &mut deadline => { break; }
        }
    }

    let _ = adapter.stop_scan().await;

    // Enrich with peripheral properties
    let peripherals = adapter.peripherals().await?;
    for p in &peripherals {
        if let Ok(Some(props)) = p.properties().await {
            // Check if this peripheral matches a device we found via beacon
            let mut matched = false;
            let mfg_map = &props.manufacturer_data;
            if !mfg_map.is_empty() {
                for (_mfg_id, data) in mfg_map {
                    if DeviceBeacon::from_manufacturer_data(data).is_some() {
                        matched = true;
                        break;
                    }
                }
            }
            if matched {
                let addr = p.address().to_string();
                for (_mfg_id, data) in &props.manufacturer_data {
                    if let Some(beacon) = DeviceBeacon::from_manufacturer_data(data) {
                        let real_addr = beacon.bt_address_string().unwrap_or_default();
                        for device in found.values_mut() {
                            if device.beacon.bt_address_string().as_deref() == Some(&real_addr) {
                                device.local_name = props.local_name.clone();
                                device.ble_address = Some(addr.clone());
                                device.rssi = props.rssi;
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(found.into_values().collect())
}

/// Find the first SpaceBuds device.
pub async fn find_first_device(duration: Duration) -> Result<Option<DiscoveredDevice>> {
    let mut devices = scan_for_devices(duration).await?;
    Ok(if devices.is_empty() { None } else { Some(devices.remove(0)) })
}
