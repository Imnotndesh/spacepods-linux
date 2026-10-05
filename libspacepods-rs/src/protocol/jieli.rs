//! JieLi **RCSP** protocol framing and Oraimo custom commands.
//!
//! RCSP is carried over the JieLi GATT service (`0000ae00`) with a write
//! characteristic (`0000ae01`) and a notify characteristic (`0000ae02`).
//!
//! Wire format (ported from `com.jieli.bluetooth.tool.ParseHelper`):
//!
//! ```text
//! FE DC BA | flags | opcode | len_hi len_lo | payload | EF
//! ```
//!
//! * `flags` bit 7 = type (1 = command, 0 = response), bit 6 = needs response.
//! * `len` is the payload length, big-endian.
//! * command payload  = `[op_code_sn][xm_opcode?][param…]`
//! * response payload = `[status][op_code_sn][xm_opcode?][param…]`
//!
//! Oraimo's device features are carried as `CMD_CUSTOM` (0xFF) commands whose
//! `param` starts with an Oraimo command ID (`DeviceModelJlImpl`).

use crate::protocol::e1;

pub const RCSP_HEAD: [u8; 3] = [0xFE, 0xDC, 0xBA];
pub const RCSP_TAIL: u8 = 0xEF;
pub const FLAG_COMMAND: u8 = 0x80;
pub const FLAG_HAS_RESPONSE: u8 = 0x40;

/// RCSP opcodes we use.
pub const CMD_DATA: u8 = 0x01;
pub const CMD_GET_TARGET_INFO: u8 = 0x03;
pub const CMD_RESET_AUTH_FLAG: u8 = 0x06;
pub const CMD_GET_SYS_INFO: u8 = 0x07;
pub const CMD_SET_SYS_INFO: u8 = 0x08;
pub const CMD_ADV_SETTINGS: u8 = 0xC0;
pub const CMD_CUSTOM: u8 = 0xFF;

/// `modifyDeviceSettingsInfo` sub-types (LTV packets under `CMD_ADV_SETTINGS`).
pub const ADV_TYPE_RENAME: u8 = 1;
pub const ADV_TYPE_KEY_SETTINGS: u8 = 2;
pub const ADV_TYPE_LED_SETTINGS: u8 = 3;
pub const ADV_TYPE_EAR_MODE: u8 = 5;

/// Build an LTV packet: `[len+1][type][data…]` (`ParseDataUtil.packLTVPacket`).
pub fn ltv_packet(ltv_type: u8, data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() + 2);
    out.push((data.len() + 1) as u8);
    out.push(ltv_type);
    out.extend_from_slice(data);
    out
}

/// JieLi's fixed authentication pass response.
pub const AUTH_PASS: [u8; 5] = [0x02, b'p', b'a', b's', b's'];

/// Sys-info attribute carrying the current voice (noise-control) mode.
pub const ATTR_VOICE_MODE: u8 = 13;
/// Bitmask for the current voice-mode attribute (`buildGetCurrentVoiceMode`).
pub const MASK_VOICE_MODE: u32 = 1 << 13;

/// Build a `SET_SYS_INFO` payload that sets the current voice mode.
///
/// `VoiceMode.getBytes()` is `[mode, leftMax(2), rightMax(2), leftCur(2),
/// rightCur(2)]` and the attribute is wrapped as `[len+1][type][data…]`
/// (`AttrBean.toData`) behind the `0xFF` function byte.
pub fn voice_mode_set_param(
    mode: u8,
    left_max: u16,
    right_max: u16,
    left_cur: u16,
    right_cur: u16,
) -> Vec<u8> {
    let mut vm = Vec::with_capacity(9);
    vm.push(mode);
    vm.extend_from_slice(&left_max.to_be_bytes());
    vm.extend_from_slice(&right_max.to_be_bytes());
    vm.extend_from_slice(&left_cur.to_be_bytes());
    vm.extend_from_slice(&right_cur.to_be_bytes());

    let mut payload = vec![0xFF]; // function = -1
    payload.push((vm.len() + 1) as u8);
    payload.push(ATTR_VOICE_MODE);
    payload.extend_from_slice(&vm);
    payload
}

/// Build a `GET_SYS_INFO` payload: `[function][mask:u32 big-endian]`.
pub fn get_sys_info_param(function: u8, mask: u32) -> Vec<u8> {
    let mut p = vec![function];
    p.extend_from_slice(&mask.to_be_bytes());
    p
}

/// Find an attribute value in a sys-info payload/report
/// (`[function][len+1][type][data…]…`).
pub fn find_attr(param: &[u8], attr_type: u8) -> Option<&[u8]> {
    if param.len() < 2 {
        return None;
    }
    let mut i = 1; // skip the function byte
    while i + 2 <= param.len() {
        let len = param[i] as usize;
        if len == 0 {
            break;
        }
        let ty = param[i + 1];
        let data_len = len - 1;
        let start = i + 2;
        if start + data_len > param.len() {
            break;
        }
        if ty == attr_type {
            return Some(&param[start..start + data_len]);
        }
        i = start + data_len;
    }
    None
}

/// Oraimo custom command IDs.
///
/// Sourced from `SDKUtils.onRcspCommand` (notification parsing) and the
/// `buildCustomCmd(new byte[]{id, …})` call sites in `DeviceActivity` /
/// `DeviceNewActivity`. Queries are `{id, 0}`; sets are `{id, 2}` (on) or
/// `{id, 1}` (off); the device reports state as `{id, 0|2, …}`.
pub mod ora {
    /// Game mode toggle.
    pub const GAME_MODE: u8 = 1;
    /// Gesture / ear-control switch.
    pub const GESTURE: u8 = 2;
    /// Noise control. Set: `[3, mode]` or `[3, mode, hi, lo]`.
    /// Report: `[3, mode, hi, lo]`.
    pub const ANC: u8 = 3;
    /// Gesture OTA (version/start/data/result).
    pub const GESTURE_OTA: u8 = 4;
    /// Ear auto (wear auto play/pause).
    pub const EAR_MODE: u8 = 5;
    /// Adaptive noise control (environment mode).
    pub const NOISE_ENV: u8 = 6;
    /// Chat mode (voice passthrough).
    pub const CHAT_MODE: u8 = 7;
    /// Wind-noise detection.
    pub const WIND_NOISE: u8 = 8;
    /// Area tap switch.
    pub const AREA_TAP: u8 = 9;
    /// Multipoint / dual device toggle.
    pub const DUAL_DEVICE: u8 = 10;
    /// LED info.
    pub const LED: u8 = 11;
    /// Tone info (`[12, …]`).
    pub const TONE_INFO: u8 = 12;
    /// LED info (300 family).
    pub const LED_300: u8 = 14;
    /// Long-endurance mode.
    pub const LONG_ENDURANCE: u8 = 15;
    /// 3D / spatial audio.
    pub const SPACE_AUDIO: u8 = 16;
    /// Magnet switch.
    pub const MAGNET: u8 = 17;
    /// Vibration.
    pub const VIBRATION: u8 = 18;
    /// Voice prompt.
    pub const VOICE_PROMPT: u8 = 19;
    /// FM radio switch.
    pub const FM: u8 = 20;
    /// FM favourites list.
    pub const FM_FAVORITES: u8 = 22;
    /// Warehouse / shipping sleep state.
    pub const WAREHOUSE_SLEEP: u8 = 23;
    /// Adaptive volume.
    pub const VOLUME_ADAPTIVE: u8 = 24;
    /// Multipoint device list (query/report).
    pub const DUAL_DEVICE_INFO: u8 = 25;
    /// Tone volume (`[27, level]`).
    pub const TONE_VOLUME: u8 = 27;
    /// LDAC toggle.
    pub const LDAC: u8 = 28;
    /// In-ear detection toggle.
    pub const IN_EAR_DETECTION: u8 = 29;
    /// Voice control.
    pub const VOICE_CONTROL: u8 = 30;
    /// LED info (671 family).
    pub const LED_671: u8 = 31;
}

/// A boolean Oraimo feature that is queried/set with a single command id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OraFeature {
    GameMode,
    Gesture,
    EarMode,
    NoiseEnv,
    ChatMode,
    WindNoise,
    AreaTap,
    DualDevice,
    LongEndurance,
    SpaceAudio,
    Magnet,
    Vibration,
    VoicePrompt,
    Fm,
    VolumeAdaptive,
    Ldac,
    InEarDetection,
    VoiceControl,
}

impl OraFeature {
    pub fn command_id(self) -> u8 {
        match self {
            OraFeature::GameMode => ora::GAME_MODE,
            OraFeature::Gesture => ora::GESTURE,
            OraFeature::EarMode => ora::EAR_MODE,
            OraFeature::NoiseEnv => ora::NOISE_ENV,
            OraFeature::ChatMode => ora::CHAT_MODE,
            OraFeature::WindNoise => ora::WIND_NOISE,
            OraFeature::AreaTap => ora::AREA_TAP,
            OraFeature::DualDevice => ora::DUAL_DEVICE,
            OraFeature::LongEndurance => ora::LONG_ENDURANCE,
            OraFeature::SpaceAudio => ora::SPACE_AUDIO,
            OraFeature::Magnet => ora::MAGNET,
            OraFeature::Vibration => ora::VIBRATION,
            OraFeature::VoicePrompt => ora::VOICE_PROMPT,
            OraFeature::Fm => ora::FM,
            OraFeature::VolumeAdaptive => ora::VOLUME_ADAPTIVE,
            OraFeature::Ldac => ora::LDAC,
            OraFeature::InEarDetection => ora::IN_EAR_DETECTION,
            OraFeature::VoiceControl => ora::VOICE_CONTROL,
        }
    }

    pub fn from_command_id(id: u8) -> Option<Self> {
        Some(match id {
            ora::GAME_MODE => OraFeature::GameMode,
            ora::GESTURE => OraFeature::Gesture,
            ora::EAR_MODE => OraFeature::EarMode,
            ora::NOISE_ENV => OraFeature::NoiseEnv,
            ora::CHAT_MODE => OraFeature::ChatMode,
            ora::WIND_NOISE => OraFeature::WindNoise,
            ora::AREA_TAP => OraFeature::AreaTap,
            ora::DUAL_DEVICE => OraFeature::DualDevice,
            ora::LONG_ENDURANCE => OraFeature::LongEndurance,
            ora::SPACE_AUDIO => OraFeature::SpaceAudio,
            ora::MAGNET => OraFeature::Magnet,
            ora::VIBRATION => OraFeature::Vibration,
            ora::VOICE_PROMPT => OraFeature::VoicePrompt,
            ora::FM => OraFeature::Fm,
            ora::VOLUME_ADAPTIVE => OraFeature::VolumeAdaptive,
            ora::LDAC => OraFeature::Ldac,
            ora::IN_EAR_DETECTION => OraFeature::InEarDetection,
            ora::VOICE_CONTROL => OraFeature::VoiceControl,
            _ => return None,
        })
    }

    /// Stable, chip-agnostic name used by the IPC `set_feature` command.
    pub fn name(self) -> &'static str {
        match self {
            OraFeature::GameMode => "game_mode",
            OraFeature::Gesture => "gesture",
            OraFeature::EarMode => "ear_mode",
            OraFeature::NoiseEnv => "adaptive_anc",
            OraFeature::ChatMode => "chat_mode",
            OraFeature::WindNoise => "wind_noise",
            OraFeature::AreaTap => "area_tap",
            OraFeature::DualDevice => "dual_device",
            OraFeature::LongEndurance => "long_endurance",
            OraFeature::SpaceAudio => "space_audio",
            OraFeature::Magnet => "magnet",
            OraFeature::Vibration => "vibration",
            OraFeature::VoicePrompt => "voice_prompt",
            OraFeature::Fm => "fm",
            OraFeature::VolumeAdaptive => "adaptive_volume",
            OraFeature::Ldac => "ldac",
            OraFeature::InEarDetection => "in_ear_detection",
            OraFeature::VoiceControl => "voice_control",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "game_mode" => OraFeature::GameMode,
            "gesture" => OraFeature::Gesture,
            "ear_mode" => OraFeature::EarMode,
            "adaptive_anc" => OraFeature::NoiseEnv,
            "chat_mode" => OraFeature::ChatMode,
            "wind_noise" => OraFeature::WindNoise,
            "area_tap" => OraFeature::AreaTap,
            "dual_device" => OraFeature::DualDevice,
            "long_endurance" => OraFeature::LongEndurance,
            "space_audio" | "3d_audio" => OraFeature::SpaceAudio,
            "magnet" => OraFeature::Magnet,
            "vibration" => OraFeature::Vibration,
            "voice_prompt" => OraFeature::VoicePrompt,
            "fm" => OraFeature::Fm,
            "adaptive_volume" => OraFeature::VolumeAdaptive,
            "ldac" => OraFeature::Ldac,
            "in_ear_detection" => OraFeature::InEarDetection,
            "voice_control" => OraFeature::VoiceControl,
            _ => return None,
        })
    }
}

/// Encode a boolean toggle payload (`2` = on, `1` = off).
pub fn toggle_payload(command_id: u8, enabled: bool) -> Vec<u8> {
    vec![command_id, if enabled { 2 } else { 1 }]
}

/// Encode a state query payload (`{command_id, 0}`).
pub fn query_payload(command_id: u8) -> Vec<u8> {
    vec![command_id, 0]
}

/// A parsed or to-be-sent RCSP packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RcspPacket {
    pub is_command: bool,
    pub has_response: bool,
    pub op_code: u8,
    pub op_code_sn: u8,
    pub status: u8,
    pub xm_op_code: u8,
    pub param: Vec<u8>,
}

impl RcspPacket {
    /// Build a command packet. `param` excludes the sequence byte.
    pub fn command(op_code: u8, sn: u8, param: Vec<u8>) -> Self {
        Self {
            is_command: true,
            has_response: true,
            op_code,
            op_code_sn: sn,
            status: 0,
            xm_op_code: 0,
            param,
        }
    }

    /// Build an Oraimo custom command (`CMD_CUSTOM`).
    pub fn custom(sn: u8, payload: Vec<u8>) -> Self {
        Self::command(CMD_CUSTOM, sn, payload)
    }

    /// Serialize to the on-wire frame.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut payload = Vec::new();
        if !self.is_command {
            payload.push(self.status);
        }
        payload.push(self.op_code_sn);
        if self.op_code == CMD_DATA {
            payload.push(self.xm_op_code);
        }
        payload.extend_from_slice(&self.param);

        let mut flags = 0u8;
        if self.is_command {
            flags |= FLAG_COMMAND;
        }
        if self.has_response {
            flags |= FLAG_HAS_RESPONSE;
        }

        let mut out = Vec::with_capacity(payload.len() + 8);
        out.extend_from_slice(&RCSP_HEAD);
        out.push(flags);
        out.push(self.op_code);
        out.extend_from_slice(&(payload.len() as u16).to_be_bytes());
        out.extend_from_slice(&payload);
        out.push(RCSP_TAIL);
        out
    }

    /// Parse a complete frame (including the `FE DC BA` magic and `EF` tail).
    pub fn parse(frame: &[u8]) -> Option<Self> {
        if frame.len() < 8 || frame[..3] != RCSP_HEAD {
            return None;
        }
        let flags = frame[3];
        let op_code = frame[4];
        let param_len = u16::from_be_bytes([frame[5], frame[6]]) as usize;
        if frame.len() < 7 + param_len + 1 || frame[7 + param_len] != RCSP_TAIL {
            return None;
        }
        let body = &frame[7..7 + param_len];
        let is_command = flags & FLAG_COMMAND != 0;
        let mut packet = RcspPacket {
            is_command,
            has_response: flags & FLAG_HAS_RESPONSE != 0,
            op_code,
            op_code_sn: 0,
            status: 0,
            xm_op_code: 0,
            param: Vec::new(),
        };
        if !body.is_empty() {
            let mut i = 0;
            if !is_command {
                packet.status = body[0];
                i = 1;
            }
            if i >= body.len() {
                return None;
            }
            packet.op_code_sn = body[i];
            i += 1;
            if op_code == CMD_DATA {
                if i >= body.len() {
                    return None;
                }
                packet.xm_op_code = body[i];
                i += 1;
            }
            packet.param = body[i..].to_vec();
        }
        Some(packet)
    }
}

/// Events produced by the reassembler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RcspEvent {
    /// A complete RCSP frame.
    Packet(RcspPacket),
    /// Bytes that are not RCSP framing (e.g. raw authentication data).
    Raw(Vec<u8>),
}

/// Stream reassembler for BLE notifications, which may fragment or concatenate
/// RCSP frames.
#[derive(Default)]
pub struct RcspStream {
    buf: Vec<u8>,
}

const MAX_BUF: usize = 8192;

impl RcspStream {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, data: &[u8]) -> Vec<RcspEvent> {
        self.buf.extend_from_slice(data);
        let mut events = Vec::new();
        loop {
            if self.buf.is_empty() {
                break;
            }
            match self.buf.windows(3).position(|w| w == RCSP_HEAD) {
                Some(0) => {
                    if self.buf.len() < 7 {
                        break;
                    }
                    let param_len = u16::from_be_bytes([self.buf[5], self.buf[6]]) as usize;
                    let total = 7 + param_len + 1;
                    if self.buf.len() < total {
                        break;
                    }
                    if self.buf[7 + param_len] != RCSP_TAIL {
                        // Corrupt tail: drop one byte and resync.
                        self.buf.remove(0);
                        continue;
                    }
                    let frame: Vec<u8> = self.buf.drain(..total).collect();
                    if let Some(packet) = RcspPacket::parse(&frame) {
                        events.push(RcspEvent::Packet(packet));
                    }
                }
                Some(pos) => {
                    let raw: Vec<u8> = self.buf.drain(..pos).collect();
                    events.push(RcspEvent::Raw(raw));
                }
                None => {
                    // Hold raw bytes for the caller (e.g. auth), bounded.
                    if self.buf.len() > MAX_BUF {
                        let drop = self.buf.len() - MAX_BUF;
                        self.buf.drain(..drop);
                    }
                    break;
                }
            }
        }
        events
    }

    /// Drain any held non-RCSP bytes (used after the auth handshake).
    pub fn take_raw(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.buf)
    }
}

/// The RCSP "reset auth flag" frame, sent before authentication.
pub fn auth_reset_frame() -> Vec<u8> {
    RcspPacket::command(CMD_RESET_AUTH_FLAG, 0, vec![0x01]).to_bytes()
}

/// Compute the response to a device challenge.
pub fn auth_response(challenge: &[u8]) -> [u8; 16] {
    e1::jieli_encrypt(challenge)
}

/// Generate a 16-byte random challenge (non-cryptographic; the auth scheme is
/// a fixed-key challenge/response, not a security boundary here).
pub fn random16() -> [u8; 16] {
    use std::time::{SystemTime, UNIX_EPOCH};
    let mut seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9E37_79B9_7F4A_7C15)
        ^ (std::process::id() as u64) << 32;
    let mut out = [0u8; 16];
    for chunk in out.chunks_mut(8) {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let bytes = seed.to_le_bytes();
        chunk.copy_from_slice(&bytes[..chunk.len()]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reset_auth_frame_bytes() {
        // Quarkslab observed FEDCBAC00600020001EF.
        assert_eq!(
            auth_reset_frame(),
            vec![0xFE, 0xDC, 0xBA, 0xC0, 0x06, 0x00, 0x02, 0x00, 0x01, 0xEF]
        );
    }

    #[test]
    fn test_custom_roundtrip() {
        let packet = RcspPacket::custom(5, vec![ora::ANC, 1]);
        let bytes = packet.to_bytes();
        // FE DC BA C0 FF 00 03 05 03 01 EF
        assert_eq!(
            bytes,
            vec![0xFE, 0xDC, 0xBA, 0xC0, 0xFF, 0x00, 0x03, 0x05, 0x03, 0x01, 0xEF]
        );
        let parsed = RcspPacket::parse(&bytes).unwrap();
        assert_eq!(parsed, packet);
    }

    #[test]
    fn test_response_parse() {
        // status=0, sn=5, param=[0x03, 0x02]
        let frame = vec![
            0xFE, 0xDC, 0xBA, 0x00, 0xFF, 0x00, 0x04, 0x00, 0x05, 0x03, 0x02, 0xEF,
        ];
        let parsed = RcspPacket::parse(&frame).unwrap();
        assert!(!parsed.is_command);
        assert_eq!(parsed.status, 0);
        assert_eq!(parsed.op_code_sn, 5);
        assert_eq!(parsed.param, vec![0x03, 0x02]);
    }

    #[test]
    fn test_stream_reassembly() {
        let packet = RcspPacket::custom(1, vec![0x03, 0x01]);
        let bytes = packet.to_bytes();
        let mut stream = RcspStream::new();
        // Split in the middle of the frame.
        let mut events = stream.push(&bytes[..5]);
        assert!(events.is_empty());
        events.extend(stream.push(&bytes[5..]));
        assert_eq!(events, vec![RcspEvent::Packet(packet)]);
    }
}
