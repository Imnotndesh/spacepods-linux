# experimental-handoff — PoC Plan

## What we built

### 1. DeviceBeacon parser (`libspacepods-rs/src/beacon/mod.rs`)

A pure Rust port of the Oraimo Sound APK's `DeviceBeacon` system. Parses manufacturer-specific
BLE advertisement data (AD type 0xFF) to identify Oraimo/SpaceBuds devices.

Supported beacon formats:
- **V1** (13 bytes): Includes battery levels (left/right/case) and charging status
- **V2** (13 bytes): Includes brand ID, MAC address XOR'd with 0xAD
- **V3** (8 bytes):  Minimal — just product ID + XOR'd MAC
- **V4** (27/13/9 bytes): Extended format, MAC XOR'd

Each beacon extracts:
- Product ID (maps to device profile in `device_profile.rs` on the `experimental-rewrite` branch)
- Real Bluetooth MAC address (de-XOR'd with `0xAD`)
- Connection state, auth flag, SPP UUID flag
- Battery data (V1 only)

Tests pass for V1, V3, and invalid data rejection.

### 2. Event-driven scanner (`libspacepods-rs/src/beacon/scanner.rs`)

Uses `btleplug`'s `adapter.events()` stream (instead of sleep-then-poll) to:
- Apply `ScanFilter` with service UUIDs (`0xFF17` + `0xFE2C`) at the hardware level
- Listen for `CentralEvent::ManufacturerDataAdvertisement` events
- Post-filter using `DeviceBeacon::from_manufacturer_data()`
- Enrich results with peripheral properties (name, RSSI)

### 3. Standalone test binary (`libspacepods-rs/src/bin/spacepods-scan.rs`)

A self-contained binary for testing:
```bash
cargo run --bin spacepods-scan
```

Scans for 30 seconds, prints every SpaceBuds device found with full beacon info.
Also does a post-scan poll (old approach) for comparison.

## What's next (to finish the handoff flow)

### Phase 2: Connection Handoff
The handoff flow: scan → connect → discover services → subscribe → handshake → ready

Current problems:
- `connect()` has no timeout
- `discover_services()` called on every connect (no caching)
- Notification stream spawned per `BleConnection::new()` but orphaned on reconnect
- Handshake is fire-and-forget

Plan:
1. Add `connect_with_timeout()` wrapper
2. Cache GATT characteristics after first discovery
3. Tie notification stream lifecycle to connection (cancel on disconnect)
4. Add `Handshaking` state — wait for valid TLV response after CMD_DEVICE_INFO
5. State machine: `Disconnected → Scanning → Connecting → Handshaking → Connected`

### Phase 3: Update ConnectionManager
- Add `connect_by_address(real_mac)` using the beacon's de-obfuscated MAC
- Add exponential backoff reconnection
- Add `Disconnecting` state to prevent double-disconnect races

### Phase 4: Update IPC Service
- Surface beacon info (product_id, real MAC, battery from V1 beacons)
- Add `scan_beacons` IPC command that returns parsed beacon data

### Phase 5: UI Updates
- Show beacon-derived info in setup page (product name, battery)
- Filter scan results to only show SpaceBuds devices
- Show "connecting to <product_name>" instead of raw MAC

## Key design decisions

1. **Manufacturer data is the authoritative filter.** Service UUID matching is a pre-filter on
   hardware; the beacon parser does the real identification. Only genuine Oraimo/Bluetrum devices
   produce this beacon format.

2. **Event-driven scanning.** The `adapter.events()` API gives real-time `ManufacturerDataAdvertisement`
   events, avoiding the long sleep-and-pray approach.

3. **Beacon ported from APK.** The V1-V4 beacon parsing logic is directly ported from the decompiled
   `DeviceBeacon*.java` files, including the XOR de-obfuscation with `0xAD`.

4. **BlueZ caveat.** The btleplug docs warn that `ScanFilter` on Linux is system-wide merged,
   so post-filtering is mandatory. Our DeviceBeacon parser handles this.

## Testing

```bash
# Run beacon unit tests
cargo test -p libspacepods -- beacon

# Run the scan PoC (requires Bluetooth adapter + SpaceBuds in pairing mode)
cargo run --bin spacepods-scan
```
