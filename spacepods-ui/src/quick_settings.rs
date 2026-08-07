//! Quick-settings popover panel shown from the header bar.
//!
//! Provides an at-a-glance control surface for the connected earbuds:
//! battery rings, ANC / noise-control segmented control, an ambient-level
//! slider and a conversation-awareness toggle. All reads/writes go through
//! the daemon IPC (`SpacePodsClient`).

use std::cell::Cell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Box, Button, Label, Orientation, Scale, DrawingArea, ToggleButton};
use gtk4::cairo;

use crate::context::AppContext;
use crate::log::Log;

/// Build the quick-settings popover. `ctx` is used to reach the daemon and to
/// label the connected device from its profile.
pub fn build(ctx: &Rc<AppContext>) -> gtk4::Popover {
    let panel = Box::new(Orientation::Vertical, 0);
    panel.set_size_request(300, -1);

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

    let gear_btn = Button::from_icon_name("emblem-system-symbolic");
    gear_btn.add_css_class("flat");
    gear_btn.add_css_class("circular");
    gear_btn.set_tooltip_text(Some("Settings"));

    header.append(&title);
    header.append(&gear_btn);

    // ── Battery row ──
    let battery_row = Box::new(Orientation::Horizontal, 12);
    battery_row.set_halign(gtk4::Align::Center);
    battery_row.set_margin_bottom(12);

    let left_ring = BatteryRing::new('L');
    let right_ring = BatteryRing::new('R');
    let left_widget = left_ring.widget();
    let right_widget = right_ring.widget();
    battery_row.append(&left_widget);
    battery_row.append(&right_widget);

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
    level.set_width_request(190);
    level.set_draw_value(false);

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
    panel.append(&battery_row);
    panel.append(&noise_block);
    panel.append(&level_block);
    panel.append(&chat_block);

    // Wire to the daemon.
    wiring(
        &left_ring, &right_ring,
        &off_btn, &trans_btn, &anc_btn,
        &level, &minus_btn, &plus_btn,
        &chat_on, &chat_off,
        ctx,
    );

    gtk4::Popover::builder()
        .child(&panel)
        .build()
}

fn wiring(
    left_ring: &BatteryRing,
    right_ring: &BatteryRing,
    off_btn: &ToggleButton,
    trans_btn: &ToggleButton,
    anc_btn: &ToggleButton,
    level: &Scale,
    minus_btn: &Button,
    plus_btn: &Button,
    chat_on: &ToggleButton,
    chat_off: &ToggleButton,
    ctx: &Rc<AppContext>,
) {
    let ctx = ctx.clone();
    let left_ring = left_ring.clone();
    let right_ring = right_ring.clone();
    let off_btn = off_btn.clone();
    let trans_btn = trans_btn.clone();
    let anc_btn = anc_btn.clone();
    let level = level.clone();
    let minus_btn = minus_btn.clone();
    let plus_btn = plus_btn.clone();
    let chat_on = chat_on.clone();
    let chat_off = chat_off.clone();

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
                        left_ring.set_percent(s.battery.left);
                        right_ring.set_percent(s.battery.right);
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
                            "batt={:?}/{:?} mode={} level={} adaptive={:?}",
                            s.battery.left, s.battery.right, s.anc.mode as u8,
                            s.anc.level, s.features.adaptive_anc
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
                Ok(_) => ctx.success(if on { "Conversation awareness on" } else { "Conversation awareness off" }),
                Err(e) => ctx.error(format!("Conversation: {}", e)),
            },
            Err(e) => ctx.daemon_unreachable(e),
        }
    });
}

// ───────────────────────────────────────────────────────────────────────────
// Battery ring
// ───────────────────────────────────────────────────────────────────────────

/// A circular percentage ring drawn with cairo, with a letter badge (L/R)
/// centred inside.
#[derive(Clone)]
pub struct BatteryRing {
    area: DrawingArea,
    percent: Rc<Cell<f64>>,
}

impl BatteryRing {
    pub fn new(badge: char) -> Self {
        let percent: Rc<Cell<f64>> = Rc::new(Cell::new(0.0));
        let area = DrawingArea::new();
        area.set_size_request(64, 64);

        let p = percent.clone();
        area.set_draw_func(move |_, cr, w, h| {
            let pct = p.get();
            let cx = w as f64 / 2.0;
            let cy = h as f64 / 2.0;
            let radius = (w.min(h) as f64 / 2.0) - 6.0;

            // Track
            cr.set_source_rgba(0.25, 0.25, 0.27, 1.0);
            cr.set_line_width(5.0);
            cr.set_line_cap(cairo::LineCap::Round);
            cr.arc(cx, cy, radius, 0.0, std::f64::consts::TAU);
            let _ = cr.stroke();

            // Filled arc (green)
            let frac = (pct / 100.0).clamp(0.0, 1.0);
            if frac > 0.001 {
                let start = -std::f64::consts::FRAC_PI_2;
                let end = start + frac * std::f64::consts::TAU;
                cr.set_source_rgba(0.35, 0.9, 0.45, 1.0);
                cr.arc(cx, cy, radius, start, end);
                let _ = cr.stroke();
            }

            // Badge + percentage
            cr.set_source_rgba(1.0, 1.0, 1.0, 0.85);
            cr.select_font_face("sans-serif", cairo::FontSlant::Normal, cairo::FontWeight::Bold);
            cr.set_font_size((radius * 0.32).max(8.0));
            let txt = format!("{}  {}", badge, pct.round() as u64);
            if let Ok(extents) = cr.text_extents(&txt) {
                cr.move_to(cx - extents.width() / 2.0, cy + extents.height() / 2.0);
                let _ = cr.show_text(&txt);
            }
        });

        Self { area, percent }
    }

    pub fn widget(&self) -> DrawingArea {
        self.area.clone()
    }

    pub fn set_percent(&self, value: Option<u8>) {
        self.percent.set(value.unwrap_or(0) as f64);
        self.area.queue_draw();
    }
}
