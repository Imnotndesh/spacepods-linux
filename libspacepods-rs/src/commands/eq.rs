use crate::commands::BleCommand;
use crate::protocol::tlv::TlvParser;
use crate::protocol::constants::{CMD_EQ_SETTING, CMD_HANDSHAKE, ID_EQ_SETTING};
use crate::{Error, Result};
use std::time::Duration;

// ── EQ Preset Struct ──

#[derive(Debug, Clone)]
pub struct EqPreset {
    pub id: u8,
    pub name: &'static str,
    pub description: &'static str,
    pub gains: [i8; 10],
}

impl EqPreset {
    pub const fn new(id: u8, name: &'static str, description: &'static str, gains: [i8; 10]) -> Self {
        Self { id, name, description, gains }
    }
}

pub const EQ_PRESETS: &[EqPreset] = &[
    EqPreset::new(0, "Flat", "Neutral, uncolored sound", [0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
    EqPreset::new(1, "Bass Boost", "Warm, punchy bass (Harman headphone curve inspired)", [6, 4, 1, 0, 0, 1, 2, 0, 0, 0]),
    EqPreset::new(2, "Rock", "Energetic V-shape for guitars and drums", [4, 3, -1, -1, 2, 4, 5, 0, 0, 0]),
    EqPreset::new(3, "Jazz", "Smooth mids, detailed cymbals", [2, 2, 1, 1, -1, 2, 4, 0, 0, 0]),
    EqPreset::new(4, "Vocal", "Enhanced presence for vocals and speech", [-2, -1, 0, 4, 3, 1, 1, 0, 0, 0]),
    EqPreset::new(5, "Treble Boost", "Crisp highs for classical and acoustic", [-2, -1, 0, 1, 3, 5, 7, 0, 0, 0]),
    EqPreset::new(6, "Custom", "User-defined EQ curve", [0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
];

pub const SPECIAL_PRESETS: &[EqPreset] = &[
    EqPreset::new(10, "Harman AE/OE", "Research-optimized consumer curve", [4, 3, 1, 0, -1, 1, 3, 0, 0, 0]),
    EqPreset::new(11, "Cinema", "Enhanced for movies and dialogue", [2, 2, 0, 3, 2, 0, 2, 0, 0, 0]),
    EqPreset::new(12, "Podcast", "Clear speech, reduced sibilance", [-1, 0, 2, 5, 2, -1, -2, 0, 0, 0]),
    EqPreset::new(13, "Night Listening", "Reduced dynamics for quiet environments", [-3, -2, -1, 0, 0, -1, -2, 0, 0, 0]),
];

impl EqPreset {
    pub fn find(id: u8) -> Option<&'static Self> {
        EQ_PRESETS.iter().chain(SPECIAL_PRESETS.iter()).find(|p| p.id == id)
    }

    pub fn gains_description(gains: &[i8; 10]) -> String {
        let bass = (gains[0] + gains[1]) / 2;
        let mid = (gains[2] + gains[3] + gains[4]) / 3;
        let treble = (gains[5] + gains[6]) / 2;
        format!("Bass: {}dB, Mid: {}dB, Treble: {}dB", bass, mid, treble)
    }
}


#[derive(Debug, Clone)]
pub struct EqState {
    pub mode: u8,
    pub name: String,
    pub description: String,
    pub gains: Vec<i8>,
}

impl EqState {
    pub fn is_custom(&self) -> bool {
        self.mode == 6 || self.mode >= 10
    }
}

// ── EqCommand ──

#[derive(Debug, Clone)]
pub enum EqCommand {
    GetState,
    SetPreset(u8),
    SetCustom(Vec<i8>),
}

#[derive(Debug, Clone)]
pub enum EqResponse {
    State(Option<EqState>),
    Ack,
}

/// Convert an i8 gain value to its u8 wire representation.
fn gain_to_u8(gain: i8) -> u8 {
    gain as u8
}

/// Convert a u8 wire value back to an i8 gain value.
fn u8_to_gain(byte: u8) -> i8 {
    if byte < 128 {
        byte as i8
    } else {
        (byte as i16 - 256) as i8
    }
}

impl BleCommand for EqCommand {
    type Response = EqResponse;

    fn cmd_id(&self) -> u8 {
        match self {
            Self::GetState => CMD_HANDSHAKE,
            Self::SetPreset(_) | Self::SetCustom(_) => CMD_EQ_SETTING,
        }
    }

    fn encode(&self) -> Vec<u8> {
        match self {
            Self::GetState => vec![0xFF, 0x00, ID_EQ_SETTING, 0x00],
            Self::SetPreset(preset_id) => {
                if let Some(preset) = EqPreset::find(*preset_id) {
                    let mut payload = vec![10, preset.id];
                    for &gain in preset.gains.iter() {
                        payload.push(gain_to_u8(gain));
                    }
                    payload
                } else {
                    vec![]
                }
            }
            Self::SetCustom(gains) => {
                let mut final_gains = gains.clone();
                final_gains.resize(10, 0);
                let mut payload = vec![10, 6]; // mode = 6 (Custom)
                for &gain in final_gains.iter().take(10) {
                    payload.push(gain_to_u8(gain));
                }
                payload
            }
        }
    }

    fn decode(&self, payload: &[u8]) -> Result<Self::Response> {
        match self {
            Self::GetState => {
                let mut parser = TlvParser::new(payload);
                if let Some(eq_data) = parser.get_bytes(ID_EQ_SETTING) {
                    if eq_data.len() >= 2 {
                        let mode = eq_data[1];
                        let gains: Vec<i8> = eq_data[2..]
                            .iter()
                            .map(|&b| u8_to_gain(b))
                            .collect();

                        let preset = EqPreset::find(mode);
                        let (name, description) = if let Some(p) = preset {
                            (p.name.to_string(), p.description.to_string())
                        } else {
                            ("Unknown".to_string(), "Unknown preset".to_string())
                        };

                        return Ok(EqResponse::State(Some(EqState {
                            mode,
                            name,
                            description,
                            gains,
                        })));
                    }
                }
                Ok(EqResponse::State(None))
            }
            Self::SetPreset(_) | Self::SetCustom(_) => Ok(EqResponse::Ack),
        }
    }
}

// ── EqController ──

pub struct EqController<'a> {
    pub(crate) buds: &'a crate::SpaceBuds,
}

impl EqController<'_> {
    pub async fn get_state(&self) -> Result<Option<EqState>> {
        let resp = self.buds.manager.send(&EqCommand::GetState).await?;
        match resp {
            EqResponse::State(state) => Ok(state),
            _ => Err(Error::Parse("Unexpected response type for get_state")),
        }
    }

    pub async fn set_preset(&self, preset_id: u8) -> Result<()> {
        if EqPreset::find(preset_id).is_none() {
            return Err(Error::InvalidPreset(preset_id));
        }
        self.buds.manager.send(&EqCommand::SetPreset(preset_id)).await?;
        tokio::time::sleep(Duration::from_millis(300)).await;
        Ok(())
    }

    pub async fn set_custom(&self, gains: Vec<i8>) -> Result<()> {
        self.buds.manager.send(&EqCommand::SetCustom(gains)).await?;
        tokio::time::sleep(Duration::from_millis(300)).await;
        Ok(())
    }

    pub fn list_presets() -> Vec<(u8, String, String, String)> {
        let mut presets: Vec<_> = EQ_PRESETS
            .iter()
            .map(|p| {
                (
                    p.id,
                    p.name.to_string(),
                    p.description.to_string(),
                    EqPreset::gains_description(&p.gains),
                )
            })
            .collect();

        presets.extend(SPECIAL_PRESETS.iter().map(|p| {
            (
                p.id,
                format!("{} (Special)", p.name),
                p.description.to_string(),
                EqPreset::gains_description(&p.gains),
            )
        }));

        presets
    }
}
