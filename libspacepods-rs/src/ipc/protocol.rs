use crate::protocol::{AncMode, BatteryLevel, ConnectionState};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[cfg(unix)]
pub const DEFAULT_SOCKET_PATH: &str = "/tmp/spacepods.sock";

#[cfg(windows)]
pub const DEFAULT_SOCKET_PATH: &str = r"\\.\pipe\spacepods";

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
    Error { message: String },

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
            },
            features: FeatureInfo {
                adaptive_anc: None,
                dual_device: None,
            },
            product_id: None,
            key_settings: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ConnectionInfo {
    pub connected: bool,
    pub address: Option<String>,
    pub state: ConnectionState,
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
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FeatureInfo {
    pub adaptive_anc: Option<bool>,
    pub dual_device: Option<bool>,
}
