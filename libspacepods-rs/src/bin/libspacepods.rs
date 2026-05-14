use clap::{Parser, Subcommand};
use anyhow::{Context, Result};
use std::path::PathBuf;
use tokio::signal;

#[cfg(unix)]
const DEFAULT_SOCKET: &str = "/tmp/spacepods.sock";
#[cfg(windows)]
const DEFAULT_SOCKET: &str = r"\\.\pipe\spacepods";

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

    /// Execute a single command (requires service)
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
            // Run as service
            println!("Starting SpacePods service...");
            let mut service = libspacepods::service::SpacePodsService::new(Some(socket_path)).await?;

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