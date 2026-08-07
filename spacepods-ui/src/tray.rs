use std::sync::mpsc::{self, Receiver, Sender};
use ksni::blocking::TrayMethods;

#[derive(Debug, Clone)]
pub enum TrayCommand {
    PresentWindow,
    HideWindow,
    Quit,
    SetAncMode(u8),
    SetEqPreset(u8),
}

#[derive(Clone)]
pub struct TrayHandle {
    sender: Sender<TrayCommand>,
    #[allow(dead_code)]
    tray_service: ksni::blocking::Handle<SpacePodsTray>,
}

impl TrayHandle {
    pub fn send(&self, cmd: TrayCommand) {
        let _ = self.sender.send(cmd);
    }

    /// Keep the tray's internal menu state in sync with the app.
    pub fn set_anc_mode(&self, mode: u8) {
        self.tray_service.update(|t| t.anc_mode = mode as usize);
    }

    pub fn set_eq_preset(&self, preset: u8) {
        self.tray_service.update(|t| t.eq_preset = preset as usize);
    }
}

/// Start the tray and return a control handle plus the incoming-command
/// receiver. The receiver is handed to the caller (app startup) so it can be
/// drained and dispatched to the window.
///
/// Uses ksni's synchronous API (the crate is built with the `blocking`
/// feature) so it can start outside an async runtime. `assume_sni_available(true)`
/// is set so hosts without an SNI implementation (e.g. stock GNOME without the
/// AppIndicator extension) degrade gracefully instead of failing the app.
///
/// In a Flatpak sandbox we disable owning a D-Bus well-known name (the spec
/// normally requires it, but sandboxes block it) — the session bus is still
/// reachable thanks to `--socket=session-bus`.
pub fn spawn_tray() -> (TrayHandle, Receiver<TrayCommand>) {
    let (tx, rx) = mpsc::channel::<TrayCommand>();

    let tray = SpacePodsTray {
        sender: tx.clone(),
        anc_mode: 0,
        eq_preset: 0,
    };

    let sandboxed = std::env::var("FLATPAK_ID").is_ok();

    let tray_service = tray
        .disable_dbus_name(sandboxed)
        .assume_sni_available(true)
        .spawn()
        .expect("failed to spawn tray icon");

    let handle = TrayHandle {
        sender: tx,
        tray_service,
    };

    (handle, rx)
}

#[derive(Debug)]
pub struct SpacePodsTray {
    sender: Sender<TrayCommand>,
    pub anc_mode: usize,
    pub eq_preset: usize,
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

fn anc_mode_name(idx: usize) -> &'static str {
    ANC_MODES.get(idx).map(|m| m.0).unwrap_or("Off")
}

fn eq_preset_name(idx: usize) -> &'static str {
    EQ_PRESET_NAMES.get(idx).map(|m| m.0).unwrap_or("Flat")
}

impl ksni::Tray for SpacePodsTray {
    fn id(&self) -> String {
        "spacepods".into()
    }

    fn title(&self) -> String {
        "SpacePods".into()
    }

    fn icon_name(&self) -> String {
        "com.spacepods.ui".into()
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;

        vec![
            // ── Show window ──
            StandardItem {
                label: "Show SpacePods".into(),
                icon_name: "window-restore-symbolic".into(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.sender.send(TrayCommand::PresentWindow);
                }),
                ..Default::default()
            }
                .into(),

            MenuItem::Separator,

            // ── ANC Mode header ──
            StandardItem {
                label: "ANC Mode".into(),
                icon_name: "org.gnome.Settings-accessibility-hearing-symbolic".into(),
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

            // ── EQ Preset header ──
            StandardItem {
                label: "EQ Preset".into(),
                icon_name: "audio-card-symbolic".into(),
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

            // ── Window controls ──
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

// Keep a small accessor so tooltip / future live state can reuse the mapping.
#[allow(dead_code)]
fn _tooltip_text(anc: usize, eq: usize) -> String {
    format!("ANC: {}  •  EQ: {}", anc_mode_name(anc), eq_preset_name(eq))
}
