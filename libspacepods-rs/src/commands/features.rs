use crate::commands::BleCommand;
use crate::protocol::{TlvParser, CMD_DUAL_DEVICE, CMD_ENV_ADAPTIVE, CMD_HANDSHAKE, ID_DUAL_DEVICE, ID_ENV_ADAPTIVE};
use crate::{Error, Result};
use std::time::Duration;

// ── FeatureCommand ──

#[derive(Debug, Clone)]
pub enum FeatureCommand {
    GetAdaptiveAnc,
    SetAdaptiveAnc(bool),
    GetDualDevice,
    SetDualDevice(bool),
}

#[derive(Debug, Clone)]
pub enum FeatureResponse {
    AdaptiveAnc(bool),
    DualDevice(bool),
    Ack,
}

impl BleCommand for FeatureCommand {
    type Response = FeatureResponse;

    fn cmd_id(&self) -> u8 {
        match self {
            Self::GetAdaptiveAnc | Self::GetDualDevice => CMD_HANDSHAKE,
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
}
