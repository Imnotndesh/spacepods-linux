use serde::{Deserialize, Serialize};
use std::fmt;

// ── ANC Mode ──

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum AncMode {
    Off = 0,
    Active = 1,
    Transparency = 2,
}

impl AncMode {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Off),
            1 => Some(Self::Active),
            2 => Some(Self::Transparency),
            _ => None,
        }
    }
}

impl From<AncMode> for u8 {
    fn from(m: AncMode) -> Self {
        m as u8
    }
}

impl TryFrom<u8> for AncMode {
    type Error = crate::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::from_u8(value).ok_or(crate::Error::UnknownAncMode(value))
    }
}

impl fmt::Display for AncMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Off => write!(f, "OFF"),
            Self::Active => write!(f, "ANC"),
            Self::Transparency => write!(f, "TRANSPARENCY"),
        }
    }
}

// ── Message Type ──

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MessageType {
    Request = 0x01,
    Response = 0x02,
    Notify = 0x03,
}

impl MessageType {
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x01 => Some(Self::Request),
            0x02 => Some(Self::Response),
            0x03 => Some(Self::Notify),
            _ => None,
        }
    }
}

impl From<MessageType> for u8 {
    fn from(mt: MessageType) -> Self {
        mt as u8
    }
}

impl TryFrom<u8> for MessageType {
    type Error = crate::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::from_u8(value).ok_or(crate::Error::InvalidMessageType(value))
    }
}

// ── Mac Address ──

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MacAddress(pub [u8; 6]);

impl MacAddress {
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() >= 6 {
            let mut addr = [0u8; 6];
            addr.copy_from_slice(&bytes[..6]);
            Some(Self(addr))
        } else {
            None
        }
    }
}

impl fmt::Display for MacAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5]
        )
    }
}

// ── Connection State ──

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionState {
    Disconnected,
    Scanning,
    Connecting,
    Connected,
    Reconnecting,
    Failed(String),
}


impl fmt::Display for ConnectionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disconnected => write!(f, "Disconnected"),
            Self::Scanning => write!(f, "Scanning"),
            Self::Connecting => write!(f, "Connecting"),
            Self::Connected => write!(f, "Connected"),
            Self::Reconnecting => write!(f, "Reconnecting"),
            Self::Failed(msg) => write!(f, "Failed: {}", msg),
        }
    }
}

// ── Battery Level ──

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatteryLevel {
    pub left: Option<u8>,
    pub right: Option<u8>,
    pub case: Option<u8>,
}

impl BatteryLevel {
    pub fn new(left: Option<u8>, right: Option<u8>, case: Option<u8>) -> Self {
        Self {
            left: left.map(|v| v.min(100)),
            right: right.map(|v| v.min(100)),
            case: case.map(|v| v.min(100)),
        }
    }

    pub fn is_known(&self) -> bool {
        self.left.is_some() || self.right.is_some() || self.case.is_some()
    }
}

impl fmt::Display for BatteryLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let parts: Vec<String> = [
            self.left.map(|v| format!("L:{}%", v)),
            self.right.map(|v| format!("R:{}%", v)),
            self.case.map(|v| format!("Case:{}%", v)),
        ]
            .into_iter()
            .flatten()
            .collect();

        if parts.is_empty() {
            write!(f, "Unknown")
        } else {
            write!(f, "{}", parts.join("  "))
        }
    }
}
