//! End-to-end checks for the daemon socket: refusal of a second instance, the
//! ownership guard in `Drop`, and the bounded-frame reply. These run a real
//! `SpacePodsService` on a temporary socket; no Bluetooth hardware is needed
//! because the device side fails quietly when no adapter is present.

use libspacepods::ipc::{SpacePodsClient, SpacePodsService};
use libspacepods::Error;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

fn temp_socket(tag: &str) -> PathBuf {
    // These tests exercise the socket only. Keep them off the real session bus.
    // SAFETY: set before any service starts in this process; tests only read it.
    unsafe { std::env::set_var("SPACEPODS_NO_DBUS", "1") };
    let dir = std::env::temp_dir().join(format!("spacepods-it-{}-{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    dir.join("spacepods.sock")
}

async fn wait_for_socket(path: &PathBuf) {
    for _ in 0..200 {
        if path.exists() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("socket never appeared at {}", path.display());
}

#[tokio::test(flavor = "multi_thread")]
async fn second_instance_is_refused_and_first_keeps_running() {
    let path = temp_socket("refuse");

    let first_path = path.clone();
    let first = tokio::spawn(async move {
        let mut svc = SpacePodsService::new(Some(first_path)).await;
        svc.run().await
    });
    wait_for_socket(&path).await;

    // A second daemon on the same path must refuse, and must not delete the socket.
    let mut second = SpacePodsService::new(Some(path.clone())).await;
    let result = second.run().await;
    assert!(
        matches!(result, Err(Error::AlreadyRunning { .. })),
        "second instance should be refused, got {:?}",
        result
    );
    drop(second);
    assert!(path.exists(), "dropping a refused instance removed the live socket");

    // The first daemon still answers.
    let mut client = SpacePodsClient::connect(Some(path.clone())).await.unwrap();
    assert!(client.ping().await.unwrap());

    first.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn socket_file_is_0600() {
    let path = temp_socket("mode");
    let p = path.clone();
    let task = tokio::spawn(async move {
        let mut svc = SpacePodsService::new(Some(p)).await;
        svc.run().await
    });
    wait_for_socket(&path).await;
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    task.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn oversized_frame_gets_error_and_disconnect() {
    let path = temp_socket("oversize");
    let p = path.clone();
    let task = tokio::spawn(async move {
        let mut svc = SpacePodsService::new(Some(p)).await;
        svc.run().await
    });
    wait_for_socket(&path).await;

    let mut stream = UnixStream::connect(&path).await.unwrap();
    // 70 KiB with no newline, above the 64 KiB frame limit.
    let _ = stream.write_all(&vec![b'a'; 70 * 1024]).await;

    let mut reply = Vec::new();
    let _ = tokio::time::timeout(Duration::from_secs(5), stream.read_to_end(&mut reply)).await;
    let text = String::from_utf8_lossy(&reply);
    assert!(text.contains("payload too large"), "reply was: {}", text);

    task.abort();
}
