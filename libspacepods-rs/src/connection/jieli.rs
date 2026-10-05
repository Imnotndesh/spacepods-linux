//! JieLi RCSP connection.
//!
//! Used for devices whose control channel is the JieLi GATT service
//! (`0000ae00`, write `0000ae01`, notify `0000ae02`) such as the
//! `oraimo BoomPop Pro`. Authentication is a fixed-key challenge/response
//! (Bluetooth E1), see [`crate::protocol::e1`].

use crate::errors::{Error, Result};
use crate::protocol::e1;
use crate::protocol::jieli::{self, RcspEvent, RcspPacket, RcspStream};
use crate::protocol::{UUID_JIELI_NOTIFY, UUID_JIELI_WRITE};
use btleplug::api::{Characteristic, Peripheral as _, ValueNotification, WriteType};
use btleplug::platform::Peripheral;
use futures::stream::{Stream, StreamExt};
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, Mutex};
use tokio::time;

const AUTH_TIMEOUT: Duration = Duration::from_secs(5);
const CMD_TIMEOUT: Duration = Duration::from_secs(3);
/// Conservative ATT payload size for write-without-response.
const WRITE_CHUNK: usize = 20;

type NotificationStream = Pin<Box<dyn Stream<Item = ValueNotification> + Send>>;

#[derive(Clone)]
pub struct JieliConnection {
    peripheral: Arc<Peripheral>,
    write_char: Characteristic,
    seq: Arc<Mutex<u8>>,
    rx: broadcast::Sender<RcspEvent>,
    /// Last noise-control mode we set (the device ACKs sets without echoing
    /// the new state).
    anc_mode: Arc<Mutex<Option<u8>>>,
}

impl JieliConnection {
    pub async fn new(peripheral: Peripheral) -> Result<Self> {
        let peripheral = Arc::new(peripheral);
        peripheral.connect().await?;
        peripheral.discover_services().await?;
        time::sleep(Duration::from_millis(500)).await;

        let chars = peripheral.characteristics();
        let write_char = chars
            .iter()
            .find(|c| c.uuid == UUID_JIELI_WRITE)
            .cloned()
            .ok_or(Error::WriteCharNotFound)?;
        let notify_char = chars
            .iter()
            .find(|c| c.uuid == UUID_JIELI_NOTIFY)
            .cloned()
            .ok_or(Error::NotifyCharNotFound)?;

        peripheral.subscribe(&notify_char).await?;
        let mut notifications = peripheral.notifications().await?;
        time::sleep(Duration::from_millis(300)).await;

        // Authenticate before any RCSP command may be accepted. Retry a couple
        // of times as a just-disconnected device can drop the first attempt.
        let mut last_err = None;
        for attempt in 1..=3 {
            match Self::authenticate(&peripheral, &write_char, &mut notifications).await {
                Ok(()) => {
                    last_err = None;
                    break;
                }
                Err(e) => {
                    crate::log::warn(
                        "JIELI",
                        &format!("RCSP auth attempt {} failed: {}", attempt, e),
                    );
                    last_err = Some(e);
                    time::sleep(Duration::from_millis(500)).await;
                }
            }
        }
        if let Some(e) = last_err {
            return Err(e);
        }

        let (rx, _) = broadcast::channel(64);
        let rx_clone = rx.clone();
        tokio::spawn(async move {
            let mut stream = RcspStream::new();
            while let Some(notification) = notifications.next().await {
                if notification.uuid == UUID_JIELI_NOTIFY {
                    for event in stream.push(&notification.value) {
                        let _ = rx_clone.send(event);
                    }
                }
            }
        });

        Ok(Self {
            peripheral,
            write_char,
            seq: Arc::new(Mutex::new(0)),
            rx,
            anc_mode: Arc::new(Mutex::new(None)),
        })
    }

    // ── Authentication ──

    async fn authenticate(
        peripheral: &Peripheral,
        write_char: &Characteristic,
        notifications: &mut NotificationStream,
    ) -> Result<()> {
        crate::log::info("JIELI", "starting RCSP authentication");

        // 1. Reset the auth flag (a normal RCSP command).
        Self::write_raw(peripheral, write_char, &jieli::auth_reset_frame()).await?;
        time::sleep(Duration::from_millis(50)).await;

        // 2. App challenge: 0x00 || 16 random bytes.
        let challenge = jieli::random16();
        let mut msg = Vec::with_capacity(17);
        msg.push(0x00);
        msg.extend_from_slice(&challenge);
        Self::write_raw(peripheral, write_char, &msg).await?;

        // 3. Device must return 0x01 || E1(challenge).
        let device_reply = Self::wait_auth(notifications, 0x01).await?;
        if device_reply.len() < 17 {
            return Err(Error::Ipc("RCSP auth: short device reply".into()));
        }
        let expected = e1::jieli_encrypt(&challenge);
        if device_reply[1..17] != expected {
            return Err(Error::Ipc("RCSP auth: challenge response mismatch".into()));
        }
        Self::write_raw(peripheral, write_char, &jieli::AUTH_PASS).await?;

        // 4. Device challenge: 0x00 || 16 bytes.
        let device_challenge = Self::wait_auth(notifications, 0x00).await?;
        if device_challenge.len() < 17 {
            return Err(Error::Ipc("RCSP auth: short device challenge".into()));
        }
        let mut reply = Vec::with_capacity(17);
        reply.push(0x01);
        reply.extend_from_slice(&e1::jieli_encrypt(&device_challenge[1..17]));
        Self::write_raw(peripheral, write_char, &reply).await?;

        // 5. Device confirms with "pass".
        let pass = Self::wait_auth(notifications, 0x02).await?;
        if pass.len() < 5 || pass[..5] != jieli::AUTH_PASS {
            return Err(Error::Ipc("RCSP auth: device did not confirm".into()));
        }

        crate::log::info("JIELI", "RCSP authentication succeeded");
        Ok(())
    }

    async fn wait_auth(
        notifications: &mut NotificationStream,
        first_byte: u8,
    ) -> Result<Vec<u8>> {
        let deadline = time::sleep(AUTH_TIMEOUT);
        tokio::pin!(deadline);
        loop {
            tokio::select! {
                notification = notifications.next() => {
                    match notification {
                        Some(n) => {
                            if n.uuid == UUID_JIELI_NOTIFY
                                && !n.value.is_empty()
                                && n.value[0] == first_byte
                            {
                                return Ok(n.value);
                            }
                        }
                        None => return Err(Error::ConnectionLost),
                    }
                }
                _ = &mut deadline => {
                    return Err(Error::Timeout(AUTH_TIMEOUT));
                }
            }
        }
    }

    // ── Sending ──

    async fn next_seq(&self) -> u8 {
        let mut seq = self.seq.lock().await;
        let current = *seq;
        *seq = current.wrapping_add(1) & 0x3F;
        current
    }

    async fn write_raw(
        peripheral: &Peripheral,
        write_char: &Characteristic,
        data: &[u8],
    ) -> Result<()> {
        for chunk in data.chunks(WRITE_CHUNK) {
            peripheral
                .write(write_char, chunk, WriteType::WithoutResponse)
                .await?;
            time::sleep(Duration::from_millis(10)).await;
        }
        Ok(())
    }

    async fn write_frame(&self, packet: &RcspPacket) -> Result<()> {
        Self::write_raw(&self.peripheral, &self.write_char, &packet.to_bytes()).await
    }

    /// Send an Oraimo custom command and optionally wait for its response.
    pub async fn send_custom(
        &self,
        payload: Vec<u8>,
        expect_response: bool,
    ) -> Result<Option<RcspPacket>> {
        let sn = self.next_seq().await;
        let packet = RcspPacket::custom(sn, payload);
        let mut rx = self.rx.subscribe();
        self.write_frame(&packet).await?;

        if !expect_response {
            return Ok(None);
        }

        let deadline = time::sleep(CMD_TIMEOUT);
        tokio::pin!(deadline);
        loop {
            tokio::select! {
                event = rx.recv() => {
                    match event {
                        Ok(RcspEvent::Packet(p))
                            if !p.is_command
                                && p.op_code == jieli::CMD_CUSTOM
                                && p.op_code_sn == sn =>
                        {
                            crate::log::full(
                                "JIELI",
                                &format!("rx sn={} status={} param={:02X?}", p.op_code_sn, p.status, p.param),
                            );
                            return Ok(Some(p));
                        }
                        Ok(_) => {}
                        Err(_) => return Err(Error::ConnectionLost),
                    }
                }
                _ = &mut deadline => {
                    return Err(Error::Timeout(CMD_TIMEOUT));
                }
            }
        }
    }

    /// Send an Oraimo custom command without waiting (fire and forget).
    pub async fn send_custom_no_wait(&self, payload: Vec<u8>) -> Result<()> {
        self.send_custom(payload, false).await?;
        Ok(())
    }

    // ── High-level feature helpers ──

    /// Set the noise-control mode (`0` = off, `1` = ANC, `2` = transparency).
    pub async fn set_anc_mode(&self, mode: u8) -> Result<()> {
        self.send_custom(vec![jieli::ora::ANC, mode], true).await?;
        *self.anc_mode.lock().await = Some(mode);
        Ok(())
    }

    /// Query the current noise-control mode.
    pub async fn get_anc_mode(&self) -> Result<u8> {
        if let Some(mode) = *self.anc_mode.lock().await {
            return Ok(mode);
        }
        // Best-effort query; some firmware ACKs with an empty payload and
        // reports the state via an unsolicited notification instead.
        let response = self
            .send_custom(vec![jieli::ora::ANC, 0x00], true)
            .await?
            .ok_or_else(|| Error::Ipc("RCSP: no ANC response".into()))?;
        response
            .param
            .get(1)
            .copied()
            .ok_or_else(|| Error::Parse("RCSP: ANC response too short"))
    }

    /// Set the noise-control level (ANC/transparency strength).
    pub async fn set_anc_level(&self, mode: u8, level: u8) -> Result<()> {
        self.send_custom(
            vec![jieli::ora::ANC, mode, (level >> 8) as u8, level as u8],
            true,
        )
        .await?;
        Ok(())
    }

    /// Enable/disable multipoint (dual device).
    pub async fn set_dual_device(&self, enable: bool) -> Result<()> {
        self.send_custom(vec![jieli::ora::DUAL_DEVICE, if enable { 2 } else { 1 }], false)
            .await?;
        Ok(())
    }

    pub async fn is_connected(&self) -> bool {
        self.peripheral.is_connected().await.unwrap_or(false)
    }

    pub async fn disconnect(&self) -> Result<()> {
        self.peripheral.disconnect().await?;
        Ok(())
    }

    pub fn address(&self) -> String {
        self.peripheral.address().to_string()
    }

    pub async fn detect_product_id(&self) -> Option<u16> {
        if let Ok(Some(props)) = self.peripheral.properties().await {
            for data in props.manufacturer_data.values() {
                if let Some(beacon) = crate::beacon::DeviceBeacon::from_manufacturer_data(data) {
                    return Some(
                        crate::beacon::device_type_from_pid(beacon.product_id)
                            .unwrap_or(beacon.product_id),
                    );
                }
            }
        }
        None
    }

    pub async fn reconnect_with_rediscovery(&self) -> Result<()> {
        self.peripheral.disconnect().await?;
        time::sleep(Duration::from_millis(500)).await;
        self.peripheral.connect().await?;
        time::sleep(Duration::from_millis(500)).await;
        self.peripheral.discover_services().await?;
        time::sleep(Duration::from_millis(500)).await;
        Ok(())
    }
}
