//! Battery level, read in the background without ever blocking anything else.
//!
//! Sources, in order:
//! 1. The standard GATT battery level (`0x2A19`), read over the existing BLE link. A read
//!    only: nothing is written to the earbuds.
//! 2. The percentage BlueZ reports for the device (`org.bluez.Battery1`), the way desktop
//!    battery applets read it.
//! 3. Nothing: the level is reported as unknown rather than guessed.
//!
//! The reads run in a supervised task. A panic in the task is caught, reported in the
//! status, and the task is restarted after a backoff. The daemon itself never stops
//! because of it, and the UI keeps working, showing the battery as unknown until it
//! returns.

use crate::ipc::protocol::DeviceStatus;
use crate::SpaceBuds;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, RwLock};

/// Where a battery reading came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatterySource {
    #[default]
    Unknown,
    /// Standard GATT battery level, over the BLE link.
    Gatt,
    /// BlueZ's `Battery1` value.
    Bluez,
}

/// Whether the reported level is current.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatteryState {
    /// Read recently.
    Ok,
    /// The last read timed out; the level shown is the last good one.
    Stale,
    /// Not connected, or no source has a value.
    #[default]
    Unavailable,
    /// The reader panicked and is being restarted.
    Restarting,
}

/// How often a connected device's battery is read.
const READ_INTERVAL: Duration = Duration::from_secs(60);
/// Longest time one read may take. A slow or hung read must not hold anything up.
const READ_TIMEOUT: Duration = Duration::from_secs(8);
/// Retry sooner when the last read had no value: the battery is often published a few
/// seconds after the link comes up.
const RETRY_INTERVAL: Duration = Duration::from_secs(15);
/// How often the reader checks for a connection while disconnected.
const IDLE_INTERVAL: Duration = Duration::from_secs(5);
/// Restart backoff after a panic: starts here and doubles up to the cap.
const RESTART_BACKOFF_START: Duration = Duration::from_secs(2);
const RESTART_BACKOFF_CAP: Duration = Duration::from_secs(60);

/// Read one level from the best available source.
pub async fn read_level(buds: &SpaceBuds, address: Option<&str>) -> (Option<u8>, BatterySource) {
    if let Some(level) = buds.manager.read_battery_level().await {
        return (Some(level), BatterySource::Gatt);
    }
    if let Some(address) = address {
        // BlueZ may report the battery on the BLE device or on the audio device.
        let audio = crate::bluez::audio_address_for(address).await;
        for candidate in std::iter::once(address).chain(audio.as_deref()) {
            if let Some(level) = crate::bluez::battery_percentage(candidate).await {
                return (Some(level), BatterySource::Bluez);
            }
        }
    }
    (None, BatterySource::Unknown)
}

/// Start the battery reader under supervision. Call once, from the daemon's run loop.
pub fn spawn_supervised(
    buds: SpaceBuds,
    status: Arc<RwLock<DeviceStatus>>,
    status_tx: broadcast::Sender<DeviceStatus>,
) {
    tokio::spawn(async move {
        let mut backoff = RESTART_BACKOFF_START;
        loop {
            let worker = tokio::spawn(run_reader(buds.clone(), status.clone(), status_tx.clone()));
            match worker.await {
                // The reader only returns if its loop ends, which it does not in normal use.
                Ok(()) => break,
                Err(e) if e.is_panic() => {
                    crate::log::warn("BATTERY", "battery reader crashed; restarting");
                    set_state(&status, &status_tx, BatteryState::Restarting, None, BatterySource::Unknown).await;
                    tokio::time::sleep(backoff).await;
                    backoff = (backoff * 2).min(RESTART_BACKOFF_CAP);
                }
                Err(_) => break,
            }
        }
    });
}

async fn run_reader(
    buds: SpaceBuds,
    status: Arc<RwLock<DeviceStatus>>,
    status_tx: broadcast::Sender<DeviceStatus>,
) {
    // When the last read happened. A new connection is read at once; otherwise reads are
    // READ_INTERVAL apart. The loop wakes every IDLE_INTERVAL to check.
    let mut last_read: Option<std::time::Instant> = None;
    let mut last_had_value = false;
    loop {
        if !buds.is_connected().await {
            set_state(&status, &status_tx, BatteryState::Unavailable, None, BatterySource::Unknown).await;
            last_read = None;
            tokio::time::sleep(IDLE_INTERVAL).await;
            continue;
        }
        let wait = if last_had_value { READ_INTERVAL } else { RETRY_INTERVAL };
        let due = last_read.is_none_or(|t| t.elapsed() >= wait);
        if !due {
            tokio::time::sleep(IDLE_INTERVAL).await;
            continue;
        }
        last_read = Some(std::time::Instant::now());
        let address = buds.address().await;
        match tokio::time::timeout(READ_TIMEOUT, read_level(&buds, address.as_deref())).await {
            Ok((Some(level), source)) => {
                last_had_value = true;
                set_state(&status, &status_tx, BatteryState::Ok, Some(level), source).await;
            }
            Ok((None, _)) => {
                last_had_value = false;
                set_state(&status, &status_tx, BatteryState::Unavailable, None, BatterySource::Unknown).await;
            }
            Err(_) => {
                last_had_value = false;
                // Keep the last good level, but mark it stale.
                let (level, source) = {
                    let s = status.read().await;
                    (s.battery.level, s.battery.source)
                };
                set_state(&status, &status_tx, BatteryState::Stale, level, source).await;
            }
        }
        tokio::time::sleep(IDLE_INTERVAL).await;
    }
}

/// Store a reading and tell the listeners. The status lock is not held while sending.
async fn set_state(
    status: &Arc<RwLock<DeviceStatus>>,
    status_tx: &broadcast::Sender<DeviceStatus>,
    state: BatteryState,
    level: Option<u8>,
    source: BatterySource,
) {
    let snapshot = {
        let mut s = status.write().await;
        s.battery.state = state;
        s.battery.level = level;
        s.battery.source = source;
        s.clone()
    };
    let _ = status_tx.send(snapshot);
}
