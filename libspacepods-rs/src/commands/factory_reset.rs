use crate::errors::Result;
use crate::protocol::CMD_FACTORY_RESET;
use crate::SpaceBuds;

pub struct FactoryResetController {
    buds: SpaceBuds,
}

impl FactoryResetController {
    pub fn new(buds: SpaceBuds) -> Self {
        Self { buds }
    }

    /// Perform a factory reset. The earbuds will likely disconnect and reboot.
    /// Payload is empty according to the Android `FactoryResetRequest`.
    pub async fn reset(&self) -> Result<()> {
        self.buds
            .with_connection(|conn| async move { conn.command(CMD_FACTORY_RESET, vec![]).await })
            .await
    }
}