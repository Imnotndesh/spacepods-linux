use std::rc::Rc;
use gtk4::prelude::*;
use gtk4::{Box, Label, Orientation, Switch};
use libadwaita::prelude::*;
use libadwaita::{PreferencesGroup, ActionRow, Clamp};

use crate::context::{AppContext, set_busy};

pub struct GamingPage;

impl GamingPage {
    pub fn new(ctx: Rc<AppContext>) -> gtk4::Widget {

        let gaming_group = PreferencesGroup::new();
        gaming_group.set_title("Low Latency Mode");

        let game_switch_row = ActionRow::new();
        game_switch_row.set_title("Game Mode");
        game_switch_row.set_subtitle("Enable low-latency audio for gaming");
        let game_switch = Switch::new();
        game_switch.set_sensitive(false); // unknown until read from the earbuds
        game_switch.set_valign(gtk4::Align::Center);
        game_switch_row.add_suffix(&game_switch);
        game_switch_row.set_activatable_widget(Some(&game_switch));
        gaming_group.add(&game_switch_row);

        let status_label = Label::new(Some("Game Mode is unavailable right now"));
        status_label.add_css_class("dim-label");
        status_label.set_halign(gtk4::Align::Center);

        {
            let status_label = status_label.clone();
            let ctx = ctx.clone();
            let game_switch_row = game_switch_row.clone();
            game_switch.connect_state_set(move |sw, active| {
                let previous_text = if active {
                    "Game Mode is OFF — Normal audio mode"
                } else {
                    "Game Mode is ON — Low latency audio"
                };

                status_label.set_text(if active {
                    "Game Mode is ON — Low latency audio"
                } else {
                    "Game Mode is OFF — Normal audio mode"
                });

                set_busy(&[&game_switch_row], true);
                let sw = sw.clone();
                let ctx = ctx.clone();
                let status_label = status_label.clone();
                let game_switch_row = game_switch_row.clone();
                glib::spawn_future_local(async move {
                    let result = match libspacepods::client::SpacePodsClient::connect(None).await {
                        Ok(mut client) => client.set_feature("game_mode", active).await,
                        Err(e) => Err(e),
                    };
                    set_busy(&[&game_switch_row], false);
                    if let Err(e) = result {
                        ctx.failed("Couldn't change Game Mode", &e);
                        // Revert — the earbuds never actually got the command.
                        sw.set_state(!active);
                        status_label.set_text(previous_text);
                    }
                });
                glib::Propagation::Proceed
            });
        }

        // ── Header with refresh ──
        let header_row = Box::new(Orientation::Horizontal, 0);
        header_row.set_margin_top(24);
        header_row.set_margin_bottom(8);


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

        let content = Box::new(Orientation::Vertical, 12);
        content.set_margin_top(0);
        content.set_margin_bottom(32);
        content.set_margin_start(16);
        content.set_margin_end(16);
        content.append(&header_row);
        content.append(&gaming_group);
        content.append(&status_label);

        {
            let game_switch = game_switch.clone();
            let status_label = status_label.clone();
            let ctx = ctx.clone();
            refresh_btn.connect_clicked(move |_| {
                let game_switch = game_switch.clone();
                let status_label = status_label.clone();
                crate::context::read_features(&ctx, &["game_mode"], move |_, on| {
                    game_switch.set_sensitive(true);
                    if game_switch.state() != on {
                        game_switch.set_state(on);
                    }
                    status_label.set_text(if on {
                        "Game Mode is ON — Low latency audio"
                    } else {
                        "Game Mode is OFF — Normal audio mode"
                    });
                });
            });
            // Read the state once on open, so switches show what the earbuds report.
            refresh_btn.emit_clicked();
        }

        let clamp = Clamp::new();
        clamp.set_maximum_size(500);
        clamp.set_child(Some(&content));

        clamp.upcast()
    }
}