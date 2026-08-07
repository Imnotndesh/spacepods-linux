use gtk4::prelude::*;
use libadwaita::{Application, ApplicationWindow, ToastOverlay};
use libadwaita::prelude::AdwApplicationWindowExt;

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

        // Set app icon from resources
        gtk4::Window::set_default_icon_name("com.spacepods.ui");

        // Register custom icons once a display is available
        if let Some(display) = gtk4::gdk::Display::default() {
            let icon_theme = gtk4::IconTheme::for_display(&display);
            icon_theme.add_resource_path("/com/spacepods/ui/icons");
        }

        window.set_title(Some("SpacePods"));
        window.set_default_size(850, 600);
        window.set_width_request(360);
        window.set_height_request(480);

        // Wrap everything in a ToastOverlay so toasts work everywhere
        let toast_overlay = ToastOverlay::new();

        let window_weak = window.downgrade();
        let setup_page = SetupPage::new(move |product_id| {
            if let Some(win) = window_weak.upgrade() {
                let home_view = crate::home::HomeView::new(&win, product_id);
                win.set_content(Some(&home_view));
            }
        });
        toast_overlay.set_child(Some(&setup_page));
        window.set_content(Some(&toast_overlay));
        window.present();
    });

    app.run()
}
