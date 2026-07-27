use gtk4::prelude::*;
use gtk4::{Box, Label, Orientation, Spinner, ListBox, ListBoxRow, Align, Image};
use libadwaita::prelude::*;
use libadwaita::{ActionRow, Clamp, PreferencesGroup, StatusPage};
use std::rc::Rc;
use std::cell::RefCell;
use glib::clone;
use libspacepods::client::SpacePodsClient;
use crate::storage::add_known_device;

pub struct SetupPage;

impl SetupPage {
    pub fn new<F: Fn() + 'static + Clone>(on_complete: F) -> gtk4::Widget {
        let status_page = StatusPage::new();
        status_page.set_icon_name(Some("audio-headset-symbolic"));
        status_page.set_title("Find Your SpaceBuds");
        status_page.set_description(Some(
            "Make sure your earbuds are in pairing mode and nearby."
        ));
        status_page.set_vexpand(true);

        let spinner = Spinner::new();
        spinner.set_size_request(32, 32);
        spinner.set_halign(gtk4::Align::Center);

        let scan_status = Label::new(None);
        scan_status.add_css_class("dim-label");
        scan_status.add_css_class("title-4");
        scan_status.set_halign(gtk4::Align::Center);

        let scan_spinner_box = Box::new(Orientation::Vertical, 12);
        scan_spinner_box.set_halign(Align::Center);
        scan_spinner_box.set_margin_bottom(16);
        scan_spinner_box.append(&spinner);
        scan_spinner_box.append(&scan_status);
        scan_spinner_box.set_visible(false);

        let error_label = Label::new(None);
        error_label.add_css_class("error");
        error_label.set_halign(gtk4::Align::Center);
        error_label.set_wrap(true);
        error_label.set_max_width_chars(50);
        error_label.set_visible(false);

        let device_list = ListBox::new();
        device_list.set_selection_mode(gtk4::SelectionMode::Single);
        device_list.set_visible(false);
        device_list.add_css_class("boxed-list");

        let scanned: Rc<RefCell<Vec<(String, String)>>> = Rc::new(RefCell::new(Vec::new()));

        let scan_group = PreferencesGroup::new();
        let scan_row = ActionRow::new();
        scan_row.set_title("Scan for devices");
        scan_row.set_subtitle("Search for nearby SpaceBuds over Bluetooth");
        scan_row.set_activatable(true);
        let scan_icon = gtk4::Image::from_icon_name("bluetooth-symbolic");
        scan_icon.add_css_class("dim-label");
        scan_row.add_prefix(&scan_icon);
        scan_row.add_suffix(&gtk4::Image::from_icon_name("go-next-symbolic"));
        scan_group.add(&scan_row);

        let content = Box::new(Orientation::Vertical, 0);
        content.set_valign(Align::Center);
        content.set_vexpand(true);
        content.set_margin_top(32);
        content.set_margin_bottom(32);
        content.set_margin_start(16);
        content.set_margin_end(16);
        content.append(&status_page);
        content.append(&scan_spinner_box);
        content.append(&error_label);
        content.append(&device_list);
        content.append(&scan_group);

        let clamp = Clamp::new();
        clamp.set_maximum_size(500);
        clamp.set_tightening_threshold(400);
        clamp.set_child(Some(&content));

        let scroll = gtk4::ScrolledWindow::new();
        scroll.set_hscrollbar_policy(gtk4::PolicyType::Never);
        scroll.set_vexpand(true);
        scroll.set_child(Some(&clamp));

        scan_row.connect_activated(clone!(
            #[weak] scan_spinner_box,
            #[weak] spinner,
            #[weak] scan_status,
            #[weak] device_list,
            #[weak] error_label,
            #[weak] scan_row,
            #[strong] scanned,
            move |_| {
                scan_row.set_sensitive(false);
                scan_spinner_box.set_visible(true);
                spinner.start();
                scan_status.set_text("Connecting to service…");
                device_list.set_visible(false);
                error_label.set_visible(false);

                while let Some(child) = device_list.first_child() {
                    device_list.remove(&child);
                }
                scanned.borrow_mut().clear();

                let scanned_ref = scanned.clone();

                glib::spawn_future_local(clone!(
                    #[weak] scan_spinner_box,
                    #[weak] spinner,
                    #[weak] scan_status,
                    #[weak] device_list,
                    #[weak] error_label,
                    #[weak] scan_row,
                    async move {
                        let mut client = match SpacePodsClient::connect(None).await {
                            Ok(c) => c,
                            Err(e) => {
                                scan_spinner_box.set_visible(false);
                                spinner.stop();
                                scan_row.set_sensitive(true);
                                error_label.set_text(&format!(
                                    "Cannot reach the SpacePods daemon.\n\nMake sure 'libspacepods service' is running.\n\n{}",
                                    e
                                ));
                                error_label.set_visible(true);
                                return;
                            }
                        };

                        scan_status.set_text("Scanning for SpaceBuds…");
                        let results = client.scan(5).await;

                        scan_spinner_box.set_visible(false);
                        spinner.stop();
                        scan_row.set_sensitive(true);

                        match results {
                            Ok(devices) if devices.is_empty() => {
                                scan_status.set_text("No SpaceBuds found nearby.");
                                scan_spinner_box.set_visible(true);
                                spinner.set_visible(false);
                            }
                            Ok(devices) => {
                                *scanned_ref.borrow_mut() = devices
                                    .iter()
                                    .map(|d| (d.name.clone(), d.address.clone()))
                                    .collect();

                                for device in &devices {
                                    let row = ActionRow::new();
                                    row.set_title(&device.name);
                                    row.set_subtitle(&device.address);
                                    row.set_icon_name(Some("audio-headset-symbolic"));
                                    row.add_suffix(&gtk4::Image::from_icon_name("go-next-symbolic"));
                                    device_list.append(&row);
                                }

                                device_list.set_visible(true);
                                scan_status.set_text(&format!("{} device(s) found — tap one to connect", devices.len()));
                                scan_spinner_box.set_visible(true);
                                spinner.set_visible(false);
                            }
                            Err(e) => {
                                error_label.set_text(&format!("Scan failed: {}", e));
                                error_label.set_visible(true);
                            }
                        }
                    }
                ));
            }
        ));

        let on_complete_skip = on_complete.clone();
        device_list.connect_row_activated(clone!(
            #[strong] scanned,
            move |_, row| {
                let idx = row.index() as usize;
                let devices = scanned.borrow();
                if let Some((name, address)) = devices.get(idx) {
                    let name = name.clone();
                    let address = address.clone();
                    let on_complete = on_complete.clone();
                    glib::spawn_future_local(async move {
                        match SpacePodsClient::connect(None).await {
                            Ok(mut client) => {
                                match client.connect_device(address.clone()).await {
                                    Ok(_) => {
                                        add_known_device(name.clone(), address);
                                        let _ = crate::storage::load_settings();
                                        on_complete();
                                    }
                                    Err(e) => {
                                        // Connection failed — go home anyway, daemon may still work
                                        eprintln!("Setup: connect failed for {}: {}", name, e);
                                        on_complete();
                                    }
                                }
                            }
                            Err(e) => {
                                eprintln!("Setup: service unreachable: {}", e);
                                on_complete();
                            }
                        }
                    });
                }
            }
        ));

        // The return value — scrolled clamp
        scroll.upcast()
    }
}
