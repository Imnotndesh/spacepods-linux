use serde::{Deserialize, Serialize};
use thiserror::Error;
use std::path::PathBuf;
use std::time::Duration;

/// Stable, machine-readable error category. It travels over the socket and D-Bus.
///
/// Clients map codes to user-facing text and must not parse messages. Add variants
/// only together with a documentation update, since clients match on them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ErrorCode {
    /// No device is connected.
    NotConnected,
    /// A scan or connect found no matching device.
    DeviceNotFound,
    /// No Bluetooth adapter, or a BlueZ error.
    BluetoothUnavailable,
    /// The Bluetooth adapter is switched off. The person can turn it on.
    BluetoothOff,
    /// The device or daemon did not answer in time.
    Timeout,
    /// Another connect or command is in progress.
    Busy,
    /// The device does not support this feature.
    UnsupportedFeature,
    /// The request is valid but the current state does not allow it (for example
    /// setting a level while ANC is off).
    InvalidState,
    /// The client sent a value the daemon cannot use.
    InvalidArgument,
    /// Client and daemon speak different protocol versions.
    ProtocolMismatch,
    /// A request frame was over the size limit.
    PayloadTooLarge,
    /// The daemon is not running, is shutting down, or the connection dropped.
    ServiceUnavailable,
    /// A bug or an unexpected condition.
    #[default]
    Internal,
}

impl ErrorCode {
    /// Whether retrying the same request later could succeed without any change.
    pub fn retryable(self) -> bool {
        matches!(
            self,
            ErrorCode::NotConnected
                | ErrorCode::DeviceNotFound
                | ErrorCode::BluetoothUnavailable
                | ErrorCode::BluetoothOff
                | ErrorCode::Timeout
                | ErrorCode::Busy
                | ErrorCode::ServiceUnavailable
        )
    }
}

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

    #[error("{0} control protocol is not supported yet")]
    UnsupportedChip(&'static str),

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

    #[error("another SpacePods service is already running at {}", path.display())]
    AlreadyRunning { path: PathBuf },

    #[error("unsafe socket directory {}: {reason}", path.display())]
    UnsafeSocketDir { path: PathBuf, reason: String },

    #[error("D-Bus error: {0}")]
    Dbus(String),

    #[error("Bluetooth is turned off")]
    BluetoothOff,

    /// The device was found but its protocol could not be identified, so it is refused.
    #[error("{0}")]
    UnrecognisedDevice(&'static str),

    /// An error the daemon reported, with its code. `message` is for logs and
    /// developers; user-facing text is chosen from `code`.
    #[error("{message}")]
    Service { code: ErrorCode, message: String },

    #[error("IPC frame exceeds {limit} bytes")]
    PayloadTooLarge { limit: usize },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    /// The stable category for this error.
    pub fn code(&self) -> ErrorCode {
        match self {
            Error::Bluetooth(_) => ErrorCode::BluetoothUnavailable,
            Error::BluetoothOff => ErrorCode::BluetoothOff,
            Error::UnrecognisedDevice(_) => ErrorCode::UnsupportedFeature,
            Error::DeviceNotFound { .. } => ErrorCode::DeviceNotFound,
            Error::NotConnected | Error::ConnectionLost => ErrorCode::NotConnected,
            Error::WriteCharNotFound | Error::NotifyCharNotFound => ErrorCode::UnsupportedFeature,
            Error::UnsupportedChip(_) => ErrorCode::UnsupportedFeature,
            Error::InvalidMessageType(_) | Error::Parse(_) | Error::Serde(_) => ErrorCode::Internal,
            Error::UnknownAncMode(_) | Error::InvalidPreset(_) => ErrorCode::InvalidArgument,
            Error::CommandFailed(_) => ErrorCode::Internal,
            Error::Timeout(_) => ErrorCode::Timeout,
            Error::Ipc(_) | Error::Io(_) | Error::ServiceStopped | Error::Dbus(_) => {
                ErrorCode::ServiceUnavailable
            }
            Error::AlreadyRunning { .. } | Error::UnsafeSocketDir { .. } => ErrorCode::Internal,
            Error::PayloadTooLarge { .. } => ErrorCode::PayloadTooLarge,
            Error::Service { code, .. } => *code,
        }
    }

    /// Whether retrying later could succeed without any change.
    pub fn retryable(&self) -> bool {
        self.code().retryable()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ipc::protocol::IpcResult;

    #[test]
    fn codes_serialise_as_snake_case() {
        assert_eq!(serde_json::to_string(&ErrorCode::NotConnected).unwrap(), "\"not_connected\"");
        assert_eq!(serde_json::to_string(&ErrorCode::ServiceUnavailable).unwrap(), "\"service_unavailable\"");
    }

    #[test]
    fn coded_reply_carries_code_and_retryable() {
        let json = serde_json::to_string(&IpcResult::error(ErrorCode::Timeout, "slow")).unwrap();
        assert!(json.contains("\"code\":\"timeout\""), "{json}");
        assert!(json.contains("\"retryable\":true"), "{json}");
    }

    #[test]
    fn reply_from_older_daemon_defaults_to_internal() {
        // A v1 daemon sends only a message. The client must still parse it.
        let parsed: IpcResult = serde_json::from_str(r#"{"type":"error","message":"boom"}"#).unwrap();
        match parsed {
            IpcResult::Error { code, retryable, message } => {
                assert_eq!(code, ErrorCode::Internal);
                assert!(!retryable);
                assert_eq!(message, "boom");
            }
            other => panic!("unexpected {:?}", other),
        }
    }

    #[test]
    fn library_errors_map_to_codes() {
        assert_eq!(Error::NotConnected.code(), ErrorCode::NotConnected);
        assert_eq!(Error::Timeout(Duration::from_secs(1)).code(), ErrorCode::Timeout);
        assert_eq!(Error::DeviceNotFound { retries: 3 }.code(), ErrorCode::DeviceNotFound);
        assert_eq!(Error::Ipc("closed".into()).code(), ErrorCode::ServiceUnavailable);
        assert!(Error::Timeout(Duration::from_secs(1)).retryable());
        assert!(!Error::UnknownAncMode(9).retryable());
    }

    #[test]
    fn service_error_keeps_its_code() {
        let e = Error::Service { code: ErrorCode::Busy, message: "busy".into() };
        assert_eq!(e.code(), ErrorCode::Busy);
        assert_eq!(e.to_string(), "busy");
    }
}
