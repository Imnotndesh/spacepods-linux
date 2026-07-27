use gtk4::prelude::*;
use gtk4::{Box, Orientation, DropDown};
use libadwaita::prelude::*;
use libadwaita::{PreferencesGroup, ActionRow, Clamp};
use std::rc::Rc;
use glib::clone;

use crate::context::AppContext;
use crate::log::Log;

// ── Protocol constants (from KeyRequest.java) ──

const CMD_KEY: u8 = 0x22;

const KEY_LEFT_SINGLE: u8 = 1;
const KEY_RIGHT_SINGLE: u8 = 2;
const KEY_LEFT_DOUBLE: u8 = 3;
const KEY_RIGHT_DOUBLE: u8 = 4;
const KEY_LEFT_TRIPLE: u8 = 5;
const KEY_RIGHT_TRIPLE: u8 = 6;
const KEY_LEFT_LONG: u8 = 7;
const KEY_RIGHT_LONG: u8 = 8;

const FUNC_NONE: u8 = 0;
const FUNC_CALLBACK: u8 = 1;
const FUNC_ASSISTANT: u8 = 2;
const FUNC_PREVIOUS: u8 = 3;
const FUNC_NEXT: u8 = 4;
const FUNC_VOL_UP: u8 = 5;
const FUNC_VOL_DOWN: u8 = 6;
const FUNC_PLAY_PAUSE: u8 = 7;
const FUNC_GAME_MODE: u8 = 8;
const FUNC_ANC_SWITCH: u8 = 9;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct KeyAction {
    label: &'static str,
    func: u8,
}

impl KeyAction {
    fn all() -> Vec<Self> {
        vec![
            Self { label: "No action", func: FUNC_NONE },
            Self { label: "Toggle ANC", func: FUNC_ANC_SWITCH },
            Self { label: "Play / Pause", func: FUNC_PLAY_PAUSE },
            Self { label: "Next track", func: FUNC_NEXT },
            Self { label: "Previous track", func: FUNC_PREVIOUS },
            Self { label: "Volume up", func: FUNC_VOL_UP },
            Self { label: "Volume down", func: FUNC_VOL_DOWN },
            Self { label: "Voice assistant", func: FUNC_ASSISTANT },
            Self { label: "Game mode", func: FUNC_GAME_MODE },
            Self { label: "Answer call", func: FUNC_CALLBACK },
        ]
    }
}

#[derive(Debug, Clone, Copy)]
struct GestureSlot {
    label: &'static str,
    key_type: u8,
}

impl GestureSlot {
    fn all() -> Vec<Self> {
        vec![
            Self { label: "Single tap", key_type: 0 }, // placeholder, set per-ear below
            Self { label: "Double tap", key_type: 0 },
            Self { label: "Triple tap", key_type: 0 },
            Self { label: "Long press", key_type: 0 },
        ]
    }
}

fn left_slots() -> Vec<GestureSlot> {
    vec![
        GestureSlot { label: "Single tap", key_type: KEY_LEFT_SINGLE },
        GestureSlot { label: "Double tap", key_type: KEY_LEFT_DOUBLE },
        GestureSlot { label: "Triple tap", key_type: KEY_LEFT_TRIPLE },
        GestureSlot { label: "Long press",   key_type: KEY_LEFT_LONG },
    ]
}

fn right_slots() -> Vec<GestureSlot> {
    vec![
        GestureSlot { label: "Single tap", key_type: KEY_RIGHT_SINGLE },
        GestureSlot { label: "Double tap", key_type: KEY_RIGHT_DOUBLE },
        GestureSlot { label: "Triple tap", key_type: KEY_RIGHT_TRIPLE },
        GestureSlot { label: "Long press",   key_type: KEY_RIGHT_LONG },
    ]
}

pub struct SpecialPage;

impl SpecialPage {
    pub fn new(ctx: Rc<AppContext>) -> gtk4::Widget {
        let content = Box::new(Orientation::Vertical, 24);
        content.set_margin_top(16);
        content.set_margin_bottom(32);
        content.set_margin_start(16);
        content.set_margin_end(16);

        // ── Left ear ──
        let left_group = PreferencesGroup::new();
        left_group.set_title("Left Earbud");
        left_group.set_description(Some("Configure what happens when you interact with the left earbud"));
        Self::populate_group(&left_group, &left_slots(), ctx.clone());

        // ── Right ear ──
        let right_group = PreferencesGroup::new();
        right_group.set_title("Right Earbud");
        right_group.set_description(Some("Configure what happens when you interact with the right earbud"));
        Self::populate_group(&right_group, &right_slots(), ctx.clone());

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

    fn populate_group(group: &PreferencesGroup, slots: &[GestureSlot], ctx: Rc<AppContext>) {
        let all_actions = KeyAction::all();
        let labels: Vec<&str> = all_actions.iter().map(|a| a.label).collect();
        let slots_vec: Vec<GestureSlot> = slots.to_vec();

        // Query daemon for current key settings
        glib::spawn_future_local(clone!(
            #[weak] group,
            #[strong] ctx,
        async move {
            let current_map = match libspacepods::client::SpacePodsClient::connect(None).await {
                Ok(mut client) => match client.get_status().await {
                    Ok(s) => s.key_settings.unwrap_or_default(),
                    Err(_) => std::collections::HashMap::new(),
                },
                Err(_) => std::collections::HashMap::new(),
            };
            Log::full("GESTURE", &format!("Current key settings: {:?}", current_map));

            // Build dropdowns in order; set values from status
            for slot in &slots_vec {
                let row = ActionRow::new();
                row.set_title(slot.label);

                let dropdown = DropDown::from_strings(&labels);
                dropdown.add_css_class("flat");

                // Set current value from device
                if let Some(&func) = current_map.get(&slot.key_type) {
                    if let Some(idx) = all_actions.iter().position(|a| a.func == func) {
                        dropdown.set_selected(idx as u32);
                    }
                }

                let key_type = slot.key_type;
                let actions = all_actions.clone();
                let ctx = ctx.clone();

                dropdown.connect_selected_item_notify(move |dd| {
                    let i = dd.selected() as usize;
                    if i >= actions.len() {
                        return;
                    }
                    let action = &actions[i];
                    let payload = vec![key_type, 0x01, action.func];
                    Log::info("GESTURE",
                        &format!("Setting {:?} to {} (payload {:02x?})", key_type, action.label, payload));

                    let cc = libspacepods::ipc::ServiceCommand::Custom {
                        command_id: CMD_KEY,
                        payload,
                    };
                    let ctx = ctx.clone();
                    let label = action.label;
                    glib::spawn_future_local(async move {
                        match libspacepods::client::SpacePodsClient::connect(None).await {
                            Ok(mut client) => {
                                match client.send_command_raw(cc).await {
                                    Ok(_) => ctx.success(&format!("{} set", label)),
                                    Err(e) => ctx.error(format!("Gesture failed: {}", e)),
                                }
                            }
                            Err(e) => ctx.daemon_unreachable(e),
                        }
                    });
                });

                row.add_suffix(&dropdown);
                group.add(&row);
            }
        }));
    }
}
