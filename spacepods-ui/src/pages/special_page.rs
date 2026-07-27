use gtk4::prelude::*;
use gtk4::{Box, Orientation, DropDown, Image};
use libadwaita::prelude::*;
use libadwaita::{PreferencesGroup, ActionRow, Clamp};
use std::rc::Rc;
use std::cell::RefCell;

use crate::context::AppContext;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GestureAction {
    NoAction,
    ToggleAnc,
    CycleAnc,
    VolumeUp,
    VolumeDown,
    PlayPause,
    NextTrack,
    PrevTrack,
    VoiceAssistant,
    AmbientSound,
    QuickAmbient,
    LaunchApp,
}

impl GestureAction {
    fn label(&self) -> &'static str {
        match self {
            Self::NoAction => "No action",
            Self::ToggleAnc => "Toggle ANC",
            Self::CycleAnc => "Cycle ANC modes",
            Self::VolumeUp => "Volume up",
            Self::VolumeDown => "Volume down",
            Self::PlayPause => "Play / Pause",
            Self::NextTrack => "Next track",
            Self::PrevTrack => "Previous track",
            Self::VoiceAssistant => "Voice assistant",
            Self::AmbientSound => "Ambient sound",
            Self::QuickAmbient => "Quick ambient",
            Self::LaunchApp => "Launch app",
        }
    }

    fn all() -> Vec<Self> {
        use GestureAction::*;
        vec![
            NoAction, ToggleAnc, CycleAnc, VolumeUp, VolumeDown,
            PlayPause, NextTrack, PrevTrack, VoiceAssistant,
            AmbientSound, QuickAmbient, LaunchApp,
        ]
    }
}

/// The gesture types we can configure per ear.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GestureType {
    SingleTap,
    DoubleTap,
    TripleTap,
    LongPress,
}

impl GestureType {
    fn label(&self) -> &'static str {
        match self {
            Self::SingleTap => "Single tap",
            Self::DoubleTap => "Double tap",
            Self::TripleTap => "Triple tap",
            Self::LongPress => "Long press",
        }
    }

    fn all() -> Vec<Self> {
        use GestureType::*;
        vec![SingleTap, DoubleTap, TripleTap, LongPress]
    }
}

/// Per-ear configuration store.
struct EarConfig {
    single_tap: GestureAction,
    double_tap: GestureAction,
    triple_tap: GestureAction,
    long_press: GestureAction,
}

impl EarConfig {
    fn default_left() -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            single_tap: GestureAction::NoAction,
            double_tap: GestureAction::ToggleAnc,
            triple_tap: GestureAction::VoiceAssistant,
            long_press: GestureAction::QuickAmbient,
        }))
    }

    fn default_right() -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            single_tap: GestureAction::PlayPause,
            double_tap: GestureAction::NextTrack,
            triple_tap: GestureAction::PrevTrack,
            long_press: GestureAction::VoiceAssistant,
        }))
    }
}

pub struct SpecialPage;

impl SpecialPage {
    /// `ctx` isn't hit by a daemon round-trip yet (gesture mapping is still
    /// local-only), but it's threaded through like every other page so a
    /// future "send gesture map to daemon" call has consistent toast/error
    /// handling for free instead of silently swallowing failures.
    pub fn new(ctx: Rc<AppContext>) -> gtk4::Widget {
        let left_config = EarConfig::default_left();
        let right_config = EarConfig::default_right();

        let content = Box::new(Orientation::Vertical, 24);
        content.set_margin_top(16);
        content.set_margin_bottom(32);
        content.set_margin_start(16);
        content.set_margin_end(16);

        // ── Left ear ──
        let left_group = PreferencesGroup::new();
        left_group.set_title("Left Earbud");
        left_group.set_description(Some("Configure what happens when you interact with the left earbud"));

        let left_header = Box::new(Orientation::Horizontal, 8);
        left_header.set_halign(gtk4::Align::Start);
        let left_icon = Image::from_icon_name("audio-headset-left-symbolic");
        left_icon.set_pixel_size(24);
        left_icon.add_css_class("dim-label");
        left_header.append(&left_icon);
        let left_label = gtk4::Label::new(Some("Left Earbud"));
        left_label.add_css_class("heading");
        left_header.append(&left_label);

        Self::populate_gesture_rows(&left_group, &left_config, ctx.clone());

        // ── Right ear ──
        let right_group = PreferencesGroup::new();
        right_group.set_title("Right Earbud");
        right_group.set_description(Some("Configure what happens when you interact with the right earbud"));

        let right_header = Box::new(Orientation::Horizontal, 8);
        right_header.set_halign(gtk4::Align::Start);
        let right_icon = Image::from_icon_name("audio-headset-right-symbolic");
        right_icon.set_pixel_size(24);
        right_icon.add_css_class("dim-label");
        right_header.append(&right_icon);
        let right_label = gtk4::Label::new(Some("Right Earbud"));
        right_label.add_css_class("heading");
        right_header.append(&right_label);

        Self::populate_gesture_rows(&right_group, &right_config, ctx.clone());

        content.append(&left_group);
        content.append(&right_group);

        let clamp = Clamp::new();
        clamp.set_maximum_size(700);
        clamp.set_tightening_threshold(500);
        clamp.set_child(Some(&content));

        let scroll = gtk4::ScrolledWindow::new();
        scroll.set_hscrollbar_policy(gtk4::PolicyType::Never);
        scroll.set_vexpand(true);
        scroll.set_child(Some(&clamp));

        scroll.upcast()
    }

    fn populate_gesture_rows(group: &PreferencesGroup, config: &Rc<RefCell<EarConfig>>, ctx: Rc<AppContext>) {
        let all_actions = GestureAction::all();
        let labels: Vec<&str> = all_actions.iter().map(|a| a.label()).collect();

        for gesture in GestureType::all() {
            let row = ActionRow::new();
            row.set_title(gesture.label());

            let dropdown = DropDown::from_strings(&labels);
            dropdown.add_css_class("flat");

            // Set current value
            let current = {
                let cfg = config.borrow();
                match gesture {
                    GestureType::SingleTap => cfg.single_tap,
                    GestureType::DoubleTap => cfg.double_tap,
                    GestureType::TripleTap => cfg.triple_tap,
                    GestureType::LongPress => cfg.long_press,
                }
            };
            let idx = all_actions.iter().position(|a| *a == current).unwrap_or(0);
            dropdown.set_selected(idx as u32);

            // Wire up changes — clone actions per closure
            let cfg = Rc::clone(config);
            let g = gesture;
            let actions_clone = all_actions.clone();  // <── clone per iteration
            let ctx = ctx.clone();
            dropdown.connect_selected_item_notify(move |dd| {
                let i = dd.selected() as usize;
                if i < actions_clone.len() {
                    let mut c = cfg.borrow_mut();
                    match g {
                        GestureType::SingleTap => c.single_tap = actions_clone[i],
                        GestureType::DoubleTap => c.double_tap = actions_clone[i],
                        GestureType::TripleTap => c.triple_tap = actions_clone[i],
                        GestureType::LongPress => c.long_press = actions_clone[i],
                    }
                    ctx.toast(&format!("{} set to {}", g.label(), actions_clone[i].label()));
                }
            });

            row.add_suffix(&dropdown);
            group.add(&row);
        }
    }
}