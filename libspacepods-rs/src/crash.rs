//! Crash reports, saved on this computer only.
//!
//! A Rust panic writes a report to `$XDG_STATE_HOME/spacepods/crashes/` (or
//! `~/.local/state/spacepods/crashes/`). Nothing is uploaded. The next time the app
//! starts, it can tell the person that a report exists, and where it is.
//!
//! Native crashes (segmentation faults and the like) are not caught here. ABRT (Fedora),
//! apport (Ubuntu) and systemd-coredump capture those on systems that have them, and
//! they need no code from this project.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Folder that holds crash reports.
pub fn reports_dir() -> PathBuf {
    let base = std::env::var_os("XDG_STATE_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("spacepods").join("crashes")
}

/// Install a panic hook that writes a report, then runs the previous hook (so the
/// usual message still appears on the terminal or in the journal).
///
/// `component` names the process: `"daemon"` or `"ui"`.
pub fn install_panic_hook(component: &'static str) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = write_report(component, info);
        previous(info);
    }));
}

fn write_report(component: &str, info: &std::panic::PanicHookInfo<'_>) -> std::io::Result<PathBuf> {
    let dir = reports_dir();
    fs::create_dir_all(&dir)?;
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let path = dir.join(format!("{}-{}-{}.txt", stamp, component, std::process::id()));
    let mut file = fs::File::create(&path)?;

    let message = info
        .payload()
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| info.payload().downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "unknown panic".to_string());
    let location = info
        .location()
        .map(|l| format!("{}:{}", l.file(), l.line()))
        .unwrap_or_else(|| "unknown".to_string());

    writeln!(file, "SpacePods crash report")?;
    writeln!(file, "component: {}", component)?;
    writeln!(file, "version: {}", crate::VERSION)?;
    writeln!(file, "time: {} (unix seconds)", stamp)?;
    writeln!(file, "os: {}", std::env::consts::OS)?;
    writeln!(file, "flatpak: {}", std::env::var_os("FLATPAK_ID").is_some())?;
    writeln!(file, "location: {}", location)?;
    writeln!(file, "message: {}", message)?;
    writeln!(file)?;
    writeln!(file, "backtrace:")?;
    writeln!(file, "{}", std::backtrace::Backtrace::force_capture())?;
    Ok(path)
}

/// Reports not yet shown to the person (files still ending in `.txt`), oldest first.
pub fn pending_reports() -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = fs::read_dir(reports_dir())
        .map(|entries| {
            entries
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|x| x == "txt"))
                .collect()
        })
        .unwrap_or_default();
    found.sort();
    found
}

/// Mark a report as shown, so it is not reported again.
pub fn mark_reported(path: &Path) {
    let _ = fs::rename(path, path.with_extension("txt.reported"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panic_writes_a_report_with_message_and_location() {
        let state = std::env::temp_dir().join(format!("spacepods-crash-{}", std::process::id()));
        let _ = fs::remove_dir_all(&state);
        // SAFETY: this is the only test in the crate that reads XDG_STATE_HOME.
        unsafe { std::env::set_var("XDG_STATE_HOME", &state) };

        install_panic_hook("test");
        let _ = std::panic::catch_unwind(|| panic!("boom for the report test"));
        let _ = std::panic::take_hook();

        let pending = pending_reports();
        assert_eq!(pending.len(), 1, "expected one report, found {:?}", pending);
        let text = fs::read_to_string(&pending[0]).unwrap();
        assert!(text.contains("message: boom for the report test"), "{text}");
        assert!(text.contains("component: test"), "{text}");
        assert!(text.contains("location:"), "{text}");

        mark_reported(&pending[0]);
        assert!(pending_reports().is_empty());
        let _ = fs::remove_dir_all(&state);
    }
}
