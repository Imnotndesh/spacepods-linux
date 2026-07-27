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
}

// ── Device Status (composed) ──

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DeviceStatus {
    pub connection: ConnectionInfo,
    pub anc: AncInfo,
    pub eq: Option<EqInfo>,
    pub battery: BatteryInfo,
    pub features: FeatureInfo,
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
