use gtk4::prelude::*;
use gtk4::{Box, Label, Orientation, DropDown};
use libadwaita::prelude::*;
use libadwaita::{PreferencesGroup, ActionRow, Clamp, StatusPage};
use std::cell::RefCell;
use std::rc::Rc;
use glib::clone;

use crate::context::AppContext;
use crate::log::Log;

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

#[derive(Debug, Clone, Copy)]
struct KeyAction { label: &'static str, func: u8 }

impl KeyAction {
    fn all() -> Vec<Self> { vec![
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
    ]}
}

#[derive(Debug, Clone, Copy)]
struct GestureSlot { label: &'static str, key_type: u8 }

fn left_slots() -> Vec<GestureSlot> { vec![
    GestureSlot { label: "Single tap", key_type: KEY_LEFT_SINGLE },
    GestureSlot { label: "Double tap", key_type: KEY_LEFT_DOUBLE },
    GestureSlot { label: "Triple tap", key_type: KEY_LEFT_TRIPLE },
    GestureSlot { label: "Long press",   key_type: KEY_LEFT_LONG },
]}

fn right_slots() -> Vec<GestureSlot> { vec![
    GestureSlot { label: "Single tap", key_type: KEY_RIGHT_SINGLE },
    GestureSlot { label: "Double tap", key_type: KEY_RIGHT_DOUBLE },
    GestureSlot { label: "Triple tap", key_type: KEY_RIGHT_TRIPLE },
    GestureSlot { label: "Long press",   key_type: KEY_RIGHT_LONG },
]}

pub struct SpecialPage;

impl SpecialPage {
    pub fn new(ctx: Rc<AppContext>) -> gtk4::Widget {
        // Header
        let header_row = Box::new(Orientation::Horizontal, 0);
        header_row.set_margin_top(24);
        header_row.set_margin_bottom(8);
        let title = Label::new(Some("Earbud Gestures"));
        title.add_css_class("title-1");
        title.set_halign(gtk4::Align::Start);
        title.set_hexpand(true);
        header_row.append(&title);
        let refresh_btn = gtk4::Button::from_icon_name("view-refresh-symbolic");
        refresh_btn.add_css_class("flat");
        refresh_btn.add_css_class("circular");
        refresh_btn.set_valign(gtk4::Align::Center);
        refresh_btn.set_tooltip_text(Some("Refresh status"));
        header_row.append(&refresh_btn);

        let left_group = PreferencesGroup::new();
        left_group.set_title("Left Earbud");
        left_group.set_description(Some("Tap actions for the left earbud"));

        let right_group = PreferencesGroup::new();
        right_group.set_title("Right Earbud");
        right_group.set_description(Some("Tap actions for the right earbud"));

        // Track rows we add ourselves so refresh can remove exactly those
        // (the public API removes children; walking PreferencesGroup internals
        // is not reliable and leads to duplicate rows on refresh).
        let left_rows: Rc<RefCell<Vec<gtk4::Widget>>> = Rc::new(RefCell::new(Vec::new()));
        let right_rows: Rc<RefCell<Vec<gtk4::Widget>>> = Rc::new(RefCell::new(Vec::new()));

        let content = Box::new(Orientation::Vertical, 12);
        content.set_margin_top(0);
        content.set_margin_bottom(32);
        content.set_margin_start(16);
        content.set_margin_end(16);
        content.append(&header_row);
        content.append(&left_group);
        content.append(&right_group);

        // Populate dropdowns
        Self::populate_group(&left_group, &left_slots(), ctx.clone(), left_rows.clone());
        Self::populate_group(&right_group, &right_slots(), ctx.clone(), right_rows.clone());

        // Refresh button
        {
            let ctx = ctx.clone();
            let left_group = left_group.clone();
            let right_group = right_group.clone();
            let left_rows = left_rows.clone();
            let right_rows = right_rows.clone();
            refresh_btn.connect_clicked(move |_| {
                let ctx = ctx.clone();
                let lg = left_group.clone();
                let rg = right_group.clone();
                let lr = left_rows.clone();
                let rr = right_rows.clone();
                glib::spawn_future_local(async move {
                    // Re-populate with fresh data
                    Self::refresh_group(&lg, &left_slots(), ctx.clone(), lr);
                    Self::refresh_group(&rg, &right_slots(), ctx.clone(), rr);
                    ctx.success("Gestures refreshed");
                });
            });
        }

        let clamp = Clamp::new();
        clamp.set_maximum_size(500);
        clamp.set_child(Some(&content));
        clamp.upcast()
    }

    fn populate_group(
        group: &PreferencesGroup,
        slots: &[GestureSlot],
        ctx: Rc<AppContext>,
        rows: Rc<RefCell<Vec<gtk4::Widget>>>,
    ) {
        let all_actions = KeyAction::all();
        let labels: Vec<&str> = all_actions.iter().map(|a| a.label).collect();
        let slots_vec: Vec<GestureSlot> = slots.to_vec();

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

            for slot in &slots_vec {
                let row = gtk4::Widget::from(ActionRow::new());
                let action_row = row.clone().downcast::<ActionRow>().unwrap();
                action_row.set_title(slot.label);

                let dropdown = DropDown::from_strings(&labels);
                dropdown.add_css_class("flat");

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
                    if i >= actions.len() { return; }
                    let action = &actions[i];
                    let payload = vec![key_type, 0x01, action.func];
                    Log::info("GESTURE", &format!("Setting {:?} to {}", key_type, action.label));

                    let cc = libspacepods::ipc::ServiceCommand::Custom { command_id: CMD_KEY, payload };
                    let ctx = ctx.clone();
                    let label = action.label;
                    glib::spawn_future_local(async move {
                        match libspacepods::client::SpacePodsClient::connect(None).await {
                            Ok(mut client) => match client.send_command_raw(cc).await {
                                Ok(_) => ctx.success(&format!("{} set", label)),
                                Err(e) => ctx.error(format!("Gesture: {}", e)),
                            },
                            Err(e) => ctx.daemon_unreachable(e),
                        }
                    });
                });

                action_row.add_suffix(&dropdown);
                group.add(&action_row);
                // Remember the row so refresh can remove exactly this widget.
                let row_widget: gtk4::Widget = action_row.clone().upcast();
                rows.borrow_mut().push(row_widget);
            }
        }));
    }

    fn refresh_group(
        group: &PreferencesGroup,
        slots: &[GestureSlot],
        ctx: Rc<AppContext>,
        rows: Rc<RefCell<Vec<gtk4::Widget>>>,
    ) {
        // Remove exactly the rows we created. PreferencesGroup::remove is the
        // public API for this; walking the widget internals (as before) does not
        // find the ActionRows and caused duplicates on every refresh.
        let to_remove = std::mem::take(&mut *rows.borrow_mut());
        for row in to_remove {
            group.remove(&row);
        }
        Self::populate_group(group, slots, ctx, rows);
    }
}
