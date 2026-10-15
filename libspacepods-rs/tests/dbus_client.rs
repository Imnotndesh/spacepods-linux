//! The client prefers the session-bus interface when the daemon owns its name.
//! Needs a session bus; run with `dbus-run-session -- cargo test --test dbus_client`.
//! Without `DBUS_SESSION_BUS_ADDRESS` the test is skipped.

use libspacepods::ipc::{SpacePodsClient, SpacePodsService};
use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread")]
async fn client_uses_dbus_when_daemon_owns_the_name() {
    if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none() {
        eprintln!("skipping: no session bus");
        return;
    }

    let dir = std::env::temp_dir().join(format!("spacepods-dbus-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    let socket = dir.join("spacepods.sock");

    let p = socket.clone();
    let task = tokio::spawn(async move {
        let mut svc = SpacePodsService::new(Some(p)).await;
        svc.run().await
    });

    // Wait until the daemon has registered its bus name.
    let mut client = None;
    for _ in 0..200 {
        if let Ok(c) = SpacePodsClient::connect_dbus().await {
            client = Some(c);
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let mut client = client.expect("daemon never registered on the session bus");

    assert_eq!(client.transport_name(), "dbus");
    assert!(client.ping().await.unwrap());

    // The default connect must also choose D-Bus now that the name is owned.
    let default = SpacePodsClient::connect(None).await.unwrap();
    assert_eq!(default.transport_name(), "dbus");

    // Subscribing over D-Bus needs no request; it must succeed.
    let _rx = client.subscribe().await.unwrap();

    task.abort();
}
