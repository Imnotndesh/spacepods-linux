use gtk4::prelude::*;
use gtk4::{Box, Label, Orientation};

pub struct AboutPage;

impl AboutPage {
    pub fn new() -> Box {
        let container = Box::new(Orientation::Vertical, 12);
        let label = Label::new(Some("About SpacePods"));
        container.append(&label);
        container
    }
}