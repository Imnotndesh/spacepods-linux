use gtk4::prelude::*;
use gtk4::{Box, Label, Orientation, Spinner, Button, Align};
use libadwaita::{StatusPage, ToolbarView, HeaderBar, WindowTitle, Clamp};
use glib::clone;
use libspacepods::client::SpacePodsClient;
use crate::storage::{get_last_connected_device, add_known_device};
use crate::log::Log;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

pub struct LoadingPage;

#[derive(Clone)]
pub enum LoadingOutcome {
    Connected { client: Arc<Mutex<SpacePodsClient>>, product_id: Option<u16> },
    NoDevice,
    Retry,
}

impl LoadingPage {
    pub fn new(on_complete: Rc<dyn Fn(LoadingOutcome)>) -> gtk4::Widget {
        let status_page = StatusPage::new();
        status_page.set_icon_name(Some("audio-headset-symbolic"));
        status_page.set_title("Starting SpacePods");
        status_page.set_description(Some("Checking service and connecting to your earbuds…"));
        status_page.set_vexpand(true);

        let spinner = Spinner::new();
        spinner.set_size_request(48, 48);
        spinner.set_halign(gtk4::Align::Center);
        spinner.start();

        let info_label = Label::new(Some("Connecting to service…"));
        info_label.add_css_class("dim-label");
        info_label.add_css_class("title-4");
        info_label.set_halign(gtk4::Align::Center);

        let error_label = Label::new(None);
        error_label.add_css_class("error");
        error_label.set_halign(gtk4::Align::Center);
        error_label.set_wrap(true);
        error_label.set_max_width_chars(50);
        error_label.set_visible(false);

        let buttons_box = Box::new(Orientation::Horizontal, 8);
        buttons_box.set_halign(gtk4::Align::Center);
        buttons_box.set_visible(false);

        let retry_button = Button::with_label("Retry");
        retry_button.add_css_class("suggested-action");
        retry_button.add_css_class("pill");

        let setup_button = Button::with_label("Go to Setup");
        setup_button.add_css_class("pill");

        buttons_box.append(&retry_button);
        buttons_box.append(&setup_button);

        let content = Box::new(Orientation::Vertical, 16);
        content.set_valign(Align::Center);
        content.set_halign(Align::Center);
        content.set_vexpand(true);
        content.set_hexpand(true);
        content.set_margin_top(48);
        content.set_margin_bottom(48);
        content.set_margin_start(32);
        content.set_margin_end(32);
        content.append(&spinner);
        content.append(&info_label);
        content.append(&error_label);
        content.append(&buttons_box);

        let clamp = Clamp::new();
        clamp.set_maximum_size(500);
        clamp.set_child(Some(&content));

        glib::spawn_future_local(clone!(
            #[strong] on_complete,
            #[strong] info_label,
            #[strong] error_label,
            #[strong] retry_button,
            #[strong] setup_button,
            #[strong] buttons_box,
            #[strong] spinner,
            async move {
                let outcome = Self::run_checks(&info_label).await;
                spinner.stop();
                spinner.set_visible(false);
                match outcome {
                    Ok(outcome @ LoadingOutcome::Connected { .. }) => {
                        info_label.set_text("Connected!");
                        on_complete(outcome);
                    }
                    Ok(LoadingOutcome::NoDevice) => {
                        on_complete(LoadingOutcome::NoDevice);
                    }
                    Err(e) => {
                        info_label.set_visible(false);
                        error_label.set_text(&e);
                        error_label.set_visible(true);
                        buttons_box.set_visible(true);

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

        clamp.upcast()
    }

    async fn run_checks(info_label: &Label) -> Result<LoadingOutcome, String> {
        info_label.set_text("Connecting to SpacePods service…");

        let client = match SpacePodsClient::connect(None).await {
            Ok(c) => {
                Log::info("LOADING", "Connected to daemon IPC");
                c
            }
            Err(e) => {
                Log::info("LOADING", &format!("IPC connect failed: {}", e));
                return Err(format!(
                    "Cannot reach the SpacePods daemon.\n\nMake sure 'libspacepods service' is running.\n\nDetails: {}",
                    e
                ));
            }
        };
        let mut client = client;

        info_label.set_text("Checking service health…");
        if !client.ping().await.map_err(|e| format!("Service health check failed: {}", e))? {
            return Err("Service is running but did not respond properly.\nTry restarting: libspacepods service".to_string());
        }

        info_label.set_text("Looking for saved device…");
        let saved = get_last_connected_device();

        if saved.is_none() {
            Log::info("LOADING", "No saved device — routing to setup");
            info_label.set_text("No saved device found.");
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            return Ok(LoadingOutcome::NoDevice);
        }

        let device = saved.unwrap();
        Log::info("LOADING", &format!("Trying saved device: {} ({})", device.name, device.address));
        info_label.set_text(&format!("Looking for {}…", device.name));

        // The daemon auto-connects on startup via its status updater loop.
        // Just poll get_status() — no explicit Connect needed.
        let start = tokio::time::Instant::now();
        let timeout = tokio::time::Duration::from_secs(10);
        loop {
            match client.get_status().await {
                Ok(status) => {
                    if status.connection.connected {
                        add_known_device(device.name.clone(), device.address.clone());
                        let product_id = status.product_id;
                        Log::info("LOADING", &format!("Connected! product_id={:?}", product_id));
                        info_label.set_text("Connected successfully!");
                        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                        return Ok(LoadingOutcome::Connected {
                            client: Arc::new(Mutex::new(client)),
                            product_id,
                        });
                    }
                }
                Err(e) => Log::warn("LOADING", &format!("Status poll: {}", e)),
            }
            if start.elapsed() > timeout {
                break;
            }
            info_label.set_text(&format!("Waiting for {}… ({:.0}s)", device.name, (timeout - start.elapsed()).as_secs_f64()));
            tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;
        }

        // Timeout — try explicit connect as fallback
        Log::info("LOADING", "Daemon didn't auto-connect, trying explicit");
        info_label.set_text(&format!("Trying explicit connect to {}… (15s)", device.name));

        let connect_fut = client.connect_device(device.address.clone());
        let explicit_timeout = tokio::time::Duration::from_secs(15);
        match tokio::time::timeout(explicit_timeout, connect_fut).await {
            Ok(Ok(_)) => {
                tokio::time::sleep(Duration::from_millis(800)).await;
                let status = client.get_status().await.map_err(|e| format!("Status: {}", e))?;
                add_known_device(device.name.clone(), device.address.clone());
                let product_id = status.product_id;
                Log::info("LOADING", &format!("Explicit connect OK, product_id={:?}", product_id));
                info_label.set_text("Connected!");
                tokio::time::sleep(Duration::from_millis(500)).await;
                Ok(LoadingOutcome::Connected { client: Arc::new(Mutex::new(client)), product_id })
            }
            Ok(Err(e)) => Err(format!("Connection failed: {}", e)),
            Err(_) => Err(format!("Connection to {} timed out after 15s", device.name)),
        }
    }
}
