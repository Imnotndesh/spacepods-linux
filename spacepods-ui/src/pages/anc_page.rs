use std::cell::Cell;
use std::rc::Rc;
use glib::clone;
use gtk4::prelude::*;
use gtk4::{Box, Label, Orientation, Switch, ToggleButton, Spinner};
use libadwaita::{ActionRow, PreferencesGroup, Clamp, StatusPage};
use libadwaita::prelude::*;

use crate::context::AppContext;
use crate::log::Log;

pub struct AncPage;

impl AncPage {
    pub fn new(ctx: Rc<AppContext>) -> gtk4::Widget {
        let clamp = Clamp::new();
        clamp.set_maximum_size(560);
        clamp.set_tightening_threshold(420);

        let container = Box::new(Orientation::Vertical, 16);
        container.set_margin_top(24);
        container.set_margin_bottom(32);
        container.set_margin_start(16);
        container.set_margin_end(16);

        // ── Header ──
        let header_row = Box::new(Orientation::Horizontal, 0);
        let title = Label::new(Some("ANC Control"));
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

        // ── Mode row ──
        let off_btn = ToggleButton::with_label("Off");
        let anc_btn = ToggleButton::with_label("ANC");
        let trans_btn = ToggleButton::with_label("Transparency");
        anc_btn.set_group(Some(&off_btn));
        trans_btn.set_group(Some(&off_btn));
        for b in [&off_btn, &anc_btn, &trans_btn] {
            b.add_css_class("pill");
            b.set_sensitive(false);
        }

        let mode_row = Box::new(Orientation::Horizontal, 6);
        mode_row.add_css_class("linked");
        mode_row.set_halign(gtk4::Align::Center);
        mode_row.append(&off_btn);
        mode_row.append(&anc_btn);
        mode_row.append(&trans_btn);

        let mode_spinner = Spinner::new();
        mode_spinner.set_visible(false);
        mode_spinner.set_halign(gtk4::Align::Center);
        
        let anc_group = PreferencesGroup::new();
        
        let low_btn = ToggleButton::with_label("Low");
        let med_btn = ToggleButton::with_label("Medium");
        let high_btn = ToggleButton::with_label("High");
        med_btn.set_group(Some(&low_btn));
        high_btn.set_group(Some(&low_btn));
        for b in [&low_btn, &med_btn, &high_btn] {
            b.add_css_class("pill");
            b.set_sensitive(false);
        }
        med_btn.set_active(true);

        let intensity_btns = Box::new(Orientation::Horizontal, 6);
        intensity_btns.add_css_class("linked");
        intensity_btns.set_halign(gtk4::Align::Center);
        intensity_btns.set_margin_top(4);
        intensity_btns.set_margin_bottom(8);
        intensity_btns.append(&low_btn);
        intensity_btns.append(&med_btn);
        intensity_btns.append(&high_btn);

        let features_group = PreferencesGroup::new();
        features_group.set_title("Additional Features");

        let adaptive_row = ActionRow::new();
        adaptive_row.set_title("Adaptive ANC");
        adaptive_row.set_subtitle("Dynamically adjust based on environment");
        let adaptive_switch = Switch::new();
        adaptive_switch.set_valign(gtk4::Align::Center);
        adaptive_row.add_suffix(&adaptive_switch);
        adaptive_row.set_activatable_widget(Some(&adaptive_switch));
        adaptive_switch.set_sensitive(false);
        adaptive_row.set_sensitive(false);
        if ctx.has_feature(libspacepods::device_profile::DetailFeature::Noise) {
            features_group.add(&adaptive_row);
        }

        let dual_row = ActionRow::new();
        dual_row.set_title("Dual Device (Multi-point)");
        dual_row.set_subtitle("Connect to two devices simultaneously");
        let dual_switch = Switch::new();
        dual_switch.set_valign(gtk4::Align::Center);
        dual_row.add_suffix(&dual_switch);
        dual_row.set_activatable_widget(Some(&dual_switch));
        dual_switch.set_sensitive(false);
        dual_row.set_sensitive(false);
        if ctx.has_feature(libspacepods::device_profile::DetailFeature::DualDeviceSwitch) {
            features_group.add(&dual_row);
        }

        // Chat Mode
        let chat_switch = Switch::new();
        chat_switch.set_valign(gtk4::Align::Center);
        if ctx.has_feature(libspacepods::device_profile::DetailFeature::ChatMode) {
            let chat_row = ActionRow::new();
            chat_row.set_title("Chat Mode");
            chat_row.set_subtitle("Optimize audio for voice conversations");
            chat_row.add_suffix(&chat_switch);
            chat_row.set_activatable_widget(Some(&chat_switch));
            chat_switch.set_sensitive(false);
            chat_row.set_sensitive(false);
            features_group.add(&chat_row);
        }

        // Long Endurance
        let endurance_switch = Switch::new();
        endurance_switch.set_valign(gtk4::Align::Center);
        if ctx.has_feature(libspacepods::device_profile::DetailFeature::LongEndurance) {
            let endurance_row = ActionRow::new();
            endurance_row.set_title("Long Endurance");
            endurance_row.set_subtitle("Extend battery life by reducing performance");
            endurance_row.add_suffix(&endurance_switch);
            endurance_row.set_activatable_widget(Some(&endurance_switch));
            endurance_switch.set_sensitive(false);
            endurance_row.set_sensitive(false);
            features_group.add(&endurance_row);
        }

        let find_ear_row = ActionRow::new();
        find_ear_row.set_title("Find My Earbuds");
        find_ear_row.set_subtitle("Ring your earbuds to locate them");
        let find_ear_btn = gtk4::Button::with_label("Ring");
        find_ear_btn.add_css_class("destructive-action");
        find_ear_btn.set_valign(gtk4::Align::Center);
        find_ear_row.add_suffix(&find_ear_btn);
        if ctx.has_feature(libspacepods::device_profile::DetailFeature::FindDevice) {
            features_group.add(&find_ear_row);
        }

        let offline_status = StatusPage::new();
        offline_status.set_icon_name(Some("network-offline-symbolic"));
        offline_status.set_title("Daemon Unreachable");
        offline_status.set_description(Some("Couldn't connect to the SpacePods service."));
        offline_status.set_visible(false);
        offline_status.set_vexpand(true);

        container.append(&header_row);
        container.append(&refresh_btn);
        container.append(&mode_row);
        container.append(&mode_spinner);
        container.append(&anc_group);
        container.append(&intensity_btns);
        container.append(&features_group);
        container.append(&offline_status);

        clamp.set_child(Some(&container));

        let applying = Rc::new(Cell::new(false));
        let current_mode = Rc::new(Cell::new(0u8));

        glib::spawn_future_local(clone!(
            #[strong] off_btn, #[strong] anc_btn, #[strong] trans_btn,
            #[strong] low_btn, #[strong] med_btn, #[strong] high_btn,
            #[strong] adaptive_switch, #[strong] adaptive_row,
            #[strong] dual_switch, #[strong] dual_row,
            #[strong] mode_spinner, #[strong] mode_row,
            #[strong] offline_status, #[strong] features_group, #[strong] anc_group,
            #[strong] applying, #[strong] current_mode, #[strong] ctx,
            async move {
                use libspacepods::client::SpacePodsClient;
                match SpacePodsClient::connect(None).await {
                    Ok(mut client) => match client.get_status().await {
                        Ok(s) => {
                            Log::info("ANC", &format!("Status received: mode={:?} level={} max={} adaptive={:?} dual={:?}",
                                s.anc.mode, s.anc.level, s.anc.max_level,
                                s.features.adaptive_anc, s.features.dual_device));

                            for b in [&off_btn, &anc_btn, &trans_btn] { b.set_sensitive(true); }

                            let mode = s.anc.mode as u8;
                            current_mode.set(mode);
                            ctx.anc_mode.set(mode);
                            crate::tray::ANC_MODE_ATOMIC.store(mode, std::sync::atomic::Ordering::Relaxed);
                            applying.set(true);
                            match mode {
                                0 => off_btn.set_active(true),
                                1 => anc_btn.set_active(true),
                                2 => trans_btn.set_active(true),
                                _ => {}
                            }
                            applying.set(false);
                            Self::apply_mode_ui(mode, &low_btn, &med_btn, &high_btn, &adaptive_switch, &adaptive_row);

                            match s.anc.level as u8 {
                                1..=2 => low_btn.set_active(true),
                                3 => med_btn.set_active(true),
                                4..=5 => high_btn.set_active(true),
                                _ => {}
                            }

                            if let Some(v) = s.features.adaptive_anc {
                                adaptive_switch.set_active(v);
                            }
                            if let Some(v) = s.features.dual_device {
                                dual_switch.set_active(v);
                            }
                            adaptive_switch.set_sensitive(mode == 1);
                            adaptive_row.set_sensitive(mode == 1);
                            dual_switch.set_sensitive(true);
                            dual_row.set_sensitive(true);
                        }
                        Err(e) => {
                            mode_row.set_visible(false);
                            anc_group.set_visible(false);
                            features_group.set_visible(false);
                            offline_status.set_visible(true);
                            ctx.daemon_unreachable(e);
                        }
                    },
                    Err(e) => {
                        mode_row.set_visible(false);
                        anc_group.set_visible(false);
                        features_group.set_visible(false);
                        offline_status.set_visible(true);
                        ctx.daemon_unreachable(e);
                    }
                }
            }
        ));

        // ── Refresh button ──
        {
            let btns = vec![
                off_btn.clone(), anc_btn.clone(), trans_btn.clone(),
                low_btn.clone(), med_btn.clone(), high_btn.clone(),
            ];
            let rows = vec![adaptive_row.clone(), dual_row.clone()];
            let asw = adaptive_switch.clone();
            let dsw = dual_switch.clone();
            let am = applying.clone();
            let cm = current_mode.clone();
            let ctx = ctx.clone();
            refresh_btn.connect_clicked(move |_| {
                let btns = btns.clone();
                let asw = asw.clone();
                let dsw = dsw.clone();
                let am = am.clone();
                let cm = cm.clone();
                let ctx = ctx.clone();
                glib::spawn_future_local(async move {
                    use libspacepods::client::SpacePodsClient;
                    match SpacePodsClient::connect(None).await {
                        Ok(mut client) => match client.get_status().await {
                            Ok(s) => {
                                let mode = s.anc.mode as u8;
                                cm.set(mode);
                                am.set(true);
                                // Only set intensity buttons — mode buttons handled by connect_mode
                                btns[3].set_sensitive(mode != 0);
                                btns[4].set_sensitive(mode != 0);
                                btns[5].set_sensitive(mode != 0);
                                match s.anc.level {
                                    1..=2 => {
                                        if !btns[3].is_active() { btns[3].set_active(true); }
                                    }
                                    3 => {
                                        if !btns[4].is_active() { btns[4].set_active(true); }
                                    }
                                    4..=5 => {
                                        if !btns[5].is_active() { btns[5].set_active(true); }
                                    }
                                    _ => {}
                                }
                                am.set(false);
                                if let Some(v) = s.features.adaptive_anc { asw.set_active(v); }
                                if let Some(v) = s.features.dual_device { dsw.set_active(v); }
                                ctx.success("Status refreshed");
                            }
                            Err(e) => ctx.error(format!("Status: {}", e)),
                        },
                        Err(e) => ctx.daemon_unreachable(e),
                    }
                });
            });
        }

        Self::connect_mode(&off_btn, 0, "off", &low_btn, &med_btn, &high_btn,
                           &adaptive_switch, &adaptive_row, &mode_spinner,
                           &applying, &current_mode, ctx.clone());
        Self::connect_mode(&anc_btn, 1, "anc", &low_btn, &med_btn, &high_btn,
                           &adaptive_switch, &adaptive_row, &mode_spinner,
                           &applying, &current_mode, ctx.clone());
        Self::connect_mode(&trans_btn, 2, "transparency", &low_btn, &med_btn, &high_btn,
                           &adaptive_switch, &adaptive_row, &mode_spinner,
                           &applying, &current_mode, ctx.clone());

        {
            let ctx = ctx.clone();
            let applying = applying.clone();
            let send_level = move |level: u8| {
                let ctx = ctx.clone();
                glib::spawn_future_local(async move {
                    if let Ok(mut client) = libspacepods::client::SpacePodsClient::connect(None).await {
                        if let Err(e) = client.set_level(level).await {
                            ctx.error(format!("Couldn't set ANC level: {}", e));
                        }
                    } else {
                        ctx.daemon_unreachable("connection failed");
                    }
                });
            };

            let applying1 = applying.clone();
            let low_send = send_level.clone();
            low_btn.connect_toggled(move |b| {
                if applying1.get() { return; }
                if b.is_active() { low_send(1); }
            });

            let applying2 = applying.clone();
            let med_send = send_level.clone();
            med_btn.connect_toggled(move |b| {
                if applying2.get() { return; }
                if b.is_active() { med_send(3); }
            });

            let applying3 = applying.clone();
            high_btn.connect_toggled(move |b| {
                if applying3.get() { return; }
                if b.is_active() { send_level(5); }
            });
        }
        
        {
            let ctx = ctx.clone();
            let applying = applying.clone();
            adaptive_switch.connect_state_set(move |sw, state| {
                if applying.get() { return glib::Propagation::Proceed; }
                let sw = sw.clone();
                let ctx = ctx.clone();
                glib::spawn_future_local(async move {
                    match libspacepods::client::SpacePodsClient::connect(None).await {
                        Ok(mut client) => {
                            if let Err(e) = client.set_adaptive_anc(state).await {
                                ctx.error(format!("Couldn't set Adaptive ANC: {}", e));
                                sw.set_state(!state);
                            }
                        }
                        Err(e) => {
                            ctx.daemon_unreachable(e);
                            sw.set_state(!state);
                        }
                    }
                });
                glib::Propagation::Proceed
            });
        }

        // ── Dual device switch ──
        {
            let ctx = ctx.clone();
            let applying = applying.clone();
            dual_switch.connect_state_set(move |sw, state| {
                if applying.get() { return glib::Propagation::Proceed; }
                let sw = sw.clone();
                let ctx = ctx.clone();
                glib::spawn_future_local(async move {
                    match libspacepods::client::SpacePodsClient::connect(None).await {
                        Ok(mut client) => {
                            if let Err(e) = client.set_dual_device(state).await {
                                ctx.error(format!("Couldn't set Dual Device: {}", e));
                                sw.set_state(!state);
                            }
                        }
                        Err(e) => {
                            ctx.daemon_unreachable(e);
                            sw.set_state(!state);
                        }
                    }
                });
                glib::Propagation::Proceed
            });
        }

        // ── Chat Mode switch ──
        {
            let ctx = ctx.clone();
            chat_switch.connect_state_set(move |sw, state| {
                let payload = vec![if state { 0x01 } else { 0x00 }];
                let cc = libspacepods::ipc::ServiceCommand::Custom { command_id: 0x35, payload };
                let sw = sw.clone();
                let ctx = ctx.clone();
                glib::spawn_future_local(async move {
                    if let Ok(mut client) = libspacepods::client::SpacePodsClient::connect(None).await {
                        if let Err(e) = client.send_command_raw(cc).await {
                            ctx.error(format!("Chat Mode: {}", e));
                            sw.set_state(!state);
                        }
                    } else {
                        ctx.daemon_unreachable("no connection");
                        sw.set_state(!state);
                    }
                });
                glib::Propagation::Proceed
            });
        }

        // ── Long Endurance switch ──
        {
            let ctx = ctx.clone();
            endurance_switch.connect_state_set(move |sw, state| {
                let payload = vec![if state { 0x01 } else { 0x00 }];
                let cc = libspacepods::ipc::ServiceCommand::Custom { command_id: 0x38, payload };
                let sw = sw.clone();
                let ctx = ctx.clone();
                glib::spawn_future_local(async move {
                    if let Ok(mut client) = libspacepods::client::SpacePodsClient::connect(None).await {
                        if let Err(e) = client.send_command_raw(cc).await {
                            ctx.error(format!("Long Endurance: {}", e));
                            sw.set_state(!state);
                        }
                    } else {
                        ctx.daemon_unreachable("no connection");
                        sw.set_state(!state);
                    }
                });
                glib::Propagation::Proceed
            });
        }

        {
            let ctx = ctx.clone();
            let finding = std::rc::Rc::new(std::cell::Cell::new(false));
            find_ear_btn.connect_clicked(move |btn| {
                let is_active = finding.get();
                if is_active {
                    // Stop finding
                    finding.set(false);
                    btn.set_label("Ring");
                    btn.remove_css_class("suggested-action");
                    btn.add_css_class("destructive-action");
                    let payload = vec![0x00];
                    let ctx = ctx.clone();
                    glib::spawn_future_local(async move {
                        let _ = libspacepods::client::SpacePodsClient::connect(None).await
                            .map(|mut c| glib::spawn_future_local(async move { let _ = c.send_command_raw(libspacepods::ipc::ServiceCommand::Custom { command_id: 0x2A, payload }).await; }));
                    });
                } else {
                    finding.set(true);
                    btn.set_label("Stop Ringing");
                    btn.remove_css_class("destructive-action");
                    btn.add_css_class("suggested-action");
                    let payload = vec![0x01];
                    let ctx = ctx.clone();
                    let finding = finding.clone();
                    let btn_ref = btn.clone();
                    glib::spawn_future_local(async move {
                        let result = match libspacepods::client::SpacePodsClient::connect(None).await {
                            Ok(mut client) => client.send_command_raw(
                                libspacepods::ipc::ServiceCommand::Custom {
                                    command_id: 0x2A,
                                    payload,
                                }
                            ).await,
                            Err(e) => Err(e),
                        };
                        if let Err(e) = result {
                            ctx.error(format!("Couldn't ring earbuds: {}", e));
                            finding.set(false);
                            btn_ref.set_label("Ring");
                            btn_ref.remove_css_class("suggested-action");
                            btn_ref.add_css_class("destructive-action");
                        }
                    });
                }
            });
        }

        clamp.upcast()
    }

    fn apply_mode_ui(
        mode: u8,
        low_btn: &ToggleButton,
        med_btn: &ToggleButton,
        high_btn: &ToggleButton,
        adaptive_switch: &Switch,
        adaptive_row: &ActionRow,
    ) {
        let sensitive = mode != 0;
        low_btn.set_sensitive(sensitive);
        med_btn.set_sensitive(sensitive);
        high_btn.set_sensitive(sensitive);
        adaptive_switch.set_sensitive(mode == 1);
        adaptive_row.set_sensitive(mode == 1);
    }

    fn connect_mode(
        btn: &ToggleButton, mode_id: u8, mode_name: &'static str,
        low_btn: &ToggleButton, med_btn: &ToggleButton, high_btn: &ToggleButton,
        adaptive_switch: &Switch, adaptive_row: &ActionRow,
        mode_spinner: &Spinner,
        applying: &Rc<Cell<bool>>, current_mode: &Rc<Cell<u8>>,
        ctx: Rc<AppContext>,
    ) {
        let low = low_btn.clone();
        let med = med_btn.clone();
        let high = high_btn.clone();
        let asw = adaptive_switch.clone();
        let ar = adaptive_row.clone();
        let spinner = mode_spinner.clone();
        let btn_owned = btn.clone();
        let btn_for_cb = btn.clone();
        let applying = Rc::clone(applying);
        let current_mode = Rc::clone(current_mode);
        let cmd = mode_name.to_string();

        btn_owned.connect_toggled(move |b| {
            if !b.is_active() {
                b.remove_css_class("suggested-action");
                return;
            }
            b.add_css_class("suggested-action");
            if applying.get() {
                Self::apply_mode_ui(mode_id, &low, &med, &high, &asw, &ar);
                return;
            }

            let previous_mode = current_mode.get();
            current_mode.set(mode_id);
            ctx.anc_mode.set(mode_id);
            crate::tray::ANC_MODE_ATOMIC.store(mode_id, std::sync::atomic::Ordering::Relaxed);
            Self::apply_mode_ui(mode_id, &low, &med, &high, &asw, &ar);

            spinner.set_visible(true);
            spinner.start();
            b.set_sensitive(false);

            let mc = cmd.clone();
            let ctx = ctx.clone();
            let btn_ref = btn_for_cb.clone();
            let spinner_ref = spinner.clone();
            let low_ref = low.clone();
            let med_ref = med.clone();
            let high_ref = high.clone();
            let asw_ref = asw.clone();
            let ar_ref = ar.clone();
            let current_mode_ref = current_mode.clone();
            glib::spawn_future_local(async move {
                let result = match libspacepods::client::SpacePodsClient::connect(None).await {
                    Ok(mut client) => client.set_anc_mode(&mc).await,
                    Err(e) => Err(e),
                };
                spinner_ref.stop();
                spinner_ref.set_visible(false);
                btn_ref.set_sensitive(true);

                if let Err(e) = result {
                    ctx.error(format!("Couldn't switch ANC mode: {}", e));
                    current_mode_ref.set(previous_mode);
                    ctx.anc_mode.set(previous_mode);
                    crate::tray::ANC_MODE_ATOMIC.store(previous_mode, std::sync::atomic::Ordering::Relaxed);
                    Self::apply_mode_ui(previous_mode, &low_ref, &med_ref, &high_ref, &asw_ref, &ar_ref);
                }
            });
        });
    }
}
