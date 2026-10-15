use gtk4::prelude::*;
use gtk4::{Box, DropDown, Label, Orientation};
use libadwaita::prelude::*;
use libadwaita::{PreferencesGroup, ActionRow, Clamp};
use std::cell::RefCell;
use std::rc::Rc;
use glib::clone;

use crate::context::AppContext;
use crate::log::Log;

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
struct GestureSlot { label: &'static str, key_type: u8, icon: &'static str }

fn left_slots() -> Vec<GestureSlot> { vec![
    GestureSlot { label: "Single tap", key_type: KEY_LEFT_SINGLE, icon: "input-touchpad-symbolic" },
    GestureSlot { label: "Double tap", key_type: KEY_LEFT_DOUBLE, icon: "view-dual-symbolic" },
    GestureSlot { label: "Triple tap", key_type: KEY_LEFT_TRIPLE, icon: "view-grid-symbolic" },
    GestureSlot { label: "Long press", key_type: KEY_LEFT_LONG, icon: "alarm-symbolic" },
]}

fn right_slots() -> Vec<GestureSlot> { vec![
    GestureSlot { label: "Single tap", key_type: KEY_RIGHT_SINGLE, icon: "input-touchpad-symbolic" },
    GestureSlot { label: "Double tap", key_type: KEY_RIGHT_DOUBLE, icon: "view-dual-symbolic" },
    GestureSlot { label: "Triple tap", key_type: KEY_RIGHT_TRIPLE, icon: "view-grid-symbolic" },
    GestureSlot { label: "Long press", key_type: KEY_RIGHT_LONG, icon: "alarm-symbolic" },
]}

/// How long a changed gesture icon stays highlighted.
const FLASH_MS: u64 = 700;

pub struct SpecialPage;

impl SpecialPage {
    pub fn new(ctx: Rc<AppContext>) -> gtk4::Widget {
        // Header: the window header carries Refresh, which triggers this one.
        let header_row = Box::new(Orientation::Horizontal, 0);
        let refresh_btn = gtk4::Button::from_icon_name("view-refresh-symbolic");
        refresh_btn.add_css_class("flat");
        refresh_btn.add_css_class("circular");
        refresh_btn.set_tooltip_text(Some("Refresh gestures"));
        refresh_btn.update_property(&[gtk4::accessible::Property::Label("Refresh gestures")]);
        refresh_btn.set_hexpand(true);
        refresh_btn.set_halign(gtk4::Align::End);
        header_row.append(&refresh_btn);
        refresh_btn.set_widget_name("page-refresh");
        header_row.set_visible(false);

        // Track rows we add ourselves so refresh can remove exactly those
        // (the public API removes children; walking PreferencesGroup internals
        // is not reliable and leads to duplicate rows on refresh).
        let left = Self::side_panel("Left earbud", "audio-headset-symbolic");
        let right = Self::side_panel("Right earbud", "audio-headset-symbolic");

        let content = Box::new(Orientation::Vertical, 16);
        content.set_margin_top(12);
        content.set_margin_bottom(32);
        content.set_margin_start(16);
        content.set_margin_end(16);
        content.append(&header_row);
        content.append(&left.card);
        content.append(&left.group);
        content.append(&right.card);
        content.append(&right.group);

        // Populate dropdowns
        Self::populate_group(&left.group, &left.summary, &left_slots(), ctx.clone(), left.rows.clone());
        Self::populate_group(&right.group, &right.summary, &right_slots(), ctx.clone(), right.rows.clone());

        // Refresh button
        {
            let ctx = ctx.clone();
            refresh_btn.connect_clicked(move |_| {
                let ctx = ctx.clone();
                let (lg, ls, lr) = (left.group.clone(), left.summary.clone(), left.rows.clone());
                let (rg, rs, rr) = (right.group.clone(), right.summary.clone(), right.rows.clone());
                glib::spawn_future_local(async move {
                    Self::refresh_group(&lg, &ls, &left_slots(), ctx.clone(), lr);
                    Self::refresh_group(&rg, &rs, &right_slots(), ctx.clone(), rr);
                    ctx.success("Gestures refreshed");
                });
            });
        }

        let clamp = Clamp::new();
        clamp.set_maximum_size(500);
        clamp.set_child(Some(&content));
        clamp.upcast()
    }

    /// One earbud: a header card with its name and a live summary, then its gestures.
    fn side_panel(title: &str, icon: &str) -> SidePanel {
        let card = Box::new(Orientation::Horizontal, 16);
        card.add_css_class("card");
        card.set_margin_top(4);
        card.set_margin_bottom(4);
        card.set_margin_start(4);
        card.set_margin_end(4);
        card.set_margin_top(8);
        let image = gtk4::Image::from_icon_name(icon);
        image.set_pixel_size(64);
        image.add_css_class("accent");
        image.set_margin_start(16);
        image.set_margin_top(12);
        image.set_margin_bottom(12);
        card.append(&image);

        let text = Box::new(Orientation::Vertical, 2);
        text.set_valign(gtk4::Align::Center);
        text.set_hexpand(true);
        let name = Label::new(Some(title));
        name.add_css_class("title-3");
        name.set_halign(gtk4::Align::Start);
        let summary = Label::new(Some("Loading gestures…"));
        summary.add_css_class("dim-label");
        summary.set_halign(gtk4::Align::Start);
        summary.set_wrap(true);
        text.append(&name);
        text.append(&summary);
        card.append(&text);

        let group = PreferencesGroup::new();
        group.set_title(&format!("{} gestures", title.trim_end_matches(" earbud")));

        SidePanel {
            card,
            group,
            summary,
            rows: Rc::new(RefCell::new(Vec::new())),
        }
    }

    fn populate_group(
        group: &PreferencesGroup,
        summary: &Label,
        slots: &[GestureSlot],
        ctx: Rc<AppContext>,
        rows: Rc<RefCell<Vec<gtk4::Widget>>>,
    ) {
        let all_actions = KeyAction::all();
        let labels: Vec<&str> = all_actions.iter().map(|a| a.label).collect();
        let slots_vec: Vec<GestureSlot> = slots.to_vec();

        glib::spawn_future_local(clone!(
            #[weak] group,
            #[weak] summary,
            #[strong] ctx,
        async move {
            let current_map = match libspacepods::client::SpacePodsClient::connect(None).await {
                Ok(mut client) => match client.get_status().await {
                    Ok(s) => s.key_settings.unwrap_or_default(),
                    Err(e) => {
                        summary.set_text("Couldn't read the gestures");
                        Log::warn("GESTURE", &format!("status read failed: {:?}", e));
                        std::collections::HashMap::new()
                    }
                },
                Err(_) => {
                    summary.set_text("Connect your earbuds to see gestures");
                    std::collections::HashMap::new()
                }
            };
            Log::full("GESTURE", &format!("Current key settings: {:?}", current_map));

            let mut set_count = 0;
            for slot in &slots_vec {
                let action_row = ActionRow::new();
                action_row.set_title(slot.label);

                // The current action is shown as the subtitle, so the row reads at a glance.
                let current = current_map
                    .get(&slot.key_type)
                    .and_then(|&func| all_actions.iter().find(|a| a.func == func))
                    .map(|a| a.label)
                    .unwrap_or("No action");
                if current != "No action" {
                    set_count += 1;
                }
                action_row.set_subtitle(current);

                let icon = gtk4::Image::from_icon_name(slot.icon);
                icon.set_pixel_size(20);
                icon.add_css_class("accent");
                action_row.add_prefix(&icon);

                let dropdown = DropDown::from_strings(&labels);
                dropdown.set_valign(gtk4::Align::Center);
                dropdown.update_property(&[gtk4::accessible::Property::Label(
                    &format!("{} action", slot.label),
                )]);
                if let Some(idx) = all_actions.iter().position(|a| Some(a.label) == Some(current)) {
                    dropdown.set_selected(idx as u32);
                }

                let key_type = slot.key_type;
                let actions = all_actions.clone();
                let ctx = ctx.clone();
                let action_row_ref = action_row.clone();
                let icon_ref = icon.clone();

                dropdown.connect_selected_item_notify(move |dd| {
                    let i = dd.selected() as usize;
                    if i >= actions.len() { return; }
                    let action = actions[i];
                    Log::info("GESTURE", &format!("Setting {:?} to {}", key_type, action.label));
                    action_row_ref.set_subtitle(action.label);

                    let ctx = ctx.clone();
                    let icon = icon_ref.clone();
                    let label = action.label;
                    let func = action.func;
                    glib::spawn_future_local(async move {
                        let result = match libspacepods::client::SpacePodsClient::connect(None).await {
                            Ok(mut client) => client.set_key_setting(key_type, func).await,
                            Err(e) => Err(e),
                        };
                        match result {
                            Ok(_) => {
                                ctx.success(&format!("{} set", label));
                                Self::flash(&icon);
                            }
                            Err(e) => ctx.failed("Couldn't save the gesture", &e),
                        }
                    });
                });

                action_row.add_suffix(&dropdown);
                group.add(&action_row);
                // Remember the row so refresh can remove exactly this widget.
                let row_widget: gtk4::Widget = action_row.clone().upcast();
                rows.borrow_mut().push(row_widget);
            }

            summary.set_text(&match set_count {
                0 => "No gestures set yet".to_string(),
                1 => "1 gesture set".to_string(),
                n => format!("{} gestures set", n),
            });
        }));
    }

    /// Briefly highlight an icon after a successful change. Only stock style classes are
    /// used, so the highlight follows the desktop's accent colour.
    fn flash(icon: &gtk4::Image) {
        icon.add_css_class("success");
        let icon = icon.downgrade();
        glib::timeout_add_local_once(std::time::Duration::from_millis(FLASH_MS), move || {
            if let Some(icon) = icon.upgrade() {
                icon.remove_css_class("success");
            }
        });
    }

    fn refresh_group(
        group: &PreferencesGroup,
        summary: &Label,
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
        Self::populate_group(group, summary, slots, ctx, rows);
    }
}

/// The widgets for one earbud.
struct SidePanel {
    card: Box,
    group: PreferencesGroup,
    summary: Label,
    rows: Rc<RefCell<Vec<gtk4::Widget>>>,
}
