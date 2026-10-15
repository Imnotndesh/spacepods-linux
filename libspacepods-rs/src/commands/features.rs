use crate::commands::BleCommand;
use crate::protocol::{TlvParser, CMD_DUAL_DEVICE, CMD_ENV_ADAPTIVE, CMD_HANDSHAKE, ID_AREA_TAP, ID_DUAL_DEVICE, ID_ENV_ADAPTIVE, ID_KEY_SETTINGS, ID_SOUND_EFFECT_3D};
use crate::{Error, Result};
use std::time::Duration;

// ── FeatureCommand ──

#[derive(Debug, Clone)]
pub enum FeatureCommand {
    GetAdaptiveAnc,
    SetAdaptiveAnc(bool),
    GetDualDevice,
    SetDualDevice(bool),
    GetKeySettings,
    /// Area tap state, from device info 29 (the same id the official app reads).
    GetAreaTap,
    /// 3D (space) audio state, from device info 24.
    GetSpaceAudio,
}

#[derive(Debug, Clone)]
pub enum FeatureResponse {
    AdaptiveAnc(bool),
    DualDevice(bool),
    AreaTap(bool),
    SpaceAudio(bool),
    KeySettings(std::collections::HashMap<u8, u8>),
    Ack,
}

impl BleCommand for FeatureCommand {
    type Response = FeatureResponse;

    fn cmd_id(&self) -> u8 {
        match self {
            Self::GetAdaptiveAnc | Self::GetDualDevice | Self::GetKeySettings | Self::GetAreaTap | Self::GetSpaceAudio => CMD_HANDSHAKE,
            Self::SetAdaptiveAnc(_) => CMD_ENV_ADAPTIVE,
            Self::SetDualDevice(_) => CMD_DUAL_DEVICE,
        }
    }

    fn encode(&self) -> Vec<u8> {
        match self {
            Self::GetAdaptiveAnc => vec![0xFF, 0x00, ID_ENV_ADAPTIVE, 0x00],
            Self::SetAdaptiveAnc(enabled) => vec![if *enabled { 0x01 } else { 0x00 }],
            Self::GetDualDevice => vec![0xFF, 0x00, ID_DUAL_DEVICE, 0x00],
            Self::SetDualDevice(enabled) => vec![0x01, 0x02, if *enabled { 0x01 } else { 0x00 }],
            Self::GetKeySettings => vec![ID_KEY_SETTINGS, 0x00],
            Self::GetAreaTap => vec![0xFF, 0x00, ID_AREA_TAP, 0x00],
            Self::GetSpaceAudio => vec![0xFF, 0x00, ID_SOUND_EFFECT_3D, 0x00],
        }
    }

    fn decode(&self, payload: &[u8]) -> Result<Self::Response> {
        match self {
            Self::GetAdaptiveAnc => {
                let mut parser = TlvParser::new(payload);
                let val = parser.get_int(ID_ENV_ADAPTIVE).unwrap_or(0);
                Ok(FeatureResponse::AdaptiveAnc(val == 1))
            }
            Self::GetDualDevice => {
                let mut parser = TlvParser::new(payload);
                let val = parser.get_int(ID_DUAL_DEVICE).unwrap_or(0);
                Ok(FeatureResponse::DualDevice(val == 1))
            }
            Self::GetAreaTap => {
                let mut parser = TlvParser::new(payload);
                let val = parser.get_int(ID_AREA_TAP).unwrap_or(0);
                Ok(FeatureResponse::AreaTap(val == 1))
            }
            Self::GetSpaceAudio => {
                let mut parser = TlvParser::new(payload);
                let val = parser.get_int(ID_SOUND_EFFECT_3D).unwrap_or(0);
                Ok(FeatureResponse::SpaceAudio(val == 1))
            }
            Self::GetKeySettings => {
                // DeviceInfo response: TLV format [info_id, len, ...data] repeated
                let mut map = std::collections::HashMap::new();
                let mut parser = TlvParser::new(payload);
                // TlvParser handles the TLV parsing — each info_id is the key
                if let Some(data) = parser.get_bytes(ID_KEY_SETTINGS) {
                    // Key settings sub-payload: [keyType, 0x01, keyFunction] repeated
                    let mut i = 0;
                    while i + 2 < data.len() {
                        let key_type = data[i];
                        let len = data[i + 1] as usize;
                        if len == 1 && i + 3 <= data.len() {
                            map.insert(key_type, data[i + 2]);
                        }
                        i = i + 2 + len;
                    }
                }
                Ok(FeatureResponse::KeySettings(map))
            }
            Self::SetAdaptiveAnc(_) | Self::SetDualDevice(_) => Ok(FeatureResponse::Ack),
        }
    }
}

// ── FeatureController ──

pub struct FeatureController<'a> {
    pub(crate) buds: &'a crate::SpaceBuds,
}

/// Map a chip-agnostic feature name to a raw Bluetrum command.
///
/// Used by the daemon's `set_feature` IPC handler for Bluetrum devices. The
/// JieLi equivalent is `crate::protocol::OraFeature`.
pub fn bluetrum_feature_command(feature: &str, enabled: bool) -> Option<(u8, Vec<u8>)> {
    use crate::protocol::constants::*;
    let on = if enabled { 0x01u8 } else { 0x00u8 };
    Some(match feature {
        "game_mode" => (CMD_WORK_MODE, vec![on]),
        "chat_mode" => (CMD_CHAT_MODE, vec![on]),
        "area_tap" => (CMD_AREA_TAP, vec![on]),
        "in_ear_detection" => (CMD_IN_EAR_DETECT, vec![on]),
        "adaptive_volume" => (CMD_ADAPTIVE_VOLUME, vec![on]),
        "voice_prompt" => (CMD_VOICE_PROMPT, vec![on]),
        "space_audio" | "3d_audio" => (CMD_SOUND_EFFECT_3D, vec![on]),
        "long_endurance" => (CMD_LONG_ENDURANCE_MODE, vec![on]),
        "karaoke" => (CMD_KARAOKE_EFFECT, vec![on]),
        "bass_eq" => (CMD_BASS_EQ_SWITCH, vec![on]),
        "dual_device" => (CMD_DUAL_DEVICE, vec![0x01, 0x02, on]),
        _ => return None,
    })
}

impl FeatureController<'_> {
    /// Area tap state. Bluetrum only; JieLi reads go through `OraFeature`.
    pub async fn get_area_tap(&self) -> Result<bool> {
        match self.buds.manager.send(&FeatureCommand::GetAreaTap).await? {
            FeatureResponse::AreaTap(v) => Ok(v),
            _ => Err(Error::Parse("Unexpected response for get_area_tap")),
        }
    }

    /// 3D (space) audio state. Bluetrum only.
    pub async fn get_space_audio(&self) -> Result<bool> {
        match self.buds.manager.send(&FeatureCommand::GetSpaceAudio).await? {
            FeatureResponse::SpaceAudio(v) => Ok(v),
            _ => Err(Error::Parse("Unexpected response for get_space_audio")),
        }
    }

    pub async fn get_adaptive_anc(&self) -> Result<bool> {
        if self.buds.manager.is_jieli().await {
            return Ok(self
                .buds
                .manager
                .jieli_get_feature(crate::protocol::OraFeature::NoiseEnv)
                .await?
                .unwrap_or(false));
        }
        let resp = self.buds.manager.send(&FeatureCommand::GetAdaptiveAnc).await?;
        match resp {
            FeatureResponse::AdaptiveAnc(v) => Ok(v),
            _ => Err(Error::Parse("Unexpected response for get_adaptive_anc")),
        }
    }

    pub async fn set_adaptive_anc(&self, enable: bool) -> Result<()> {
        if self.buds.manager.is_jieli().await {
            return self
                .buds
                .manager
                .jieli_set_feature(crate::protocol::OraFeature::NoiseEnv, enable)
                .await;
        }
        self.buds.manager.send(&FeatureCommand::SetAdaptiveAnc(enable)).await?;
        tokio::time::sleep(Duration::from_millis(300)).await;
        Ok(())
    }

    pub async fn get_dual_device(&self) -> Result<bool> {
        if self.buds.manager.is_jieli().await {
            return Ok(self
                .buds
                .manager
                .jieli_get_feature(crate::protocol::OraFeature::DualDevice)
                .await?
                .unwrap_or(false));
        }
        let resp = self.buds.manager.send(&FeatureCommand::GetDualDevice).await?;
        match resp {
            FeatureResponse::DualDevice(v) => Ok(v),
            _ => Err(Error::Parse("Unexpected response for get_dual_device")),
        }
    }

    pub async fn set_dual_device(&self, enable: bool) -> Result<()> {
        if self.buds.manager.is_jieli().await {
            self.buds.manager.jieli_set_dual_device(enable).await?;
            return Ok(());
        }
        self.buds.manager.send(&FeatureCommand::SetDualDevice(enable)).await?;
        tokio::time::sleep(Duration::from_millis(300)).await;
        Ok(())
    }

    pub async fn get_key_settings(&self) -> Result<std::collections::HashMap<u8, u8>> {
        let resp = self.buds.manager.send(&FeatureCommand::GetKeySettings).await?;
        match resp {
            FeatureResponse::KeySettings(map) => Ok(map),
            _ => Err(Error::Parse("Unexpected response for get_key_settings")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::constants::{CMD_AREA_TAP, CMD_SOUND_EFFECT_3D};

    // Mock device replies: the TLV reply the earbuds send for a device-info query, with
    // the info id, a length of 1, and the value. These are built by hand, so the tests
    // run without earbuds.
    fn reply(id: u8, value: u8) -> Vec<u8> {
        vec![id, 0x01, value]
    }

    #[test]
    fn area_tap_reads_on_and_off() {
        let on = FeatureCommand::GetAreaTap.decode(&reply(ID_AREA_TAP, 1)).unwrap();
        let off = FeatureCommand::GetAreaTap.decode(&reply(ID_AREA_TAP, 0)).unwrap();
        assert!(matches!(on, FeatureResponse::AreaTap(true)));
        assert!(matches!(off, FeatureResponse::AreaTap(false)));
    }

    #[test]
    fn space_audio_reads_on_and_off() {
        let on = FeatureCommand::GetSpaceAudio.decode(&reply(ID_SOUND_EFFECT_3D, 1)).unwrap();
        let off = FeatureCommand::GetSpaceAudio.decode(&reply(ID_SOUND_EFFECT_3D, 0)).unwrap();
        assert!(matches!(on, FeatureResponse::SpaceAudio(true)));
        assert!(matches!(off, FeatureResponse::SpaceAudio(false)));
    }

    #[test]
    fn missing_reply_reads_as_off() {
        // An empty reply must not be read as on.
        assert!(matches!(FeatureCommand::GetAreaTap.decode(&[]).unwrap(), FeatureResponse::AreaTap(false)));
        assert!(matches!(FeatureCommand::GetSpaceAudio.decode(&[]).unwrap(), FeatureResponse::SpaceAudio(false)));
    }

    #[test]
    fn read_queries_use_the_info_ids_from_the_app() {
        // The official app reads area tap from info 29 and 3D audio from info 24.
        assert_eq!(FeatureCommand::GetAreaTap.encode(), vec![0xFF, 0x00, 29, 0x00]);
        assert_eq!(FeatureCommand::GetSpaceAudio.encode(), vec![0xFF, 0x00, 24, 0x00]);
    }

    #[test]
    fn set_commands_map_to_the_app_command_ids() {
        assert_eq!(bluetrum_feature_command("area_tap", true), Some((CMD_AREA_TAP, vec![0x01])));
        assert_eq!(bluetrum_feature_command("space_audio", false), Some((CMD_SOUND_EFFECT_3D, vec![0x00])));
    }
}
