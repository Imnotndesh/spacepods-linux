//! Scan PoC — test binary that demonstrates the new event-driven scanner
//! with DeviceBeacon manufacturer data filtering.
//!
//! Usage:
//!   cargo run --bin spacepods-scan
//!
//! This binary does:
//!   1. Opens the BLE adapter
//!   2. Starts scanning with service UUID filter
//!   3. Listens for ManufacturerDataAdvertisement events
//!   4. Parses each advertisement through DeviceBeacon
//!   5. Prints discovered devices with their beacon info

use btleplug::api::{Central, CentralEvent, Manager as _, Peripheral as _, ScanFilter};
use btleplug::platform::Manager;
use futures::stream::StreamExt;
use std::time::Duration;
use tokio::time;

// We inline a minimal version of the beacon parser to avoid crate dependency issues
mod beacon {
    /// Result of parsing a DeviceBeacon from manufacturer data.
    #[derive(Debug, Clone)]
    pub struct DeviceBeacon {
        pub version: u8,
        pub product_id: u16,
        pub bt_address: Option<[u8; 6]>,
        pub connected: bool,
        pub need_auth: bool,
        pub use_custom_spp_uuid: bool,
        pub brand_id: Option<u32>,
        pub left_battery: Option<u8>,
        pub right_battery: Option<u8>,
        pub case_battery: Option<u8>,
        pub left_charging: Option<bool>,
        pub right_charging: Option<bool>,
        pub case_charging: Option<bool>,
    }

    impl DeviceBeacon {
        pub fn from_manufacturer_data(data: &[u8]) -> Option<Self> {
            if data.is_empty() {
                return None;
            }
            let len = data.len();
            match len {
                8 => Self::parse_v3(data),
                9 | 13 | 27 => {
                    let version = data[0] & 0x0F;
                    match version {
                        1 => {
                            if len == 27 || len == 9 {
                                Self::parse_v4(data)
                            } else {
                                Self::parse_v1(data)
                            }
                        }
                        2 => Self::parse_v2(data),
                        _ => Self::parse_v4(data),
                    }
                }
                _ => None,
            }
        }

        fn parse_v1(data: &[u8]) -> Option<Self> {
            if data.len() < 13 { return None; }
            let product_id = u16::from_le_bytes([data[1], data[2]]);
            let mut bt_addr = [0u8; 6];
            bt_addr.copy_from_slice(&data[3..9]);
            let flags = data.get(9).copied().unwrap_or(0);
            let need_auth = (flags & 0x01) != 0;
            let connected = ((flags >> 2) & 0x03) == 1;
            let left_raw = data.get(10).copied().unwrap_or(0);
            let left_charging = (left_raw & 0x80) != 0;
            let left_battery = left_raw & 0x7F;
            let right_raw = data.get(11).copied().unwrap_or(0);
            let right_charging = (right_raw & 0x80) != 0;
            let right_battery = right_raw & 0x7F;
            let case_raw = data.get(12).copied().unwrap_or(0);
            let case_charging = (case_raw & 0x80) != 0;
            let case_battery = case_raw & 0x7F;

            Some(Self {
                version: 1, product_id, bt_address: Some(bt_addr),
                connected, need_auth, use_custom_spp_uuid: false, brand_id: None,
                left_battery: Some(left_battery), right_battery: Some(right_battery),
                case_battery: Some(case_battery),
                left_charging: Some(left_charging), right_charging: Some(right_charging),
                case_charging: Some(case_charging),
            })
        }

        fn parse_v2(data: &[u8]) -> Option<Self> {
            if data.len() < 13 { return None; }
            let product_id = u16::from_le_bytes([data[1], data[2]]);
            let mut bt_addr = [0u8; 6];
            bt_addr.copy_from_slice(&data[3..9]);
            xor_bytes(&mut bt_addr, 0xAD);
            let flags = data.get(9).copied().unwrap_or(0);
            let need_auth = (flags & 0x01) != 0;
            let connected = ((flags >> 2) & 0x03) == 1;
            let use_custom_spp_uuid = ((flags >> 4) & 0x01) != 0;
            let brand_id = if data.len() >= 13 {
                Some(u32::from_le_bytes([data[10], data[11], data[12], 0]))
            } else { None };

            Some(Self {
                version: 2, product_id, bt_address: Some(bt_addr),
                connected, need_auth, use_custom_spp_uuid, brand_id,
                left_battery: None, right_battery: None, case_battery: None,
                left_charging: None, right_charging: None, case_charging: None,
            })
        }

        fn parse_v3(data: &[u8]) -> Option<Self> {
            if data.len() < 8 { return None; }
            let product_id = u16::from_le_bytes([data[0], data[1]]);
            let mut bt_addr = [0u8; 6];
            bt_addr.copy_from_slice(&data[2..8]);
            xor_bytes(&mut bt_addr, 0xAD);
            Some(Self {
                version: 3, product_id, bt_address: Some(bt_addr),
                connected: false, need_auth: false, use_custom_spp_uuid: false,
                brand_id: None,
                left_battery: None, right_battery: None, case_battery: None,
                left_charging: None, right_charging: None, case_charging: None,
            })
        }

        fn parse_v4(data: &[u8]) -> Option<Self> {
            if data.len() < 9 { return None; }
            let product_id = u16::from_le_bytes([data[1], data[2]]);
            let mut bt_addr = [0u8; 6];
            let copy_len = (data.len() - 3).min(6);
            bt_addr[..copy_len].copy_from_slice(&data[3..3 + copy_len]);
            xor_bytes(&mut bt_addr, 0xAD);
            let is_special = matches!(product_id, 19|21|25|32|33|34|20|6|12288|12289|17);
            let connected = if is_special && data[0] > 1 {
                true
            } else {
                let flags = if data.len() > 9 { data[9] } else { data[0] };
                ((flags >> 2) & 0x03) == 1
            };
            Some(Self {
                version: 4, product_id, bt_address: Some(bt_addr),
                connected, need_auth: false, use_custom_spp_uuid: false, brand_id: None,
                left_battery: None, right_battery: None, case_battery: None,
                left_charging: None, right_charging: None, case_charging: None,
            })
        }

        pub fn bt_address_string(&self) -> Option<String> {
            self.bt_address.map(|addr| {
                format!("{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                    addr[0], addr[1], addr[2], addr[3], addr[4], addr[5])
            })
        }

        /// Human-readable product name for known product IDs
        pub fn product_name(&self) -> &'static str {
            match self.product_id {
                64 => "SpaceBuds Neo 2 (321)",
                65 => "SpaceBuds 2 (631 Star)",
                4100 => "OpenSnap Hayato (371 FF)",
                4101 => "OPN671",
                55 => "627S",
                _ => "Unknown Oraimo Device",
            }
        }
    }

    fn xor_bytes(data: &mut [u8], key: u8) {
        for b in data.iter_mut() { *b ^= key; }
    }
}

const UUID_FF17: uuid::Uuid = uuid::Uuid::from_u128(0x0000ff17_0000_1000_8000_00805f9b34fb);
const UUID_FE2C: uuid::Uuid = uuid::Uuid::from_u128(0x0000fe2c_0000_1000_8000_00805f9b34fb);

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== SpacePods Scanner PoC ===\n");
    println!("This scans for SpaceBuds/Oraimo devices using:");
    println!("  1. Service UUID filter (0xFF17 + 0xFE2C)");
    println!("  2. Manufacturer data (DeviceBeacon) post-filtering");
    println!();

    let manager = Manager::new().await?;
    let adapters = manager.adapters().await?;

    if adapters.is_empty() {
        eprintln!("ERROR: No Bluetooth adapters found!");
        return Ok(());
    }

    let adapter = adapters.into_iter().next().unwrap();
    println!("Adapter: {:?}", adapter.adapter_info().await?);
    println!();

    // Start scan with service UUID filter
    let filter = ScanFilter {
        services: vec![UUID_FF17, UUID_FE2C],
    };

    println!("Starting scan (Ctrl+C to stop)...");
    adapter.start_scan(filter).await?;

    let mut events = adapter.events().await?;
    let mut scan_count = 0u64;
    let mut mfg_events = 0u64;
    let mut beacon_hits = 0u64;

    println!("Waiting for events...\n");

    // Scan for up to 30 seconds
    let deadline = time::sleep(Duration::from_secs(30));
    tokio::pin!(deadline);

    loop {
        tokio::select! {
            event = events.next() => {
                match event {
                    Some(CentralEvent::DeviceDiscovered(id)) => {
                        scan_count += 1;
                        // Don't print every single device — too noisy
                        if scan_count <= 5 || scan_count % 20 == 0 {
                            println!("[SCAN] DeviceDiscovered #{}: {:?}", scan_count, id);
                            if scan_count == 5 && scan_count < 20 {
                                println!("[SCAN] ... (suppressing further DeviceDiscovered, will show manufacturer data hits)");
                            }
                        }
                    }
                    Some(CentralEvent::ManufacturerDataAdvertisement { id, manufacturer_data }) => {
                        mfg_events += 1;
                        for (mfg_id, data) in &manufacturer_data {
                            if let Some(beacon) = beacon::DeviceBeacon::from_manufacturer_data(data) {
                                beacon_hits += 1;
                                println!();
                                println!("-------------------------------------------");
                                println!("  [FOUND] SPACEBUDS DEVICE");
                                println!("-------------------------------------------");
                                println!("  Product:       {} (ID={})", beacon.product_name(), beacon.product_id);
                                println!("  Beacon version: V{}", beacon.version);
                                if let Some(addr) = beacon.bt_address_string() {
                                    println!("  Real BT Addr:  {}", addr);
                                }
                                println!("  Connected:     {}", if beacon.connected { "YES (to another phone)" } else { "NO (available)" });
                                println!("  Need auth:     {}", beacon.need_auth);
                                println!("  Custom SPP:    {}", beacon.use_custom_spp_uuid);
                                if let Some(bid) = beacon.brand_id {
                                    println!("  Brand ID:      0x{:06X}", bid);
                                }
                                if let Some(lb) = beacon.left_battery {
                                    println!("  Left batt:     {}% {}", lb, if beacon.left_charging.unwrap_or(false) { "(charging)" } else { "" });
                                }
                                if let Some(rb) = beacon.right_battery {
                                    println!("  Right batt:    {}% {}", rb, if beacon.right_charging.unwrap_or(false) { "(charging)" } else { "" });
                                }
                                if let Some(cb) = beacon.case_battery {
                                    println!("  Case batt:     {}% {}", cb, if beacon.case_charging.unwrap_or(false) { "(charging)" } else { "" });
                                }
                                println!("  Manufacturer ID: 0x{:04X}", mfg_id);
                                println!("  Raw data:      {} bytes: {:02X?}", data.len(), &data[..data.len().min(27)]);
                                println!("-------------------------------------------");
                                println!();
                            }
                        }
                    }
                    Some(CentralEvent::DeviceUpdated(id)) => {
                        // Quiet
                        let _ = id;
                    }
                    None => {
                        println!("Event stream ended.");
                        break;
                    }
                    _ => {}
                }
            }
            _ = &mut deadline => {
                println!("\n--- 30s scan timeout reached ---");
                break;
            }
            _ = tokio::signal::ctrl_c() => {
                println!("\n--- Ctrl+C, stopping scan ---");
                break;
            }
        }
    }

    let _ = adapter.stop_scan().await;

    // Also try the old poll-based approach for comparison
    println!();
    println!("=== Post-scan poll (old approach) ===");
    let peripherals = adapter.peripherals().await?;
    println!("Total peripherals in adapter cache: {}", peripherals.len());

    let mut spacebuds_count = 0;
    for p in &peripherals {
        if let Ok(Some(props)) = p.properties().await {
            // Check manufacturer data
            let mut found_via_beacon = false;
            let mfg_map = &props.manufacturer_data;
            if !mfg_map.is_empty() {
                for (_mfg_id, data) in mfg_map {
                    if let Some(beacon) = beacon::DeviceBeacon::from_manufacturer_data(data) {
                        if !found_via_beacon {
                            spacebuds_count += 1;
                            found_via_beacon = true;
                            println!("  * {} — {} (beacon V{})",
                                props.local_name.as_deref().unwrap_or("unnamed"),
                                beacon.product_name(),
                                beacon.version,
                            );
                        }
                    }
                }
            }

            // Also check services as fallback
            if !found_via_beacon {
                for uuid in &props.services {
                    let s = uuid.to_string().to_lowercase();
                    if s.contains("ff17") || s.contains("fe2c") {
                        spacebuds_count += 1;
                        println!("  WARNING: {} — matched via service UUID (NO beacon data!)",
                            props.local_name.as_deref().unwrap_or("unnamed"),
                        );
                        break;
                    }
                }
            }
        }
    }

    if spacebuds_count == 0 {
        println!("  [FAIL] No SpaceBuds devices found.");
        println!();
        println!("  Troubleshooting:");
        println!("    1. Make sure your SpaceBuds are in pairing mode (LED flashing)");
        println!("    2. Make sure they're not already connected to your phone");
        println!("    3. Try restarting Bluetooth: sudo systemctl restart bluetooth");
        println!("    4. Make sure BlueZ is recent: bluetoothctl --version");
        println!();
        println!("  Raw device list (first 10 peripherals):");
        for (i, p) in peripherals.iter().take(10).enumerate() {
            if let Ok(Some(props)) = p.properties().await {
                println!("    [{}] {:?} — services: {:?}, mfg_data: {:?}",
                    i,
                    props.local_name.as_deref().unwrap_or("unnamed"),
                    props.services.iter().map(|u| u.to_string()).collect::<Vec<_>>(),
                    props.manufacturer_data.keys().collect::<Vec<_>>(),
                );
            }
        }
    }

    println!();
    println!("=== Summary ===");
    println!("  DeviceDiscovered events:  {}", scan_count);
    println!("  ManufacturerData events:  {}", mfg_events);
    println!("  Beacon hits:              {}", beacon_hits);
    println!("  SpaceBuds in cache:       {}", spacebuds_count);
    println!("  Total cached peripherals: {}", peripherals.len());

    Ok(())
}
