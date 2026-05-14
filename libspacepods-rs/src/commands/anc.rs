// commands/anc.rs
use crate::ble::BleConnection;
use crate::errors::Result;
use crate::protocol::*;
use crate::SpaceBuds;
use std::time::Duration;
use tokio::sync::MutexGuard;

pub struct AncController {
    buds: SpaceBuds,
}

impl AncController {
    pub fn new(buds: SpaceBuds) -> Self {
        Self { buds }
    }

    async fn get_connection(&self) -> Result<MutexGuard<'_, Option<BleConnection>>> {
        self.buds.ensure_connected().await?;
        Ok(self.buds.conn.lock().await)
    }

    pub async fn set_mode(&self, mode: u8) -> Result<()> {
        let conn_guard = self.get_connection().await?;
        let conn = conn_guard.as_ref().unwrap();
        conn.command(CMD_ANC_MODE, vec![mode]).await?;
        tokio::time::sleep(Duration::from_millis(300)).await;
        Ok(())
    }

    pub async fn get_mode(&self) -> Result<Option<u8>> {
        let conn_guard = self.get_connection().await?;
        let conn = conn_guard.as_ref().unwrap();

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
        ).await
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

    pub async fn get_level(&self) -> Result<(u8, u8)> {
        let conn_guard = self.get_connection().await?;
        let conn = conn_guard.as_ref().unwrap();

        let result = conn.query(
            CMD_HANDSHAKE,
            vec![
                0xFF, 0x00,
                ID_ANC_MODE, 0x00,
                ID_ANC_GAIN, 0x00,
                ID_ANC_MAX, 0x00,
                ID_TRANS_GAIN, 0x00,
                ID_TRANS_MAX, 0x00,
            ],
            |packet| {
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
                        _ => Some((0, 0)),
                    }
                } else {
                    None
                }
            },
            Duration::from_secs(3),
        ).await?;

        Ok(result.unwrap_or((0, 0)))
    }

    pub async fn set_level(&self, level: u8) -> Result<bool> {
        let (_, max_level) = self.get_level().await?;

        if max_level == 0 {
            return Ok(false);
        }

        let level = level.min(max_level);
        let mode = self.get_mode().await?.unwrap_or(0);

        let conn_guard = self.get_connection().await?;
        let conn = conn_guard.as_ref().unwrap();

        match mode {
            MODE_ANC => {
                conn.command(CMD_ANC_GAIN, vec![level]).await?;
                Ok(true)
            }
            MODE_TRANSPARENCY => {
                conn.command(CMD_TRANS_GAIN, vec![level]).await?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }
}