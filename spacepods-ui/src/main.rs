use gtk4::prelude::*;
use libadwaita::{Application, ApplicationWindow};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use libadwaita::prelude::AdwApplicationWindowExt;

mod pages;
mod home;
mod tray;
mod storage;

use home::HomeView;
use pages::loading_page::{LoadingPage, LoadingOutcome};
use pages::setup_page::SetupPage;

fn main() -> glib::ExitCode {
    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    let _guard = rt.enter();

    let app = Application::new(Some("com.spacepods.ui"), Default::default());

    app.connect_activate(|app| {
        let window = ApplicationWindow::new(app);
        window.set_title(Some("SpacePods"));
        window.set_default_size(600, 500);

        let window_weak = window.downgrade();

        let callback_holder: Rc<RefCell<Option<Rc<dyn Fn(LoadingOutcome)>>>> =
            Rc::new(RefCell::new(None));

        let callback: Rc<dyn Fn(LoadingOutcome)> = {
            let window_weak = window_weak.clone();
            let callback_holder = callback_holder.clone();
            Rc::new(move |outcome| {
                if let Some(window) = window_weak.upgrade() {
                    match outcome {
                        LoadingOutcome::Connected(client) => {
                            let home_view = HomeView::new(client, || {});
                            window.set_content(Some(&home_view));
                        }
                        LoadingOutcome::NoDevice => {
                            let go_to_home = {
                                let window = window.clone();
                                move || {
                                    let window = window.clone();
                                    glib::spawn_future_local(async move {
                                        if let Ok(client) = libspacepods::client::SpacePodsClient::connect(None).await {
                                            let client = Arc::new(tokio::sync::Mutex::new(client));
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