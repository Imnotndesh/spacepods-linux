use serde::{Serialize, Deserialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnownDevice {
    pub name: String,
    pub address: String,
    pub last_connected: u64,
}

fn devices_file() -> PathBuf {
    glib::user_data_dir().join("spacepods").join("devices.json")
}

pub fn load_known_devices() -> Vec<KnownDevice> {
    let path = devices_file();
    if !path.exists() {
        return vec![];
    }
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn remove_known_device(address: &str) {
    let mut devices = load_known_devices();
    devices.retain(|d| d.address != address);
    let path = devices_file();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(&path, serde_json::to_string_pretty(&devices).unwrap());
}

pub fn add_known_device(name: String, address: String) {
    let mut devices = load_known_devices();
    devices.retain(|d| d.address != address);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    devices.push(KnownDevice { name, address, last_connected: now });
    devices.sort_by(|a, b| b.last_connected.cmp(&a.last_connected));
    devices.truncate(10);

    let path = devices_file();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(&path, serde_json::to_string_pretty(&devices).unwrap());
}

