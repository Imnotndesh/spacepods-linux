//! DeviceBeacon — parses Oraimo/SpaceBuds manufacturer-specific BLE advertisement data.
//!
//! Ported from the decompiled Oraimo Sound APK:
//!   com/bluetrum/devicemanager/models/DeviceBeacon.java
//!   com/bluetrum/devicemanager/models/DeviceBeaconV1.java
//!   com/bluetrum/devicemanager/models/DeviceBeaconV2.java
//!   com/bluetrum/devicemanager/models/DeviceBeaconV3.java
//!   com/bluetrum/devicemanager/models/DeviceBeaconV4.java
//!
//! ## Beacon format
//!
//! Oraimo/SpaceBuds devices embed a proprietary beacon inside the BLE Manufacturer Specific
//! Data field (AD type 0xFF). The beacon encodes:
//!   - Beacon version (1-4)
//!   - Product ID (16-bit, little-endian)
//!   - Bluetooth MAC address (6 bytes, XOR-obfuscated with 0xAD in V2/V3/V4)
//!   - Connection state, auth flag, custom SPP UUID flag
//!   - Battery levels and charging status (V1 only)
//!   - Brand ID (V2)
//!
//! ## Beacon versions
//!
//! | Version | Data length | Notes |
//! |---------|------------|-------|
//! | V1      | 13 bytes   | Includes battery levels + charging |
//! | V2      | 13 bytes   | Includes brand ID, MAC XOR'd |
//! | V3      | 8 bytes    | Minimal, MAC XOR'd, no battery |
//! | V4      | 27/13/9    | Extended, MAC XOR'd |
//!
//! The beacon version is encoded differently per length:
//!   - 8 bytes  → V3
//!   - 13 bytes → data[0] & 0x0F (1 = V1 or V4, 2 = V2)
//!   - 27 bytes → data[0] & 0x0F (>=0)
//!   - 9 bytes  → data[0] & 0x0F (1 = V4)

pub mod scanner;

/// Known Oraimo manufacturer IDs observed in the wild.
/// The APK doesn't hardcode a single manufacturer ID — it just checks if the data parses
/// as a valid DeviceBeacon. But for scan filtering we may want these.
pub const ORAIMO_MANUFACTURER_IDS: &[u16] = &[
    0x00E0, // Google (some chips use this)
    // The bluetrum SDK devices often use custom manufacturer IDs.
    // We'll discover the actual ID dynamically.
];

/// Result of parsing a DeviceBeacon from manufacturer data.
#[derive(Debug, Clone)]
pub struct DeviceBeacon {
    /// Beacon format version (1-4)
    pub version: u8,
    /// Product ID (e.g., 64 = SpaceBuds Neo 2, 65 = SpaceBuds 2)
    pub product_id: u16,
    /// Bluetooth MAC address (de-obfuscated)
    pub bt_address: Option<[u8; 6]>,
    /// Whether the device is currently connected (to a phone)
    pub connected: bool,
    /// Whether authentication is required
    pub need_auth: bool,
    /// Whether the device uses a custom SPP UUID
    pub use_custom_spp_uuid: bool,
    /// Brand ID (V2 only)
    pub brand_id: Option<u32>,
    /// Left bud battery level 0-100 (V1 only)
    pub left_battery: Option<u8>,
    /// Right bud battery level 0-100 (V1 only)
    pub right_battery: Option<u8>,
    /// Case battery level 0-100 (V1 only)
    pub case_battery: Option<u8>,
    /// Left bud charging (V1 only)
    pub left_charging: Option<bool>,
    /// Right bud charging (V1 only)
    pub right_charging: Option<bool>,
    /// Case charging (V1 only)
    pub case_charging: Option<bool>,
}

impl DeviceBeacon {
    /// Try to parse a DeviceBeacon from manufacturer-specific data bytes.
    ///
    /// Returns `None` if the data doesn't look like a valid Oraimo beacon.
    pub fn from_manufacturer_data(data: &[u8]) -> Option<Self> {
        if data.is_empty() {
            return None;
        }

        let len = data.len();

        // Determine version and parse
        match len {
            8 => {
                // V3: 8 bytes, version is implicitly 1
                Self::parse_v3(data)
            }
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
                    _ => Self::parse_v4(data), // fallback
                }
            }
            _ => None,
        }
    }

    /// V1: 13 bytes. Includes battery levels.
    /// Layout (after product_id bytes consumed by parent DeviceBeacon):
    ///   [bt_addr:6][flags:1][left_batt:1][right_batt:1][case_batt:1]
    fn parse_v1(data: &[u8]) -> Option<Self> {
        if data.len() < 13 {
            return None;
        }

        // Product ID is at bytes 1-2 (little-endian), version nibble is in byte 0
        let product_id = u16::from_le_bytes([data[1], data[2]]);

        // Bytes 3-8: BT address (NOT XOR'd in V1)
        let mut bt_addr = [0u8; 6];
        bt_addr.copy_from_slice(&data[3..9]);

        // Byte 9: flags
        let flags = data.get(9).copied().unwrap_or(0);
        let need_auth = (flags & 0x01) != 0;
        let connected = ((flags >> 2) & 0x03) == 1;

        // Byte 10: left battery + charging
        let left_raw = data.get(10).copied().unwrap_or(0);
        let left_charging = (left_raw & 0x80) != 0;
        let left_battery = left_raw & 0x7F;

        // Byte 11: right battery + charging
        let right_raw = data.get(11).copied().unwrap_or(0);
        let right_charging = (right_raw & 0x80) != 0;
        let right_battery = right_raw & 0x7F;

        // Byte 12: case battery + charging
        let case_raw = data.get(12).copied().unwrap_or(0);
        let case_charging = (case_raw & 0x80) != 0;
        let case_battery = case_raw & 0x7F;

        Some(Self {
            version: 1,
            product_id,
            bt_address: Some(bt_addr),
            connected,
            need_auth,
            use_custom_spp_uuid: false,
            brand_id: None,
            left_battery: Some(left_battery),
            right_battery: Some(right_battery),
            case_battery: Some(case_battery),
            left_charging: Some(left_charging),
            right_charging: Some(right_charging),
            case_charging: Some(case_charging),
        })
    }

    /// V2: 13 bytes. Includes brand ID. MAC is XOR'd.
    /// Layout: [bt_addr_xor:6][flags:1][brand_id:3]
    fn parse_v2(data: &[u8]) -> Option<Self> {
        if data.len() < 13 {
            return None;
        }

        let product_id = u16::from_le_bytes([data[1], data[2]]);

        // Bytes 3-8: XOR'd BT address
        let mut bt_addr = [0u8; 6];
        bt_addr.copy_from_slice(&data[3..9]);
        deobfuscate_bt_address(&mut bt_addr);

        // Byte 9: flags
        let flags = data.get(9).copied().unwrap_or(0);
        let need_auth = (flags & 0x01) != 0;
        let connected = ((flags >> 2) & 0x03) == 1;
        let use_custom_spp_uuid = ((flags >> 4) & 0x01) != 0;

        // Bytes 10-12: brand ID (3 bytes, little-endian)
        let brand_id = if data.len() >= 13 {
            u32::from_le_bytes([data[10], data[11], data[12], 0])
        } else {
            0
        };

        Some(Self {
            version: 2,
            product_id,
            bt_address: Some(bt_addr),
            connected,
            need_auth,
            use_custom_spp_uuid,
            brand_id: Some(brand_id),
            left_battery: None,
            right_battery: None,
            case_battery: None,
            left_charging: None,
            right_charging: None,
            case_charging: None,
        })
    }

    /// V3: 8 bytes. Minimal beacon. MAC is XOR'd.
    /// Layout: [bt_addr_xor:6]
    fn parse_v3(data: &[u8]) -> Option<Self> {
        if data.len() < 8 {
            return None;
        }

        // For V3, product ID is at offset 0-1 (no version byte consumed)
        let product_id = u16::from_le_bytes([data[0], data[1]]);

        // Bytes 2-7: XOR'd BT address
        let mut bt_addr = [0u8; 6];
        bt_addr.copy_from_slice(&data[2..8]);
        deobfuscate_bt_address(&mut bt_addr);

        Some(Self {
            version: 3,
            product_id,
            bt_address: Some(bt_addr),
            connected: false,
            need_auth: false,
            use_custom_spp_uuid: false,
            brand_id: None,
            left_battery: None,
            right_battery: None,
            case_battery: None,
            left_charging: None,
            right_charging: None,
            case_charging: None,
        })
    }

    /// V4: 27, 13, or 9 bytes. Extended. MAC is XOR'd.
    fn parse_v4(data: &[u8]) -> Option<Self> {
        if data.len() < 9 {
            return None;
        }

        let product_id = u16::from_le_bytes([data[1], data[2]]);

        // Bytes 3-8: XOR'd BT address
        let mut bt_addr = [0u8; 6];
        if data.len() >= 9 {
            let copy_len = (data.len() - 3).min(6);
            bt_addr[..copy_len].copy_from_slice(&data[3..3 + copy_len]);
        }
        deobfuscate_bt_address(&mut bt_addr);

        // Connection state handling from the APK:
        //   - Some product IDs have special handling
        //   - Generally: (byte[0] or byte[9] >> 2) & 3
        let is_special_pid = matches!(
            product_id,
            19 | 21 | 25 | 32 | 33 | 34 | 20 | 6 | 12288 | 12289 | 17
        );

        let connected = if is_special_pid && data[0] > 1 {
            // Use fixed value 1 (connected)
            true
        } else {
            let flags = if data.len() > 9 { data[9] } else { data[0] };
            ((flags >> 2) & 0x03) == 1
        };

        Some(Self {
            version: 4,
            product_id,
            bt_address: Some(bt_addr),
            connected,
            need_auth: false,
            use_custom_spp_uuid: false,
            brand_id: None,
            left_battery: None,
            right_battery: None,
            case_battery: None,
            left_charging: None,
            right_charging: None,
            case_charging: None,
        })
    }

    /// Format the BT address as a colon-separated hex string.
    pub fn bt_address_string(&self) -> Option<String> {
        self.bt_address.map(|addr| {
            format!(
                "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                addr[0], addr[1], addr[2], addr[3], addr[4], addr[5]
            )
        })
    }

    /// Human-readable product name for known product IDs.
    pub fn product_name(&self) -> &'static str {
        match self.product_id {
            64 => "SpaceBuds Neo 2",
            65 => "SpaceBuds 2",
            4100 => "OpenSnap Hayato",
            4101 => "OPN671",
            55 => "627S",
            _ => "Oraimo SpaceBuds",
        }
    }
}

/// De-obfuscate a Bluetooth address by XORing each byte with 0xAD.
///
/// This matches the `deobfuscateBtAddress` method in:
///   DeviceBeaconV2.java, DeviceBeaconV3.java, DeviceBeaconV4.java
fn deobfuscate_bt_address(addr: &mut [u8]) {
    for byte in addr.iter_mut() {
        *byte ^= 0xAD;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deobfuscate() {
        let mut addr = [0x00u8; 6];
        deobfuscate_bt_address(&mut addr);
        // 0x00 ^ 0xAD = 0xAD
        assert_eq!(addr, [0xAD; 6]);

        let mut addr2 = [0xADu8; 6];
        deobfuscate_bt_address(&mut addr2);
        // 0xAD ^ 0xAD = 0x00
        assert_eq!(addr2, [0x00; 6]);
    }

    #[test]
    fn test_v3_parse() {
        // Sample V3 beacon: 8 bytes
        // bytes 0-1: product_id = 64 (SpaceBuds Neo 2) = 0x0040
        // bytes 2-7: xor'd MAC (let's use a dummy)
        let pid: u16 = 64; // SpaceBuds Neo 2
        let mut data = vec![0u8; 8];
        data[0] = pid as u8;
        data[1] = (pid >> 8) as u8;
        // MAC: 11:22:33:44:55:66  XOR'd with 0xAD
        let mac: [u8; 6] = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66];
        for (i, b) in mac.iter().enumerate() {
            data[2 + i] = b ^ 0xAD;
        }

        let beacon = DeviceBeacon::from_manufacturer_data(&data).unwrap();
        assert_eq!(beacon.version, 3);
        assert_eq!(beacon.product_id, 64);
        assert_eq!(beacon.bt_address, Some(mac));
    }

    #[test]
    fn test_v1_parse() {
        // V1: 13 bytes, version=1
        let mut data = vec![0u8; 13];
        data[0] = 0x01; // version 1
        data[1] = 65;   // product_id low
        data[2] = 0;    // product_id high = 65 = SpaceBuds 2
        // MAC not XOR'd in V1
        let mac: [u8; 6] = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];
        data[3..9].copy_from_slice(&mac);
        data[9] = 0x04; // connected (bit 2 set)
        data[10] = 0x50;  // left: not charging, 80%
        data[11] = 0x8F;  // right: charging, 15%
        data[12] = 0x64;  // case: not charging, 100%

        let beacon = DeviceBeacon::from_manufacturer_data(&data).unwrap();
        assert_eq!(beacon.version, 1);
        assert_eq!(beacon.product_id, 65);
        assert_eq!(beacon.bt_address, Some(mac));
        assert!(beacon.connected);
        assert!(!beacon.need_auth);
        assert_eq!(beacon.left_battery, Some(80));
        assert_eq!(beacon.left_charging, Some(false));
        assert_eq!(beacon.right_battery, Some(15));
        assert_eq!(beacon.right_charging, Some(true));
        assert_eq!(beacon.case_battery, Some(100));
        assert_eq!(beacon.case_charging, Some(false));
    }

    #[test]
    fn test_invalid_data() {
        assert!(DeviceBeacon::from_manufacturer_data(&[]).is_none());
        assert!(DeviceBeacon::from_manufacturer_data(&[0x00, 0x01]).is_none());
        // 7 bytes — not a valid length
        assert!(DeviceBeacon::from_manufacturer_data(&[0u8; 7]).is_none());
    }
}
