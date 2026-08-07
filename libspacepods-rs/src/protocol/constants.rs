// ── BLE Service UUIDs ──

pub const UUID_WRITE: uuid::Uuid = uuid::Uuid::from_u128(0x0000ff17_0000_1000_8000_00805f9b34fb);
pub const UUID_NOTIFY: uuid::Uuid = uuid::Uuid::from_u128(0x0000ff18_0000_1000_8000_00805f9b34fb);
pub const UUID_FAST_PAIR: uuid::Uuid = uuid::Uuid::from_u128(0x0000fe2c_0000_1000_8000_00805f9b34fb);
pub const UUID_BATTERY_SERVICE: uuid::Uuid = uuid::Uuid::from_u128(0x0000180f_0000_1000_8000_00805f9b34fb);
pub const UUID_BATTERY_LEVEL: uuid::Uuid = uuid::Uuid::from_u128(0x00002a19_0000_1000_8000_00805f9b34fb);

// Aliases used by beacon/scanner for scan filtering
pub const UUID_FF17: uuid::Uuid = UUID_WRITE;
pub const UUID_FE2C: uuid::Uuid = UUID_FAST_PAIR;

// ── Command IDs (mapped from bluetrum SDK Command.java) ──
// These are the `COMMAND_*` constants, sent as request command bytes.

pub const CMD_EQ_SETTING: u8 = 0x20; // 32  COMMAND_EQ
pub const CMD_KEY: u8 = 0x22;        // 34  COMMAND_KEY
pub const CMD_AUTO_SHUTDOWN: u8 = 0x23;  // 35
pub const CMD_FACTORY_RESET: u8 = 0x24;  // 36
pub const CMD_WORK_MODE: u8 = 0x25;      // 37  COMMAND_WORK_MODE (game mode)
pub const CMD_IN_EAR_DETECT: u8 = 0x26;  // 38  COMMAND_IN_EAR_DETECT
pub const CMD_DEVICE_INFO: u8 = 0x27;    // 39  COMMAND_DEVICE_INFO
pub const CMD_NOTIFY: u8 = 0x28;         // 40  COMMAND_NOTIFY (notification from device)
pub const CMD_LANGUAGE: u8 = 0x29;       // 41  COMMAND_LANGUAGE
pub const CMD_FIND_DEVICE: u8 = 0x2A;    // 42  COMMAND_FIND_DEVICE
pub const CMD_AUTO_ANSWER: u8 = 0x2B;    // 43  COMMAND_AUTO_ANSWER
pub const CMD_ANC_MODE: u8 = 0x2C;       // 44  COMMAND_ANC_MODE
pub const CMD_BLUETOOTH_NAME: u8 = 0x2D; // 45  COMMAND_BLUETOOTH_NAME
pub const CMD_LED_MODE: u8 = 0x2E;       // 46  COMMAND_LED_MODE
pub const CMD_CLEAR_PAIR_RECORD: u8 = 0x2F; // 47
pub const CMD_ANC_GAIN: u8 = 0x30;       // 48  COMMAND_ANC_GAIN
pub const CMD_TRANS_GAIN: u8 = 0x31;     // 49  COMMAND_TRANSPARENCY_GAIN
pub const CMD_SOUND_EFFECT_3D: u8 = 0x32; // 50 COMMAND_SOUND_EFFECT_3D
pub const CMD_DUAL_DEVICE: u8 = 0x33;    // 51  COMMAND_DUAL_DEVICE
pub const CMD_AREA_TAP: u8 = 0x34;        // 52  COMMAND_AREA_TAP
pub const CMD_CHAT_MODE: u8 = 0x35;       // 53  COMMAND_CHAT_MODE
pub const CMD_SPACE_AUDIO: u8 = 0x36;     // 54  COMMAND_SPACE_AUDIO
pub const CMD_ENV_ADAPTIVE: u8 = 0x37;    // 55  COMMAND_ENV_ADAPTIVE
pub const CMD_LONG_ENDURANCE_MODE: u8 = 0x38; // 56 COMMAND_LONG_ENDURANCE_MODE
pub const CMD_TWS_PAIRING: u8 = 0x39;     // 57
pub const CMD_VOICE_PROMPT: u8 = 0x3A;    // 58  COMMAND_VOICE_PROMPT
pub const CMD_LED_INFO: u8 = 0x3B;        // 59  COMMAND_LED_INFO
pub const CMD_KARAOKE_EFFECT: u8 = 0x3C;  // 60  COMMAND_KARAOKE_EFFECT
pub const CMD_STEP_COUNT_SWITCH: u8 = 0x3D; // 61
pub const CMD_SYNC_TIME: u8 = 0x3E;        // 62
pub const CMD_RESET_SPORT: u8 = 0x3F;      // 63
pub const CMD_SPORT_HISTORY_DATA: u8 = 0x40; // 64
pub const CMD_BASS_EQ_SWITCH: u8 = 0x41;   // 65  COMMAND_BASS_EQ_SWITCH
pub const CMD_CANCEL_OTA: u8 = 0x42;       // 66
pub const CMD_TONE_VOLUME: u8 = 0x43;      // 67  COMMAND_TONE_VOLUME
pub const CMD_HEARING_CARE: u8 = 0x44;     // 68  COMMAND_HEARING_CARE
pub const CMD_ADAPTIVE_VOLUME: u8 = 0x45;  // 69  COMMAND_ADAPTIVE_VOLUME

// Backward-compatible aliases
pub const CMD_HANDSHAKE: u8 = CMD_DEVICE_INFO; // 0x27

// ── Info IDs (for TLV queries via handshake/device_info) ──
// These are the `INFO_*` constants — sent in TLV requests to query device state.

pub const ID_DEVICE_POWER: u8 = 0x01;        // 1   INFO_DEVICE_POWER
pub const ID_FIRMWARE_VERSION: u8 = 0x02;    // 2   INFO_FIRMWARE_VERSION
pub const ID_BLUETOOTH_NAME: u8 = 0x03;      // 3   INFO_BLUETOOTH_NAME
pub const ID_EQ_SETTING: u8 = 0x04;          // 4   INFO_EQ_SETTING
pub const ID_KEY_SETTINGS: u8 = 0x05;        // 5   INFO_KEY_SETTINGS
pub const ID_DEVICE_VOLUME: u8 = 0x06;       // 6   INFO_DEVICE_VOLUME
pub const ID_PLAY_STATE: u8 = 0x07;          // 7   INFO_PLAY_STATE
pub const ID_WORK_MODE: u8 = 0x08;           // 8   INFO_WORK_MODE
pub const ID_IN_EAR_STATUS: u8 = 0x09;       // 9   INFO_IN_EAR_STATUS
pub const ID_LANGUAGE_SETTING: u8 = 0x0A;    // 10
pub const ID_AUTO_ANSWER: u8 = 0x0B;         // 11
pub const ID_ANC_MODE: u8 = 0x0C;            // 12  INFO_ANC_MODE
pub const ID_IS_TWS: u8 = 0x0D;              // 13
pub const ID_TWS_CONNECTED: u8 = 0x0E;       // 14
pub const ID_LED_SWITCH: u8 = 0x0F;          // 15  INFO_LED_SWITCH
pub const ID_FW_CHECKSUM: u8 = 0x10;         // 16
pub const ID_ANC_GAIN: u8 = 0x11;            // 17  INFO_ANC_GAIN
pub const ID_TRANS_GAIN: u8 = 0x12;          // 18  INFO_TRANSPARENCY_GAIN
pub const ID_ANC_MAX: u8 = 0x13;             // 19  INFO_ANC_GAIN_NUM
pub const ID_TRANS_MAX: u8 = 0x14;           // 20  INFO_TRANSPARENCY_GAIN_NUM
pub const ID_ALL_EQ_SETTINGS: u8 = 0x15;     // 21
pub const ID_MAIN_SIDE: u8 = 0x16;           // 22
pub const ID_PRODUCT_COLOR: u8 = 0x17;       // 23
pub const ID_SOUND_EFFECT_3D: u8 = 0x18;     // 24  INFO_SOUND_EFFECT_3D
pub const ID_DUAL_DEVICE: u8 = 0x19;         // 25  INFO_DUAL_DEVICE
pub const ID_MULTIPOINT_INFO: u8 = 0x1A;     // 26
pub const ID_AREA_TAP: u8 = 0x1D;            // 29  INFO_AREA_TAP
pub const ID_CHAT_MODE: u8 = 0x1F;           // 31  INFO_CHAT_MODE
pub const ID_SPACE_AUDIO: u8 = 0x20;         // 32  INFO_SPACE_AUDIO
pub const ID_ENV_ADAPTIVE: u8 = 0x21;        // 33  INFO_ENV_ADAPTIVE
pub const ID_LONG_ENDURANCE_MODE: u8 = 0x22; // 34
pub const ID_TWS_PAIRING: u8 = 0x23;         // 35
pub const ID_VOICE_PROMPT: u8 = 0x24;        // 36  INFO_VOICE_PROMPT
pub const ID_LED_INFO: u8 = 0x25;            // 37  INFO_LED_INFO
pub const ID_KARAOKE_EFFECT: u8 = 0x26;      // 38  INFO_KARAOKE_EFFECT
pub const ID_STEP_COUNT_SWITCH: u8 = 0x27;   // 39
pub const ID_TONE_VOLUME: u8 = 0x2C;         // 44  INFO_TONE_VOLUME
pub const ID_HEARING_CARE: u8 = 0x2D;        // 45  INFO_HEARING_CARE
pub const ID_ADAPTIVE_VOLUME: u8 = 0x2E;     // 46  INFO_ADAPTIVE_VOLUME
pub const ID_BASS_EQ_SWITCH: u8 = 0x2B;      // 43  INFO_BASS_EQ_SWITCH

// ── Device Info / Handshake magic bytes ──
pub const HANDSHAKE_PREFIX: &[u8] = &[0xFF, 0x00];
