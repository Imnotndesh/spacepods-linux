use std::rc::Rc;
use gtk4::prelude::*;
use gtk4::{Box, Label, Orientation, Switch};
use libadwaita::prelude::*;
use libadwaita::{PreferencesGroup, ActionRow, Clamp};

use crate::context::AppContext;

pub struct SpatialAudioPage;

impl SpatialAudioPage {
    pub fn new(ctx: Rc<AppContext>) -> gtk4::Widget {

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

        let space_group = PreferencesGroup::new();
        space_group.set_title("Spatial Audio");

        let space_switch_row = ActionRow::new();
        space_switch_row.set_title("Space Audio");
        space_switch_row.set_subtitle("Enable 3D spatial audio processing");
        let space_switch = Switch::new();
        space_switch.set_sensitive(false); // unknown until read from the earbuds
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
                
                let sw = sw.clone();
                let ctx = ctx_toggle.clone();
                let status_label = status_label.clone();
                glib::spawn_future_local(async move {
                    let result = match libspacepods::client::SpacePodsClient::connect(None).await {
                        Ok(mut client) => client.set_feature("space_audio", active).await,
                        Err(e) => Err(e),
                    };
                    if let Err(e) = result {
                        ctx.failed("Couldn't change Spatial Audio", &e);
                        sw.set_state(!active);
                        status_label.set_text(previous_text);
                    }
                });
                glib::Propagation::Proceed
            });
        }
        
        let content = Box::new(Orientation::Vertical, 12);
        content.set_margin_top(0);
        content.set_margin_bottom(32);
        content.set_margin_start(16);
        content.set_margin_end(16);
        content.append(&header_row);
        content.append(&space_group);
        content.append(&status_label);

        {
            let ctx = ctx.clone();
            refresh_btn.connect_clicked(move |_| {
                let switch = space_switch.clone();
                let space_switch = space_switch.clone();
                crate::context::read_features(&ctx, &["space_audio"], move |_, on| {
                    space_switch.set_sensitive(true);
                    if switch.state() != on {
                        switch.set_state(on);
                    }
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