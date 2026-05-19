use crate::errors::{Result, SpaceBudsError};
use crate::protocol::{CMD_EQ_SETTING, CMD_HANDSHAKE, ID_EQ_SETTING, TlvParser};
use crate::SpaceBuds;
use std::time::Duration;

pub const EQ_PRESETS: [(u8, &str, &str, [i8; 10]); 7] = [
    (0, "Flat", "Neutral, uncolored sound", [0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
    (1, "Bass Boost", "Warm, punchy bass", [6, 4, 1, 0, 0, 1, 2, 0, 0, 0]),
    (2, "Rock", "Energetic V-shape", [4, 3, -1, -1, 2, 4, 5, 0, 0, 0]),
    (3, "Jazz", "Smooth mids, detailed cymbals", [2, 2, 1, 1, -1, 2, 4, 0, 0, 0]),
    (4, "Vocal", "Enhanced presence", [-2, -1, 0, 4, 3, 1, 1, 0, 0, 0]),
    (5, "Treble Boost", "Crisp highs", [-2, -1, 0, 1, 3, 5, 7, 0, 0, 0]),
    (6, "Custom", "User-defined EQ curve", [0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
];

pub const SPECIAL_PRESETS: [(u8, &str, &str, [i8; 10]); 4] = [
    (10, "Harman AE/OE", "Research-optimised curve", [4, 3, 1, 0, -1, 1, 3, 0, 0, 0]),
    (11, "Cinema", "Enhanced for movies", [2, 2, 0, 3, 2, 0, 2, 0, 0, 0]),
    (12, "Podcast", "Clear speech", [-1, 0, 2, 5, 2, -1, -2, 0, 0, 0]),
    (13, "Night Listening", "Reduced dynamics", [-3, -2, -1, 0, 0, -1, -2, 0, 0, 0]),
];

#[derive(Debug, Clone)]
pub struct EqState {
    pub mode: u8,
    pub name: String,
    pub description: String,
    pub gains: Vec<i8>,
}

pub struct EqController {
    buds: SpaceBuds,
}

impl EqController {
    pub fn new(buds: SpaceBuds) -> Self {
        Self { buds }
    }

    pub async fn set_preset(&self, preset_id: u8) -> Result<()> {
        let gains = if let Some((_, _, _, gains)) = EQ_PRESETS.iter().find(|(id, ..)| *id == preset_id) {
            *gains
        } else if let Some((_, _, _, gains)) = SPECIAL_PRESETS.iter().find(|(id, ..)| *id == preset_id) {
            *gains
        } else {
            return Err(SpaceBudsError::InvalidPreset(preset_id));
        };
        self.send_eq_command(preset_id, &gains).await
    }

    pub async fn set_custom(&self, gains: Vec<i8>) -> Result<()> {
        let mut final_gains = gains;
        final_gains.resize(10, 0);
        let gains_array: [i8; 10] = final_gains[..10].try_into().unwrap();
        self.send_eq_command(6, &gains_array).await
    }

    async fn send_eq_command(&self, mode: u8, gains: &[i8; 10]) -> Result<()> {
        let mut payload = vec![10, mode];
        payload.extend(gains.iter().map(|&g| g as u8));
        self.buds
            .with_connection(|conn| async move { conn.command(CMD_EQ_SETTING, payload).await })
            .await?;
        tokio::time::sleep(Duration::from_millis(300)).await;
        Ok(())
    }

    pub async fn get_state(&self) -> Result<Option<EqState>> {
        self.buds
            .with_connection(|conn| async move {
                conn.query(
                    CMD_HANDSHAKE,
                    vec![0xFF, 0x00, ID_EQ_SETTING, 0x00],
                    |packet| {
                        if packet.cmd_id == CMD_HANDSHAKE {
                            let mut parser = TlvParser::new(&packet.payload);
                            if let Some(eq_data) = parser.get_bytes(ID_EQ_SETTING) {
                                if eq_data.len() >= 2 {
                                    let mode = eq_data[1];
                                    let gains: Vec<i8> = eq_data[2..]
                                        .iter()
                                        .map(|&b| if b < 128 { b as i8 } else { (b as i16 - 256) as i8 })
                                        .collect();
                                    let (name, description) = Self::lookup_preset_name(mode);
                                    return Some(EqState { mode, name, description, gains });
                                }
                            }
                        }
                        None
                    },
                    Duration::from_secs(3),
                )
                    .await
            })
            .await
    }

    fn lookup_preset_name(mode: u8) -> (String, String) {
        if let Some((_, name, desc, _)) = EQ_PRESETS.iter().find(|(id, ..)| *id == mode) {
            (name.to_string(), desc.to_string())
        } else if let Some((_, name, desc, _)) = SPECIAL_PRESETS.iter().find(|(id, ..)| *id == mode) {
            (name.to_string(), desc.to_string())
        } else {
            ("Custom".to_string(), "User-defined EQ curve".to_string())
        }
    }

    pub fn list_presets(&self) -> Vec<(u8, String, String)> {
        let mut presets: Vec<(u8, String, String)> = EQ_PRESETS
            .iter()
            .map(|(id, name, desc, _)| (*id, name.to_string(), desc.to_string()))
            .collect();
        presets.extend(
            SPECIAL_PRESETS
                .iter()
                .map(|(id, name, desc, _)| (*id, name.to_string(), desc.to_string())),
        );
        presets
    }
}