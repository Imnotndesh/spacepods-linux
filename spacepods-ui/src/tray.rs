// src/tray.rs
//
// System tray icon using `ksni` (StatusNotifierItem / freedesktop spec).
// Supported on: GNOME Shell + AppIndicator extension, KDE Plasma, XFCE, etc.
//
// Cargo.toml additions:
//   ksni = { version = "0.3", features = ["blocking"] }
//   (ksni's blocking feature gives us a synchronous TrayMethods::spawn())
//
// Architecture
// ────────────
// ksni runs the tray on its own internal async runtime (separate thread).
// GTK is NOT thread-safe, so tray callbacks MUST NOT touch GTK widgets directly.
// Instead, tray callbacks send a `TrayCommand` over a `std::sync::mpsc` channel;
// the GTK main loop polls the channel via `glib::idle_add` and dispatches on
// the main thread.
//
// Usage (in main.rs / window setup):
//
//   let (tray_handle, tray_receiver) = tray::spawn_tray();
//
//   // Poll the receiver from the GTK main loop:
//   let window_ref = window.clone();
//   glib::idle_add(move || {
//       while let Ok(cmd) = tray_receiver.try_recv() {
//           match cmd {
//               TrayCommand::ShowWindow  => window_ref.present(),
//               TrayCommand::HideWindow  => window_ref.set_visible(false),
//               TrayCommand::Quit        => window_ref.close(),
//               TrayCommand::SetAncMode(m) => { /* call your BLE layer */ }
//               TrayCommand::SetEqPreset(p) => { /* call your BLE layer */ }
//               _ => {}
//           }
//       }
//       glib::ControlFlow::Continue
//   });

use std::sync::mpsc::{self, Receiver, Sender};
use ksni::TrayMethods;

#[derive(Debug, Clone)]
pub enum TrayCommand {
    ShowWindow,
    HideWindow,
    Quit,
    SetAncMode(u8),
    SetEqPreset(u8),
    Show,
    Hide,
}


#[derive(Clone)]
pub struct TrayHandle {
    sender: Sender<TrayCommand>,
    /// ksni handle — used to update the tray state (e.g. checked radio items)
    tray_service: ksni::Handle<SpacePodsTray>,
}

impl TrayHandle {
    /// Send a command to the tray service itself (e.g. Show/Hide).
    pub fn send(&self, cmd: TrayCommand) {
        match cmd {
            TrayCommand::Show => {
                self.tray_service.update(|t| t.visible = true);
            }
            TrayCommand::Hide => {
                self.tray_service.update(|t| t.visible = false);
            }
            other => {
                let _ = self.sender.send(other);
            }
        }
    }

    /// Update which ANC mode is shown as checked in the tray menu.
    pub fn set_anc_mode(&self, mode: u8) {
        self.tray_service
            .update(|t| t.anc_mode = mode as usize);
    }

    /// Update which EQ preset is shown as checked in the tray menu.
    pub fn set_eq_preset(&self, preset: u8) {
        self.tray_service
            .update(|t| t.eq_preset = preset as usize);
    }
}

pub async fn spawn_tray() -> (TrayHandle, Receiver<TrayCommand>) {
    let (tx, rx) = mpsc::channel::<TrayCommand>();

    let tray = SpacePodsTray {
        sender: tx.clone(),
        anc_mode: 0,
        eq_preset: 0,
        visible: false,
    };

    let service_handle = tray.spawn().await.expect("failed to spawn tray icon");

    let handle = TrayHandle {
        sender: tx,
        tray_service: service_handle,
    };

    (handle, rx)
}


#[derive(Debug)]
pub struct SpacePodsTray {
    sender: Sender<TrayCommand>,
    /// Index into ANC_MODES (0 = Off, 1 = ANC, 2 = Transparency)
    pub anc_mode: usize,
    /// Index into EQ_PRESET_NAMES
    pub eq_preset: usize,
    /// Whether the icon is currently shown
    pub visible: bool,
}

const ANC_MODES: [(&str, u8); 3] = [
    ("Off", 0),
    ("ANC", 1),
    ("Transparency", 2),
];

const EQ_PRESET_NAMES: [(&str, u8); 6] = [
    ("Flat", 0),
    ("Bass Boost", 1),
    ("Rock", 2),
    ("Jazz", 3),
    ("Vocal", 4),
    ("Treble Boost", 5),
];

impl ksni::Tray for SpacePodsTray {
    fn id(&self) -> String {
        "spacepods".into()
    }

    fn title(&self) -> String {
        "SpacePods".into()
    }

    /// Use a themed icon name so it adapts to the desktop's icon theme.
    fn icon_name(&self) -> String {
        "audio-headset-symbolic".into()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            icon_name: "audio-headset-symbolic".into(),
            icon_pixmap: vec![],
            title: "SpacePods".into(),
            description: format!(
                "ANC: {}  •  EQ: {}",
                ANC_MODES[self.anc_mode].0,
                EQ_PRESET_NAMES[self.eq_preset].0
            ),
        }
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

            StandardItem{
                label: "ANC Mode".into(),
                enabled: false,
                ..Default::default()
            }
            .into(),
            RadioGroup {
                selected: self.anc_mode,
                select: Box::new(|this: &mut Self, idx| {
                    this.anc_mode = idx;
                    let mode = ANC_MODES[idx].1;
                    let _ = this.sender.send(TrayCommand::SetAncMode(mode));
                }),
                options: ANC_MODES
                    .iter()
                    .map(|(label, _)| RadioItem {
                        label: (*label).into(),
                        ..Default::default()
                    })
                    .collect(),
            }
            .into(),

            MenuItem::Separator,

            StandardItem{
                label: "EQ Preset".into(),
                enabled: false,
                ..Default::default()
            }
            .into(),
            RadioGroup {
                selected: self.eq_preset,
                select: Box::new(|this: &mut Self, idx| {
                    this.eq_preset = idx;
                    let preset = EQ_PRESET_NAMES[idx].1;
                    let _ = this.sender.send(TrayCommand::SetEqPreset(preset));
                }),
                options: EQ_PRESET_NAMES
                    .iter()
                    .map(|(label, _)| RadioItem {
                        label: (*label).into(),
                        ..Default::default()
                    })
                    .collect(),
            }
            .into(),

            MenuItem::Separator,

            StandardItem {
                label: "Hide window".into(),
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

// ── Integration snippet for main.rs ──────────────────────────────────────────
//
// fn main() -> glib::ExitCode {
//     let app = adw::Application::builder()
//         .application_id("io.github.yourname.spacepods")
//         .build();
//
//     app.connect_activate(|app| {
//         let (tray_handle, tray_rx) = tray::spawn_tray();
//
//         let window = build_window(app, tray_handle.clone());
//
//         // Poll tray commands on every GTK idle tick
//         let win_ref = window.clone();
//         let tray_ref = tray_handle.clone();
//         glib::idle_add_local(move || {
//             while let Ok(cmd) = tray_rx.try_recv() {
//                 match cmd {
//                     TrayCommand::ShowWindow  => win_ref.present(),
//                     TrayCommand::HideWindow  => win_ref.set_visible(false),
//                     TrayCommand::Quit        => win_ref.close(),
//                     TrayCommand::SetAncMode(m) => {
//                         tray_ref.set_anc_mode(m);
//                         // TODO: send BLE command
//                     }
//                     TrayCommand::SetEqPreset(p) => {
//                         tray_ref.set_eq_preset(p);
//                         // TODO: send BLE command
//                     }
//                     _ => {}
//                 }
//             }
//             glib::ControlFlow::Continue
//         });
//
//         window.present();
//     });
//
//     app.run()
// }