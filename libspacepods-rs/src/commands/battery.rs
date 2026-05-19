use crate::errors::Result;
use crate::SpaceBuds;

pub struct BatteryController {
    buds: SpaceBuds,
}

impl BatteryController {
    pub fn new(buds: SpaceBuds) -> Self {
        Self { buds }
    }

    /// Returns (left, right, case) battery percentages, each Option<u8>
    pub async fn get_levels(&self) -> Result<(Option<u8>, Option<u8>, Option<u8>)> {
        self.buds
            .with_connection(|conn| async move { conn.get_battery_level().await })
            .await
    }
}