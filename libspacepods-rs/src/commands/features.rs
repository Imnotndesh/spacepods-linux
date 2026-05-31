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

    pub async fn set_language(&self, lang_id: u8) -> Result<()> {
        let payload = vec![lang_id];
        self.buds
            .with_connection(|conn| async move { conn.command(0x23, payload).await })
            .await?;
        Ok(())
    }

    /// Sync Epoch Timestamp to Device (SyncTimeRequest -> CMD_SYNC_TIME = 0x32)
    /// Converts u64 timestamp into a 4-byte big-endian payload array matching Java's shift allocations
    pub async fn sync_device_time(&self, timestamp: u64) -> Result<()> {
        let secs = (timestamp & 0xFFFFFFFF) as u32;
        let payload = secs.to_be_bytes().to_vec();
        self.buds
            .with_connection(|conn| async move { conn.command(0x32, payload).await })
            .await?;
        Ok(())
    }
    pub async fn get_spatial_audio(&self) -> Result<Option<bool>> {
        self.buds
            .with_connection(|conn| async move {
                conn.query(
                    CMD_HANDSHAKE,
                    vec![0xFF, 0x00, CMD_SPATIAL_AUDIO, 0x00],
                    |packet| {
                        if packet.cmd_id == CMD_HANDSHAKE {
                            let mut parser = TlvParser::new(&packet.payload);
                            parser.get_int(CMD_SPATIAL_AUDIO).map(|v| v == 0)
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

    /// Set Auto Shutdown Timeout in Minutes (AutoShutdownRequest -> CMD_AUTO_SHUTDOWN = 0x36)
    /// Maps u16 into a 2-byte big-endian payload array
    pub async fn set_auto_shutdown(&self, minutes: u16) -> Result<()> {
        let payload = minutes.to_be_bytes().to_vec();
        self.buds
            .with_connection(|conn| async move { conn.command(0x36, payload).await })
            .await?;
        Ok(())
    }

    /// Toggle Spatial Audio (SpaceAudioRequest -> CMD_SPATIAL_AUDIO = 0x38)
    pub async fn set_spatial_audio(&self, enabled: bool) -> Result<()> {
        let payload = vec![if enabled { 0x01 } else { 0x00 }];
        self.buds
            .with_connection(|conn| async move { conn.command(0x38, payload).await })
            .await?;
        Ok(())
    }

    /// Change 3D Sound Effect profile (SoundEffect3dRequest -> CMD_SOUND_EFFECT_3D = 0x39)
    pub async fn set_3d_sound_effect(&self, mode: u8) -> Result<()> {
        let payload = vec![mode];
        self.buds
            .with_connection(|conn| async move { conn.command(0x39, payload).await })
            .await?;
        Ok(())
    }

    /// Toggle Hearing Care Safe-Volume Limits (HearingCareRequest -> CMD_HEARING_CARE = 0x3A)
    pub async fn set_hearing_care(&self, enabled: bool) -> Result<()> {
        let payload = vec![if enabled { 0x01 } else { 0x00 }];
        self.buds
            .with_connection(|conn| async move { conn.command(0x3A, payload).await })
            .await?;
        Ok(())
    }

    /// Adjust Prompt Tone Volume Level (ToneVolumeRequest -> CMD_TONE_VOLUME = 0x3B)
    pub async fn set_tone_volume(&self, volume: u8) -> Result<()> {
        let payload = vec![volume];
        self.buds
            .with_connection(|conn| async move { conn.command(0x3B, payload).await })
            .await?;
        Ok(())
    }

    /// Toggle Environmental Adaptive Volume (AdaptiveVolumeRequest -> CMD_ADAPTIVE_VOL = 0x3C)
    pub async fn set_adaptive_volume(&self, enabled: bool) -> Result<()> {
        let payload = vec![if enabled { 0x01 } else { 0x00 }];
        self.buds
            .with_connection(|conn| async move { conn.command(0x3C, payload).await })
            .await?;
        Ok(())
    }

    /// Toggle Karaoke Mode Reverb (KaraokeRequest -> CMD_KARAOKE_MODE = 0x3D)
    pub async fn set_karaoke_mode(&self, enabled: bool) -> Result<()> {
        let payload = vec![if enabled { 0x01 } else { 0x00 }];
        self.buds
            .with_connection(|conn| async move { conn.command(0x3D, payload).await })
            .await?;
        Ok(())
    }

    /// Toggle Intelligent Chat Quick-Transparency (ChatModeRequest -> CMD_CHAT_MODE = 0x3E)
    pub async fn set_chat_mode(&self, enabled: bool) -> Result<()> {
        let payload = vec![if enabled { 0x01 } else { 0x00 }];
        self.buds
            .with_connection(|conn| async move { conn.command(0x3E, payload).await })
            .await?;
        Ok(())
    }

    /// Toggle Ultra-Long Battery Endurance Mode (LongEnduranceModeRequest -> CMD_LONG_ENDURANCE = 0x3F)
    pub async fn set_long_endurance(&self, enabled: bool) -> Result<()> {
        let payload = vec![if enabled { 0x01 } else { 0x00 }];
        self.buds
            .with_connection(|conn| async move { conn.command(0x3F, payload).await })
            .await?;
        Ok(())
    }

    /// Toggle Auto Answer Incoming Calls on Earbud Insertion (AutoAnswerRequest -> CMD_AUTO_ANSWER = 0x41)
    pub async fn set_auto_answer(&self, enabled: bool) -> Result<()> {
        let payload = vec![if enabled { 0x01 } else { 0x00 }];
        self.buds
            .with_connection(|conn| async move { conn.command(0x41, payload).await })
            .await?;
        Ok(())
    }

    /// Toggle Pedometer / Step Counting (StepCountSwitchRequest -> CMD_STEP_COUNT = 0x43)
    pub async fn set_step_counting(&self, enabled: bool) -> Result<()> {
        let payload = vec![if enabled { 0x01 } else { 0x00 }];
        self.buds
            .with_connection(|conn| async move { conn.command(0x43, payload).await })
            .await?;
        Ok(())
    }

    /// Reset Active Fitness / Running Sport metrics (ResetSportRequest -> CMD_RESET_SPORT = 0x44)
    pub async fn reset_sport_data(&self) -> Result<()> {
        let payload = vec![0x01]; // Trigger flag byte
        self.buds
            .with_connection(|conn| async move { conn.command(0x44, payload).await })
            .await?;
        Ok(())
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