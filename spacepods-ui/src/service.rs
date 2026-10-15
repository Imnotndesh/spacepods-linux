/// Check if the SpacePods daemon socket is reachable.
pub async fn check_daemon_running() -> bool {
    libspacepods::client::SpacePodsClient::connect(None).await.is_ok()
}
