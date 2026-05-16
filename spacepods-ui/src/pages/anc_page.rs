use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use tokio::sync::Mutex;
use libspacepods::client::SpacePodsClient;
use libadwaita::prelude::*;
use gtk4::{Box, Grid, Label, Orientation, Spinner, Switch, ToggleButton};
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

    let vbox = Box::new(Orientation::Vertical, 4);
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


        let batt_header = Box::new(Orientation::Horizontal, 0);
        let batt_title = Label::new(Some("Battery"));
        batt_title.add_css_class("heading");
        batt_title.set_hexpand(true);
        batt_title.set_halign(gtk4::Align::Start);

        let refresh_btn = gtk4::Button::new();
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


        let batt_grid = Grid::new();
        batt_grid.set_column_spacing(8);
        batt_grid.set_row_spacing(0);
        batt_grid.set_hexpand(true);

        let (left_card,  left_icon,  left_val)  = make_battery_card("Left");
        let (case_card,  case_icon,  case_val)  = make_battery_card("Case");
        let (right_card, right_icon, right_val) = make_battery_card("Right");

        batt_grid.attach(&left_card,  0, 0, 1, 1);
        batt_grid.attach(&case_card,  1, 0, 1, 1);
        batt_grid.attach(&right_card, 2, 0, 1, 1);


        let nc_header = Box::new(Orientation::Horizontal, 0);
        let title = Label::new(Some("Noise Control"));
        title.add_css_class("title-1");
        title.set_halign(gtk4::Align::Start);
        title.set_hexpand(true);
        nc_header.append(&title);


        let off_btn   = ToggleButton::with_label("OFF");
        let anc_btn   = ToggleButton::with_label("ANC");
        let trans_btn = ToggleButton::with_label("Transparency");

        anc_btn.set_group(Some(&off_btn));
        trans_btn.set_group(Some(&off_btn));

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

        let low_btn  = ToggleButton::with_label("Low");
        let med_btn  = ToggleButton::with_label("Med");
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

        inner.append(&batt_header);
        inner.append(&batt_grid);
        inner.append(&nc_header);
        inner.append(&buttons_box);
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
            let low_btn     = low_btn.clone();
            let med_btn     = med_btn.clone();
            let high_btn    = high_btn.clone();
            let anc_max     = anc_max.clone();
            let setting_ref = setting_from_status.clone();
            Rc::new(move |level: u8| {
                let max   = anc_max.get().max(3);
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


        let update_battery = {
            let left_icon  = left_icon.clone();
            let left_val   = left_val.clone();
            let case_icon  = case_icon.clone();
            let case_val   = case_val.clone();
            let right_icon = right_icon.clone();
            let right_val  = right_val.clone();
            Rc::new(move |bl: Option<u8>, bc: Option<u8>, br: Option<u8>| {
                left_val.set_text(&battery_label(bl));
                case_val.set_text(&battery_label(bc));
                right_val.set_text(&battery_label(br));
                left_icon.set_icon_name(Some(battery_icon(bl)));
                case_icon.set_icon_name(Some(battery_icon(bc)));
                right_icon.set_icon_name(Some(battery_icon(br)));
            })
        };


        {
            let saved = load_settings();

            setting_from_status.set(true);

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

            setting_from_status.set(false);
        }


        {
            let client      = Arc::clone(&client);
            let update_batt = update_battery.clone();
            let batt_spin   = batt_spinner.clone();

            glib::spawn_future_local(async move {
                if let Ok(s) = { let mut c = client.lock().await; c.get_status().await } {
                    update_batt(s.battery_left, s.battery_case, s.battery_right);
                }
                batt_spin.stop();
                batt_spin.set_visible(false);
            });
        }


        {
            let client      = Arc::clone(&client);
            let update_batt = update_battery.clone();
            let batt_spin   = batt_spinner.clone();
            refresh_btn.connect_clicked(move |btn| {
                btn.set_sensitive(false);
                batt_spin.set_visible(true);
                batt_spin.start();
                let client      = Arc::clone(&client);
                let update_batt = update_batt.clone();
                let btn2        = btn.clone();
                let spin2       = batt_spin.clone();
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


        {
            let client      = Arc::clone(&client);
            let level_box   = level_box.clone();
            let adaptive_sw = adaptive_switch.clone();
            let adaptive_r  = adaptive_row.clone();
            let setting_ref = setting_from_status.clone();
            off_btn.connect_toggled(move |btn| {
                if !btn.is_active() { btn.remove_css_class("suggested-action"); return; }
                if setting_ref.get() { btn.add_css_class("suggested-action"); return; }
                btn.add_css_class("suggested-action");
                level_box.set_visible(false);
                adaptive_sw.set_sensitive(false);
                adaptive_r.set_sensitive(false);
                let client = Arc::clone(&client);
                glib::spawn_future_local(async move {
                    let mut c = client.lock().await;
                    if let Err(e) = c.set_anc_mode("off").await { eprintln!("anc off: {}", e); }
                });
            });
        }
        {
            let client      = Arc::clone(&client);
            let level_box   = level_box.clone();
            let adaptive_sw = adaptive_switch.clone();
            let adaptive_r  = adaptive_row.clone();
            let setting_ref = setting_from_status.clone();
            anc_btn.connect_toggled(move |btn| {
                if !btn.is_active() { btn.remove_css_class("suggested-action"); return; }
                if setting_ref.get() { btn.add_css_class("suggested-action"); return; }
                btn.add_css_class("suggested-action");
                level_box.set_visible(true);
                adaptive_sw.set_sensitive(true);
                adaptive_r.set_sensitive(true);
                let client = Arc::clone(&client);
                glib::spawn_future_local(async move {
                    let mut c = client.lock().await;
                    if let Err(e) = c.set_anc_mode("on").await { eprintln!("anc on: {}", e); }
                });
            });
        }
        {
            let client      = Arc::clone(&client);
            let level_box   = level_box.clone();
            let adaptive_sw = adaptive_switch.clone();
            let adaptive_r  = adaptive_row.clone();
            let setting_ref = setting_from_status.clone();
            trans_btn.connect_toggled(move |btn| {
                if !btn.is_active() { btn.remove_css_class("suggested-action"); return; }
                if setting_ref.get() { btn.add_css_class("suggested-action"); return; }
                btn.add_css_class("suggested-action");
                level_box.set_visible(true);
                adaptive_sw.set_sensitive(false);
                adaptive_r.set_sensitive(false);
                let client = Arc::clone(&client);
                glib::spawn_future_local(async move {
                    let mut c = client.lock().await;
                    if let Err(e) = c.set_anc_mode("transparency").await { eprintln!("anc transparency: {}", e); }
                });
            });
        }


        {
            let client      = Arc::clone(&client);
            let anc_max     = anc_max.clone();
            let setting_ref = setting_from_status.clone();
            let med_c       = med_btn.clone();
            let high_c      = high_btn.clone();
            low_btn.connect_toggled(move |btn| {
                if !btn.is_active() { btn.remove_css_class("suggested-action"); return; }
                if setting_ref.get() { btn.add_css_class("suggested-action"); return; }
                btn.add_css_class("suggested-action");
                med_c.remove_css_class("suggested-action");
                high_c.remove_css_class("suggested-action");
                let max   = anc_max.get().max(3);
                let level = (max / 3).max(1);
                let client = Arc::clone(&client);
                glib::spawn_future_local(async move {
                    tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;
                    let mut c = client.lock().await;
                    if let Err(e) = c.set_level(level).await { eprintln!("level low: {}", e); }
                });
            });
        }
        {
            let client      = Arc::clone(&client);
            let anc_max     = anc_max.clone();
            let setting_ref = setting_from_status.clone();
            let low_c       = low_btn.clone();
            let high_c      = high_btn.clone();
            med_btn.connect_toggled(move |btn| {
                if !btn.is_active() { btn.remove_css_class("suggested-action"); return; }
                if setting_ref.get() { btn.add_css_class("suggested-action"); return; }
                btn.add_css_class("suggested-action");
                low_c.remove_css_class("suggested-action");
                high_c.remove_css_class("suggested-action");
                let max   = anc_max.get().max(3);
                let level = ((max / 3) * 2).max(2);
                let client = Arc::clone(&client);
                glib::spawn_future_local(async move {
                    tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;
                    let mut c = client.lock().await;
                    if let Err(e) = c.set_level(level).await { eprintln!("level med: {}", e); }
                });
            });
        }
        {
            let client      = Arc::clone(&client);
            let anc_max     = anc_max.clone();
            let setting_ref = setting_from_status.clone();
            let low_c       = low_btn.clone();
            let med_c       = med_btn.clone();
            high_btn.connect_toggled(move |btn| {
                if !btn.is_active() { btn.remove_css_class("suggested-action"); return; }
                if setting_ref.get() { btn.add_css_class("suggested-action"); return; }
                btn.add_css_class("suggested-action");
                low_c.remove_css_class("suggested-action");
                med_c.remove_css_class("suggested-action");
                let max    = anc_max.get().max(3);
                let client = Arc::clone(&client);
                glib::spawn_future_local(async move {
                    tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;
                    let mut c = client.lock().await;
                    if let Err(e) = c.set_level(max).await { eprintln!("level high: {}", e); }
                });
            });
        }


        {
            let client      = Arc::clone(&client);
            let setting_ref = setting_from_status.clone();
            adaptive_switch.connect_state_set(move |_, state| {
                if setting_ref.get() { return glib::Propagation::Proceed; }
                let client = Arc::clone(&client);
                glib::spawn_future_local(async move {
                    let mut c = client.lock().await;
                    if let Err(e) = c.set_adaptive_anc(state).await { eprintln!("adaptive_anc: {}", e); }
                });
                glib::Propagation::Proceed
            });
        }
        {
            let client      = Arc::clone(&client);
            let setting_ref = setting_from_status.clone();
            dual_switch.connect_state_set(move |_, state| {
                if setting_ref.get() { return glib::Propagation::Proceed; }
                let client = Arc::clone(&client);
                glib::spawn_future_local(async move {
                    let mut c = client.lock().await;
                    if let Err(e) = c.set_dual_device(state).await { eprintln!("dual_device: {}", e); }
                });
                glib::Propagation::Proceed
            });
        }

        container.append(&scroll);
        container
    }
}