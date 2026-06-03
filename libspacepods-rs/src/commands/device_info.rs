use crate::errors::Result;
use crate::protocol::{CMD_HANDSHAKE, TlvParser};
use crate::SpaceBuds;
use std::time::Duration;

pub struct DeviceInfoController {
    buds: SpaceBuds,
}

impl DeviceInfoController {
    pub fn new(buds: SpaceBuds) -> Self {
        Self { buds }
    }

    pub async fn get_multiple(&self, tags: &[u8]) -> Result<Vec<(u8, Vec<u8>)>> {
        let mut payload = vec![0xFF, 0x00];
        for &tag in tags {
            payload.push(tag);
            payload.push(0x00);
        }
        self.buds
            .with_connection(|conn| async move {
                let packet = crate::protocol::Packet::new_request(
                    conn.next_seq().await,
                    CMD_HANDSHAKE,
                    payload,
                );
                let mut response_rx = conn.response_tx.subscribe();
                while response_rx.try_recv().is_ok() {}
                conn.write(&packet).await?;

                tokio::select! {
                    result = async {
                        loop {
                            match response_rx.recv().await {
                                Ok(p) if p.cmd_id == CMD_HANDSHAKE => {
                                    let mut parser = TlvParser::new(&p.payload);
                                    let mut results = Vec::new();
                                    while let Some((tag, value)) = parser.next() {
                                        results.push((tag, value.to_vec()));
                                    }
                                    return Some(results);
                                }
                                Ok(_) => continue,
                                Err(_) => return None,
                            }
                        }
                    } => Ok(result.unwrap_or_default()),
                    _ = tokio::time::sleep(Duration::from_secs(3)) => Ok(Vec::new()),
                }
            })
            .await
    }
}