use uuid::Uuid;

// Command IDs
pub const CMD_HANDSHAKE: u8 = 0x27;
pub const CMD_ANC_MODE: u8 = 0x2C;
pub const CMD_ANC_GAIN: u8 = 0x30;
pub const CMD_TRANS_GAIN: u8 = 0x31;
pub const CMD_DUAL_DEVICE: u8 = 0x33;
pub const CMD_EQ_SETTING: u8 = 0x20;
pub const CMD_ENV_ADAPTIVE: u8 = 0x37;

// Info IDs (TLV tags)
pub const ID_BATTERY: u8 = 0x01;
pub const ID_ANC_MODE: u8 = 0x0C;
pub const ID_ANC_GAIN: u8 = 0x11;
pub const ID_TRANS_GAIN: u8 = 0x12;
pub const ID_ANC_MAX: u8 = 0x13;
pub const ID_TRANS_MAX: u8 = 0x14;
pub const ID_DUAL_DEVICE: u8 = 0x19;
pub const ID_EQ_SETTING: u8 = 0x04;
pub const ID_ENV_ADAPTIVE: u8 = 0x21;

pub const CMD_FACTORY_RESET: u8 = 0x24;
pub const CMD_WORK_MODE: u8 = 0x25;
pub const CMD_FIND_DEVICE: u8 = 0x2A;
pub const ID_WORK_MODE: u8 = 0x08;

// Message types
pub const TYPE_REQUEST: u8 = 0x01;
pub const TYPE_RESPONSE: u8 = 0x02;

// ANC modes
pub const MODE_OFF: u8 = 0;
pub const MODE_ANC: u8 = 1;
pub const MODE_TRANSPARENCY: u8 = 2;
// Mapped Key Types (From KeyRequest.java)
pub const KEY_LEFT_SINGLE_TAP: u8 = 1;
pub const KEY_RIGHT_SINGLE_TAP: u8 = 2;
pub const KEY_LEFT_DOUBLE_TAP: u8 = 3;
pub const KEY_RIGHT_DOUBLE_TAP: u8 = 4;
pub const KEY_LEFT_TRIPLE_TAP: u8 = 5;
pub const KEY_RIGHT_TRIPLE_TAP: u8 = 6;
pub const KEY_LEFT_LONG_PRESS: u8 = 7;
pub const KEY_RIGHT_LONG_PRESS: u8 = 8;

// Mapped Key Functions (From KeyRequest.java)
pub const KEY_FUNCTION_NONE: u8 = 0;
pub const KEY_FUNCTION_RECALL: u8 = 1;
pub const KEY_FUNCTION_ASSISTANT: u8 = 2;
pub const KEY_FUNCTION_PREVIOUS: u8 = 3;
pub const KEY_FUNCTION_NEXT: u8 = 4;
pub const KEY_FUNCTION_VOLUME_UP: u8 = 5;
pub const KEY_FUNCTION_VOLUME_DOWN: u8 = 6;
pub const KEY_FUNCTION_PLAY_PAUSE: u8 = 7;
pub const KEY_FUNCTION_GAME_MODE: u8 = 8;
pub const KEY_FUNCTION_ANC_MODE: u8 = 9;
pub const CMD_LANGUAGE_SET: u8 = 0x23;       // LanguageRequest
pub const CMD_CLEAR_PAIR: u8 = 0x28;         // ClearPairRecordRequest
pub const CMD_SYNC_TIME: u8 = 0x32;          // SyncTimeRequest
pub const CMD_AUTO_SHUTDOWN: u8 = 0x36;      // AutoShutdownRequest
pub const CMD_SPATIAL_AUDIO: u8 = 0x38;      // SpaceAudioRequest
pub const CMD_SOUND_EFFECT_3D: u8 = 0x39;    // SoundEffect3dRequest
pub const CMD_HEARING_CARE: u8 = 0x3A;       // HearingCareRequest
pub const CMD_TONE_VOLUME: u8 = 0x3B;        // ToneVolumeRequest
pub const CMD_ADAPTIVE_VOL: u8 = 0x3C;       // AdaptiveVolumeRequest
pub const CMD_KARAOKE_MODE: u8 = 0x3D;       // KaraokeRequest
pub const CMD_CHAT_MODE: u8 = 0x3E;          // ChatModeRequest
pub const CMD_LONG_ENDURANCE: u8 = 0x3F;     // LongEnduranceModeRequest
pub const CMD_AUTO_ANSWER: u8 = 0x41;        // AutoAnswerRequest
pub const CMD_STEP_COUNT: u8 = 0x43;         // StepCountSwitchRequest
pub const CMD_RESET_SPORT: u8 = 0x44;        // ResetSportRequest
// Missing Command IDs from Java requests
pub const CMD_KEY_SETTINGS: u8 = 34;
pub const CMD_IN_EAR_DETECT: u8 = 38;
pub const CMD_VOICE_PROMPT: u8 = 43;
pub const CMD_LED_SWITCH: u8 = 46;

// TLV Query IDs for state synchronization (matching Bluetrum protocol standards)
pub const ID_IN_EAR_DETECT: u8 = 0x26;
pub const ID_VOICE_PROMPT: u8 = 0x2B;
pub const ID_LED_SWITCH: u8 = 0x2E;

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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TouchGesture {
    LeftSingleTap = 1,
    RightSingleTap = 2,
    LeftDoubleTap = 3,
    RightDoubleTap = 4,
    LeftTripleTap = 5,
    RightTripleTap = 6,
    LeftLongPress = 7,
    RightLongPress = 8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum GestureFunction {
    None = 0,
    VoiceRecall = 1,
    Assistant = 2,
    PreviousTrack = 3,
    NextTrack = 4,
    VolumeUp = 5,
    VolumeDown = 6,
    PlayPause = 7,
    GameMode = 8,
    AncModeSwitch = 9,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum TouchSensitivity {
    Normal = 0,
    Low = 1,
    High = 2,
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

// TLV Parser
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