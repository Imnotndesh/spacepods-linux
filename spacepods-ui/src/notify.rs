//! Desktop notifications, through GNotification.
//!
//! GNotification is the freedesktop-common route: GNOME, KDE, Ubuntu and Fedora all show
//! it, and under Flatpak it goes through the Notifications portal. Notifications are for
//! changes the person would otherwise miss, such as the earbuds dropping out. Each kind
//! has its own id, so a repeat replaces the earlier notice instead of stacking up.

use gtk4::gio;
use gtk4::prelude::*;

/// Send a notification. Silently does nothing if the application is not running yet.
pub fn send(id: &str, title: &str, body: &str) {
    let Some(app) = gio::Application::default() else { return };
    let notification = gio::Notification::new(title);
    notification.set_body(Some(body));
    notification.set_icon(&gio::ThemedIcon::new(crate::prefs::APP_ID));
    app.send_notification(Some(id), &notification);
}
