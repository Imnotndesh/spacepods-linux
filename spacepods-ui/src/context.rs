//! Shared UI context passed down to every page.

use gtk4::prelude::*;
use libadwaita::{Toast, ToastOverlay};
use libspacepods::device_profile::{DeviceProfile, profile_for_product};
use std::cell::Cell;
use std::rc::Rc;

use crate::log::Log;
use crate::tray::TrayHandle;

/// Controls the main window so tray commands can show/hide/quit it reliably.
///
/// Holds a weak reference to the live window and exposes a small set of
/// window operations usable from tray commands and close-to-background logic.
#[derive(Clone, Default)]
pub struct WindowController {
    window: Rc<std::cell::RefCell<glib::WeakRef<libadwaita::ApplicationWindow>>>,
    /// Set when a real quit is requested (e.g. via tray "Quit") so the window's
    /// close-request handler allows the window to actually close instead of
    /// hiding-to-background.
    force_close: Rc<std::cell::Cell<bool>>,
}

impl WindowController {
    pub fn new() -> Rc<Self> {
        Rc::new(Self::default())
    }

    /// Register the live window. Called once per window activation.
    pub fn set_window(&self, win: &libadwaita::ApplicationWindow) {
        self.window.borrow_mut().set(Some(win));
    }

    /// Bring the window to the foreground.
    pub fn present(&self) {
        if let Some(w) = self.upgrade() {
            w.present();
        }
    }

    /// Hide the window (keep the app and tray running).
    pub fn hide(&self) {
        if let Some(w) = self.upgrade() {
            w.set_visible(false);
        }
    }

    /// Emit a close request on the window (subject to its close handler).
    pub fn close(&self) {
        if let Some(w) = self.upgrade() {
            w.close();
        }
    }

    /// Whether a real quit has been requested (bypasses close-to-background).
    pub fn force_close_requested(&self) -> bool {
        self.force_close.get()
    }

    /// Clear the real-quit flag after it has been honoured.
    pub fn clear_force_close(&self) {
        self.force_close.set(false);
    }

    /// Request a real quit: the window will be allowed to close on its next
    /// close request regardless of the close-to-background setting.
    pub fn force_quit(&self) {
        self.force_close.set(true);
        self.close();
    }

    /// Whether the window is currently visible.
    pub fn is_visible(&self) -> bool {
        self.upgrade().map(|w| w.is_visible()).unwrap_or(false)
    }

    fn upgrade(&self) -> Option<libadwaita::ApplicationWindow> {
        self.window.borrow().upgrade()
    }
}

#[derive(Clone)]
pub struct AppContext {
    pub toast_overlay: ToastOverlay,
    /// Product ID of the connected device (None until detected).
    pub product_id: Rc<Cell<Option<u16>>>,
    /// Handle to the tray icon (None if it could not be started).
    pub tray: Option<TrayHandle>,
    /// Controls the main window (show/hide/quit).
    pub window: Rc<WindowController>,
    /// Current ANC mode: 0=off, 1=ANC, 2=transparency.
    /// Shared between the ANC page and the tray popup so they stay in sync.
    pub anc_mode: Rc<Cell<u8>>,
}

impl AppContext {
    pub fn new(
        toast_overlay: ToastOverlay,
        tray: Option<TrayHandle>,
        window: Rc<WindowController>,
    ) -> Rc<Self> {
        Rc::new(Self {
            toast_overlay,
            product_id: Rc::new(Cell::new(None)),
            tray,
            window,
            anc_mode: Rc::new(Cell::new(0)),
        })
    }

    /// Get the device profile for the currently connected device, or None.
    pub fn profile(&self) -> Option<&'static DeviceProfile> {
        self.product_id.get().map(profile_for_product)
    }

    /// Check if the connected device supports a particular feature.
    pub fn has_feature(&self, feature: libspacepods::device_profile::DetailFeature) -> bool {
        let result = self.profile()
            .map(|p| p.features.contains(&feature))
            .unwrap_or(false);
        Log::full("CTX", &format!("has_feature({:?}) → {}", feature, result));
        result
    }

    /// Neutral, short-lived toast (confirmation of a background sync, etc).
    pub fn toast(&self, message: &str) {
        let toast = Toast::new(message);
        toast.set_timeout(2);
        self.toast_overlay.add_toast(toast);
    }

    /// A command to the daemon failed. Always show this — never swallow it.
    pub fn error(&self, message: impl AsRef<str>) {
        let toast = Toast::new(message.as_ref());
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
/// not.
pub fn set_busy(widgets: &[&impl IsA<gtk4::Widget>], busy: bool) {
    for w in widgets {
        w.set_sensitive(!busy);
    }
}
