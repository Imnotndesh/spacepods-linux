/// Device profiles for the Oraimo product lineup.
///
/// Each device model declares which features it supports.
/// Feature detection is based on the BLE service UUIDs found during scanning
/// and the product ID parsed from the beacon advertisement data.
///
/// Original reference: `Product.java` + `DetailFeature.java` from the Oraimo Sound APK.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::sync::OnceLock;

// ── Oraimo Product IDs (beacon ID space, from Product.java) ──

pub const PID_SPACEBUDS_NEO_2: u16 = 64;    // OTW_321
pub const PID_OPENSNAP_HAYATO: u16 = 4100;  // OTW_371_FF
pub const PID_SPACEBUDS_2: u16 = 65;        // OTW_631_STAR
pub const PID_627S: u16 = 55;               // PRODUCT_627S
pub const PID_OPN671: u16 = 4101;           // PRODUCT_OPN671

// ── Detail Features (mapped from DetailFeature.java) ──

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DetailFeature {
    Noise,
    NoiseAdjust,
    ChatMode,
    DualDeviceInfo,
    DualDeviceSwitch,
    GameMode,
    SpaceAudio,
    Led,
    Care,
    EarControl,
    KeySetting,
    FirmwareUpgrade,
    ToneVolume,
    AdaptiveVolume,
    VoicePrompt,
    Sport,
    EarDetection,
    EarAuto,
    CustomTone,
    AiTone,
    ModifyName,
    FindEar,
    EqSwitch,
    DefaultEqHavyBass,
    ShowDiscoverUi,
    Location,
    BassEqSwitch,
    LongEndurance,
    Karaoke,
    VoiceControl,
    Ldac,
    SoundEffect3D,
    HearingCare,
    AreaTap,
    ClearPairRecord,
    BluetoothName,
    InEarDetection,
    AutoAnswer,
    FindDevice,
}

// ── Device Profile ──

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceProfile {
    pub product_id: u16,
    pub name: &'static str,
    pub model: &'static str,
    pub chip: ChipPlatform,
    pub form: FormFactor,
    pub features: BTreeSet<DetailFeature>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChipPlatform {
    Bluetrum,
    JieLi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FormFactor {
    InEar,
    HalfInEar,
    OpenEar,
    Headphones,
}

/// Helper to build a feature set from a list of feature names.
macro_rules! fs {
    ($($x:expr),+ $(,)?) => {{
        let mut s = BTreeSet::new();
        $(s.insert($x);)*
        s
    }};
}

/// Build the full profile list once.
///
/// The five curated entries below carry the exact feature sets ported from
/// `Product.java`. Every other product ID known to the APK is then added with
/// the broad generic feature set so `profile_for_product(917)` returns a
/// correctly *named* profile instead of silently falling back.
fn build_profiles() -> Vec<DeviceProfile> {
    let mut profiles = vec![
        DeviceProfile {
            product_id: PID_SPACEBUDS_NEO_2,
            name: "oraimo SpaceBuds Neo 2",
            model: "321",
            chip: ChipPlatform::Bluetrum,
            form: FormFactor::InEar,
            features: fs![
                DetailFeature::ToneVolume, DetailFeature::KeySetting,
                DetailFeature::ModifyName, DetailFeature::FindEar,
                DetailFeature::HearingCare, DetailFeature::DefaultEqHavyBass,
                DetailFeature::ShowDiscoverUi,
            ],
        },
        DeviceProfile {
            product_id: PID_SPACEBUDS_2,
            name: "oraimo SpaceBuds 2",
            model: "631_star",
            chip: ChipPlatform::JieLi,
            form: FormFactor::HalfInEar,
            features: fs![
                DetailFeature::Noise, DetailFeature::NoiseAdjust,
                DetailFeature::SpaceAudio, DetailFeature::EarControl,
                DetailFeature::GameMode, DetailFeature::Led,
                DetailFeature::CustomTone, DetailFeature::AiTone,
                DetailFeature::DualDeviceSwitch,
                DetailFeature::ModifyName, DetailFeature::FindEar,
                DetailFeature::HearingCare,
                DetailFeature::DefaultEqHavyBass, DetailFeature::ShowDiscoverUi,
            ],
        },
        DeviceProfile {
            product_id: PID_OPENSNAP_HAYATO,
            name: "oraimo OpenSnap Hayato",
            model: "OPN371_FF",
            chip: ChipPlatform::JieLi,
            form: FormFactor::OpenEar,
            features: fs![
                DetailFeature::SpaceAudio, DetailFeature::ToneVolume,
                DetailFeature::EarControl, DetailFeature::GameMode,
                DetailFeature::DualDeviceInfo, DetailFeature::AdaptiveVolume,
                DetailFeature::ModifyName, DetailFeature::FindEar,
                DetailFeature::DefaultEqHavyBass, DetailFeature::ShowDiscoverUi,
            ],
        },
        DeviceProfile {
            product_id: PID_627S,
            name: "oraimo SpaceBuds Fit",
            model: "6311",
            chip: ChipPlatform::JieLi,
            form: FormFactor::InEar,
            features: fs![
                DetailFeature::GameMode, DetailFeature::EarControl,
            ],
        },
        DeviceProfile {
            product_id: PID_OPN671,
            name: "oraimo OpenSnap 2",
            model: "671",
            chip: ChipPlatform::JieLi,
            form: FormFactor::HalfInEar,
            features: fs![DetailFeature::EarControl],
        },
        DeviceProfile {
            product_id: 917,
            name: "oraimo BoomPop Pro",
            model: "917",
            chip: ChipPlatform::JieLi,
            form: FormFactor::Headphones,
            // The BoomPop Pro disables a lot of the generic set. From the APK's
            // `DeviceNewActivity` 917 branch: noise, space audio, game mode and
            // dual device are enabled; `layout_care` and
            // `layout_earphone_control` are hidden; LED, tone volume, area tap,
            // long endurance, voice prompt, adaptive volume, magnet, vibration
            // and env-adaptive are `gone` by default and never enabled.
            features: fs![
                DetailFeature::Noise,
                DetailFeature::SpaceAudio,
                DetailFeature::GameMode,
                DetailFeature::DualDeviceSwitch,
                DetailFeature::EqSwitch,
                DetailFeature::ChatMode,
                DetailFeature::EarControl,
                DetailFeature::KeySetting,
            ],
        },
    ];

    // Add every other known product ID with the generic feature set. This keeps
    // the displayed device name accurate across the whole Oraimo lineup while
    // avoiding over-claiming feature support we have not verified.
    for &id in crate::beacon::KNOWN_DEVICE_TYPES {
        if profiles.iter().any(|p| p.product_id == id) {
            continue;
        }
        if let Some(name) = crate::beacon::device_type_name(id) {
            profiles.push(DeviceProfile {
                product_id: id,
                name,
                model: "",
                chip: ChipPlatform::Bluetrum,
                form: FormFactor::InEar,
                features: generic_features(),
            });
        }
    }

    profiles
}

fn profiles() -> &'static Vec<DeviceProfile> {
    static PROFILES: OnceLock<Vec<DeviceProfile>> = OnceLock::new();
    PROFILES.get_or_init(build_profiles)
}

fn generic_features() -> BTreeSet<DetailFeature> {
    fs![
        DetailFeature::Noise,
        DetailFeature::GameMode,
        DetailFeature::EarControl,
        DetailFeature::KeySetting,
        DetailFeature::ToneVolume,
        DetailFeature::ModifyName,
        DetailFeature::FindEar,
        DetailFeature::HearingCare,
        DetailFeature::DefaultEqHavyBass,
        DetailFeature::ShowDiscoverUi,
        DetailFeature::SpaceAudio,
        DetailFeature::SoundEffect3D,
        DetailFeature::AreaTap,
        DetailFeature::Led,
        DetailFeature::DualDeviceSwitch,
        DetailFeature::ChatMode,
        DetailFeature::LongEndurance,
        DetailFeature::VoicePrompt,
        DetailFeature::InEarDetection,
        DetailFeature::AutoAnswer,
        DetailFeature::AdaptiveVolume,
        DetailFeature::ClearPairRecord,
        DetailFeature::BluetoothName,
        DetailFeature::FindDevice,
        DetailFeature::EqSwitch,
    ]
}

fn generic_profile() -> DeviceProfile {
    DeviceProfile {
        product_id: 0,
        name: "Oraimo SpaceBuds",
        model: "generic",
        chip: ChipPlatform::Bluetrum,
        form: FormFactor::InEar,
        features: generic_features(),
    }
}

fn generic_static() -> &'static DeviceProfile {
    static GENERIC: OnceLock<DeviceProfile> = OnceLock::new();
    GENERIC.get_or_init(generic_profile)
}

/// Get the profile for a known product ID, or the generic fallback.
pub fn profile_for_product(product_id: u16) -> &'static DeviceProfile {
    profiles().iter()
        .find(|p| p.product_id == product_id)
        .unwrap_or_else(generic_static)
}

/// Get the profile for a known device name.
pub fn profile_for_name(name: &str) -> Option<&'static DeviceProfile> {
    let lower = name.to_lowercase();
    profiles().iter().find(|p| {
        p.name.to_lowercase() == lower
            || (!p.model.is_empty() && lower.contains(&p.model.to_lowercase()))
    })
}

/// Decode the Oraimo capability bitfield (custom command 33) into the set of
/// features the firmware itself reports.
///
/// Bit `n` is little-endian within each byte
/// (`DeviceCapability.isSupported`). This is authoritative for devices that
/// broadcast it — e.g. the BoomPop Pro reports no `Led` bit — and should be
/// preferred over the static product profile when present.
pub fn features_from_capabilities(bytes: &[u8]) -> BTreeSet<DetailFeature> {
    let has = |bit: usize| -> bool {
        bytes
            .get(bit / 8)
            .map(|b| b & (1u8 << (bit % 8)) != 0)
            .unwrap_or(false)
    };
    let mut f = BTreeSet::new();
    if has(0) {
        f.insert(DetailFeature::Noise);
    }
    if has(1) {
        f.insert(DetailFeature::EqSwitch);
    }
    if has(2) {
        f.insert(DetailFeature::SpaceAudio);
    }
    if has(3) {
        f.insert(DetailFeature::VoicePrompt);
    }
    if has(4) {
        f.insert(DetailFeature::Led);
    }
    if has(5) {
        f.insert(DetailFeature::EarControl);
    }
    if has(6) {
        f.insert(DetailFeature::GameMode);
    }
    if has(7) {
        f.insert(DetailFeature::DualDeviceSwitch);
    }
    if has(8) {
        f.insert(DetailFeature::ModifyName);
    }
    if has(9) {
        f.insert(DetailFeature::FirmwareUpgrade);
    }
    if has(10) {
        f.insert(DetailFeature::FindDevice);
    }
    if has(11) {
        f.insert(DetailFeature::HearingCare);
    }
    if has(15) {
        f.insert(DetailFeature::InEarDetection);
    }
    if has(16) {
        f.insert(DetailFeature::Ldac);
    }
    if has(17) {
        f.insert(DetailFeature::ToneVolume);
    }
    if has(20) {
        f.insert(DetailFeature::SoundEffect3D);
    }
    if has(21) {
        f.insert(DetailFeature::LongEndurance);
    }
    if has(24) {
        f.insert(DetailFeature::CustomTone);
    }
    if has(26) {
        f.insert(DetailFeature::AreaTap);
    }
    if has(31) {
        f.insert(DetailFeature::KeySetting);
    }
    if has(33) {
        f.insert(DetailFeature::Location);
    }
    if has(35) {
        f.insert(DetailFeature::Karaoke);
    }
    if has(36) {
        f.insert(DetailFeature::VoicePrompt);
    }
    f
}

#[cfg(test)]
mod capability_tests {
    use super::*;

    #[test]
    fn decodes_little_endian_bits() {
        // bit 0 (Noise) + bit 4 (Led) + bit 10 (FindDevice)
        let mut bytes = vec![0u8; 2];
        bytes[0] |= 1 << 0;
        bytes[0] |= 1 << 4;
        bytes[1] |= 1 << 2; // bit 10 = byte 1, bit 2
        let f = features_from_capabilities(&bytes);
        assert!(f.contains(&DetailFeature::Noise));
        assert!(f.contains(&DetailFeature::Led));
        assert!(f.contains(&DetailFeature::FindDevice));
        assert!(!f.contains(&DetailFeature::GameMode));
    }

    #[test]
    fn empty_bytes_yield_no_features() {
        assert!(features_from_capabilities(&[]).is_empty());
    }

    #[test]
    fn boompop_pro_lacks_led() {
        let p = profile_for_product(917);
        assert_eq!(p.name, "oraimo BoomPop Pro");
        assert!(p.features.contains(&DetailFeature::Noise));
        assert!(p.features.contains(&DetailFeature::GameMode));
        assert!(!p.features.contains(&DetailFeature::Led));
        assert!(!p.features.contains(&DetailFeature::ToneVolume));
        assert!(!p.features.contains(&DetailFeature::HearingCare));
    }
}

/// Detect the Oraimo device type encoded in a BLE advertisement's
/// manufacturer data.
///
/// Delegates to [`crate::beacon::DeviceBeacon`] so the 8-byte layout (where the
/// product ID starts at offset 0) and the XOR-obfuscated variants are handled
/// correctly, then maps the beacon product ID to an Oraimo device type via
/// [`crate::beacon::device_type_from_pid`] so [`profile_for_product`] can find
/// a matching profile. Unknown IDs are returned as-is.
pub fn detect_product_id(manufacturer_data: &[u8]) -> Option<u16> {
    let to_device_type = |pid: u16| crate::beacon::device_type_from_pid(pid).unwrap_or(pid);
    if let Some(beacon) = crate::beacon::DeviceBeacon::from_manufacturer_data(manufacturer_data) {
        return Some(to_device_type(beacon.product_id));
    }
    // Last-resort legacy fallback for payloads the strict parser rejects.
    if manufacturer_data.len() >= 3 {
        let pid = u16::from_le_bytes([manufacturer_data[1], manufacturer_data[2]]);
        return Some(to_device_type(pid));
    }
    None
}
