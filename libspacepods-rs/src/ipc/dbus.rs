//! Session-bus interface for the daemon.
//!
//! The daemon owns [`BUS_NAME`] on the user's session bus and serves [`OBJECT_PATH`].
//! Access is limited to the user's own session, and the name is the only thing a
//! sandboxed client needs (`--talk-name` in Flatpak), so no other bus access is granted.
//!
//! The interface mirrors the socket protocol. `Request` takes the same JSON command
//! body (without the socket's `id` field) and returns the same JSON reply, so both
//! transports share one dispatch path. `Status` and `StatusChanged` carry the device
//! state. Typed methods can be added later without breaking this surface.

use crate::ipc::protocol::{DeviceStatus, IpcResult, ServiceCommand, PROTOCOL_VERSION};
use crate::ErrorCode;
use crate::ipc::service::SpacePodsService;
use crate::SpaceBuds;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

/// Well-known bus name owned by the daemon.
pub const BUS_NAME: &str = "com.github.imnotndesh.SpacePods.Daemon1";
/// Object path that serves the interface.
pub const OBJECT_PATH: &str = "/com/github/imnotndesh/SpacePods/Daemon1";

/// Shared state the interface needs. Cloned cheaply; every field is reference-counted.
#[derive(Clone)]
pub(crate) struct DaemonState {
    pub buds: SpaceBuds,
    pub status: Arc<RwLock<DeviceStatus>>,
    pub status_tx: broadcast::Sender<DeviceStatus>,
    pub manual: Arc<AtomicBool>,
    pub connecting: Arc<AtomicBool>,
}

/// The exported object. Methods here are thin wrappers over the shared dispatcher.
struct Daemon1 {
    state: DaemonState,
}

#[zbus::interface(name = "com.github.imnotndesh.SpacePods.Daemon1")]
impl Daemon1 {
    /// Run one command. `json` uses the socket's command schema, for example
    /// `{"cmd":"anc","args":{"mode":"anc"}}`. Returns the reply as JSON.
    async fn request(&self, json: &str) -> String {
        let s = &self.state;
        let reply = match serde_json::from_str::<ServiceCommand>(json) {
            Ok(ServiceCommand::Subscribe) | Ok(ServiceCommand::Unsubscribe) => IpcResult::error(
                ErrorCode::InvalidArgument,
                "status is pushed on StatusChanged; no subscribe call is needed",
            ),
            Ok(cmd) => {
                SpacePodsService::execute_command(cmd, &s.buds, &s.status, &s.status_tx, &s.manual, &s.connecting)
                    .await
            }
            Err(e) => IpcResult::error(ErrorCode::InvalidArgument, format!("invalid command: {}", e)),
        };
        serde_json::to_string(&reply).unwrap_or_else(|_| {
            r#"{"type":"error","message":"internal: could not encode reply"}"#.to_string()
        })
    }

    /// Current device status as JSON (same shape as the socket's `status` reply).
    #[zbus(property)]
    async fn status(&self) -> String {
        let status = self.state.status.read().await;
        serde_json::to_string(&*status).unwrap_or_default()
    }

    /// Version of the request and status schema.
    #[zbus(property)]
    fn protocol_version(&self) -> u32 {
        PROTOCOL_VERSION
    }
}

/// Register the bus name, export the object, and forward status changes as signals.
///
/// Returns the connection; the caller must keep it alive for as long as the
/// interface should stay available.
pub(crate) async fn serve(state: DaemonState) -> zbus::Result<zbus::Connection> {
    let mut status_rx = state.status_tx.subscribe();

    let conn = zbus::connection::Builder::session()?
        .name(BUS_NAME)?
        .serve_at(OBJECT_PATH, Daemon1 { state })?
        .build()
        .await?;

    let emit_conn = conn.clone();
    tokio::spawn(async move {
        loop {
            match status_rx.recv().await {
                Ok(status) => {
                    let Ok(json) = serde_json::to_string(&status) else { continue };
                    let Ok(emitter) = zbus::object_server::SignalEmitter::new(&emit_conn, OBJECT_PATH) else {
                        continue;
                    };
                    if let Err(e) = emitter.emit(BUS_NAME, "StatusChanged", &json).await {
                        crate::log::warn("DBUS", &format!("StatusChanged emit failed: {}", e));
                    }
                }
                // A slow listener missed some updates. The next one is still current.
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    // Watch our own bus name. If it is ever released, say so and ask for it again, so a
    // daemon that is still running is never silently unreachable.
    let watch = conn.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            let owned = match zbus::fdo::DBusProxy::new(&watch).await {
                Ok(proxy) => proxy.name_has_owner(BUS_NAME.try_into().unwrap()).await.unwrap_or(false),
                Err(_) => false,
            };
            if !owned {
                crate::log::warn("DBUS", &format!("{} was released; requesting it again", BUS_NAME));
                match watch.request_name(BUS_NAME).await {
                    Ok(()) => crate::log::info("DBUS", "bus name acquired again"),
                    Err(e) => crate::log::warn("DBUS", &format!("could not request bus name: {}", e)),
                }
            }
        }
    });

    Ok(conn)
}
