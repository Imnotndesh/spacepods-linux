use glib::clone;
use gtk4::prelude::*;
use gtk4::{Box, Label, Orientation, ScrolledWindow, Switch};
use libadwaita::prelude::*;
use libadwaita::{
    ActionRow, Clamp, HeaderBar, NavigationPage, PreferencesGroup, SwitchRow,
};
use crate::storage::{load_known_devices, rename_known_device};
use std::cell::Cell;
use std::rc::Rc;

use crate::tray::{TrayCommand, TrayHandle};
use crate::storage::{load_settings, update_settings};

pub struct SettingsPage;

impl SettingsPage {
    pub fn navigation_page(tray_handle: Option<TrayHandle>) -> NavigationPage {
        let tray_handle = Rc::new(tray_handle);

        let tray_enabled = Rc::new(Cell::new(false));
        let close_to_tray = Rc::new(Cell::new(false));

        // Load saved settings
        let saved_settings = load_settings();

        let header = HeaderBar::new();
        let title_widget = libadwaita::WindowTitle::new("Settings", "");
        header.set_title_widget(Some(&title_widget));
        let clamp = Clamp::new();
        clamp.set_maximum_size(600);
        clamp.set_tightening_threshold(480);

        let content = Box::new(Orientation::Vertical, 24);
        content.set_margin_top(16);
        content.set_margin_bottom(32);
        content.set_margin_start(16);
        content.set_margin_end(16);
        let general_group = PreferencesGroup::new();
        general_group.set_title("General");

        let autostart_row = SwitchRow::new();
        autostart_row.set_title("Start on login");
        autostart_row.set_subtitle("Launch SpacePods automatically at login");
        autostart_row.set_active(saved_settings.autostart);
        autostart_row.connect_active_notify(|row| {
            let enabled = row.is_active();
            update_settings(|s| s.autostart = enabled);
            write_autostart_entry(enabled);
        });

        general_group.add(&autostart_row);
        let tray_group = PreferencesGroup::new();
        tray_group.set_title("System Tray");
        tray_group.set_description(Some(
            "Requires a desktop environment that supports the StatusNotifierItem specification \
             (GNOME Shell with AppIndicator extension, KDE Plasma, etc.)",
        ));

        let tray_row = SwitchRow::new();
        tray_row.set_title("Enable tray icon");
        tray_row.set_subtitle("Show SpacePods in the system notification area");
        tray_row.set_active(saved_settings.tray_enabled);
        let close_tray_row = SwitchRow::new();
        close_tray_row.set_title("Minimise to tray on close");
        close_tray_row.set_subtitle("Closing the window hides it instead of quitting");
        close_tray_row.set_active(saved_settings.close_to_tray);
        close_tray_row.set_sensitive(saved_settings.tray_enabled);

        {
            let close_tray_row_ref = close_tray_row.clone();
            let tray_enabled_ref = tray_enabled.clone();
            let tray_handle_ref = tray_handle.clone();
            tray_row.connect_active_notify(clone!(
                #[weak] close_tray_row_ref,
                move |row| {
                    let enabled = row.is_active();
                    tray_enabled_ref.set(enabled);
                    close_tray_row_ref.set_sensitive(enabled);
                    update_settings(|s| s.tray_enabled = enabled);

                    if let Some(ref handle) = *tray_handle_ref {
                        if enabled {
                            handle.send(TrayCommand::Show);
                        } else {
                            handle.send(TrayCommand::Hide);
                            close_tray_row_ref.set_active(false);
                        }
                    }
                }
            ));
        }
        {
            let close_to_tray_ref = close_to_tray.clone();
            close_tray_row.connect_active_notify(move |row| {
                let enabled = row.is_active();
                close_to_tray_ref.set(enabled);
                update_settings(|s| s.close_to_tray = enabled);
            });
        }

        tray_group.add(&tray_row);
        tray_group.add(&close_tray_row);
        let service_group = PreferencesGroup::new();
        service_group.set_title("Background Service");
        service_group.set_description(Some(
            "The SpacePods daemon handles Bluetooth communication. \
             It must be running for the app to function.",
        ));

        let service_row = ActionRow::new();
        service_row.set_title("SpacePods daemon");
        service_row.set_subtitle("Manages the BLE connection to your earbuds");

        let service_status = Label::new(Some("Checking…"));
        service_status.add_css_class("dim-label");
        service_status.add_css_class("caption");
        service_status.set_valign(gtk4::Align::Center);

        let start_btn = gtk4::Button::with_label("Start");
        start_btn.add_css_class("suggested-action");
        start_btn.add_css_class("pill");
        start_btn.set_valign(gtk4::Align::Center);

        let stop_btn = gtk4::Button::with_label("Stop");
        stop_btn.add_css_class("destructive-action");
        stop_btn.add_css_class("pill");
        stop_btn.set_valign(gtk4::Align::Center);
        stop_btn.set_visible(false);

        let restart_btn = gtk4::Button::with_label("Restart");
        restart_btn.add_css_class("pill");
        restart_btn.set_valign(gtk4::Align::Center);
        restart_btn.set_visible(false);

        // Check actual daemon status on open
        {
            let service_status_r = service_status.clone();
            let start_btn_r = start_btn.clone();
            let stop_btn_r = stop_btn.clone();
            let restart_btn_r = restart_btn.clone();
            glib::spawn_future_local(async move {
                let running = check_daemon_running().await;
                update_daemon_ui(&service_status_r, &start_btn_r, &stop_btn_r, &restart_btn_r, running);
            });
        }

        {
            let service_status_r = service_status.clone();
            let start_btn_r = start_btn.clone();
            let stop_btn_r = stop_btn.clone();
            let restart_btn_r = restart_btn.clone();
            start_btn.connect_clicked(move |_| {
                let ss = service_status_r.clone();
                let sb = start_btn_r.clone();
                let stb = stop_btn_r.clone();
                let rb = restart_btn_r.clone();
                glib::spawn_future_local(async move {
                    spawn_daemon();
                    tokio::time::sleep(tokio::time::Duration::from_millis(900)).await;
                    let running = check_daemon_running().await;
                    update_daemon_ui(&ss, &sb, &stb, &rb, running);
                });
            });
        }
        {
            let service_status_r = service_status.clone();
            let start_btn_r = start_btn.clone();
            let stop_btn_r = stop_btn.clone();
            let restart_btn_r = restart_btn.clone();
            stop_btn.connect_clicked(move |_| {
                kill_daemon();
                let ss = service_status_r.clone();
                let sb = start_btn_r.clone();
                let stb = stop_btn_r.clone();
                let rb = restart_btn_r.clone();
                glib::spawn_future_local(async move {
                    tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;
                    let running = check_daemon_running().await;
                    update_daemon_ui(&ss, &sb, &stb, &rb, running);
                });
            });
        }
        {
            let service_status_r = service_status.clone();
            let start_btn_r = start_btn.clone();
            let stop_btn_r = stop_btn.clone();
            let restart_btn_r = restart_btn.clone();
            restart_btn.connect_clicked(move |_| {
                kill_daemon();
                let ss = service_status_r.clone();
                let sb = start_btn_r.clone();
                let stb = stop_btn_r.clone();
                let rb = restart_btn_r.clone();
                glib::spawn_future_local(async move {
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    spawn_daemon();
                    tokio::time::sleep(tokio::time::Duration::from_millis(900)).await;
                    let running = check_daemon_running().await;
                    update_daemon_ui(&ss, &sb, &stb, &rb, running);
                });
            });
        }

        service_row.add_suffix(&service_status);
        service_row.add_suffix(&start_btn);
        service_row.add_suffix(&stop_btn);
        service_row.add_suffix(&restart_btn);
        service_group.add(&service_row);
        let about_group = PreferencesGroup::new();
        about_group.set_title("About");

        let version_row = ActionRow::new();
        version_row.set_title("Version");
        let version_label = Label::new(Some(env!("CARGO_PKG_VERSION")));
        version_label.add_css_class("dim-label");
        version_label.set_valign(gtk4::Align::Center);
        version_row.add_suffix(&version_label);

        let source_row = ActionRow::new();
        source_row.set_title("Source code");
        source_row.set_subtitle("github.com/your-user/spacepods");
        source_row.set_activatable(true);
        source_row.connect_activated(|_| {
            let _ = gtk4::UriLauncher::new("https://github.com/Imnotndesh/spacepods-linux")
                .launch(gtk4::Window::NONE, gio::Cancellable::NONE, |_| {});
        });
        let chevron = gtk4::Image::from_icon_name("go-next-symbolic");
        chevron.add_css_class("dim-label");
        source_row.add_suffix(&chevron);
        use crate::storage::{load_known_devices, rename_known_device};
        let devices_group = PreferencesGroup::new();
        devices_group.set_title("Saved Devices");
        devices_group.set_description(Some("Tap a device name to rename it"));

        for device in load_known_devices() {
            let row = libadwaita::EntryRow::new();
            row.set_title(&device.address);
            row.set_text(&device.name);
            row.set_show_apply_button(true);

            let address = device.address.clone();
            row.connect_apply(move |r| {
                let new_name = r.text().trim().to_string();
                if !new_name.is_empty() {
                    rename_known_device(&address, new_name);
                }
            });

            devices_group.add(&row);
        }

        about_group.add(&version_row);
        about_group.add(&source_row);
        content.append(&general_group);
        content.append(&devices_group);
        content.append(&service_group);
        content.append(&about_group);
        clamp.set_child(Some(&content));

        let scroll = ScrolledWindow::new();
        scroll.set_hscrollbar_policy(gtk4::PolicyType::Never);
        scroll.set_vexpand(true);
        scroll.set_child(Some(&clamp));
        let toolbar_view = libadwaita::ToolbarView::new();
        toolbar_view.add_top_bar(&header);
        toolbar_view.set_content(Some(&scroll));

        let page = NavigationPage::new(&toolbar_view, "Settings");
        page
    }
}

fn write_autostart_entry(enable: bool) {
    let path = glib::user_config_dir()
        .join("autostart")
        .join("spacepods.desktop");

    if enable {
        let content = "[Desktop Entry]\n\
            Type=Application\n\
            Name=SpacePods\n\
            Exec=spacepods\n\
            Icon=audio-headset\n\
            Comment=SpacePods earbuds manager\n\
            X-GNOME-Autostart-enabled=true\n";
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&path, content);
    } else {
        let _ = std::fs::remove_file(&path);
    }
}
fn update_daemon_ui(
    status: &Label,
    start: &gtk4::Button,
    stop: &gtk4::Button,
    restart: &gtk4::Button,
    running: bool,
) {
    if running {
        status.set_text("Running");
        status.remove_css_class("dim-label");
        status.remove_css_class("error");
        status.add_css_class("success");
        start.set_visible(false);
        stop.set_visible(true);
        restart.set_visible(true);
    } else {
        status.set_text("Stopped");
        status.remove_css_class("success");
        status.remove_css_class("dim-label");
        status.add_css_class("error");
        start.set_visible(true);
        stop.set_visible(false);
        restart.set_visible(false);
    }
}

async fn check_daemon_running() -> bool {
    libspacepods::client::SpacePodsClient::connect(None).await.is_ok()
}

fn spawn_daemon() {
    let exe = std::env::current_exe().unwrap_or_else(|_| "spacepods".into());
    let _ = std::process::Command::new(exe)
        .arg("service")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

fn kill_daemon() {
    let _ = std::process::Command::new("pkill")
        .args(["-f", "spacepods service"])
        .spawn();
}