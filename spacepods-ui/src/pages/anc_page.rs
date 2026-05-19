use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Mutex;
use libspacepods::client::SpacePodsClient;
use libadwaita::prelude::*;
use gtk4::{
    Box as GtkBox, Button, Grid, Label, Orientation, Spinner, Switch, ToggleButton,
    Revealer, RevealerTransitionType, AlertDialog,
};
use libadwaita::{ActionRow, PreferencesGroup};
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
    pub fn new(client: Arc<Mutex<SpacePodsClient>>) -> GtkBox {
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

        // Battery grid
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

        // Noise Control
        let nc_header = GtkBox::new(Orientation::Horizontal, 0);
        let title = Label::new(Some("Noise Control"));
        title.add_css_class("title-1");
        title.set_halign(gtk4::Align::Start);
        title.set_hexpand(true);
        nc_header.append(&title);

        let off_btn = ToggleButton::with_label("OFF");
        let anc_btn = ToggleButton::with_label("ANC");
        let trans_btn = ToggleButton::with_label("Transparency");

        anc_btn.set_group(Some(&off_btn));
        trans_btn.set_group(Some(&off_btn));

        off_btn.set_hexpand(true);
        anc_btn.set_hexpand(true);
        trans_btn.set_hexpand(true);

        let buttons_box = GtkBox::new(Orientation::Horizontal, 8);
        buttons_box.set_hexpand(true);
        buttons_box.append(&off_btn);
        buttons_box.append(&anc_btn);
        buttons_box.append(&trans_btn);

        // Level buttons
        let level_box = GtkBox::new(Orientation::Vertical, 8);
        level_box.set_hexpand(true);
        level_box.set_visible(false);

        let level_label = Label::new(Some("Intensity"));
        level_label.set_halign(gtk4::Align::Start);
        level_label.add_css_class("caption");

        let low_btn = ToggleButton::with_label("Low");
        let med_btn = ToggleButton::with_label("Med");
        let high_btn = ToggleButton::with_label("High");
        med_btn.set_group(Some(&low_btn));
        high_btn.set_group(Some(&low_btn));

        low_btn.set_hexpand(true);
        med_btn.set_hexpand(true);
        high_btn.set_hexpand(true);

        let level_buttons_box = GtkBox::new(Orientation::Horizontal, 8);
        level_buttons_box.set_hexpand(true);
        level_buttons_box.append(&low_btn);
        level_buttons_box.append(&med_btn);
        level_buttons_box.append(&high_btn);

        level_box.append(&level_label);
        level_box.append(&level_buttons_box);

        // Additional Features
        let adaptive_row = ActionRow::new();
        adaptive_row.set_title("Adaptive ANC");
        adaptive_row.set_subtitle("Dynamically adjust based on environment");
        let adaptive_switch = Switch::new();
        adaptive_switch.set_valign(gtk4::Align::Center);
        adaptive_switch.set_vexpand(false);
        adaptive_row.add_suffix(&adaptive_switch);
        adaptive_row.set_activatable_widget(Some(&adaptive_switch));

        let dual_row = ActionRow::new();
        dual_row.set_title("Dual Device (Multi-point)");
        dual_row.set_subtitle("Connect to two devices simultaneously");
        let dual_switch = Switch::new();
        dual_switch.set_valign(gtk4::Align::Center);
        dual_switch.set_vexpand(false);
        dual_row.add_suffix(&dual_switch);
        dual_row.set_activatable_widget(Some(&dual_switch));

        let features_group = PreferencesGroup::new();
        features_group.set_title("Additional Features");
        features_group.set_valign(gtk4::Align::Start);
        features_group.set_vexpand(false);
        features_group.add(&adaptive_row);
        features_group.add(&dual_row);

        // Advanced Features
        let game_row = ActionRow::new();
        game_row.set_title("Game Mode");
        game_row.set_subtitle("Low latency mode for gaming");
        let game_switch = Switch::new();
        game_switch.set_valign(gtk4::Align::Center);
        game_switch.set_vexpand(false);
        game_row.add_suffix(&game_switch);
        game_row.set_activatable_widget(Some(&game_switch));

        let find_row = ActionRow::new();
        find_row.set_title("Find Device");
        find_row.set_subtitle("Make earbuds beep to locate them");
        let find_box = GtkBox::new(Orientation::Horizontal, 6);
        let find_start = Button::with_label("Start");
        find_start.add_css_class("suggested-action");
        let find_stop = Button::with_label("Stop");
        find_stop.add_css_class("destructive-action");
        find_box.append(&find_start);
        find_box.append(&find_stop);
        find_row.add_suffix(&find_box);

        let reset_row = ActionRow::new();
        reset_row.set_title("Factory Reset");
        reset_row.set_subtitle("Reset all settings (irreversible)");
        let reset_button = Button::with_label("Reset");
        reset_button.add_css_class("destructive-action");
        reset_row.add_suffix(&reset_button);

        let extra_group = PreferencesGroup::new();
        extra_group.set_title("Advanced Features");
        extra_group.add(&game_row);
        extra_group.add(&find_row);
        extra_group.add(&reset_row);

        inner.append(&batt_header);
        inner.append(&batt_grid);
        inner.append(&nc_header);
        inner.append(&buttons_box);
        inner.append(&level_box);
        inner.append(&features_group);
        inner.append(&extra_group);

        clamp.set_child(Some(&inner));
        let scroll = gtk4::ScrolledWindow::new();
        scroll.set_hscrollbar_policy(gtk4::PolicyType::Never);
        scroll.set_vexpand(true);
        scroll.set_child(Some(&clamp));
        container.append(&scroll);

        // Thread-safe state
        let setting_from_status = Arc::new(AtomicBool::new(false));
        let anc_max = Rc::new(Cell::new(15u8));

        // Helper to set level buttons based on numeric level
        let set_level_buttons = {
            let low_btn = low_btn.clone();
            let med_btn = med_btn.clone();
            let high_btn = high_btn.clone();
            let anc_max = anc_max.clone();
            let setting_ref = setting_from_status.clone();
            Rc::new(move |level: u8| {
                let max = anc_max.get().max(3);
                let third = (max / 3).max(1);
                setting_ref.store(true, Ordering::Relaxed);
                if level <= third {
                    low_btn.set_active(true);
                    low_btn.add_css_class("suggested-action");
                    med_btn.remove_css_class("suggested-action");
                    high_btn.remove_css_class("suggested-action");
                } else if level <= third * 2 {
                    med_btn.set_active(true);
                    med_btn.add_css_class("suggested-action");
                    low_btn.remove_css_class("suggested-action");
                    high_btn.remove_css_class("suggested-action");
                } else {
                    high_btn.set_active(true);
                    high_btn.add_css_class("suggested-action");
                    low_btn.remove_css_class("suggested-action");
                    med_btn.remove_css_class("suggested-action");
                }
                setting_ref.store(false, Ordering::Relaxed);
            })
        };

        // Helper to update battery icons and labels
        let update_battery = {
            let left_icon = left_icon.clone();
            let left_val = left_val.clone();
            let case_icon = case_icon.clone();
            let case_val = case_val.clone();
            let right_icon = right_icon.clone();
            let right_val = right_val.clone();
            Rc::new(move |bl: Option<u8>, bc: Option<u8>, br: Option<u8>| {
                left_val.set_text(&battery_label(bl));
                case_val.set_text(&battery_label(bc));
                right_val.set_text(&battery_label(br));
                left_icon.set_icon_name(Some(battery_icon(bl)));
                case_icon.set_icon_name(Some(battery_icon(bc)));
                right_icon.set_icon_name(Some(battery_icon(br)));
            })
        };

        // Load saved settings
        {
            let saved = load_settings();
            setting_from_status.store(true, Ordering::Relaxed);
            match saved.last_anc_mode {
                1 => {
                    anc_btn.set_active(true);
                    anc_btn.add_css_class("suggested-action");
                    level_box.set_visible(true);
                    adaptive_switch.set_sensitive(true);
                    adaptive_row.set_sensitive(true);
                }
                2 => {
                    trans_btn.set_active(true);
                    trans_btn.add_css_class("suggested-action");
                    level_box.set_visible(true);
                    adaptive_switch.set_sensitive(false);
                    adaptive_row.set_sensitive(false);
                }
                _ => {
                    off_btn.set_active(true);
                    off_btn.add_css_class("suggested-action");
                    level_box.set_visible(false);
                    adaptive_switch.set_sensitive(false);
                    adaptive_row.set_sensitive(false);
                }
            }
            set_level_buttons(saved.last_anc_level);
            adaptive_switch.set_active(saved.adaptive_anc_enabled);
            dual_switch.set_active(saved.dual_device_enabled);
            setting_from_status.store(false, Ordering::Relaxed);
        }

        // Initial battery fetch
        {
            let client = Arc::clone(&client);
            let update_batt = update_battery.clone();
            let batt_spin = batt_spinner.clone();
            glib::spawn_future_local(async move {
                if let Ok(s) = { let mut c = client.lock().await; c.get_status().await } {
                    update_batt(s.battery_left, s.battery_case, s.battery_right);
                }
                batt_spin.stop();
                batt_spin.set_visible(false);
            });
        }

        // Refresh button
        {
            let client = Arc::clone(&client);
            let update_batt = update_battery.clone();
            let batt_spin = batt_spinner.clone();
            refresh_btn.connect_clicked(move |btn| {
                btn.set_sensitive(false);
                batt_spin.set_visible(true);
                batt_spin.start();
                let client = Arc::clone(&client);
                let update_batt = update_batt.clone();
                let btn2 = btn.clone();
                let spin2 = batt_spin.clone();
                glib::spawn_future_local(async move {
                    if let Ok(s) = { let mut c = client.lock().await; c.get_status().await } {
                        update_batt(s.battery_left, s.battery_case, s.battery_right);
                    }
                    spin2.stop();
                    spin2.set_visible(false);
                    btn2.set_sensitive(true);
                });
            });
        }

        // Real‑time status subscription (updates battery, connected state, etc.)
        let client_sub = Arc::clone(&client);
        let update_batt_sub = update_battery.clone();
        let set_level_buttons_sub = set_level_buttons.clone();
        let setting_flag_sub = setting_from_status.clone();
        let anc_btns_sub = (off_btn.clone(), anc_btn.clone(), trans_btn.clone());
        let adaptive_sw_sub = adaptive_switch.clone();
        let dual_sw_sub = dual_switch.clone();
        let game_sw_sub = game_switch.clone();
        let level_box_sub = level_box.clone();
        let disconnect_revealer_sub = disconnect_revealer.clone();

        glib::spawn_future_local(async move {
            let mut sub_client = client_sub.lock().await;
            let mut rx = match sub_client.subscribe().await {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Failed to subscribe to status updates: {}", e);
                    return;
                }
            };
            drop(sub_client);

            while let Ok(status) = rx.recv().await {
                let update_batt = update_batt_sub.clone();
                let set_level = set_level_buttons_sub.clone();
                let setting_flag = setting_flag_sub.clone();
                let (off_btn, anc_btn, trans_btn) = anc_btns_sub.clone();
                let adaptive_sw = adaptive_sw_sub.clone();
                let dual_sw = dual_sw_sub.clone();
                let game_sw = game_sw_sub.clone();
                let level_box_ref = level_box_sub.clone();
                let disconnect_revealer = disconnect_revealer_sub.clone();

                glib::idle_add_local(move || {
                    // Update battery
                    update_batt(status.battery_left, status.battery_case, status.battery_right);
                    // Show/hide disconnect banner (informational only)
                    let connected = status.connected;
                    disconnect_revealer.set_reveal_child(!connected);
                    // IMPORTANT: Do NOT disable widgets based on connected flag.
                    // The user should be able to interact even if the service reports disconnected.
                    if !connected {
                        return glib::ControlFlow::Continue;
                    }
                    // Update UI with current device state
                    setting_flag.store(true, Ordering::Relaxed);
                    match status.anc_mode {
                        Some(1) => {
                            anc_btn.set_active(true);
                            anc_btn.add_css_class("suggested-action");
                            level_box_ref.set_visible(true);
                        }
                        Some(2) => {
                            trans_btn.set_active(true);
                            trans_btn.add_css_class("suggested-action");
                            level_box_ref.set_visible(true);
                        }
                        _ => {
                            off_btn.set_active(true);
                            off_btn.add_css_class("suggested-action");
                            level_box_ref.set_visible(false);
                        }
                    }
                    set_level(status.anc_level);
                    adaptive_sw.set_active(status.adaptive_anc.unwrap_or(false));
                    dual_sw.set_active(status.dual_device.unwrap_or(false));
                    game_sw.set_active(status.game_mode.unwrap_or(false));
                    setting_flag.store(false, Ordering::Relaxed);
                    glib::ControlFlow::Continue
                });
            }
        });

        // Reconnect button
        let client_reconnect = Arc::clone(&client);
        reconnect_btn.connect_clicked(move |_| {
            let client = Arc::clone(&client_reconnect);
            glib::spawn_future_local(async move {
                let mut c = client.lock().await;
                let _ = c.connect_device("".to_string()).await;
            });
        });

        // ----- Signal connections (user actions) -----
        // ANC OFF
        {
            let client = Arc::clone(&client);
            let level_box_clone = level_box.clone();
            let adaptive_sw = adaptive_switch.clone();
            let adaptive_row = adaptive_row.clone();
            let setting_ref = setting_from_status.clone();
            off_btn.connect_toggled(move |btn| {
                if !btn.is_active() {
                    btn.remove_css_class("suggested-action");
                    return;
                }
                if setting_ref.load(Ordering::Relaxed) {
                    btn.add_css_class("suggested-action");
                    return;
                }
                btn.add_css_class("suggested-action");
                level_box_clone.set_visible(false);
                adaptive_sw.set_sensitive(false);
                adaptive_row.set_sensitive(false);
                let client = Arc::clone(&client);
                glib::spawn_future_local(async move {
                    let mut c = client.lock().await;
                    if let Err(e) = c.set_anc_mode("off").await {
                        eprintln!("anc off: {}", e);
                    }
                });
            });
        }
        // ANC ON
        {
            let client = Arc::clone(&client);
            let level_box_clone = level_box.clone();
            let adaptive_sw = adaptive_switch.clone();
            let adaptive_row = adaptive_row.clone();
            let setting_ref = setting_from_status.clone();
            anc_btn.connect_toggled(move |btn| {
                if !btn.is_active() {
                    btn.remove_css_class("suggested-action");
                    return;
                }
                if setting_ref.load(Ordering::Relaxed) {
                    btn.add_css_class("suggested-action");
                    return;
                }
                btn.add_css_class("suggested-action");
                level_box_clone.set_visible(true);
                adaptive_sw.set_sensitive(true);
                adaptive_row.set_sensitive(true);
                let client = Arc::clone(&client);
                glib::spawn_future_local(async move {
                    let mut c = client.lock().await;
                    if let Err(e) = c.set_anc_mode("on").await {
                        eprintln!("anc on: {}", e);
                    }
                });
            });
        }
        // Transparency
        {
            let client = Arc::clone(&client);
            let level_box_clone = level_box.clone();
            let adaptive_sw = adaptive_switch.clone();
            let adaptive_row = adaptive_row.clone();
            let setting_ref = setting_from_status.clone();
            trans_btn.connect_toggled(move |btn| {
                if !btn.is_active() {
                    btn.remove_css_class("suggested-action");
                    return;
                }
                if setting_ref.load(Ordering::Relaxed) {
                    btn.add_css_class("suggested-action");
                    return;
                }
                btn.add_css_class("suggested-action");
                level_box_clone.set_visible(true);
                adaptive_sw.set_sensitive(false);
                adaptive_row.set_sensitive(false);
                let client = Arc::clone(&client);
                glib::spawn_future_local(async move {
                    let mut c = client.lock().await;
                    if let Err(e) = c.set_anc_mode("transparency").await {
                        eprintln!("anc transparency: {}", e);
                    }
                });
            });
        }
        // Low level
        {
            let client = Arc::clone(&client);
            let anc_max_ref = anc_max.clone();
            let setting_ref = setting_from_status.clone();
            let med_btn_clone = med_btn.clone();
            let high_btn_clone = high_btn.clone();
            low_btn.connect_toggled(move |btn| {
                if !btn.is_active() {
                    btn.remove_css_class("suggested-action");
                    return;
                }
                if setting_ref.load(Ordering::Relaxed) {
                    btn.add_css_class("suggested-action");
                    return;
                }
                btn.add_css_class("suggested-action");
                med_btn_clone.remove_css_class("suggested-action");
                high_btn_clone.remove_css_class("suggested-action");
                let max = anc_max_ref.get().max(3);
                let level = (max / 3).max(1);
                let client = Arc::clone(&client);
                glib::spawn_future_local(async move {
                    tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;
                    let mut c = client.lock().await;
                    if let Err(e) = c.set_level(level).await {
                        eprintln!("level low: {}", e);
                    }
                });
            });
        }
        // Medium level
        {
            let client = Arc::clone(&client);
            let anc_max_ref = anc_max.clone();
            let setting_ref = setting_from_status.clone();
            let low_btn_clone = low_btn.clone();
            let high_btn_clone = high_btn.clone();
            med_btn.connect_toggled(move |btn| {
                if !btn.is_active() {
                    btn.remove_css_class("suggested-action");
                    return;
                }
                if setting_ref.load(Ordering::Relaxed) {
                    btn.add_css_class("suggested-action");
                    return;
                }
                btn.add_css_class("suggested-action");
                low_btn_clone.remove_css_class("suggested-action");
                high_btn_clone.remove_css_class("suggested-action");
                let max = anc_max_ref.get().max(3);
                let level = ((max / 3) * 2).max(2);
                let client = Arc::clone(&client);
                glib::spawn_future_local(async move {
                    tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;
                    let mut c = client.lock().await;
                    if let Err(e) = c.set_level(level).await {
                        eprintln!("level med: {}", e);
                    }
                });
            });
        }
        // High level
        {
            let client = Arc::clone(&client);
            let anc_max_ref = anc_max.clone();
            let setting_ref = setting_from_status.clone();
            let low_btn_clone = low_btn.clone();
            let med_btn_clone = med_btn.clone();
            high_btn.connect_toggled(move |btn| {
                if !btn.is_active() {
                    btn.remove_css_class("suggested-action");
                    return;
                }
                if setting_ref.load(Ordering::Relaxed) {
                    btn.add_css_class("suggested-action");
                    return;
                }
                btn.add_css_class("suggested-action");
                low_btn_clone.remove_css_class("suggested-action");
                med_btn_clone.remove_css_class("suggested-action");
                let max = anc_max_ref.get().max(3);
                let client = Arc::clone(&client);
                glib::spawn_future_local(async move {
                    tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;
                    let mut c = client.lock().await;
                    if let Err(e) = c.set_level(max).await {
                        eprintln!("level high: {}", e);
                    }
                });
            });
        }
        // Adaptive ANC
        {
            let client = Arc::clone(&client);
            let setting_ref = setting_from_status.clone();
            adaptive_switch.connect_state_set(move |_, state| {
                if setting_ref.load(Ordering::Relaxed) {
                    return glib::Propagation::Proceed;
                }
                let client = Arc::clone(&client);
                glib::spawn_future_local(async move {
                    let mut c = client.lock().await;
                    if let Err(e) = c.set_adaptive_anc(state).await {
                        eprintln!("adaptive_anc: {}", e);
                    }
                });
                glib::Propagation::Proceed
            });
        }
        // Dual device
        {
            let client = Arc::clone(&client);
            let setting_ref = setting_from_status.clone();
            dual_switch.connect_state_set(move |_, state| {
                if setting_ref.load(Ordering::Relaxed) {
                    return glib::Propagation::Proceed;
                }
                let client = Arc::clone(&client);
                glib::spawn_future_local(async move {
                    let mut c = client.lock().await;
                    if let Err(e) = c.set_dual_device(state).await {
                        eprintln!("dual_device: {}", e);
                    }
                });
                glib::Propagation::Proceed
            });
        }
        // Game mode
        {
            let client = Arc::clone(&client);
            let setting_ref = setting_from_status.clone();
            game_switch.connect_state_set(move |_, state| {
                if setting_ref.load(Ordering::Relaxed) {
                    return glib::Propagation::Proceed;
                }
                let client = Arc::clone(&client);
                glib::spawn_future_local(async move {
                    let mut c = client.lock().await;
                    if let Err(e) = c.set_work_mode(state).await {
                        eprintln!("game mode: {}", e);
                    }
                });
                glib::Propagation::Proceed
            });
        }
        // Find device Start
        {
            let client = Arc::clone(&client);
            find_start.connect_clicked(move |_| {
                let client = Arc::clone(&client);
                glib::spawn_future_local(async move {
                    let mut c = client.lock().await;
                    if let Err(e) = c.find_device(true).await {
                        eprintln!("find device start: {}", e);
                    }
                });
            });
        }
        // Find device Stop
        {
            let client = Arc::clone(&client);
            find_stop.connect_clicked(move |_| {
                let client = Arc::clone(&client);
                glib::spawn_future_local(async move {
                    let mut c = client.lock().await;
                    if let Err(e) = c.find_device(false).await {
                        eprintln!("find device stop: {}", e);
                    }
                });
            });
        }
        // Factory reset with AlertDialog
        {
            let client = Arc::clone(&client);
            reset_button.connect_clicked(move |_| {
                let client = Arc::clone(&client);
                let dialog = AlertDialog::builder()
                    .message("Factory reset will erase all settings and disconnect the earbuds. Continue?")
                    .buttons(vec!["Cancel", "Reset"])
                    .build();

                glib::spawn_future_local(async move {
                    match dialog.choose_future(None::<&gtk4::Window>).await {
                        Ok(index) if index == 1 => { // "Reset" is the second button (index 1)
                            let mut c = client.lock().await;
                            if let Err(e) = c.factory_reset().await {
                                eprintln!("factory reset: {}", e);
                            }
                        }
                        _ => {}
                    }
                });
            });
        }

        container
    }
}