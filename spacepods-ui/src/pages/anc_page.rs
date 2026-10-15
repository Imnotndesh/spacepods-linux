use std::cell::Cell;
use std::rc::Rc;
use glib::clone;
use gtk4::prelude::*;
use gtk4::{Box, Orientation, Switch, ToggleButton, Spinner};
use libadwaita::{ActionRow, PreferencesGroup, Clamp};
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
        // Fill the page height, so the offline status page is centred in the space it has.
        container.set_vexpand(true);
        clamp.set_vexpand(true);
        container.set_margin_top(24);
        container.set_margin_bottom(32);
        container.set_margin_start(16);
        container.set_margin_end(16);

        // ── Header ──
        let header_row = Box::new(Orientation::Horizontal, 0);

        let refresh_btn = gtk4::Button::from_icon_name("view-refresh-symbolic");
        refresh_btn.add_css_class("flat");
        refresh_btn.add_css_class("circular");
        refresh_btn.set_valign(gtk4::Align::Center);
        refresh_btn.set_tooltip_text(Some("Refresh status"));
        refresh_btn.set_hexpand(true);
        refresh_btn.set_halign(gtk4::Align::End);
        header_row.append(&refresh_btn);
        // The window header carries the Refresh button and triggers this one.
        refresh_btn.set_widget_name("page-refresh");
        header_row.set_visible(false);

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
        // Shown only once the earbuds report they are connected (see the status check below).
        features_group.set_visible(false);

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

        let offline_status = crate::widgets::centered_message(
            "network-offline-symbolic",
            "Can't reach SpacePods",
            "Check that the SpacePods background service is running. Use Retry above to try again.",
        );
        offline_status.set_visible(false);

        container.append(&header_row);
        // Above the mode switcher: the connected device's drawing (it changes with the
        // device kind) and a battery row below it (icon, then the percentage).
        let device_name = ctx.profile().map(|p| p.name).unwrap_or("");
        let device_picture = gtk4::Image::from_paintable(crate::widgets::still_for_kind(
            crate::widgets::kind_for_name(device_name),
        ).as_ref());
        device_picture.set_pixel_size(144);
        device_picture.set_halign(gtk4::Align::Center);
        device_picture.set_margin_bottom(4);
        device_picture.update_property(&[gtk4::accessible::Property::Label("Your earbuds")]);
        container.append(&device_picture);

        let battery_row = Box::new(Orientation::Horizontal, 6);
        battery_row.set_halign(gtk4::Align::Center);
        battery_row.set_margin_bottom(12);
        let battery_icon = gtk4::Picture::new();
        battery_icon.set_size_request(28, 14);
        battery_icon.set_content_fit(gtk4::ContentFit::Contain);
        battery_icon.set_can_shrink(true);
        let battery_text = gtk4::Label::new(Some("—"));
        battery_text.add_css_class("caption-heading");
        battery_row.append(&battery_icon);
        battery_row.append(&battery_text);
        battery_row.set_tooltip_text(Some("Battery"));
        battery_row.update_property(&[gtk4::accessible::Property::Label("Battery")]);
        container.append(&battery_row);

        // Keep the picture and the battery row in step with the connection. The connect
        // animation plays once per run of the app.
        {
            let device_picture = device_picture.clone();
            let battery_icon = battery_icon.clone();
            let battery_text = battery_text.clone();
            let battery_row = battery_row.clone();
            let shown_kind = std::rc::Rc::new(std::cell::Cell::new(None::<crate::widgets::DeviceKind>));
            let update = std::rc::Rc::new(move |s: &libspacepods::ipc::protocol::DeviceStatus| {
                let level = s.battery.level;
                battery_icon.set_paintable(crate::widgets::battery_art(level).as_ref());
                battery_text.set_text(&match level {
                    Some(l) => format!("{}%", l),
                    None => "—".to_string(),
                });
                // Unknown battery is dimmed, not guessed.
                battery_row.set_opacity(if level.is_some() { 1.0 } else { 0.5 });

                if s.connection.connected {
                    let kind = libspacepods::device_profile::profile_for_product(s.product_id.unwrap_or(0));
                    let kind = crate::widgets::kind_for_name(kind.name);
                    if shown_kind.get() != Some(kind) {
                        shown_kind.set(Some(kind));
                        if crate::widgets::claim_intro() {
                            crate::widgets::play_connect_animation(&device_picture, kind);
                        } else {
                            device_picture.set_paintable(crate::widgets::still_for_kind(kind).as_ref());
                        }
                    }
                }
            });
            // Status may already have arrived before this page was built.
            let listener = update.clone();
            ctx.on_status(move |s| listener(s));
            let current = ctx.status.borrow().clone();
            if let Some(s) = current {
                update(&s);
            }
        }
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
            #[strong] mode_row, #[strong] intensity_btns,
            #[strong] offline_status, #[strong] features_group, #[strong] anc_group, #[strong] container,
            #[strong] applying, #[strong] current_mode, #[strong] ctx,
            async move {
                use libspacepods::client::SpacePodsClient;
                match SpacePodsClient::connect(None).await {
                    Ok(mut client) => match client.get_status().await {
                        Ok(s) => {
                            // Back online: show the controls that were hidden while offline.
                            offline_status.set_visible(false);
                            container.set_margin_top(24);
                            container.set_margin_bottom(32);
                            mode_row.set_visible(true);
                            anc_group.set_visible(true);
                            // The feature rows only mean something with earbuds connected.
                            features_group.set_visible(s.connection.connected);
                            intensity_btns.set_visible(true);
                            Log::info("ANC", &format!("Status received: mode={:?} level={} max={} adaptive={:?} dual={:?}",
                                s.anc.mode, s.anc.level, s.anc.max_level,
                                s.features.adaptive_anc, s.features.dual_device));
                            Self::apply_status(
                                &s, &off_btn, &anc_btn, &trans_btn,
                                &low_btn, &med_btn, &high_btn,
                                &adaptive_switch, &adaptive_row,
                                &dual_switch, &dual_row,
                                &applying, &current_mode, &ctx,
                            );
                        }
                        Err(e) => {
                            mode_row.set_visible(false);
                            intensity_btns.set_visible(false);
                            anc_group.set_visible(false);
                            features_group.set_visible(false);
                            offline_status.set_visible(true);
                            container.set_margin_top(0);
                            container.set_margin_bottom(0);
                            ctx.daemon_unreachable(e);
                        }
                    },
                    Err(e) => {
                        mode_row.set_visible(false);
                        intensity_btns.set_visible(false);
                        anc_group.set_visible(false);
                        features_group.set_visible(false);
                        offline_status.set_visible(true);
                        container.set_margin_top(0);
                        container.set_margin_bottom(0);
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
                            Err(e) => ctx.failed("Couldn't read the status", &e),
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

        // React to pushed status updates (from the daemon, CLI, or earbuds).
        {
            let off = off_btn.clone();
            let anc = anc_btn.clone();
            let trans = trans_btn.clone();
            let low = low_btn.clone();
            let med = med_btn.clone();
            let high = high_btn.clone();
            let asw = adaptive_switch.clone();
            let ar = adaptive_row.clone();
            let dsw = dual_switch.clone();
            let dr = dual_row.clone();
            let applying = applying.clone();
            let current_mode = current_mode.clone();
            let ctx_weak = Rc::downgrade(&ctx);
            ctx.on_status(move |s| {
                if let Some(ctx) = ctx_weak.upgrade() {
                    Self::apply_status(
                        s, &off, &anc, &trans, &low, &med, &high,
                        &asw, &ar, &dsw, &dr, &applying, &current_mode, &ctx,
                    );
                }
            });
        }

        {
            let ctx = ctx.clone();
            let applying = applying.clone();
            let send_level = move |level: u8| {
                let ctx = ctx.clone();
                glib::spawn_future_local(async move {
                    if let Ok(mut client) = libspacepods::client::SpacePodsClient::connect(None).await {
                        if let Err(e) = client.set_level(level).await {
                            ctx.failed("Couldn't set ANC level", &e);
                        }
                    } else {
                        ctx.error("Can't reach the SpacePods background service.");
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
                                ctx.failed("Couldn't set Adaptive ANC", &e);
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
                                ctx.failed("Couldn't set Dual Device", &e);
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
                let sw = sw.clone();
                let ctx = ctx.clone();
                glib::spawn_future_local(async move {
                    if let Ok(mut client) = libspacepods::client::SpacePodsClient::connect(None).await {
                        if let Err(e) = client.set_feature("chat_mode", state).await {
                            ctx.failed("Couldn't change Chat Mode", &e);
                            sw.set_state(!state);
                        }
                    } else {
                        ctx.error("Can't reach the SpacePods background service.");
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
                let sw = sw.clone();
                let ctx = ctx.clone();
                glib::spawn_future_local(async move {
                    if let Ok(mut client) = libspacepods::client::SpacePodsClient::connect(None).await {
                        if let Err(e) = client.set_feature("long_endurance", state).await {
                            ctx.failed("Couldn't change Long Endurance", &e);
                            sw.set_state(!state);
                        }
                    } else {
                        ctx.error("Can't reach the SpacePods background service.");
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
                        match libspacepods::client::SpacePodsClient::connect(None).await {
                            Ok(mut c) => {
                                let cmd = libspacepods::ipc::ServiceCommand::Custom { command_id: 0x2A, payload };
                                if let Err(e) = c.send_command_raw(cmd).await {
                                    ctx.failed("Couldn't stop ringing the earbuds", &e);
                                }
                            }
                            Err(e) => ctx.daemon_unreachable(e),
                        }
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
                            ctx.failed("Couldn't ring earbuds", &e);
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

    /// Apply a daemon status snapshot to the ANC widgets. Used both for the
    /// initial load and for pushed status updates, so the page stays reactive.
    #[allow(clippy::too_many_arguments)]
    fn apply_status(
        s: &libspacepods::ipc::protocol::DeviceStatus,
        off_btn: &ToggleButton,
        anc_btn: &ToggleButton,
        trans_btn: &ToggleButton,
        low_btn: &ToggleButton,
        med_btn: &ToggleButton,
        high_btn: &ToggleButton,
        adaptive_switch: &Switch,
        adaptive_row: &ActionRow,
        dual_switch: &Switch,
        dual_row: &ActionRow,
        applying: &Rc<Cell<bool>>,
        current_mode: &Rc<Cell<u8>>,
        ctx: &Rc<AppContext>,
    ) {
        for b in [off_btn, anc_btn, trans_btn] {
            b.set_sensitive(true);
        }
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
        Self::apply_mode_ui(mode, low_btn, med_btn, high_btn, adaptive_switch, adaptive_row);
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
            Log::info("ANC", &format!("mode button {} toggled active (applying={})", mode_id, applying.get()));
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
                Log::info("ANC", &format!("set_anc_mode '{}' -> {}", mc, if result.is_ok() { "ok" } else { "failed" }));
                spinner_ref.stop();
                spinner_ref.set_visible(false);
                btn_ref.set_sensitive(true);

                if let Err(e) = result {
                    Log::warn("ANC", &format!("set_anc_mode '{}' failed: {}", mc, e));
                    ctx.failed("Couldn't switch ANC mode", &e);
                    current_mode_ref.set(previous_mode);
                    ctx.anc_mode.set(previous_mode);
                    crate::tray::ANC_MODE_ATOMIC.store(previous_mode, std::sync::atomic::Ordering::Relaxed);
                    Self::apply_mode_ui(previous_mode, &low_ref, &med_ref, &high_ref, &asw_ref, &ar_ref);
                }
            });
        });
    }
}
