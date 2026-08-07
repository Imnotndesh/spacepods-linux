use crate::commands::BleCommand;
use crate::protocol::{TlvParser, CMD_DUAL_DEVICE, CMD_ENV_ADAPTIVE, CMD_HANDSHAKE, ID_DUAL_DEVICE, ID_ENV_ADAPTIVE, ID_KEY_SETTINGS};
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
}

#[derive(Debug, Clone)]
pub enum FeatureResponse {
    AdaptiveAnc(bool),
    DualDevice(bool),
    KeySettings(std::collections::HashMap<u8, u8>),
    Ack,
}

impl BleCommand for FeatureCommand {
    type Response = FeatureResponse;

    fn cmd_id(&self) -> u8 {
        match self {
            Self::GetAdaptiveAnc | Self::GetDualDevice | Self::GetKeySettings => CMD_HANDSHAKE,
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

impl FeatureController<'_> {
    pub async fn get_adaptive_anc(&self) -> Result<bool> {
        let resp = self.buds.manager.send(&FeatureCommand::GetAdaptiveAnc).await?;
        match resp {
            FeatureResponse::AdaptiveAnc(v) => Ok(v),
            _ => Err(Error::Parse("Unexpected response for get_adaptive_anc")),
        }
    }

    pub async fn set_adaptive_anc(&self, enable: bool) -> Result<()> {
        self.buds.manager.send(&FeatureCommand::SetAdaptiveAnc(enable)).await?;
        tokio::time::sleep(Duration::from_millis(300)).await;
        Ok(())
    }

    pub async fn get_dual_device(&self) -> Result<bool> {
        let resp = self.buds.manager.send(&FeatureCommand::GetDualDevice).await?;
        match resp {
            FeatureResponse::DualDevice(v) => Ok(v),
            _ => Err(Error::Parse("Unexpected response for get_dual_device")),
        }
    }

    pub async fn set_dual_device(&self, enable: bool) -> Result<()> {
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
