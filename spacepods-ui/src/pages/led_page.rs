use gtk4::prelude::*;
use gtk4::{Box, Label, Orientation, DropDown, Switch, Scale};
use libadwaita::prelude::*;
use libadwaita::{PreferencesGroup, ActionRow, Clamp};

pub struct LedPage;

impl LedPage {
    pub fn new() -> gtk4::Widget {
        // ── Header with refresh ──
        let header_row = Box::new(Orientation::Horizontal, 0);
        let title = Label::new(Some("LED Control"));
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

        // ── LED Mode ──
        let mode_group = PreferencesGroup::new();
        mode_group.set_title("LED Mode");

        let led_switch_row = ActionRow::new();
        led_switch_row.set_title("LED Indicator");
        led_switch_row.set_subtitle("Show LED lights on the earbuds");
        let led_switch = Switch::new();
        led_switch.set_valign(gtk4::Align::Center);
        led_switch.set_active(true);
        led_switch_row.add_suffix(&led_switch);
        led_switch_row.set_activatable_widget(Some(&led_switch));
        mode_group.add(&led_switch_row);

        // ── LED Mode selection ──
        let mode_row = ActionRow::new();
        mode_row.set_title("Lighting Mode");
        mode_row.set_subtitle("Choose the LED pattern");
        let mode_dropdown = DropDown::from_strings(&[
            "Always On",
            "Breathing",
            "Flash",
            "Gradual",
            "Music Sync",
            "Off",
        ]);
        mode_dropdown.add_css_class("flat");
        mode_row.add_suffix(&mode_dropdown);
        mode_group.add(&mode_row);

        // ── LED Color ──
        let color_group = PreferencesGroup::new();
        color_group.set_title("Color");

        let color_row = ActionRow::new();
        color_row.set_title("LED Color");
        color_row.set_subtitle("Choose from preset colors or custom");
        let color_dropdown = DropDown::from_strings(&[
            "White",
            "Red",
            "Blue",
            "Green",
            "Purple",
            "Cyan",
            "Orange",
            "Rainbow Cycle",
        ]);
        color_dropdown.add_css_class("flat");
        color_row.add_suffix(&color_dropdown);
        color_group.add(&color_row);

        // ── Brightness ──
        let brightness_scale = Scale::with_range(Orientation::Horizontal, 0.0, 100.0, 5.0);
        brightness_scale.set_hexpand(true);
        brightness_scale.set_draw_value(true);
        brightness_scale.set_value(70.0);
        brightness_scale.set_margin_start(16);
        brightness_scale.set_margin_end(16);
        brightness_scale.set_margin_top(8);
        brightness_scale.set_margin_bottom(8);

        // ── Speed ──
        let speed_scale = Scale::with_range(Orientation::Horizontal, 1.0, 10.0, 1.0);
        speed_scale.set_hexpand(true);
        speed_scale.set_draw_value(true);
        speed_scale.set_value(5.0);
        speed_scale.set_margin_start(16);
        speed_scale.set_margin_end(16);
        speed_scale.set_margin_top(8);
        speed_scale.set_margin_bottom(8);

        // ── Preview note ──
        let preview_label = Label::new(Some("Note: LED controls may not be supported on all devices."));
        preview_label.add_css_class("dim-label");
        preview_label.add_css_class("caption");
        preview_label.set_halign(gtk4::Align::Center);
        preview_label.set_margin_top(8);

        // ── Layout ──
        let content = Box::new(Orientation::Vertical, 12);
        content.set_margin_top(16);
        content.set_margin_bottom(32);
        content.set_margin_start(16);
        content.set_margin_end(16);
        content.append(&header_row);
        content.append(&mode_group);
        content.append(&color_group);
        content.append(&brightness_scale);
        content.append(&speed_scale);
        content.append(&preview_label);

        let clamp = Clamp::new();
        clamp.set_maximum_size(600);
        clamp.set_child(Some(&content));

        // ── Wire up commands ──
        {
            let led_switch = led_switch.clone();
            glib::spawn_future_local(async move {
                // Send LED on/off via daemon
                let _ = led_switch.connect_state_set(move |_, active| {
                    let payload = if active { vec![0x01] } else { vec![0x00] };
                    let cc = libspacepods::ipc::ServiceCommand::Custom {
                        command_id: 0x2E, // CMD_LED_MODE
                        payload,
                    };
                    glib::spawn_future_local(async move {
                        if let Ok(mut client) = libspacepods::client::SpacePodsClient::connect(None).await {
                            let _ = client.send_command_raw(cc).await;
                        }
                    });
                    glib::Propagation::Proceed
                });
            });
        }

        clamp.upcast()
    }
}
