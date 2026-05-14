use thiserror::Error;

#[derive(Error, Debug)]
pub enum SpaceBudsError {
    #[error("Bluetooth error: {0}")]
    Bluetooth(#[from] btleplug::Error),

    #[error("Device not found")]
    DeviceNotFound,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Connection lost")]
    ConnectionLost,

    #[error("Write characteristic not found")]
    WriteCharNotFound,

    #[error("Notify characteristic not found")]
    NotifyCharNotFound,

    #[error("Command failed: {0}")]
    CommandFailed(String),

    #[error("Timeout")]
    Timeout,

    #[error("IPC error: {0}")]
    Ipc(String),

    #[error("Invalid preset: {0}")]
    InvalidPreset(u8),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, SpaceBudsError>;