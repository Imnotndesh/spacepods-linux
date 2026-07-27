use std::rc::Rc;
use gtk4::prelude::*;
use gtk4::{Box, Orientation, Switch, Scale};
use libadwaita::prelude::*;
use libadwaita::{PreferencesGroup, ActionRow, Clamp, StatusPage};

use crate::context::{AppContext, set_busy};

pub struct HearingPage;

impl HearingPage {
    pub fn new(ctx: Rc<AppContext>) -> gtk4::Widget {

        let status_page = StatusPage::new();
        status_page.set_icon_name(Some("heart-symbolic"));
        status_page.set_title("Hearing Care");
        status_page.set_description(Some(
            "Monitor and protect your hearing with smart volume management."
        ));
        status_page.set_vexpand(true);
        
        let volume_group = PreferencesGroup::new();
        volume_group.set_title("Volume Management");

        let adapt_vol_row = ActionRow::new();
        adapt_vol_row.set_title("Adaptive Volume");
        adapt_vol_row.set_subtitle("Automatically adjust volume based on ambient noise");
        let adapt_switch = Switch::new();
        adapt_switch.set_valign(gtk4::Align::Center);
        adapt_vol_row.add_suffix(&adapt_switch);
        adapt_vol_row.set_activatable_widget(Some(&adapt_switch));
        volume_group.add(&adapt_vol_row);
        
        let tone_group = PreferencesGroup::new();
        tone_group.set_title("Tone Settings");

        let tone_vol_row = ActionRow::new();
        tone_vol_row.set_title("Tone Volume");
        tone_vol_row.set_subtitle("Volume of button tones and voice prompts");
        let tone_vol_scale = Scale::with_range(Orientation::Horizontal, 0.0, 10.0, 1.0);
        tone_vol_scale.set_hexpand(true);
        tone_vol_scale.set_draw_value(true);
        tone_vol_scale.set_value(7.0);
        tone_vol_scale.set_valign(gtk4::Align::Center);
        tone_vol_row.add_suffix(&tone_vol_scale);
        tone_group.add(&tone_vol_row);
        
        {
            let ctx = ctx.clone();
            let tone_vol_row = tone_vol_row.clone();
            let gc = gtk4::GestureClick::new();
            gc.set_button(0);
            let scale_for_release = tone_vol_scale.clone();
            gc.connect_released(move |_, _, _, _| {
                let level = scale_for_release.value() as u8;
                let cc = libspacepods::ipc::ServiceCommand::Custom {
                    command_id: 0x46,
                    payload: vec![level],
                };
                set_busy(&[&tone_vol_row], true);
                let ctx = ctx.clone();
                let tone_vol_row = tone_vol_row.clone();
                glib::spawn_future_local(async move {
                    let result = match libspacepods::client::SpacePodsClient::connect(None).await {
                        Ok(mut client) => client.send_command_raw(cc).await,
                        Err(e) => Err(e),
                    };
                    set_busy(&[&tone_vol_row], false);
                    if let Err(e) = result {
                        ctx.error(format!("Couldn't set tone volume: {}", e));
                    }
                });
            });
            tone_vol_scale.add_controller(gc);
        }
        
        let detect_group = PreferencesGroup::new();
        detect_group.set_title("Wear Detection");

        let ear_detect_row = ActionRow::new();
        ear_detect_row.set_title("In-Ear Detection");
        ear_detect_row.set_subtitle("Auto-pause music when removing earbuds");
        let ear_switch = Switch::new();
        ear_switch.set_valign(gtk4::Align::Center);
        ear_switch.set_active(true);
        ear_detect_row.add_suffix(&ear_switch);
        ear_detect_row.set_activatable_widget(Some(&ear_switch));
        detect_group.add(&ear_detect_row);

        let auto_answer_row = ActionRow::new();
        auto_answer_row.set_title("Auto Answer Calls");
        auto_answer_row.set_subtitle("Automatically answer incoming calls");
        let answer_switch = Switch::new();
        answer_switch.set_valign(gtk4::Align::Center);
        auto_answer_row.add_suffix(&answer_switch);
        auto_answer_row.set_activatable_widget(Some(&answer_switch));
        detect_group.add(&auto_answer_row);

        {
            let ctx = ctx.clone();
            answer_switch.connect_state_set(move |sw, active| {
                let payload = vec![if active { 0x01 } else { 0x00 }];
                let cc = libspacepods::ipc::ServiceCommand::Custom {
                    command_id: 0x47,
                    payload,
                };
                let sw = sw.clone();
                let ctx = ctx.clone();
                glib::spawn_future_local(async move {
                    let result = match libspacepods::client::SpacePodsClient::connect(None).await {
                        Ok(mut client) => client.send_command_raw(cc).await,
                        Err(e) => Err(e),
                    };
                    if let Err(e) = result {
                        ctx.error(format!("Couldn't change Auto Answer: {}", e));
                        sw.set_state(!active);
                    }
                });
                glib::Propagation::Proceed
            });
        }
        
        let voice_group = PreferencesGroup::new();
        voice_group.set_title("Voice Prompts");

        let voice_row = ActionRow::new();
        voice_row.set_title("Voice Prompts");
        voice_row.set_subtitle("Spoken notifications for mode changes");
        let voice_switch = Switch::new();
        voice_switch.set_valign(gtk4::Align::Center);
        voice_switch.set_active(true);
        voice_row.add_suffix(&voice_switch);
        voice_row.set_activatable_widget(Some(&voice_switch));
        voice_group.add(&voice_row);

        {
            let ctx = ctx.clone();
            voice_switch.connect_state_set(move |sw, active| {
                let payload = vec![if active { 0x01 } else { 0x00 }];
                let cc = libspacepods::ipc::ServiceCommand::Custom {
                    command_id: 0x48, // CMD_VOICE_PROMPTS
                    payload,
                };
                let sw = sw.clone();
                let ctx = ctx.clone();
                glib::spawn_future_local(async move {
                    let result = match libspacepods::client::SpacePodsClient::connect(None).await {
                        Ok(mut client) => client.send_command_raw(cc).await,
                        Err(e) => Err(e),
                    };
                    if let Err(e) = result {
                        ctx.error(format!("Couldn't change Voice Prompts: {}", e));
                        sw.set_state(!active);
                    }
                });
                glib::Propagation::Proceed
            });
        }
        
        {
            let ctx = ctx.clone();
            let adapt_vol_row = adapt_vol_row.clone();
            adapt_switch.connect_state_set(move |sw, active| {
                let payload = vec![if active { 0x01 } else { 0x00 }];
                let cc = libspacepods::ipc::ServiceCommand::Custom {
                    command_id: 0x45, // CMD_ADAPTIVE_VOLUME
                    payload,
                };
                set_busy(&[&adapt_vol_row], true);
                let sw = sw.clone();
                let ctx = ctx.clone();
                let adapt_vol_row = adapt_vol_row.clone();
                glib::spawn_future_local(async move {
                    let result = match libspacepods::client::SpacePodsClient::connect(None).await {
                        Ok(mut client) => client.send_command_raw(cc).await,
                        Err(e) => Err(e),
                    };
                    set_busy(&[&adapt_vol_row], false);
                    if let Err(e) = result {
                        ctx.error(format!("Couldn't set Adaptive Volume: {}", e));
                        sw.set_state(!active);
                    }
                });
                glib::Propagation::Proceed
            });
        }

        {
            let ctx = ctx.clone();
            let ear_detect_row = ear_detect_row.clone();
            ear_switch.connect_state_set(move |sw, active| {
                let payload = vec![if active { 0x01 } else { 0x00 }];
                let cc = libspacepods::ipc::ServiceCommand::Custom {
                    command_id: 0x26, // CMD_IN_EAR_DETECT
                    payload,
                };
                set_busy(&[&ear_detect_row], true);
                let sw = sw.clone();
                let ctx = ctx.clone();
                let ear_detect_row = ear_detect_row.clone();
                glib::spawn_future_local(async move {
                    let result = match libspacepods::client::SpacePodsClient::connect(None).await {
                        Ok(mut client) => client.send_command_raw(cc).await,
                        Err(e) => Err(e),
                    };
                    set_busy(&[&ear_detect_row], false);
                    if let Err(e) = result {
                        ctx.error(format!("Couldn't change In-Ear Detection: {}", e));
                        sw.set_state(!active);
                    }
                });
                glib::Propagation::Proceed
            });
        }
        
        let content = Box::new(Orientation::Vertical, 12);
        content.set_margin_top(16);
        content.set_margin_bottom(32);
        content.set_margin_start(16);
        content.set_margin_end(16);
        content.append(&status_page);
        content.append(&volume_group);
        content.append(&tone_group);
        content.append(&detect_group);
        content.append(&voice_group);

        let clamp = Clamp::new();
        clamp.set_maximum_size(600);
        clamp.set_child(Some(&content));

        clamp.upcast()
    }
}