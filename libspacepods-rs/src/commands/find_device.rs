use crate::errors::Result;
use crate::protocol::CMD_FIND_DEVICE;
use crate::SpaceBuds;

pub struct FindDeviceController {
    buds: SpaceBuds,
}

impl FindDeviceController {
    pub fn new(buds: SpaceBuds) -> Self {
        Self { buds }
    }

    /// Start playing a beep sound on the earbuds (payload 1).
    /// Stop the beep (payload 0).
    pub async fn set_enabled(&self, enable: bool) -> Result<()> {
        let payload = vec![if enable { 1 } else { 0 }];
        self.buds
            .with_connection(|conn| async move { conn.command(CMD_FIND_DEVICE, payload).await })
            .await
    }
}