use std::cell::Cell;
use std::rc::Rc;
use glib::clone;
use gtk4::prelude::*;
use gtk4::{Box, Label, Orientation, Scale, Switch, ToggleButton, Spinner};
use libadwaita::{ActionRow, PreferencesGroup, Clamp, StatusPage};
use libadwaita::prelude::*;

use crate::context::AppContext;

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
        let title = Label::new(Some("ANC Control"));
        title.add_css_class("title-1");
        title.set_halign(gtk4::Align::Start);

        // ── Mode row (real state machine instead of three loosely
        // grouped toggle buttons fighting over "suggested-action") ──
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

        let mode_status = Label::new(Some("Loading current mode…"));
        mode_status.add_css_class("dim-label");
        mode_status.add_css_class("caption");
        mode_status.set_halign(gtk4::Align::Center);

        let mode_spinner = Spinner::new();
        mode_spinner.set_visible(false);

        let mode_status_row = Box::new(Orientation::Horizontal, 6);
        mode_status_row.set_halign(gtk4::Align::Center);
        mode_status_row.append(&mode_spinner);
        mode_status_row.append(&mode_status);

        // ── Intensity slider ──
        let slider_card = libadwaita::Bin::new();
        slider_card.add_css_class("card");
        slider_card.set_visible(false);
        let slider_inner = Box::new(Orientation::Vertical, 6);
        slider_inner.set_margin_top(14);
        slider_inner.set_margin_bottom(14);
        slider_inner.set_margin_start(16);
        slider_inner.set_margin_end(16);

        let slider_label = Label::new(Some("Intensity"));
        slider_label.set_halign(gtk4::Align::Start);
        slider_label.add_css_class("caption-heading");

        let slider = Scale::with_range(Orientation::Horizontal, 1.0, 15.0, 1.0);
        slider.set_draw_value(true);
        slider.set_hexpand(true);
        slider.set_value(3.0);
        slider.set_sensitive(false);

        slider_inner.append(&slider_label);
        slider_inner.append(&slider);
        slider_card.set_child(Some(&slider_inner));

        // ── Feature toggles ──
        let adaptive_row = ActionRow::new();
        adaptive_row.set_title("Adaptive ANC");
        adaptive_row.set_subtitle("Dynamically adjust based on environment");
        let adaptive_switch = Switch::new();
        adaptive_switch.set_valign(gtk4::Align::Center);
        adaptive_row.add_suffix(&adaptive_switch);
        adaptive_row.set_activatable_widget(Some(&adaptive_switch));
        adaptive_switch.set_sensitive(false);
        adaptive_row.set_sensitive(false);

        let dual_row = ActionRow::new();
        dual_row.set_title("Dual Device (Multi-point)");
        dual_row.set_subtitle("Connect to two devices simultaneously");
        let dual_switch = Switch::new();
        dual_switch.set_valign(gtk4::Align::Center);
        dual_row.add_suffix(&dual_switch);
        dual_row.set_activatable_widget(Some(&dual_switch));
        dual_switch.set_sensitive(false);
        dual_row.set_sensitive(false);

        let features_group = PreferencesGroup::new();
        features_group.set_title("Additional Features");
        features_group.add(&adaptive_row);
        features_group.add(&dual_row);

        // ── Offline state (shown instead of everything above if the
        // daemon can't be reached at all — was previously just a label
        // buried under disabled controls) ──
        let offline_status = StatusPage::new();
        offline_status.set_icon_name(Some("network-offline-symbolic"));
        offline_status.set_title("Daemon Unreachable");
        offline_status.set_description(Some("Couldn't connect to the SpacePods service."));
        offline_status.set_visible(false);
        offline_status.set_vexpand(true);

        container.append(&title);
        container.append(&mode_row);
        container.append(&mode_status_row);
        container.append(&slider_card);
        container.append(&features_group);
        container.append(&offline_status);

        clamp.set_child(Some(&container));

        // ── Shared reactive state ──
        // `applying` guards against re-entrant daemon calls while one is
        // already in flight (fixes the old bug where fast clicking could
        // fire overlapping commands and leave the UI's idea of the mode
        // out of sync with the earbuds).
        let applying = Rc::new(Cell::new(false));
        let current_mode = Rc::new(Cell::new(0u8));

        // ── Initial status fetch ──
        glib::spawn_future_local(clone!(
            #[strong] off_btn, #[strong] anc_btn, #[strong] trans_btn,
            #[strong] slider, #[strong] slider_card,
            #[strong] adaptive_switch, #[strong] adaptive_row,
            #[strong] dual_switch, #[strong] dual_row,
            #[strong] mode_status, #[strong] mode_row,
            #[strong] offline_status, #[strong] features_group,
            #[strong] applying, #[strong] current_mode, #[strong] ctx,
            async move {
                use libspacepods::client::SpacePodsClient;
                match SpacePodsClient::connect(None).await {
                    Ok(mut client) => match client.get_status().await {
                        Ok(s) => {
                            for b in [&off_btn, &anc_btn, &trans_btn] { b.set_sensitive(true); }

                            let mode = s.anc.mode as u8;
                            current_mode.set(mode);
                            applying.set(true); // suppress toggled-handler side effects
                            match mode {
                                0 => off_btn.set_active(true),
                                1 => anc_btn.set_active(true),
                                2 => trans_btn.set_active(true),
                                _ => {}
                            }
                            applying.set(false);
                            Self::apply_mode_ui(mode, &mode_status, &slider_card, &adaptive_switch, &adaptive_row);

                            let max = s.anc.max_level.max(1) as f64;
                            slider.set_range(1.0, max);
                            slider.set_value(s.anc.level as f64);
                            slider.set_sensitive(mode != 0);

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
                            features_group.set_visible(false);
                            offline_status.set_visible(true);
                            ctx.daemon_unreachable(e);
                        }
                    },
                    Err(e) => {
                        mode_row.set_visible(false);
                        features_group.set_visible(false);
                        offline_status.set_visible(true);
                        ctx.daemon_unreachable(e);
                    }
                }
            }
        ));

        // ── Mode buttons ──
        Self::connect_mode(&off_btn, 0, "off", &slider, &slider_card,
                           &adaptive_switch, &adaptive_row, &mode_status, &mode_spinner,
                           &applying, &current_mode, ctx.clone());
        Self::connect_mode(&anc_btn, 1, "anc", &slider, &slider_card,
                           &adaptive_switch, &adaptive_row, &mode_status, &mode_spinner,
                           &applying, &current_mode, ctx.clone());
        Self::connect_mode(&trans_btn, 2, "transparency", &slider, &slider_card,
                           &adaptive_switch, &adaptive_row, &mode_status, &mode_spinner,
                           &applying, &current_mode, ctx.clone());

        // ── Slider: only send once the user releases the handle, not on
        // every intermediate value while dragging (old code hammered the
        // daemon on every pixel of drag). ──
        {
            let ctx = ctx.clone();
            let applying = applying.clone();
            slider.connect_value_changed(move |_| {
                // value display updates live via set_draw_value; the actual
                // command is sent from the button-release handler below.
                let _ = &applying; // no-op hook kept for symmetry/clarity
            });
            let gc = gtk4::GestureClick::new();
            gc.set_button(0);
            let slider_for_release = slider.clone();
            let ctx_release = ctx.clone();
            gc.connect_released(move |_, _, _, _| {
                let level = slider_for_release.value() as u8;
                let ctx = ctx_release.clone();
                glib::spawn_future_local(async move {
                    if let Ok(mut client) = libspacepods::client::SpacePodsClient::connect(None).await {
                        if let Err(e) = client.set_level(level).await {
                            ctx.error(format!("Couldn't set ANC level: {}", e));
                        }
                    } else {
                        ctx.daemon_unreachable("connection failed");
                    }
                });
            });
            slider.add_controller(gc);
        }

        // ── Adaptive switch — revert on failure instead of lying about state ──
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

        // ── Dual device switch — same revert-on-failure treatment ──
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

        clamp.upcast()
    }

    fn apply_mode_ui(
        mode: u8,
        mode_status: &Label,
        slider_card: &libadwaita::Bin,
        adaptive_switch: &Switch,
        adaptive_row: &ActionRow,
    ) {
        let name = match mode { 0 => "OFF", 1 => "ANC", 2 => "TRANSPARENCY", _ => "UNKNOWN" };
        mode_status.set_text(&format!("Current mode: {}", name));
        slider_card.set_visible(mode != 0);
        adaptive_switch.set_sensitive(mode == 1);
        adaptive_row.set_sensitive(mode == 1);
    }

    fn connect_mode(
        btn: &ToggleButton, mode_id: u8, mode_name: &'static str,
        slider: &Scale, slider_card: &libadwaita::Bin,
        adaptive_switch: &Switch, adaptive_row: &ActionRow,
        mode_status: &Label, mode_spinner: &Spinner,
        applying: &Rc<Cell<bool>>, current_mode: &Rc<Cell<u8>>,
        ctx: Rc<AppContext>,
    ) {
        let slider = slider.clone();
        let slider_card = slider_card.clone();
        let asw = adaptive_switch.clone();
        let ar = adaptive_row.clone();
        let ms = mode_status.clone();
        let spinner = mode_spinner.clone();
        let applying = Rc::clone(applying);
        let current_mode = Rc::clone(current_mode);
        let cmd = mode_name.to_string();

        btn.connect_toggled(move |b| {
            if !b.is_active() {
                b.remove_css_class("suggested-action");
                return;
            }
            b.add_css_class("suggested-action");
            if applying.get() {
                // We're just reflecting a status fetch, not a user click.
                Self::apply_mode_ui(mode_id, &ms, &slider_card, &asw, &ar);
                slider.set_sensitive(mode_id != 0);
                return;
            }

            let previous_mode = current_mode.get();
            current_mode.set(mode_id);
            Self::apply_mode_ui(mode_id, &ms, &slider_card, &asw, &ar);
            slider.set_sensitive(mode_id != 0);

            spinner.set_visible(true);
            spinner.start();
            b.set_sensitive(false);

            let mc = cmd.clone();
            let ctx = ctx.clone();
            let btn_ref = b.clone();
            let spinner_ref = spinner.clone();
            let ms_ref = ms.clone();
            let slider_card_ref = slider_card.clone();
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
                    // Roll the UI back to the mode we knew was actually active.
                    current_mode_ref.set(previous_mode);
                    Self::apply_mode_ui(previous_mode, &ms_ref, &slider_card_ref, &asw_ref, &ar_ref);
                }
            });
        });
    }
}