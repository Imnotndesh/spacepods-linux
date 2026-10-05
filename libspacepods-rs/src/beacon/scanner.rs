//! Event-driven BLE scanner with DeviceBeacon manufacturer data filtering.
//!
//! On Linux/BlueZ, `ScanFilter` service UUIDs are merged across D-Bus clients
//! and can hide devices that only advertise manufacturer data. We therefore
//! scan with an empty filter and post-filter, accepting a device when **any**
//! of the following holds:
//!
//!   1. its manufacturer data parses as an Oraimo `DeviceBeacon`,
//!   2. it advertises the Oraimo company ID (`0x4301`) or the `ff17`/`fe2c`
//!      service UUIDs,
//!   3. its local name looks like an Oraimo product.
//!
//! The previous implementation only listened for
//! `CentralEvent::ManufacturerDataAdvertisement` under a service-UUID filter,
//! which silently dropped every device whose beacon arrived under a different
//! company ID or which only advertised service data.

use crate::beacon::{looks_like_oraimo_name, DeviceBeacon};
use crate::{Error, Result};
use btleplug::api::{Central, CentralEvent, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::{Manager, PeripheralId};
use futures::stream::StreamExt;
use std::collections::HashMap;
use std::time::Duration;
use tokio::time;

/// A discovered SpaceBuds device.
///
/// `beacon` is `None` for devices that matched by service UUID or local name but
/// did not expose a parseable proprietary beacon; `product_name()` callers
/// should fall back to `local_name` in that case.
#[derive(Debug, Clone)]
pub struct DiscoveredDevice {
    pub beacon: Option<DeviceBeacon>,
    pub local_name: Option<String>,
    pub ble_address: Option<String>,
    pub rssi: Option<i16>,
    pub peripheral_id: PeripheralId,
}

impl DiscoveredDevice {
    fn anonymous(id: PeripheralId) -> Self {
        Self {
            beacon: None,
            local_name: None,
            ble_address: None,
            rssi: None,
            peripheral_id: id,
        }
    }
}

/// True when a service UUID is the SpacePods GATT service (`ff17`) or Fast Pair
/// (`fe2c`).
fn is_oraimo_service(uuid: &uuid::Uuid) -> bool {
    let s = uuid.to_string().to_lowercase();
    s.contains("ff17") || s.contains("fe2c")
}

/// Scan for SpaceBuds using event-driven discovery + DeviceBeacon post-filtering.
pub async fn scan_for_devices(duration: Duration) -> Result<Vec<DiscoveredDevice>> {
    let manager = Manager::new().await?;
    let adapters = manager.adapters().await?;
    let adapter = adapters
        .into_iter()
        .next()
        .ok_or(Error::DeviceNotFound { retries: 0 })?;

    // Empty filter: some firmware only advertises manufacturer data, which a
    // service-UUID discovery filter can hide entirely.
    adapter.start_scan(ScanFilter::default()).await?;
    let mut events = adapter.events().await?;
    let mut found: HashMap<PeripheralId, DiscoveredDevice> = HashMap::new();
    let deadline = time::sleep(duration);
    tokio::pin!(deadline);

    loop {
        tokio::select! {
            event = events.next() => {
                match event {
                    Some(CentralEvent::ManufacturerDataAdvertisement { id, manufacturer_data }) => {
                        for (mfg_id, data) in &manufacturer_data {
                            if crate::beacon::is_oraimo_manufacturer_data(*mfg_id, data) {
                                if let Some(beacon) = DeviceBeacon::from_manufacturer_data(data) {
                                    let entry = found
                                        .entry(id.clone())
                                        .or_insert_with(|| DiscoveredDevice::anonymous(id.clone()));
                                    entry.beacon = Some(beacon);
                                }
                            }
                        }
                    }
                    Some(CentralEvent::ServiceDataAdvertisement { id, service_data }) => {
                        if service_data.keys().any(is_oraimo_service) {
                            found
                                .entry(id.clone())
                                .or_insert_with(|| DiscoveredDevice::anonymous(id.clone()));
                        }
                    }
                    Some(CentralEvent::ServicesAdvertisement { id, services }) => {
                        if services.iter().any(is_oraimo_service) {
                            found
                                .entry(id.clone())
                                .or_insert_with(|| DiscoveredDevice::anonymous(id.clone()));
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

    // Enrich candidates with advertised properties and pick up name-only
    // matches the event stream may have missed.
    let peripherals = adapter.peripherals().await?;
    for p in peripherals {
        let id = p.id();
        let Ok(Some(props)) = p.properties().await else {
            continue;
        };

        let beacon = props.manufacturer_data.iter().find_map(|(id, data)| {
            if crate::beacon::is_oraimo_manufacturer_data(*id, data) {
                DeviceBeacon::from_manufacturer_data(data)
            } else {
                None
            }
        });

        let name_match = props
            .local_name
            .as_deref()
            .map(looks_like_oraimo_name)
            .unwrap_or(false);
        let service_match = props.services.iter().any(is_oraimo_service)
            || props.service_data.keys().any(is_oraimo_service);

        if beacon.is_none() && !name_match && !service_match {
            continue;
        }

        let entry = found
            .entry(id.clone())
            .or_insert_with(|| DiscoveredDevice::anonymous(id));
        if beacon.is_some() {
            entry.beacon = beacon;
        }
        if entry.local_name.is_none() {
            entry.local_name = props.local_name.clone();
        }
        entry.ble_address = Some(p.address().to_string());
        entry.rssi = props.rssi;
    }

    // Merge duplicate advertisements for the same physical device. BlueZ can
    // expose the identity (public) address and a resolvable private address as
    // separate peripherals; the beacon de-obfuscates to the public MAC, so use
    // it as the canonical key and prefer the entry that carries a beacon.
    let mut merged: Vec<DiscoveredDevice> = Vec::new();
    for device in found.into_values() {
        if let Some(key) = canonical_mac(&device) {
            if let Some(existing) = merged
                .iter_mut()
                .find(|e| canonical_mac(e).as_deref() == Some(key.as_str()))
            {
                if existing.beacon.is_none() && device.beacon.is_some() {
                    existing.beacon = device.beacon.clone();
                    existing.ble_address = device.ble_address.clone();
                }
                if existing.local_name.is_none() {
                    existing.local_name = device.local_name.clone();
                }
                if existing.rssi.is_none() {
                    existing.rssi = device.rssi;
                }
                continue;
            }
        }
        merged.push(device);
    }

    Ok(merged)
}

/// Stable identity used to de-duplicate advertisements: the beacon's
/// de-obfuscated public MAC when available, otherwise the BLE address.
fn canonical_mac(device: &DiscoveredDevice) -> Option<String> {
    device
        .beacon
        .as_ref()
        .and_then(|b| b.bt_address_string())
        .or_else(|| device.ble_address.clone())
        .map(|s| s.to_ascii_uppercase())
}

/// Find the first SpaceBuds device.
pub async fn find_first_device(duration: Duration) -> Result<Option<DiscoveredDevice>> {
    let mut devices = scan_for_devices(duration).await?;
    Ok(if devices.is_empty() {
        None
    } else {
        Some(devices.remove(0))
    })
}
