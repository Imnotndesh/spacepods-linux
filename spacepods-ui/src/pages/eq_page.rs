use glib::clone;
use gtk4::prelude::*;
use gtk4::{
    Box, DrawingArea, GestureClick, GestureLongPress, Label, Orientation,
    Scale, ScrolledWindow, ToggleButton,
};
use libadwaita::prelude::*;
use libadwaita::{Clamp, EntryRow, PreferencesGroup};
use serde::{Deserialize, Serialize};
use std::cell::{Cell, RefCell};
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use tokio::sync::Mutex;
use libspacepods::client::SpacePodsClient;
use crate::storage::{update_settings};

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
    pub fn new(client: Arc<Mutex<SpacePodsClient>>) -> gtk4::Widget {
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

        {
            let gains_ref = current_gains.clone();
            drawing.set_draw_func(move |_area, cr, width, height| {
                let gains = gains_ref.borrow();
                draw_eq_curve(cr, width as f64, height as f64, &*gains);
            });
        }

        let builtin_label = Label::new(Some("Presets"));
        builtin_label.add_css_class("heading");
        builtin_label.set_halign(gtk4::Align::Start);

        let flow = make_flowbox();
        let first_toggle: Rc<RefCell<Option<ToggleButton>>> = Rc::new(RefCell::new(None));

        for (id, name, gains) in BUILTIN_PRESETS.iter() {
            let btn = make_preset_button(name, gains, false);

            if first_toggle.borrow().is_none() {
                btn.set_active(true);
                btn.add_css_class("suggested-action");
                *first_toggle.borrow_mut() = Some(btn.clone());
            } else if let Some(ref f) = *first_toggle.borrow() {
                btn.set_group(Some(f));
            }

            let gains_copy = *gains;
            let preset_id = *id;
            let drawing_ref = drawing.clone();
            let gains_ref = current_gains.clone();
            let in_custom_ref = in_custom_mode.clone();
            let client_ref = Arc::clone(&client);  // add this

            btn.connect_toggled(clone!(
                #[weak] drawing_ref,
                move |b| {
                    if b.is_active() {
                        b.add_css_class("suggested-action");
                        in_custom_ref.set(false);
                        *gains_ref.borrow_mut() = gains_copy;
                        drawing_ref.queue_draw();
                        update_settings(|s| s.last_eq_preset = preset_id);
                        let client = Arc::clone(&client_ref);
                        glib::spawn_future_local(async move {
                            let mut c = client.lock().await;
                            if let Err(e) = c.set_eq_preset(preset_id).await {
                                eprintln!("set_eq_preset {}: {}", preset_id, e);
                            }
                        });
                    } else {
                        b.remove_css_class("suggested-action");
                    }
                }
            ));

            let child = gtk4::FlowBoxChild::new();
            child.set_child(Some(&btn));
            child.set_focusable(false);
            flow.append(&child);
        }

        let custom_section_label = Label::new(Some("Custom Presets"));
        custom_section_label.add_css_class("heading");
        custom_section_label.set_halign(gtk4::Align::Start);
        custom_section_label.set_margin_top(8);

        let custom_flow = make_flowbox();

        let sliders_card = libadwaita::Bin::new();
        sliders_card.add_css_class("card");
        sliders_card.set_hexpand(true);
        sliders_card.set_visible(false);

        let sliders_inner = Box::new(Orientation::Vertical, 8);
        sliders_inner.set_margin_top(16);
        sliders_inner.set_margin_bottom(16);
        sliders_inner.set_margin_start(16);
        sliders_inner.set_margin_end(16);

        let sliders_title = Label::new(Some("Adjust Bands"));
        sliders_title.add_css_class("caption-heading");
        sliders_title.set_halign(gtk4::Align::Start);
        sliders_inner.append(&sliders_title);

        let band_sliders: Vec<Scale> = (0..7)
            .map(|i| {
                let row = Box::new(Orientation::Horizontal, 8);
                row.set_hexpand(true);

                let lbl = Label::new(Some(BAND_LABELS[i]));
                lbl.add_css_class("caption");
                lbl.set_width_chars(6);
                lbl.set_halign(gtk4::Align::End);

                let scale = Scale::with_range(Orientation::Horizontal, -12.0, 12.0, 1.0);
                scale.set_hexpand(true);
                scale.set_draw_value(true);
                scale.set_value_pos(gtk4::PositionType::Right);
                scale.add_mark(-12.0, gtk4::PositionType::Bottom, None);
                scale.add_mark(0.0, gtk4::PositionType::Bottom, Some("0"));
                scale.add_mark(12.0, gtk4::PositionType::Bottom, None);
                scale.set_value(0.0);

                row.append(&lbl);
                row.append(&scale);
                sliders_inner.append(&row);
                scale
            })
            .collect();

        for (i, slider) in band_sliders.iter().enumerate() {
            let gains_ref = current_gains.clone();
            let drawing_ref = drawing.clone();
            slider.connect_value_changed(move |s| {
                gains_ref.borrow_mut()[i] = s.value() as i8;
                drawing_ref.queue_draw();
            });
        }

        let save_row = Box::new(Orientation::Horizontal, 8);
        save_row.set_margin_top(8);
        save_row.set_hexpand(true);

        let name_group = PreferencesGroup::new();
        name_group.set_hexpand(true);
        let name_entry = EntryRow::new();
        name_entry.set_title("Preset name");
        name_group.add(&name_entry);

        let save_btn = gtk4::Button::with_label("Save Preset");
        save_btn.add_css_class("suggested-action");
        save_btn.set_valign(gtk4::Align::Center);

        save_row.append(&name_group);
        save_row.append(&save_btn);
        sliders_inner.append(&save_row);
        sliders_card.set_child(Some(&sliders_inner));

        let edit_btn = ToggleButton::with_label("+ New Custom Preset");
        edit_btn.add_css_class("pill");
        edit_btn.set_halign(gtk4::Align::Center);
        if let Some(ref f) = *first_toggle.borrow() {
            edit_btn.set_group(Some(f));
        }

        {
            let sliders_card_ref = sliders_card.clone();
            let in_custom_ref = in_custom_mode.clone();
            let gains_ref = current_gains.clone();
            let drawing_ref = drawing.clone();
            let sliders_copy = band_sliders.clone();

            edit_btn.connect_toggled(clone!(
                #[weak] sliders_card_ref,
                move |b| {
                    if b.is_active() {
                        b.add_css_class("suggested-action");
                        in_custom_ref.set(true);
                        let g = *gains_ref.borrow();
                        for (i, s) in sliders_copy.iter().enumerate() {
                            s.set_value(g[i] as f64);
                        }
                        sliders_card_ref.set_visible(true);
                        drawing_ref.queue_draw();
                        // When entering custom mode, set last_eq_preset to 6 (custom)
                        update_settings(|s| s.last_eq_preset = 6);
                    } else {
                        b.remove_css_class("suggested-action");
                        sliders_card_ref.set_visible(false);
                    }
                }
            ));
        }

        {
            let presets_ref = custom_presets.clone();
            let gains_ref = current_gains.clone();
            let name_entry_ref = name_entry.clone();
            let custom_flow_ref = custom_flow.clone();
            let edit_btn_ref = edit_btn.clone();
            let sliders_card_ref = sliders_card.clone();
            let first_toggle_ref = first_toggle.clone();
            let drawing_ref = drawing.clone();
            let in_custom_ref = in_custom_mode.clone();

            save_btn.connect_clicked(move |_| {
                let name = name_entry_ref.text().trim().to_string();
                if name.is_empty() {
                    return;
                }

                let gains = *gains_ref.borrow();
                let preset = CustomPreset { name: name.clone(), gains };

                presets_ref.borrow_mut().push(preset.clone());
                save_custom_presets(&presets_ref.borrow());

                add_custom_preset_button(
                    &custom_flow_ref,
                    &preset,
                    presets_ref.clone(),
                    gains_ref.clone(),
                    drawing_ref.clone(),
                    first_toggle_ref.clone(),
                    in_custom_ref.clone(),
                );

                name_entry_ref.set_text("");
                sliders_card_ref.set_visible(false);
                edit_btn_ref.set_active(false);
                update_settings(|s| s.last_eq_preset = 6);
            });
        }

        {
            let existing = custom_presets.borrow().clone();
            for preset in existing.iter() {
                add_custom_preset_button(
                    &custom_flow,
                    preset,
                    custom_presets.clone(),
                    current_gains.clone(),
                    drawing.clone(),
                    first_toggle.clone(),
                    in_custom_mode.clone(),
                );
            }
        }

        root.append(&title);
        root.append(&curve_card);
        root.append(&builtin_label);
        root.append(&flow);
        root.append(&custom_section_label);
        root.append(&custom_flow);
        root.append(&edit_btn);
        root.append(&sliders_card);

        clamp.set_child(Some(&root));

        let scroll = ScrolledWindow::new();
        scroll.set_hscrollbar_policy(gtk4::PolicyType::Never);
        scroll.set_vexpand(true);
        scroll.set_child(Some(&clamp));

        scroll.upcast()
    }
}

fn add_custom_preset_button(
    flow: &gtk4::FlowBox,
    preset: &CustomPreset,
    presets_store: Rc<RefCell<Vec<CustomPreset>>>,
    gains_state: Rc<RefCell<[i8; 7]>>,
    drawing: DrawingArea,
    first_toggle: Rc<RefCell<Option<ToggleButton>>>,
    in_custom_mode: Rc<Cell<bool>>,
) {
    let btn = make_preset_button(&preset.name, &preset.gains, true);

    if let Some(ref f) = *first_toggle.borrow() {
        btn.set_group(Some(f));
    }

    let gains_copy = preset.gains;
    {
        let drawing_ref = drawing.clone();
        let gains_ref = gains_state.clone();
        let in_custom_ref = in_custom_mode.clone();

        btn.connect_toggled(clone!(
            #[weak] drawing_ref,
            move |b| {
                if b.is_active() {
                    b.add_css_class("suggested-action");
                    in_custom_ref.set(false);
                    *gains_ref.borrow_mut() = gains_copy;
                    drawing_ref.queue_draw();
                    // For custom preset, we might not have a preset ID; but we can set last_eq_preset to 6
                    update_settings(|s| s.last_eq_preset = 6);
                } else {
                    b.remove_css_class("suggested-action");
                }
            }
        ));
    }

    let child = gtk4::FlowBoxChild::new();
    child.set_child(Some(&btn));
    child.set_focusable(false);
    let gc = GestureClick::new();
    gc.set_button(3);
    {
        let child_ref = child.clone();
        let presets_ref = presets_store.clone();
        let name = preset.name.clone();
        let flow_ref = flow.clone();
        gc.connect_pressed(move |g, _n, _x, _y| {
            g.set_state(gtk4::EventSequenceState::Claimed);
            show_delete_popover(&child_ref, &flow_ref, presets_ref.clone(), &name);
        });
    }
    btn.add_controller(gc);
    let glp = GestureLongPress::new();
    glp.set_touch_only(false);
    {
        let child_ref = child.clone();
        let presets_ref = presets_store.clone();
        let name = preset.name.clone();
        let flow_ref = flow.clone();
        glp.connect_pressed(move |g, _x, _y| {
            g.set_state(gtk4::EventSequenceState::Claimed);
            show_delete_popover(&child_ref, &flow_ref, presets_ref.clone(), &name);
        });
    }
    btn.add_controller(glp);

    flow.append(&child);
}

fn show_delete_popover(
    child: &gtk4::FlowBoxChild,
    flow: &gtk4::FlowBox,
    presets: Rc<RefCell<Vec<CustomPreset>>>,
    name: &str,
) {
    let popover = gtk4::Popover::new();
    popover.set_parent(child);

    let vbox = Box::new(Orientation::Vertical, 8);
    vbox.set_margin_top(8);
    vbox.set_margin_bottom(8);
    vbox.set_margin_start(12);
    vbox.set_margin_end(12);

    let lbl = Label::new(Some(&format!("Delete \"{}\"?", name)));
    lbl.add_css_class("body");

    let del_btn = gtk4::Button::with_label("Delete");
    del_btn.add_css_class("destructive-action");

    vbox.append(&lbl);
    vbox.append(&del_btn);
    popover.set_child(Some(&vbox));

    let name_owned = name.to_string();
    let child_ref = child.clone();
    let flow_ref = flow.clone();
    let popover_ref = popover.clone();

    del_btn.connect_clicked(move |_| {
        presets.borrow_mut().retain(|p| p.name != name_owned);
        save_custom_presets(&presets.borrow());
        flow_ref.remove(&child_ref);
        popover_ref.popdown();
    });

    popover.popup();
}

fn make_flowbox() -> gtk4::FlowBox {
    let fb = gtk4::FlowBox::new();
    fb.set_selection_mode(gtk4::SelectionMode::None);
    fb.set_homogeneous(true);
    fb.set_min_children_per_line(2);
    fb.set_max_children_per_line(6);
    fb.set_row_spacing(8);
    fb.set_column_spacing(8);
    fb.set_hexpand(true);
    fb
}

fn make_preset_button(name: &str, gains: &[i8; 7], is_custom: bool) -> ToggleButton {
    let btn = ToggleButton::new();
    btn.set_hexpand(true);

    let content = Box::new(Orientation::Vertical, 4);
    content.set_margin_top(10);
    content.set_margin_bottom(10);
    content.set_margin_start(8);
    content.set_margin_end(8);

    let preview = DrawingArea::new();
    preview.set_content_height(36);
    preview.set_hexpand(true);
    let g = *gains;
    preview.set_draw_func(move |_area, cr, w, h| {
        draw_mini_curve(cr, w as f64, h as f64, &g);
    });

    let name_lbl = Label::new(Some(name));
    name_lbl.add_css_class("caption-heading");

    content.append(&preview);
    content.append(&name_lbl);

    if is_custom {
        let badge = Label::new(Some("custom"));
        badge.add_css_class("caption");
        badge.add_css_class("dim-label");
        content.append(&badge);
    }

    btn.set_child(Some(&content));
    btn
}

fn db_to_y(db: f64, height: f64) -> f64 {
    height * (1.0 - (db.max(-12.0).min(12.0) + 12.0) / 24.0)
}

fn band_center_x(i: usize, n: usize, width: f64) -> f64 {
    ((i as f64 + 0.5) / n as f64) * width
}

fn draw_eq_curve(cr: &gtk4::cairo::Context, w: f64, h: f64, gains: &[i8]) {
    let n = gains.len();

    for &db in &[-12.0f64, -6.0, 0.0, 6.0, 12.0] {
        let y = db_to_y(db, h);
        if db.abs() < 0.1 {
            cr.set_source_rgba(0.5, 0.5, 0.5, 0.5);
            cr.set_line_width(1.5);
        } else {
            cr.set_source_rgba(0.5, 0.5, 0.5, 0.18);
            cr.set_line_width(1.0);
        }
        cr.move_to(0.0, y);
        cr.line_to(w, y);
        let _ = cr.stroke();
    }

    cr.set_source_rgba(0.5, 0.5, 0.5, 0.1);
    cr.set_line_width(1.0);
    for i in 1..n {
        let x = (i as f64 / n as f64) * w;
        cr.move_to(x, 0.0);
        cr.line_to(x, h);
        let _ = cr.stroke();
    }

    let pts: Vec<(f64, f64)> = (0..n)
        .map(|i| (band_center_x(i, n, w), db_to_y(gains[i] as f64, h)))
        .collect();

    cr.new_path();
    cr.move_to(pts[0].0, h);
    cr.line_to(pts[0].0, pts[0].1);
    for i in 1..pts.len() {
        let cpx = (pts[i-1].0 + pts[i].0) / 2.0;
        cr.curve_to(cpx, pts[i-1].1, cpx, pts[i].1, pts[i].0, pts[i].1);
    }
    cr.line_to(pts[n-1].0, h);
    cr.close_path();
    cr.set_source_rgba(0.208, 0.518, 0.894, 0.18);
    let _ = cr.fill();

    cr.new_path();
    cr.move_to(pts[0].0, pts[0].1);
    for i in 1..pts.len() {
        let cpx = (pts[i-1].0 + pts[i].0) / 2.0;
        cr.curve_to(cpx, pts[i-1].1, cpx, pts[i].1, pts[i].0, pts[i].1);
    }
    cr.set_source_rgba(0.208, 0.518, 0.894, 0.95);
    cr.set_line_width(2.5);
    let _ = cr.stroke();

    for &(x, y) in &pts {
        cr.arc(x, y, 4.5, 0.0, std::f64::consts::TAU);
        cr.set_source_rgba(0.208, 0.518, 0.894, 1.0);
        let _ = cr.fill();
        cr.arc(x, y, 4.5, 0.0, std::f64::consts::TAU);
        cr.set_source_rgba(1.0, 1.0, 1.0, 0.9);
        cr.set_line_width(1.5);
        let _ = cr.stroke();
    }
}

fn draw_mini_curve(cr: &gtk4::cairo::Context, w: f64, h: f64, gains: &[i8; 7]) {
    let n = gains.len();
    let pts: Vec<(f64, f64)> = (0..n)
        .map(|i| (band_center_x(i, n, w), db_to_y(gains[i] as f64, h)))
        .collect();

    cr.new_path();
    cr.move_to(pts[0].0, h);
    cr.line_to(pts[0].0, pts[0].1);
    for i in 1..pts.len() {
        let cpx = (pts[i-1].0 + pts[i].0) / 2.0;
        cr.curve_to(cpx, pts[i-1].1, cpx, pts[i].1, pts[i].0, pts[i].1);
    }
    cr.line_to(pts[n-1].0, h);
    cr.close_path();
    cr.set_source_rgba(0.208, 0.518, 0.894, 0.22);
    let _ = cr.fill();

    cr.new_path();
    cr.move_to(pts[0].0, pts[0].1);
    for i in 1..pts.len() {
        let cpx = (pts[i-1].0 + pts[i].0) / 2.0;
        cr.curve_to(cpx, pts[i-1].1, cpx, pts[i].1, pts[i].0, pts[i].1);
    }
    cr.set_source_rgba(0.208, 0.518, 0.894, 0.85);
    cr.set_line_width(1.5);
    let _ = cr.stroke();
}