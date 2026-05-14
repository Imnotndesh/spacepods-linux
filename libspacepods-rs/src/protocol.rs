use uuid::Uuid;

pub const CMD_HANDSHAKE: u8 = 0x27;
pub const CMD_ANC_MODE: u8 = 0x2C;
pub const CMD_ANC_GAIN: u8 = 0x30;
pub const CMD_TRANS_GAIN: u8 = 0x31;
pub const CMD_DUAL_DEVICE: u8 = 0x33;
pub const CMD_EQ_SETTING: u8 = 0x20;
pub const CMD_ENV_ADAPTIVE: u8 = 0x37;

// Info IDs (for queries)
pub const ID_ANC_MODE: u8 = 0x0C;
pub const ID_ANC_GAIN: u8 = 0x11;
pub const ID_TRANS_GAIN: u8 = 0x12;
pub const ID_ANC_MAX: u8 = 0x13;
pub const ID_TRANS_MAX: u8 = 0x14;
pub const ID_DUAL_DEVICE: u8 = 0x19;
pub const ID_EQ_SETTING: u8 = 0x04;
pub const ID_ENV_ADAPTIVE: u8 = 0x21;

// Message types
pub const TYPE_REQUEST: u8 = 0x01;
pub const TYPE_RESPONSE: u8 = 0x02;

// ANC modes
pub const MODE_OFF: u8 = 0;
pub const MODE_ANC: u8 = 1;
pub const MODE_TRANSPARENCY: u8 = 2;

// Characteristic UUIDs
pub const UUID_WRITE: Uuid = Uuid::from_u128(0x0000ff17_0000_1000_8000_00805f9b34fb);
pub const UUID_NOTIFY: Uuid = Uuid::from_u128(0x0000ff18_0000_1000_8000_00805f9b34fb);
pub const UUID_FAST_PAIR: Uuid = Uuid::from_u128(0x0000fe2c_0000_1000_8000_00805f9b34fb);
pub const UUID_BATTERY_SERVICE: Uuid = Uuid::from_u128(0x0000180f_0000_1000_8000_00805f9b34fb);
pub const UUID_BATTERY_LEVEL: Uuid = Uuid::from_u128(0x00002a19_0000_1000_8000_00805f9b34fb);

#[derive(Debug, Clone)]
pub struct Packet {
    pub seq: u8,
    pub cmd_id: u8,
    pub msg_type: u8,
    pub payload: Vec<u8>,
}

impl Packet {
    pub fn new_request(seq: u8, cmd_id: u8, payload: Vec<u8>) -> Self {
        Self {
            seq: seq & 0x0F,
            cmd_id,
            msg_type: TYPE_REQUEST,
            payload,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = vec![
            self.seq,
            self.cmd_id,
            self.msg_type,
            0x00, // fragment
            self.payload.len() as u8,
        ];
        bytes.extend(&self.payload);
        bytes
    }

    pub fn from_bytes(data: &[u8]) -> Option<Self> {
        if data.len() < 5 {
            return None;
        }

        Some(Self {
            seq: data[0],
            cmd_id: data[1],
            msg_type: data[2],
            payload: data[5..].to_vec(),
        })
    }
}

// TLV Parser (matches Python's _parse_tlv)
pub struct TlvParser<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> TlvParser<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub fn next(&mut self) -> Option<(u8, &'a [u8])> {
        if self.pos + 2 > self.data.len() {
            return None;
        }

        let tag = self.data[self.pos];
        let len = self.data[self.pos + 1] as usize;
        let val_start = self.pos + 2;

        if val_start + len > self.data.len() {
            return None;
        }

        let value = &self.data[val_start..val_start + len];
        self.pos = val_start + len;

        Some((tag, value))
    }

    pub fn get_int(&mut self, target_tag: u8) -> Option<u8> {
        while let Some((tag, value)) = self.next() {
            if tag == target_tag && !value.is_empty() {
                return Some(value[0]);
            }
        }
        None
    }

    pub fn get_bytes(&mut self, target_tag: u8) -> Option<&'a [u8]> {
        while let Some((tag, value)) = self.next() {
            if tag == target_tag {
                return Some(value);
            }
        }
        None
    }
}