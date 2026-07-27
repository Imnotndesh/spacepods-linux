use std::rc::Rc;
use gtk4::prelude::*;
use gtk4::{Box, Label, Orientation, Switch};
use libadwaita::prelude::*;
use libadwaita::{PreferencesGroup, ActionRow, Clamp, StatusPage};
use glib::clone;

use crate::context::{AppContext, set_busy};
use crate::log::Log;

pub struct GamingPage;

impl GamingPage {
    pub fn new(ctx: Rc<AppContext>) -> gtk4::Widget {

        let status_page = StatusPage::new();
        status_page.set_icon_name(Some("input-gaming-symbolic"));
        status_page.set_title("Game Mode");
        status_page.set_description(Some(
            "Reduces audio latency for a better gaming experience. \
             May increase battery consumption."
        ));
        status_page.set_vexpand(true);

        let gaming_group = PreferencesGroup::new();
        gaming_group.set_title("Low Latency Mode");

        let game_switch_row = ActionRow::new();
        game_switch_row.set_title("Game Mode");
        game_switch_row.set_subtitle("Enable low-latency audio for gaming");
        let game_switch = Switch::new();
        game_switch.set_valign(gtk4::Align::Center);
        game_switch_row.add_suffix(&game_switch);
        game_switch_row.set_activatable_widget(Some(&game_switch));
        gaming_group.add(&game_switch_row);

        let status_label = Label::new(Some("Game Mode is OFF"));
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

                let payload = if active { vec![0x01] } else { vec![0x00] };
                let cc = libspacepods::ipc::ServiceCommand::Custom {
                    command_id: 0x25,
                    payload,
                };

                set_busy(&[&game_switch_row], true);
                let sw = sw.clone();
                let ctx = ctx.clone();
                let status_label = status_label.clone();
                let game_switch_row = game_switch_row.clone();
                glib::spawn_future_local(async move {
                    let result = match libspacepods::client::SpacePodsClient::connect(None).await {
                        Ok(mut client) => client.send_command_raw(cc).await,
                        Err(e) => Err(e),
                    };
                    set_busy(&[&game_switch_row], false);
                    if let Err(e) = result {
                        ctx.error(format!("Couldn't change Game Mode: {}", e));
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

        let title_lbl = Label::new(Some("Gaming Mode"));
        title_lbl.add_css_class("title-1");
        title_lbl.set_halign(gtk4::Align::Start);
        title_lbl.set_hexpand(true);
        header_row.append(&title_lbl);

        let refresh_btn = gtk4::Button::from_icon_name("view-refresh-symbolic");
        refresh_btn.add_css_class("flat");
        refresh_btn.add_css_class("circular");
        refresh_btn.set_valign(gtk4::Align::Center);
        refresh_btn.set_tooltip_text(Some("Refresh status"));
        header_row.append(&refresh_btn);

        let content = Box::new(Orientation::Vertical, 12);
        content.set_margin_top(0);
        content.set_margin_bottom(32);
        content.set_margin_start(16);
        content.set_margin_end(16);
        content.append(&header_row);
        content.append(&status_page);
        content.append(&gaming_group);
        content.append(&status_label);

        {
            let game_switch = game_switch.clone();
            let status_label = status_label.clone();
            let ctx = ctx.clone();
            refresh_btn.connect_clicked(move |_| {
                let gs = game_switch.clone();
                let sl = status_label.clone();
                let ctx = ctx.clone();
                glib::spawn_future_local(async move {
                    use libspacepods::client::SpacePodsClient;
                    match SpacePodsClient::connect(None).await {
                        Ok(mut client) => match client.get_status().await {
                            Ok(s) => {
                                // Game mode isn't in the status struct — use raw value from features/work_mode
                                ctx.success("Status refreshed");
                                Log::full("GAMING", &format!("Status: anc={:?} level={}", s.anc.mode, s.anc.level));
                            }
                            Err(e) => ctx.error(format!("Status: {}", e)),
                        },
                        Err(e) => ctx.daemon_unreachable(e),
                    }
                });
            });
        }

        let clamp = Clamp::new();
        clamp.set_maximum_size(500);
        clamp.set_child(Some(&content));

        clamp.upcast()
    }
}