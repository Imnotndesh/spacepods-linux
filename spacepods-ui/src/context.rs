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
/// window operations usable from tray commands and closing-to-tray logic.
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

    /// Whether a window has been created and is still alive (hidden or shown).
    pub fn exists(&self) -> bool {
        self.upgrade().is_some()
    }

    /// Whether the window is currently shown (false when hidden or gone).
    pub fn is_visible(&self) -> bool {
        self.upgrade().map(|w| w.is_visible()).unwrap_or(false)
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

    /// Whether a real quit has been requested (bypasses closing to the tray).
    pub fn force_close_requested(&self) -> bool {
        self.force_close.get()
    }

    /// Clear the real-quit flag after it has been honoured.
    pub fn clear_force_close(&self) {
        self.force_close.set(false);
    }

    /// Request a real quit: the window will be allowed to close on its next
    /// close request regardless of whether a tray is running.
    pub fn force_quit(&self) {
        self.force_close.set(true);
        self.close();
    }


    fn upgrade(&self) -> Option<libadwaita::ApplicationWindow> {
        self.window.borrow().upgrade()
    }
}

/// Whether the background service answered the last time we asked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DaemonState {
    /// No attempt has finished yet.
    Unknown,
    Online,
    Offline,
}

/// What the banner's button does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BannerAction {
    /// No button.
    None,
    /// Check the service again.
    Retry,
    /// Open the Bluetooth settings.
    OpenBluetooth,
}

#[derive(Clone)]
pub struct AppContext {
    pub toast_overlay: ToastOverlay,
    /// Product ID of the connected device (None until detected).
    pub product_id: Rc<Cell<Option<u16>>>,
    /// Features the device itself reported (capability bitfield, custom command
    /// 33). When present this is authoritative over the static product profile.
    pub capabilities: Rc<RefCell<Option<Vec<libspacepods::device_profile::DetailFeature>>>>,
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
    /// Whether the background service is reachable.
    pub daemon: Rc<Cell<DaemonState>>,
    /// Whether the Bluetooth adapter is switched off (reported by the daemon).
    pub bluetooth_off: Rc<Cell<bool>>,
    /// The earbuds' last known connection state, to notice changes.
    pub connected_seen: Rc<Cell<Option<bool>>>,
    /// Called when the daemon state or the earbuds' connection state changes.
    daemon_listeners: Rc<RefCell<Vec<Box<dyn Fn()>>>>,
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
            capabilities: Rc::new(RefCell::new(None)),
            tray,
            window,
            anc_mode: Rc::new(Cell::new(0)),
            status: Rc::new(RefCell::new(None)),
            status_listeners: Rc::new(RefCell::new(Vec::new())),
            daemon: Rc::new(Cell::new(DaemonState::Unknown)),
            bluetooth_off: Rc::new(Cell::new(false)),
            connected_seen: Rc::new(Cell::new(None)),
            daemon_listeners: Rc::new(RefCell::new(Vec::new())),
        })
    }

    /// Register a callback for status updates (called on the main loop).
    pub fn on_status(&self, f: impl Fn(&DeviceStatus) + 'static) {
        self.status_listeners.borrow_mut().push(Box::new(f));
    }

    /// Store a new status and notify listeners.
    pub fn publish_status(&self, s: &DeviceStatus) {
        if let Some(pid) = s.product_id {
            self.product_id.set(Some(pid));
        }
        if let Some(caps) = &s.capabilities {
            *self.capabilities.borrow_mut() = Some(caps.clone());
        }
        *self.status.borrow_mut() = Some(s.clone());
        // The device's ANC mode is the truth: the radio groups in the tray and the
        // pages follow it, even when the change came from another client.
        // Tell the person when the earbuds connect or drop, once per change.
        let connected = s.connection.connected;
        if let Some(previous) = self.connected_seen.get() {
            if previous != connected {
                if connected {
                    crate::notify::send("earbuds-connection", "Earbuds connected", "SpacePods is connected to your earbuds.");
                } else {
                    crate::notify::send("earbuds-connection", "Earbuds disconnected", "SpacePods lost the connection to your earbuds.");
                }
            }
        }
        self.connected_seen.set(Some(connected));

        let mode = s.anc.mode as u8;
        self.anc_mode.set(mode);
        crate::tray::ANC_MODE_ATOMIC.store(mode, std::sync::atomic::Ordering::Relaxed);
        if let Some(tray) = &self.tray {
            tray.refresh();
        }
        let listeners = self.status_listeners.borrow();
        for f in listeners.iter() {
            f(s);
        }
        drop(listeners);
        self.notify_daemon_listeners();
    }

    /// Register a callback for daemon-state and earbud-connection changes.
    pub fn on_daemon_change(&self, f: impl Fn() + 'static) {
        self.daemon_listeners.borrow_mut().push(Box::new(f));
    }

    fn notify_daemon_listeners(&self) {
        for f in self.daemon_listeners.borrow().iter() {
            f();
        }
    }

    /// Record whether the background service is reachable. Listeners run only on change.
    pub fn set_daemon_state(&self, state: DaemonState) {
        if self.daemon.get() != state {
            self.daemon.set(state);
            self.notify_daemon_listeners();
        }
    }

    /// Record whether the Bluetooth adapter is on. Listeners run only on change.
    pub fn set_bluetooth_off(&self, off: bool) {
        if self.bluetooth_off.get() != off {
            self.bluetooth_off.set(off);
            if off {
                crate::notify::send(
                    "bluetooth",
                    "Bluetooth is turned off",
                    "Turn it on to connect your earbuds.",
                );
            }
            self.notify_daemon_listeners();
        }
    }

    /// What the persistent banner should say, if anything, and what its button does.
    /// The most urgent problem wins: service down, then Bluetooth off, then earbuds.
    pub fn banner_message(&self) -> Option<(&'static str, BannerAction)> {
        match self.daemon.get() {
            DaemonState::Offline => Some((
                "Can't reach the SpacePods background service.",
                BannerAction::Retry,
            )),
            DaemonState::Online if self.bluetooth_off.get() => Some((
                "Bluetooth is turned off.",
                BannerAction::OpenBluetooth,
            )),
            DaemonState::Online => match self.status.borrow().as_ref() {
                Some(s) if !s.connection.connected => {
                    Some(("Your earbuds aren't connected.", BannerAction::None))
                }
                _ => None,
            },
            DaemonState::Unknown => None,
        }
    }

    /// Check the service now (used by the banner's Retry action). Also reads whether
    /// Bluetooth is on, so the banner stays correct.
    pub fn check_daemon(self: &Rc<Self>) {
        let ctx = self.clone();
        glib::spawn_future_local(async move {
            use libspacepods::client::SpacePodsClient;
            let (online, bluetooth_off) = match SpacePodsClient::connect(None).await {
                Ok(mut client) => {
                    let online = client.ping().await.unwrap_or(false);
                    let powered = client.bluetooth_powered().await.ok().flatten();
                    (online, powered == Some(false))
                }
                Err(_) => (false, false),
            };
            ctx.set_bluetooth_off(bluetooth_off);
            ctx.set_daemon_state(if online { DaemonState::Online } else { DaemonState::Offline });
        });
    }

    /// Get the device profile for the currently connected device, or None.
    pub fn profile(&self) -> Option<&'static DeviceProfile> {
        self.product_id.get().map(profile_for_product)
    }

    /// Check if the connected device supports a particular feature.
    ///
    /// Prefers the capability bitfield the device broadcast (which is accurate
    /// for models like the BoomPop Pro that disable features); falls back to the
    /// static product profile when the device has not reported capabilities.
    pub fn has_feature(&self, feature: libspacepods::device_profile::DetailFeature) -> bool {
        let result = if let Some(caps) = self.capabilities.borrow().as_ref() {
            caps.contains(&feature)
        } else {
            self.profile()
                .map(|p| p.features.contains(&feature))
                .unwrap_or(false)
        };
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

    /// A daemon action failed. Shows `context` with the plain-language reason, and logs
    /// the full error for diagnosis.
    pub fn failed(&self, context: &str, err: &libspacepods::Error) {
        Log::warn("UI", &format!("{} failed: {:?}", context, err));
        match err.code() {
            libspacepods::ErrorCode::BluetoothOff => self.set_bluetooth_off(true),
            // The earbuds do not have this feature. The control stays disabled; there is
            // nothing for the person to fix, so no message is shown.
            libspacepods::ErrorCode::UnsupportedFeature => return,
            _ => {}
        }
        self.error(format!("{}: {}", context, crate::errors::user_text(err)));
    }

    /// The daemon could not be reached at all.
    pub fn daemon_unreachable(&self, err: libspacepods::Error) {
        Log::warn("UI", &format!("daemon unreachable: {:?}", err));
        // The banner already says this while the service is down. A toast on top of it
        // would repeat the same message for every page that loads.
        if self.daemon.get() != DaemonState::Offline {
            self.error(crate::errors::user_text(&err));
        }
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
                    ctx.set_daemon_state(DaemonState::Online);
                    if let Ok(powered) = client.bluetooth_powered().await {
                        ctx.set_bluetooth_off(powered == Some(false));
                    }
                    if let Ok(mut rx) = client.subscribe().await {
                        // The client stays alive for the whole loop: it owns the
                        // transport the updates arrive on.
                        loop {
                            match rx.recv().await {
                                Ok(status) => ctx.publish_status(&status),
                                // This listener fell behind and missed updates. Take a
                                // fresh snapshot and keep the subscription, instead of
                                // dropping it and reconnecting.
                                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                                    if let Ok(status) = client.get_status().await {
                                        ctx.publish_status(&status);
                                    }
                                }
                                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                            }
                        }
                    }
                }
            }
            ctx.set_daemon_state(DaemonState::Offline);
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

/// Read boolean features from the daemon and pass each reported value to `apply`.
///
/// Used by refresh buttons so the switches show what the earbuds report, not the last
/// value sent. Features the device does not report are skipped. Failures are shown
/// through [`AppContext::failed`]; success is confirmed once at the end.
pub fn read_features(
    ctx: &Rc<AppContext>,
    features: &'static [&'static str],
    apply: impl Fn(&str, bool) + 'static,
) {
    let ctx = ctx.clone();
    glib::spawn_future_local(async move {
        use libspacepods::client::SpacePodsClient;
        let mut client = match SpacePodsClient::connect(None).await {
            Ok(c) => c,
            Err(e) => {
                // The banner says this; several pages reading at once should not toast.
                crate::log::Log::warn("UI", &format!("feature read skipped, daemon unreachable: {:?}", e));
                ctx.set_daemon_state(DaemonState::Offline);
                return;
            }
        };
        let mut all_ok = true;
        for &feature in features {
            match client.get_feature(feature).await {
                Ok(Some(on)) => apply(feature, on),
                Ok(None) => {}
                // Not connected, or Bluetooth off: the banner already says so. Quiet here,
                // so a page does not toast for every setting it reads.
                Err(e) if matches!(
                    e.code(),
                    libspacepods::ErrorCode::NotConnected
                        | libspacepods::ErrorCode::BluetoothOff
                        | libspacepods::ErrorCode::UnsupportedFeature
                ) => {
                    all_ok = false;
                    Log::info("UI", &format!("{} not read: {:?}", feature, e));
                }
                Err(e) => {
                    all_ok = false;
                    ctx.failed(&format!("Couldn't read {}", feature.replace('_', " ")), &e);
                }
            }
        }
        if all_ok {
            ctx.success("Refreshed");
        }
    });
}
