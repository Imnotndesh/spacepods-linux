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
use crate::prefs;
use crate::tray::{self, TrayCommand};

/// Poll the tray command channel on the GTK main loop. `try_recv` is
/// non-blocking so this keeps everything on one thread (no `Send` issues with
/// `Rc<WindowController>`). Window operations must happen on the main loop.
fn drain_tray_commands(
    rx: std::sync::mpsc::Receiver<TrayCommand>,
    window: Rc<WindowController>,
    ctx_slot: Rc<RefCell<Option<Weak<crate::context::AppContext>>>>,
    tray: tray::TrayHandle,
) {
    glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
        while let Ok(cmd) = rx.try_recv() {
            crate::log::Log::info("TRAY", &format!("command {:?} (window exists: {}, visible: {})", cmd, window.exists(), window.is_visible()));
            match cmd {
                TrayCommand::ShowWindow => window.present(),
                TrayCommand::HideWindow => window.hide(),
                TrayCommand::ShowPopup { x, y } => {
                    if let Some(ctx) = ctx_slot.borrow().as_ref().and_then(|w| w.upgrade()) {
                        crate::quick_settings::show_popup(&ctx, x, y);
                    }
                }
                TrayCommand::SetAncMode(mode) => {
                    let ctx = ctx_slot.borrow().as_ref().and_then(|w| w.upgrade());
                    // Show the choice straight away; it is reverted if the change fails.
                    let previous = tray::ANC_MODE_ATOMIC.swap(mode, std::sync::atomic::Ordering::Relaxed);
                    if let Some(ctx) = &ctx {
                        ctx.anc_mode.set(mode);
                    }
                    tray.refresh();
                    let label = match mode { 0 => "off", 1 => "anc", 2 => "transparency", _ => "off" };
                    let tray = tray.clone();
                    glib::spawn_future_local(async move {
                        use libspacepods::client::SpacePodsClient;
                        let result = match SpacePodsClient::connect(None).await {
                            Ok(mut c) => c.set_anc_mode(label).await,
                            Err(e) => Err(e),
                        };
                        if let Err(e) = result {
                            // The earbuds did not change, so the radio must not claim they did.
                            tray::ANC_MODE_ATOMIC.store(previous, std::sync::atomic::Ordering::Relaxed);
                            if let Some(ctx) = &ctx {
                                ctx.anc_mode.set(previous);
                                ctx.failed("Couldn't change the noise control", &e);
                            }
                            tray.refresh();
                        }
                    });
                }
                TrayCommand::Quit => {
                    // A real quit: closes the window and exits.
                    window.force_quit();
                }
            }
        }
        glib::ControlFlow::Continue
    });
}

pub fn run_app() -> glib::ExitCode {
    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    let _guard = rt.enter();

    prefs::migrate_legacy_settings();

    let app = Application::new(Some("com.github.imnotndesh.spacepods-linux"), Default::default());

    // After a crash, say so once, and say where the report is. Nothing is sent anywhere.
    app.connect_startup(|_| {
        let pending = libspacepods::crash::pending_reports();
        if let Some(first) = pending.first() {
            let folder = libspacepods::crash::reports_dir();
            crate::notify::send(
                "crash-report",
                "SpacePods closed unexpectedly",
                &format!("A report was saved to {}. Nothing was sent.", folder.display()),
            );
            crate::log::Log::warn("CRASH", &format!("previous crash report: {}", first.display()));
        }
        for report in &pending {
            libspacepods::crash::mark_reported(report);
        }
    });

    // `--background` is passed at login (see autostart.rs). The window is created but
    // not shown, so the app starts quietly in the background.
    let start_hidden = Rc::new(std::cell::Cell::new(false));
    app.add_main_option(
        "background",
        glib::Char::from(b'b'),
        glib::OptionFlags::NONE,
        glib::OptionArg::None,
        "Start without showing the window",
        None,
    );
    {
        let start_hidden = start_hidden.clone();
        app.connect_handle_local_options(move |_, options| {
            if options.contains("background") {
                start_hidden.set(true);
            }
            std::ops::ControlFlow::Continue(())
        });
    }
    install_app_actions(&app);
    let window_controller = WindowController::new();

    // Start the tray icon once. It runs on its own background thread (via the
    // blocking API); the returned receiver gives us the menu commands.
    let (tray_handle, tray_rx) = tray::spawn_tray();

    // Shared slot for the AppContext — populated by HomeView::new, read by
    // the tray command loop for ShowPopup (which needs ctx to query the daemon).
    let ctx_slot: Rc<RefCell<Option<std::rc::Weak<crate::context::AppContext>>>> =
        Rc::new(RefCell::new(None));
    drain_tray_commands(tray_rx, window_controller.clone(), ctx_slot.clone(), tray_handle.clone());

    let tray_handle = Rc::new(tray_handle);
    // A live tray keeps the app running with no window open, so the tray can bring the
    // window back. The hold lasts for the whole process; Quit still exits.
    if tray_handle.is_active() {
        std::mem::forget(app.hold());
    }
    let ctx_slot2 = ctx_slot.clone();

    app.connect_activate(move |app| {
        // A second launch activates the running instance. Show the window that already
        // exists instead of building another one.
        if window_controller.exists() {
            window_controller.present();
            return;
        }

        let window = ApplicationWindow::new(app);
        window_controller.set_window(&window);

        // Set app icon from resources
        gtk4::Window::set_default_icon_name("com.github.imnotndesh.spacepods-linux");

        // Register custom icons once a display is available
        if let Some(display) = gtk4::gdk::Display::default() {
            let icon_theme = gtk4::IconTheme::for_display(&display);
            icon_theme.add_resource_path("/com/spacepods/ui/icons");
        }

        window.set_title(Some("SpacePods"));
        // Development aid: SPACEPODS_DEV_SIZE=WIDTHxHEIGHT to test other window sizes.
        let (w, h) = std::env::var("SPACEPODS_DEV_SIZE")
            .ok()
            .and_then(|v| v.split_once('x').and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?))))
            .unwrap_or((850, 600));
        window.set_default_size(w, h);
        window.set_width_request(360);
        window.set_height_request(480);

        // Close-to-background: when the setting is enabled, closing the window
        // hides it (the tray keeps the app alive) instead of quitting. A tray
        // "Quit" bypasses this via WindowController::force_quit().
        let close_wc = window_controller.clone();
        let tray_active = tray_handle.is_active();
        window.connect_close_request(move |win| {
            // Real quit requested (tray Quit / etc.).
            if close_wc.force_close_requested() {
                close_wc.clear_force_close();
                if let Some(app) = win.application() {
                    app.quit();
                }
                return glib::Propagation::Proceed;
            }
            // With a live tray, closing hides the window. The tray brings it back, so
            // the window must not be destroyed and the app must keep running.
            if tray_active {
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
        let (setup_tray, setup_wc, setup_slot) = (tray.clone(), wc.clone(), slot.clone());
        let setup_page = SetupPage::new(move |status: Option<libspacepods::ipc::protocol::DeviceStatus>| {
            if let Some(win) = window_weak.upgrade() {
                enter_home(&win, &setup_tray, &setup_wc, &setup_slot, status);
            }
        });
        // Development aid: skip setup and open the home view directly (no device needed).
        if std::env::var_os("SPACEPODS_DEV_SKIP_SETUP").is_some() {
            enter_home(&window, &tray, &window_controller, &ctx_slot2, None);
        }
        toast_overlay.set_child(Some(&setup_page));
        if std::env::var_os("SPACEPODS_DEV_SKIP_SETUP").is_none() {
            window.set_content(Some(&toast_overlay));
        }
        if start_hidden.replace(false) {
            crate::log::Log::info("APP", "started in the background");
        } else {
            window.present();
        }
        // Development aid: save the window to a PNG after it settles, then quit. Set
        // SPACEPODS_SCREENSHOT to the output path. GNOME does not allow screen capture
        // from outside the compositor, so the window is rendered directly.
        if let Some(path) = std::env::var_os("SPACEPODS_SCREENSHOT") {
            let win = window.downgrade();
            let app = app.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(2500), move || {
                if let Some(win) = win.upgrade() {
                    if let Err(e) = save_window_png(&win, std::path::Path::new(&path)) {
                        crate::log::Log::warn("SHOT", &e);
                    }
                }
                app.quit();
            });
        }
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
    about.set_application_icon("com.github.imnotndesh.spacepods-linux");
    about.set_version(libspacepods::VERSION);
    about.set_developer_name("Imnotndesh");
    about.set_developers(&["Imnotndesh"]);
    about.set_website("https://github.com/Imnotndesh/spacepods-linux");
    about.set_issue_url("https://github.com/Imnotndesh/spacepods-linux/issues");
    about.set_license_type(gtk4::License::Gpl30Only);
    about.set_copyright("\u{00A9} 2026 Imnotndesh");
    about.present(Some(parent));
}

/// Show the keyboard shortcuts dialog (libadwaita 1.8+; replaces the deprecated
/// `GtkShortcutsWindow`).
pub fn show_shortcuts(parent: &gtk4::Window) {
    let dialog = libadwaita::ShortcutsDialog::new();
    let section = libadwaita::ShortcutsSection::new(Some("General"));
    for (title, accel) in [
        ("Quit", "<Primary>q"),
        ("Close Window", "<Primary>w"),
        ("Settings", "<Primary>comma"),
        ("Toggle Sidebar", "F9"),
    ] {
        section.add(libadwaita::ShortcutsItem::new(title, accel));
    }
    dialog.add(section);
    dialog.present(Some(parent));
}

/// Build the home view for a connected (or, in development, absent) device and show it
/// in place of the setup page.
fn enter_home(
    win: &libadwaita::ApplicationWindow,
    tray: &tray::TrayHandle,
    wc: &Rc<WindowController>,
    slot: &Rc<RefCell<Option<std::rc::Weak<crate::context::AppContext>>>>,
    status: Option<libspacepods::ipc::protocol::DeviceStatus>,
) {
    let ctx = crate::context::AppContext::new(
        ToastOverlay::new(),
        Some(tray.clone()),
        wc.clone(),
    );
    if let Some(s) = &status {
        ctx.product_id.set(s.product_id);
        ctx.capabilities.replace(s.capabilities.clone());
    }

    // Sync the tray's ANC radio-group to the shared cell.
    tray::ANC_MODE_ATOMIC.store(ctx.anc_mode.get(), std::sync::atomic::Ordering::Relaxed);

    // Keep the UI reactive to daemon status pushes.
    crate::context::spawn_status_subscription(ctx.clone());

    // Expose the context so the tray command loop can open the popup.
    *slot.borrow_mut() = Some(std::rc::Rc::downgrade(&ctx));

    let home_view = crate::home::HomeView::from_ctx(win, ctx);
    win.set_content(Some(&home_view));
}

/// Render `window` to a PNG file. Used only by the `SPACEPODS_SCREENSHOT` hook.
fn save_window_png(window: &libadwaita::ApplicationWindow, path: &std::path::Path) -> Result<(), String> {
    use gtk4::prelude::*;
    let paintable = gtk4::WidgetPaintable::new(Some(window));
    let (w, h) = (window.width() as f64, window.height() as f64);
    let snapshot = gtk4::Snapshot::new();
    paintable.snapshot(&snapshot, w, h);
    let node = snapshot.to_node().ok_or("nothing to render")?;
    let native = window.native().ok_or("window has no native surface")?;
    let renderer = native.renderer().ok_or("no renderer")?;
    let texture = renderer.render_texture(&node, None);
    texture.save_to_png(path).map_err(|e| e.to_string())?;
    Ok(())
}
