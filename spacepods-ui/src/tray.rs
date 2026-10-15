//! Optional system-tray icon (StatusNotifierItem, via `ksni`).
//!
//! The tray is **off by default**. GNOME does not show StatusNotifierItem icons unless
//! the AppIndicator extension is installed, and the quick-settings popup it opened
//! cannot be positioned on Wayland. Build with `--features tray` to turn it on, for
//! example for KDE or other desktops that show tray icons.
//!
//! When the feature is off, [`spawn_tray`] returns an inert handle and a receiver that
//! never yields, so the rest of the app compiles and behaves the same without a tray.

use std::sync::atomic::AtomicU8;
use std::sync::mpsc::{self, Receiver};

/// Commands the tray sends to the GTK main loop.
#[derive(Debug, Clone)]
#[cfg_attr(not(feature = "tray"), allow(dead_code))]
pub enum TrayCommand {
    /// Show the quick-settings popup at (x, y) screen coords.
    ShowPopup { x: i32, y: i32 },
    /// Show the main application window.
    ShowWindow,
    /// Hide the main application window.
    HideWindow,
    /// Real quit: closes the window and exits, even with a tray running.
    Quit,
    /// Set ANC mode (0=off, 1=anc, 2=transparency).
    SetAncMode(u8),
}

/// Current ANC mode, read by the tray's radio group. Written by the app.
pub static ANC_MODE_ATOMIC: AtomicU8 = AtomicU8::new(0);

#[cfg(feature = "tray")]
pub use sni::{spawn_tray, TrayHandle};

#[cfg(not(feature = "tray"))]
pub use inert::{spawn_tray, TrayHandle};

#[cfg(not(feature = "tray"))]
mod inert {
    use super::{mpsc, Receiver, TrayCommand};

    /// Placeholder with the same API as the real tray handle. Commands are dropped.
    #[derive(Clone)]
    pub struct TrayHandle;

    impl TrayHandle {
        #[allow(dead_code)]
        pub fn send(&self, _cmd: TrayCommand) {}

        /// Whether a tray icon is registered and can bring the window back.
        pub fn is_active(&self) -> bool {
            false
        }

        pub fn refresh(&self) {}
    }

    /// Returns an inert handle. The receiver stays open and never yields.
    pub fn spawn_tray() -> (TrayHandle, Receiver<TrayCommand>) {
        let (_tx, rx) = mpsc::channel();
        (TrayHandle, rx)
    }
}

#[cfg(feature = "tray")]
mod sni {
    use super::{mpsc, Receiver, TrayCommand, ANC_MODE_ATOMIC};
    use std::sync::mpsc::Sender;
    use ksni::blocking::{Handle as TrayHandleInner, TrayMethods};

    /// Handle used by the rest of the app. Holds the ksni service alive.
    #[derive(Clone)]
    pub struct TrayHandle {
        #[allow(dead_code)]
        sender: Sender<TrayCommand>,
        /// `None` when the tray could not be registered; the app still runs without it.
        #[allow(dead_code)]
        tray_service: Option<TrayHandleInner<SpacePodsTray>>,
    }

    impl TrayHandle {
        #[allow(dead_code)]
        pub fn send(&self, cmd: TrayCommand) {
            let _ = self.sender.send(cmd);
        }

        /// Whether a tray icon is registered and can bring the window back.
        pub fn is_active(&self) -> bool {
            self.tray_service.is_some()
        }

        /// Re-read the menu's state (the ANC radio group) from the shared cells.
        pub fn refresh(&self) {
            if let Some(service) = &self.tray_service {
                let _ = service.update(|_| {});
            }
        }
    }

    /// Start the tray. A failure is logged and the app runs without a tray.
    pub fn spawn_tray() -> (TrayHandle, Receiver<TrayCommand>) {
        let (tx, rx) = mpsc::channel();
        let tray = SpacePodsTray { sender: tx.clone() };
        let sandboxed = std::env::var("FLATPAK_ID").is_ok();

        let tray_service = match tray
            .disable_dbus_name(sandboxed)
            .assume_sni_available(true)
            .spawn()
        {
            Ok(service) => Some(service),
            Err(e) => {
                crate::log::Log::warn("TRAY", &format!("tray unavailable, continuing without it: {}", e));
                None
            }
        };

        (TrayHandle { sender: tx, tray_service }, rx)
    }

    struct SpacePodsTray {
        sender: Sender<TrayCommand>,
    }

    impl SpacePodsTray {
        fn read_anc_mode() -> usize {
            ANC_MODE_ATOMIC.load(std::sync::atomic::Ordering::Relaxed) as usize
        }
    }

    impl ksni::Tray for SpacePodsTray {
        fn id(&self) -> String {
            "com.github.imnotndesh.SpacePods.Tray".into()
        }

        fn title(&self) -> String {
            "SpacePods".into()
        }

        fn icon_name(&self) -> String {
            "com.github.imnotndesh.spacepods-linux".into()
        }

        fn tool_tip(&self) -> ksni::ToolTip {
            ksni::ToolTip {
                icon_name: "com.github.imnotndesh.spacepods-linux".into(),
                icon_pixmap: vec![],
                title: "SpacePods".into(),
                description: "Control your SpaceBuds".into(),
            }
        }

        /// Left-click sends coordinates for the popup.
        fn activate(&mut self, x: i32, y: i32) {
            let _ = self.sender.send(TrayCommand::ShowPopup { x, y });
        }

        fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
            use ksni::menu::*;

            vec![
                StandardItem {
                    label: "Show SpacePods".into(),
                    icon_name: "window-restore-symbolic".into(),
                    activate: Box::new(|this: &mut Self| {
                        let _ = this.sender.send(TrayCommand::ShowWindow);
                    }),
                    ..Default::default()
                }
                .into(),
                MenuItem::Separator,
                StandardItem {
                    label: "Noise Cancellation".into(),
                    icon_name: "org.gnome.Settings-accessibility-hearing-symbolic".into(),
                    enabled: false,
                    ..Default::default()
                }
                .into(),
                RadioGroup {
                    selected: Self::read_anc_mode(),
                    select: Box::new(|this: &mut Self, idx| {
                        let _ = this.sender.send(TrayCommand::SetAncMode(idx as u8));
                    }),
                    options: vec![
                        RadioItem { label: "Off".into(), ..Default::default() },
                        RadioItem { label: "ANC".into(), ..Default::default() },
                        RadioItem { label: "Transparency".into(), ..Default::default() },
                    ],
                    ..Default::default()
                }
                .into(),
                MenuItem::Separator,
                StandardItem {
                    label: "Hide SpacePods".into(),
                    icon_name: "window-minimize-symbolic".into(),
                    activate: Box::new(|this: &mut Self| {
                        let _ = this.sender.send(TrayCommand::HideWindow);
                    }),
                    ..Default::default()
                }
                .into(),
                MenuItem::Separator,
                StandardItem {
                    label: "Quit SpacePods".into(),
                    icon_name: "application-exit-symbolic".into(),
                    activate: Box::new(|this: &mut Self| {
                        let _ = this.sender.send(TrayCommand::Quit);
                    }),
                    ..Default::default()
                }
                .into(),
            ]
        }
    }
}
