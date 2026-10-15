use gtk4::prelude::*;
use gtk4::{Box, Label, Orientation};
use libadwaita::prelude::*;
use libadwaita::{PreferencesGroup, ActionRow, Clamp, SwitchRow};
use std::rc::Rc;

use crate::context::AppContext;
use crate::errors::user_text;

use crate::prefs;
use crate::autostart::{self, Outcome};
use crate::service;

pub struct SettingsPage;

impl SettingsPage {
    pub fn page(ctx: Rc<AppContext>, on_disconnect: impl Fn() + 'static + Clone) -> gtk4::Widget {

        // Header
        // General
        let general_group = PreferencesGroup::new();
        general_group.set_title("General");

        let autostart_row = SwitchRow::new();
        autostart_row.set_title("Start on login");
        autostart_row.set_subtitle("Launch SpacePods automatically at login");
        autostart_row.set_active(prefs::get_bool(prefs::AUTOSTART));
        // Set while we put the switch back after a refusal, so that does not
        // trigger another request.
        let reverting = Rc::new(std::cell::Cell::new(false));
        let autostart_ctx = ctx.clone();
        autostart_row.connect_active_notify(move |row| {
            if reverting.get() {
                return;
            }
            let wanted = row.is_active();
            let row = row.clone();
            let reverting = reverting.clone();
            let ctx = autostart_ctx.clone();
            glib::spawn_future_local(async move {
                let revert = |row: &SwitchRow| {
                    reverting.set(true);
                    row.set_active(!wanted);
                    reverting.set(false);
                };
                match autostart::set_enabled(wanted).await {
                    Ok(Outcome::Enabled) => prefs::set_bool(prefs::AUTOSTART, true),
                    Ok(Outcome::Disabled) => prefs::set_bool(prefs::AUTOSTART, false),
                    Ok(Outcome::Denied) => {
                        revert(&row);
                        ctx.toast("SpacePods wasn't allowed to start on login.");
                    }
                    Err(e) => {
                        revert(&row);
                        crate::log::Log::warn("AUTOSTART", &e);
                        ctx.error("Couldn't change start on login. Try again.");
                    }
                }
            });
        });
        general_group.add(&autostart_row);

        // Graphics renderer. GTK reads it once at startup, so the change applies next time.
        const RENDERERS: [(&str, &str); 3] = [
            ("cairo", "Cairo (least memory)"),
            ("vulkan", "Vulkan (GPU)"),
            ("gl", "OpenGL (GPU)"),
        ];
        let renderer_row = libadwaita::ComboRow::new();
        renderer_row.set_title("Graphics renderer");
        renderer_row.set_subtitle("Takes effect the next time SpacePods starts");
        let labels = gtk4::StringList::new(&RENDERERS.map(|(_, label)| label));
        renderer_row.set_model(Some(&labels));
        let current = prefs::get_string(prefs::RENDERER);
        let selected = RENDERERS.iter().position(|(id, _)| *id == current).unwrap_or(0);
        renderer_row.set_selected(selected as u32);
        renderer_row.connect_selected_notify(|row| {
            if let Some((id, _)) = RENDERERS.get(row.selected() as usize) {
                prefs::set_string(prefs::RENDERER, id);
            }
        });
        general_group.add(&renderer_row);


        // Service
        let service_group = PreferencesGroup::new();
        service_group.set_title("Background Service");
        service_group.set_description(Some(
            "SpacePods talks to your earbuds through a background service. \
             It starts on its own when SpacePods opens.",
        ));

        let service_row = ActionRow::new();
        service_row.set_title("SpacePods daemon");
        let service_status = Label::new(Some("Checking…"));
        service_status.add_css_class("dim-label");
        service_status.add_css_class("caption");
        service_status.set_valign(gtk4::Align::Center);
        service_row.add_suffix(&service_status);
        service_group.add(&service_row);

        {
            let status_label = service_status.clone();
            glib::spawn_future_local(async move {
                let running = service::check_daemon_running().await;
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

        // Device
        let device_group = PreferencesGroup::new();
        device_group.set_title("Device");
        device_group.set_description(Some("Actions that affect your connected SpaceBuds"));

        let disconnect_row = ActionRow::new();
        disconnect_row.set_title("Disconnect");
        disconnect_row.set_subtitle("Disconnect from the current device and return to setup");
        disconnect_row.set_activatable(true);
        disconnect_row.add_suffix(&gtk4::Image::from_icon_name("go-next-symbolic"));
        device_group.add(&disconnect_row);

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

        // About
        let about_group = PreferencesGroup::new();
        about_group.set_title("About");

        let version_row = ActionRow::new();
        version_row.set_title("Version");
        let version_label = Label::new(Some(libspacepods::VERSION));
        version_label.add_css_class("dim-label");
        version_label.set_valign(gtk4::Align::Center);
        version_row.add_suffix(&version_label);
        about_group.add(&version_row);

        let about_row = ActionRow::new();
        about_row.set_title("About SpacePods");
        about_row.set_subtitle("Version, license and credits");
        about_row.set_activatable(true);
        let about_chevron = gtk4::Image::from_icon_name("go-next-symbolic");
        about_chevron.add_css_class("dim-label");
        about_row.add_suffix(&about_chevron);
        about_row.connect_activated(move |row| {
            if let Some(win) = row.root().and_then(|r| r.downcast::<gtk4::Window>().ok()) {
                crate::app::show_about(&win);
            }
        });
        about_group.add(&about_row);

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
        about_group.add(&source_row);

        let content = Box::new(Orientation::Vertical, 12);
        content.set_margin_top(0);
        content.set_margin_bottom(32);
        content.set_margin_start(16);
        content.set_margin_end(16);
        content.append(&general_group);
        content.append(&service_group);
        content.append(&device_group);
        content.append(&about_group);

        // Handlers
        disconnect_row.connect_activated({
            let on_disconnect = on_disconnect.clone();
            let ctx = ctx.clone();
            move |_| {
                let on_disconnect = on_disconnect.clone();
                let ctx = ctx.clone();
                glib::spawn_future_local(async move {
                    use libspacepods::client::SpacePodsClient;
                    // Only return to setup once the daemon has confirmed the disconnect.
                    match SpacePodsClient::connect(None).await {
                        Ok(mut client) => match client.disconnect_device().await {
                            Ok(()) => on_disconnect(),
                            Err(e) => ctx.failed("Couldn't disconnect", &e),
                        },
                        Err(e) => ctx.daemon_unreachable(e),
                    }
                });
            }
        });

        let rename_ctx = ctx.clone();
        rename_row.connect_activated(move |row| {
            let parent = row.root().and_then(|r| r.downcast::<gtk4::Window>().ok());
            let dialog = libadwaita::AlertDialog::new(
                Some("Rename Device"),
                Some("Choose the name other devices will see when pairing."),
            );
            let entry = libadwaita::EntryRow::new();
            entry.set_title("Bluetooth name");
            dialog.set_extra_child(Some(&entry));
            dialog.add_response("cancel", "Cancel");
            dialog.add_response("rename", "Rename");
            dialog.set_response_appearance("rename", libadwaita::ResponseAppearance::Suggested);
            dialog.set_default_response(Some("rename"));
            dialog.set_close_response("cancel");
            {
                let entry = entry.clone();
                let ctx = rename_ctx.clone();
                dialog.connect_response(None, move |_, response| {
                    if response != "rename" {
                        return;
                    }
                    let name = entry.text().to_string();
                    if name.is_empty() {
                        return;
                    }
                    let ctx = ctx.clone();
                    glib::spawn_future_local(async move {
                        use libspacepods::client::SpacePodsClient;
                        match SpacePodsClient::connect(None).await {
                            Ok(mut client) => {
                                if let Err(e) = client.rename(&name).await {
                                    ctx.failed("Couldn't rename the earbuds", &e);
                                }
                            }
                            Err(e) => ctx.daemon_unreachable(e),
                        }
                    });
                });
            }
            entry.grab_focus();
            dialog.present(parent.as_ref());
        });

        {
            let clear_status = clear_status.clone();
            clear_pair_row.connect_activated(move |row| {
                let parent = row.root().and_then(|r| r.downcast::<gtk4::Window>().ok());
                let dialog = libadwaita::AlertDialog::new(
                    Some("Clear pairing records?"),
                    Some("This removes all paired devices from the earbuds and restarts them."),
                );
                dialog.add_response("cancel", "Cancel");
                dialog.add_response("clear", "Clear");
                dialog.set_response_appearance("clear", libadwaita::ResponseAppearance::Destructive);
                dialog.set_close_response("cancel");
                {
                    let cs = clear_status.clone();
                    dialog.connect_response(None, move |_, response| {
                        if response != "clear" {
                            return;
                        }
                        let cs = cs.clone();
                        cs.set_text("Clearing pairing records…");
                        cs.set_visible(true);
                        glib::spawn_future_local(async move {
                            use libspacepods::client::SpacePodsClient;
                            let cc = libspacepods::ipc::ServiceCommand::Custom {
                                command_id: 0x2F, payload: vec![],
                            };
                            match SpacePodsClient::connect(None).await {
                                Ok(mut client) => match client.send_command_raw(cc).await {
                                    Ok(_) => cs.set_text("Pairing records cleared. Device will restart."),
                                    Err(e) => cs.set_text(&format!("Couldn't clear pairing: {}", user_text(&e))),
                                },
                                Err(e) => cs.set_text(&format!("Couldn't clear pairing: {}", user_text(&e))),
                            }
                        });
                    });
                }
                dialog.present(parent.as_ref());
            });
        }

        let clamp = Clamp::new();
        clamp.set_maximum_size(500);
        clamp.set_child(Some(&content));
        clamp.upcast()
    }
}
