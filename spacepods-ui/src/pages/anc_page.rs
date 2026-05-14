use std::cell::Cell;
use std::rc::Rc;
use libadwaita::prelude::PreferencesGroupExt;
use libadwaita::prelude::PreferencesRowExt;
use glib::clone;
use gtk4::prelude::*;
use gtk4::{Box, Label, Orientation, Scale, Switch, ToggleButton};
use libadwaita::{ActionRow, PreferencesGroup};
use libadwaita::prelude::ActionRowExt;

pub struct AncPage;

impl AncPage {
    pub fn new() -> Box {
        let container = Box::new(Orientation::Vertical, 12);
        container.set_halign(gtk4::Align::Center);
        container.set_valign(gtk4::Align::Center);
        container.set_vexpand(false);

        let title = Label::new(Some("ANC Control"));
        title.add_css_class("title-1");

        let off_btn = ToggleButton::with_label("OFF");
        let anc_btn = ToggleButton::with_label("ANC");
        let trans_btn = ToggleButton::with_label("Transparency");
        let mode_status = Label::new(Some("Current mode: Unknown"));

        anc_btn.set_group(Some(&off_btn));
        trans_btn.set_group(Some(&off_btn));

        let buttons_box = Box::new(Orientation::Horizontal, 12);
        buttons_box.set_halign(gtk4::Align::Center);
        buttons_box.append(&off_btn);
        buttons_box.append(&anc_btn);
        buttons_box.append(&trans_btn);

        // --- Intensity slider (shown for ANC and Transparency) ---
        let slider_box = Box::new(Orientation::Vertical, 4);
        slider_box.set_halign(gtk4::Align::Fill);
        slider_box.set_hexpand(true);
        slider_box.set_visible(false); // hidden by default (OFF mode)

        let slider_label = Label::new(Some("Intensity"));
        slider_label.set_halign(gtk4::Align::Start);
        slider_label.add_css_class("caption");

        let slider = Scale::with_range(Orientation::Horizontal, 1.0, 5.0, 1.0);
        slider.set_draw_value(false);
        slider.set_hexpand(true);
        slider.set_value(3.0); // default to middle

        // Add named marks: Low, Mid, High at positions 1, 3, 5
        slider.add_mark(1.0, gtk4::PositionType::Bottom, Some("Low"));
        slider.add_mark(2.0, gtk4::PositionType::Bottom, None);
        slider.add_mark(3.0, gtk4::PositionType::Bottom, Some("Mid"));
        slider.add_mark(4.0, gtk4::PositionType::Bottom, None);
        slider.add_mark(5.0, gtk4::PositionType::Bottom, Some("High"));

        slider_box.append(&slider_label);
        slider_box.append(&slider);

        let current_mode = Rc::new(Cell::new(0u8));

        let update_ui = {
            let slider_box = slider_box.clone();
            move |mode: u8, adaptive_switch: &Switch, adaptive_row: &ActionRow| {
                match mode {
                    0 => {
                        slider_box.set_visible(false);
                        adaptive_switch.set_sensitive(false);
                        adaptive_row.set_sensitive(false);
                        adaptive_row.set_tooltip_text(Some("Available in ANC mode"));
                    }
                    1 => {
                        // ANC
                        slider_box.set_visible(true);
                        adaptive_switch.set_sensitive(true);
                        adaptive_row.set_sensitive(true);
                        adaptive_row.set_tooltip_text(None);
                    }
                    2 => {
                        // Transparency
                        slider_box.set_visible(true);
                        adaptive_switch.set_sensitive(false);
                        adaptive_row.set_sensitive(false);
                        adaptive_row.set_tooltip_text(Some("Available in ANC mode"));
                    }
                    _ => {}
                }
            }
        };

        let adaptive_row = ActionRow::new();
        adaptive_row.set_title("Adaptive ANC");
        adaptive_row.set_subtitle("Dynamically adjust based on environment");
        let adaptive_switch = Switch::new();
        adaptive_switch.set_valign(gtk4::Align::Center);
        adaptive_switch.set_vexpand(false);
        adaptive_row.add_suffix(&adaptive_switch);
        adaptive_row.set_activatable_widget(Some(&adaptive_switch));

        adaptive_switch.set_sensitive(false);
        adaptive_row.set_sensitive(false);
        adaptive_row.set_tooltip_text(Some("Available in ANC mode"));

        off_btn.connect_toggled(clone!(
            #[weak] mode_status,
            #[weak] adaptive_switch,
            #[weak] adaptive_row,
            #[strong] current_mode,
            #[strong] update_ui,
            move |btn| {
                if btn.is_active() {
                    current_mode.set(0);
                    btn.add_css_class("suggested-action");
                    mode_status.set_text("Current mode: OFF");
                    update_ui(0, &adaptive_switch, &adaptive_row);
                } else {
                    btn.remove_css_class("suggested-action");
                }
            }
        ));

        anc_btn.connect_toggled(clone!(
            #[weak] mode_status,
            #[weak] adaptive_switch,
            #[weak] adaptive_row,
            #[strong] current_mode,
            #[strong] update_ui,
            move |btn| {
                if btn.is_active() {
                    current_mode.set(1);
                    btn.add_css_class("suggested-action");
                    mode_status.set_text("Current mode: ANC");
                    update_ui(1, &adaptive_switch, &adaptive_row);
                } else {
                    btn.remove_css_class("suggested-action");
                }
            }
        ));

        trans_btn.connect_toggled(clone!(
            #[weak] mode_status,
            #[weak] adaptive_switch,
            #[weak] adaptive_row,
            #[strong] current_mode,
            #[strong] update_ui,
            move |btn| {
                if btn.is_active() {
                    current_mode.set(2);
                    btn.add_css_class("suggested-action");
                    mode_status.set_text("Current mode: Transparency");
                    update_ui(2, &adaptive_switch, &adaptive_row);
                } else {
                    btn.remove_css_class("suggested-action");
                }
            }
        ));

        let features_group = PreferencesGroup::new();
        features_group.set_title("Additional Features");
        features_group.set_valign(gtk4::Align::Start);
        features_group.set_vexpand(false);

        let dual_row = ActionRow::new();
        dual_row.set_title("Dual Device (Multi-point)");
        dual_row.set_subtitle("Connect to two devices simultaneously");
        let dual_switch = Switch::new();
        dual_switch.set_valign(gtk4::Align::Center);
        dual_switch.set_vexpand(false);
        dual_row.add_suffix(&dual_switch);
        dual_row.set_activatable_widget(Some(&dual_switch));

        features_group.add(&adaptive_row);
        features_group.add(&dual_row);

        container.append(&title);
        container.append(&buttons_box);
        container.append(&mode_status);
        container.append(&slider_box);
        container.append(&features_group);

        container
    }
}