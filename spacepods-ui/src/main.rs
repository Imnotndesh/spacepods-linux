use gtk4::prelude::*;
use libadwaita::{Application, ApplicationWindow};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use libadwaita::prelude::AdwApplicationWindowExt;
use tokio::sync::Mutex;

mod pages;
mod home;
mod tray;
mod storage;

use home::HomeView;
use pages::loading_page::{LoadingPage, LoadingOutcome};
use pages::setup_page::SetupPage;
use storage::load_settings;

fn main() -> glib::ExitCode {
    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    let _guard = rt.enter();
    let settings = load_settings();
    write_autostart_entry(settings.autostart);
    ensure_daemon_running();

    let app = Application::new(Some("com.spacepods.ui"), Default::default());

    app.connect_activate(|app| {
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
        let tray_handle = Rc::new(Some(tray_handle));

        {
            let window_ref = window.clone();
            glib::idle_add_local(move || {
                while let Ok(cmd) = tray_rx.try_recv() {
                    match cmd {
                        tray::TrayCommand::ShowWindow => window_ref.present(),
                        tray::TrayCommand::HideWindow => window_ref.set_visible(false),
                        tray::TrayCommand::Quit => window_ref.close(),
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
        let callback_holder: Rc<RefCell<Option<Rc<dyn Fn(LoadingOutcome)>>>> =
            Rc::new(RefCell::new(None));

        let callback: Rc<dyn Fn(LoadingOutcome)> = {
            let window_weak = window_weak.clone();
            let callback_holder = callback_holder.clone();
            let tray_handle = tray_handle.clone();
            Rc::new(move |outcome| {
                if let Some(window) = window_weak.upgrade() {
                    match outcome {
                        LoadingOutcome::Connected(client) => {
                            let tray_handle_inner = tray_handle.clone();
                            let home_view = HomeView::new(client, move || {
                                // on_add_device — could push setup page; left as stub
                            });
                            window.set_content(Some(&home_view));
                        }
                        LoadingOutcome::NoDevice => {
                            let go_to_home = {
                                let window = window.clone();
                                move || {
                                    let window = window.clone();
                                    glib::spawn_future_local(async move {
                                        if let Ok(client) =
                                            libspacepods::client::SpacePodsClient::connect(None).await
                                        {
                                            let client = Arc::new(Mutex::new(client));
                                            let home_view = HomeView::new(client, || {});
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