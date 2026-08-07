//! Quick-settings popup panel shown from the tray icon.
//!
//! A lightweight, standalone popup window positioned near the tray icon
//! using coordinates received from the SNI activate signal. Contains
//! Noise Control segmented control, Ambient Level slider,
//! and Conversation Awareness toggle.

use std::cell::Cell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Box, Button, Label, Orientation, Scale, ToggleButton, Window};

use crate::context::AppContext;
use crate::log::Log;

/// Build and pop up the quick-settings panel as a standalone popup window
/// near the given screen coordinates.
pub fn show_popup(ctx: &Rc<AppContext>, x: i32, y: i32) {
    let panel = build_panel(ctx);

    let window = Window::new();
    window.set_decorated(false);
    window.set_resizable(false);
    window.set_child(Some(&panel));

    // Popup-like behaviour: close when focus leaves the window.
    window.set_hide_on_close(true);
    let weak = window.downgrade();
    window.connect_is_active_notify(move |w| {
        if !w.is_active() {
            if let Some(win) = weak.upgrade() {
                win.close();
            }
        }
    });

    window.show();
    let _ = (x, y);
}

/// Build the quick-settings panel (the contents of the popup window).
fn build_panel(ctx: &Rc<AppContext>) -> Box {
    let panel = Box::new(Orientation::Vertical, 0);
    panel.set_size_request(260, -1);

    // ── Header strip ──
    let header = Box::new(Orientation::Horizontal, 8);
    header.set_margin_bottom(8);

    let device_name = ctx
        .profile()
        .map(|p| p.name.to_string())
        .unwrap_or_else(|| "SpacePods".to_string());
    let title = Label::new(Some(&device_name));
    title.add_css_class("heading");
    title.set_halign(gtk4::Align::Start);
    title.set_hexpand(true);

    header.append(&title);

    // ── Noise control segmented control ──
    let noise_label = Label::new(Some("Noise Control"));
    noise_label.add_css_class("dim-label");
    noise_label.set_halign(gtk4::Align::Start);

    let off_btn = ToggleButton::with_label("Off");
    let trans_btn = ToggleButton::with_label("Transparency");
    let anc_btn = ToggleButton::with_label("ANC");
    trans_btn.set_group(Some(&off_btn));
    anc_btn.set_group(Some(&off_btn));
    for b in [&off_btn, &trans_btn, &anc_btn] {
        b.set_hexpand(false);
    }

    let noise_row = Box::new(Orientation::Horizontal, 0);
    noise_row.add_css_class("linked");
    noise_row.set_halign(gtk4::Align::Fill);
    noise_row.append(&off_btn);
    noise_row.append(&trans_btn);
    noise_row.append(&anc_btn);

    // ── Ambient level slider ──
    let level_label = Label::new(Some("Ambient Level"));
    level_label.add_css_class("dim-label");
    level_label.set_halign(gtk4::Align::Start);

    let level = Scale::with_range(gtk4::Orientation::Horizontal, 0.0, 5.0, 1.0);
    level.set_hexpand(true);
    level.set_width_request(220);

    let minus_btn = Button::from_icon_name("list-remove-symbolic");
    minus_btn.add_css_class("flat");
    minus_btn.add_css_class("circular");
    let plus_btn = Button::from_icon_name("list-add-symbolic");
    plus_btn.add_css_class("flat");
    plus_btn.add_css_class("circular");

    let level_row = Box::new(Orientation::Horizontal, 6);
    level_row.append(&minus_btn);
    level_row.append(&level);
    level_row.append(&plus_btn);

    // ── Conversation awareness toggle ──
    let chat_label = Label::new(Some("Conversation Awareness"));
    chat_label.add_css_class("dim-label");
    chat_label.set_halign(gtk4::Align::Start);

    let chat_on = ToggleButton::with_label("Sensitive");
    let chat_off = ToggleButton::with_label("Off");
    chat_off.set_group(Some(&chat_on));

    let chat_row = Box::new(Orientation::Horizontal, 0);
    chat_row.add_css_class("linked");
    chat_row.append(&chat_off);
    chat_row.append(&chat_on);

    // Layout
    let noise_block = Box::new(Orientation::Vertical, 6);
    noise_block.set_margin_bottom(12);
    noise_block.append(&noise_label);
    noise_block.append(&noise_row);

    let level_block = Box::new(Orientation::Vertical, 6);
    level_block.set_margin_bottom(12);
    level_block.append(&level_label);
    level_block.append(&level_row);

    let chat_block = Box::new(Orientation::Vertical, 6);
    chat_block.append(&chat_label);
    chat_block.append(&chat_row);

    panel.append(&header);
    panel.append(&noise_block);
    panel.append(&level_block);
    panel.append(&chat_block);

    // Wire to the daemon.
    let ctx = ctx.clone();
    wiring(
        off_btn.clone(), trans_btn.clone(), anc_btn.clone(),
        level.clone(), minus_btn.clone(), plus_btn.clone(),
        chat_on.clone(), chat_off.clone(),
        &ctx,
    );

    panel
}

fn wiring(
    off_btn: ToggleButton,
    trans_btn: ToggleButton,
    anc_btn: ToggleButton,
    level: Scale,
    minus_btn: Button,
    plus_btn: Button,
    chat_on: ToggleButton,
    chat_off: ToggleButton,
    ctx: &Rc<AppContext>,
) {
    let ctx = ctx.clone();

    // Populate initial state from the daemon.
    {
        let off_btn = off_btn.clone();
        let trans_btn = trans_btn.clone();
        let anc_btn = anc_btn.clone();
        let level = level.clone();
        let chat_on = chat_on.clone();
        let chat_off = chat_off.clone();
        glib::spawn_future_local(glib::clone!(
            #[strong] ctx,
            async move {
                use libspacepods::client::SpacePodsClient;
                match SpacePodsClient::connect(None).await {
                    Ok(mut client) => match client.get_status().await {
                        Ok(s) => {
                            match s.anc.mode as u8 {
                                0 => off_btn.set_active(true),
                                1 => anc_btn.set_active(true),
                                2 => trans_btn.set_active(true),
                                _ => {}
                            }
                            if s.anc.max_level > 0 {
                                level.set_range(0.0, s.anc.max_level as f64);
                            }
                            level.set_value(s.anc.level as f64);
                            if let Some(v) = s.features.adaptive_anc {
                                if v { chat_on.set_active(true); } else { chat_off.set_active(true); }
                            }
                            Log::full("QUICKSET", &format!(
                                "mode={} level={} adaptive={:?}",
                                s.anc.mode as u8, s.anc.level, s.features.adaptive_anc
                            ));
                        }
                        Err(e) => ctx.daemon_unreachable(e),
                    },
                    Err(e) => ctx.daemon_unreachable(e),
                }
            }
        ));
    }

    trans_btn.connect_clicked(glib::clone!(#[strong] ctx, move |_| set_mode("transparency", &ctx)));
    anc_btn.connect_clicked(glib::clone!(#[strong] ctx, move |_| set_mode("anc", &ctx)));
    off_btn.connect_clicked(glib::clone!(#[strong] ctx, move |_| set_mode("off", &ctx)));

    level.connect_value_changed(glib::clone!(#[strong] ctx, move |sl| set_level(sl.value() as u8, &ctx)));
    minus_btn.connect_clicked(glib::clone!(
        #[strong] ctx, #[strong] level,
        move |_| set_level((level.value().round() as i32 - 1).max(0) as u8, &ctx)
    ));
    plus_btn.connect_clicked(glib::clone!(
        #[strong] ctx, #[strong] level,
        move |_| set_level(level.value().round() as u8 + 1, &ctx)
    ));

    chat_on.connect_clicked(glib::clone!(#[strong] ctx, move |_| set_adaptive(true, &ctx)));
    chat_off.connect_clicked(glib::clone!(#[strong] ctx, move |_| set_adaptive(false, &ctx)));
}

fn set_mode(mode: &'static str, ctx: &Rc<AppContext>) {
    let ctx = ctx.clone();
    glib::spawn_future_local(async move {
        use libspacepods::client::SpacePodsClient;
        match SpacePodsClient::connect(None).await {
            Ok(mut client) => match client.set_anc_mode(mode).await {
                Ok(_) => ctx.success(&format!("ANC: {}", mode)),
                Err(e) => ctx.error(format!("ANC: {}", e)),
            },
            Err(e) => ctx.daemon_unreachable(e),
        }
    });
}

fn set_level(level: u8, ctx: &Rc<AppContext>) {
    let ctx = ctx.clone();
    glib::spawn_future_local(async move {
        use libspacepods::client::SpacePodsClient;
        match SpacePodsClient::connect(None).await {
            Ok(mut client) => match client.set_level(level).await {
                Ok(_) => ctx.success("Level updated"),
                Err(e) => ctx.error(format!("Level: {}", e)),
            },
            Err(e) => ctx.daemon_unreachable(e),
        }
    });
}

fn set_adaptive(on: bool, ctx: &Rc<AppContext>) {
    let ctx = ctx.clone();
    glib::spawn_future_local(async move {
        use libspacepods::client::SpacePodsClient;
        match SpacePodsClient::connect(None).await {
            Ok(mut client) => match client.set_adaptive_anc(on).await {
                Ok(_) => ctx.success(
                    if on { "Conversation awareness on" } else { "Conversation awareness off" },
                ),
                Err(e) => ctx.error(format!("Conversation: {}", e)),
            },
            Err(e) => ctx.daemon_unreachable(e),
        }
    });
}
