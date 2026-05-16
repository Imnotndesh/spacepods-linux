use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use tokio::sync::Mutex;
use libspacepods::client::SpacePodsClient;
use libadwaita::prelude::*;
use gtk4::{Box, Label, Orientation, Switch, ToggleButton};
use libadwaita::{ActionRow, PreferencesGroup};

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

impl AncPage {
    pub fn new(client: Arc<Mutex<SpacePodsClient>>) -> Box {
        let container = Box::new(Orientation::Vertical, 0);
        container.set_hexpand(true);
        container.set_vexpand(true);

        let clamp = libadwaita::Clamp::new();
        clamp.set_maximum_size(600);
        clamp.set_tightening_threshold(480);
        clamp.set_hexpand(true);
        clamp.set_vexpand(true);

        let inner = Box::new(Orientation::Vertical, 16);
        inner.set_margin_top(24);
        inner.set_margin_bottom(24);
        inner.set_margin_start(16);
        inner.set_margin_end(16);

        let battery_group = PreferencesGroup::new();
        battery_group.set_title("Battery");

        let left_row = ActionRow::new();
        left_row.set_title("Left");
        let left_icon = gtk4::Image::from_icon_name("battery-missing-symbolic");
        left_icon.set_icon_size(gtk4::IconSize::Normal);
        let left_val = Label::new(Some("—"));
        left_val.add_css_class("dim-label");
        left_val.set_valign(gtk4::Align::Center);
        left_row.add_prefix(&left_icon);
        left_row.add_suffix(&left_val);

        let right_row = ActionRow::new();
        right_row.set_title("Right");
        let right_icon = gtk4::Image::from_icon_name("battery-missing-symbolic");
        right_icon.set_icon_size(gtk4::IconSize::Normal);
        let right_val = Label::new(Some("—"));
        right_val.add_css_class("dim-label");
        right_val.set_valign(gtk4::Align::Center);
        right_row.add_prefix(&right_icon);
        right_row.add_suffix(&right_val);

        let case_row = ActionRow::new();
        case_row.set_title("Case");
        let case_icon = gtk4::Image::from_icon_name("battery-missing-symbolic");
        case_icon.set_icon_size(gtk4::IconSize::Normal);
        let case_val = Label::new(Some("—"));
        case_val.add_css_class("dim-label");
        case_val.set_valign(gtk4::Align::Center);
        case_row.add_prefix(&case_icon);
        case_row.add_suffix(&case_val);

        battery_group.add(&left_row);
        battery_group.add(&right_row);
        battery_group.add(&case_row);

        let title = Label::new(Some("Noise Control"));
        title.add_css_class("title-1");
        title.set_halign(gtk4::Align::Start);

        let off_btn = ToggleButton::with_label("OFF");
        let anc_btn = ToggleButton::with_label("ANC");
        let trans_btn = ToggleButton::with_label("Transparency");
        let mode_status = Label::new(Some("Loading…"));
        mode_status.add_css_class("dim-label");
        mode_status.set_halign(gtk4::Align::Center);

        anc_btn.set_group(Some(&off_btn));
        trans_btn.set_group(Some(&off_btn));

        off_btn.set_sensitive(false);
        anc_btn.set_sensitive(false);
        trans_btn.set_sensitive(false);

        off_btn.set_hexpand(true);
        anc_btn.set_hexpand(true);
        trans_btn.set_hexpand(true);

        let buttons_box = Box::new(Orientation::Horizontal, 8);
        buttons_box.set_hexpand(true);
        buttons_box.append(&off_btn);
        buttons_box.append(&anc_btn);
        buttons_box.append(&trans_btn);

        let level_box = Box::new(Orientation::Vertical, 8);
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

        let level_buttons_box = Box::new(Orientation::Horizontal, 8);
        level_buttons_box.set_hexpand(true);
        level_buttons_box.append(&low_btn);
        level_buttons_box.append(&med_btn);
        level_buttons_box.append(&high_btn);

        level_box.append(&level_label);
        level_box.append(&level_buttons_box);

        let adaptive_row = ActionRow::new();
        adaptive_row.set_title("Adaptive ANC");
        adaptive_row.set_subtitle("Dynamically adjust based on environment");
        let adaptive_switch = Switch::new();
        adaptive_switch.set_valign(gtk4::Align::Center);
        adaptive_switch.set_vexpand(false);
        adaptive_row.add_suffix(&adaptive_switch);
        adaptive_row.set_activatable_widget(Some(&adaptive_switch));
        adaptive_switch.set_sensitive(false);
        adaptive_row.set_sensitive(false);

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

        inner.append(&battery_group);
        inner.append(&title);
        inner.append(&buttons_box);
        inner.append(&mode_status);
        inner.append(&level_box);
        inner.append(&features_group);

        clamp.set_child(Some(&inner));

        let scroll = gtk4::ScrolledWindow::new();
        scroll.set_hscrollbar_policy(gtk4::PolicyType::Never);
        scroll.set_vexpand(true);
        scroll.set_child(Some(&clamp));

        let setting_from_status = Rc::new(Cell::new(false));
        let anc_max = Rc::new(Cell::new(15u8));

        let set_level_buttons = {
            let low_btn = low_btn.clone();
            let med_btn = med_btn.clone();
            let high_btn = high_btn.clone();
            let anc_max = anc_max.clone();
            let setting_ref = setting_from_status.clone();
            Rc::new(move |level: u8| {
                let max = anc_max.get().max(3);
                let third = (max / 3).max(1);
                setting_ref.set(true);
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
                setting_ref.set(false);
            })
        };

        {
            let client = Arc::clone(&client);
            let off_btn = off_btn.clone();
            let anc_btn = anc_btn.clone();
            let trans_btn = trans_btn.clone();
            let level_box = level_box.clone();
            let adaptive_switch = adaptive_switch.clone();
            let adaptive_row = adaptive_row.clone();
            let dual_switch = dual_switch.clone();
            let mode_status = mode_status.clone();
            let setting_ref = setting_from_status.clone();
            let anc_max = anc_max.clone();
            let set_level_buttons = set_level_buttons.clone();
            let left_val = left_val.clone();
            let right_val = right_val.clone();
            let case_val = case_val.clone();
            let left_icon = left_icon.clone();
            let right_icon = right_icon.clone();
            let case_icon = case_icon.clone();

            glib::spawn_future_local(async move {
                let mut attempts = 0u8;
                let s = loop {
                    attempts += 1;
                    let res = {
                        let mut c = client.lock().await;
                        c.get_status().await
                    };
                    match res {
                        Ok(s) if s.connected && s.anc_mode.is_some() => break s,
                        Ok(s) if attempts >= 10 =>{
                            break s;
                        }
                        Ok(_) => {
                            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                        }
                        Err(e) => {
                            mode_status.set_text(&format!("Error: {}", e));
                            off_btn.set_sensitive(true);
                            anc_btn.set_sensitive(true);
                            trans_btn.set_sensitive(true);
                            return;
                        }
                    }
                };

                setting_ref.set(true);
                off_btn.set_sensitive(true);
                anc_btn.set_sensitive(true);
                trans_btn.set_sensitive(true);

                left_val.set_text(&battery_label(s.battery_left));
                right_val.set_text(&battery_label(s.battery_right));
                case_val.set_text(&battery_label(s.battery_case));
                left_icon.set_icon_name(Some(battery_icon(s.battery_left)));
                right_icon.set_icon_name(Some(battery_icon(s.battery_right)));
                case_icon.set_icon_name(Some(battery_icon(s.battery_case)));

                anc_max.set(s.anc_max.max(3));

                match s.anc_mode.unwrap_or(0) {
                    0 => {
                        off_btn.set_active(true);
                        off_btn.add_css_class("suggested-action");
                        mode_status.set_text("OFF");
                        level_box.set_visible(false);
                        adaptive_switch.set_sensitive(false);
                        adaptive_row.set_sensitive(false);
                    }
                    1 => {
                        anc_btn.set_active(true);
                        anc_btn.add_css_class("suggested-action");
                        mode_status.set_text("ANC");
                        level_box.set_visible(true);
                        adaptive_switch.set_sensitive(true);
                        adaptive_row.set_sensitive(true);
                    }
                    2 => {
                        trans_btn.set_active(true);
                        trans_btn.add_css_class("suggested-action");
                        mode_status.set_text("Transparency");
                        level_box.set_visible(true);
                        adaptive_switch.set_sensitive(false);
                        adaptive_row.set_sensitive(false);
                    }
                    _ => {}
                }

                set_level_buttons(s.anc_level);

                if let Some(v) = s.adaptive_anc {
                    adaptive_switch.set_active(v);
                }
                if let Some(v) = s.dual_device {
                    dual_switch.set_active(v);
                }
                setting_ref.set(false);

                // If battery still missing after first good status, poll once more after delay
                if s.battery_left.is_none() {
                    tokio::time::sleep(tokio::time::Duration::from_millis(2000)).await;
                    if let Ok(s2) = { let mut c = client.lock().await; c.get_status().await } {
                        left_val.set_text(&battery_label(s2.battery_left));
                        right_val.set_text(&battery_label(s2.battery_right));
                        case_val.set_text(&battery_label(s2.battery_case));
                        left_icon.set_icon_name(Some(battery_icon(s2.battery_left)));
                        right_icon.set_icon_name(Some(battery_icon(s2.battery_right)));
                        case_icon.set_icon_name(Some(battery_icon(s2.battery_case)));
                    }
                }
            });
        }

        {
            let client = Arc::clone(&client);
            let level_box = level_box.clone();
            let adaptive_switch = adaptive_switch.clone();
            let adaptive_row = adaptive_row.clone();
            let mode_status = mode_status.clone();
            let setting_ref = setting_from_status.clone();
            off_btn.connect_toggled(move |btn| {
                if !btn.is_active() { btn.remove_css_class("suggested-action"); return; }
                if setting_ref.get() { btn.add_css_class("suggested-action"); return; }
                btn.add_css_class("suggested-action");
                mode_status.set_text("OFF");
                level_box.set_visible(false);
                adaptive_switch.set_sensitive(false);
                adaptive_row.set_sensitive(false);
                let client = Arc::clone(&client);
                glib::spawn_future_local(async move {
                    let mut c = client.lock().await;
                    if let Err(e) = c.set_anc_mode("off").await {
                        eprintln!("set_anc_mode off: {}", e);
                    }
                });
            });
        }
        {
            let client = Arc::clone(&client);
            let level_box = level_box.clone();
            let adaptive_switch = adaptive_switch.clone();
            let adaptive_row = adaptive_row.clone();
            let mode_status = mode_status.clone();
            let setting_ref = setting_from_status.clone();
            anc_btn.connect_toggled(move |btn| {
                if !btn.is_active() { btn.remove_css_class("suggested-action"); return; }
                if setting_ref.get() { btn.add_css_class("suggested-action"); return; }
                btn.add_css_class("suggested-action");
                mode_status.set_text("ANC");
                level_box.set_visible(true);
                adaptive_switch.set_sensitive(true);
                adaptive_row.set_sensitive(true);
                let client = Arc::clone(&client);
                glib::spawn_future_local(async move {
                    let mut c = client.lock().await;
                    if let Err(e) = c.set_anc_mode("on").await {
                        eprintln!("set_anc_mode on: {}", e);
                    }
                });
            });
        }
        {
            let client = Arc::clone(&client);
            let level_box = level_box.clone();
            let adaptive_switch = adaptive_switch.clone();
            let adaptive_row = adaptive_row.clone();
            let mode_status = mode_status.clone();
            let setting_ref = setting_from_status.clone();
            trans_btn.connect_toggled(move |btn| {
                if !btn.is_active() { btn.remove_css_class("suggested-action"); return; }
                if setting_ref.get() { btn.add_css_class("suggested-action"); return; }
                btn.add_css_class("suggested-action");
                mode_status.set_text("Transparency");
                level_box.set_visible(true);
                adaptive_switch.set_sensitive(false);
                adaptive_row.set_sensitive(false);
                let client = Arc::clone(&client);
                glib::spawn_future_local(async move {
                    let mut c = client.lock().await;
                    if let Err(e) = c.set_anc_mode("transparency").await {
                        eprintln!("set_anc_mode transparency: {}", e);
                    }
                });
            });
        }

        {
            let client = Arc::clone(&client);
            let anc_max = anc_max.clone();
            let setting_ref = setting_from_status.clone();
            let med_btn_c = med_btn.clone();
            let high_btn_c = high_btn.clone();
            low_btn.connect_toggled(move |btn| {
                if !btn.is_active() { btn.remove_css_class("suggested-action"); return; }
                if setting_ref.get() { btn.add_css_class("suggested-action"); return; }
                btn.add_css_class("suggested-action");
                med_btn_c.remove_css_class("suggested-action");
                high_btn_c.remove_css_class("suggested-action");
                let max = anc_max.get().max(3);
                let level = (max / 3).max(1);
                let client = Arc::clone(&client);
                glib::spawn_future_local(async move {
                    tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;
                    let mut c = client.lock().await;
                    if let Err(e) = c.set_level(level).await {
                        eprintln!("set_level low {}: {}", level, e);
                    }
                });
            });
        }
        {
            let client = Arc::clone(&client);
            let anc_max = anc_max.clone();
            let setting_ref = setting_from_status.clone();
            let low_btn_c = low_btn.clone();
            let high_btn_c = high_btn.clone();
            med_btn.connect_toggled(move |btn| {
                if !btn.is_active() { btn.remove_css_class("suggested-action"); return; }
                if setting_ref.get() { btn.add_css_class("suggested-action"); return; }
                btn.add_css_class("suggested-action");
                low_btn_c.remove_css_class("suggested-action");
                high_btn_c.remove_css_class("suggested-action");
                let max = anc_max.get().max(3);
                let level = ((max / 3) * 2).max(2);
                let client = Arc::clone(&client);
                glib::spawn_future_local(async move {
                    tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;
                    let mut c = client.lock().await;
                    if let Err(e) = c.set_level(level).await {
                        eprintln!("set_level med {}: {}", level, e);
                    }
                });
            });
        }
        {
            let client = Arc::clone(&client);
            let anc_max = anc_max.clone();
            let setting_ref = setting_from_status.clone();
            let low_btn_c = low_btn.clone();
            let med_btn_c = med_btn.clone();
            high_btn.connect_toggled(move |btn| {
                if !btn.is_active() { btn.remove_css_class("suggested-action"); return; }
                if setting_ref.get() { btn.add_css_class("suggested-action"); return; }
                btn.add_css_class("suggested-action");
                low_btn_c.remove_css_class("suggested-action");
                med_btn_c.remove_css_class("suggested-action");
                let max = anc_max.get().max(3);
                let client = Arc::clone(&client);
                glib::spawn_future_local(async move {
                    tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;
                    let mut c = client.lock().await;
                    if let Err(e) = c.set_level(max).await {
                        eprintln!("set_level high {}: {}", max, e);
                    }
                });
            });
        }

        {
            let client = Arc::clone(&client);
            let setting_ref = setting_from_status.clone();
            adaptive_switch.connect_state_set(move |_, state| {
                if setting_ref.get() { return glib::Propagation::Proceed; }
                let client = Arc::clone(&client);
                glib::spawn_future_local(async move {
                    let mut c = client.lock().await;
                    if let Err(e) = c.set_adaptive_anc(state).await {
                        eprintln!("set_adaptive_anc: {}", e);
                    }
                });
                glib::Propagation::Proceed
            });
        }

        {
            let client = Arc::clone(&client);
            let setting_ref = setting_from_status.clone();
            dual_switch.connect_state_set(move |_, state| {
                if setting_ref.get() { return glib::Propagation::Proceed; }
                let client = Arc::clone(&client);
                glib::spawn_future_local(async move {
                    let mut c = client.lock().await;
                    if let Err(e) = c.set_dual_device(state).await {
                        eprintln!("set_dual_device: {}", e);
                    }
                });
                glib::Propagation::Proceed
            });
        }

        container.append(&scroll);
        container
    }
}