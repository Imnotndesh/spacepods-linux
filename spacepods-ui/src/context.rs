//! Shared UI context passed down to every page.

use gtk4::prelude::*;
use libadwaita::{Toast, ToastOverlay};
use libspacepods::device_profile::{DeviceProfile, profile_for_product};
use libspacepods::ipc::protocol::DeviceStatus;
use std::cell::{Cell, RefCell};
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
    /// Latest status pushed by the daemon (kept live by a subscription).
    pub status: Rc<RefCell<Option<DeviceStatus>>>,
    /// Callbacks invoked whenever a new status arrives, on the GTK main loop.
    status_listeners: Rc<RefCell<Vec<Box<dyn Fn(&DeviceStatus)>>>>,
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
            status: Rc::new(RefCell::new(None)),
            status_listeners: Rc::new(RefCell::new(Vec::new())),
        })
    }

    /// Register a callback for status updates (called on the main loop).
    pub fn on_status(&self, f: impl Fn(&DeviceStatus) + 'static) {
        self.status_listeners.borrow_mut().push(Box::new(f));
    }

    /// Store a new status and notify listeners.
    pub fn publish_status(&self, s: &DeviceStatus) {
        *self.status.borrow_mut() = Some(s.clone());
        let listeners = self.status_listeners.borrow();
        for f in listeners.iter() {
            f(s);
        }
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

/// Start a background subscription to the daemon's pushed status updates.
///
/// Reconnects automatically if the socket drops. This is what makes the UI
/// react to changes (from the CLI, the earbuds, or another client) without a
/// manual refresh.
pub fn spawn_status_subscription(ctx: Rc<AppContext>) {
    glib::spawn_future_local(async move {
        loop {
            {
                use libspacepods::client::SpacePodsClient;
                if let Ok(mut client) = SpacePodsClient::connect(None).await {
                    if let Ok(mut rx) = client.subscribe().await {
                        // Keep the socket (and its writer half) alive while the
                        // reader task feeds the broadcast channel.
                        let _keep = client;
                        while let Ok(status) = rx.recv().await {
                            ctx.publish_status(&status);
                        }
                    }
                }
            }
            glib::timeout_future(std::time::Duration::from_secs(3)).await;
        }
    });
}

/// Marks a set of widgets sensitive/insensitive together, so a slow
/// round-trip to the daemon can't leave half the UI clickable and half
/// not.
pub fn set_busy(widgets: &[&impl IsA<gtk4::Widget>], busy: bool) {
    for w in widgets {
        w.set_sensitive(!busy);
    }
}
