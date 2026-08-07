use clap::{Parser, Subcommand};
use anyhow::{Context, Result};
use std::path::PathBuf;
use tokio::signal;

#[cfg(unix)]
const DEFAULT_SOCKET: &str = "/tmp/spacepods.sock";
#[cfg(windows)]
const DEFAULT_SOCKET: &str = r"\\.\pipe\spacepods";

#[derive(Parser)]
#[command(
    author,
    about = "SpacePods - Control your SpaceBuds",
    long_about = None,
    version = libspacepods::VERSION,
)]
#[command(propagate_version = true)]
struct Cli {
    /// Optional socket path for IPC
    #[arg(short, long, value_name = "SOCKET")]
    socket: Option<PathBuf>,

    /// Log level: info, warn, full (default: info)
    #[arg(long, value_name = "LEVEL", default_value = "info")]
    log_level: String,

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
        mode: String,
    },

    /// Set ANC/transparency level
    Level { level: u8 },

    /// Set EQ preset
    Eq { preset: u8 },

    /// Set adaptive ANC
    Adaptive {
        #[arg(value_parser = ["on", "off"])]
        state: String,
    },

    /// Set dual device mode
    Dual {
        #[arg(value_parser = ["on", "off"])]
        state: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Set log level before anything else
    libspacepods::log::set_log_level(libspacepods::log::LogLevel::from_str(&cli.log_level));
    libspacepods::log::info("DAEMON", &format!("Starting with log-level={}", cli.log_level));

    let socket_path = cli.socket.unwrap_or_else(|| PathBuf::from(DEFAULT_SOCKET));

    match cli.command {
        Commands::Service => {
            println!("Starting SpacePods service...");
            let mut service = libspacepods::ipc::SpacePodsService::new(Some(socket_path)).await;

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
            let cli = libspacepods::cli::InteractiveCli::new(Some(socket_path))
                .await
                .context("Failed to connect to SpacePods service. Is it running?")?;
            cli.run().await?;
            Ok(())
        }

        Commands::Exec { exec_cmd } => {
            let mut client = libspacepods::ipc::SpacePodsClient::connect(Some(socket_path))
                .await
                .context("Failed to connect to SpacePods service. Is it running?")?;

            match exec_cmd {
                ExecCommands::Status => {
                    let status = client.get_status().await?;
                    print_status(&status);
                }
                ExecCommands::Anc { mode } => {
                    client.set_anc_mode(&mode).await?;
                    println!("\u{2713} ANC mode set to: {}", mode);
                }
                ExecCommands::Level { level } => {
                    client.set_level(level).await?;
                    println!("\u{2713} Level set to: {}", level);
                }
                ExecCommands::Eq { preset } => {
                    client.set_eq_preset(preset).await?;
                    println!("\u{2713} EQ preset set to: {}", preset);
                }
                ExecCommands::Adaptive { state } => {
                    let enable = state == "on";
                    client.set_adaptive_anc(enable).await?;
                    println!("\u{2713} Adaptive ANC: {}", state.to_uppercase());
                }
                ExecCommands::Dual { state } => {
                    let enable = state == "on";
                    client.set_dual_device(enable).await?;
                    println!("\u{2713} Dual Device: {}", state.to_uppercase());
                }
            }

            Ok(())
        }
    }
}

fn print_status(status: &libspacepods::ipc::DeviceStatus) {
    println!("SpacePods Status:");
    println!("  Connected: {}", if status.connection.connected { "\u{2713}" } else { "\u{2717}" });
    if let Some(ref addr) = status.connection.address {
        println!("  Address: {}", addr);
    }
    println!("  ANC Mode: {}", status.anc.mode);
    println!("  Level: {}/{}", status.anc.level, status.anc.max_level);
    if let Some(ref eq) = status.eq {
        println!("  EQ: {} - {}", eq.name, eq.description);
    }
    println!("  Adaptive ANC: {}", match status.features.adaptive_anc {
        Some(true) => "ON",
        Some(false) => "OFF",
        None => "UNKNOWN",
    });
    println!("  Dual Device: {}", match status.features.dual_device {
        Some(true) => "ON",
        Some(false) => "OFF",
        None => "UNKNOWN",
    });
}
