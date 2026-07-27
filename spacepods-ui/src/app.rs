use gtk4::prelude::*;
use libadwaita::{Application, ApplicationWindow};
use libadwaita::prelude::AdwApplicationWindowExt;
use std::rc::Rc;

use crate::home::HomeView;
use crate::pages::loading_page::{LoadingPage, LoadingOutcome};
use crate::pages::setup_page::SetupPage;
use crate::storage::load_settings;
use crate::service::write_autostart_entry;

pub fn run_app() -> glib::ExitCode {
    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    let _guard = rt.enter();

    let settings = load_settings();
    write_autostart_entry(settings.autostart);

    let app = Application::new(Some("com.spacepods.ui"), Default::default());

    app.connect_activate(move |app| {
        let window = ApplicationWindow::new(app);
        window.set_title(Some("SpacePods"));
        window.set_default_size(850, 600);
        window.set_width_request(360);
        window.set_height_request(480);

        // ── Navigation callbacks ──
        let window_weak = window.downgrade();
        let callback_holder: Rc<std::cell::RefCell<Option<Rc<dyn Fn(LoadingOutcome)>>>> =
            Rc::new(std::cell::RefCell::new(None));

        let ch_closure = callback_holder.clone();
        let ch_store = callback_holder.clone();

        {
            let window_weak = window_weak.clone();

            let callback: Rc<dyn Fn(LoadingOutcome)> = Rc::new(move |outcome| {
                let window = match window_weak.upgrade() {
                    Some(w) => w,
                    None => return,
                };

                match outcome {
                    LoadingOutcome::Connected(_client) => {
                        let home_view = HomeView::new(&window);
                        window.set_content(Some(&home_view));
                    }
                    LoadingOutcome::NoDevice => {
                        let window_clone = window.clone();
                        let go_to_home = move || {
                            let win = window_clone.clone();
                            glib::spawn_future_local(async move {
                                let home_view = HomeView::new(&win);
                                win.set_content(Some(&home_view));
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
