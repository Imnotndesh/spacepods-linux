use gtk4::prelude::*;
use gtk4::{Box, Label, Orientation, Spinner, ListBox, ListBoxRow};
use libadwaita::prelude::*;
use libadwaita::{ActionRow, Clamp, HeaderBar, PreferencesGroup, StatusPage, ToolbarView, WindowTitle};
use std::rc::Rc;
use std::cell::RefCell;
use glib::clone;
use libspacepods::client::SpacePodsClient;
use crate::storage::add_known_device;

pub struct SetupPage;

impl SetupPage {
    pub fn new<F: Fn() + 'static + Clone>(on_complete: F) -> ToolbarView {
        let header = HeaderBar::new();
        let title_widget = WindowTitle::new("SpacePods Setup", "Connect your earbuds");
        header.set_title_widget(Some(&title_widget));

        let close_btn = gtk4::Button::with_label("Skip");
        close_btn.add_css_class("flat");
        header.pack_end(&close_btn);

        let status_page = StatusPage::new();
        status_page.set_icon_name(Some("audio-headset-symbolic"));
        status_page.set_title("Find Your SpaceBuds");
        status_page.set_description(Some(
            "Make sure your earbuds are in pairing mode and nearby.",
        ));
        status_page.set_vexpand(true);

        let spinner = Spinner::new();
        spinner.set_size_request(32, 32);
        spinner.set_halign(gtk4::Align::Center);

        let scan_status = Label::new(None);
        scan_status.add_css_class("dim-label");
        scan_status.set_halign(gtk4::Align::Center);

        let spinner_box = Box::new(Orientation::Vertical, 8);
        spinner_box.set_halign(gtk4::Align::Center);
        spinner_box.set_margin_bottom(16);
        spinner_box.append(&spinner);
        spinner_box.append(&scan_status);
        spinner_box.set_visible(false);

        let device_list = ListBox::new();
        device_list.set_selection_mode(gtk4::SelectionMode::Single);
        device_list.set_visible(false);
        device_list.add_css_class("boxed-list");

        // Stored scan results indexed parallel to list rows
        let scanned: Rc<RefCell<Vec<(String, String)>>> = Rc::new(RefCell::new(Vec::new()));

        let group = PreferencesGroup::new();
        let scan_row = ActionRow::new();
        scan_row.set_title("Scan for devices");
        scan_row.set_subtitle("Search for nearby SpaceBuds over Bluetooth");
        scan_row.set_activatable(true);
        let scan_icon = gtk4::Image::from_icon_name("bluetooth-symbolic");
        scan_icon.add_css_class("dim-label");
        scan_row.add_prefix(&scan_icon);
        let scan_arrow = gtk4::Image::from_icon_name("go-next-symbolic");
        scan_arrow.add_css_class("dim-label");
        scan_row.add_suffix(&scan_arrow);
        group.add(&scan_row);

        let service_error = Label::new(None);
        service_error.add_css_class("error");
        service_error.set_halign(gtk4::Align::Center);
        service_error.set_visible(false);

        let clamp = Clamp::new();
        clamp.set_maximum_size(480);
        clamp.set_tightening_threshold(400);

        let content = Box::new(Orientation::Vertical, 0);
        content.set_margin_top(24);
        content.set_margin_bottom(32);
        content.set_margin_start(16);
        content.set_margin_end(16);
        content.append(&status_page);
        content.append(&spinner_box);
        content.append(&service_error);
        content.append(&device_list);
        content.append(&group);
        clamp.set_child(Some(&content));

        let scroll = gtk4::ScrolledWindow::new();
        scroll.set_hscrollbar_policy(gtk4::PolicyType::Never);
        scroll.set_vexpand(true);
        scroll.set_child(Some(&clamp));

        let toolbar_view = ToolbarView::new();
        toolbar_view.add_top_bar(&header);
        toolbar_view.set_content(Some(&scroll));

        scan_row.connect_activated(clone!(
            #[weak] spinner_box,
            #[weak] spinner,
            #[weak] scan_status,
            #[weak] device_list,
            #[weak] service_error,
            #[weak] scan_row,
            #[strong] scanned,
            move |_| {
                scan_row.set_sensitive(false);
                spinner_box.set_visible(true);
                spinner.start();
                scan_status.set_text("Connecting to SpacePods service…");
                device_list.set_visible(false);
                service_error.set_visible(false);

                while let Some(child) = device_list.first_child() {
                    device_list.remove(&child);
                }
                scanned.borrow_mut().clear();

                let scanned_ref = scanned.clone();

                glib::spawn_future_local(clone!(
                    #[weak] spinner_box,
                    #[weak] spinner,
                    #[weak] scan_status,
                    #[weak] device_list,
                    #[weak] service_error,
                    #[weak] scan_row,
                    async move {
                        let mut client = match SpacePodsClient::connect(None).await {
                            Ok(c) => c,
                            Err(e) => {
                                spinner_box.set_visible(false);
                                spinner.stop();
                                scan_row.set_sensitive(true);
                                service_error.set_text(&format!(
                                    "Cannot reach service: {}. Is 'spacepods service' running?", e
                                ));
                                service_error.set_visible(true);
                                return;
                            }
                        };

                        scan_status.set_text("Scanning for SpaceBuds… (5s)");

                        let results = client.scan(5).await;

                        spinner_box.set_visible(false);
                        spinner.stop();
                        scan_row.set_sensitive(true);

                        match results {
                            Ok(devices) if devices.is_empty() => {
                                scan_status.set_text("No devices found. Try again.");
                                spinner_box.set_visible(true);
                            }
                            Ok(devices) => {
                                *scanned_ref.borrow_mut() = devices.clone();

                                for (name, address) in &devices {
                                    let row = ListBoxRow::new();
                                    let hbox = Box::new(Orientation::Horizontal, 12);
                                    hbox.set_margin_top(10);
                                    hbox.set_margin_bottom(10);
                                    hbox.set_margin_start(12);
                                    hbox.set_margin_end(12);

                                    let name_lbl = Label::new(Some(name.as_str()));
                                    name_lbl.set_hexpand(true);
                                    name_lbl.set_halign(gtk4::Align::Start);

                                    let addr_lbl = Label::new(Some(address.as_str()));
                                    addr_lbl.add_css_class("dim-label");
                                    addr_lbl.add_css_class("caption");

                                    hbox.append(&name_lbl);
                                    hbox.append(&addr_lbl);
                                    row.set_child(Some(&hbox));
                                    device_list.append(&row);
                                }

                                device_list.set_visible(true);
                                scan_status.set_text("Select a device to connect");
                                spinner_box.set_visible(true);
                            }
                            Err(e) => {
                                service_error.set_text(&format!("Scan failed: {}", e));
                                service_error.set_visible(true);
                            }
                        }
                    }
                ));
            }
        ));

        let on_complete_close = on_complete.clone();
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
                        if let Ok(mut client) = SpacePodsClient::connect(None).await {
                            if let Ok(_) = client.connect_device(address.clone()).await {
                                add_known_device(name, address);
                                let _ = crate::storage::load_settings();
                            }
                        }
                        on_complete();
                    });
                }
            }
        ));

        close_btn.connect_clicked(move |_| on_complete_close());

        toolbar_view
    }
}