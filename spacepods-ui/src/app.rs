use gtk4::prelude::*;
use gtk4::gio;
use libadwaita::{Application, ApplicationWindow, ToastOverlay};
use libadwaita::prelude::AdwApplicationWindowExt;
use libadwaita::prelude::AdwDialogExt;

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
                TrayCommand::SetAncMode(mode) => {
                    tray::ANC_MODE_ATOMIC.store(mode, std::sync::atomic::Ordering::Relaxed);
                    if let Some(ctx) = ctx_slot.borrow().as_ref().and_then(|w| w.upgrade()) {
                        ctx.anc_mode.set(mode);
                    }
                    let label = match mode { 0 => "off", 1 => "anc", 2 => "transparency", _ => "off" };
                    glib::spawn_future_local(async move {
                        use libspacepods::client::SpacePodsClient;
                        if let Ok(mut c) = SpacePodsClient::connect(None).await {
                            let _ = c.set_anc_mode(label).await;
                        }
                    });
                }
                TrayCommand::Quit => {
                    // A real quit that bypasses close-to-background.
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
    install_app_actions(&app);
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

                // Sync the tray's ANC radio-group to the shared cell.
                tray::ANC_MODE_ATOMIC.store(ctx.anc_mode.get(), std::sync::atomic::Ordering::Relaxed);

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

/// A `GtkMenuButton` with the standard primary (hamburger) menu.
///
/// Shared by the sidebar and content header bars so both expose the same
/// app-wide actions. See the GNOME HIG "primary menu" pattern.
pub fn primary_menu_button() -> gtk4::MenuButton {
    let menu = gio::Menu::new();
    let app_section = gio::Menu::new();
    app_section.append(Some("Keyboard _Shortcuts"), Some("app.shortcuts"));
    app_section.append(Some("A_bout SpacePods"), Some("app.about"));
    app_section.append(Some("_Quit"), Some("app.quit"));
    menu.append_section(None, &app_section);

    let button = gtk4::MenuButton::new();
    button.set_icon_name("open-menu-symbolic");
    button.set_menu_model(Some(&menu));
    button.set_primary(true);
    button.set_tooltip_text(Some("Main Menu"));
    button.update_property(&[gtk4::accessible::Property::Label("Main Menu")]);
    button
}

/// Register the app-wide `Gio.Action`s and their standard accelerators.
fn install_app_actions(app: &Application) {
    let quit = gio::SimpleAction::new("quit", None);
    {
        let app = app.clone();
        quit.connect_activate(move |_, _| app.quit());
    }
    app.add_action(&quit);
    app.set_accels_for_action("app.quit", &["<primary>q"]);

    let close = gio::SimpleAction::new("close", None);
    {
        let app = app.clone();
        close.connect_activate(move |_, _| {
            if let Some(win) = app.active_window() {
                win.close();
            }
        });
    }
    app.add_action(&close);
    app.set_accels_for_action("app.close", &["<primary>w"]);

    let about = gio::SimpleAction::new("about", None);
    {
        let app = app.clone();
        about.connect_activate(move |_, _| {
            if let Some(win) = app.active_window() {
                show_about(&win);
            }
        });
    }
    app.add_action(&about);

    let shortcuts = gio::SimpleAction::new("shortcuts", None);
    {
        let app = app.clone();
        shortcuts.connect_activate(move |_, _| {
            if let Some(win) = app.active_window() {
                show_shortcuts(&win);
            }
        });
    }
    app.add_action(&shortcuts);
    app.set_accels_for_action("app.shortcuts", &["<primary>question"]);

    // Window-scoped actions, registered by `HomeView`. Declaring the
    // accelerators here means they work without each window re-registering them.
    app.set_accels_for_action("win.toggle-sidebar", &["F9"]);
    app.set_accels_for_action("win.show-settings", &["<primary>comma"]);
}

/// Show the standard GNOME About window.
pub fn show_about(parent: &gtk4::Window) {
    let about = libadwaita::AboutDialog::new();
    about.set_application_name("SpacePods Linux");
    about.set_application_icon("com.spacepods.ui");
    about.set_version(libspacepods::VERSION);
    about.set_developer_name("Imnotndesh");
    about.set_developers(&["Imnotndesh"]);
    about.set_website("https://github.com/Imnotndesh/spacepods-linux");
    about.set_issue_url("https://github.com/Imnotndesh/spacepods-linux/issues");
    about.set_license_type(gtk4::License::Gpl30Only);
    about.set_copyright("\u{00A9} 2026 Imnotndesh");
    about.present(Some(parent));
}

/// Show the standard keyboard shortcuts window.
pub fn show_shortcuts(parent: &gtk4::Window) {
    let window = gtk4::ShortcutsWindow::builder()
        .title("Keyboard Shortcuts")
        .transient_for(parent)
        .modal(true)
        .build();
    window.set_section_name(Some("shortcuts"));
    window.set_view_name(Some("SpacePods"));

    let add = |group: &gtk4::ShortcutsGroup, title: &str, accel: &str| {
        let shortcut = gtk4::ShortcutsShortcut::builder()
            .title(title)
            .accelerator(accel)
            .build();
        group.add_shortcut(&shortcut);
    };

    let general = gtk4::ShortcutsGroup::builder().title("General").build();
    add(&general, "Quit", "<Primary>q");
    add(&general, "Close Window", "<Primary>w");
    add(&general, "Settings", "<Primary>comma");
    add(&general, "Toggle Sidebar", "F9");

    let section = gtk4::ShortcutsSection::builder()
        .title("General")
        .build();
    section.set_section_name(Some("shortcuts"));
    section.set_view_name(Some("SpacePods"));
    section.add_group(&general);
    window.add_section(&section);

    window.present();
}
