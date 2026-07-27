//! Shared UI context passed down to every page.
//!
//! This replaces the old pattern where each page silently ate errors from
//! the daemon (`let _ = client.set_x().await;`). Every page now gets a
//! handle to the window's `ToastOverlay` so failures are always visible,
//! and a tiny helper for marking a control "busy" while a command is in
//! flight so the UI never looks unresponsive or lies about its state.

use gtk4::prelude::*;
use libadwaita::{Toast, ToastOverlay};
use std::rc::Rc;

#[derive(Clone)]
pub struct AppContext {
    pub toast_overlay: ToastOverlay,
}

impl AppContext {
    pub fn new(toast_overlay: ToastOverlay) -> Rc<Self> {
        Rc::new(Self { toast_overlay })
    }

    /// Neutral, short-lived toast (confirmation of a background sync, etc).
    pub fn toast(&self, message: &str) {
        let toast = Toast::new(message);
        toast.set_timeout(2);
        self.toast_overlay.add_toast(toast);
    }

    /// A command to the daemon failed. Always show this — never swallow it.
    pub fn error(&self, message: impl AsRef<str>) {
        let toast = Toast::new(&format!("⚠ {}", message.as_ref()));
        toast.set_timeout(5);
        toast.set_priority(libadwaita::ToastPriority::High);
        self.toast_overlay.add_toast(toast);
    }

    /// A command succeeded and it's worth confirming (e.g. "Preset saved").
    pub fn success(&self, message: impl AsRef<str>) {
        let toast = Toast::new(message.as_ref());
        toast.set_timeout(2);
        self.toast_overlay.add_toast(toast);
    }

    /// Daemon unreachable — the one error users will see most often.
    pub fn daemon_unreachable(&self, err: impl std::fmt::Display) {
        self.error(format!("Can't reach SpacePods service: {}", err));
    }
}

/// Marks a set of widgets sensitive/insensitive together, so a slow
/// round-trip to the daemon can't leave half the UI clickable and half
/// not. Returns a guard-like closure pair (start/stop) rather than a
/// struct, since GTK widgets aren't `Send` and we're always on the main
/// loop here anyway.
pub fn set_busy(widgets: &[&impl IsA<gtk4::Widget>], busy: bool) {
    for w in widgets {
        w.set_sensitive(!busy);
    }
}