use gtk4::prelude::*;
use gtk4::{
    Box, Label, Orientation, Spinner, Button, Align, ScrolledWindow,
    ListBox, ListBoxRow, PolicyType,
};
use glib::clone;
use std::rc::Rc;
use libadwaita::prelude::*;
use libadwaita::{
    Clamp, HeaderBar, StatusPage,
    ToolbarView, WindowTitle, Toast, ToastOverlay,
};
use libspacepods::client::SpacePodsClient;
use libspacepods::ipc::protocol::ScannedDevice;
use crate::storage::{add_known_device, load_known_devices, load_settings, remove_known_device, update_settings};
use std::cell::RefCell;

// ── Helpers ──

fn friendly_error(err: &str) -> String {
    let lower = err.to_lowercase();
    if lower.contains("connection refused") || lower.contains("no such file") {
        "SpacePods service isn't running.\nStart it with: spacepods service".into()
    } else if lower.contains("timeout") || lower.contains("timed out") {
        "The operation took too long.\nMake sure your earbuds are nearby and in pairing mode.".into()
    } else if lower.contains("not found") || lower.contains("device not found") {
        "No SpaceBuds found.\nMake sure they're in pairing mode (LED flashing) and nearby.".into()
    } else if lower.contains("permission") || lower.contains("denied") {
        "Bluetooth permission issue.\nCheck that Bluetooth is enabled and accessible.".into()
    } else if lower.contains("bluetooth") || lower.contains("adapter") {
        "A Bluetooth error occurred.\nTry toggling Bluetooth off and on again.".into()
    } else if lower.contains("not connected") {
        "Not connected to earbuds.\nSelect a device and tap Connect.".into()
    } else {
        format!("Something went wrong.\n\nDetails: {}", err)
    }
}

fn rssi_label(rssi: Option<i16>) -> &'static str {
    match rssi {
        None => "",
        Some(v) if v >= -50 => "Very close",
        Some(v) if v >= -65 => "Near",
        Some(v) if v >= -80 => "Far",
        _ => "Distant",
    }
}

fn rssi_css(rssi: Option<i16>) -> &'static str {
    match rssi {
        None => "dim-label",
        Some(v) if v >= -50 => "success",
        Some(v) if v >= -65 => "accent",
        Some(v) if v >= -80 => "warning",
        _ => "dim-label",
    }
}

fn format_last_used(timestamp: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let diff = now.saturating_sub(timestamp);
    if diff < 60 { "Just now".into() }
    else if diff < 3600 { format!("{}m ago", diff / 60) }
    else if diff < 86400 { format!("{}h ago", diff / 3600) }
    else if diff < 604800 { format!("{}d ago", diff / 86400) }
    else {
        let months = (timestamp / 86400) / 30;
        if months < 12 { format!("{}mo ago", months) }
        else { format!("{}y ago", months / 12) }
    }
}

// ═══════════════════════════════════════════
// SETUP PAGE
// ═══════════════════════════════════════════

pub struct SetupPage;

impl SetupPage {
    pub fn new<F: Fn(Option<u16>) + 'static + Clone>(on_connected: F) -> gtk4::Widget {
        let toast_overlay = ToastOverlay::new();

        let show_toast = {
            let to = toast_overlay.clone();
            move |msg: &str| {
                let toast = Toast::new(msg);
                toast.set_timeout(4);
                toast.set_priority(libadwaita::ToastPriority::High);
                to.add_toast(toast);
            }
        };
        let show_error = {
            let st = show_toast.clone();
            move |err: &str| st(&format!("{}: {}", friendly_error(err).lines().next().unwrap_or(""), err))
        };

        // ── Header ──
        let header = HeaderBar::new();
        header.set_title_widget(Some(&WindowTitle::new("SpacePods Linux", "Connect your earbuds")));

        // ── Disclaimer banner ──
        let settings = load_settings();
        let disclaimer_revealer = gtk4::Revealer::new();
        disclaimer_revealer.set_transition_type(gtk4::RevealerTransitionType::SlideDown);
        disclaimer_revealer.set_transition_duration(300);
        disclaimer_revealer.set_reveal_child(!settings.disclaimer_dismissed);

        let banner = libadwaita::Banner::new("Requires libspacepods daemon");
        banner.set_title("Background Service Required");
        banner.set_button_label(Some("Got it"));
        banner.set_use_markup(false);
        banner.connect_button_clicked(glib::clone!(
            #[weak] disclaimer_revealer,
            move |_| {
                disclaimer_revealer.set_reveal_child(false);
                update_settings(|s| s.disclaimer_dismissed = true);
            }
        ));
        disclaimer_revealer.set_child(Some(&banner));

        // ═══ LEFT PANEL ═══
        let status_page = StatusPage::new();
        status_page.set_icon_name(Some("audio-headset-symbolic"));
        status_page.set_title("Find Your SpaceBuds");
        status_page.set_description(Some("Make sure your earbuds are in pairing mode and nearby."));

        let daemon_status = Label::new(None);
        daemon_status.add_css_class("caption");
        daemon_status.set_halign(gtk4::Align::Center);
        daemon_status.set_margin_top(8);
        daemon_status.set_margin_bottom(8);

        let scan_spinner = Spinner::new();
        scan_spinner.set_size_request(16, 16);
        scan_spinner.set_visible(false);

        let scan_btn = Button::with_label("Scan for devices");
        scan_btn.add_css_class("suggested-action");
        scan_btn.add_css_class("pill");
        scan_btn.set_valign(Align::Center);

        let scan_btn_row = Box::new(Orientation::Horizontal, 8);
        scan_btn_row.set_halign(Align::Center);
        scan_btn_row.set_margin_top(12);
        scan_btn_row.append(&scan_spinner);
        scan_btn_row.append(&scan_btn);

        let scan_status = Label::new(None);
        scan_status.add_css_class("caption");
        scan_status.add_css_class("dim-label");
        scan_status.set_halign(gtk4::Align::Center);
        scan_status.set_margin_top(4);

        let left_content = Box::new(Orientation::Vertical, 0);
        left_content.set_valign(Align::Center);
        left_content.set_vexpand(false);
        left_content.set_margin_top(16);
        left_content.set_margin_bottom(16);
        left_content.set_margin_start(16);
        left_content.set_margin_end(16);
        left_content.append(&status_page);
        left_content.append(&daemon_status);
        left_content.append(&scan_btn_row);
        left_content.append(&scan_status);

        // ═══ RIGHT PANEL ═══
        // -- Saved devices section --
        let saved_section = Box::new(Orientation::Vertical, 0);
        saved_section.set_vexpand(true);

        let saved_label = Label::new(Some("Saved Devices"));
        saved_label.add_css_class("title-4");
        saved_label.set_halign(gtk4::Align::Start);
        saved_label.set_margin_start(12);
        saved_label.set_margin_top(12);
        saved_label.set_margin_bottom(4);

        let saved_list = ListBox::new();
        saved_list.set_selection_mode(gtk4::SelectionMode::None);
        saved_list.add_css_class("boxed-list");
        saved_list.set_vexpand(true);

        let saved_scroll = ScrolledWindow::new();
        saved_scroll.set_hscrollbar_policy(PolicyType::Never);
        saved_scroll.set_vexpand(true);
        saved_scroll.set_child(Some(&saved_list));

        saved_section.append(&saved_label);
        saved_section.append(&saved_scroll);

        // -- Found devices section --
        let found_section = Box::new(Orientation::Vertical, 0);
        found_section.set_vexpand(true);

        let found_label = Label::new(None);
        found_label.add_css_class("title-4");
        found_label.set_halign(gtk4::Align::Start);
        found_label.set_margin_start(12);
        found_label.set_margin_top(16);
        found_label.set_margin_bottom(4);

        let device_list = ListBox::new();
        device_list.set_selection_mode(gtk4::SelectionMode::Single);
        device_list.add_css_class("boxed-list");
        device_list.set_vexpand(true);

        let device_scroll = ScrolledWindow::new();
        device_scroll.set_hscrollbar_policy(PolicyType::Never);
        device_scroll.set_vexpand(true);
        device_scroll.set_child(Some(&device_list));

        found_section.append(&found_label);
        found_section.append(&device_scroll);

        // Stack: saved on top when no scan, found on top when scanning
        let right_stack = gtk4::Stack::new();
        right_stack.set_transition_type(gtk4::StackTransitionType::Crossfade);
        right_stack.set_transition_duration(200);
        right_stack.add_named(&saved_section, Some("saved"));
        right_stack.add_named(&found_section, Some("found"));
        right_stack.set_visible_child_name("saved");

        let right_content = Box::new(Orientation::Vertical, 0);
        right_content.set_vexpand(true);
        right_content.set_margin_top(8);
        right_content.set_margin_bottom(8);
        right_content.set_margin_start(8);
        right_content.set_margin_end(8);
        right_content.append(&right_stack);

        // ═══ RESPONSIVE LAYOUT — side-by-side on wide, stacked on narrow ═══
        let left_clamp = Clamp::new();
        left_clamp.set_maximum_size(600);
        left_clamp.set_vexpand(true);
        left_clamp.set_child(Some(&left_content));

        let right_clamp = Clamp::new();
        right_clamp.set_maximum_size(600);
        right_clamp.set_vexpand(true);
        right_clamp.set_child(Some(&right_content));

        // Horizontal layout for wide screens
        let hbox = Box::new(Orientation::Horizontal, 0);
        hbox.set_hexpand(true);
        hbox.set_vexpand(true);
        hbox.set_homogeneous(true);
        hbox.append(&left_clamp);
        hbox.append(&right_clamp);

        // Use AdwBreakpointBin to switch between horizontal and vertical
        let layout_bin = libadwaita::BreakpointBin::new();
        layout_bin.set_width_request(300);
        layout_bin.set_height_request(200);
        layout_bin.set_child(Some(&hbox));

        // On narrow screens (<720px), switch to vertical stacking
        let narrow_condition = libadwaita::BreakpointCondition::new_length(
            libadwaita::BreakpointConditionLengthType::MaxWidth,
            720.0,
            libadwaita::LengthUnit::Px,
        );
        let narrow_bp = libadwaita::Breakpoint::new(narrow_condition);
        narrow_bp.add_setter(&hbox, "orientation", Some(&Orientation::Vertical.to_value()));
        narrow_bp.add_setter(&hbox, "homogeneous", Some(&false.to_value()));
        narrow_bp.add_setter(&hbox, "spacing", Some(&16u32.to_value()));
        narrow_bp.add_setter(&left_clamp, "maximum-size", Some(&600u32.to_value()));
        narrow_bp.add_setter(&right_clamp, "maximum-size", Some(&600u32.to_value()));
        narrow_bp.add_setter(&left_clamp, "vexpand", Some(&false.to_value()));
        narrow_bp.add_setter(&status_page, "vexpand", Some(&false.to_value()));
        layout_bin.add_breakpoint(narrow_bp);

        let main_scroll = ScrolledWindow::new();
        main_scroll.set_hscrollbar_policy(PolicyType::Never);
        main_scroll.set_vexpand(true);
        main_scroll.set_child(Some(&layout_bin));

        let toolbar_content = Box::new(Orientation::Vertical, 0);
        toolbar_content.append(&disclaimer_revealer);
        toolbar_content.append(&main_scroll);

        let toolbar_view = ToolbarView::new();
        toolbar_view.add_top_bar(&header);
        toolbar_view.set_content(Some(&toolbar_content));
        toast_overlay.set_child(Some(&toolbar_view));

        // ═══ DAEMON CHECK ═══
        let check_daemon = {
            let ds = daemon_status.clone();
            let st = show_toast.clone();
            move || {
                let ds = ds.clone();
                let st = st.clone();
                glib::spawn_future_local(async move {
                    match SpacePodsClient::connect(None).await {
                        Ok(mut c) => {
                            if c.ping().await.unwrap_or(false) {
                                ds.set_text("Service running");
                                ds.add_css_class("success");
                                ds.remove_css_class("error");
                                ds.remove_css_class("warning");
                            } else {
                                ds.set_text("Service unresponsive");
                                ds.add_css_class("warning");
                                ds.remove_css_class("error");
                                ds.remove_css_class("success");
                            }
                        }
                        Err(_) => {
                            ds.set_text("Service not running");
                            ds.add_css_class("error");
                            ds.remove_css_class("warning");
                            ds.remove_css_class("success");
                            st("Start the daemon with: spacepods service");
                        }
                    }
                });
            }
        };
        check_daemon();

        // Populate saved devices
        let on_connected_rc = Rc::new(on_connected.clone());
        let show_error_rc = Rc::new(show_error.clone());

        let rebuild_saved = {
            let saved_list = saved_list.clone();
            let saved_section = saved_section.clone();
            let on_connected_rc = on_connected_rc.clone();
            let show_error_rc = show_error_rc.clone();
            move || {
                while let Some(child) = saved_list.first_child() {
                    saved_list.remove(&child);
                }
                let kd = load_known_devices();
                for dev in &kd {
                    saved_list.append(&Self::saved_row(
                        dev,
                        on_connected_rc.clone(),
                        show_error_rc.clone(),
                    ));
                }
                let has = !kd.is_empty();
                saved_section.set_visible(has);
            }
        };
        rebuild_saved();

        // Store scan results for click handling
        let scanned_data: Rc<RefCell<Vec<ScannedDevice>>> = Rc::new(RefCell::new(Vec::new()));

        // ═══ SCAN BUTTON ═══
        let oc = on_connected_rc.clone();
        let se = show_error_rc.clone();
        scan_btn.connect_clicked(clone!(
            #[weak] scan_btn,
            #[weak] scan_spinner,
            #[weak] scan_status,
            #[weak] found_label,
            #[weak] device_list,
            #[strong] scanned_data,
            #[strong] oc,
            #[strong] se,
            #[strong] check_daemon,
            #[strong] right_stack,
            move |_| {
                scan_btn.set_sensitive(false);
                scan_spinner.set_visible(true);
                scan_spinner.start();
                scan_status.set_text("Scanning…");
                while let Some(child) = device_list.first_child() {
                    device_list.remove(&child);
                }
                scanned_data.borrow_mut().clear();
                right_stack.set_visible_child_name("found");

                glib::spawn_future_local(clone!(
                    #[weak] scan_btn,
                    #[weak] scan_spinner,
                    #[weak] scan_status,
                    #[weak] found_label,
                    #[weak] device_list,
                    #[strong] scanned_data,
                    #[strong] oc,
                    #[strong] se,
                    #[strong] check_daemon,
                    async move {
                        let oc = oc.clone();
                        let se = se.clone();
                        let mut client = match SpacePodsClient::connect(None).await {
                            Ok(c) => c,
                            Err(e) => {
                                scan_spinner.set_visible(false);
                                scan_spinner.stop();
                                scan_btn.set_sensitive(true);
                                se(&e.to_string());
                                check_daemon();
                                return;
                            }
                        };

                        match client.scan(5).await {
                            Err(e) => {
                                se(&e.to_string());
                                scan_status.set_text("");
                            }
                            Ok(devices) if devices.is_empty() => {
                                scan_status.set_text("No devices found — try again");
                                found_label.set_visible(false);
                            }
                            Ok(devices) => {
                                let mut sorted = devices;
                                sorted.sort_by_key(|d| d.rssi.unwrap_or(-100));
                                sorted.reverse();

                                let oc = oc.clone();
                                let se = se.clone();
                                for d in &sorted {
                                    device_list.append(&Self::device_row(d, oc.clone(), se.clone()));
                                }

                                *scanned_data.borrow_mut() = sorted.clone();

                                found_label.set_text(&format!("Found ({})", sorted.len()));
                                found_label.set_visible(true);
                                scan_status.set_text(&format!(
                                    "{} device(s) — sorted by proximity", sorted.len()
                                ));
                            }
                        }
                        scan_spinner.set_visible(false);
                        scan_spinner.stop();
                        scan_btn.set_sensitive(true);
                    }
                ));
            }
        ));

        // ═══ SCANNED DEVICE CLICK → CONNECT ═══
        device_list.connect_row_activated(clone!(
            #[strong] scanned_data,
            #[strong] check_daemon,
            move |_, row| {
                let idx = row.index() as usize;
                let data = scanned_data.borrow();
                if let Some(device) = data.get(idx) {
                    let name = device.name.clone();
                    let address = device.address.clone();
                    let on_connected = on_connected_rc.clone();
                    let show_error = show_error_rc.clone();
                    let check_daemon = check_daemon.clone();
                    glib::spawn_future_local(async move {
                        match SpacePodsClient::connect(None).await {
                            Ok(mut client) => match client.connect_device(address.clone()).await {
                                Ok(_) => {
                                    add_known_device(name, address);
                                    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;
                                    let pid = client.get_status().await.ok().and_then(|s| s.product_id);
                                    (*on_connected)(pid);
                                }
                                Err(e) => { (*show_error)(&e.to_string()); check_daemon(); }
                            },
                            Err(e) => { (*show_error)(&e.to_string()); check_daemon(); }
                        }
                    });
                }
            }
        ));

        toast_overlay.upcast()
    }

    // ── Device row (scanned) ──
    fn device_row(
        device: &ScannedDevice,
        on_connected: Rc<impl Fn(Option<u16>) + 'static>,
        show_error: Rc<impl Fn(&str) + 'static>,
    ) -> ListBoxRow {
        let row = ListBoxRow::new();
        row.add_css_class("activatable");

        let outer = Box::new(Orientation::Vertical, 4);
        outer.set_margin_top(10);
        outer.set_margin_bottom(10);
        outer.set_margin_start(14);
        outer.set_margin_end(14);

        let line1 = Box::new(Orientation::Horizontal, 8);
        let product = device.product_name.as_deref().unwrap_or("SpaceBuds");
        let display = if product == "Oraimo SpaceBuds" || product.is_empty() {
            device.name.clone()
        } else {
            product.to_string()
        };
        let name_lbl = Label::new(Some(&display));
        name_lbl.set_hexpand(true);
        name_lbl.set_halign(gtk4::Align::Start);
        name_lbl.add_css_class("heading");
        line1.append(&name_lbl);

        if let Some(_r) = device.rssi {
            let dist = Label::new(Some(rssi_label(device.rssi)));
            dist.add_css_class("caption");
            dist.add_css_class(rssi_css(device.rssi));
            line1.append(&dist);
        }
        if device.already_connected {
            let warn = Label::new(Some("In use"));
            warn.add_css_class("warning");
            warn.add_css_class("caption");
            line1.append(&warn);
        }
        outer.append(&line1);

        let line2 = Box::new(Orientation::Horizontal, 8);
        let ble_info = if device.name != display {
            device.name.clone()
        } else {
            device.address.clone()
        };
        let addr_lbl = Label::new(Some(&ble_info));
        addr_lbl.add_css_class("dim-label");
        addr_lbl.add_css_class("caption");
        addr_lbl.set_hexpand(true);
        addr_lbl.set_halign(gtk4::Align::Start);
        line2.append(&addr_lbl);

        // Connect button + spinner
        let conn_spinner = Spinner::new();
        conn_spinner.set_visible(false);
        conn_spinner.set_halign(Align::Center);
        let conn_btn = Button::with_label("Connect");
        conn_btn.add_css_class("suggested-action");
        conn_btn.add_css_class("pill");
        conn_btn.set_valign(Align::Center);
        line2.append(&conn_spinner);
        line2.append(&conn_btn);

        {
            let name = device.name.clone();
            let address = device.address.clone();
            let spinner = conn_spinner.clone();
            let btn = conn_btn.clone();
            let on_connected = on_connected.clone();
            let show_error = show_error.clone();
            conn_btn.connect_clicked(move |_| {
                spinner.set_visible(true);
                btn.set_sensitive(false);
                btn.set_label("Connecting…");
                let name = name.clone();
                let address = address.clone();
                let spinner = spinner.clone();
                let btn = btn.clone();
                let on_connected = on_connected.clone();
                let show_error = show_error.clone();
                glib::spawn_future_local(async move {
                    match SpacePodsClient::connect(None).await {
                        Ok(mut client) => match client.connect_device(address.clone()).await {
                            Ok(_) => {
                                add_known_device(name, address);
                                tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;
                                let pid = client.get_status().await.ok().and_then(|s| s.product_id);
                                (*on_connected)(pid);
                            }
                            Err(e) => {
                                (*show_error)(&e.to_string());
                                spinner.set_visible(false);
                                btn.set_sensitive(true);
                                btn.set_label("Connect");
                            }
                        },
                        Err(e) => {
                            (*show_error)(&e.to_string());
                            spinner.set_visible(false);
                            btn.set_sensitive(true);
                            btn.set_label("Connect");
                        }
                    }
                });
            });
        }

        if device.battery_left.is_some() || device.battery_right.is_some() {
            let mut s = format!("L:{}% / R:{}%",
                device.battery_left.map_or("?".into(), |b| b.to_string()),
                device.battery_right.map_or("?".into(), |b| b.to_string()),
            );
            if let Some(c) = device.battery_case {
                s = format!("{}  C:{}%", s, c);
            }
            let bat = Label::new(Some(&s));
            bat.add_css_class("caption");
            bat.add_css_class("accent");
            line2.append(&bat);
        }
        if let Some(v) = device.beacon_version {
            let ver = Label::new(Some(&format!("V{}", v)));
            ver.add_css_class("caption");
            ver.add_css_class("dim-label");
            line2.append(&ver);
        }
        outer.append(&line2);
        row.set_child(Some(&outer));
        row
    }

    // ── Saved device row ──
    fn saved_row(
        dev: &crate::storage::KnownDevice,
        on_connected: Rc<impl Fn(Option<u16>) + 'static>,
        show_error: Rc<impl Fn(&str) + 'static>,
    ) -> ListBoxRow {
        let row = ListBoxRow::new();
        row.set_activatable(false);

        let outer = Box::new(Orientation::Vertical, 4);
        outer.set_margin_top(10);
        outer.set_margin_bottom(10);
        outer.set_margin_start(14);
        outer.set_margin_end(14);

        let line1 = Box::new(Orientation::Horizontal, 8);
        let name_lbl = Label::new(Some(&dev.name));
        name_lbl.set_hexpand(true);
        name_lbl.set_halign(gtk4::Align::Start);
        name_lbl.add_css_class("heading");
        line1.append(&name_lbl);

        let time_lbl = Label::new(Some(&format_last_used(dev.last_connected)));
        time_lbl.add_css_class("caption");
        time_lbl.add_css_class("accent");
        line1.append(&time_lbl);

        outer.append(&line1);

        let line2 = Box::new(Orientation::Horizontal, 8);
        let addr_lbl = Label::new(Some(&dev.address));
        addr_lbl.add_css_class("dim-label");
        addr_lbl.add_css_class("caption");
        addr_lbl.set_hexpand(true);
        addr_lbl.set_halign(gtk4::Align::Start);
        line2.append(&addr_lbl);

        // Delete button
        let del_btn = Button::from_icon_name("user-trash-symbolic");
        del_btn.add_css_class("flat");
        del_btn.add_css_class("circular");
        del_btn.set_valign(Align::Center);
        del_btn.set_tooltip_text(Some("Forget this device"));
        line2.append(&del_btn);

        // Connect button + spinner
        let conn_spinner = Spinner::new();
        conn_spinner.set_visible(false);
        conn_spinner.set_halign(Align::Center);
        let conn_btn = Button::with_label("Connect");
        conn_btn.add_css_class("suggested-action");
        conn_btn.add_css_class("pill");
        conn_btn.set_valign(Align::Center);
        line2.append(&conn_spinner);
        line2.append(&conn_btn);

        outer.append(&line2);
        row.set_child(Some(&outer));

        // Wire delete
        let addr = dev.address.clone();
        let row_weak = row.downgrade();
        del_btn.connect_clicked(move |_| {
            remove_known_device(&addr);
            if let Some(r) = row_weak.upgrade() {
                if let Some(parent) = r.parent() {
                    if let Ok(list) = parent.downcast::<ListBox>() {
                        list.remove(&r);
                    }
                }
            }
        });

        // Wire connect
        let name = dev.name.clone();
        let address = dev.address.clone();
        {
            let spinner = conn_spinner.clone();
            let btn = conn_btn.clone();
            let on_connected = on_connected.clone();
            let show_error = show_error.clone();
            conn_btn.connect_clicked(move |_| {
                spinner.set_visible(true);
                btn.set_sensitive(false);
                btn.set_label("Connecting…");
                let name = name.clone();
                let address = address.clone();
                let spinner = spinner.clone();
                let btn = btn.clone();
                let on_connected = on_connected.clone();
                let show_error = show_error.clone();
            glib::spawn_future_local(async move {
                match SpacePodsClient::connect(None).await {
                    Ok(mut client) => match client.connect_device(address.clone()).await {
                        Ok(_) => {
                            add_known_device(name, address);
                            tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;
                            let pid = client.get_status().await.ok().and_then(|s| s.product_id);
                            (*on_connected)(pid);
                        }
                        Err(e) => {
                            (*show_error)(&e.to_string());
                            spinner.set_visible(false);
                            btn.set_sensitive(true);
                            btn.set_label("Connect");
                        }
                    },
                    Err(e) => {
                        (*show_error)(&e.to_string());
                        spinner.set_visible(false);
                        btn.set_sensitive(true);
                        btn.set_label("Connect");
                    }
                }
            });
        });
        }

        row
    }
}
