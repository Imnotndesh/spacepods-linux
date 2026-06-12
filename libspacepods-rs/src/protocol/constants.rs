
pub const UUID_WRITE: uuid::Uuid = uuid::Uuid::from_u128(0x0000ff17_0000_1000_8000_00805f9b34fb);
pub const UUID_NOTIFY: uuid::Uuid = uuid::Uuid::from_u128(0x0000ff18_0000_1000_8000_00805f9b34fb);
pub const UUID_FAST_PAIR: uuid::Uuid = uuid::Uuid::from_u128(0x0000fe2c_0000_1000_8000_00805f9b34fb);
pub const UUID_BATTERY_SERVICE: uuid::Uuid = uuid::Uuid::from_u128(0x0000180f_0000_1000_8000_00805f9b34fb);
pub const UUID_BATTERY_LEVEL: uuid::Uuid = uuid::Uuid::from_u128(0x00002a19_0000_1000_8000_00805f9b34fb);

// Command IDs
pub const CMD_HANDSHAKE: u8 = 0x27;
pub const CMD_ANC_MODE: u8 = 0x2C;
pub const CMD_ANC_GAIN: u8 = 0x30;
pub const CMD_TRANS_GAIN: u8 = 0x31;
pub const CMD_DUAL_DEVICE: u8 = 0x33;
pub const CMD_EQ_SETTING: u8 = 0x20;
pub const CMD_ENV_ADAPTIVE: u8 = 0x37;

// Info IDs (for TLV queries)
pub const ID_ANC_MODE: u8 = 0x0C;
pub const ID_ANC_GAIN: u8 = 0x11;
pub const ID_TRANS_GAIN: u8 = 0x12;
pub const ID_ANC_MAX: u8 = 0x13;
pub const ID_TRANS_MAX: u8 = 0x14;
pub const ID_DUAL_DEVICE: u8 = 0x19;
pub const ID_EQ_SETTING: u8 = 0x04;
pub const ID_ENV_ADAPTIVE: u8 = 0x21;

// Handshake magic bytes
pub const HANDSHAKE_PREFIX: &[u8] = &[0xFF, 0x00];
