use super::types::MessageType;
use super::constants::*;
use crate::Error;

/// A BLE protocol packet sent to/received from the SpaceBuds device.
///
/// Wire format:
///   [seq: 1][cmd_id: 1][msg_type: 1][fragment: 1][payload_len: 1][payload...]
#[derive(Debug, Clone)]
pub struct Packet {
    pub seq: u8,
    pub cmd_id: u8,
    pub msg_type: MessageType,
    pub fragment: u8,
    pub payload: Vec<u8>,
}

impl Packet {
    pub fn new_request(seq: u8, cmd_id: u8, payload: Vec<u8>) -> Self {
        Self {
            seq: seq & 0x0F,
            cmd_id,
            msg_type: MessageType::Request,
            fragment: 0x00,
            payload,
        }
    }

    pub fn new_response(seq: u8, cmd_id: u8, payload: Vec<u8>) -> Self {
        Self {
            seq: seq & 0x0F,
            cmd_id,
            msg_type: MessageType::Response,
            fragment: 0x00,
            payload,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = vec![
            self.seq,
            self.cmd_id,
            self.msg_type.into(),
            self.fragment,
            self.payload.len() as u8,
        ];
        bytes.extend(&self.payload);
        bytes
    }

    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 5 {
            return None;
        }

        let msg_type = MessageType::from_u8(data[2])?;

        Some(Self {
            seq: data[0],
            cmd_id: data[1],
            msg_type,
            fragment: data[3],
            payload: data[5..].to_vec(),
        })
    }

    /// Create a standard handshake TLV query packet.
    /// Queries the device for one or more info IDs.
    pub fn handshake_query(info_ids: &[u8]) -> Self {
        let mut payload = HANDSHAKE_PREFIX.to_vec();
        for &id in info_ids {
            payload.extend_from_slice(&[id, 0x00]);
        }
        // Use seq=0, caller should set proper seq if needed
        Self::new_request(0, CMD_HANDSHAKE, payload)
    }
}
