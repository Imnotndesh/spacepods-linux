/// Check if the SpacePods daemon socket is reachable.
pub async fn check_daemon_running() -> bool {
    libspacepods::client::SpacePodsClient::connect(None).await.is_ok()
}

/// Write or remove the autostart .desktop entry for the app itself.
pub fn write_autostart_entry(enable: bool) {
    let path = glib::user_config_dir()
        .join("autostart")
        .join("spacepods.desktop");
    if enable {
        let content = "[Desktop Entry]\n\
            Type=Application\n\
            Name=SpacePods\n\
            Exec=spacepods\n\
            Icon=audio-headset\n\
            Comment=SpacePods earbuds manager\n\
            X-GNOME-Autostart-enabled=true\n";
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&path, content);
    } else {
        let _ = std::fs::remove_file(&path);
    }
}
