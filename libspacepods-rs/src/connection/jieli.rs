//! JieLi RCSP connection.
//!
//! Used for devices whose control channel is the JieLi GATT service
//! (`0000ae00`, write `0000ae01`, notify `0000ae02`) such as the
//! `oraimo BoomPop Pro`. Authentication is a fixed-key challenge/response
//! (Bluetooth E1), see [`crate::protocol::e1`].

use crate::errors::{Error, Result};
use crate::protocol::e1;
use crate::protocol::jieli::{self, OraFeature, RcspEvent, RcspPacket, RcspStream};
use crate::protocol::{UUID_JIELI_NOTIFY, UUID_JIELI_WRITE};
use btleplug::api::{Characteristic, Peripheral as _, ValueNotification, WriteType};
use btleplug::platform::Peripheral;
use futures::stream::{Stream, StreamExt};
use std::collections::HashMap;
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
    /// Last voice-mode gains: `(left_max, right_max, left_cur, right_cur)`.
    anc_level: Arc<Mutex<(u16, u16, u16, u16)>>,
    /// Last reported parameter per Oraimo command id, from device
    /// notifications.
    state: Arc<Mutex<HashMap<u8, Vec<u8>>>>,
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
            // Don't leave the link open on failure: the next attempt must start
            // fresh or the device will skip the auth handshake.
            let _ = peripheral.disconnect().await;
            return Err(e);
        }

        let (rx, _) = broadcast::channel(64);
        let rx_clone = rx.clone();
        let state = Arc::new(Mutex::new(HashMap::new()));
        let state_clone = state.clone();
        tokio::spawn(async move {
            let mut stream = RcspStream::new();
            while let Some(notification) = notifications.next().await {
                if notification.uuid == UUID_JIELI_NOTIFY {
                    for event in stream.push(&notification.value) {
                        if let RcspEvent::Packet(ref p) = event {
                            crate::log::full(
                                "JIELI",
                                &format!(
                                    "pkt cmd={} op={:#x} sn={} status={} param={:02X?}",
                                    p.is_command, p.op_code, p.op_code_sn, p.status, p.param
                                ),
                            );
                            // Device-initiated commands carry state as
                            // `{command_id, 0|2, …}`.
                            if p.is_command && p.param.len() >= 2 {
                                let cmd = p.param[0];
                                if OraFeature::from_command_id(cmd).is_some()
                                    || cmd == jieli::ora::ANC
                                    || cmd == jieli::ora::DUAL_DEVICE_INFO
                                {
                                    state_clone.lock().await.insert(cmd, p.param.clone());
                                }
                            }
                        }
                        let _ = rx_clone.send(event);
                    }
                }
            }
            crate::log::warn("JIELI", "notification pump ended");
        });

        Ok(Self {
            peripheral,
            write_char,
            seq: Arc::new(Mutex::new(0)),
            rx,
            anc_mode: Arc::new(Mutex::new(None)),
            anc_level: Arc::new(Mutex::new((0, 0, 1, 1))),
            state,
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

    /// Send an RCSP packet and wait for the matching response.
    pub async fn send_rcsp(&self, packet: RcspPacket) -> Result<RcspPacket> {
        let sn = packet.op_code_sn;
        let op = packet.op_code;
        let mut rx = self.rx.subscribe();
        self.write_frame(&packet).await?;

        let deadline = time::sleep(CMD_TIMEOUT);
        tokio::pin!(deadline);
        loop {
            tokio::select! {
                event = rx.recv() => {
                    match event {
                        Ok(RcspEvent::Packet(p))
                            if !p.is_command && p.op_code == op && p.op_code_sn == sn =>
                        {
                            crate::log::full(
                                "JIELI",
                                &format!(
                                    "rx op={:#x} sn={} status={} param={:02X?}",
                                    p.op_code, p.op_code_sn, p.status, p.param
                                ),
                            );
                            return Ok(p);
                        }
                        Ok(_) => {}
                        Err(_) => return Err(Error::ConnectionLost),
                    }
                }
                _ = &mut deadline => {
                    crate::log::warn("JIELI", &format!("send_rcsp TIMEOUT op={:#x} sn={}", op, sn));
                    return Err(Error::Timeout(CMD_TIMEOUT));
                }
            }
        }
    }

    /// Send an Oraimo custom command and optionally wait for its response.
    pub async fn send_custom(
        &self,
        payload: Vec<u8>,
        expect_response: bool,
    ) -> Result<Option<RcspPacket>> {
        let sn = self.next_seq().await;
        let packet = RcspPacket::custom(sn, payload);
        if !expect_response {
            self.write_frame(&packet).await?;
            return Ok(None);
        }
        Ok(Some(self.send_rcsp(packet).await?))
    }

    /// Send an Oraimo custom command without waiting (fire and forget).
    pub async fn send_custom_no_wait(&self, payload: Vec<u8>) -> Result<()> {
        self.send_custom(payload, false).await?;
        Ok(())
    }

    // ── High-level feature helpers ──

    /// `CMD_ADV_SETTINGS` with an LTV payload (`modifyDeviceSettingsInfo`).
    pub async fn set_adv_setting(&self, ltv_type: u8, data: &[u8]) -> Result<()> {
        let param = jieli::ltv_packet(ltv_type, data);
        let sn = self.next_seq().await;
        let packet = RcspPacket::command(jieli::CMD_ADV_SETTINGS, sn, param);
        self.send_rcsp(packet).await?;
        Ok(())
    }

    /// Rename the device (`ADV_TYPE_RENAME`).
    pub async fn rename(&self, name: &str) -> Result<()> {
        self.set_adv_setting(jieli::ADV_TYPE_RENAME, name.as_bytes())
            .await
    }

    /// Set the noise-control mode (`0` = off, `1` = ANC, `2` = transparency).
    ///
    /// The reference app sends the device-reported `VoiceMode` for the target
    /// mode (with its max/current gain fields), not a zeroed one; query the
    /// full list first and reuse the matching entry.
    pub async fn set_anc_mode(&self, mode: u8) -> Result<()> {
        // Only ever send a VoiceMode the device itself reported. This avoids
        // writing bogus gain fields, which can leave the firmware in a bad
        // ANC state.
        let vm = self
            .get_all_voice_modes()
            .await?
            .into_iter()
            .find(|v| !v.is_empty() && v[0] == mode)
            .ok_or_else(|| Error::Parse("requested voice mode not offered by device"))?;
        let param = jieli::set_voice_mode_param_bytes(&vm);
        let sn = self.next_seq().await;
        let packet = RcspPacket::command(jieli::CMD_SET_SYS_INFO, sn, param);
        self.send_rcsp(packet).await?;
        *self.anc_mode.lock().await = Some(mode);
        *self.anc_level.lock().await = (
            u16::from_be_bytes([vm[1], vm[2]]),
            u16::from_be_bytes([vm[3], vm[4]]),
            u16::from_be_bytes([vm[5], vm[6]]),
            u16::from_be_bytes([vm[7], vm[8]]),
        );
        Ok(())
    }

    /// Read the device's list of voice modes (`mask 0x4000`, attr 14).
    pub async fn get_all_voice_modes(&self) -> Result<Vec<Vec<u8>>> {
        let sn = self.next_seq().await;
        let param = jieli::get_sys_info_param(0xFF, jieli::MASK_ALL_VOICE_MODES);
        let packet = RcspPacket::command(jieli::CMD_GET_SYS_INFO, sn, param);
        let resp = self.send_rcsp(packet).await?;
        let data = jieli::find_attr(&resp.param, jieli::ATTR_ALL_VOICE_MODES)
            .ok_or_else(|| Error::Parse("voice mode list attr missing"))?;
        if data.is_empty() {
            return Ok(Vec::new());
        }
        let count = data[0] as usize;
        let mut out = Vec::new();
        let mut i = 1;
        for _ in 0..count {
            if i + 9 > data.len() {
                break;
            }
            out.push(data[i..i + 9].to_vec());
            i += 9;
        }
        Ok(out)
    }

    /// Query the current noise-control mode via `GET_SYS_INFO` attr 13.
    pub async fn get_anc_mode(&self) -> Result<u8> {
        let sn = self.next_seq().await;
        let param = jieli::get_sys_info_param(0xFF, jieli::MASK_VOICE_MODE);
        let packet = RcspPacket::command(jieli::CMD_GET_SYS_INFO, sn, param);
        let resp = self.send_rcsp(packet).await?;
        let vm = jieli::find_attr(&resp.param, jieli::ATTR_VOICE_MODE)
            .ok_or_else(|| Error::Parse("voice mode attr missing"))?;
        if vm.is_empty() {
            return Err(Error::Parse("empty voice mode"));
        }
        if vm.len() >= 9 {
            *self.anc_level.lock().await = (
                u16::from_be_bytes([vm[1], vm[2]]),
                u16::from_be_bytes([vm[3], vm[4]]),
                u16::from_be_bytes([vm[5], vm[6]]),
                u16::from_be_bytes([vm[7], vm[8]]),
            );
        }
        let mode = vm[0];
        *self.anc_mode.lock().await = Some(mode);
        Ok(mode)
    }

    /// Set the noise-control level (ANC/transparency strength) via the same
    /// `SET_SYS_INFO` voice-mode attribute.
    pub async fn set_anc_level(&self, mode: u8, level: u8) -> Result<()> {
        let (lm, rm, _lc, _rc) = *self.anc_level.lock().await;
        let param = jieli::voice_mode_set_param(mode, lm, rm, level as u16, level as u16);
        let sn = self.next_seq().await;
        let packet = RcspPacket::command(jieli::CMD_SET_SYS_INFO, sn, param);
        self.send_rcsp(packet).await?;
        *self.anc_level.lock().await = (lm, rm, level as u16, level as u16);
        Ok(())
    }

    /// Current `(level, max)` for the noise-control mode, from the cached
    /// voice-mode gains.
    pub async fn get_anc_level(&self) -> Result<(u8, u8)> {
        let (lm, _rm, lc, _rc) = *self.anc_level.lock().await;
        Ok((lc.min(255) as u8, lm.min(255) as u8))
    }

    /// Enable/disable multipoint (dual device).
    pub async fn set_dual_device(&self, enable: bool) -> Result<()> {
        self.set_feature(OraFeature::DualDevice, enable).await
    }

    /// Set a boolean Oraimo feature (`{cmd, 2}` on / `{cmd, 1}` off).
    pub async fn set_feature(&self, feature: OraFeature, enabled: bool) -> Result<()> {
        let cmd = feature.command_id();
        self.send_custom(jieli::toggle_payload(cmd, enabled), true)
            .await?;
        self.state
            .lock()
            .await
            .insert(cmd, vec![cmd, if enabled { 2 } else { 1 }]);
        Ok(())
    }

    /// Query a boolean Oraimo feature.
    ///
    /// The firmware ACKs the query with an empty payload and reports the state
    /// as a device notification (`{cmd, 0|2}`), so this waits briefly for that
    /// notification and falls back to the last known value.
    pub async fn get_feature(&self, feature: OraFeature) -> Result<Option<bool>> {
        let cmd = feature.command_id();
        let mut rx = self.rx.subscribe();
        if let Err(e) = self.send_custom(jieli::query_payload(cmd), false).await {
            crate::log::full("JIELI", &format!("query {} failed: {}", cmd, e));
        }
        let deadline = time::sleep(Duration::from_millis(800));
        tokio::pin!(deadline);
        loop {
            tokio::select! {
                event = rx.recv() => {
                    if let Ok(RcspEvent::Packet(p)) = event {
                        if p.param.len() >= 2 && p.param[0] == cmd {
                            let on = p.param[1] == 2;
                            self.state.lock().await.insert(cmd, p.param.clone());
                            return Ok(Some(on));
                        }
                    }
                }
                _ = &mut deadline => break,
            }
        }
        Ok(self
            .state
            .lock()
            .await
            .get(&cmd)
            .and_then(|p| p.get(1))
            .map(|b| *b == 2))
    }

    /// Read the last reported parameter bytes for an Oraimo command id.
    pub async fn cached_param(&self, command_id: u8) -> Option<Vec<u8>> {
        self.state.lock().await.get(&command_id).cloned()
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
