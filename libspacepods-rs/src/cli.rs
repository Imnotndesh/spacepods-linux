use crate::ipc::client::SpacePodsClient;
use crate::ipc::protocol::DeviceStatus;
use crate::commands::eq::EqPreset;
use anyhow::{Context, Result};
use rustyline::config::{Config, EditMode};
use rustyline::error::ReadlineError;
use rustyline::highlight::{Highlighter, MatchingBracketHighlighter};
use rustyline::hint::HistoryHinter;
use rustyline::validate::MatchingBracketValidator;
use rustyline::{Cmd, CompletionType, Editor, EventHandler, KeyEvent};
use rustyline::history::FileHistory;
use std::borrow::Cow::{self, Borrowed, Owned};
use std::path::PathBuf;
use std::sync::Arc;
use rustyline_derive::{Completer, Helper, Hinter, Validator};
use tokio::sync::Mutex;

#[derive(Completer, Helper, Hinter, Validator)]
struct CliHelper {
    #[rustyline(Completer)]
    completer: (),
    #[rustyline(Hinter)]
    hinter: HistoryHinter,
    #[rustyline(Validator)]
    validator: MatchingBracketValidator,
    highlighter: MatchingBracketHighlighter,
    colored_prompt: String,
}

impl Highlighter for CliHelper {
    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
        &'s self,
        prompt: &'p str,
        default: bool,
    ) -> Cow<'b, str> {
        if default {
            Borrowed(&self.colored_prompt)
        } else {
            Borrowed(prompt)
        }
    }

    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        Owned("\x1b[1m".to_string() + hint + "\x1b[m")
    }

    fn highlight<'l>(&self, line: &'l str, pos: usize) -> Cow<'l, str> {
        self.highlighter.highlight(line, pos)
    }

    fn highlight_char(&self, line: &str, pos: usize, kind: rustyline::highlight::CmdKind) -> bool {
        self.highlighter.highlight_char(line, pos, kind)
    }
}

pub struct InteractiveCli {
    client: Arc<Mutex<SpacePodsClient>>,
    status: Arc<tokio::sync::RwLock<DeviceStatus>>,
}

impl InteractiveCli {
    pub async fn new(socket_path: Option<PathBuf>) -> Result<Self> {
        let client = SpacePodsClient::connect(socket_path)
            .await
            .context("Failed to connect to SpacePods service. Is it running?")?;

        let client = Arc::new(Mutex::new(client));

        // Get initial status — service may be disconnected, that's fine
        let status = {
            let mut client_lock = client.lock().await;
            client_lock.get_status().await.unwrap_or(DeviceStatus::default_disconnected())
        };

        Ok(Self {
            client,
            status: Arc::new(tokio::sync::RwLock::new(status)),
        })
    }

    pub async fn run(&self) -> Result<()> {
        let config = Config::builder()
            .history_ignore_space(true)
            .completion_type(CompletionType::List)
            .edit_mode(EditMode::Emacs)
            .build();

        let helper = CliHelper {
            completer: (),
            hinter: HistoryHinter {},
            validator: MatchingBracketValidator::new(),
            highlighter: MatchingBracketHighlighter::new(),
            colored_prompt: "".to_owned(),
        };

        let mut rl = Editor::<_, FileHistory>::with_config(config)?;
        rl.set_helper(Some(helper));
        rl.bind_sequence(KeyEvent::alt('n'), EventHandler::Simple(Cmd::HistorySearchForward));
        rl.bind_sequence(KeyEvent::alt('p'), EventHandler::Simple(Cmd::HistorySearchBackward));

        if rl.load_history(".spacepods_history").is_err() {
            println!("No command history found");
        }

        println!("\x1b[1;32mSpacePods Interactive CLI\x1b[0m");
        println!("Type 'help' for commands, 'exit' to quit");

        match self.check_connection().await {
            Ok(true) => {
                println!("Service connection: \x1b[1;32m\u{2713}\x1b[0m");
                self.refresh_status().await.unwrap_or(());
                self.print_status().await;
            }
            _ => {
                println!("Service connection: \x1b[1;31m\u{2717}\x1b[0m");
                println!("Cannot connect to SpacePods service. Is it running?");
                return Ok(());
            }
        }

        loop {
            let prompt = format!("\x1b[1;34mspacepods>\x1b[0m ");
            if let Some(helper) = rl.helper_mut() {
                helper.colored_prompt = prompt.clone();
            }

            let readline = rl.readline(&prompt);

            match readline {
                Ok(line) => {
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }

                    rl.add_history_entry(line.to_string()).expect("Unexpected");

                    match self.handle_command(line).await {
                        Ok(should_exit) => {
                            if should_exit {
                                break;
                            }
                        }
                        Err(e) => {
                            println!("\x1b[1;31mError: {}\x1b[0m", e);
                        }
                    }
                }
                Err(ReadlineError::Interrupted) => {
                    println!("^C");
                    break;
                }
                Err(ReadlineError::Eof) => {
                    break;
                }
                Err(err) => {
                    println!("Error: {:?}", err);
                    break;
                }
            }
        }

        rl.save_history(".spacepods_history")?;
        println!("Goodbye!");
        Ok(())
    }

    async fn check_connection(&self) -> Result<bool> {
        let mut client = self.client.lock().await;
        match client.ping().await {
            Ok(true) => Ok(true),
            _ => Ok(false),
        }
    }

    async fn handle_command(&self, line: &str) -> Result<bool> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            return Ok(false);
        }

        match parts[0] {
            "exit" | "quit" => return Ok(true),

            "help" => {
                self.print_help();
            }

            "status" => {
                self.refresh_status().await?;
                self.print_status().await;
            }

            "watch" => {
                self.watch_status().await?;
            }

            // ── Scan for devices ──
            "scan" => {
                let timeout: u64 = if parts.len() > 1 {
                    parts[1].parse().unwrap_or(10)
                } else {
                    10
                };
                let mut client = self.client.lock().await;
                println!("Scanning for SpaceBuds devices ({}s)...", timeout);
                match client.scan(timeout).await {
                    Ok(devices) => {
                        if devices.is_empty() {
                            println!("No SpaceBuds devices found.");
                        } else {
                            println!("\nFound {} device(s):", devices.len());
                            for (i, d) in devices.iter().enumerate() {
                                println!("  {}. {}  ({})", i + 1, d.name, d.address);
                            }
                            println!("\nUse 'connect <address>' to connect to a device.");
                        }
                    }
                    Err(e) => {
                        println!("Scan failed: {}", e);
                    }
                }
            }

            // ── Connect to a device ──
            "connect" => {
                if parts.len() < 2 {
                    println!("Usage: connect <address>");
                    println!("       Use 'scan' first to find devices and their addresses.");
                    return Ok(false);
                }
                let address = parts[1..].join(" ");
                let mut client = self.client.lock().await;
                println!("Connecting to {}...", address);
                match client.connect_device(address).await {
                    Ok(_) => {
                        println!("\x1b[1;32m\u{2713}\x1b[0m Connected!");
                        // Give the service a moment to populate status
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                        drop(client);
                        self.refresh_status().await?;
                        self.print_status().await;
                    }
                    Err(e) => {
                        println!("\x1b[1;31mFailed to connect: {}\x1b[0m", e);
                    }
                }
            }

            // ── Disconnect ──
            "disconnect" => {
                // The service doesn't have a disconnect command directly;
                // we can just refresh to show disconnected state.
                self.refresh_status().await?;
                self.print_status().await;
            }

            // ── Feature commands (require service connection to BLE) ──
            "anc" => {
                if parts.len() < 2 {
                    println!("Usage: anc <on|off|transparency>");
                    return Ok(false);
                }
                let mode = parts[1];
                let mut client = self.client.lock().await;
                client.set_anc_mode(mode).await?;
                drop(client);
                println!("\x1b[1;32m\u{2713}\x1b[0m ANC mode set to: {}", mode);
                self.refresh_status().await?;
            }

            "level" => {
                if parts.len() < 2 {
                    println!("Usage: level <0-15>");
                    return Ok(false);
                }
                let level: u8 = parts[1].parse()?;
                let mut client = self.client.lock().await;
                client.set_level(level).await?;
                drop(client);
                println!("\x1b[1;32m\u{2713}\x1b[0m Level set to: {}", level);
                self.refresh_status().await?;
            }

            "eq" => {
                if parts.len() < 2 {
                    println!("Usage: eq <preset_id>");
                    println!("\nStandard Presets:");
                    for p in crate::commands::eq::EQ_PRESETS {
                        println!("  \x1b[1;33m{:<2}\x1b[0m: {} - {}", p.id, p.name, p.description);
                    }
                    println!("\n\x1b[1;36mSpecial Presets:\x1b[0m");
                    for p in crate::commands::eq::SPECIAL_PRESETS {
                        println!("  \x1b[1;33m{:<2}\x1b[0m: {} - {}", p.id, p.name, p.description);
                    }
                    println!("\nExample: eq 1");
                    return Ok(false);
                }

                let preset: u8 = parts[1].parse()?;
                let mut client = self.client.lock().await;
                client.set_eq_preset(preset).await?;
                drop(client);

                self.refresh_status().await?;

                let status = self.status.read().await;
                let preset_name = status.eq.as_ref().map(|e| e.name.as_str()).unwrap_or("Unknown");
                println!("\x1b[1;32m\u{2713}\x1b[0m EQ preset set to: {} ({})", preset, preset_name);
            }

            "adaptive" => {
                if parts.len() < 2 {
                    println!("Usage: adaptive <on|off>");
                    return Ok(false);
                }
                let enable = parts[1] == "on";
                let mut client = self.client.lock().await;
                client.set_adaptive_anc(enable).await?;
                drop(client);
                println!("\x1b[1;32m\u{2713}\x1b[0m Adaptive ANC: {}", if enable { "ON" } else { "OFF" });
                self.refresh_status().await?;
            }

            "dual" => {
                if parts.len() < 2 {
                    println!("Usage: dual <on|off>");
                    return Ok(false);
                }
                let enable = parts[1] == "on";
                let mut client = self.client.lock().await;
                client.set_dual_device(enable).await?;
                drop(client);
                println!("\x1b[1;32m\u{2713}\x1b[0m Dual Device: {}", if enable { "ON" } else { "OFF" });
                self.refresh_status().await?;
            }

            "chat" => {
                if parts.len() < 2 {
                    println!("Usage: chat <on|off>");
                    return Ok(false);
                }
                let enable = parts[1] == "on";
                let mut client = self.client.lock().await;
                client.send_command_raw(crate::ipc::ServiceCommand::Custom {
                    command_id: 0x35,
                    payload: vec![if enable { 0x01 } else { 0x00 }],
                }).await?;
                println!("\x1b[1;32m\u{2713}\x1b[0m Chat Mode: {}", if enable { "ON" } else { "OFF" });
            }

            "endurance" => {
                if parts.len() < 2 {
                    println!("Usage: endurance <on|off>");
                    return Ok(false);
                }
                let enable = parts[1] == "on";
                let mut client = self.client.lock().await;
                client.send_command_raw(crate::ipc::ServiceCommand::Custom {
                    command_id: 0x38,
                    payload: vec![if enable { 0x01 } else { 0x00 }],
                }).await?;
                println!("\x1b[1;32m\u{2713}\x1b[0m Long Endurance: {}", if enable { "ON" } else { "OFF" });
            }

            "rename" => {
                if parts.len() < 2 {
                    println!("Usage: rename <new-name>");
                    return Ok(false);
                }
                let name = parts[1..].join(" ");
                let mut client = self.client.lock().await;
                client.send_command_raw(crate::ipc::ServiceCommand::Custom {
                    command_id: 0x2D,
                    payload: name.bytes().collect(),
                }).await?;
                println!("\x1b[1;32m\u{2713}\x1b[0m Device renamed to: {}", name);
            }

            "clearpair" => {
                let mut client = self.client.lock().await;
                client.send_command_raw(crate::ipc::ServiceCommand::Custom {
                    command_id: 0x2F,
                    payload: vec![],
                }).await?;
                println!("\x1b[1;32m\u{2713}\x1b[0m Pairing records cleared. Device will restart.");
            }

            "gamemode" => {
                if parts.len() < 2 {
                    println!("Usage: gamemode <on|off>");
                    return Ok(false);
                }
                let enable = parts[1] == "on";
                let mut client = self.client.lock().await;
                client.send_command_raw(crate::ipc::ServiceCommand::Custom {
                    command_id: 0x25,
                    payload: vec![if enable { 0x01 } else { 0x00 }],
                }).await?;
                println!("\x1b[1;32m\u{2713}\x1b[0m Game Mode: {}", if enable { "ON" } else { "OFF" });
            }

            "find" => {
                let enable = parts.get(1).map_or(true, |s| *s == "on");
                let mut client = self.client.lock().await;
                client.send_command_raw(crate::ipc::ServiceCommand::Custom {
                    command_id: 0x2A,
                    payload: vec![if enable { 0x01 } else { 0x00 }],
                }).await?;
                if enable {
                    println!("\x1b[1;32m\u{2713}\x1b[0m Ringing earbuds! Use 'find off' to stop.");
                } else {
                    println!("\x1b[1;32m\u{2713}\x1b[0m Stopped ringing.");
                }
            }

            "clear" => {
                print!("\x1b[2J\x1b[1;1H");
                std::io::Write::flush(&mut std::io::stdout())?;
            }

            _ => {
                println!("Unknown command: '{}'", parts[0]);
                println!("Type 'help' for available commands");
            }
        }

        Ok(false)
    }

    async fn refresh_status(&self) -> Result<()> {
        let status = {
            let mut client = self.client.lock().await;
            client.get_status().await?
        };

        let mut status_lock = self.status.write().await;
        *status_lock = status;
        Ok(())
    }

    async fn print_status(&self) {
        let status = self.status.read().await;

        println!("\x1b[1;36m\u{2554}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2557}\x1b[0m");
        println!("\x1b[1;36m\u{2551}         SpacePods Status          \u{2551}\x1b[0m");
        println!("\x1b[1;36m\u{255a}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{255d}\x1b[0m");

        println!("  Connected    : {}", if status.connection.connected {
            "\x1b[1;32m\u{2713}\x1b[0m"
        } else {
            "\x1b[1;31m\u{2717}\x1b[0m Use 'scan' then 'connect <address>'"
        });

        if let Some(ref addr) = status.connection.address {
            println!("  Address      : {}", addr);
        }

        // Battery
        let b = &status.battery;
        match (b.left, b.right, b.case) {
            (Some(l), Some(r), Some(c)) => {
                println!("  Battery      : L:{}%  R:{}%  Case:{}%", l, r, c);
            }
            (Some(l), Some(r), None) => {
                println!("  Battery      : L:{}%  R:{}%", l, r);
            }
            (Some(b), None, None) => {
                println!("  Battery      : {}%", b);
            }
            _ => {}
        }

        // ANC
        println!("  ANC Mode     : {}", status.anc.mode);
        println!("  Level        : {}/{}", status.anc.level, status.anc.max_level);

        // EQ
        if let Some(ref eq) = status.eq {
            println!("  EQ           : {} ({})", eq.name, eq.description);
        }

        // Features
        println!("  Adaptive ANC : {}", match status.features.adaptive_anc {
            Some(true) => "\x1b[1;32mON\x1b[0m",
            Some(false) => "\x1b[1;33mOFF\x1b[0m",
            None => "UNKNOWN",
        });

        println!("  Dual Device  : {}", match status.features.dual_device {
            Some(true) => "\x1b[1;32mON\x1b[0m",
            Some(false) => "\x1b[1;33mOFF\x1b[0m",
            None => "UNKNOWN",
        });

        println!();
    }

    async fn watch_status(&self) -> Result<()> {
        println!("Watching for status updates... (Ctrl+C to stop)");

        let mut rx = {
            let mut client = self.client.lock().await;
            client.subscribe().await?
        };

        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                println!("\nStopped watching");
            }
            result = async {
                while let Ok(status) = rx.recv().await {
                    let mut status_lock = self.status.write().await;
                    *status_lock = status;
                    drop(status_lock);

                    print!("\x1b[2J\x1b[1;1H");
                    self.print_status().await;
                }
                Ok::<_, anyhow::Error>(())
            } => {
                result?;
            }
        }

        Ok(())
    }

    fn print_help(&self) {
        println!("\x1b[1;36m\u{2554}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2557}\x1b[0m");
        println!("\x1b[1;36m\u{2551}        Available Commands         \u{2551}\x1b[0m");
        println!("\x1b[1;36m\u{255a}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{255d}\x1b[0m");
        println!();
        println!("  \x1b[1;33mscan [timeout]\x1b[0m       - Scan for SpaceBuds devices");
        println!("  \x1b[1;33mconnect <address>\x1b[0m     - Connect to a device by address");
        println!("  \x1b[1;33mstatus\x1b[0m              - Show device status");
        println!("  \x1b[1;33mwatch\x1b[0m               - Live watch status updates");
        println!("  \x1b[1;33manc <on|off|transparency>\x1b[0m - Set ANC mode");
        println!("  \x1b[1;33mlevel <0-15>\x1b[0m        - Set ANC/transparency level");
        println!("  \x1b[1;33meq <0-13>\x1b[0m            - Set EQ preset");
        println!("  \x1b[1;33madaptive <on|off>\x1b[0m   - Set adaptive ANC");
        println!("  \x1b[1;33mdual <on|off>\x1b[0m       - Set dual device mode");
        println!("  \x1b[1;33mgamemode <on|off>\x1b[0m   - Toggle gaming mode");
        println!("  \x1b[1;33mchat <on|off>\x1b[0m       - Toggle chat mode");
        println!("  \x1b[1;33mendurance <on|off>\x1b[0m  - Toggle long endurance mode");
        println!("  \x1b[1;33mrename <name>\x1b[0m       - Rename Bluetooth device");
        println!("  \x1b[1;33mclearpair\x1b[0m           - Clear pairing records");
        println!("  \x1b[1;33mfind [on|off]\x1b[0m      - Ring earbuds (default: on)");
        println!("  \x1b[1;33mclear\x1b[0m               - Clear screen");
        println!("  \x1b[1;33mexit|quit\x1b[0m           - Exit CLI");
        println!();
        println!("  \x1b[1;90mWorkflow: scan -> connect <addr> -> anc/eq/level/etc.\x1b[0m");
        println!("  \x1b[1;90mThe connection lives in the service, shared by all clients.\x1b[0m");
        println!();
    }
}
