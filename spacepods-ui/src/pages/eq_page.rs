use glib::clone;
use gtk4::prelude::*;
use gtk4::{
    Box, DrawingArea, GestureClick, Label, Orientation, Scale, ScrolledWindow, ToggleButton, Button, Entry
};
use libadwaita::prelude::*;
use libadwaita::{Clamp, EntryRow, PreferencesGroup};
use serde::{Deserialize, Serialize};
use std::cell::{Cell, RefCell};
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use tokio::sync::mpsc;
use crate::storage::update_settings;

const BUILTIN_PRESETS: [(u8, &str, [i8; 7]); 6] = [
    (0, "Flat",         [0,  0,  0,  0,  0,  0,  0]),
    (1, "Bass Boost",   [6,  4,  1,  0,  0,  1,  2]),
    (2, "Rock",         [4,  3, -1, -1,  2,  4,  5]),
    (3, "Jazz",         [2,  2,  1,  1, -1,  2,  4]),
    (4, "Vocal",        [-2,-1,  0,  4,  3,  1,  1]),
    (5, "Treble Boost", [-2,-1,  0,  1,  3,  5,  7]),
];

const BAND_LABELS: [&str; 7] = ["50Hz", "100Hz", "400Hz", "1kHz", "2.5kHz", "6.3kHz", "16kHz"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomPreset {
    pub name: String,
    pub gains: [i8; 7],
}

fn presets_path() -> PathBuf {
    glib::user_data_dir()
        .join("spacepods")
        .join("custom_presets.json")
}

fn load_custom_presets() -> Vec<CustomPreset> {
    let path = presets_path();
    if !path.exists() {
        return vec![];
    }
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_custom_presets(presets: &[CustomPreset]) {
    let path = presets_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(presets) {
        let _ = fs::write(path, json);
    }
}

pub struct EqPage;

impl EqPage {
    pub fn new(tx: mpsc::Sender<crate::ClientCommand>) -> gtk4::Widget {
        let current_gains: Rc<RefCell<[i8; 7]>> = Rc::new(RefCell::new([0; 7]));
        let custom_presets: Rc<RefCell<Vec<CustomPreset>>> =
            Rc::new(RefCell::new(load_custom_presets()));
        let in_custom_mode = Rc::new(Cell::new(false));

        let clamp = Clamp::new();
        clamp.set_maximum_size(700);
        clamp.set_tightening_threshold(500);

        let root = Box::new(Orientation::Vertical, 20);
        root.set_margin_top(24);
        root.set_margin_bottom(32);
        root.set_margin_start(16);
        root.set_margin_end(16);

        let title = Label::new(Some("Equalizer"));
        title.add_css_class("title-1");
        title.set_halign(gtk4::Align::Start);

        let curve_card = libadwaita::Bin::new();
        curve_card.add_css_class("card");
        curve_card.set_hexpand(true);

        let curve_inner = Box::new(Orientation::Vertical, 4);
        curve_inner.set_margin_top(16);
        curve_inner.set_margin_bottom(8);
        curve_inner.set_margin_start(16);
        curve_inner.set_margin_end(16);

        let drawing = DrawingArea::new();
        drawing.set_content_height(140);
        drawing.set_hexpand(true);

        let band_row = Box::new(Orientation::Horizontal, 0);
        band_row.set_hexpand(true);
        for lbl in BAND_LABELS.iter() {
            let l = Label::new(Some(lbl));
            l.add_css_class("caption");
            l.add_css_class("dim-label");
            l.set_hexpand(true);
            l.set_halign(gtk4::Align::Center);
            band_row.append(&l);
        }

        curve_inner.append(&drawing);
        curve_inner.append(&band_row);
        curve_card.set_child(Some(&curve_inner));
        root.append(&title);
        root.append(&curve_card);

        let sliders_box = Box::new(Orientation::Horizontal, 14);
        sliders_box.set_halign(gtk4::Align::Center);
        sliders_box.set_margin_top(10);

        let mut sliders = Vec::new();
        let is_updating_sliders = Rc::new(Cell::new(false));

        for i in 0..7 {
            let col = Box::new(Orientation::Vertical, 6);
            let val_label = Label::new(Some("0 dB"));
            val_label.add_css_class("caption");
            val_label.add_css_class("numeric");

            let scale = Scale::with_range(Orientation::Vertical, -12.0, 12.0, 1.0);
            scale.set_inverted(true);
            scale.set_vexpand(true);
            scale.set_size_request(-1, 160);

            {
                let tx = tx.clone();
                let current_gains = current_gains.clone();
                let val_lbl = val_label.clone();
                let drawing = drawing.clone();
                let is_updating = is_updating_sliders.clone();
                let in_custom = in_custom_mode.clone();

                scale.connect_value_changed(move |sc| {
                    let v = sc.value() as i8;
                    val_lbl.set_text(&format!("{:+} dB", v));
                    if !is_updating.get() {
                        in_custom.set(true); // User moving manual sliders sets page to custom mode
                        current_gains.borrow_mut()[i] = v;
                        drawing.queue_draw();

                        let tx = tx.clone();
                        let gains_payload = *current_gains.borrow();
                        glib::spawn_future_local(async move {
                            let _ = tx.send(crate::ClientCommand::SetCustomEq(gains_payload)).await;
                        });
                    }
                });
            }

            col.append(&val_label);
            col.append(&scale);
            sliders_box.append(&col);
            sliders.push(scale);
        }

        let scroll = ScrolledWindow::new();
        scroll.set_policy(gtk4::PolicyType::Never, gtk4::PolicyType::Automatic);
        scroll.set_vexpand(true);

        // Prebuilt Presets Group
        let presets_group = PreferencesGroup::new();
        presets_group.set_title("Built-in Presets");

        let flowbox = gtk4::FlowBox::new();
        flowbox.set_valign(gtk4::Align::Start);
        flowbox.set_max_children_per_line(3);
        flowbox.set_min_children_per_line(2);
        flowbox.set_selection_mode(gtk4::SelectionMode::None);
        flowbox.set_column_spacing(10);
        flowbox.set_row_spacing(10);

        let preset_buttons: Rc<RefCell<Vec<ToggleButton>>> = Rc::new(RefCell::new(Vec::new()));

        for &(id, name, gains) in BUILTIN_PRESETS.iter() {
            let btn = ToggleButton::with_label(name);
            btn.set_hexpand(true);
            if id == 0 {
                btn.set_active(true);
            }

            {
                let tx = tx.clone();
                let current_gains = current_gains.clone();
                let drawing = drawing.clone();
                let sliders = sliders.clone();
                let is_updating = is_updating_sliders.clone();
                let p_btns = preset_buttons.clone();
                let in_custom = in_custom_mode.clone();

                btn.connect_toggled(move |this_btn| {
                    if this_btn.is_active() {
                        in_custom.set(false);
                        for b in p_btns.borrow().iter() {
                            if b != this_btn { b.set_active(false); }
                        }

                        current_gains.borrow_mut().copy_from_slice(&gains);
                        drawing.queue_draw();

                        is_updating.set(true);
                        for (idx, slider) in sliders.iter().enumerate() {
                            slider.set_value(gains[idx] as f64);
                        }
                        is_updating.set(false);

                        let tx = tx.clone();
                        glib::spawn_future_local(async move {
                            let _ = tx.send(crate::ClientCommand::SetEqPreset(id)).await;
                        });
                    }
                });
            }

            flowbox.append(&btn);
            preset_buttons.borrow_mut().push(btn);
        }
        presets_group.add(&flowbox);

        // Custom Presets Management Group
        let custom_group = PreferencesGroup::new();
        custom_group.set_title("Custom Presets");

        let save_row = EntryRow::new();
        save_row.set_title("Preset Name");

        let save_btn = Button::with_label("Save Profile");
        save_btn.add_css_class("suggested-action");
        save_row.add_suffix(&save_btn);
        custom_group.add(&save_row);

        let custom_flowbox = gtk4::FlowBox::new();
        custom_flowbox.set_valign(gtk4::Align::Start);
        custom_flowbox.set_max_children_per_line(3);
        custom_flowbox.set_min_children_per_line(2);
        custom_flowbox.set_selection_mode(gtk4::SelectionMode::None);
        custom_flowbox.set_column_spacing(10);
        custom_flowbox.set_row_spacing(10);
        custom_group.add(&custom_flowbox);
        let rebuild_custom_ui = {
            let custom_presets = custom_presets.clone();
            let custom_flowbox = custom_flowbox.clone();
            let current_gains = current_gains.clone();
            let drawing = drawing.clone();
            let sliders = sliders.clone();
            let is_updating_sliders = is_updating_sliders.clone();
            let preset_buttons = preset_buttons.clone();
            let tx = tx.clone();
            let save_row = save_row.clone();

            move || {
                while let Some(child) = custom_flowbox.first_child() {
                    custom_flowbox.remove(&child);
                }

                let profiles = custom_presets.borrow().clone();
                for profile in profiles {
                    let item_box = Box::new(Orientation::Horizontal, 4);
                    item_box.add_css_class("linked");
                    item_box.set_hexpand(true);

                    let p_btn = ToggleButton::with_label(&profile.name);
                    p_btn.set_hexpand(true);

                    {
                        let tx = tx.clone();
                        let current_gains = current_gains.clone();
                        let drawing = drawing.clone();
                        let sliders = sliders.clone();
                        let is_updating = is_updating_sliders.clone();
                        let builtins = preset_buttons.clone();
                        let c_flow = custom_flowbox.clone();

                        p_btn.connect_toggled(move |this_btn| {
                            if this_btn.is_active() {
                                // Turn off prebuilts
                                for b in builtins.borrow().iter() {
                                    b.set_active(false);
                                }
                                // Turn off other custom buttons
                                let mut next = c_flow.first_child();
                                while let Some(ref child) = next {
                                    if let Some(fb_child) = child.downcast_ref::<gtk4::FlowBoxChild>() {
                                        if let Some(row_box) = fb_child.child().and_then(|c| c.downcast::<Box>().ok()) {
                                            if let Some(tb) = row_box.first_child().and_then(|c| c.downcast::<ToggleButton>().ok()) {
                                                if &tb != this_btn {
                                                    tb.set_active(false);
                                                }
                                            }
                                        }
                                    }
                                    next = child.next_sibling();
                                }

                                current_gains.borrow_mut().copy_from_slice(&profile.gains);
                                drawing.queue_draw();

                                is_updating.set(true);
                                for (idx, slider) in sliders.iter().enumerate() {
                                    slider.set_value(profile.gains[idx] as f64);
                                }
                                is_updating.set(false);

                                let tx = tx.clone();
                                let gains_payload = profile.gains;
                                glib::spawn_future_local(async move {
                                    let _ = tx.send(crate::ClientCommand::SetCustomEq(gains_payload)).await;
                                });
                            }
                        });
                    }
                    item_box.append(&p_btn);

                    let edit_btn = Button::from_icon_name("document-edit-symbolic");
                    edit_btn.set_tooltip_text(Some("Overwrite this preset with current sliders"));
                    {
                        let custom_presets = custom_presets.clone();
                        let current_gains = current_gains.clone();
                        let profile_name = profile.name.clone();
                        let tx = tx.clone();

                        edit_btn.connect_clicked(move |_| {
                            let mut profiles = custom_presets.borrow_mut();
                            if let Some(target) = profiles.iter_mut().find(|p| p.name == profile_name) {
                                let new_gains = *current_gains.borrow();
                                target.gains = new_gains;
                                save_custom_presets(&profiles);

                                // Push the fresh modification live to the hardware daemon instantly
                                let tx = tx.clone();
                                glib::spawn_future_local(async move {
                                    let _ = tx.send(crate::ClientCommand::SetCustomEq(new_gains)).await;
                                });
                            }
                        });
                    }
                    item_box.append(&edit_btn);

                    let delete_btn = Button::from_icon_name("user-trash-symbolic");
                    delete_btn.add_css_class("destructive-action");
                    delete_btn.set_tooltip_text(Some("Delete this preset"));
                    {
                        let custom_presets = custom_presets.clone();
                        let profile_name = profile.name.clone();
                        let tx = tx.clone();
                        let custom_flowbox = custom_flowbox.clone();
                        let item_box = item_box.clone();

                        delete_btn.connect_clicked(move |_| {
                            let mut profiles = custom_presets.borrow_mut();
                            profiles.retain(|p| p.name != profile_name);
                            save_custom_presets(&profiles);

                            custom_flowbox.remove(&item_box);

                            let tx = tx.clone();
                            glib::spawn_future_local(async move {
                                let _ = tx.send(crate::ClientCommand::SetEqPreset(0)).await;
                            });
                        });
                    }
                    item_box.append(&delete_btn);

                    custom_flowbox.append(&item_box);
                }
            }
        };

        {
            let save_row = save_row.clone();
            let custom_presets = custom_presets.clone();
            let current_gains = current_gains.clone();
            let rebuild_ui = rebuild_custom_ui.clone();

            save_btn.connect_clicked(move |_| {
                let text = save_row.text().to_string();
                let name = text.trim();
                if !name.is_empty() {
                    let mut profiles = custom_presets.borrow_mut();
                    profiles.retain(|p| p.name != name); // avoid exact structural duplicate names
                    profiles.push(CustomPreset {
                        name: name.to_string(),
                        gains: *current_gains.borrow(),
                    });
                    save_custom_presets(&profiles);
                    save_row.set_text("");

                    drop(profiles);
                    rebuild_ui();
                }
            });
        }

        // Initial populate loop call for stored profiles
        rebuild_custom_ui();

        let config_wrapper = Box::new(Orientation::Vertical, 16);
        config_wrapper.append(&sliders_box);
        config_wrapper.append(&presets_group);
        config_wrapper.append(&custom_group);

        scroll.set_child(Some(&config_wrapper));
        root.append(&scroll);

        // Advanced Bezier Spline Cairo Curve Rendering mapping
        {
            let current_gains = current_gains.clone();
            drawing.set_draw_func(move |_, cr, w, h| {
                let gains = current_gains.borrow();
                let w_f = w as f64;
                let h_f = h as f64;

                cr.set_source_rgba(0.1, 0.1, 0.1, 0.03);
                cr.paint().unwrap();

                cr.set_source_rgba(0.0, 0.0, 0.0, 0.1);
                cr.set_line_width(1.0);
                cr.move_to(0.0, h_f / 2.0);
                cr.line_to(w_f, h_f / 2.0);
                cr.stroke().unwrap();

                let n = gains.len();
                let mut pts = Vec::with_capacity(n);
                for idx in 0..n {
                    let cx = (idx as f64 / (n - 1) as f64) * (w_f - 40.0) + 20.0;
                    let cy = (h_f / 2.0) - (gains[idx] as f64 / 12.0) * (h_f / 2.0 - 15.0);
                    pts.push((cx, cy));
                }

                if !pts.is_empty() {
                    cr.move_to(pts[0].0, pts[0].1);
                    for i in 1..pts.len() {
                        let cpx = (pts[i-1].0 + pts[i].0) / 2.0;
                        cr.curve_to(cpx, pts[i-1].1, cpx, pts[i].1, pts[i].0, pts[i].1);
                    }
                    cr.set_source_rgba(0.2, 0.5, 0.9, 0.9);
                    cr.set_line_width(2.5);
                    cr.stroke().unwrap();

                    for &(x, y) in &pts {
                        cr.arc(x, y, 4.0, 0.0, std::f64::consts::TAU);
                        cr.set_source_rgba(0.2, 0.5, 0.9, 1.0);
                        cr.fill().unwrap();
                        cr.arc(x, y, 4.0, 0.0, std::f64::consts::TAU);
                        cr.set_source_rgba(1.0, 1.0, 1.0, 0.9);
                        cr.set_line_width(1.5);
                        cr.stroke().unwrap();
                    }
                }
            });
        }

        clamp.set_child(Some(&root));
        clamp.upcast::<gtk4::Widget>()
    }
}