use gtk4::prelude::*;
use libadwaita::{Application, ApplicationWindow};
use libadwaita::prelude::AdwApplicationWindowExt;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::home::HomeView;
use crate::pages::loading_page::{LoadingPage, LoadingOutcome};
use crate::pages::setup_page::SetupPage;
use crate::storage::load_settings;
use crate::tray;
use crate::service::{ensure_daemon_running, write_autostart_entry};

pub fn run_app() -> glib::ExitCode {
    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    let _guard = rt.enter();

    let settings = load_settings();
    write_autostart_entry(settings.autostart);
    ensure_daemon_running();

    let app = Application::new(Some("com.spacepods.ui"), Default::default());

    app.connect_activate(move |app| {
        let window = ApplicationWindow::new(app);
        window.set_title(Some("SpacePods"));
        window.set_default_size(850, 600);
        // Sensible floor so the sidebar+content pane never gets crushed
        // below something usable before the breakpoint kicks in.
        window.set_width_request(360);
        window.set_height_request(480);

        // ── Spawn tray ──
        let (tray_handle, tray_rx) = {
            let settings = load_settings();
            let (handle, rx) = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(tray::spawn_tray())
            });
            if !settings.tray_enabled {
                handle.send(tray::TrayCommand::Hide);
            }
            (Rc::new(Some(handle)), rx)
        };

        // ── Poll tray commands ──
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

        // ── Close-to-tray ──
        {
            let window_ref = window.clone();
            let tray_handle = tray_handle.clone();
            window.connect_close_request(move |_| {
                let settings = load_settings();
                if settings.close_to_tray && settings.tray_enabled {
                    window_ref.set_visible(false);
                    if let Some(ref h) = *tray_handle {
                        h.send(tray::TrayCommand::ShowWindow);
                    }
                    return glib::Propagation::Stop;
                }
                glib::Propagation::Proceed
            });
        }

        // ── Navigation callbacks ──
        let window_weak = window.downgrade();
        let callback_holder: Rc<RefCell<Option<Rc<dyn Fn(LoadingOutcome)>>>> =
            Rc::new(RefCell::new(None));

        let ch_closure = callback_holder.clone();
        let ch_store = callback_holder.clone();

        {
            let window_weak = window_weak.clone();
            let tray_handle = tray_handle.clone();

            let callback: Rc<dyn Fn(LoadingOutcome)> = Rc::new(move |outcome| {
                let window = match window_weak.upgrade() {
                    Some(w) => w,
                    None => return,
                };

                match outcome {
                    LoadingOutcome::Connected(client) => {
                        let home_view = HomeView::new(&window, tray_handle.clone(), client);
                        window.set_content(Some(&home_view));
                    }
                    LoadingOutcome::NoDevice => {
                        let window_clone = window.clone();
                        let tray_handle = tray_handle.clone();
                        let go_to_home = move || {
                            let win = window_clone.clone();
                            let th = tray_handle.clone();
                            glib::spawn_future_local(async move {
                                if let Ok(client) =
                                    libspacepods::client::SpacePodsClient::connect(None).await
                                {
                                    let client = Arc::new(Mutex::new(client));
                                    let home_view = HomeView::new(&win, th, client);
                                    win.set_content(Some(&home_view));
                                }
                            });
                        };
                        let setup_page = SetupPage::new(go_to_home);
                        window.set_content(Some(&setup_page));
                    }
                    LoadingOutcome::Retry => {
                        if let Some(cb) = ch_closure.borrow().as_ref() {
                            let new_loading = LoadingPage::new(cb.clone());
                            window.set_content(Some(&new_loading));
                        }
                    }
                }
            });

            let cb_for_loading = callback.clone();
            *ch_store.borrow_mut() = Some(callback);
            let loading_page = LoadingPage::new(cb_for_loading);
            window.set_content(Some(&loading_page));
        }

        window.present();
    });

    app.run()
}