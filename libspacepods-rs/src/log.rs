//! Centralised logging for libspacepods daemon.
//!
//! Controlled by the `--log-level` CLI flag.  All output goes to stderr so
//! it never interferes with IPC on stdout.
//!
//! Levels (same as spacepods-ui):
//!   info  – errors + important state transitions
//!   warn  – retries, unexpected but non-fatal events
//!   full  – protocol-level details, raw packets

use std::sync::atomic::{AtomicU8, Ordering};

static LEVEL: AtomicU8 = AtomicU8::new(LogLevel::Info as u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum LogLevel {
    Info = 0,
    Warn = 1,
    Full = 2,
}

impl LogLevel {
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "full" => LogLevel::Full,
            "warn" => LogLevel::Warn,
            _ => LogLevel::Info,
        }
    }
}

pub fn set_log_level(level: LogLevel) {
    LEVEL.store(level as u8, Ordering::Relaxed);
}

fn level() -> LogLevel {
    match LEVEL.load(Ordering::Relaxed) {
        0 => LogLevel::Info,
        1 => LogLevel::Warn,
        _ => LogLevel::Full,
    }
}

pub fn info(tag: &str, msg: &str) {
    eprintln!("[SPACEPODS][{}] {}", tag, msg);
}

pub fn warn(tag: &str, msg: &str) {
    if level() >= LogLevel::Warn {
        eprintln!("[SPACEPODS][WARN][{}] {}", tag, msg);
    }
}

pub fn full(tag: &str, msg: &str) {
    if level() >= LogLevel::Full {
        eprintln!("[SPACEPODS][FULL][{}] {}", tag, msg);
    }
}
