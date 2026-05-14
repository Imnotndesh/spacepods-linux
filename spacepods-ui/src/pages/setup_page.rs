use gtk4::prelude::*;
use gtk4::{Box, Button, Label, Orientation};

pub struct SetupPage;

impl SetupPage {
    pub fn new<F: Fn() + 'static + Clone>(on_complete: F) -> Box {
        let container = Box::new(Orientation::Vertical, 12);
        container.set_halign(gtk4::Align::Center);
        container.set_valign(gtk4::Align::Center);

        let label = Label::new(Some("Welcome to SpacePods"));
        label.add_css_class("title-1");

        let scan_button = Button::with_label("Scan for devices");
        let skip_button = Button::with_label("Skip (for testing)");
        let status_label = Label::new(Some("Press 'Scan' to find your SpaceBuds"));

        let status_clone = status_label.clone();
        let on_complete_clone = on_complete.clone();
        scan_button.connect_clicked(move |_| {
            status_clone.set_text("Scanning...");
            let on_complete = on_complete_clone.clone();
            glib::timeout_add_local(std::time::Duration::from_secs(2), move || {
                on_complete();
                glib::ControlFlow::Break
            });
        });

        skip_button.connect_clicked(move |_| {
            on_complete();
        });

        container.append(&label);
        container.append(&scan_button);
        container.append(&skip_button);
        container.append(&status_label);
        container
    }
}