//! System-tray icon that opens a custom quick-settings popup when clicked.
//!
//! The tray *icon* is managed by ksni (StatusNotifierItem on DBus session bus).
//! Left-click fires activate(x, y) with screen coordinates. We dispatch a
//! command to the GTK main loop which opens a standalone quick-settings
//! popup near the icon, matching the GNOME-style quick-settings panel.

use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};

use ksni;
use ksni::blocking::TrayMethods;
use ksni::blocking::Handle as TrayHandleInner;

#[derive(Debug, Clone)]
pub enum TrayCommand {
    /// Show the quick-settings popup at (x, y) screen coords.
    ShowPopup { x: i32, y: i32 },
    /// Show the main application window.
    ShowWindow,
    /// Hide the main application window.
    HideWindow,
    /// Real quit — bypasses close-to-background.
    Quit,
    /// Set ANC mode (0=off, 1=anc, 2=transparency).
    SetAncMode(u8),
}

/// Shared cell so the tray menu can read back the current ANC mode. Populated
/// by `run_app` after the app context is created.
use std::sync::atomic::AtomicU8;
pub static ANC_MODE_ATOMIC: AtomicU8 = AtomicU8::new(0);

/// The tray handle is the only interface the rest of the app sees.
/// Commands are pushed through the sender; the receiver feeds the
/// GTK main-loop poll (see `app.rs`).
#[derive(Clone)]
pub struct TrayHandle {
    sender: Sender<TrayCommand>,
    #[allow(dead_code)]
    tray_service: TrayHandleInner<SpacePodsTray>,
}

impl TrayHandle {
    pub fn send(&self, cmd: TrayCommand) {
        let _ = self.sender.send(cmd);
    }
}

/// Start the tray and return its handle + the incoming-command stream.
pub fn spawn_tray() -> (TrayHandle, Receiver<TrayCommand>) {
    let (tx, rx) = mpsc::channel();

    let tray = SpacePodsTray { sender: tx.clone() };

    let sandboxed = std::env::var("FLATPAK_ID").is_ok();

    let tray_service = tray
        .disable_dbus_name(sandboxed)
        .assume_sni_available(true)
        .spawn()
        .expect("failed to start tray icon");

    (TrayHandle { sender: tx, tray_service }, rx)
}

// ───────────────────────────────────────────────────────────────────────
// ksni Trait impl — purely an icon + activation relay
// ───────────────────────────────────────────────────────────────────────

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
        "com.spacepods.ui.tray".into()
    }

    fn title(&self) -> String {
        "SpacePods".into()
    }

    fn icon_name(&self) -> String {
        "com.spacepods.ui".into()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            icon_name: "com.spacepods.ui".into(),
            icon_pixmap: vec![],
            title: "SpacePods".into(),
            description: "Control your SpaceBuds".into(),
        }
    }

    /// Left-click → send coordinates for the popup.
    fn activate(&mut self, x: i32, y: i32) {
        let _ = self.sender.send(TrayCommand::ShowPopup { x, y });
    }

    /// Right-click menu — includes ANC toggles that sync with the app
    /// via the shared anc_mode cell, plus window controls.
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

            // ── ANC mode — mirrors the shared cell in AppContext ──
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
