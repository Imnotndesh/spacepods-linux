# SpacePods Documentation

## Table of Contents
1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Installation](#installation)
4. [Service Mode](#service-mode)
5. [CLI Mode](#cli-mode)
6. [Direct Mode](#direct-mode)
7. [Command Reference](#command-reference)
8. [EQ Presets](#eq-presets)
9. [Troubleshooting](#troubleshooting)
10. [FAQ](#faq)

## Overview

Libspacepods-rs is a Rust-based utility that provides low-level control over SpaceBuds/oraimo earbuds via Bluetooth. It reverse-engineers the proprietary protocol used by these devices and exposes a clean, user-friendly interface.

### Supported Devices
- SpaceBuds (all variants)
- oraimo earbuds with ANC
- Compatible devices using the same BLE service UUIDs (`ff17`, `fe2c`)

## Architecture

Libspacepods operates in three distinct modes:

```
┌─────────────────────────────────────────────────────────────┐
│                         SpacePods                           │
├─────────────────┬─────────────────┬─────────────────────────┤
│   Service Mode  │    CLI Mode     │      Direct Mode        │
│   (Daemon)      │  (Interactive)  │    (One-shot)          │
├─────────────────┼─────────────────┼─────────────────────────┤
│ • Persistent    │ • Full shell    │ • No service needed    │
│ • Holds BLE     │ • History       │ • Connect & execute    │
│   connection    │ • Tab complete  │ • Disconnect           │
│ • Caches state  │ • Live watch    │ • Good for scripts     │
│ • IPC server    │ • Colored UI    │ • Higher latency       │
└─────────────────┴─────────────────┴─────────────────────────┘
```

### Bluetooth Protocol
The device uses two GATT characteristics:
- **Write (ff17)** - Send commands
- **Notify (ff18)** - Receive responses

Commands are packets with format: `[seq, cmd, 0x01, 0x00, len, payload...]`

## Installation

### From Package Managers

**Debian/Ubuntu:**
```bash
sudo dpkg -i libspacepods-1.0.0.deb
# Enable and start the user service
systemctl --user enable libspacepods
systemctl --user start libspacepods
```

**Fedora/RHEL:**
```bash
sudo rpm -ivh libspacepods-1.0.0.rpm
systemctl --user enable libspacepods
systemctl --user start libspacepods
```

### From Binary Tarball
```bash
wget https://github.com/Imnotndesh/libspacepods-rs/releases/download/<VERSION>/libspacepods-linux-x86_64.tar.gz
tar xzf libspacepods-linux-x86_64.tar.gz
sudo cp libspacepods /usr/local/bin/
```

### Post-Installation
Verify installation:
```bash
libspacepods --version
```

## Service Mode

The service mode runs a persistent daemon that maintains the Bluetooth connection and serves IPC requests.

### Starting the Service

```bash
# Foreground (for testing)
libspacepods service

# Background (using systemd)
systemctl --user start libspacepods

# Auto-start on login
systemctl --user enable libspacepods

# Check status
systemctl --user status libspacepods
```

### Service Logs
```bash
journalctl --user -u libspacepods -f
```

### Service Behavior
- **Connection Management**: Automatically reconnects if device disconnects
- **State Caching**: Maintains current device state in memory
- **IPC Server**: Listens on `/tmp/spacepods.sock` for client commands
- **Broadcast**: Notifies all subscribed clients of state changes
- **Polling**: Updates device status every 15 seconds (battery only every 5 minutes)

### Stopping the Service
```bash
systemctl --user stop libspacepods
# Or if running in foreground: Ctrl+C
```

## CLI Mode

The interactive CLI connects to the running service and provides a full-featured shell.

### Starting the CLI
```bash
libspacepods cli
```

This requires the service to be running. The CLI will automatically attempt to connect to `/tmp/spacepods.sock`.

### CLI Features

**Command History** - Use up/down arrows to navigate previous commands
**Tab Completion** - Press Tab to complete commands and options
**Colored Output** - Status indicators and error messages with ANSI colors
**Live Watch** - Real-time status updates with screen clearing
**Persistent History** - Commands saved to `.spacepods_history`

### Example Session

```bash
$ libspacepods cli
libspacepods Interactive CLI
Type 'help' for commands, 'exit' to quit
Service connection: ✓

╔════════════════════════════════════╗
║         SpacePods Status          ║
╚════════════════════════════════════╝
  Connected    : ✓
  Address      : 46:02:49:68:39:8D
  Battery      : L:85%   R:82%  Case:95%
  ANC Mode     : ANC
  Level        : 10/15
  EQ            Bass Boost
  Adaptive ANC : OFF
  Dual Device  : OFF

spacepods> anc transparency
✓ ANC mode set to: transparency

spacepods> level 12
✓ Level set to: 12

spacepods> eq
Usage: eq <preset_id>

Standard Presets:
  1 : Bass Boost - Warm, punchy bass (Harman headphone curve inspired)
  2 : Rock - Energetic V-shape for guitars and drums
  3 : Jazz - Smooth mids, detailed cymbals
  4 : Vocal - Enhanced presence for vocals and speech
  5 : Treble Boost - Crisp highs for classical and acoustic
  6 : Custom - User-defined EQ curve

Special Presets:
  10: Harman AE/OE - Research-optimized consumer curve
  11: Cinema - Enhanced for movies and dialogue
  12: Podcast - Clear speech, reduced sibilance
  13: Night Listening - Reduced dynamics for quiet environments

spacepods> eq 10
✓ EQ preset set to: 10 (Harman AE/OE)

spacepods> watch
Watching for status updates... (Ctrl+C to stop)
╔════════════════════════════════════╗
║         SpacePods Status          ║
╚════════════════════════════════════╝
  Connected    : ✓
  Battery      :  L:85%   R:82%   Case:95%
  ANC Mode     :  ANC
  Level        : 12/15
  EQ           :  Harman AE/OE
  Adaptive ANC : OFF
  Dual Device  : OFF

^C
Stopped watching
spacepods> exit
Goodbye!
```

## Direct Mode

Direct mode bypasses the service and connects directly to the device. Each command establishes a new connection, executes, and disconnects.

### Usage
```bash
# Force direct mode
libspacepods --direct <command>

# Or if service isn't running, direct mode auto-fallback
libspacepods status
```

### Use Cases
- **Scripting**: Cron jobs, automation scripts
- **One-off commands**: Quick status checks without starting service
- **Troubleshooting**: Bypass service to isolate issues
- **Recovery**: When service is stuck or unresponsive

### Example Scripts

**Battery monitor:**
```bash
#!/bin/bash
while true; do
  spacepods --direct status | grep Battery
  sleep 300  # Check every 5 minutes
done
```

**Scheduled EQ change:**
```bash
#!/bin/bash
# Switch to Night Listening preset at 10 PM
if [ $(date +%H) -eq 22 ]; then
  spacepods --direct eq 13
fi
```

**Connection watchdog:**
```bash
#!/bin/bash
# Reconnect if device disconnects
while true; do
  if ! spacepods --direct status | grep -q "Connected    : ✓"; then
    echo "Device disconnected, reconnecting..."
    spacepods --direct anc off
  fi
  sleep 60
done
```

## Command Reference

### Global Options

| Option | Description |
|--------|-------------|
| `--service` | Run as persistent daemon |
| `--direct` | Force direct connection mode |
| `--help` | Show help information |
| `--version` | Show version information |

### Service Commands

| Command | Description |
|---------|-------------|
| `spacepods service` | Start the daemon |

### CLI/Exec Commands

#### `status`
Display current device status including:
- Connection state
- Bluetooth address
- Battery levels (L/R/Case)
- ANC mode
- ANC level
- Active EQ preset
- Adaptive ANC state
- Dual device state

**Example output:**
```
╔════════════════════════════════════╗
║         SpacePods Status          ║
╚════════════════════════════════════╝
  Connected    : ✓
  Address      : 46:02:49:68:39:8D
  Battery      :  L:85%   R:82%   Case:95%
  ANC Mode     :  ANC
  Level        : 10/15
  EQ           :  Bass Boost
  Adaptive ANC : OFF
  Dual Device  : OFF
```

#### `anc <mode>`
Set ANC mode.
- `on` or `anc` - Enable noise cancellation
- `off` - Disable ANC
- `transparency` - Enable transparency mode

**Examples:**
```bash
spacepods exec anc on
spacepods exec anc transparency
spacepods exec anc off
```

#### `level <0-15>`
Set intensity level for current ANC mode.
- Range depends on device (usually 0-15)
- Automatically clamps to device maximum
- Only works when ANC or Transparency is active

**Examples:**
```bash
spacepods exec level 10
spacepods exec level 15
```

#### `eq <preset_id>`
Set EQ preset.
- Without arguments: List all available presets
- With preset_id: Apply that preset

**Examples:**
```bash
spacepods exec eq          # List presets
spacepods exec eq 1        # Set Bass Boost
spacepods exec eq 10       # Set Harman curve
```

#### `adaptive <on|off>`
Enable/disable adaptive ANC.
- Adaptive ANC automatically adjusts based on environment
- Requires ANC mode to be active

**Examples:**
```bash
spacepods exec adaptive on
spacepods exec adaptive off
```

#### `dual <on|off>`
Enable/disable dual device (multipoint) mode.
- Allows connection to two devices simultaneously
- Automatically switches audio source

**Examples:**
```bash
spacepods exec dual on
spacepods exec dual off
```

#### `watch`
Enter live status monitoring mode.
- Updates every 5 seconds
- Press Ctrl+C to exit
- Only available in interactive CLI

**Example:**
```
spacepods> watch
Watching for status updates... (Ctrl+C to stop)
[Status updates displayed every 5 seconds]
^C
Stopped watching
```

#### `clear`
Clear the terminal screen.
- Only available in interactive CLI

#### `exit` or `quit`
Exit the interactive CLI.

## EQ Presets

### Frequency Bands
The EQ controls 7 frequency bands:
- 50 Hz (Sub-bass)
- 100 Hz (Bass)
- 400 Hz (Low mids)
- 1 kHz (Midrange)
- 2.5 kHz (Upper mids)
- 6.3 kHz (Presence)
- 16 kHz (Treble air)

### Standard Presets (0-6)

| ID | Name | Description | Curve (dB) |
|----|------|-------------|------------|
| 0 | Flat | Neutral, uncolored sound | [0, 0, 0, 0, 0, 0, 0] |
| 1 | Bass Boost | Warm, punchy bass (Harman curve inspired) | [6, 4, 1, 0, 0, 1, 2] |
| 2 | Rock | Energetic V-shape for guitars/drums | [4, 3, -1, -1, 2, 4, 5] |
| 3 | Jazz | Smooth mids, detailed cymbals | [2, 2, 1, 1, -1, 2, 4] |
| 4 | Vocal | Enhanced presence for vocals | [-2, -1, 0, 4, 3, 1, 1] |
| 5 | Treble Boost | Crisp highs for classical/acoustic | [-2, -1, 0, 1, 3, 5, 7] |
| 6 | Custom | User-defined curve | [0, 0, 0, 0, 0, 0, 0] |

### Special Presets (10-13)

| ID | Name | Description | Curve (dB) |
|----|------|-------------|------------|
| 10 | Harman AE/OE | Research-optimized consumer curve | [4, 3, 1, 0, -1, 1, 3] |
| 11 | Cinema | Enhanced for movies and dialogue | [2, 2, 0, 3, 2, 0, 2] |
| 12 | Podcast | Clear speech, reduced sibilance | [-1, 0, 2, 5, 2, -1, -2] |
| 13 | Night Listening | Reduced dynamics for quiet environments | [-3, -2, -1, 0, 0, -1, -2] |

### Preset Characteristics

**Bass Boost (1)**
- Sub-bass: +6 dB
- Bass: +4 dB
- Low mids: +1 dB
- Flat mids, slight treble lift
- Best for: EDM, Hip-hop, Pop

**Rock (2)**
- V-shaped curve
- Boosted bass and treble
- Slightly scooped mids
- Best for: Rock, Metal, Alternative

**Jazz (3)**
- Warm low end
- Neutral mids
- Detailed but non-fatiguing highs
- Best for: Jazz, Classical, Acoustic

**Vocal (4)**
- Slightly reduced bass
- +4 dB presence boost at 1kHz
- Clear, forward vocals
- Best for: Podcasts, Audiobooks, Vocals

**Treble Boost (5)**
- Flat bass
- +5 dB at 6.3kHz
- +7 dB at 16kHz
- Best for: Classical, Acoustic, Detail retrieval

**Harman AE/OE (10)**
- Research-optimized consumer curve
- Gradual bass rise
- Slight treble presence
- Best for: All-purpose, Reference listening

### Custom EQ
Set a custom 10-band EQ curve:
```bash
# Currently only available via API
# CLI support coming in future release
```

## Troubleshooting

### Common Issues

#### "Failed to connect to service"
**Cause**: Service not running or socket not accessible  
**Solutions**:
```bash
# Check if service is running
systemctl --user status spacepods

# Start the service
systemctl --user start spacepods

# Or run in foreground to see errors
spacepods service
```

#### "Device not found"
**Cause**: Earbuds not in pairing mode, Bluetooth off, or incompatible device  
**Solutions**:
1. Put earbuds in pairing mode (usually hold touch button for 5 seconds until LED flashes)
2. Enable Bluetooth: `sudo systemctl start bluetooth`
3. Verify device is discoverable: `bluetoothctl scan on`
4. Check compatibility: Device should advertise UUID containing "ff17" or "fe2c"
5. Try direct mode: `spacepods --direct status`

#### "Write characteristic not found"
**Cause**: Connected to wrong device or service discovery failed  
**Solutions**:
1. Disconnect other Bluetooth devices
2. Restart service: `systemctl --user restart spacepods`
3. Force rediscovery: `spacepods --direct status`
4. Reset Bluetooth: `sudo systemctl restart bluetooth`

#### "Permission denied" on socket
**Cause**: Socket permissions incorrect or multiple users  
**Solutions**:
```bash
# Remove and let service recreate socket
rm -f /tmp/spacepods.sock
systemctl --user restart spacepods

# Or manually set permissions
sudo chmod 666 /tmp/spacepods.sock
```

#### EQ preset not updating in status
**Cause**: Status cache not refreshing  
**Solutions**:
```bash
# Force a status refresh
spacepods exec status

# Or wait for next polling cycle (15 seconds)
# Or restart the service
systemctl --user restart spacepods
```

#### Service spamming logs
**Cause**: Debug logging enabled or frequent polling  
**Solutions**:
```bash
# Disable debug logging
unset RUST_LOG
systemctl --user restart spacepods

# The service only logs EQ changes by default
# Normal operation is quiet
```

#### Bluetooth connection drops frequently
**Cause**: Power saving, interference, or device firmware  
**Solutions**:
1. Disable Bluetooth power saving:
   ```bash
   sudo sed -i 's/#IdleTimeout=.*/IdleTimeout=0/' /etc/bluetooth/btmon.conf
   sudo systemctl restart bluetooth
   ```
2. Move closer to device
3. Remove other Bluetooth devices
4. Reset earbuds (place in case, hold button 10s)

### Debug Mode

Enable detailed logging:
```bash
# Set debug flag in service
export RUST_LOG=debug
spacepods service

# Or in systemd service
mkdir -p ~/.config/systemd/user
cat > ~/.config/systemd/user/spacepods.service.d/debug.conf << EOF
[Service]
Environment=RUST_LOG=debug
EOF
systemctl --user daemon-reload
systemctl --user restart spacepods
```

### Reset Everything

Complete reset procedure:
```bash
#!/bin/bash
# Stop service
systemctl --user stop spacepods

# Remove socket and history
rm -f /tmp/spacepods.sock
rm -f ~/.spacepods_history

# Restart Bluetooth
sudo systemctl restart bluetooth
sleep 2

# Restart service
systemctl --user start spacepods

# Check status
spacepods exec status
```

## FAQ

**Q: Does this work with AirPods?**  
A: Shurely......No. AirPods use a different, encrypted protocol. This only works with SpaceBuds/oraimo earbuds and compatible clones.

**Q: Will this drain my earbuds battery?**  
A: The service maintains a BLE connection which uses minimal power (similar to keeping them connected to your phone). The status polling interval is 15 seconds, which is conservative. Battery drain is negligible.

**Q: Can I use this while listening to music?**  
A: Yes. BLE commands don't interfere with the audio A2DP connection. You can change EQ or ANC mode while music is playing.

**Q: Why do I need a service? Can't I just connect directly?**  
A: You can use direct mode with `--direct`. The service provides:
- Instant commands (no 2-3 second connection delay)
- Maintains state across multiple CLI sessions
- Allows multiple clients to control the device
- Provides live status updates
- Better battery life (reconnects less frequently)

**Q: How do I make the service start automatically?**  
A: 
```bash
systemctl --user enable spacepods
systemctl --user start spacepods
```

**Q: The service shows "Connected" but commands don't work**  
A: The device may be in a bad state. Try:
```bash
spacepods exec anc off
spacepods exec anc on
# Or disconnect and reconnect
spacepods --direct status
```

**Q: Can I control multiple pairs of earbuds?**  
A: Currently, one service instance controls one pair. You can run multiple service instances on different sockets:
```bash
# First pair (default socket)
libspacepods service

# Second pair (custom socket)
spacepods --socket /tmp/spacepods2.sock service
```

**Q: How do I contribute?**  
A: Fork the repository, make your changes, and submit a pull request. Please ensure:
- Code follows Rustfmt conventions
- Tests pass
- Documentation is updated
- No debug logging in production code

**Q: Where are log files stored?**  
A: When running as a systemd user service:
```bash
journalctl --user -u spacepods -f
```

When running in foreground: logs go to stdout/stderr.

**Q: Can I use this in my own Rust projects?**  
A: Yes! Add to your Cargo.toml:
```toml
[dependencies]
spacepods = { git = "https://github.com/YOUR_USERNAME/libspacepods" }
```

Then use the library:
```rust
use spacepods::SpaceBuds;

#[tokio::main]
async fn main() -> Result<()> {
    let buds = SpaceBuds::new().await?;
    buds.anc().set_anc().await?;
    Ok(())
}
```

**Q: Why is battery status sometimes unavailable?**  
A: Battery reading depends on the device reporting it via BLE. Some devices:
- Only report battery when in case
- Report both buds as a single value
- Use proprietary manufacturer data fields
- Don't support battery reporting at all

**Q: How do I report a bug?**  
A: [Open an issue](https://github.com/YOUR_USERNAME/libspacepods/issues) with:
- SpacePods version (`libspacepods --version`)
- Linux distribution and version
- Bluetooth adapter model
- Earbuds model
- Steps to reproduce
- Debug logs (if possible)

**Q: Will this work on other operating systems?**  
A: Currently only Linux is supported. Windows and macOS support are planned for future releases.

---

**Need additional help?** [Open an issue](https://github.com/YOUR_USERNAME/libspacepods/issues) or check existing issues for solutions.

*Last updated: 2026*