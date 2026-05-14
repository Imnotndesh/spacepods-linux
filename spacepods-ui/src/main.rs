use gtk4::prelude::*;
use libadwaita::prelude::*;
use libadwaita::{Application, ApplicationWindow};
use std::cell::RefCell;
use std::rc::Rc;
use tokio::runtime::Runtime;

mod pages;
mod home;
mod tray;

use home::HomeView;
use pages::setup_page::SetupPage;

fn main() -> glib::ExitCode {
    // Create a Tokio runtime and enter its context
    let rt = Runtime::new().expect("Failed to create tokio runtime");
    let _guard = rt.enter();

    let app = Application::new(Some("com.spacepods.ui"), Default::default());

    app.connect_activate(|app| {
        let window = ApplicationWindow::new(app);
        window.set_title(Some("SpacePods"));
        window.set_default_size(600, 500);
        let window_weak = window.downgrade();
        let on_add_device: Rc<RefCell<Option<Rc<dyn Fn()>>>> = Rc::new(RefCell::new(None));
        let go_to_home = {
            let window_weak = window_weak.clone();
            let on_add_device = on_add_device.clone();
            move || {
                if let Some(window) = window_weak.upgrade() {
                    if let Some(on_add) = on_add_device.borrow().as_ref().cloned() {
                        let home_view = HomeView::new(move || {
                            on_add();
                        });
                        window.set_content(Some(&home_view));
                    }
                }
            }
        };

        let add_device_impl = {
            let window_weak = window_weak.clone();
            let go_to_home = go_to_home.clone();
            move || {
                if let Some(window) = window_weak.upgrade() {
                    let setup_page = SetupPage::new(go_to_home.clone());
                    window.set_content(Some(&setup_page));
                }
            }
        };
        *on_add_device.borrow_mut() = Some(Rc::new(add_device_impl));
        let setup_page = SetupPage::new(go_to_home);
        window.set_content(Some(&setup_page));
        window.present();
    });

    app.run()
}