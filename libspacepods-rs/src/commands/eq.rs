// commands/eq.rs - Updated EQ Presets
use crate::ble::BleConnection;
use crate::errors::{Result, SpaceBudsError};
use crate::protocol::{CMD_EQ_SETTING, CMD_HANDSHAKE, ID_EQ_SETTING, TlvParser};
use crate::SpaceBuds;
use std::time::Duration;
use tokio::sync::MutexGuard;

// Frequency bands (for reference)
// [50Hz, 100Hz, 400Hz, 1kHz, 2.5kHz, 6.3kHz, 16kHz, ...extras]

// Professionally tuned EQ presets following Harman curves and industry standards
pub const EQ_PRESETS: [(u8, &str, &str, [i8; 10]); 7] = [
    (0, "Flat", "Neutral, uncolored sound", [0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),

    (1, "Bass Boost", "Warm, punchy bass (Harman headphone curve inspired)",
     [6, 4, 1, 0, 0, 1, 2, 0, 0, 0]),

    (2, "Rock", "Energetic V-shape for guitars and drums",
     [4, 3, -1, -1, 2, 4, 5, 0, 0, 0]),

    (3, "Jazz", "Smooth mids, detailed cymbals",
     [2, 2, 1, 1, -1, 2, 4, 0, 0, 0]),

    (4, "Vocal", "Enhanced presence for vocals and speech",
     [-2, -1, 0, 4, 3, 1, 1, 0, 0, 0]),

    (5, "Treble Boost", "Crisp highs for classical and acoustic",
     [-2, -1, 0, 1, 3, 5, 7, 0, 0, 0]),

    (6, "Custom", "User-defined EQ curve",
     [0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
];

// Additional specialized presets (accessible via extended commands)
pub const SPECIAL_PRESETS: [(u8, &str, &str, [i8; 10]); 4] = [
    (10, "Harman AE/OE", "Research-optimized consumer curve",
     [4, 3, 1, 0, -1, 1, 3, 0, 0, 0]),

    (11, "Cinema", "Enhanced for movies and dialogue",
     [2, 2, 0, 3, 2, 0, 2, 0, 0, 0]),

    (12, "Podcast", "Clear speech, reduced sibilance",
     [-1, 0, 2, 5, 2, -1, -2, 0, 0, 0]),

    (13, "Night Listening", "Reduced dynamics for quiet environments",
     [-3, -2, -1, 0, 0, -1, -2, 0, 0, 0]),
];

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

pub struct EqController {
    buds: SpaceBuds,
}

impl EqController {
    pub fn new(buds: SpaceBuds) -> Self {
        Self { buds }
    }

    async fn get_connection(&self) -> Result<MutexGuard<'_, Option<BleConnection>>> {
        self.buds.ensure_connected().await?;
        Ok(self.buds.conn.lock().await)
    }

    pub async fn get_state(&self) -> Result<Option<EqState>> {
        let conn_guard = self.get_connection().await?;
        let conn = conn_guard.as_ref().unwrap();

        let result = conn.query(
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
                                .map(|&b| {
                                    if b < 128 {
                                        b as i8
                                    } else {
                                        (b as i16 - 256) as i8
                                    }
                                })
                                .collect();

                            // Check both regular and special presets
                            let preset_info = EQ_PRESETS
                                .iter()
                                .find(|(id, _, _, _)| *id == mode)
                                .map(|(_, name, desc, _)| (name, desc));

                            let special_info = SPECIAL_PRESETS
                                .iter()
                                .find(|(id, _, _, _)| *id == mode)
                                .map(|(_, name, desc, _)| (name, desc));

                            let (name, description) = if let Some((n, d)) = preset_info {
                                (*n, *d)
                            } else if let Some((n, d)) = special_info {
                                (*n, *d)
                            } else {
                                ("Unknown", "Unknown preset")
                            };

                            return Some(EqState {
                                mode,
                                name: name.to_string(),
                                description: description.to_string(),
                                gains,
                            });
                        }
                    }
                }
                None
            },
            Duration::from_secs(3),
        ).await?;

        Ok(result)
    }

    pub async fn set_preset(&self, preset_id: u8) -> Result<()> {
        // Check regular presets first
        if let Some((id, name, _, gains)) = EQ_PRESETS.iter().find(|(id, _, _, _)| *id == preset_id) {
            println!("Setting EQ preset: {} - {}", name, gains_description(gains));
            self.send_eq_command(*id, gains).await?;
            return Ok(());
        }

        // Check special presets
        if let Some((id, name, _, gains)) = SPECIAL_PRESETS.iter().find(|(id, _, _, _)| *id == preset_id) {
            println!("Setting special EQ preset: {} - {}", name, gains_description(gains));
            self.send_eq_command(*id, gains).await?;
            return Ok(());
        }

        Err(SpaceBudsError::InvalidPreset(preset_id))
    }

    async fn send_eq_command(&self, mode: u8, gains: &[i8; 10]) -> Result<()> {
        let mut payload = vec![10, mode];
        for &gain in gains.iter() {
            payload.push(gain as u8);
        }

        let conn_guard = self.get_connection().await?;
        let conn = conn_guard.as_ref().unwrap();
        conn.command(CMD_EQ_SETTING, payload).await?;

        tokio::time::sleep(Duration::from_millis(300)).await;
        Ok(())
    }

    pub async fn set_custom(&self, gains: Vec<i8>) -> Result<()> {
        // Pad to 10 bands
        let mut final_gains = gains;
        final_gains.resize(10, 0);

        // Store gains array for the command
        let gains_array: [i8; 10] = final_gains[..10].try_into().unwrap();

        println!("Setting custom EQ curve");
        self.send_eq_command(6, &gains_array).await?;

        Ok(())
    }

    pub async fn list_presets(&self) -> Vec<(u8, String, String)> {
        let mut presets: Vec<(u8, String, String)> = EQ_PRESETS
            .iter()
            .map(|(id, name, desc, gains)| {
                (*id,
                 format!("{} - {}", name, desc),
                 gains_description(gains))
            })
            .collect();

        // Add special presets
        presets.extend(SPECIAL_PRESETS.iter().map(|(id, name, desc, gains)| {
            (*id,
             format!("{} (Special) - {}", name, desc),
             gains_description(gains))
        }));

        presets
    }

    pub async fn analyze_current_eq(&self) -> Result<String> {
        if let Some(state) = self.get_state().await? {
            if state.is_custom() {
                Ok(format!("Custom EQ: {}", gains_description(&state.gains[..10].try_into().unwrap_or(*&[0;10]))))
            } else {
                Ok(format!("{}: {}", state.name, state.description))
            }
        } else {
            Ok("Unknown EQ state".to_string())
        }
    }
}

fn gains_description(gains: &[i8; 10]) -> String {
    let bass = (gains[0] + gains[1]) / 2;
    let mids = (gains[2] + gains[3] + gains[4]) / 3;
    let treble = (gains[5] + gains[6]) / 2;

    format!("Bass: {}dB, Mids: {}dB, Treble: {}dB", bass, mids, treble)
}