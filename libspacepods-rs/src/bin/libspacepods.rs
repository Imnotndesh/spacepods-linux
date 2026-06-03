use clap::{Parser, Subcommand};
use anyhow::{Context, Result};
use std::path::PathBuf;
use tokio::signal;

#[cfg(unix)]
const DEFAULT_SOCKET: &str = "/tmp/spacepods.sock";
#[cfg(windows)]
const DEFAULT_SOCKET: &str = r"\\.\pipe\spacepods";
use dialoguer::{theme::ColorfulTheme, Select};
use std::time::Duration;
use anyhow::{anyhow};
use btleplug::api::Peripheral;
use libspacepods::{DeviceScanner, SpaceBuds};
use libspacepods::service::SpacePodsService;

#[derive(Parser)]
#[command(author, version, about = "SpacePods - Control your SpaceBuds", long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    /// Optional socket path for IPC
    #[arg(short, long, value_name = "SOCKET")]
    socket: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run as background service (daemon)
    Service,

    /// Start interactive CLI (requires service)
    Cli,

    Exec {
        #[command(subcommand)]
        exec_cmd: ExecCommands,
    },
}

#[derive(Subcommand)]
enum ExecCommands {
    /// Get device status
    Status,

    /// Set ANC mode
    Anc {
        #[arg(value_parser = ["on", "off", "transparency"])]
        mode: String
    },

    /// Set ANC/transparency level
    Level { level: u8 },

    /// Set EQ preset
    Eq { preset: u8 },

    /// Set adaptive ANC
    Adaptive {
        #[arg(value_parser = ["on", "off"])]
        state: String
    },

    /// Set dual device mode
    Dual {
        #[arg(value_parser = ["on", "off"])]
        state: String
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let socket_path = cli.socket.unwrap_or_else(|| PathBuf::from(DEFAULT_SOCKET));

    match cli.command {
       Commands::Service => {
            println!("Starting SpacePods service...");
            let mut service = libspacepods::service::SpacePodsService::new(Some(socket_path)).await;

            tokio::select! {
                _ = service.run() => {},
                _ = signal::ctrl_c() => {
                    println!("\nShutting down...");
                    service.stop().await;
                }
            }

            Ok(())
        }

        Commands::Cli => {
            // Interactive CLI - requires service
            let cli = libspacepods::cli::InteractiveCli::new(Some(socket_path)).await
                .context("Failed to connect to SpacePods service. Is it running?")?;
            cli.run().await?;
            Ok(())
        }

        Commands::Exec { exec_cmd } => {
            // Single command - requires service
            let mut client = libspacepods::client::SpacePodsClient::connect(Some(socket_path)).await
                .context("Failed to connect to SpacePods service. Is it running?")?;

            match exec_cmd {
                ExecCommands::Status => {
                    let status = client.get_status().await?;
                    print_status(&status);
                }
                ExecCommands::Anc { mode } => {
                    client.set_anc_mode(&mode).await?;
                    println!("✓ ANC mode set to: {}", mode);
                }
                ExecCommands::Level { level } => {
                    client.set_level(level).await?;
                    println!("✓ Level set to: {}", level);
                }
                ExecCommands::Eq { preset } => {
                    client.set_eq_preset(preset).await?;
                    println!("✓ EQ preset set to: {}", preset);
                }
                ExecCommands::Adaptive { state } => {
                    let enable = state == "on";
                    client.set_adaptive_anc(enable).await?;
                    println!("✓ Adaptive ANC: {}", state.to_uppercase());
                }
                ExecCommands::Dual { state } => {
                    let enable = state == "on";
                    client.set_dual_device(enable).await?;
                    println!("✓ Dual Device: {}", state.to_uppercase());
                }
            }

            Ok(())
        }
    }
}
pub async fn select_and_connect_buds() -> anyhow::Result<SpaceBuds> {
    println!("\x1b[1;36mScanning for nearby SpacePods (Service filters: FF17/FE2C)...\x1b[0m");

    let peripherals = DeviceScanner::scan_devices(Duration::from_secs(3)).await
        .context("Failed while scanning for BLE peripherals")?;

    if peripherals.is_empty() {
        return Err(anyhow!("No compatible SpacePods found in the area. Make sure they are powered and in pairing mode."));
    }

    let mut device_options = Vec::new();
    let mut item_labels = Vec::new();

    for peripheral in peripherals {
        if let Ok(Some(props)) = peripheral.properties().await {
            let name = props.local_name.unwrap_or_else(|| "Unknown SpaceBuds".to_string());
            let address = peripheral.address().to_string();
            item_labels.push(format!("{} [{}]", name, address));
            device_options.push(address);
        }
    }

    if item_labels.is_empty() {
        return Err(anyhow!("Found BLE devices, but could not read valid properties/addresses."));
    }

    println!("\x1b[1;33mMultiple space devices detected in range! Please pick yours:\x1b[0m");
    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select target device to tie background daemon service:")
        .items(&item_labels)
        .default(0)
        .interact_opt()?;

    match selection {
        Some(index) => {
            let chosen_address = &device_options[index];
            println!("\x1b[1;32mTarget locked to MAC: {}. Instantiating safe gateway...\x1b[0m", chosen_address);

            let buds = SpaceBuds::with_address(Some(chosen_address.clone())).await?;
            Ok(buds)
        }
        None => Err(anyhow!("Selection cancelled by execution environment.")),
    }
}
fn print_status(status: &libspacepods::service::DeviceStatus) {
    println!("SpacePods Status:");
    println!("  Connected: {}", if status.connected { "✓" } else { "✗" });
    if let Some(addr) = &status.address {
        println!("  Address: {}", addr);
    }
    println!("  ANC Mode: {}", match status.anc_mode {
        Some(0) => "OFF",
        Some(1) => "ANC",
        Some(2) => "TRANSPARENCY",
        _ => "UNKNOWN",
    });
    println!("  Level: {}/{}", status.anc_level, status.anc_max);
    if let Some(name) = &status.eq_name {
        println!("  EQ: {}", name);
    }
    println!("  Adaptive ANC: {}", match status.adaptive_anc {
        Some(true) => "ON",
        Some(false) => "OFF",
        None => "UNKNOWN",
    });
    println!("  Dual Device: {}", match status.dual_device {
        Some(true) => "ON",
        Some(false) => "OFF",
        None => "UNKNOWN",
    });
}