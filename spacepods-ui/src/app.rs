use gtk4::prelude::*;
use libadwaita::{Application, ApplicationWindow, ToastOverlay};
use libadwaita::prelude::AdwApplicationWindowExt;

use std::rc::Rc;
use std::cell::RefCell;
use std::rc::Weak;

use crate::context::WindowController;
use crate::pages::setup_page::SetupPage;
use crate::storage::load_settings;
use crate::service::write_autostart_entry;
use crate::tray::{self, TrayCommand};

/// Poll the tray command channel on the GTK main loop. `try_recv` is
/// non-blocking so this keeps everything on one thread (no `Send` issues with
/// `Rc<WindowController>`). Window operations must happen on the main loop.
fn drain_tray_commands(
    rx: std::sync::mpsc::Receiver<TrayCommand>,
    window: Rc<WindowController>,
    ctx_slot: Rc<RefCell<Option<Weak<crate::context::AppContext>>>>,
) {
    glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
        while let Ok(cmd) = rx.try_recv() {
            match cmd {
                TrayCommand::ShowWindow => window.present(),
                TrayCommand::HideWindow => window.hide(),
                TrayCommand::ShowPopup { x, y } => {
                    if let Some(ctx) = ctx_slot.borrow().as_ref().and_then(|w| w.upgrade()) {
                        crate::quick_settings::show_popup(&ctx, x, y);
                    }
                }
                TrayCommand::Quit => {
                    window.force_quit();
                }
                _ => {}
            }
        }
        glib::ControlFlow::Continue
    });
}

pub fn run_app() -> glib::ExitCode {
    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    let _guard = rt.enter();

    let settings = load_settings();
    write_autostart_entry(settings.autostart);

    let app = Application::new(Some("com.spacepods.ui"), Default::default());
    let window_controller = WindowController::new();

    // Start the tray icon once. It runs on its own background thread (via the
    // blocking API); the returned receiver gives us the menu commands.
    let (tray_handle, tray_rx) = tray::spawn_tray();

    // Shared slot for the AppContext — populated by HomeView::new, read by
    // the tray command loop for ShowPopup (which needs ctx to query the daemon).
    let ctx_slot: Rc<RefCell<Option<std::rc::Weak<crate::context::AppContext>>>> =
        Rc::new(RefCell::new(None));
    drain_tray_commands(tray_rx, window_controller.clone(), ctx_slot.clone());

    let tray_handle = Rc::new(tray_handle);
    let ctx_slot2 = ctx_slot.clone();

    app.connect_activate(move |app| {
        let window = ApplicationWindow::new(app);
        window_controller.set_window(&window);

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

        // Close-to-background: when the setting is enabled, closing the window
        // hides it (the tray keeps the app alive) instead of quitting. A tray
        // "Quit" bypasses this via WindowController::force_quit().
        let close_wc = window_controller.clone();
        window.connect_close_request(move |win| {
            // Real quit requested (tray Quit / etc.).
            if close_wc.force_close_requested() {
                close_wc.clear_force_close();
                return glib::Propagation::Proceed;
            }
            if crate::storage::load_settings().close_to_background {
                win.set_visible(false);
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });

        // Wrap everything in a ToastOverlay so toasts work everywhere
        let toast_overlay = ToastOverlay::new();

        let window_weak = window.downgrade();
        let tray = (*tray_handle).clone();
        let wc = window_controller.clone();
        let slot = ctx_slot2.clone();
        let setup_page = SetupPage::new(move |product_id| {
            if let Some(win) = window_weak.upgrade() {
                let ctx = crate::context::AppContext::new(
                    ToastOverlay::new(),
                    Some(tray.clone()),
                    wc.clone(),
                );
                ctx.product_id.set(product_id);

                // Expose the context so the tray command loop can open the popup.
                *slot.borrow_mut() = Some(std::rc::Rc::downgrade(&ctx));

                let home_view = crate::home::HomeView::from_ctx(&win, ctx);
                win.set_content(Some(&home_view));
            }
        });
        toast_overlay.set_child(Some(&setup_page));
        window.set_content(Some(&toast_overlay));
        window.present();
    });

    app.run()
}
