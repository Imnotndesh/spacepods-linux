//! systemd readiness and watchdog notifications (`sd_notify(3)`).
//!
//! Implemented directly over `$NOTIFY_SOCKET` so the daemon needs no extra
//! dependency. Every function is a no-op when the variable is unset, so running the
//! daemon by hand or outside systemd is unaffected.

use std::os::unix::net::UnixDatagram;
use std::path::Path;
use std::time::Duration;

/// Send one `sd_notify` state string, for example `"READY=1"`.
pub fn notify(state: &str) {
    if let Some(path) = std::env::var_os("NOTIFY_SOCKET") {
        let _ = send_to(Path::new(&path), state);
    }
}

/// Tell systemd the daemon is ready to serve requests.
pub fn ready() {
    notify("READY=1");
}

/// Tell systemd the daemon is shutting down.
pub fn stopping() {
    notify("STOPPING=1");
}

/// Keep-alive for `WatchdogSec=`. Call at least twice per watchdog interval.
pub fn watchdog_ping() {
    notify("WATCHDOG=1");
}

/// The watchdog interval systemd asked for, if the unit enables one.
///
/// Returns `None` when `WATCHDOG_USEC` is unset or does not match this process
/// (`WATCHDOG_PID`, when set, must be ours).
pub fn watchdog_interval() -> Option<Duration> {
    if let Some(pid) = std::env::var_os("WATCHDOG_PID") {
        if pid.to_str()?.parse::<u32>().ok()? != std::process::id() {
            return None;
        }
    }
    let usec: u64 = std::env::var("WATCHDOG_USEC").ok()?.parse().ok()?;
    (usec > 0).then(|| Duration::from_micros(usec))
}

fn send_to(path: &Path, state: &str) -> std::io::Result<()> {
    let sock = UnixDatagram::unbound()?;
    sock.send_to(state.as_bytes(), path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sends_state_to_the_notify_socket() {
        let dir = std::env::temp_dir().join(format!("spacepods-notify-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("notify.sock");
        let listener = UnixDatagram::bind(&path).unwrap();

        send_to(&path, "READY=1").unwrap();

        let mut buf = [0u8; 64];
        let n = listener.recv(&mut buf).unwrap();
        assert_eq!(&buf[..n], b"READY=1");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
