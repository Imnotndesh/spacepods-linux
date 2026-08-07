pub mod anc;
pub mod eq;
pub mod features;

use crate::Result;

/// A typed BLE command that knows how to encode itself for the wire
/// and decode its own response.
pub trait BleCommand: Send {
    /// The Rust type returned when this command succeeds.
    type Response: Send + 'static;

    /// BLE command ID byte.
    fn cmd_id(&self) -> u8;

    /// Encode the command into a payload byte vector.
    fn encode(&self) -> Vec<u8>;

    /// Decode a response payload into the typed response.
    fn decode(&self, payload: &[u8]) -> Result<Self::Response>;
}

/// A command that uses the handshake mechanism (TLV query/response).
/// Typically used for reading device state.
pub trait BleQuery: BleCommand {
    /// The info IDs to query in the handshake request.
    fn query_info_ids(&self) -> Vec<u8>;
}

// Re-export controllers
pub use anc::AncController;
pub use eq::{EqController, EqPreset, EqState};
pub use features::FeatureController;
