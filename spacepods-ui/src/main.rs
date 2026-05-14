use gtk4::prelude::*;
use libadwaita::prelude::*;
use libadwaita::{Application, ApplicationWindow};

mod pages;
mod home;
mod tray;

use home::HomeView;
use pages::setup_page::SetupPage;

fn main() -> glib::ExitCode {
    let app = Application::new(Some("com.spacepods.ui"), Default::default());

    app.connect_activate(|app| {
        let window = ApplicationWindow::new(app);
        window.set_title(Some("SpacePods"));
        window.set_default_size(600, 500);

        let window_weak = window.downgrade();

        let setup_page = SetupPage::new(move || {
            if let Some(window) = window_weak.upgrade() {
                let home_view = HomeView::new();
                window.set_content(Some(&home_view));
            }
        });
        window.set_content(Some(&setup_page));
        window.present();
    });

    app.run()
}