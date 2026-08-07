//! Centralised logging for spacepods-ui.
//!
//! Set the level at startup with `Log::set_level(...)`.  All logs go to
//! stderr so they can be captured independently of GTK output.
//!
//! Usage:
//! ```ignore
//! Log::info("ANC page", "mode set to ANC");
//! Log::warn("BLE", "retry attempt 2/3");
//! Log::full("Features", &format!("profile={:?}", profile));
//! ```

use std::sync::atomic::{AtomicU8, Ordering};

// ── Log level (set once at startup) ──

static LEVEL: AtomicU8 = AtomicU8::new(Level::Info as u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Level {
    /// Only errors and warnings.
    Info = 0,
    /// + important state transitions (connect / disconnect / mode changes).
    Warn = 1,
    /// + verbose diagnostic output (feature gating, profile detection, etc).
    Full = 2,
}

pub struct Log;

impl Log {
    pub fn set_level(level: Level) {
        LEVEL.store(level as u8, Ordering::Relaxed);
    }

    fn level() -> Level {
        match LEVEL.load(Ordering::Relaxed) {
            0 => Level::Info,
            1 => Level::Warn,
            _ => Level::Full,
        }
    }

    /// Always-visible messages (errors, connection success, etc.).
    pub fn info(tag: &str, msg: &str) {
        eprintln!("[SPACEPODS][{}] {}", tag, msg);
    }

    /// Transition-level messages shown at Info + Warn.
    pub fn warn(tag: &str, msg: &str) {
        if Self::level() >= Level::Warn {
            eprintln!("[SPACEPODS][WARN][{}] {}", tag, msg);
        }
    }

    /// Diagnostic messages only shown at Full.
    pub fn full(tag: &str, msg: &str) {
        if Self::level() >= Level::Full {
            eprintln!("[SPACEPODS][FULL][{}] {}", tag, msg);
        }
    }
}
