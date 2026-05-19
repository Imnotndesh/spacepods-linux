use crate::errors::Result;
use crate::protocol::*;
use crate::SpaceBuds;
use std::time::Duration;

pub struct AncController {
    buds: SpaceBuds,
}

impl AncController {
    pub fn new(buds: SpaceBuds) -> Self {
        Self { buds }
    }

    pub async fn set_mode(&self, mode: u8) -> Result<()> {
        self.buds
            .with_connection(|conn| async move { conn.command(CMD_ANC_MODE, vec![mode]).await })
            .await?;
        tokio::time::sleep(Duration::from_millis(300)).await;
        Ok(())
    }

    pub async fn set_anc(&self) -> Result<()> {
        self.set_mode(MODE_ANC).await
    }

    pub async fn set_transparency(&self) -> Result<()> {
        self.set_mode(MODE_TRANSPARENCY).await
    }

    pub async fn set_off(&self) -> Result<()> {
        self.set_mode(MODE_OFF).await
    }

    pub async fn set_level(&self, level: u8) -> Result<bool> {
        let (_, max_level) = self.get_level().await?;
        if max_level == 0 {
            return Ok(false);
        }
        let level = level.min(max_level);
        let mode = self.get_mode().await?.unwrap_or(0);
        match mode {
            MODE_ANC => {
                self.buds
                    .with_connection(|conn| async move { conn.command(CMD_ANC_GAIN, vec![level]).await })
                    .await?;
                Ok(true)
            }
            MODE_TRANSPARENCY => {
                self.buds
                    .with_connection(|conn| async move { conn.command(CMD_TRANS_GAIN, vec![level]).await })
                    .await?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    pub async fn get_mode(&self) -> Result<Option<u8>> {
        self.buds
            .with_connection(|conn| async move {
                conn.query(
                    CMD_HANDSHAKE,
                    vec![0xFF, 0x00, ID_ANC_MODE, 0x00],
                    |packet| {
                        if packet.cmd_id == CMD_HANDSHAKE {
                            let mut parser = TlvParser::new(&packet.payload);
                            parser.get_int(ID_ANC_MODE)
                        } else {
                            None
                        }
                    },
                    Duration::from_secs(3),
                )
                    .await
            })
            .await
    }

    /// Returns (current_level, max_level)
    pub async fn get_level(&self) -> Result<(u8, u8)> {
        self.buds
            .with_connection(|conn| async move {
                let opt: Option<(u8, u8)> = conn
                    .query(
                        CMD_HANDSHAKE,
                        vec![
                            0xFF, 0x00,
                            ID_ANC_MODE, 0x00,
                            ID_ANC_GAIN, 0x00,
                            ID_ANC_MAX, 0x00,
                            ID_TRANS_GAIN, 0x00,
                            ID_TRANS_MAX, 0x00,
                        ],
                        |packet| -> Option<(u8, u8)> {
                            if packet.cmd_id == CMD_HANDSHAKE {
                                let mut parser = TlvParser::new(&packet.payload);
                                let mode = parser.get_int(ID_ANC_MODE).unwrap_or(0);
                                match mode {
                                    MODE_ANC => {
                                        let current = parser.get_int(ID_ANC_GAIN).unwrap_or(0);
                                        let max = parser.get_int(ID_ANC_MAX).unwrap_or(0);
                                        Some((current, max))
                                    }
                                    MODE_TRANSPARENCY => {
                                        let current = parser.get_int(ID_TRANS_GAIN).unwrap_or(0);
                                        let max = parser.get_int(ID_TRANS_MAX).unwrap_or(0);
                                        Some((current, max))
                                    }
                                    _ => Some((0u8, 0u8)),
                                }
                            } else {
                                None
                            }
                        },
                        Duration::from_secs(3),
                    )
                    .await?;
                Ok(opt.unwrap_or((0u8, 0u8)))
            })
            .await
    }
}