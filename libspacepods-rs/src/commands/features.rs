use crate::ble::BleConnection;
use crate::errors::Result;
use crate::protocol::{
    CMD_DUAL_DEVICE, CMD_ENV_ADAPTIVE, CMD_HANDSHAKE,
    ID_DUAL_DEVICE, ID_ENV_ADAPTIVE, TlvParser,
};
use crate::SpaceBuds;
use std::time::Duration;
use tokio::sync::MutexGuard;

pub struct FeatureController {
    buds: SpaceBuds,
}

impl FeatureController {
    pub fn new(buds: SpaceBuds) -> Self {
        Self { buds }
    }

    async fn get_connection(&self) -> Result<MutexGuard<'_, Option<BleConnection>>> {
        self.buds.ensure_connected().await?;
        Ok(self.buds.conn.lock().await)
    }

    // Adaptive ANC
    pub async fn get_adaptive_anc(&self) -> Result<Option<bool>> {
        let conn_guard = self.get_connection().await?;
        let conn = conn_guard.as_ref().unwrap();

        let result = conn.query(
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
        ).await?;

        Ok(result)
    }

    pub async fn set_adaptive_anc(&self, enable: bool) -> Result<()> {
        let payload = vec![if enable { 0x01 } else { 0x00 }];

        let conn_guard = self.get_connection().await?;
        let conn = conn_guard.as_ref().unwrap();
        conn.command(CMD_ENV_ADAPTIVE, payload).await?;

        tokio::time::sleep(Duration::from_millis(300)).await;
        Ok(())
    }

    // Dual Device (Multi-point)
    pub async fn get_dual_device(&self) -> Result<Option<bool>> {
        let conn_guard = self.get_connection().await?;
        let conn = conn_guard.as_ref().unwrap();

        let result = conn.query(
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
        ).await?;

        Ok(result)
    }

    pub async fn set_dual_device(&self, enable: bool) -> Result<()> {
        // Payload format: [0x01, 0x02, status]
        let payload = vec![0x01, 0x02, if enable { 0x01 } else { 0x00 }];

        let conn_guard = self.get_connection().await?;
        let conn = conn_guard.as_ref().unwrap();
        conn.command(CMD_DUAL_DEVICE, payload).await?;

        tokio::time::sleep(Duration::from_millis(300)).await;
        Ok(())
    }
}