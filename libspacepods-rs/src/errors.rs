use thiserror::Error;
use std::time::Duration;

#[derive(Error, Debug)]
pub enum Error {
    // ── Bluetooth ──
    #[error("Bluetooth error: {0}")]
    Bluetooth(#[from] btleplug::Error),

    #[error("Device not found after {retries} retries")]
    DeviceNotFound { retries: usize },

    #[error("Not connected to any device")]
    NotConnected,

    #[error("Connection lost")]
    ConnectionLost,

    #[error("Write characteristic not found")]
    WriteCharNotFound,

    #[error("Notify characteristic not found")]
    NotifyCharNotFound,

    // ── Protocol ──
    #[error("Invalid message type: {0:#x}")]
    InvalidMessageType(u8),

    #[error("Unknown ANC mode: {0}")]
    UnknownAncMode(u8),

    #[error("Parse error: {0}")]
    Parse(&'static str),

    // ── Commands ──
    #[error("Command failed: {0}")]
    CommandFailed(String),

    #[error("Invalid preset ID: {0}")]
    InvalidPreset(u8),

    // ── Timing ──
    #[error("Timeout after {0:?}")]
    Timeout(Duration),

    // ── IPC ──
    #[error("IPC error: {0}")]
    Ipc(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("Service stopped")]
    ServiceStopped,
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
