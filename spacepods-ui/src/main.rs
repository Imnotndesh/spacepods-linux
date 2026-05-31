use gtk4::prelude::*;
use libadwaita::{Application, ApplicationWindow};
use std::sync::Arc;
use libadwaita::prelude::AdwApplicationWindowExt;
use tokio::sync::{mpsc, Mutex};
mod pages;
mod home;
mod tray;
mod storage;

use home::HomeView;
use pages::loading_page::{LoadingPage, LoadingOutcome};
use pages::setup_page::SetupPage;
use storage::load_settings;

#[derive(Debug, Clone)]
pub enum ClientCommand {
    SetAncMode(String),
    SetAncLevel(u8),
    SetAdaptiveAnc(bool),
    SetEqPreset(u8),
    SetCustomEq([i8; 7]),
    FindDevice(bool),
    FactoryReset,
    ReconnectDevice,
    RefreshBattery,
    ConnectDevice(String),
}

fn main() -> glib::ExitCode {
    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    let _guard = rt.enter();

    let settings = load_settings();
    write_autostart_entry(settings.autostart);
    ensure_daemon_running();

    let (tx, mut rx) = mpsc::channel::<ClientCommand>(64);

    let shared_client: Arc<Mutex<Option<Arc<Mutex<libspacepods::client::SpacePodsClient>>>>> =
        Arc::new(Mutex::new(None));



    let worker_shared_client = shared_client.clone();
    tokio::spawn(async move {
        loop {

            let client_opt = {
                let guard = worker_shared_client.lock().await;
                guard.clone()
            };

            if let Some(client) = client_opt {
                while let Some(cmd) = rx.recv().await {
                    let mut c = client.lock().await;
                    match cmd {
                        ClientCommand::SetAncMode(mode_str) => {
                            if let Err(e) = c.set_anc_mode(&mode_str).await {
                                canal_log_error("set_anc_mode", e);
                            }
                        }
                        ClientCommand::SetEqPreset(preset) => {
                            if let Err(e) = c.set_eq_preset(preset).await {
                                canal_log_error("set_eq_preset", e);
                            }
                        }
                        ClientCommand::FindDevice(state) => {
                            if let Err(e) = c.find_device(state).await {
                                canal_log_error("find_device", e);
                            }
                        }
                        ClientCommand::FactoryReset => {
                            if let Err(e) = c.factory_reset().await {
                                canal_log_error("factory_reset", e);
                            }
                        }
                        ClientCommand::ConnectDevice(address) => {
                            if let Err(e) = c.connect_device(address.clone()).await {
                                canal_log_error("connect_device", e);
                            }
                        }
                        ClientCommand::SetAncLevel(level) => {
                            if let Err(e) = c.set_level(level).await {
                                canal_log_error("set_level", e);
                            }
                        }
                        ClientCommand::SetAdaptiveAnc(enabled) => {
                            if let Err(e) = c.set_adaptive_anc(enabled).await {
                                canal_log_error("set_adaptive_anc", e);
                            }
                        }
                        ClientCommand::SetCustomEq(bands) => {
                            if let Err(e) = c.set_custom_eq(bands).await {
                                canal_log_error("set_custom_eq", e);
                            }
                        }
                        ClientCommand::RefreshBattery => {

                            if let Err(e) = c.get_battery().await {
                                canal_log_error("refresh_battery", e);
                            }
                        }
                        ClientCommand::ReconnectDevice => {
                            if let Ok(status) = c.get_status().await {
                                if let Some(addr) = status.address {
                                    if let Err(e) = c.connect_device(addr).await {
                                        canal_log_error("reconnect_device", e);
                                    }
                                }
                            }
                        }


                        _ => {

                        }
                    }
                }
                break;
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        }
    });

    let app = Application::new(Some("com.spacepods.ui"), Default::default());

    let tx_app = tx.clone();
    app.connect_activate(move |app| {
        let window = ApplicationWindow::new(app);
        window.set_title(Some("SpacePods"));
        window.set_default_size(600, 500);

        let (tray_handle, tray_rx) = {
            let settings = load_settings();
            let handle_rx = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(tray::spawn_tray())
            });
            if !settings.tray_enabled {
                handle_rx.0.send(tray::TrayCommand::Hide);
            }
            handle_rx
        };
        let tray_handle = std::rc::Rc::new(Some(tray_handle));


        {
            let window_ref = window.clone();
            let tx_tray = tx_app.clone();
            glib::idle_add_local(move || {
                while let Ok(cmd) = tray_rx.try_recv() {
                    match cmd {
                        tray::TrayCommand::ShowWindow => window_ref.present(),
                        tray::TrayCommand::HideWindow => window_ref.set_visible(false),
                        tray::TrayCommand::Quit => window_ref.close(),
                        tray::TrayCommand::SetAncMode(mode) => {
                            let mode_str = match mode {
                                0 => "off",
                                1 => "on",
                                2 => "transparency",
                                _ => "off",
                            };
                            let tx = tx_tray.clone();
                            let mode_string = mode_str.to_string();
                            glib::spawn_future_local(async move {
                                let _ = tx.send(ClientCommand::SetAncMode(mode_string)).await;
                            });
                        }
                        tray::TrayCommand::SetEqPreset(preset) => {
                            let tx = tx_tray.clone();
                            glib::spawn_future_local(async move {
                                let _ = tx.send(ClientCommand::SetEqPreset(preset)).await;
                            });
                        }
                        _ => {}
                    }
                }
                glib::ControlFlow::Continue
            });
        }

        {
            let window_ref = window.clone();
            let tray_handle_ref = tray_handle.clone();
            window.connect_close_request(move |_| {
                let settings = load_settings();
                if settings.close_to_tray && settings.tray_enabled {
                    window_ref.set_visible(false);
                    if let Some(ref h) = *tray_handle_ref {
                        h.send(tray::TrayCommand::ShowWindow);
                    }
                    return glib::Propagation::Stop;
                }
                glib::Propagation::Proceed
            });
        }

        let window_weak = window.downgrade();
        let callback_holder: std::rc::Rc<std::cell::RefCell<Option<std::rc::Rc<dyn Fn(LoadingOutcome)>>>> =
            std::rc::Rc::new(std::cell::RefCell::new(None));

        let callback: std::rc::Rc<dyn Fn(LoadingOutcome)> = {
            let window_weak = window_weak.clone();
            let callback_holder = callback_holder.clone();
            let tray_handle = tray_handle.clone();
            let shared_client = shared_client.clone();
            let tx_callback = tx_app.clone();
            std::rc::Rc::new(move |outcome| {
                if let Some(window) = window_weak.upgrade() {
                    match outcome {
                        LoadingOutcome::Connected(client) => {

                            {
                                let mut lock = tokio::task::block_in_place(|| {
                                    tokio::runtime::Handle::current().block_on(shared_client.lock())
                                });
                                *lock = Some(Arc::clone(&client));
                            }

                            {
                                let tray_ref = tray_handle.clone();
                                glib::spawn_future_local(async move {
                                    if let Ok(mut sub_client) =
                                        libspacepods::client::SpacePodsClient::connect(None).await
                                    {
                                        if let Ok(mut rx) = sub_client.subscribe().await {
                                            while let Ok(status) = rx.recv().await {
                                                if let Some(ref h) = *tray_ref {
                                                    h.set_anc_mode(status.anc_mode.unwrap_or(0));
                                                    h.set_eq_preset(status.eq_mode.unwrap_or(0));
                                                    let connected = status.connected;
                                                    let name = status.address.clone().unwrap_or_default();
                                                    glib::spawn_future_local({
                                                        let h2 = h.clone();
                                                        async move {
                                                            h2.set_status(
                                                                name,
                                                                status.battery_left,
                                                                status.battery_right,
                                                                status.battery_case,
                                                                connected,
                                                            );
                                                        }
                                                    });
                                                }
                                            }
                                        }
                                    }
                                });
                            }

                            let home_view = HomeView::new(tx_callback.clone(), || {});
                            window.set_content(Some(&home_view));
                        }
                        LoadingOutcome::NoDevice => {
                            let go_to_home = {
                                let window = window.clone();
                                let tx_nodetect = tx_callback.clone();
                                let inner_shared_client = shared_client.clone();
                                move || {
                                    let window = window.clone();
                                    let tx_inner = tx_nodetect.clone();
                                    let client_store = inner_shared_client.clone();
                                    glib::spawn_future_local(async move {
                                        if let Ok(client) =
                                            libspacepods::client::SpacePodsClient::connect(None).await
                                        {
                                            let arc_client = Arc::new(Mutex::new(client));
                                            {
                                                let mut lock = client_store.lock().await;
                                                *lock = Some(arc_client.clone());
                                            }
                                            let home_view = HomeView::new(tx_inner, || {});
                                            window.set_content(Some(&home_view));
                                        }
                                    });
                                }
                            };
                            let setup_page = SetupPage::new(go_to_home);
                            window.set_content(Some(&setup_page));
                        }
                        LoadingOutcome::Retry => {
                            if let Some(cb) = callback_holder.borrow().as_ref() {
                                let new_loading = LoadingPage::new(cb.clone());
                                window.set_content(Some(&new_loading));
                            }
                        }
                    }
                }
            })
        };

        *callback_holder.borrow_mut() = Some(callback.clone());
        let loading_page = LoadingPage::new(callback);
        window.set_content(Some(&loading_page));
        window.present();
    });

    app.run()
}

fn canal_log_error(action: &str, err: impl std::fmt::Display) {
    eprintln!("[Worker Loop Error] Action '{}' failed: {}", action, err);
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

fn ensure_daemon_running() {
    let socket = std::path::Path::new("/tmp/spacepods.sock");
    if !socket.exists() {
        let exe = std::env::current_exe().unwrap_or_else(|_| "spacepods".into());
        let _ = std::process::Command::new(exe)
            .arg("service")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
        std::thread::sleep(std::time::Duration::from_millis(800));
    }
}