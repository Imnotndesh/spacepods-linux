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
//! ## Beacon dispatch (ported exactly from `DeviceBeacon.getDeviceBeaconVersion`)
//!
//! The APK short-circuits on length *before* looking at any version nibble:
//!   - 8 bytes  → always V3
//!   - 13 bytes → always V4 (the nibble at `data[0]` is **not** consulted)
//!   - 27 bytes → V2 when `data[0] & 0x0F == 2`, otherwise V4
//!   - 9 bytes  → V4 (the APK additionally requires the nibble to be 1)
//!
//! V1 support is retained for direct callers and older firmware, but the
//! reference implementation never selects it for these advertisement lengths.

pub mod scanner;

/// Oraimo / Transsion Bluetooth SIG company identifier.
///
/// Observed live on an `oraimo SpacePods` advertisement:
/// `ManufacturerData.Key: 0x4301`.
pub const ORAIMO_COMPANY_ID: u16 = 0x4301;

/// Manufacturer IDs that may carry a DeviceBeacon payload.
pub const ORAIMO_MANUFACTURER_IDS: &[u16] = &[
    ORAIMO_COMPANY_ID, // 0x4301 — confirmed on real Oraimo hardware
    0x00E0,            // Google — used by some Bluetrum-based devices
];

/// True when a BLE local name looks like an Oraimo / SpaceBuds product.
///
/// Used as a discovery fallback for firmware that advertises neither a
/// parseable beacon nor the `ff17`/`fe2c` service UUIDs.
pub fn looks_like_oraimo_name(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    const NEEDLES: &[&str] = &[
        "oraimo",
        "spacebuds",
        "spacepods",
        "spacebox",
        "boompod",
        "necklace",
        "freepods",
        "airbuds",
        "opensnap",
        "openarc",
        "openloop",
        "opencirclet",
    ];
    NEEDLES.iter().any(|needle| n.contains(needle))
}

/// Every product ID understood by [`product_name_for_id`].
///
/// Used to build generic-but-correctly-named device profiles for the whole
/// Oraimo line-up without hardcoding the same list twice.
pub const KNOWN_PRODUCT_IDS: &[u16] = &[
    // beacon product IDs
    55, 64, 65, 4100, 4101, 16400,
    // device types
    104, 105, 10510, 108, 300, 3001, 3002, 3003, 310, 311, 316, 317, 321,
    323, 324, 3240, 325, 330, 340, 371, 3711, 372, 373, 374, 610, 611, 6111,
    620, 6201, 626, 630, 6301, 6302, 6303, 631, 6312, 662, 666, 670, 671,
    675, 680, 682, 687, 915, 917, 930, 931, 982, 1059, 3231, 3232, 3233,
    3251, 3252, 3253, 3254, 3300, 3301, 6100, 6311, 57421,
];

/// True when btleplug peripheral properties describe an Oraimo device.
///
/// Combines the `ff17`/`fe2c` service-UUID fast path, the manufacturer
/// beacon / company ID, and a local-name fallback. Kept here so every scanner
/// (daemon, connection manager and legacy paths) uses the same policy.
pub fn is_oraimo_peripheral(props: &btleplug::api::PeripheralProperties) -> bool {
    if props.services.iter().any(|uuid| {
        let s = uuid.to_string().to_lowercase();
        s.contains("ff17") || s.contains("fe2c")
    }) {
        return true;
    }

    if props.manufacturer_data.iter().any(|(id, data)| {
        ORAIMO_MANUFACTURER_IDS.contains(id)
            || DeviceBeacon::from_manufacturer_data(data).is_some()
    }) {
        return true;
    }

    props
        .local_name
        .as_deref()
        .map(looks_like_oraimo_name)
        .unwrap_or(false)
}

/// Human-readable name for a beacon product ID / device type.
///
/// Sourced from the Oraimo Sound APK:
///   - `Product.java` `getProductId()` (the beacon ID space)
///   - `AppConstant.DeviceType` + `MyUtils.getDeviceNameByType()` (name table)
pub fn product_name_for_id(product_id: u16) -> Option<&'static str> {
    Some(match product_id {
        // ── Beacon product IDs (`Product.getProductId()`) ──
        55 => "oraimo SpaceBuds Fit",     // PRODUCT_627S (device type 6311)
        64 => "oraimo SpaceBuds Neo 2",   // OTW_321
        65 => "oraimo SpaceBuds 2",       // OTW_631_STAR
        4100 => "oraimo OpenSnap Hayato", // OTW_371_FF (device type 3711)
        4101 => "oraimo OpenSnap 2",      // PRODUCT_OPN671 (device type 671)
        16400 => "oraimo HomeBox",        // OBS_687 (device type 687)

        // ── Device types (`AppConstant.DeviceType`) ──
        104 => "oraimo FreePods 3C",
        105 | 10510 => "oraimo FreePods 4",
        108 => "oraimo FreePods Pro+",
        300 | 3001 | 3002 | 3003 => "oraimo SpaceBox",
        310 => "oraimo Necklace Neo",
        311 => "oraimo Necklace Lite",
        316 => "oraimo BoomPop Air+",
        317 => "oraimo BoomPop Lite",
        321 => "oraimo SpaceBuds Neo 2",
        323 => "oraimo SpaceBuds Neo",
        324 | 3240 => "oraimo SpaceBuds Lite",
        325 => "oraimo SpaceBuds Lite 2",
        330 => "oraimo FreePods Lite",
        340 => "oraimo AirBuds 4",
        371 => "oraimo OpenSnap Air",
        3711 => "oraimo OpenSnap Hayato",
        372 => "oraimo OpenSnap N",
        373 => "oraimo OpenSnap",
        374 => "oraimo OpenSnap N2",
        610 => "oraimo BoomPop2",
        611 | 6111 => "oraimo Necklace 5",
        620 | 6201 => "oraimo SpaceBuds Z",
        626 => "oraimo SpaceBuds N",
        630 | 6301 | 6302 | 6303 => "oraimo SpaceBuds",
        631 | 6312 => "oraimo SpaceBuds 2",
        662 => "oraimo OpenCirclet 2",
        666 => "oraimo OpenArc",
        670 => "oraimo OpenLoop",
        671 => "oraimo OpenSnap 2",
        675 => "oraimo SpaceArc",
        680 => "oraimo SpaceBox Active",
        682 => "oraimo SpaceBox Pro",
        687 => "oraimo HomeBox",
        915 => "oraimo BoomPop N",
        917 => "oraimo BoomPop Pro",
        930 => "oraimo SpaceBuds Pro",
        931 => "oraimo SpaceBuds Pro 2",
        982 => "oraimo SpaceBox Max",
        1059 => "oraimo SpacePods",
        3231 | 3232 => "oraimo SpaceBuds Neo+",
        3233 => "oraimo SpaceBuds Air+",
        3251 => "oraimo SpaceBuds Air 2",
        3252 => "oraimo SpaceBuds Lite 2+",
        3253 => "oraimo SpaceBuds Lite 2",
        3254 => "oraimo SpaceBuds Air 2",
        3300 | 3301 => "oraimo FreePods Neo",
        6100 => "oraimo BoomPop2S",
        6311 => "oraimo SpaceBuds Fit",
        57421 => "oraimo Riff 2",
        _ => return None,
    })
}

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

        // Determine version and parse. Length is authoritative for 8 and 13
        // (see module docs): a 13-byte payload is *always* V4 even when
        // `data[0] & 0x0F == 1`, which the old port got wrong.
        match len {
            8 => Self::parse_v3(data),      // V3
            13 => Self::parse_v4(data),     // V4 (never V1/V2)
            9 => Self::parse_v4(data),      // V4 (APK also requires nibble == 1)
            27 => {
                if data[0] & 0x0F == 2 {
                    Self::parse_v2(data)    // V2
                } else {
                    Self::parse_v4(data)    // V4 (incl. nibble 1)
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

        let connected = if is_special_pid {
            // APK: `data[0] > 1 ? 4 : 1`, then `(state >> 2) & 3`.
            data[0] > 1
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
        product_name_for_id(self.product_id).unwrap_or("Oraimo SpaceBuds")
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
        // V1 is only reachable through a direct call now (the reference
        // implementation never selects it for 8/9/13/27-byte payloads).
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

        let beacon = DeviceBeacon::parse_v1(&data).unwrap();
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

    /// Regression: a 13-byte beacon is *always* V4, regardless of the
    /// `data[0]` nibble. The old port treated nibble 1 as V1 and nibble 2 as
    /// V2, producing a garbled MAC (V1 does not XOR-deobfuscate).
    #[test]
    fn test_13_byte_beacon_is_always_v4() {
        let mac: [u8; 6] = [0x24, 0xDF, 0x17, 0x5A, 0x7F, 0x26];
        for nibble in [0x01u8, 0x02, 0x09, 0x00] {
            let mut data = vec![0u8; 13];
            data[0] = nibble; // product_id 917 (BoomPop Pro)
            data[1] = 0x95;
            data[2] = 0x03;
            for (i, b) in mac.iter().enumerate() {
                data[3 + i] = b ^ 0xAD;
            }

            let beacon = DeviceBeacon::from_manufacturer_data(&data)
                .unwrap_or_else(|| panic!("13-byte beacon must parse (nibble {:#x})", nibble));
            assert_eq!(beacon.version, 4, "nibble {:#x}", nibble);
            assert_eq!(beacon.product_id, 917);
            assert_eq!(beacon.bt_address, Some(mac), "nibble {:#x}", nibble);
        }
    }

    #[test]
    fn test_27_byte_v2_dispatch() {
        let mut data = vec![0u8; 27];
        data[0] = 0x02; // nibble 2 → V2
        data[1] = 0x95;
        data[2] = 0x03;
        let beacon = DeviceBeacon::from_manufacturer_data(&data).unwrap();
        assert_eq!(beacon.version, 2);
        assert_eq!(beacon.product_id, 917);
    }

    #[test]
    fn test_product_names() {
        assert_eq!(product_name_for_id(917), Some("oraimo BoomPop Pro"));
        assert_eq!(product_name_for_id(64), Some("oraimo SpaceBuds Neo 2"));
        assert_eq!(product_name_for_id(6311), Some("oraimo SpaceBuds Fit"));
        assert_eq!(product_name_for_id(65535), None);
        assert!(looks_like_oraimo_name("oraimo BoomPop Pro"));
        assert!(looks_like_oraimo_name("Oraimo SpacePods"));
        assert!(!looks_like_oraimo_name("JBL Tune"));
    }

    #[test]
    fn test_invalid_data() {
        assert!(DeviceBeacon::from_manufacturer_data(&[]).is_none());
        assert!(DeviceBeacon::from_manufacturer_data(&[0x00, 0x01]).is_none());
        // 7 bytes — not a valid length
        assert!(DeviceBeacon::from_manufacturer_data(&[0u8; 7]).is_none());
    }
}
