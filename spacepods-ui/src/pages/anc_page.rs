use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use libadwaita::prelude::*;
use gtk4::{
    Box as GtkBox, Button, Grid, Label, Orientation, Spinner, Switch, ToggleButton,
    Revealer, RevealerTransitionType, AlertDialog, Scale,
};
use libadwaita::{ActionRow, PreferencesGroup};
use tokio::sync::mpsc;

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

        // State lock to prevent looping events when updating from device feedback
        let setting_from_status = Arc::new(AtomicBool::new(false));

        // 1. Reconnect Status Banner Row
        let disconnect_revealer = Self::build_disconnect_banner();
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

        // 2. Battery Status Module
        let (batt_header, refresh_btn) = Self::build_battery_header();
        inner.append(&batt_header);

        let (batt_grid, left_card, left_icon, left_val) = Self::build_battery_grid();
        // (Note: case_card and right_card items remain tracked if referenced elsewhere via layout index)
        inner.append(&batt_grid);

        // 3. Main Noise Control Profile Group
        let (anc_group, off_btn, anc_btn, trans_btn) = Self::build_noise_control_group();
        inner.append(&anc_group);

        // 4. Sub-features Level/Depth Mode Configurations Group
        let (sub_features_revealer, anc_scale, trans_scale, adaptive_switch, anc_level_row, adaptive_row, trans_level_row) =
            Self::build_mode_config_group();
        inner.append(&sub_features_revealer);

        // Visibility toggling handler logic loop for nested sub-sliders
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

        // Bind main Profile updates
        Self::wire_profile_buttons(&off_btn, &anc_btn, &trans_btn, &setting_from_status, tx.clone(), update_sub_features_visibility);
        Self::wire_sliders(&anc_scale, &trans_scale, &adaptive_switch, &setting_from_status, tx.clone());
        let features_section_group = Self::build_hardware_features_group(&setting_from_status, tx.clone());
        inner.append(&features_section_group);
        let utility_group = Self::build_utilities_group(tx.clone());
        inner.append(&utility_group);
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


    fn build_disconnect_banner() -> Revealer {
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
        disconnect_revealer
    }

    fn build_battery_header() -> (GtkBox, Button) {
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
        (batt_header, refresh_btn)
    }

    fn build_battery_grid() -> (Grid, gtk4::Frame, gtk4::Image, Label) {
        let batt_grid = Grid::new();
        batt_grid.set_column_spacing(8);
        batt_grid.set_row_spacing(0);
        batt_grid.set_hexpand(true);

        let (left_card, left_icon, left_val) = make_battery_card("Left");
        let (case_card, _, _) = make_battery_card("Case");
        let (right_card, _, _) = make_battery_card("Right");

        batt_grid.attach(&left_card, 0, 0, 1, 1);
        batt_grid.attach(&case_card, 1, 0, 1, 1);
        batt_grid.attach(&right_card, 2, 0, 1, 1);
        (batt_grid, left_card, left_icon, left_val)
    }

    fn build_noise_control_group() -> (PreferencesGroup, ToggleButton, ToggleButton, ToggleButton) {
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
        (anc_group, off_btn, anc_btn, trans_btn)
    }

    fn build_mode_config_group() -> (Revealer, Scale, Scale, Switch, ActionRow, ActionRow, ActionRow) {
        let sub_features_revealer = Revealer::new();
        sub_features_revealer.set_transition_type(RevealerTransitionType::SlideDown);

        let features_group = PreferencesGroup::new();
        features_group.set_title("Mode Configuration");

        let anc_level_row = ActionRow::new();
        anc_level_row.set_title("Noise Cancelling Level");
        let anc_scale = Scale::with_range(Orientation::Horizontal, 0.0, 15.0, 1.0);
        anc_scale.set_size_request(200, -1);
        anc_scale.set_draw_value(true);
        anc_level_row.add_suffix(&anc_scale);
        features_group.add(&anc_level_row);

        let adaptive_row = ActionRow::new();
        adaptive_row.set_title("Adaptive ANC");
        adaptive_row.set_subtitle("Automatically match cancellation depth to environmental ambient pressure logs");
        let adaptive_switch = Switch::new();
        adaptive_switch.set_valign(gtk4::Align::Center);
        adaptive_row.add_suffix(&adaptive_switch);
        features_group.add(&adaptive_row);

        let trans_level_row = ActionRow::new();
        trans_level_row.set_title("Transparency Level");
        let trans_scale = Scale::with_range(Orientation::Horizontal, 0.0, 15.0, 1.0);
        trans_scale.set_size_request(200, -1);
        trans_scale.set_draw_value(true);
        trans_level_row.add_suffix(&trans_scale);
        features_group.add(&trans_level_row);

        sub_features_revealer.set_child(Some(&features_group));
        (sub_features_revealer, anc_scale, trans_scale, adaptive_switch, anc_level_row, adaptive_row, trans_level_row)
    }

    /// NEW: Constructs the Adwaita PreferencesGroup for advanced feature toggles
    fn build_hardware_features_group(setting_ref: &Arc<AtomicBool>, tx: mpsc::Sender<crate::ClientCommand>) -> PreferencesGroup {
        let features_group = PreferencesGroup::new();
        features_group.set_title("Features");
        features_group.set_description(Some("Advanced custom hardware audio features"));

        // --- Spatial Audio Row ---
        let spatial_row = ActionRow::new();
        spatial_row.set_title("Spatial Audio");
        spatial_row.set_subtitle("Immersive 3D spatial acoustic tracking field mapping");
        let spatial_switch = Switch::new();
        spatial_switch.set_valign(gtk4::Align::Center);
        spatial_row.add_suffix(&spatial_switch);
        features_group.add(&spatial_row);

        // --- Dual Device Multipoint Row ---
        let multipoint_row = ActionRow::new();
        multipoint_row.set_title("Dual Device Multipoint");
        multipoint_row.set_subtitle("Seamless context switching between two source devices simultaneously");
        let multipoint_switch = Switch::new();
        multipoint_switch.set_valign(gtk4::Align::Center);
        multipoint_row.add_suffix(&multipoint_switch);
        features_group.add(&multipoint_row);
        
        let tx_spatial = tx.clone();
        let lock_spatial = Arc::clone(setting_ref);
        spatial_switch.connect_state_set(move |_, state| {
            // Check if loop lock flag is checked, if true, do nothing!
            if !lock_spatial.load(Ordering::Relaxed) {
                let tx = tx_spatial.clone();
                glib::spawn_future_local(async move {
                    let _ = tx.send(crate::ClientCommand::SetSpatialAudio(state)).await;
                });
            }
            glib::Propagation::Proceed
        });;

        // Wire Up Multipoint Switch Command
        let tx_multi = tx.clone();
        let lock_multi = Arc::clone(setting_ref);
        multipoint_switch.connect_state_set(move |_, state| {
            if !lock_multi.load(Ordering::Relaxed) {
                let tx = tx_multi.clone();
                glib::spawn_future_local(async move {
                    let _ = tx.send(crate::ClientCommand::SetMultiDevice(state)).await;
                });
            }
            glib::Propagation::Proceed
        });

        features_group
    }

    fn build_utilities_group(tx: mpsc::Sender<crate::ClientCommand>) -> PreferencesGroup {
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

        // Wire Up Utility Actions
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

        utility_group
    }

    fn wire_profile_buttons<F>(
        off_btn: &ToggleButton,
        anc_btn: &ToggleButton,
        trans_btn: &ToggleButton,
        setting_ref: &Arc<AtomicBool>,
        tx: mpsc::Sender<crate::ClientCommand>,
        update_visibility: F,
    ) where
        F: Fn() + Clone + 'static,
    {
        {
            let tx = tx.clone();
            let setting_ref = Arc::clone(setting_ref);
            let update = update_visibility.clone();
            off_btn.connect_toggled(move |btn| {
                update();
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
            let setting_ref = Arc::clone(setting_ref);
            let update = update_visibility.clone();
            anc_btn.connect_toggled(move |btn| {
                update();
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
            let setting_ref = Arc::clone(setting_ref);
            let update = update_visibility.clone();
            trans_btn.connect_toggled(move |btn| {
                update();
                if btn.is_active() && !setting_ref.load(Ordering::Relaxed) {
                    let tx = tx.clone();
                    glib::spawn_future_local(async move {
                        let _ = tx.send(crate::ClientCommand::SetAncMode("transparency".to_string())).await;
                    });
                }
            });
        }
    }

    fn wire_sliders(
        anc_scale: &Scale,
        trans_scale: &Scale,
        adaptive_switch: &Switch,
        setting_ref: &Arc<AtomicBool>,
        tx: mpsc::Sender<crate::ClientCommand>,
    ) {
        {
            let tx = tx.clone();
            let setting_ref = Arc::clone(setting_ref);
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
            let setting_ref = Arc::clone(setting_ref);
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
            let setting_ref = Arc::clone(setting_ref);
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
    }
}