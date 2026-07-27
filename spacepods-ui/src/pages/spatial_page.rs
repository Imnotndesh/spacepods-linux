use std::rc::Rc;
use gtk4::prelude::*;
use gtk4::{Box, Label, Orientation, Switch, Button, Align};
use libadwaita::prelude::*;
use libadwaita::{PreferencesGroup, ActionRow, Clamp, StatusPage};

use crate::context::AppContext;

pub struct SpatialAudioPage;

impl SpatialAudioPage {
    pub fn new(ctx: Rc<AppContext>) -> gtk4::Widget {

        let status_page = StatusPage::new();
        status_page.set_icon_name(Some("audio-speakers-symbolic"));
        status_page.set_title("3D Spatial Audio");
        status_page.set_description(Some(
            "Creates a surround sound experience with head tracking. \
             Makes audio feel like it's coming from all around you."
        ));
        status_page.set_vexpand(true);

        // ── Space Audio toggle ──
        let space_group = PreferencesGroup::new();
        space_group.set_title("Spatial Audio");

        let space_switch_row = ActionRow::new();
        space_switch_row.set_title("Space Audio");
        space_switch_row.set_subtitle("Enable 3D spatial audio processing");
        let space_switch = Switch::new();
        space_switch.set_valign(gtk4::Align::Center);
        space_switch_row.add_suffix(&space_switch);
        space_switch_row.set_activatable_widget(Some(&space_switch));
        space_group.add(&space_switch_row);
        
        let status_label = Label::new(Some("Spatial Audio is OFF"));
        status_label.add_css_class("dim-label");
        status_label.set_halign(gtk4::Align::Center);

        {
            let status_label = status_label.clone();
            let ctx_toggle = ctx.clone();
            space_switch.connect_state_set(move |sw, active| {
                let previous_text = if active {
                    "Spatial Audio is OFF — Standard stereo"
                } else {
                    "Spatial Audio is ON — Immersive 3D sound"
                };

                status_label.set_text(if active {
                    "Spatial Audio is ON — Immersive 3D sound"
                } else {
                    "Spatial Audio is OFF — Standard stereo"
                });
                
                let payload = if active { vec![0x01] } else { vec![0x00] };
                let cc = libspacepods::ipc::ServiceCommand::Custom {
                    command_id: 0x36,
                    payload,
                };

                let sw = sw.clone();
                let ctx = ctx_toggle.clone();
                let status_label = status_label.clone();
                glib::spawn_future_local(async move {
                    let result = match libspacepods::client::SpacePodsClient::connect(None).await {
                        Ok(mut client) => client.send_command_raw(cc).await,
                        Err(e) => Err(e),
                    };
                    if let Err(e) = result {
                        ctx.error(format!("Couldn't change Spatial Audio: {}", e));
                        sw.set_state(!active);
                        status_label.set_text(previous_text);
                    }
                });
                glib::Propagation::Proceed
            });
        }
        
        let content = Box::new(Orientation::Vertical, 12);
        content.set_margin_top(24);
        content.set_margin_bottom(32);
        content.set_margin_start(16);
        content.set_margin_end(16);
        content.append(&status_page);
        content.append(&space_group);
        content.append(&status_label);

        let refresh_btn = Button::with_label("Refresh");
        refresh_btn.add_css_class("flat");
        refresh_btn.set_halign(Align::Center);
        refresh_btn.set_margin_top(8);
        content.append(&refresh_btn);

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

        let clamp = Clamp::new();
        clamp.set_maximum_size(500);
        clamp.set_child(Some(&content));

        clamp.upcast()
    }
}