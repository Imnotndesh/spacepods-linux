//! Application preferences, stored in GSettings.
//!
//! Under Flatpak, GSettings uses the sandbox's keyfile backend automatically, so no
//! dconf access is needed. The schema is `data/<app-id>.gschema.xml` and must be
//! installed (and compiled) for these keys to persist. Without it, the defaults are
//! used and nothing is saved; a warning is logged once.

use gtk4::gio::{self, prelude::*};
use std::sync::Once;

/// Application ID, which is also the schema ID.
pub const APP_ID: &str = "com.github.imnotndesh.spacepods-linux";

pub const AUTOSTART: &str = "autostart";
pub const DISCLAIMER_DISMISSED: &str = "disclaimer-dismissed";
pub const RENDERER: &str = "renderer";

static WARN_ONCE: Once = Once::new();

/// The settings object, or `None` when the schema is not installed.
pub fn settings() -> Option<gio::Settings> {
    let source = gio::SettingsSchemaSource::default()?;
    if source.lookup(APP_ID, true).is_none() {
        WARN_ONCE.call_once(|| {
            crate::log::Log::warn(
                "PREFS",
                "settings schema not installed; preferences will not be saved",
            );
        });
        return None;
    }
    Some(gio::Settings::new(APP_ID))
}

/// Read a boolean preference. Missing schema gives `false`.
pub fn get_bool(key: &str) -> bool {
    settings().map(|s| s.boolean(key)).unwrap_or(false)
}

/// Write a boolean preference. Missing schema is a no-op.
pub fn set_bool(key: &str, value: bool) {
    if let Some(s) = settings() {
        if let Err(e) = s.set_boolean(key, value) {
            crate::log::Log::warn("PREFS", &format!("could not save {}: {}", key, e));
        }
    }
}

/// One-time import of the old `settings.json`, if it exists and the schema is installed.
/// The file is renamed afterwards so the import never runs twice.
pub fn migrate_legacy_settings() {
    let legacy = glib::user_config_dir().join("spacepods").join("settings.json");
    if !legacy.exists() {
        return;
    }
    let Some(settings) = settings() else { return };
    let Ok(text) = std::fs::read_to_string(&legacy) else { return };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else { return };

    for (old, key) in [
        ("autostart", AUTOSTART),
        ("disclaimer_dismissed", DISCLAIMER_DISMISSED),
    ] {
        if let Some(b) = value.get(old).and_then(|v| v.as_bool()) {
            let _ = settings.set_boolean(key, b);
        }
    }
    let _ = std::fs::rename(&legacy, legacy.with_extension("json.migrated"));
    crate::log::Log::info("PREFS", "imported settings.json into GSettings");
}

/// Read a string preference. Missing schema gives an empty string.
pub fn get_string(key: &str) -> String {
    settings().map(|s| s.string(key).to_string()).unwrap_or_default()
}

/// Write a string preference. Missing schema is a no-op.
pub fn set_string(key: &str, value: &str) {
    if let Some(s) = settings() {
        if let Err(e) = s.set_string(key, value) {
            crate::log::Log::warn("PREFS", &format!("could not save {}: {}", key, e));
        }
    }
}
