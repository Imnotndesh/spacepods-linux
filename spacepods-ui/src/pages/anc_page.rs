use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use tokio::sync::Mutex;
use libspacepods::client::SpacePodsClient;
use libadwaita::prelude::*;
use glib::clone;
use gtk4::prelude::*;
use gtk4::{Box, Label, Orientation, Scale, Switch, ToggleButton};
use libadwaita::{ActionRow, PreferencesGroup};

pub struct AncPage;

impl AncPage {
    pub fn new(client: Arc<Mutex<SpacePodsClient>>) -> Box {
        let container = Box::new(Orientation::Vertical, 12);
        container.set_halign(gtk4::Align::Center);
        container.set_valign(gtk4::Align::Center);
        container.set_vexpand(false);
        container.set_margin_top(24);
        container.set_margin_bottom(24);
        container.set_margin_start(16);
        container.set_margin_end(16);

        let title = Label::new(Some("ANC Control"));
        title.add_css_class("title-1");

        let off_btn = ToggleButton::with_label("OFF");
        let anc_btn = ToggleButton::with_label("ANC");
        let trans_btn = ToggleButton::with_label("Transparency");
        let mode_status = Label::new(Some("Loading…"));
        mode_status.add_css_class("dim-label");

        anc_btn.set_group(Some(&off_btn));
        trans_btn.set_group(Some(&off_btn));

        // Disable until status loaded
        off_btn.set_sensitive(false);
        anc_btn.set_sensitive(false);
        trans_btn.set_sensitive(false);

        let buttons_box = Box::new(Orientation::Horizontal, 12);
        buttons_box.set_halign(gtk4::Align::Center);
        buttons_box.append(&off_btn);
        buttons_box.append(&anc_btn);
        buttons_box.append(&trans_btn);

        let slider_box = Box::new(Orientation::Vertical, 4);
        slider_box.set_halign(gtk4::Align::Fill);
        slider_box.set_hexpand(true);
        slider_box.set_visible(false);

        let slider_label = Label::new(Some("Intensity"));
        slider_label.set_halign(gtk4::Align::Start);
        slider_label.add_css_class("caption");

        let slider = Scale::with_range(Orientation::Horizontal, 1.0, 15.0, 1.0);
        slider.set_draw_value(true);
        slider.set_hexpand(true);
        slider.set_value(3.0);

        slider_box.append(&slider_label);
        slider_box.append(&slider);

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

        container.append(&title);
        container.append(&buttons_box);
        container.append(&mode_status);
        container.append(&slider_box);
        container.append(&features_group);

        let setting_from_status = Rc::new(Cell::new(false));
        let current_mode = Rc::new(Cell::new(0u8));

        {
            let client = Arc::clone(&client);
            let off_btn = off_btn.clone();
            let anc_btn = anc_btn.clone();
            let trans_btn = trans_btn.clone();
            let slider = slider.clone();
            let slider_box = slider_box.clone();
            let adaptive_switch = adaptive_switch.clone();
            let adaptive_row = adaptive_row.clone();
            let dual_switch = dual_switch.clone();
            let mode_status = mode_status.clone();
            let setting_ref = Rc::clone(&setting_from_status);
            let current_mode = Rc::clone(&current_mode);

            glib::spawn_future_local(async move {
                let status = {
                    let mut c = client.lock().await;
                    c.get_status().await
                };

                match status {
                    Ok(s) => {
                        setting_ref.set(true);

                        off_btn.set_sensitive(true);
                        anc_btn.set_sensitive(true);
                        trans_btn.set_sensitive(true);

                        let mode = s.anc_mode.unwrap_or(0);
                        current_mode.set(mode);
                        match mode {
                            0 => {
                                off_btn.set_active(true);
                                mode_status.set_text("Current mode: OFF");
                                slider_box.set_visible(false);
                                adaptive_switch.set_sensitive(false);
                                adaptive_row.set_sensitive(false);
                            }
                            1 => {
                                anc_btn.set_active(true);
                                mode_status.set_text("Current mode: ANC");
                                slider_box.set_visible(true);
                                adaptive_switch.set_sensitive(true);
                                adaptive_row.set_sensitive(true);
                            }
                            2 => {
                                trans_btn.set_active(true);
                                mode_status.set_text("Current mode: Transparency");
                                slider_box.set_visible(true);
                                adaptive_switch.set_sensitive(false);
                                adaptive_row.set_sensitive(false);
                            }
                            _ => {}
                        }

                        let max = s.anc_max.max(1) as f64;
                        slider.set_range(1.0, max);
                        slider.set_value(s.anc_level as f64);

                        if let Some(v) = s.adaptive_anc {
                            adaptive_switch.set_active(v);
                        }
                        if let Some(v) = s.dual_device {
                            dual_switch.set_active(v);
                        }

                        setting_ref.set(false);
                    }
                    Err(e) => {
                        mode_status.set_text(&format!("Failed to load status: {}", e));
                        off_btn.set_sensitive(true);
                        anc_btn.set_sensitive(true);
                        trans_btn.set_sensitive(true);
                    }
                }
            });
        }

        {
            let client = Arc::clone(&client);
            let slider_box = slider_box.clone();
            let adaptive_switch = adaptive_switch.clone();
            let adaptive_row = adaptive_row.clone();
            let mode_status = mode_status.clone();
            let setting_ref = Rc::clone(&setting_from_status);
            let current_mode = Rc::clone(&current_mode);

            off_btn.connect_toggled(move |btn| {
                if !btn.is_active() { btn.remove_css_class("suggested-action"); return; }
                if setting_ref.get() { btn.add_css_class("suggested-action"); return; }
                current_mode.set(0);
                btn.add_css_class("suggested-action");
                mode_status.set_text("Current mode: OFF");
                slider_box.set_visible(false);
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
            let slider_box = slider_box.clone();
            let adaptive_switch = adaptive_switch.clone();
            let adaptive_row = adaptive_row.clone();
            let mode_status = mode_status.clone();
            let setting_ref = Rc::clone(&setting_from_status);
            let current_mode = Rc::clone(&current_mode);

            anc_btn.connect_toggled(move |btn| {
                if !btn.is_active() { btn.remove_css_class("suggested-action"); return; }
                if setting_ref.get() { btn.add_css_class("suggested-action"); return; }
                current_mode.set(1);
                btn.add_css_class("suggested-action");
                mode_status.set_text("Current mode: ANC");
                slider_box.set_visible(true);
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
            let slider_box = slider_box.clone();
            let adaptive_switch = adaptive_switch.clone();
            let adaptive_row = adaptive_row.clone();
            let mode_status = mode_status.clone();
            let setting_ref = Rc::clone(&setting_from_status);
            let current_mode = Rc::clone(&current_mode);

            trans_btn.connect_toggled(move |btn| {
                if !btn.is_active() { btn.remove_css_class("suggested-action"); return; }
                if setting_ref.get() { btn.add_css_class("suggested-action"); return; }
                current_mode.set(2);
                btn.add_css_class("suggested-action");
                mode_status.set_text("Current mode: Transparency");
                slider_box.set_visible(true);
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
            let setting_ref = Rc::clone(&setting_from_status);
            slider.connect_change_value(move |_, _, value| {
                if setting_ref.get() { return glib::Propagation::Proceed; }
                let level = value as u8;
                let client = Arc::clone(&client);
                glib::spawn_future_local(async move {
                    let mut c = client.lock().await;
                    if let Err(e) = c.set_level(level).await {
                        eprintln!("set_level {}: {}", level, e);
                    }
                });
                glib::Propagation::Proceed
            });
        }

        {
            let client = Arc::clone(&client);
            let setting_ref = Rc::clone(&setting_from_status);
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
            let setting_ref = Rc::clone(&setting_from_status);
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

        container
    }
}