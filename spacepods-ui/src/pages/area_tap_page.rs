use std::rc::Rc;
use gtk4::prelude::*;
use gtk4::{Box, Label, Orientation, Switch};
use libadwaita::prelude::*;
use libadwaita::{PreferencesGroup, ActionRow, Clamp, StatusPage};

use crate::context::AppContext;

pub struct AreaTapPage;

impl AreaTapPage {
    pub fn new(ctx: Rc<AppContext>) -> gtk4::Widget {
        let header_row = Box::new(Orientation::Horizontal, 0);
        let title = Label::new(Some("Wide Area Tap"));
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

        let area_group = PreferencesGroup::new();
        area_group.set_title("Area Tap");

        let area_switch_row = ActionRow::new();
        area_switch_row.set_title("Wide Area Tap");
        area_switch_row.set_subtitle("Enable zone-based tap detection using microphones");
        let area_switch = Switch::new();
        area_switch.set_valign(gtk4::Align::Center);
        area_switch_row.add_suffix(&area_switch);
        area_switch_row.set_activatable_widget(Some(&area_switch));
        area_group.add(&area_switch_row);

        let status_label = Label::new(Some("Wide Area Tap is OFF"));
        status_label.add_css_class("dim-label");
        status_label.set_halign(gtk4::Align::Center);

        {
            let status_label = status_label.clone();
            let ctx = ctx.clone();
            area_switch.connect_state_set(move |sw, active| {
                status_label.set_text(if active {
                    "Wide Area Tap is ON"
                } else {
                    "Wide Area Tap is OFF"
                });

                let payload = if active { vec![0x01] } else { vec![0x00] };
                let cc = libspacepods::ipc::ServiceCommand::Custom {
                    command_id: 0x34, // CMD_AREA_TAP
                    payload,
                };

                let sw = sw.clone();
                let ctx = ctx.clone();
                let status_label = status_label.clone();
                glib::spawn_future_local(async move {
                    let result = match libspacepods::client::SpacePodsClient::connect(None).await {
                        Ok(mut client) => client.send_command_raw(cc).await,
                        Err(e) => Err(e),
                    };
                    if let Err(e) = result {
                        ctx.error(format!("Couldn't change Area Tap: {}", e));
                        sw.set_state(!active);
                        status_label.set_text(if active {
                            "Wide Area Tap is OFF"
                        } else {
                            "Wide Area Tap is ON"
                        });
                    }
                });
                glib::Propagation::Proceed
            });
        }

        // Refresh button
        {
            let ctx = ctx.clone();
            refresh_btn.connect_clicked(move |_| {
                let ctx = ctx.clone();
                glib::spawn_future_local(async move {
                    use libspacepods::client::SpacePodsClient;
                    match SpacePodsClient::connect(None).await {
                        Ok(mut client) => match client.get_status().await {
                            Ok(_) => ctx.success("Status refreshed"),
                            Err(e) => ctx.error(format!("Status: {}", e)),
                        },
                        Err(e) => ctx.daemon_unreachable(e),
                    }
                });
            });
        }

        let content = Box::new(Orientation::Vertical, 12);
        content.set_margin_top(0);
        content.set_margin_bottom(32);
        content.set_margin_start(16);
        content.set_margin_end(16);
        content.append(&header_row);
        content.append(&area_group);
        content.append(&status_label);

        let clamp = Clamp::new();
        clamp.set_maximum_size(500);
        clamp.set_child(Some(&content));

        clamp.upcast()
    }
}
