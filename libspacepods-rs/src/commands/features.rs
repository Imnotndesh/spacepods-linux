use crate::errors::Result;
use crate::protocol::{CMD_DUAL_DEVICE, CMD_ENV_ADAPTIVE, CMD_HANDSHAKE, ID_DUAL_DEVICE, ID_ENV_ADAPTIVE, TlvParser};
use crate::{SpaceBuds, CMD_IN_EAR_DETECT, CMD_KEY_SETTINGS, CMD_LED_SWITCH, CMD_VOICE_PROMPT, ID_IN_EAR_DETECT, ID_LED_SWITCH, ID_VOICE_PROMPT};
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
    pub async fn set_in_ear_detect(&self, enable: bool) -> Result<()> {
        let payload = vec![if enable { 0x01 } else { 0x00 }];
        self.buds
            .with_connection(|conn| async move { conn.command(CMD_IN_EAR_DETECT, payload).await })
            .await?;
        tokio::time::sleep(Duration::from_millis(300)).await;
        Ok(())
    }
    pub async fn get_in_ear_detect(&self) -> Result<Option<bool>> {
        self.buds
            .with_connection(|conn| async move {
                conn.query(
                    CMD_HANDSHAKE,
                    vec![0xFF, 0x00, ID_IN_EAR_DETECT, 0x00],
                    |packet| {
                        if packet.cmd_id == CMD_HANDSHAKE {
                            let mut parser = TlvParser::new(&packet.payload);
                            parser.get_int(ID_IN_EAR_DETECT).map(|v| v == 1)
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

    pub async fn set_led_switch(&self, led_on: bool) -> Result<()> {
        let payload = vec![if led_on { 0x01 } else { 0x00 }];
        self.buds
            .with_connection(|conn| async move { conn.command(CMD_LED_SWITCH, payload).await })
            .await?;
        tokio::time::sleep(Duration::from_millis(300)).await;
        Ok(())
    }
    pub async fn get_led_switch(&self) -> Result<Option<bool>> {
        self.buds
            .with_connection(|conn| async move {
                conn.query(
                    CMD_HANDSHAKE,
                    vec![0xFF, 0x00, ID_LED_SWITCH, 0x00],
                    |packet| {
                        if packet.cmd_id == CMD_HANDSHAKE {
                            let mut parser = TlvParser::new(&packet.payload);
                            parser.get_int(ID_LED_SWITCH).map(|v| v == 1)
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

    pub async fn set_voice_prompt(&self, enable: bool) -> Result<()> {
        let payload = vec![if enable { 0x01 } else { 0x00 }];
        self.buds
            .with_connection(|conn| async move { conn.command(CMD_VOICE_PROMPT, payload).await })
            .await?;
        tokio::time::sleep(Duration::from_millis(300)).await;
        Ok(())
    }

    pub async fn get_voice_prompt(&self) -> Result<Option<bool>> {
        self.buds
            .with_connection(|conn| async move {
                conn.query(
                    CMD_HANDSHAKE,
                    vec![0xFF, 0x00, ID_VOICE_PROMPT, 0x00],
                    |packet| {
                        if packet.cmd_id == CMD_HANDSHAKE {
                            let mut parser = TlvParser::new(&packet.payload);
                            parser.get_int(ID_VOICE_PROMPT).map(|v| v == 1)
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
    pub async fn remap_gesture(&self, gesture: u8, function: u8) -> Result<()> {
        let payload = vec![gesture, 1, function];
        self.buds
            .with_connection(|conn| async move { conn.command(CMD_KEY_SETTINGS, payload).await })
            .await?;
        tokio::time::sleep(Duration::from_millis(300)).await;
        Ok(())
    }
}