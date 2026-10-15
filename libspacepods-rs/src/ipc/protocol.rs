use crate::protocol::{AncMode, ConnectionState};
use crate::device_profile::DetailFeature;
use crate::errors::{Error, ErrorCode};
use serde::{Deserialize, Serialize};

/// Version of the request/response schema shared by the socket and D-Bus interfaces.
/// Bump when a change is not backward compatible.
pub const PROTOCOL_VERSION: u32 = 1;

// ── IPC Message Envelope ──

/// Single envelope type for all IPC communication.
/// Uses an `id` field for request-response correlation.
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum IpcMessage {
    Request {
        id: u64,
        #[serde(flatten)]
        command: ServiceCommand,
    },
    Response {
        id: u64,
        #[serde(flatten)]
        result: IpcResult,
    },
}

// ── Service Commands ──

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "cmd", content = "args")]
pub enum ServiceCommand {
    #[serde(rename = "ping")]
    Ping,

    #[serde(rename = "status")]
    GetStatus,

    #[serde(rename = "anc")]
    SetAncMode { mode: String },

    #[serde(rename = "level")]
    SetLevel { level: u8 },

    #[serde(rename = "scan")]
    Scan { timeout_secs: u64 },

    #[serde(rename = "connect")]
    Connect { address: String },

    #[serde(rename = "eq")]
    SetEqPreset { preset: u8 },

    #[serde(rename = "adaptive")]
    SetAdaptiveAnc { enabled: bool },

    #[serde(rename = "dual")]
    SetDualDevice { enabled: bool },

    #[serde(rename = "subscribe")]
    Subscribe,

    #[serde(rename = "unsubscribe")]
    Unsubscribe,

    /// Disconnect from the current device.
    #[serde(rename = "disconnect")]
    Disconnect,

    /// Send a raw BLE command by its command ID.
    /// The daemon will forward this directly to the earbuds.
    #[serde(rename = "custom")]
    Custom {
        command_id: u8,
        payload: Vec<u8>,
    },

    /// Send a raw Oraimo/JieLi RCSP custom command and return the response
    /// parameter bytes (if any).
    #[serde(rename = "jieli_custom")]
    JieliCustom { payload: Vec<u8> },

    /// Set a boolean feature by stable, chip-agnostic name.
    #[serde(rename = "set_feature")]
    SetFeature { feature: String, enabled: bool },

    /// Query a boolean feature by stable, chip-agnostic name.
    #[serde(rename = "get_feature")]
    GetFeature { feature: String },

    /// Rename the device (chip-agnostic).
    #[serde(rename = "rename")]
    Rename { name: String },

    /// Set the button/voice-prompt tone volume (0-10, chip-agnostic).
    #[serde(rename = "set_tone_volume")]
    SetToneVolume { level: u8 },

    /// Remap a gesture key to a function (chip-agnostic).
    #[serde(rename = "set_key_setting")]
    SetKeySetting { key_type: u8, function: u8 },
}

// ── IPC Response ──

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum IpcResult {
    #[serde(rename = "success")]
    Success {
        message: Option<String>,
        data: Option<serde_json::Value>,
    },

    #[serde(rename = "error")]
    Error {
        /// Stable category; clients choose user-facing text from this.
        #[serde(default)]
        code: ErrorCode,
        /// For logs and developers. Not a stable interface.
        message: String,
        #[serde(default)]
        retryable: bool,
    },

    #[serde(rename = "scan_results")]
    ScanResults { devices: Vec<ScannedDevice> },

    #[serde(rename = "status_update")]
    StatusUpdate { status: DeviceStatus },
}

// ── Scanned Device ──

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ScannedDevice {
    pub name: String,
    pub address: String,
    /// Product name from beacon (e.g. "SpaceBuds Neo 2")
    #[serde(default)]
    pub product_name: Option<String>,
    /// Product ID from beacon
    #[serde(default)]
    pub product_id: Option<u16>,
    /// Real Bluetooth MAC from beacon (de-obfuscated)
    #[serde(default)]
    pub real_mac: Option<String>,
    /// Beacon version (1-4)
    #[serde(default)]
    pub beacon_version: Option<u8>,
    /// Battery levels from beacon (V1 only)
    #[serde(default)]
    pub battery_left: Option<u8>,
    #[serde(default)]
    pub battery_right: Option<u8>,
    #[serde(default)]
    pub battery_case: Option<u8>,
    /// Whether the device is currently connected to another phone
    #[serde(default)]
    pub already_connected: bool,
    /// Signal strength in dBm (closer to 0 = stronger)
    #[serde(default)]
    pub rssi: Option<i16>,
}

// ── Device Status (composed) ──

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DeviceStatus {
    pub connection: ConnectionInfo,
    pub anc: AncInfo,
    pub eq: Option<EqInfo>,
    pub battery: BatteryInfo,
    pub features: FeatureInfo,
    #[serde(default)]
    pub product_id: Option<u16>,
    /// Features the device itself reported (capability bitfield). When present
    /// this is authoritative over the static product profile.
    #[serde(default)]
    pub capabilities: Option<Vec<DetailFeature>>,
    /// Current key/gesture mappings: key_type -> function
    #[serde(default)]
    pub key_settings: Option<std::collections::HashMap<u8, u8>>,
}

impl DeviceStatus {
    pub fn default_disconnected() -> Self {
        Self {
            connection: ConnectionInfo {
                connected: false,
                address: None,
                state: ConnectionState::Disconnected,
                audio: false,
            },
            anc: AncInfo {
                mode: AncMode::Off,
                level: 0,
                max_level: 0,
            },
            eq: None,
            battery: BatteryInfo {
                left: None,
                right: None,
                case: None,
                level: None,
                source: Default::default(),
                state: Default::default(),
            },
            features: FeatureInfo {
                adaptive_anc: None,
                dual_device: None,
            },
            product_id: None,
            capabilities: None,
            key_settings: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConnectionInfo {
    pub connected: bool,
    pub address: Option<String>,
    pub state: ConnectionState,
    /// Whether the audio link is up (BlueZ media transport). The control link and the
    /// audio link are separate; this is the second one.
    #[serde(default)]
    pub audio: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AncInfo {
    pub mode: AncMode,
    pub level: u8,
    pub max_level: u8,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EqInfo {
    pub mode: u8,
    pub name: String,
    pub description: String,
    pub gains: Vec<i8>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BatteryInfo {
    pub left: Option<u8>,
    pub right: Option<u8>,
    pub case: Option<u8>,
    /// The earbuds' battery percentage as one number, when known.
    #[serde(default)]
    pub level: Option<u8>,
    /// Where `level` came from.
    #[serde(default)]
    pub source: crate::battery::BatterySource,
    /// Whether `level` is current. See [`crate::battery::BatteryState`].
    #[serde(default)]
    pub state: crate::battery::BatteryState,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FeatureInfo {
    pub adaptive_anc: Option<bool>,
    pub dual_device: Option<bool>,
}

impl IpcResult {
    /// An error reply with a code. `retryable` follows the code.
    pub fn error(code: ErrorCode, message: impl Into<String>) -> Self {
        IpcResult::Error {
            code,
            message: message.into(),
            retryable: code.retryable(),
        }
    }

    /// An error reply for a failed operation: `context` says what was attempted, and
    /// the code comes from the underlying error.
    pub fn failed(context: &str, err: &Error) -> Self {
        Self::error(err.code(), format!("{}: {}", context, err))
    }
}
