use crate::protocol::constants::CMD_FIND_DEVICE;
use crate::commands::BleCommand;

/// "Find My Earbuds" command — plays a beep on the specified earbud(s).
#[derive(Debug, Clone)]
pub struct FindDeviceCommand {
    pub enable: bool,
}

impl BleCommand for FindDeviceCommand {
    type Response = ();

    fn cmd_id(&self) -> u8 {
        CMD_FIND_DEVICE
    }

    fn encode(&self) -> Vec<u8> {
        vec![if self.enable { 1 } else { 0 }]
    }

    fn decode(&self, _payload: &[u8]) -> Result<Self::Response, crate::Error> {
        Ok(())
    }
}
