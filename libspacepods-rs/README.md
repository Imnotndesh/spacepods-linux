# Libspacepods (Rust)

**Libspacepods-rs** is a Linux utility for controlling SpaceBuds/oraimo earbuds. It provides ANC control, EQ presets, battery monitoring, and more - all from your terminal.

## Features

- **ANC Control** - Toggle between Off, ANC, and Transparency modes
-  **Level Adjustment** - Fine-tune ANC/Transparency intensity (0-15)
- ️ **EQ Presets** - Bass Boost, Rock, Jazz, Vocal, Treble, Harman curve
-  **Battery Status** - Left/right earbud and charging case levels
-  **Dual Device** - Enable/disable multipoint connection
-  **Adaptive ANC** - Automatic noise cancellation adjustment
-  **Persistent Service** - Keeps connection alive for instant commands
-  **Interactive CLI** - Full-featured shell with history and tab completion

## Installation

### Debian/Ubuntu (.deb)
```bash
sudo dpkg -i spacepods-*.deb
```
### Fedora/RHEL (.rpm)
```bash
sudo rpm -ivh spacepods-*.rpm
```
### Binary tarball
```bash

tar xzf spacepods-linux-x86_64.tar.gz
sudo cp spacepods /usr/local/bin/
```
### From source
```bash

git clone https://github.com/Imnotndesh/libspacepods-rs.git
cd libspacepods
cargo build --release
sudo cp target/release/spacepods /usr/local/bin/
```
### Prerequisites
* Bluetooth adapter - Built-in or external USB
* BlueZ - Linux Bluetooth stack (sudo apt install bluez or sudo dnf install bluez)
* Systemd - For running as a user service (optional)

## Quick Start
```bash

# 1. Start the service (keeps connection alive)
spacepods service

# 2. In another terminal, launch the interactive CLI
spacepods cli

# 3. Or run one-shot commands
spacepods exec status
spacepods exec anc on
spacepods exec eq 1
```
## Documentation
For complete documentation, see [Docs.md](https://github.com/Imnotndesh/libspacepods-rs/Docs.md).
