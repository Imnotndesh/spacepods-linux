use crate::errors::Result;
use crate::protocol::{CMD_DUAL_DEVICE, CMD_ENV_ADAPTIVE, CMD_HANDSHAKE, ID_DUAL_DEVICE, ID_ENV_ADAPTIVE, TlvParser};
use crate::{SpaceBuds, CMD_IN_EAR_DETECT, CMD_KEY_SETTINGS, CMD_LED_SWITCH, CMD_SPATIAL_AUDIO, CMD_VOICE_PROMPT, ID_IN_EAR_DETECT, ID_LED_SWITCH, ID_VOICE_PROMPT};
use std::time::Duration;

pub struct FeatureController {
    buds: SpaceBuds,
}

impl FeatureController {
    pub fn new(buds: SpaceBuds) -> Self {
        Self { buds }
    }

    async fn set_bool(&self, cmd: u8, enable: bool, delay_ms: Option<u64>) -> Result<()> {
        let payload = vec![if enable { 0x01 } else { 0x00 }];
        self.buds
            .with_connection(|conn| async move { conn.command(cmd, payload).await })
            .await?;
        if let Some(ms) = delay_ms {
            tokio::time::sleep(Duration::from_millis(ms)).await;
        }
        Ok(())
    }



    async fn get_bool(&self, tlv_id: u8, active_value: u8) -> Result<Option<bool>> {
        self.buds
            .with_connection(|conn| async move {
                conn.query(
                    CMD_HANDSHAKE,
                    vec![0xFF, 0x00, tlv_id, 0x00],
                    move |packet| {
                        if packet.cmd_id == CMD_HANDSHAKE {
                            let mut parser = TlvParser::new(&packet.payload);
                            parser.get_int(tlv_id).map(|v| v == active_value)
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

    pub async fn set_adaptive_anc(&self, enable: bool) -> Result<()> {
        self.set_bool(CMD_ENV_ADAPTIVE, enable, Some(300)).await
    }

    pub async fn get_adaptive_anc(&self) -> Result<Option<bool>> {
        self.get_bool(ID_ENV_ADAPTIVE, 1u8).await
    }

    pub async fn set_dual_device(&self, enable: bool) -> Result<()> {
        self.set_bool(CMD_DUAL_DEVICE, enable, None).await
    }

    pub async fn get_dual_device(&self) -> Result<Option<bool>> {
        self.get_bool(ID_DUAL_DEVICE, 0).await
    }

    pub async fn set_in_ear_detect(&self, enable: bool) -> Result<()> {
        self.set_bool(CMD_IN_EAR_DETECT, enable, Some(300)).await
    }

    pub async fn get_in_ear_detect(&self) -> Result<Option<bool>> {
        self.get_bool(ID_IN_EAR_DETECT, 1).await
    }


    pub async fn set_led_switch(&self, led_on: bool) -> Result<()> {
        self.set_bool(CMD_LED_SWITCH, led_on, Some(300)).await
    }

    pub async fn get_led_switch(&self) -> Result<Option<bool>> {
        self.get_bool(ID_LED_SWITCH, 1).await
    }

    pub async fn set_voice_prompt(&self, enable: bool) -> Result<()> {
        self.set_bool(CMD_VOICE_PROMPT, enable, Some(300)).await
    }

    pub async fn get_voice_prompt(&self) -> Result<Option<bool>> {
        self.get_bool(ID_VOICE_PROMPT, 1).await
    }

    pub async fn set_spatial_audio(&self, enable: bool) -> Result<()> {
        self.set_bool(57, enable, Some(150)).await
    }

    pub async fn get_spatial_audio(&self) -> Result<Option<bool>> {
        self.get_bool(CMD_SPATIAL_AUDIO, 0).await
    }

    pub async fn set_hearing_care(&self, enabled: bool) -> Result<()> {
        self.set_bool(0x3A, enabled, None).await
    }

    pub async fn set_adaptive_volume(&self, enabled: bool) -> Result<()> {
        self.set_bool(0x3C, enabled, None).await
    }

    pub async fn set_karaoke_mode(&self, enabled: bool) -> Result<()> {
        self.set_bool(0x3D, enabled, None).await
    }


    pub async fn set_chat_mode(&self, enabled: bool) -> Result<()> {
        self.set_bool(0x3E, enabled, None).await
    }


    pub async fn set_long_endurance(&self, enabled: bool) -> Result<()> {
        self.set_bool(0x3F, enabled, None).await
    }


    pub async fn set_auto_answer(&self, enabled: bool) -> Result<()> {
        self.set_bool(0x41, enabled, None).await
    }


    pub async fn set_step_counting(&self, enabled: bool) -> Result<()> {
        self.set_bool(0x43, enabled, None).await
    }


    pub async fn set_bass_boost(&self, enabled: bool) -> Result<()> {
        self.set_bool(33, enabled, None).await
    }

    pub async fn set_language(&self, lang_id: u8) -> Result<()> {
        let payload = vec![lang_id];
        self.buds
            .with_connection(|conn| async move { conn.command(0x23, payload).await })
            .await?;
        Ok(())
    }



    pub async fn sync_device_time(&self, timestamp: u64) -> Result<()> {
        let secs = (timestamp & 0xFFFFFFFF) as u32;
        let payload = secs.to_be_bytes().to_vec();
        self.buds
            .with_connection(|conn| async move { conn.command(0x32, payload).await })
            .await?;
        Ok(())
    }



    pub async fn set_auto_shutdown(&self, minutes: u16) -> Result<()> {
        let payload = minutes.to_be_bytes().to_vec();
        self.buds
            .with_connection(|conn| async move { conn.command(0x36, payload).await })
            .await?;
        Ok(())
    }


    pub async fn set_3d_sound_effect(&self, mode: u8) -> Result<()> {
        let payload = vec![mode];
        self.buds
            .with_connection(|conn| async move { conn.command(0x39, payload).await })
            .await?;
        Ok(())
    }


    pub async fn set_tone_volume(&self, volume: u8) -> Result<()> {
        let payload = vec![volume];
        self.buds
            .with_connection(|conn| async move { conn.command(0x3B, payload).await })
            .await?;
        Ok(())
    }

    pub async fn reset_sport_data(&self) -> Result<()> {
        let payload = vec![0x01];
        self.buds
            .with_connection(|conn| async move { conn.command(0x44, payload).await })
            .await?;
        Ok(())
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