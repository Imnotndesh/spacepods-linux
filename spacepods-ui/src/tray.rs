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

/// TODO: Work on:
/// 1. Linking this tray to the ui
/// 2. Fixing battery to sync to ui
/// 3. Figure out the extension situation for gnome and KDE


#[derive(Clone)]
pub struct TrayHandle {
    sender: Sender<TrayCommand>,
    tray_service: ksni::Handle<SpacePodsTray>,
}

impl TrayHandle {
    pub fn send(&self, cmd: TrayCommand) {
        match cmd {
            TrayCommand::Show => { let _ = self.tray_service.update(|t| t.visible = true); }
            TrayCommand::Hide => { let _ = self.tray_service.update(|t| t.visible = false); }
            other => { let _ = self.sender.send(other); }
        }
    }

    pub fn set_anc_mode(&self, mode: u8) {
        self.tray_service.update(|t| t.anc_mode = mode as usize);
    }

    pub fn set_eq_preset(&self, preset: u8) {
        self.tray_service.update(|t| t.eq_preset = preset as usize);
    }
    pub fn set_status(&self, device_name: String, battery_left: Option<u8>, battery_right: Option<u8>, battery_case: Option<u8>, connected: bool) {
        self.tray_service.update(|t| {
            t.device_name = device_name.clone();
            t.battery_left = battery_left;
            t.battery_right = battery_right;
            t.battery_case = battery_case;
            t.connected = connected;
        });
    }
}

pub async fn spawn_tray() -> (TrayHandle, Receiver<TrayCommand>) {
    let (tx, rx) = mpsc::channel::<TrayCommand>();

    let tray = SpacePodsTray {
        sender: tx.clone(),
        anc_mode: 0,
        eq_preset: 0,
        visible: false,
        device_name: String::new(),
        battery_left: None,
        battery_right: None,
        battery_case: None,
        connected: false,
    };

    let service_handle = tray.spawn().await.expect("failed to spawn tray icon");
    let handle = TrayHandle { sender: tx, tray_service: service_handle };
    (handle, rx)
}

#[derive(Debug)]
pub struct SpacePodsTray {
    sender: Sender<TrayCommand>,
    pub anc_mode: usize,
    pub eq_preset: usize,
    pub visible: bool,
    pub device_name: String,
    pub battery_left: Option<u8>,
    pub battery_right: Option<u8>,
    pub battery_case: Option<u8>,
    pub connected: bool,
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

fn battery_icon(level: u8) -> &'static str {
    match level {
        81..=100 => "󰁹",
        61..=80  => "󰂁",
        41..=60  => "󰁿",
        21..=40  => "󰁽",
        _        => "󰁻",
    }
}

impl ksni::Tray for SpacePodsTray {
    fn id(&self) -> String { "spacepods".into() }
    fn title(&self) -> String { "SpacePods".into() }

    fn icon_name(&self) -> String {
            "audio-headset-symbolic".into()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        let status_line = if !self.connected {
            "Not connected".to_string()
        } else {
            let anc = ANC_MODES[self.anc_mode].0;
            let eq = EQ_PRESET_NAMES[self.eq_preset.min(5)].0;
            let batt = match (self.battery_left, self.battery_right) {
                (Some(l), Some(r)) => format!("  •  L: {}%  R: {}%", l, r),
                (Some(b), None) => format!("  •  {}%", b),
                _ => String::new(),
            };
            format!("ANC: {}  •  EQ: {}{}", anc, eq, batt)
        };

        ksni::ToolTip {
            icon_name: "audio-headset-symbolic".into(),
            icon_pixmap: vec![],
            title: if self.device_name.is_empty() {
                "SpacePods".into()
            } else {
                self.device_name.clone()
            },
            description: status_line,
        }
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;

        let batt_label = match (self.battery_left, self.battery_right, self.battery_case) {
            (Some(l), Some(r), Some(c)) => format!("L: {}%   R: {}%   Case: {}%", l, r, c),
            (Some(l), Some(r), None)    => format!("Left: {}%   Right: {}%", l, r),
            (Some(b), None, _)          => format!("Battery: {}%", b),
            _                           => "Battery: unknown".into(),
        };

        let device_label = if self.device_name.is_empty() {
            "SpacePods".to_string()
        } else {
            self.device_name.clone()
        };

        let mut items: Vec<ksni::MenuItem<Self>> = vec![
            StandardItem {
                label: device_label,
                enabled: false,
                ..Default::default()
            }.into(),

            StandardItem {
                label: batt_label,
                icon_name: "battery-symbolic".into(),
                enabled: false,
                ..Default::default()
            }.into(),

            MenuItem::Separator,

            StandardItem {
                label: "Open SpacePods".into(),
                icon_name: "audio-headset-symbolic".into(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.sender.send(TrayCommand::ShowWindow);
                }),
                ..Default::default()
            }.into(),

            MenuItem::Separator,

            StandardItem {
                label: "Noise Control".into(),
                enabled: false,
                ..Default::default()
            }.into(),

            RadioGroup {
                selected: self.anc_mode,
                select: Box::new(|this: &mut Self, idx| {
                    this.anc_mode = idx;
                    let _ = this.sender.send(TrayCommand::SetAncMode(ANC_MODES[idx].1));
                }),
                options: ANC_MODES.iter().map(|(label, _)| RadioItem {
                    label: (*label).into(),
                    ..Default::default()
                }).collect(),
            }.into(),

            MenuItem::Separator,

            StandardItem {
                label: "Equalizer".into(),
                enabled: false,
                ..Default::default()
            }.into(),

            RadioGroup {
                selected: self.eq_preset.min(5),
                select: Box::new(|this: &mut Self, idx| {
                    this.eq_preset = idx;
                    let _ = this.sender.send(TrayCommand::SetEqPreset(EQ_PRESET_NAMES[idx].1));
                }),
                options: EQ_PRESET_NAMES.iter().map(|(label, _)| RadioItem {
                    label: (*label).into(),
                    ..Default::default()
                }).collect(),
            }.into(),

            MenuItem::Separator,

            StandardItem {
                label: "Hide".into(),
                icon_name: "window-minimize-symbolic".into(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.sender.send(TrayCommand::HideWindow);
                }),
                ..Default::default()
            }.into(),

            StandardItem {
                label: "Quit".into(),
                icon_name: "application-exit-symbolic".into(),
                activate: Box::new(|this: &mut Self| {
                    let _ = this.sender.send(TrayCommand::Quit);
                }),
                ..Default::default()
            }.into(),
        ];

        items
    }
}