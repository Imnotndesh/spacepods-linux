use crate::errors::Result;
use crate::protocol::{CMD_WORK_MODE, CMD_HANDSHAKE, ID_WORK_MODE, TlvParser};
use crate::SpaceBuds;
use std::time::Duration;

pub struct WorkModeController {
    buds: SpaceBuds,
}

impl WorkModeController {
    pub fn new(buds: SpaceBuds) -> Self {
        Self { buds }
    }

    /// Enable game mode (low latency) – payload 1.
    /// Disable (normal mode) – payload 0.
    pub async fn set_game_mode(&self, enable: bool) -> Result<()> {
        let payload = vec![if enable { 1 } else { 0 }];
        self.buds
            .with_connection(|conn| async move { conn.command(CMD_WORK_MODE, payload).await })
            .await
    }

    /// Query current work mode. Returns `Some(true)` if game mode is on,
    /// `Some(false)` if normal mode, `None` if query fails.
    pub async fn get_game_mode(&self) -> Result<Option<bool>> {
        self.buds
            .with_connection(|conn| async move {
                conn.query(
                    CMD_HANDSHAKE,
                    vec![0xFF, 0x00, ID_WORK_MODE, 0x00],
                    |packet| {
                        if packet.cmd_id == CMD_HANDSHAKE {
                            let mut parser = TlvParser::new(&packet.payload);
                            parser.get_int(ID_WORK_MODE).map(|v| v == 1)
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
}