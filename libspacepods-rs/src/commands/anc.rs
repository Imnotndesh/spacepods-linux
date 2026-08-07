use crate::commands::BleCommand;
use crate::protocol::{AncMode, TlvParser, *};
use crate::{Error, Result};
use std::time::Duration;

// ── AncCommand enum ──

#[derive(Debug, Clone)]
pub enum AncCommand {
    GetMode,
    SetMode(AncMode),
    GetLevel,
    SetLevel(u8),
    SetAncLevel(u8),
    SetTransparencyLevel(u8),
}

#[derive(Debug, Clone)]
pub enum AncResponse {
    Mode(AncMode),
    Level { current: u8, max: u8 },
    Ack,
}

impl BleCommand for AncCommand {
    type Response = AncResponse;

    fn cmd_id(&self) -> u8 {
        match self {
            Self::GetMode => CMD_HANDSHAKE,
            Self::SetMode(_) => CMD_ANC_MODE,
            Self::GetLevel => CMD_HANDSHAKE,
            Self::SetLevel(_) => CMD_ANC_MODE, // inferred from current mode
            Self::SetAncLevel(_) => CMD_ANC_GAIN,
            Self::SetTransparencyLevel(_) => CMD_TRANS_GAIN,
        }
    }

    fn encode(&self) -> Vec<u8> {
        match self {
            Self::GetMode => vec![0xFF, 0x00, ID_ANC_MODE, 0x00],
            Self::SetMode(mode) => vec![(*mode).into()],
            Self::GetLevel => vec![
                0xFF, 0x00,
                ID_ANC_MODE, 0x00,
                ID_ANC_GAIN, 0x00,
                ID_ANC_MAX, 0x00,
                ID_TRANS_GAIN, 0x00,
                ID_TRANS_MAX, 0x00,
            ],
            Self::SetLevel(level) => vec![*level],
            Self::SetAncLevel(level) => vec![*level],
            Self::SetTransparencyLevel(level) => vec![*level],
        }
    }

    fn decode(&self, payload: &[u8]) -> Result<Self::Response> {
        match self {
            Self::GetMode => {
                let mut parser = TlvParser::new(payload);
                let mode_id = parser
                    .get_int(ID_ANC_MODE)
                    .ok_or(Error::Parse("ANC mode not found in response"))?;
                let mode = AncMode::from_u8(mode_id)
                    .ok_or(Error::UnknownAncMode(mode_id))?;
                Ok(AncResponse::Mode(mode))
            }
            Self::GetLevel => {
                let mut parser = TlvParser::new(payload);
                let mode_id = parser.get_int(ID_ANC_MODE).unwrap_or(0);
                let (current, max) = match mode_id {
                    1 => {
                        let c = parser.get_int(ID_ANC_GAIN).unwrap_or(0);
                        let m = parser.get_int(ID_ANC_MAX).unwrap_or(0);
                        (c, m)
                    }
                    2 => {
                        let c = parser.get_int(ID_TRANS_GAIN).unwrap_or(0);
                        let m = parser.get_int(ID_TRANS_MAX).unwrap_or(0);
                        (c, m)
                    }
                    _ => (0, 0),
                };
                Ok(AncResponse::Level { current, max })
            }
            Self::SetMode(_) | Self::SetLevel(_) | Self::SetAncLevel(_) | Self::SetTransparencyLevel(_) => {
                Ok(AncResponse::Ack)
            }
        }
    }
}

// ── AncController ──

pub struct AncController<'a> {
    pub(crate) buds: &'a crate::SpaceBuds,
}

impl AncController<'_> {
    pub async fn get_mode(&self) -> Result<AncMode> {
        let resp = self.buds.manager.send(&AncCommand::GetMode).await?;
        match resp {
            AncResponse::Mode(m) => Ok(m),
            _ => Err(Error::Parse("Unexpected response type for get_mode")),
        }
    }

    pub async fn set_mode(&self, mode: AncMode) -> Result<()> {
        self.buds.manager.send(&AncCommand::SetMode(mode)).await?;
        tokio::time::sleep(Duration::from_millis(300)).await;
        Ok(())
    }

    pub async fn set_anc(&self) -> Result<()> {
        self.set_mode(AncMode::Active).await
    }

    pub async fn set_transparency(&self) -> Result<()> {
        self.set_mode(AncMode::Transparency).await
    }

    pub async fn set_off(&self) -> Result<()> {
        self.set_mode(AncMode::Off).await
    }

    pub async fn get_level(&self) -> Result<(u8, u8)> {
        let resp = self.buds.manager.send(&AncCommand::GetLevel).await?;
        match resp {
            AncResponse::Level { current, max } => Ok((current, max)),
            _ => Err(Error::Parse("Unexpected response type for get_level")),
        }
    }

    pub async fn set_level(&self, level: u8) -> Result<bool> {
        let (_, max_level) = self.get_level().await?;
        if max_level == 0 {
            return Ok(false);
        }
        let level = level.min(max_level);
        let mode = self.get_mode().await?;
        match mode {
            AncMode::Active => {
                self.buds.manager.send(&AncCommand::SetAncLevel(level)).await?;
            }
            AncMode::Transparency => {
                self.buds.manager.send(&AncCommand::SetTransparencyLevel(level)).await?;
            }
            AncMode::Off => return Ok(false),
        }
        Ok(true)
    }

    pub async fn set_level_direct(&self, level: u8, mode: AncMode) -> Result<()> {
        match mode {
            AncMode::Active => {
                self.buds.manager.send(&AncCommand::SetAncLevel(level)).await?;
            }
            AncMode::Transparency => {
                self.buds.manager.send(&AncCommand::SetTransparencyLevel(level)).await?;
            }
            AncMode::Off => { /* no-op */ }
        }
        Ok(())
    }
}
