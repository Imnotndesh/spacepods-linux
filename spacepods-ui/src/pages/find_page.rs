use std::rc::Rc;
use gtk4::prelude::*;
use gtk4::{Box, Label, Orientation, Button, CheckButton, Image};
use libadwaita::prelude::*;
use libadwaita::{HeaderBar, ToolbarView, StatusPage, Clamp, WindowTitle};

use crate::context::AppContext;

pub struct FindPage;

impl FindPage {
    pub fn new(ctx: Rc<AppContext>) -> ToolbarView {
        let header = HeaderBar::new();
        let title_widget = WindowTitle::new("Find My Earbuds", "Locate your lost earbuds");
        header.set_title_widget(Some(&title_widget));

        let status_page = StatusPage::new();
        status_page.set_icon_name(Some("find-location-symbolic"));
        status_page.set_title("Find Your Earbuds");
        status_page.set_description(Some(
            "Select which earbud to ping. The selected earbud will play a loud beep."
        ));
        status_page.set_vexpand(true);

        // ── Ear selection ──
        let left_check = CheckButton::with_label("Left Earbud");
        left_check.set_halign(gtk4::Align::Center);
        left_check.set_margin_top(8);

        let right_check = CheckButton::with_label("Right Earbud");
        right_check.set_halign(gtk4::Align::Center);

        let both_hint = Label::new(Some("Select both to play stereo beep"));
        both_hint.add_css_class("dim-label");
        both_hint.add_css_class("caption");
        both_hint.set_halign(gtk4::Align::Center);

        // ── Ear icons ──
        let left_icon = Image::from_icon_name("audio-headset-left-symbolic");
        left_icon.set_pixel_size(64);
        left_icon.set_halign(gtk4::Align::Center);
        left_icon.set_opacity(0.3);
        left_icon.set_margin_top(24);

        let right_icon = Image::from_icon_name("audio-headset-right-symbolic");
        right_icon.set_pixel_size(64);
        right_icon.set_halign(gtk4::Align::Center);
        right_icon.set_opacity(0.3);

        // Animate opacity when checked
        {
            let icon = left_icon.clone();
            left_check.connect_toggled(move |cb| {
                icon.set_opacity(if cb.is_active() { 1.0 } else { 0.3 });
            });
        }
        {
            let icon = right_icon.clone();
            right_check.connect_toggled(move |cb| {
                icon.set_opacity(if cb.is_active() { 1.0 } else { 0.3 });
            });
        }

        // ── Start/Stop button ──
        let find_btn = Button::with_label("Start Finding");
        find_btn.add_css_class("suggested-action");
        find_btn.add_css_class("pill");
        find_btn.set_halign(gtk4::Align::Center);
        find_btn.set_margin_top(16);

        let status_label = Label::new(None);
        status_label.add_css_class("dim-label");
        status_label.set_halign(gtk4::Align::Center);

        // Track finding state
        let finding = std::rc::Rc::new(std::cell::Cell::new(false));

        {
            let finding = finding.clone();
            let find_btn = find_btn.clone();
            let left_check = left_check.clone();
            let right_check = right_check.clone();
            let status_label = status_label.clone();
            let ctx = ctx.clone();

            find_btn.connect_clicked(move |btn| {
                let is_active = finding.get();
                if is_active {
                    // Stop finding
                    finding.set(false);
                    btn.set_label("Start Finding");
                    btn.remove_css_class("destructive-action");
                    btn.add_css_class("suggested-action");
                    status_label.set_text("");

                    // Send stop command to daemon
                    let ctx = ctx.clone();
                    glib::spawn_future_local(async move {
                        let payload = vec![0x00]; // disable
                        let result = match libspacepods::client::SpacePodsClient::connect(None).await {
                            Ok(mut client) => client.send_command_raw(
                                libspacepods::ipc::ServiceCommand::Custom {
                                    command_id: 0x2A, // CMD_FIND_DEVICE
                                    payload,
                                }
                            ).await,
                            Err(e) => Err(e),
                        };
                        if let Err(e) = result {
                            ctx.error(format!("Couldn't stop Find My Earbuds: {}", e));
                        }
                    });
                } else if left_check.is_active() || right_check.is_active() {
                    // Start finding
                    finding.set(true);
                    btn.set_label("Stop Finding");
                    btn.remove_css_class("suggested-action");
                    btn.add_css_class("destructive-action");

                    let left = left_check.is_active();
                    let right = right_check.is_active();
                    if left && right {
                        status_label.set_text("Playing stereo beep on both earbuds...");
                    } else if left {
                        status_label.set_text("Playing beep on LEFT earbud...");
                    } else {
                        status_label.set_text("Playing beep on RIGHT earbud...");
                    }

                    let ctx = ctx.clone();
                    let finding = finding.clone();
                    let btn_ref = btn.clone();
                    let status_label_ref = status_label.clone();
                    glib::spawn_future_local(async move {
                        let payload = vec![0x01]; // enable
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
                            ctx.error(format!("Couldn't start Find My Earbuds: {}", e));
                            // Roll the button back — the earbuds never got the ping.
                            finding.set(false);
                            btn_ref.set_label("Start Finding");
                            btn_ref.remove_css_class("destructive-action");
                            btn_ref.add_css_class("suggested-action");
                            status_label_ref.set_text("");
                        }
                    });
                } else {
                    status_label.set_text("Please select at least one earbud");
                }
            });
        }

        // ── Layout ──
        let vbox = Box::new(Orientation::Vertical, 12);
        vbox.set_halign(gtk4::Align::Center);
        vbox.set_valign(gtk4::Align::Center);
        vbox.set_margin_top(24);
        vbox.set_margin_bottom(32);
        vbox.set_margin_start(16);
        vbox.set_margin_end(16);
        vbox.append(&status_page);
        vbox.append(&left_icon);
        vbox.append(&left_check);
        vbox.append(&right_icon);
        vbox.append(&right_check);
        vbox.append(&both_hint);
        vbox.append(&find_btn);
        vbox.append(&status_label);

        let clamp = Clamp::new();
        clamp.set_maximum_size(400);
        clamp.set_child(Some(&vbox));

        let toolbar_view = ToolbarView::new();
        toolbar_view.add_top_bar(&header);
        toolbar_view.set_content(Some(&clamp));

        toolbar_view
    }
}