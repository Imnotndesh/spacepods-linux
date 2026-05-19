// commands/features.rs

use crate::ble::BleConnection;
use crate::errors::Result;
use crate::protocol::{CMD_DUAL_DEVICE, CMD_ENV_ADAPTIVE, CMD_HANDSHAKE, ID_DUAL_DEVICE, ID_ENV_ADAPTIVE, TlvParser};
use crate::SpaceBuds;
use std::time::Duration;

pub struct FeatureController {
    buds: SpaceBuds,
}

impl FeatureController {
    pub fn new(buds: SpaceBuds) -> Self {
        Self { buds }
    }

    // --- Adaptive ANC ---
    pub async fn set_adaptive_anc(&self, enable: bool) -> Result<()> {
        let payload = vec![if enable { 0x01 } else { 0x00 }];
        self.buds
            .with_connection(|conn| async move { conn.command(CMD_ENV_ADAPTIVE, payload).await })
            .await?;
        tokio::time::sleep(Duration::from_millis(300)).await;
        Ok(())
    }

    pub async fn get_adaptive_anc(&self) -> Result<Option<bool>> {
        self.buds
            .with_connection(|conn| async move {
                conn.query(
                    CMD_HANDSHAKE,
                    vec![0xFF, 0x00, ID_ENV_ADAPTIVE, 0x00],
                    |packet| {
                        if packet.cmd_id == CMD_HANDSHAKE {
                            let mut parser = TlvParser::new(&packet.payload);
                            parser.get_int(ID_ENV_ADAPTIVE).map(|v| v == 1)
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

    // --- Dual Device (Multi‑point) ---
    pub async fn set_dual_device(&self, enable: bool) -> Result<()> {
        let payload = vec![0x01, 0x02, if enable { 0x01 } else { 0x00 }];
        self.buds
            .with_connection(|conn| async move { conn.command(CMD_DUAL_DEVICE, payload).await })
            .await?;
        tokio::time::sleep(Duration::from_millis(300)).await;
        Ok(())
    }

    pub async fn get_dual_device(&self) -> Result<Option<bool>> {
        self.buds
            .with_connection(|conn| async move {
                conn.query(
                    CMD_HANDSHAKE,
                    vec![0xFF, 0x00, ID_DUAL_DEVICE, 0x00],
                    |packet| {
                        if packet.cmd_id == CMD_HANDSHAKE {
                            let mut parser = TlvParser::new(&packet.payload);
                            parser.get_int(ID_DUAL_DEVICE).map(|v| v == 1)
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