use crate::errors::Result;
use crate::protocol::*;
use crate::SpaceBuds;

pub struct KeySettingsController {
    buds: SpaceBuds,
}

impl KeySettingsController {
    pub fn new(buds: SpaceBuds) -> Self {
        Self { buds }
    }

    /// Rebinds a physical interaction gesture to a specific software operation (Command 34 / 0x22)
    /// Payload structure: [key_type, 1, key_function]
    pub async fn configure_key(&self, key_type: u8, key_function: u8) -> Result<()> {
        let payload = vec![key_type, 1, key_function];
        self.buds
            .with_connection(|conn| async move { conn.command(CMD_KEY_SETTINGS, payload).await })
            .await?;
        Ok(())
    }
}