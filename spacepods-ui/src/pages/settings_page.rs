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

        content.append(&general_group);
        content.append(&service_group);
        content.append(&about_group);
        clamp.set_child(Some(&content));

        let scroll = ScrolledWindow::new();
        scroll.set_hscrollbar_policy(gtk4::PolicyType::Never);
        scroll.set_vexpand(true);
        scroll.set_child(Some(&clamp));

        scroll.upcast()
    }
}
