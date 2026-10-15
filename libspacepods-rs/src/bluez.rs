//! Bluetooth adapter state from BlueZ, read over the system bus.
//!
//! BlueZ keeps the adapter's `Powered` flag on `org.bluez.Adapter1`. When Bluetooth is
//! switched off (in settings, or by a radio kill switch) the flag is false, and every
//! scan or connect fails with a generic error. Reading the flag first lets the daemon
//! report "Bluetooth is off" instead.

/// Object path of the default adapter.
pub const ADAPTER_PATH: &str = "/org/bluez/hci0";

/// Whether the adapter is powered on. `None` if BlueZ cannot be read (no system bus,
/// no adapter, or BlueZ not running). Callers treat `None` as "try anyway".
pub async fn adapter_powered() -> Option<bool> {
    let conn = zbus::Connection::system().await.ok()?;
    let proxy = zbus::Proxy::new(
        &conn,
        "org.bluez",
        ADAPTER_PATH,
        "org.freedesktop.DBus.Properties",
    )
    .await
    .ok()?;
    let reply = proxy
        .call_method("Get", &("org.bluez.Adapter1", "Powered"))
        .await
        .ok()?;
    let value: zbus::zvariant::OwnedValue = reply.body().deserialize().ok()?;
    bool::try_from(value).ok()
}

/// True only when BlueZ says the adapter is off. Unknown counts as not off.
pub async fn is_off() -> bool {
    adapter_powered().await == Some(false)
}

/// Object path of a device on the default adapter, from its Bluetooth address
/// (`AA:BB:CC:DD:EE:FF` becomes `/org/bluez/hci0/dev_AA_BB_CC_DD_EE_FF`).
pub fn device_path(address: &str) -> String {
    format!("{}/dev_{}", ADAPTER_PATH, address.replace(':', "_"))
}

/// Ask BlueZ to drop every link to the device (audio and control). Used by a
/// force disconnect, which must also clear the audio link that the BLE layer
/// does not own. A device BlueZ does not know is not an error.
pub async fn disconnect_device(address: &str) -> Result<(), String> {
    let conn = zbus::Connection::system().await.map_err(|e| e.to_string())?;
    let proxy = zbus::Proxy::new(&conn, "org.bluez", device_path(address), "org.bluez.Device1")
        .await
        .map_err(|e| e.to_string())?;
    match proxy.call_method("Disconnect", &()).await {
        Ok(_) => Ok(()),
        // Already gone is what the caller wants.
        Err(zbus::Error::MethodError(name, _, _))
            if name.as_str().contains("DoesNotExist") || name.as_str().contains("NotConnected") =>
        {
            Ok(())
        }
        Err(e) => Err(e.to_string()),
    }
}

/// The Bluetooth Audio Sink profile (A2DP), by its short UUID.
pub const A2DP_SINK_UUID: &str = "0000110b-0000-1000-8000-00805f9b34fb";

/// Battery percentage BlueZ reports for the device (`org.bluez.Battery1`). BlueZ fills
/// this from the standard battery service when the device exposes one. `None` if BlueZ
/// has no value.
pub async fn battery_percentage(address: &str) -> Option<u8> {
    let conn = zbus::Connection::system().await.ok()?;
    let proxy = zbus::Proxy::new(
        &conn,
        "org.bluez",
        device_path(address),
        "org.freedesktop.DBus.Properties",
    )
    .await
    .ok()?;
    let reply = proxy
        .call_method("Get", &("org.bluez.Battery1", "Percentage"))
        .await
        .ok()?;
    let value: zbus::zvariant::OwnedValue = reply.body().deserialize().ok()?;
    u8::try_from(value).ok().filter(|v| *v <= 100)
}

/// Whether the audio link to the device is up: BlueZ has a media transport under the
/// device's object path. This is what makes audio play, so it is what "audio connected"
/// means here.
pub async fn audio_connected(address: &str) -> bool {
    let Ok(conn) = zbus::Connection::system().await else { return false };
    let Ok(proxy) = zbus::Proxy::new(
        &conn,
        "org.bluez",
        "/",
        "org.freedesktop.DBus.ObjectManager",
    )
    .await
    else {
        return false;
    };
    let Ok(reply) = proxy.call_method("GetManagedObjects", &()).await else { return false };
    type Objects = std::collections::HashMap<
        zbus::zvariant::OwnedObjectPath,
        std::collections::HashMap<String, std::collections::HashMap<String, zbus::zvariant::OwnedValue>>,
    >;
    let Ok(objects) = reply.body().deserialize::<Objects>() else { return false };
    let prefix = device_path(address);
    objects.iter().any(|(path, interfaces)| {
        path.as_str().starts_with(&prefix) && interfaces.contains_key("org.bluez.MediaTransport1")
    })
}

/// Connect the audio profiles of a device that the control link has already reached.
///
/// Does nothing if audio is already up, so it is safe to call on every connect. Returns
/// whether audio is up afterwards. The attempt is bounded: it waits a few seconds for the
/// transport and then gives up, rather than retrying in a loop.
pub async fn ensure_audio(address: &str) -> bool {
    if audio_connected(address).await {
        return true;
    }
    let Ok(conn) = zbus::Connection::system().await else { return false };
    let Ok(proxy) = zbus::Proxy::new(&conn, "org.bluez", device_path(address), "org.bluez.Device1").await
    else {
        return false;
    };
    if let Err(e) = proxy.call_method("Connect", &()).await {
        crate::log::warn("AUDIO", &format!("audio connect failed: {}", e));
        return false;
    }
    for _ in 0..20 {
        if audio_connected(address).await {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    false
}

/// The audio-side (classic) address for a device, given either its BLE or its classic
/// address. `None` if BlueZ has no paired audio side for it.
pub async fn audio_side_of(address: &str) -> Option<String> {
    if let Some(audio) = audio_address_for(address).await {
        return Some(audio);
    }
    devices()
        .await
        .into_iter()
        .find(|d| d.paired && !d.alias.ends_with(" BLE") && d.address.eq_ignore_ascii_case(address))
        .map(|d| d.address)
}

/// One BlueZ device, as read from the object manager.
#[derive(Debug, Clone)]
pub struct DeviceEntry {
    pub address: String,
    pub alias: String,
    pub paired: bool,
}

type ManagedObjects = std::collections::HashMap<
    zbus::zvariant::OwnedObjectPath,
    std::collections::HashMap<String, std::collections::HashMap<String, zbus::zvariant::OwnedValue>>,
>;

async fn managed_objects() -> Option<ManagedObjects> {
    let conn = zbus::Connection::system().await.ok()?;
    let proxy = zbus::Proxy::new(&conn, "org.bluez", "/", "org.freedesktop.DBus.ObjectManager")
        .await
        .ok()?;
    let reply = proxy.call_method("GetManagedObjects", &()).await.ok()?;
    reply.body().deserialize::<ManagedObjects>().ok()
}

/// Every device BlueZ knows about on the default adapter.
pub async fn devices() -> Vec<DeviceEntry> {
    let Some(objects) = managed_objects().await else { return Vec::new() };
    objects
        .iter()
        .filter_map(|(_, interfaces)| {
            let props = interfaces.get("org.bluez.Device1")?;
            let address = props.get("Address").and_then(|v| String::try_from(v.clone()).ok())?;
            let alias = props
                .get("Alias")
                .and_then(|v| String::try_from(v.clone()).ok())
                .unwrap_or_default();
            let paired = props
                .get("Paired")
                .and_then(|v| bool::try_from(v.clone()).ok())
                .unwrap_or(false);
            Some(DeviceEntry { address, alias, paired })
        })
        .collect()
}

/// The classic (audio) address of a device whose control link is the BLE address.
///
/// BlueZ lists the earbuds twice: the BLE side as "<name> BLE", and the audio side under the
/// plain name. The audio side is the paired device whose name matches. `None` if there is no
/// such pair, in which case there is no audio to connect.
pub async fn audio_address_for(ble_address: &str) -> Option<String> {
    let all = devices().await;
    let ble = all.iter().find(|d| d.address.eq_ignore_ascii_case(ble_address))?;
    let base = ble.alias.strip_suffix(" BLE")?;
    all.iter()
        .find(|d| d.paired && d.alias == base && !d.address.eq_ignore_ascii_case(ble_address))
        .map(|d| d.address.clone())
}
