//! Start SpacePods when the person logs in.
//!
//! Under Flatpak the sandbox cannot write the host's autostart folder. The
//! Background portal grants the request, and the desktop records it. Outside Flatpak
//! a desktop entry is written instead.

use futures::StreamExt;
use std::collections::HashMap;
use std::time::Duration;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};

const PORTAL_DEST: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const BACKGROUND_IFACE: &str = "org.freedesktop.portal.Background";
const REQUEST_IFACE: &str = "org.freedesktop.portal.Request";

/// How long to wait for the person to answer the portal's dialog.
const PORTAL_TIMEOUT: Duration = Duration::from_secs(120);

/// What happened when autostart was requested.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Enabled,
    Disabled,
    /// The person declined in the portal's dialog.
    Denied,
}

/// Turn autostart on or off.
pub async fn set_enabled(enable: bool) -> Result<Outcome, String> {
    if std::env::var_os("FLATPAK_ID").is_some() {
        portal_request(enable).await
    } else {
        desktop_entry(enable)
    }
}

async fn portal_request(enable: bool) -> Result<Outcome, String> {
    let conn = zbus::Connection::session().await.map_err(|e| e.to_string())?;

    // The portal names its request object after our unique name and the handle token.
    // Subscribe to that path before calling, so an early Response is not missed.
    let token = format!("spacepods{}", std::process::id());
    let sender = conn
        .unique_name()
        .map(|n| n.as_str().trim_start_matches(':').replace('.', "_"))
        .unwrap_or_default();
    let expected = format!("{}/request/{}/{}", PORTAL_PATH, sender, token);
    let request = zbus::Proxy::new(&conn, PORTAL_DEST, expected.as_str(), REQUEST_IFACE)
        .await
        .map_err(|e| e.to_string())?;
    let mut responses = request.receive_signal("Response").await.map_err(|e| e.to_string())?;

    let background = zbus::Proxy::new(&conn, PORTAL_DEST, PORTAL_PATH, BACKGROUND_IFACE)
        .await
        .map_err(|e| e.to_string())?;

    let mut options: HashMap<&str, Value> = HashMap::new();
    options.insert("handle_token", Value::from(token.as_str()));
    options.insert(
        "reason",
        Value::from("SpacePods can start when you log in, so your earbuds are ready."),
    );
    options.insert("autostart", Value::from(enable));
    options.insert("background", Value::from(true));
    options.insert("commandline", Value::from(vec!["spacepods-ui", "--background"]));
    options.insert("dbus-activatable", Value::from(false));

    let reply = background
        .call_method("RequestBackground", &("", options))
        .await
        .map_err(|e| e.to_string())?;
    let returned: OwnedObjectPath = reply.body().deserialize().map_err(|e| e.to_string())?;
    if returned.as_str() != expected {
        // The portal chose a different path. Fall back to waiting on the one it gave us.
        return Err("portal used an unexpected request path".into());
    }

    let message = tokio::time::timeout(PORTAL_TIMEOUT, responses.next())
        .await
        .map_err(|_| "the portal did not answer".to_string())?
        .ok_or_else(|| "the portal closed the request".to_string())?;
    let (code, results): (u32, HashMap<String, OwnedValue>) =
        message.body().deserialize().map_err(|e| e.to_string())?;

    if code != 0 {
        return Ok(Outcome::Denied);
    }
    let granted = results
        .get("autostart")
        .and_then(|v| bool::try_from(v).ok())
        .unwrap_or(enable);
    Ok(match (enable, granted) {
        (true, true) => Outcome::Enabled,
        (false, _) => Outcome::Disabled,
        (true, false) => Outcome::Denied,
    })
}

fn desktop_entry(enable: bool) -> Result<Outcome, String> {
    let path = glib::user_config_dir()
        .join("autostart")
        .join(format!("{}.desktop", crate::prefs::APP_ID));
    if !enable {
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        }
        return Ok(Outcome::Disabled);
    }

    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let content = format!(
        "[Desktop Entry]\nType=Application\nName=SpacePods Linux\nComment=SpacePods earbuds manager\n\
         Exec={} --background\nIcon={}\nX-GNOME-Autostart-enabled=true\n",
        exe.display(),
        crate::prefs::APP_ID
    );
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, content).map_err(|e| e.to_string())?;
    Ok(Outcome::Enabled)
}
