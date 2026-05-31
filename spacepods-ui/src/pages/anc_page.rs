use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use libadwaita::prelude::*;
use gtk4::{
    Box as GtkBox, Button, Grid, Label, Orientation, Spinner, Switch, ToggleButton,
    Revealer, RevealerTransitionType, AlertDialog, Scale,
};
use libadwaita::{ActionRow, PreferencesGroup};
use tokio::sync::mpsc;
use crate::storage::load_settings;

pub struct AncPage;

fn battery_label(level: Option<u8>) -> String {
    match level {
        Some(v) => format!("{}%", v),
        None => "—".to_string(),
    }
}

fn battery_icon(level: Option<u8>) -> &'static str {
    match level {
        Some(v) if v > 80 => "battery-full-symbolic",
        Some(v) if v > 50 => "battery-good-symbolic",
        Some(v) if v > 20 => "battery-low-symbolic",
        Some(_) => "battery-caution-symbolic",
        None => "battery-missing-symbolic",
    }
}

fn make_battery_card(title: &str) -> (gtk4::Frame, gtk4::Image, Label) {
    let frame = gtk4::Frame::new(None);
    frame.add_css_class("card");
    frame.set_hexpand(true);

    let vbox = GtkBox::new(Orientation::Vertical, 4);
    vbox.set_margin_top(10);
    vbox.set_margin_bottom(10);
    vbox.set_margin_start(12);
    vbox.set_margin_end(12);

    let title_lbl = Label::new(Some(title));
    title_lbl.add_css_class("caption-heading");
    title_lbl.set_halign(gtk4::Align::Start);

    let icon = gtk4::Image::from_icon_name("battery-missing-symbolic");
    icon.set_icon_size(gtk4::IconSize::Large);
    icon.set_halign(gtk4::Align::Center);

    let val = Label::new(Some("—"));
    val.add_css_class("title-2");
    val.set_halign(gtk4::Align::Center);

    vbox.append(&title_lbl);
    vbox.append(&icon);
    vbox.append(&val);
    frame.set_child(Some(&vbox));
    (frame, icon, val)
}

impl AncPage {
    pub fn new(tx: mpsc::Sender<crate::ClientCommand>) -> GtkBox {
        let container = GtkBox::new(Orientation::Vertical, 0);
        container.set_hexpand(true);
        container.set_vexpand(true);

        // Disconnected banner (Revealer) – informational only, does not disable controls
        let disconnect_revealer = Revealer::new();
        disconnect_revealer.set_transition_type(RevealerTransitionType::SlideDown);
        disconnect_revealer.set_reveal_child(false);

        let banner_box = GtkBox::new(Orientation::Horizontal, 12);
        banner_box.set_margin_top(8);
        banner_box.set_margin_bottom(8);
        banner_box.set_margin_start(16);
        banner_box.set_margin_end(16);

        let disconnected_label = Label::new(Some("Disconnected – attempting to reconnect…"));
        disconnected_label.add_css_class("error");
        let reconnect_btn = Button::with_label("Reconnect Now");
        reconnect_btn.add_css_class("suggested-action");

        banner_box.append(&disconnected_label);
        banner_box.append(&reconnect_btn);
        disconnect_revealer.set_child(Some(&banner_box));
        container.append(&disconnect_revealer);

        let clamp = libadwaita::Clamp::new();
        clamp.set_maximum_size(600);
        clamp.set_tightening_threshold(480);
        clamp.set_hexpand(true);
        clamp.set_vexpand(true);

        let inner = GtkBox::new(Orientation::Vertical, 16);
        inner.set_margin_top(24);
        inner.set_margin_bottom(24);
        inner.set_margin_start(16);
        inner.set_margin_end(16);

        // Battery header
        let batt_header = GtkBox::new(Orientation::Horizontal, 0);
        let batt_title = Label::new(Some("Battery"));
        batt_title.add_css_class("heading");
        batt_title.set_hexpand(true);
        batt_title.set_halign(gtk4::Align::Start);

        let refresh_btn = Button::new();
        refresh_btn.set_icon_name("view-refresh-symbolic");
        refresh_btn.add_css_class("flat");
        refresh_btn.add_css_class("circular");
        refresh_btn.set_tooltip_text(Some("Refresh battery"));

        let batt_spinner = Spinner::new();
        batt_spinner.set_size_request(16, 16);
        batt_spinner.set_valign(gtk4::Align::Center);
        batt_spinner.set_visible(true);
        batt_spinner.start();

        batt_header.append(&batt_title);
        batt_header.append(&batt_spinner);
        batt_header.append(&refresh_btn);
        inner.append(&batt_header);

        // Battery grid setup
        let batt_grid = Grid::new();
        batt_grid.set_column_spacing(8);
        batt_grid.set_row_spacing(0);
        batt_grid.set_hexpand(true);

        let (left_card, left_icon, left_val) = make_battery_card("Left");
        let (case_card, case_icon, case_val) = make_battery_card("Case");
        let (right_card, right_icon, right_val) = make_battery_card("Right");

        batt_grid.attach(&left_card, 0, 0, 1, 1);
        batt_grid.attach(&case_card, 1, 0, 1, 1);
        batt_grid.attach(&right_card, 2, 0, 1, 1);
        inner.append(&batt_grid);

        // Noise Control setup
        let anc_group = PreferencesGroup::new();
        anc_group.set_title("Noise Control");

        let modes_box = GtkBox::new(Orientation::Horizontal, 0);
        modes_box.add_css_class("linked");
        modes_box.set_halign(gtk4::Align::Center);

        let off_btn = ToggleButton::with_label("Off");
        off_btn.set_hexpand(true);
        let anc_btn = ToggleButton::with_label("Noise Cancelling");
        anc_btn.set_hexpand(true);
        let trans_btn = ToggleButton::with_label("Transparency");
        trans_btn.set_hexpand(true);

        anc_btn.set_group(Some(&off_btn));
        trans_btn.set_group(Some(&off_btn));

        modes_box.append(&off_btn);
        modes_box.append(&anc_btn);
        modes_box.append(&trans_btn);
        anc_group.add(&modes_box);
        inner.append(&anc_group);

        // Sub-features setup: Noise Cancelling and Transparency sub-sliders
        let sub_features_revealer = Revealer::new();
        sub_features_revealer.set_transition_type(RevealerTransitionType::SlideDown);

        let features_group = PreferencesGroup::new();
        features_group.set_title("Mode Configuration");

        // Noise Cancelling level layout
        let anc_level_row = ActionRow::new();
        anc_level_row.set_title("Noise Cancelling Level");
        let anc_scale = Scale::with_range(Orientation::Horizontal, 0.0, 15.0, 1.0);
        anc_scale.set_size_request(200, -1);
        anc_scale.set_draw_value(true);
        anc_level_row.add_suffix(&anc_scale);
        features_group.add(&anc_level_row);

        let adaptive_row = ActionRow::new();
        adaptive_row.set_title("Adaptive ANC");
        adaptive_row.set_subtitle("Automatically match cancellation depth to atmospheric ambient pressure logs");
        let adaptive_switch = Switch::new();
        adaptive_switch.set_valign(gtk4::Align::Center);
        adaptive_row.add_suffix(&adaptive_switch);
        features_group.add(&adaptive_row);

        let trans_level_row = ActionRow::new();
        trans_level_row.set_title("Transparency Level");
        // FUH!! trans scale sio
        let trans_scale = Scale::with_range(Orientation::Horizontal, 0.0, 15.0, 1.0);
        trans_scale.set_size_request(200, -1);
        trans_scale.set_draw_value(true);
        trans_level_row.add_suffix(&trans_scale);
        features_group.add(&trans_level_row);

        sub_features_revealer.set_child(Some(&features_group));
        inner.append(&sub_features_revealer);

        let setting_from_status = Arc::new(AtomicBool::new(false));

        let update_sub_features_visibility = {
            let off_btn = off_btn.clone();
            let anc_btn = anc_btn.clone();
            let trans_btn = trans_btn.clone();
            let anc_level_row = anc_level_row.clone();
            let adaptive_row = adaptive_row.clone();
            let trans_level_row = trans_level_row.clone();
            let sub_features_revealer = sub_features_revealer.clone();

            move || {
                if off_btn.is_active() {
                    sub_features_revealer.set_reveal_child(false);
                } else {
                    sub_features_revealer.set_reveal_child(true);
                    if anc_btn.is_active() {
                        anc_level_row.set_visible(true);
                        adaptive_row.set_visible(true);
                        trans_level_row.set_visible(false);
                    } else if trans_btn.is_active() {
                        anc_level_row.set_visible(false);
                        adaptive_row.set_visible(false);
                        trans_level_row.set_visible(true);
                    }
                }
            }
        };


        {
            let tx = tx.clone();
            let setting_ref = Arc::clone(&setting_from_status);
            let update_visibility = update_sub_features_visibility.clone();
            off_btn.connect_toggled(move |btn| {
                update_visibility();
                if btn.is_active() && !setting_ref.load(Ordering::Relaxed) {
                    let tx = tx.clone();
                    glib::spawn_future_local(async move {
                        let _ = tx.send(crate::ClientCommand::SetAncMode("off".to_string())).await;
                    });
                }
            });
        }
        {
            let tx = tx.clone();
            let setting_ref = Arc::clone(&setting_from_status);
            let update_visibility = update_sub_features_visibility.clone();
            anc_btn.connect_toggled(move |btn| {
                update_visibility();
                if btn.is_active() && !setting_ref.load(Ordering::Relaxed) {
                    let tx = tx.clone();
                    glib::spawn_future_local(async move {
                        let _ = tx.send(crate::ClientCommand::SetAncMode("on".to_string())).await;
                    });
                }
            });
        }
        {
            let tx = tx.clone();
            let setting_ref = Arc::clone(&setting_from_status);
            let update_visibility = update_sub_features_visibility.clone();
            trans_btn.connect_toggled(move |btn| {
                update_visibility();
                if btn.is_active() && !setting_ref.load(Ordering::Relaxed) {
                    let tx = tx.clone();
                    glib::spawn_future_local(async move {
                        let _ = tx.send(crate::ClientCommand::SetAncMode("transparency".to_string())).await;
                    });
                }
            });
        }

        // Wire up sub-feature scale changes
        {
            let tx = tx.clone();
            let setting_ref = Arc::clone(&setting_from_status);
            anc_scale.connect_value_changed(move |scale| {
                if !setting_ref.load(Ordering::Relaxed) {
                    let tx = tx.clone();
                    let val = scale.value() as u8;
                    glib::spawn_future_local(async move {
                        let _ = tx.send(crate::ClientCommand::SetAncLevel(val)).await;
                    });
                }
            });
        }
        {
            let tx = tx.clone();
            let setting_ref = Arc::clone(&setting_from_status);
            trans_scale.connect_value_changed(move |scale| {
                if !setting_ref.load(Ordering::Relaxed) {
                    let tx = tx.clone();
                    let val = scale.value() as u8;
                    glib::spawn_future_local(async move {
                        let _ = tx.send(crate::ClientCommand::SetAncLevel(val)).await;
                    });
                }
            });
        }
        {
            let tx = tx.clone();
            let setting_ref = Arc::clone(&setting_from_status);
            adaptive_switch.connect_state_set(move |_, state| {
                if !setting_ref.load(Ordering::Relaxed) {
                    let tx = tx.clone();
                    glib::spawn_future_local(async move {
                        let _ = tx.send(crate::ClientCommand::SetAdaptiveAnc(state)).await;
                    });
                }
                glib::Propagation::Proceed
            });
        }

        // Wire up Utility items
        let utility_group = PreferencesGroup::new();
        utility_group.set_title("Utilities");

        let find_row = ActionRow::new();
        find_row.set_title("Find My Earbuds");
        find_row.set_subtitle("Plays a loud localized acoustic ping inside selected shell casings");

        let find_start = Button::with_label("Ring");
        let find_stop = Button::with_label("Stop");
        find_stop.add_css_class("destructive-action");

        let find_box = GtkBox::new(Orientation::Horizontal, 6);
        find_box.append(&find_start);
        find_box.append(&find_stop);
        find_row.add_suffix(&find_box);
        utility_group.add(&find_row);

        let reset_row = ActionRow::new();
        reset_row.set_title("Factory Reset");
        reset_row.set_subtitle("Clear saved pairs and defaults");

        let reset_button = Button::with_label("Reset");
        reset_button.add_css_class("destructive-action");
        reset_row.add_suffix(&reset_button);
        utility_group.add(&reset_row);
        inner.append(&utility_group);

        {
            let tx = tx.clone();
            find_start.connect_clicked(move |_| {
                let tx = tx.clone();
                glib::spawn_future_local(async move {
                    let _ = tx.send(crate::ClientCommand::FindDevice(true)).await;
                });
            });
        }
        {
            let tx = tx.clone();
            find_stop.connect_clicked(move |_| {
                let tx = tx.clone();
                glib::spawn_future_local(async move {
                    let _ = tx.send(crate::ClientCommand::FindDevice(false)).await;
                });
            });
        }
        {
            let tx = tx.clone();
            reset_button.connect_clicked(move |_| {
                let tx = tx.clone();
                let dialog = AlertDialog::builder()
                    .message("Factory reset will erase all settings and disconnect the earbuds. Continue?")
                    .buttons(vec!["Cancel", "Reset"])
                    .build();

                glib::spawn_future_local(async move {
                    if let Ok(index) = dialog.choose_future(None::<&gtk4::Window>).await {
                        if index == 1 {
                            let _ = tx.send(crate::ClientCommand::FactoryReset).await;
                        }
                    }
                });
            });
        }

        {
            let tx = tx.clone();
            refresh_btn.connect_clicked(move |_| {
                let tx = tx.clone();
                glib::spawn_future_local(async move {
                    let _ = tx.send(crate::ClientCommand::RefreshBattery).await;
                });
            });
        }

        clamp.set_child(Some(&inner));
        container.append(&clamp);
        container
    }
}