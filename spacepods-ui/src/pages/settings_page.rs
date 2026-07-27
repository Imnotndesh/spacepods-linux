use gtk4::prelude::*;
use gtk4::{Box, Label, Orientation, ScrolledWindow};
use libadwaita::prelude::*;
use libadwaita::{ActionRow, Clamp, PreferencesGroup, SwitchRow};

use crate::storage::{load_settings, update_settings};
use crate::service::{self, check_daemon_running};

pub struct SettingsPage;

impl SettingsPage {
    pub fn page() -> gtk4::Widget {
        let saved_settings = load_settings();

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
            service::write_autostart_entry(enabled);
        });

        general_group.add(&autostart_row);

        let service_group = PreferencesGroup::new();
        service_group.set_title("Background Service");
        service_group.set_description(Some(
            "The SpacePods daemon (libspacepods) handles Bluetooth communication. \
             Start it from a terminal: `libspacepods service`",
        ));

        let service_row = ActionRow::new();
        service_row.set_title("SpacePods daemon");

        let service_status = Label::new(Some("Checking…"));
        service_status.add_css_class("dim-label");
        service_status.add_css_class("caption");
        service_status.set_valign(gtk4::Align::Center);

        // Check daemon status on open
        {
            let status_label = service_status.clone();
            glib::spawn_future_local(async move {
                let running = check_daemon_running().await;
                if running {
                    status_label.set_text("Running");
                    status_label.remove_css_class("dim-label");
                    status_label.remove_css_class("error");
                    status_label.add_css_class("success");
                } else {
                    status_label.set_text("Not running");
                    status_label.remove_css_class("success");
                    status_label.remove_css_class("dim-label");
                    status_label.add_css_class("error");
                }
            });
        }

        service_row.add_suffix(&service_status);
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
        source_row.set_subtitle("github.com/Imnotndesh/spacepods-linux");
        source_row.set_activatable(true);
        source_row.connect_activated(|_| {
            let _ = gtk4::UriLauncher::new("https://github.com/Imnotndesh/spacepods-linux")
                .launch(gtk4::Window::NONE, gio::Cancellable::NONE, |_| {});
        });
        let chevron = gtk4::Image::from_icon_name("go-next-symbolic");
        chevron.add_css_class("dim-label");
        source_row.add_suffix(&chevron);

        about_group.add(&version_row);
        about_group.add(&source_row);

        // ── Device Actions ──
        let device_group = PreferencesGroup::new();
        device_group.set_title("Device");
        device_group.set_description(Some("Actions that affect your connected SpaceBuds"));

        let rename_row = ActionRow::new();
        rename_row.set_title("Change Bluetooth Name");
        rename_row.set_subtitle("Rename your device as seen by other devices");
        rename_row.set_activatable(true);
        rename_row.add_suffix(&gtk4::Image::from_icon_name("go-next-symbolic"));
        device_group.add(&rename_row);

        let clear_pair_row = ActionRow::new();
        clear_pair_row.set_title("Clear Pairing Record");
        clear_pair_row.set_subtitle("Remove all paired devices from the earbuds");
        clear_pair_row.set_activatable(true);
        clear_pair_row.add_suffix(&gtk4::Image::from_icon_name("go-next-symbolic"));
        device_group.add(&clear_pair_row);

        let clear_status = Label::new(None);
        clear_status.add_css_class("caption");
        clear_status.add_css_class("dim-label");
        clear_status.set_halign(gtk4::Align::Center);
        clear_status.set_visible(false);
        device_group.add(&clear_status);

        content.append(&general_group);
        content.append(&service_group);
        content.append(&device_group);
        content.append(&about_group);
        clamp.set_child(Some(&content));

        let scroll = ScrolledWindow::new();
        scroll.set_hscrollbar_policy(gtk4::PolicyType::Never);
        scroll.set_vexpand(true);
        scroll.set_child(Some(&clamp));

        // ── Clear Pairing Record handler ──
        {
            let clear_status = clear_status.clone();
            clear_pair_row.connect_activated(move |_| {
                let cs = clear_status.clone();
                cs.set_text("Clearing pairing records…");
                cs.set_visible(true);
                glib::spawn_future_local(async move {
                    use libspacepods::client::SpacePodsClient;
                    let cc = libspacepods::ipc::ServiceCommand::Custom { command_id: 0x2F, payload: vec![] };
                    match SpacePodsClient::connect(None).await {
                        Ok(mut client) => match client.send_command_raw(cc).await {
                            Ok(_) => cs.set_text("Pairing records cleared. Device will restart."),
                            Err(e) => cs.set_text(&format!("Failed: {}", e)),
                        },
                        Err(e) => cs.set_text(&format!("Service unreachable: {}", e)),
                    }
                });
            });
        }

        // ── Rename handler ──
        rename_row.connect_activated(move |_| {
            // Open an input dialog — for now use a simple approach
            let dialog = gtk4::Dialog::new();
            dialog.set_title(Some("Rename Device"));
            dialog.set_modal(true);
            let entry = gtk4::Entry::new();
            entry.set_placeholder_text(Some("Enter new Bluetooth name…"));
            entry.set_margin_top(12);
            entry.set_margin_bottom(12);
            entry.set_margin_start(12);
            entry.set_margin_end(12);
            dialog.content_area().append(&entry);
            dialog.add_button("Cancel", gtk4::ResponseType::Cancel);
            dialog.add_button("Rename", gtk4::ResponseType::Accept);
            dialog.connect_response(move |d, resp| {
                if resp == gtk4::ResponseType::Accept {
                    let name = entry.text().to_string();
                    if !name.is_empty() {
                        let payload: Vec<u8> = name.bytes().collect();
                        let cc = libspacepods::ipc::ServiceCommand::Custom { command_id: 0x2D, payload };
                        glib::spawn_future_local(async move {
                            use libspacepods::client::SpacePodsClient;
                            if let Ok(mut client) = SpacePodsClient::connect(None).await {
                                let _ = client.send_command_raw(cc).await;
                            }
                        });
                    }
                }
                d.close();
            });
            dialog.present();
        });

        scroll.upcast()
    }
}
