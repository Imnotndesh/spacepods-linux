use gtk4::prelude::*;
use gtk4::{Box, Label, Orientation, Spinner, Button};
use libadwaita::{StatusPage, ToolbarView, HeaderBar, WindowTitle};
use glib::clone;
use libspacepods::client::SpacePodsClient;
use crate::storage::{get_last_connected_device, add_known_device, load_settings};
use std::rc::Rc;

pub struct LoadingPage;

#[derive(Clone)]
pub enum LoadingOutcome {
    Connected,
    NoDevice,
    Retry,
}

impl LoadingPage {
    pub fn new(on_complete: Rc<dyn Fn(LoadingOutcome)>) -> ToolbarView {
        let header = HeaderBar::new();
        let title_widget = WindowTitle::new("SpacePods", "Starting…");
        header.set_title_widget(Some(&title_widget));

        let status_page = StatusPage::new();
        status_page.set_icon_name(Some("audio-headset-symbolic"));
        status_page.set_title("Starting SpacePods");
        status_page.set_description(Some("Checking service and connecting to your earbuds…"));
        status_page.set_vexpand(true);

        let spinner = Spinner::new();
        spinner.set_size_request(32, 32);
        spinner.set_halign(gtk4::Align::Center);
        spinner.start();

        let info_label = Label::new(Some("Initialising…"));
        info_label.add_css_class("dim-label");
        info_label.set_halign(gtk4::Align::Center);

        let error_label = Label::new(None);
        error_label.add_css_class("error");
        error_label.set_halign(gtk4::Align::Center);
        error_label.set_visible(false);

        let retry_button = Button::with_label("Retry");
        retry_button.add_css_class("suggested-action");
        retry_button.set_visible(false);

        let setup_button = Button::with_label("Go to Setup");
        setup_button.set_visible(false);

        let vbox = Box::new(Orientation::Vertical, 12);
        vbox.set_halign(gtk4::Align::Center);
        vbox.set_valign(gtk4::Align::Center);
        vbox.set_margin_top(24);
        vbox.append(&status_page);
        vbox.append(&spinner);
        vbox.append(&info_label);
        vbox.append(&error_label);
        vbox.append(&retry_button);
        vbox.append(&setup_button);

        let content = libadwaita::Clamp::new();
        content.set_child(Some(&vbox));

        let toolbar_view = ToolbarView::new();
        toolbar_view.add_top_bar(&header);
        toolbar_view.set_content(Some(&content));

        glib::spawn_future_local(clone!(
            #[strong] on_complete,
            #[strong] info_label,
            #[strong] error_label,
            #[strong] retry_button,
            #[strong] setup_button,
            #[strong] spinner,
            async move {
                let outcome = Self::run_checks(&info_label).await;
                spinner.stop();
                match outcome {
                    Ok(LoadingOutcome::Connected) => {
                        on_complete(LoadingOutcome::Connected);
                    }
                    Ok(LoadingOutcome::NoDevice) => {
                        on_complete(LoadingOutcome::NoDevice);
                    }
                    Err(e) => {
                        info_label.set_visible(false);
                        error_label.set_text(&format!("Error: {}", e));
                        error_label.set_visible(true);
                        retry_button.set_visible(true);
                        setup_button.set_visible(true);

                        let on_complete_retry = on_complete.clone();
                        retry_button.connect_clicked(move |_| {
                            on_complete_retry(LoadingOutcome::Retry);
                        });
                        let on_complete_setup = on_complete.clone();
                        setup_button.connect_clicked(move |_| {
                            on_complete_setup(LoadingOutcome::NoDevice);
                        });
                    }
                    Ok(LoadingOutcome::Retry) => {
                        on_complete(LoadingOutcome::Retry);
                    }
                }
            }
        ));

        toolbar_view
    }

    async fn run_checks(info_label: &Label) -> Result<LoadingOutcome, String> {
        info_label.set_text("Connecting to SpacePods service…");

        let client = match SpacePodsClient::connect(None).await {
            Ok(c) => c,
            Err(e) => return Err(format!("Cannot connect to service: {}", e)),
        };
        let mut client = client;

        info_label.set_text("Checking service health…");
        if !client.ping().await.map_err(|e| format!("Ping failed: {}", e))? {
            return Err("Service did not respond".to_string());
        }

        info_label.set_text("Looking for previously connected device…");
        let saved = get_last_connected_device();

        if saved.is_none() {
            info_label.set_text("No saved device found. Proceeding to setup…");
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            return Ok(LoadingOutcome::NoDevice);
        }

        let device = saved.unwrap();
        info_label.set_text(&format!("Trying to connect to {}", device.name));

        match client.connect_device(device.address.clone()).await {
            Ok(_) => {
                tokio::time::sleep(tokio::time::Duration::from_millis(800)).await;
                let status = client
                    .get_status()
                    .await
                    .map_err(|e| format!("Status check failed: {}", e))?;
                if status.connected || status.address.as_deref() == Some(&device.address){
                    add_known_device(device.name.clone(), device.address.clone());

                    let settings = load_settings();
                    info_label.set_text("Restoring settings…");
                    if let Err(e) = client.set_anc_mode(match settings.last_anc_mode {
                        0 => "off",
                        1 => "anc",
                        _ => "transparency",
                    }).await {
                        eprintln!("Failed to restore ANC mode: {}", e);
                    }
                    if let Err(e) = client.set_level(settings.last_anc_level).await {
                        eprintln!("Failed to restore ANC level: {}", e);
                    }
                    if let Err(e) = client.set_eq_preset(settings.last_eq_preset).await {
                        eprintln!("Failed to restore EQ preset: {}", e);
                    }
                    if let Err(e) = client.set_adaptive_anc(settings.adaptive_anc_enabled).await {
                        eprintln!("Failed to restore adaptive ANC: {}", e);
                    }
                    if let Err(e) = client.set_dual_device(settings.dual_device_enabled).await {
                        eprintln!("Failed to restore dual device: {}", e);
                    }
                    info_label.set_text("Connected successfully!");
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    Ok(LoadingOutcome::Connected)
                } else {
                    Err("Connection established but device not reported as connected".to_string())
                }
            }
            Err(e) => Err(format!("Failed to connect to {}: {}", device.name, e)),
        }
    }
}