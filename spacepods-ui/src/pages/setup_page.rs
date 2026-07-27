use gtk4::prelude::*;
use gtk4::{Box, Label, Orientation, Spinner, ListBox, Align, Button};
use libadwaita::prelude::*;
use libadwaita::{ActionRow, Clamp, PreferencesGroup, StatusPage};
use std::rc::Rc;
use std::cell::RefCell;
use glib::clone;
use libspacepods::client::SpacePodsClient;
use crate::storage::add_known_device;
use crate::log::Log;

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
        device_list.set_selection_mode(gtk4::SelectionMode::None);
        device_list.set_visible(false);
        device_list.add_css_class("boxed-list");
        device_list.set_margin_bottom(24);

        let scanned: Rc<RefCell<Vec<(String, String)>>> = Rc::new(RefCell::new(Vec::new()));

        // Track which entries are currently connecting
        let connecting_flags: Rc<RefCell<Vec<bool>>> = Rc::new(RefCell::new(Vec::new()));

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

        // ── Scan button ──
        {
            let on_complete = on_complete.clone();
            let sr = scan_row.clone();
            let ssb = scan_spinner_box.clone();
            let sp = spinner.clone();
            let sc = scan_status.clone();
            let dl = device_list.clone();
            let el = error_label.clone();
            let scn = scanned.clone();
            let cf = connecting_flags.clone();
            scan_row.connect_activated({
                let sr = sr.clone();
                let ssb = ssb.clone();
                let sp = sp.clone();
                let sc = sc.clone();
                let dl = dl.clone();
                let el = el.clone();
                let scn = scn.clone();
                let cf = cf.clone();
                let on = on_complete.clone();
                move |_| {
                sr.set_sensitive(false);
                ssb.set_visible(true);
                sp.start();
                sc.set_text("Connecting to service…");
                dl.set_visible(false);
                el.set_visible(false);

                while let Some(child) = dl.first_child() {
                    dl.remove(&child);
                }
                scn.borrow_mut().clear();
                cf.borrow_mut().clear();

                let scanned_ref = scn.clone();
                let flags_ref = cf.clone();

                let ssb2 = ssb.clone();
                let sp2 = sp.clone();
                let sc2 = sc.clone();
                let dl2 = dl.clone();
                let el2 = el.clone();
                let sr2 = sr.clone();
                let on = on.clone();

                glib::spawn_future_local(async move {
                    let mut client = match SpacePodsClient::connect(None).await {
                        Ok(c) => c,
                        Err(e) => {
                            ssb2.set_visible(false);
                            sp2.stop();
                            sr2.set_sensitive(true);
                            el2.set_text(&format!(
                                "Cannot reach the SpacePods daemon.\n\nMake sure 'libspacepods service' is running.\n\n{}",
                                e
                            ));
                            el2.set_visible(true);
                            return;
                        }
                    };

                    sc2.set_text("Scanning for SpaceBuds…");
                    let results = client.scan(5).await;

                    ssb2.set_visible(false);
                    sp2.stop();
                    sr2.set_sensitive(true);

                    match results {
                        Ok(devices) if devices.is_empty() => {
                            sc2.set_text("No SpaceBuds found nearby.");
                            ssb2.set_visible(true);
                            sp2.set_visible(false);
                        }
                        Ok(devices) => {
                            let mut devs = scanned_ref.borrow_mut();
                            let mut flags = flags_ref.borrow_mut();
                            for device in &devices {
                                devs.push((device.name.clone(), device.address.clone()));
                                flags.push(false);
                            }

                            for (idx, device) in devices.iter().enumerate() {
                                dl2.append(&Self::device_row(
                                    idx,
                                    &device.name,
                                    &device.address,
                                    &scanned_ref,
                                    &flags_ref,
                                    on.clone(),
                                ));
                            }

                            dl2.set_visible(true);
                            sc2.set_text(&format!("{} device(s) found — tap Connect",
                                devices.len()));
                            ssb2.set_visible(true);
                            sp2.set_visible(false);
                        }
                        Err(e) => {
                            el2.set_text(&format!("Scan failed: {}", e));
                            el2.set_visible(true);
                        }
                    }
                });
            }});
        }

        scroll.upcast()
    }

    /// Build a single device row with name, address and a Connect button.
    fn device_row<F: Fn() + 'static + Clone>(
        idx: usize,
        name: &str,
        address: &str,
        scanned: &Rc<RefCell<Vec<(String, String)>>>,
        connecting_flags: &Rc<RefCell<Vec<bool>>>,
        on_complete: F,
    ) -> gtk4::Widget {
        let name = name.to_string();
        let address = address.to_string();

        let row = ActionRow::new();
        row.set_title(&name);
        row.set_subtitle(&address);
        row.set_icon_name(Some("audio-headset-symbolic"));
        row.set_margin_top(6);
        row.set_margin_bottom(6);

        let connect_box = Box::new(Orientation::Horizontal, 8);
        connect_box.set_valign(Align::Center);

        let btn = Button::with_label("Connect");
        btn.add_css_class("suggested-action");
        btn.add_css_class("pill");
        btn.set_valign(Align::Center);

        let conn_spinner = Spinner::new();
        conn_spinner.set_size_request(16, 16);
        conn_spinner.set_visible(false);

        let status_lbl = Label::new(None);
        status_lbl.add_css_class("caption");
        status_lbl.add_css_class("dim-label");
        status_lbl.set_valign(Align::Center);
        status_lbl.set_visible(false);

        connect_box.append(&status_lbl);
        connect_box.append(&conn_spinner);
        connect_box.append(&btn);
        row.add_suffix(&connect_box);

        let flags_ref = connecting_flags.clone();
        let scanned_ref = scanned.clone();

        let flags2 = flags_ref.clone();
        let conn_spinner2 = conn_spinner.clone();
        let status_lbl2 = status_lbl.clone();
        let btn2 = btn.clone();
        let name2 = name.clone();
        let addr2 = address.clone();

        btn.connect_clicked(move |_| {
            btn2.set_sensitive(false);
            btn2.set_label("Connecting…");
            conn_spinner2.set_visible(true);
            conn_spinner2.start();
            status_lbl2.set_visible(false);

            // Disable all other connect buttons
            {
                let mut flags = flags2.borrow_mut();
                if idx < flags.len() {
                    flags[idx] = true;
                }
            }

            let btn_clone = btn2.clone();
            let spinner_clone = conn_spinner2.clone();
            let status_lbl_clone = status_lbl2.clone();
            let name_clone = name2.clone();
            let addr_clone = addr2.clone();
            let on_complete = on_complete.clone();

            let flags_clone = flags_ref.clone();
            glib::spawn_future_local(async move {
                Log::info("SETUP", &format!("Connecting to {} ({})", name_clone, addr_clone));
                let outcome = match SpacePodsClient::connect(None).await {
                    Ok(mut client) => {
                        match client.connect_device(addr_clone.clone()).await {
                            Ok(_) => {
                                add_known_device(name_clone.clone(), addr_clone);
                                // Wait for daemon to detect product_id, then proceed
                                tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;
                                let _ = crate::storage::load_settings();
                                Log::info("SETUP", &format!("Connected to {}", name_clone));
                                on_complete();
                                return;
                            }
                            Err(e) => {
                                Log::info("SETUP", &format!("Connect failed for {}: {}", name_clone, e));
                                format!("Connection failed: {}", e)
                            }
                        }
                    }
                    Err(e) => {
                        Log::info("SETUP", &format!("Service unreachable: {}", e));
                        format!("Service unreachable: {}", e)
                    }
                };

                // Reset on error
                spinner_clone.stop();
                spinner_clone.set_visible(false);
                status_lbl_clone.set_text(&outcome);
                status_lbl_clone.add_css_class("error");
                status_lbl_clone.set_visible(true);
                btn_clone.set_label("Connect");
                btn_clone.set_sensitive(true);

                {
                    let mut flags = flags_clone.borrow_mut();
                    if idx < flags.len() {
                        flags[idx] = false;
                    }
                }
            });
        });

        row.upcast()
    }
}
