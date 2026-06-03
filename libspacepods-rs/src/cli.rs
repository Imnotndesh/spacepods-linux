use crate::client::SpacePodsClient;
use crate::service::DeviceStatus;
use anyhow::{Context, Result};
use crate::commands::eq::{EQ_PRESETS, SPECIAL_PRESETS};
use rustyline::config::{ Config, EditMode};
use rustyline::error::ReadlineError;
use rustyline::highlight::{Highlighter, MatchingBracketHighlighter};
use rustyline::hint::{HistoryHinter};
use rustyline::validate::{MatchingBracketValidator};
use rustyline::{Cmd, CompletionType, Editor, EventHandler, KeyEvent};
use rustyline::history::FileHistory;
use std::borrow::Cow::{self, Borrowed, Owned};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use rustyline_derive::{Completer, Helper, Hinter, Validator};
use tokio::sync::Mutex;
use crate::service::ServiceCommand;
use dialoguer::{theme::ColorfulTheme, Select};

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
    fn highlight<'l>(&self, line: &'l str, pos: usize) -> Cow<'l, str> {
        self.highlighter.highlight(line, pos)
    }
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
    fn highlight_char(&self, line: &str, pos: usize, kind: rustyline::highlight::CmdKind) -> bool {
        self.highlighter.highlight_char(line, pos, kind)
    }
}

pub struct InteractiveCli {
    client: Arc<Mutex<SpacePodsClient>>,
    status: Arc<tokio::sync::RwLock<DeviceStatus>>,
    active_device_selected: AtomicBool,
}

impl InteractiveCli {
    pub async fn new(socket_path: Option<PathBuf>) -> Result<Self> {
        let client = SpacePodsClient::connect(socket_path)
            .await
            .context("Failed to connect to SpacePods service. Is it running?")?;
        let client = Arc::new(Mutex::new(client));
        let status = {
            let mut client_lock = client.lock().await;
            client_lock.get_status().await?
        };
        Ok(Self {
            client,
            status: Arc::new(tokio::sync::RwLock::new(status)),
            active_device_selected: Default::default(),
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

        println!("\x1b[1;36mSpacePods Interactive Shell Initialization Completed.\x1b[0m");
        println!("Type \x1b[1;33mscan\x1b[0m to look for devices and bind your session.");
        println!();

        if rl.load_history(".spacepods_history").is_err() {
            println!("No command history found");
        }

        println!("\x1b[1;32mSpacePods Interactive CLI\x1b[0m");
        println!("Type 'help' for commands, 'exit' to quit");

        match self.check_connection().await {
            Ok(true) => {
                println!("Service connection: \x1b[1;32m✓\x1b[0m");
            }
            _ => {
                println!("Service connection: \x1b[1;31m✗\x1b[0m");
                println!("Cannot connect to SpacePods service. Is it running?");
            }
        }

        loop {
            let prompt = if self.active_device_selected.load(Ordering::SeqCst) {
                "\x1b[1;32mspacepods (connected)>\x1b[0m ".to_string()
            } else {
                "\x1b[1;31mspacepods (no-target)>\x1b[0m ".to_string()
            };

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

    pub async fn handle_command(&self, line: &str) -> Result<bool> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            return Ok(false);
        }

        let cmd = parts[0];

        if cmd == "exit" || cmd == "quit" {
            return Ok(true);
        }
        if cmd == "help" {
            self.print_help();
            return Ok(false);
        }

        if cmd == "scan" {
            println!("\x1b[1;36mQuerying background daemon service to scan BLE space (3s)...\x1b[0m");

            let mut client_lock = self.client.lock().await;
            match client_lock.scan(3).await {
                Ok(devices) => {
                    if devices.is_empty() {
                        println!("\x1b[1;31mNo compatible SpacePods devices detected nearby.\x1b[0m");
                        return Ok(false);
                    }

                    let item_labels: Vec<String> = devices.iter()
                        .map(|(name, addr)| format!("{} \x1b[90m[{}]\x1b[0m", name, addr))
                        .collect();

                    println!("\x1b[1;33mNearby SpacePods detected. Use Up/Down arrows to select yours:\x1b[0m");
                    let selection = Select::with_theme(&ColorfulTheme::default())
                        .with_prompt("Target Device Selection")
                        .items(&item_labels)
                        .default(0)
                        .interact_opt()?;

                    if let Some(idx) = selection {
                        let (name, address) = &devices[idx];
                        println!("Instructing daemon to migrate connection endpoint to {}...", address);

                        match client_lock.send_command(ServiceCommand::SetTargetAddress { address: address.clone() }).await {
                            Ok(_) => {
                                self.active_device_selected.store(true, Ordering::SeqCst);
                                println!("\x1b[1;32m✔ CLI Session Linked to {} [{}]. Controls Unlocked!\x1b[0m", name, address);


                                drop(client_lock);
                                let _ = self.refresh_status().await;
                            }
                            Err(e) => println!("\x1b[1;31mDaemon connection update rejected: {}\x1b[0m", e),
                        }
                    } else {
                        println!("\x1b[1;33mSelection canceled.\x1b[0m");
                    }
                }
                Err(e) => println!("\x1b[1;31mBLE Scanning procedure failed: {}\x1b[0m", e),
            }
            return Ok(false);
        }


        if !self.active_device_selected.load(Ordering::SeqCst) {
            println!("\x1b[1;31m[!] Access Denied: No targeted SpaceBuds assigned for this session.\x1b[0m");
            println!("    You must execute the \x1b[1;33mscan\x1b[0m command and choose your buds first.");
            return Ok(false);
        }


        match cmd {
            "status" => {
                self.refresh_status().await?;
                self.print_status().await;
            }
            "anc" => {
                if parts.len() < 2 {
                    println!("Usage: anc <on|off|transparency>");
                    return Ok(false);
                }
                let mode = parts[1];
                let mut client_lock = self.client.lock().await;
                if let Err(e) = client_lock.set_anc_mode(mode).await {
                    println!("Failed to set ANC mode: {}", e);
                }
            }
            "level" => {
                if parts.len() < 2 {
                    println!("Usage: level <0-15>");
                    return Ok(false);
                }
                if let Ok(lvl) = parts[1].parse::<u8>() {
                    let mut client_lock = self.client.lock().await;
                    if let Err(e) = client_lock.set_level(lvl).await {
                        println!("Failed to set level: {}", e);
                    }
                }
            }
            "watch" => {
                println!("\x1b[1;35mStarting live device event watch loop...\x1b[0m");
                println!("\x1b[90m────────────────────────────────────────────────────────────────\x1b[0m");
                if let Err(e) = self.watch_status().await {
                    println!("\x1b[1;31mWatch loop encountered an error: {}\x1b[0m", e);
                }
            }
            "eq" => {
                if parts.len() < 2 {
                    println!("Usage: eq <preset_index_or_name> (Type 'eq list' to view available styles)");
                    return Ok(false);
                }

                if parts[1] == "list" {
                    println!("\x1b[1;34m--- Standard EQ Presets ---\x1b[0m");
                    for (i, name) in EQ_PRESETS.iter().enumerate() {
                        println!("  [{:?}] {:?}", i, name);
                    }
                    println!("\x1b[1;34m--- Special EQ Presets ---\x1b[0m");
                    for (i, name) in SPECIAL_PRESETS.iter().enumerate() {
                        println!("  [{:?}] {:?}", i + EQ_PRESETS.len(), name);
                    }
                    return Ok(false);
                }

                let target_preset: Option<u8> = if let Ok(idx) = parts[1].parse::<u8>() {
                    if (idx as usize) < (EQ_PRESETS.len() + SPECIAL_PRESETS.len()) {
                        Some(idx)
                    } else {
                        None
                    }
                } else {
                    let search_term = parts[1].to_lowercase();

                    if let Some(pos) = EQ_PRESETS.iter().position(|&s| s.1.to_lowercase() == search_term) {
                        Some(pos as u8)
                    } else if let Some(pos) = SPECIAL_PRESETS.iter().position(|&s| s.1.to_lowercase() == search_term) {
                        Some((pos + EQ_PRESETS.len()) as u8)
                    } else {
                        None
                    }
                };

                match target_preset {
                    Some(preset_id) => {
                        let label = if (preset_id as usize) < EQ_PRESETS.len() {
                            EQ_PRESETS[preset_id as usize]
                        } else {
                            SPECIAL_PRESETS[(preset_id as usize) - EQ_PRESETS.len()]
                        };

                        println!("Switching equalizer signature profile matrix to {:?} (ID: {:?})...", label, preset_id);
                        let mut client_lock = self.client.lock().await;
                        if let Err(e) = client_lock.set_eq_preset(preset_id).await {
                            println!("Failed to push EQ preset changes: {}", e);
                        } else {
                            println!("\x1b[1;32mEqualizer profile shifted successfully.\x1b[0m");
                        }
                    }
                    None => println!("\x1b[1;31mError: Target index or profile string name not recognized. Type 'eq list' for help.\x1b[0m"),
                }
            }
            "adaptive" => {
                if parts.len() < 2 {
                    println!("Usage: adaptive <on|off>");
                    return Ok(false);
                }
                let enabled = parts[1] == "on";
                let mut client_lock = self.client.lock().await;
                if let Err(e) = client_lock.set_adaptive_anc(enabled).await {
                    println!("Failed to toggle adaptive feature: {}", e);
                }
            }
            "dual" => {
                if parts.len() < 2 {
                    println!("Usage: dual <on|off>");
                    return Ok(false);
                }
                let enabled = parts[1] == "on";
                let mut client_lock = self.client.lock().await;
                if let Err(e) = client_lock.set_multi_device(enabled).await {
                    println!("Failed to switch multipoint state: {}", e);
                }
            }
            "remap" => {
                if parts.len() < 3 {
                    println!("Usage: remap <gesture_type_id> <action_id>");
                    return Ok(false);
                }
                if let (Ok(g_type), Ok(action)) = (parts[1].parse::<u8>(), parts[2].parse::<u8>()) {
                    let mut client_lock = self.client.lock().await;
                    if let Err(e) = client_lock.remap_gesture(g_type, action).await {
                        println!("Failed to update touch controls: {}", e);
                    } else {
                        println!("Sent gesture remapping request successfully.");
                    }
                }
            }
            _ => println!("Unknown command. Type 'help' for available choices."),
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

        println!("\x1b[1;36m╔════════════════════════════════════╗\x1b[0m");
        println!("\x1b[1;36m║         SpacePods Status          ║\x1b[0m");
        println!("\x1b[1;36m╚════════════════════════════════════╝\x1b[0m");

        println!("  Connected    : {}", if status.connected {
            "\x1b[1;32m✓\x1b[0m"
        } else {
            "\x1b[1;31m✗\x1b[0m"
        });

        if let Some(addr) = &status.address {
            println!("  Address      : {}", addr);
        }

        match (status.battery_left, status.battery_right, status.battery_case) {
            (Some(l), Some(r), Some(c)) => {
                let l_icon = if l > 80 { "🔋" } else if l > 20 { "🔋" } else { "🪫" };
                let r_icon = if r > 80 { "🔋" } else if r > 20 { "🔋" } else { "🪫" };
                let c_icon = if c > 80 { "🔋" } else if c > 20 { "🔋" } else { "🪫" };
                println!("  Battery      : {} L:{}%  {} R:{}%  {} Case:{}%",
                         l_icon, l, r_icon, r, c_icon, c);
            }
            (Some(l), Some(r), None) => {
                println!("  Battery      : L:{}%  R:{}%", l, r);
            }
            (Some(b), None, None) => {
                println!("  Battery      : {}%", b);
            }
            _ => {}
        }

        let anc_icon = match status.anc_mode {
            Some(0) => "🔇",
            Some(1) => "🎧",
            Some(2) => "👂",
            _ => "❓",
        };
        println!("  ANC Mode     : {} {}", anc_icon, match status.anc_mode {
            Some(0) => "OFF",
            Some(1) => "ANC",
            Some(2) => "TRANSPARENCY",
            _ => "UNKNOWN",
        });

        println!("  Level        : {}/{}", status.anc_level, status.anc_max);

        if let Some(name) = &status.eq_name {
            let eq_icon = "🎛️";
            println!("  EQ           : {} {}", eq_icon, name);
        }

        println!("  Adaptive ANC : {}", match status.adaptive_anc {
            Some(true) => "\x1b[1;32mON\x1b[0m",
            Some(false) => "\x1b[1;33mOFF\x1b[0m",
            None => "UNKNOWN",
        });

        println!("  Dual Device  : {}", match status.dual_device {
            Some(true) => "\x1b[1;32mON\x1b[0m",
            Some(false) => "\x1b[1;33mOFF\x1b[0m",
            None => "UNKNOWN",
        });

        if let Some(gm) = status.game_mode {
            println!("  Game Mode    : {}", if gm { "\x1b[1;32mON\x1b[0m" } else { "\x1b[1;33mOFF\x1b[0m" });
        }

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
        println!("\x1b[1;36m╔══════════════════════════════════════════════════════════════╗\x1b[0m");
        println!("\x1b[1;36m║                        Available Commands                      ║\x1b[0m");
        println!("\x1b[1;36m╚══════════════════════════════════════════════════════════════╝\x1b[0m");
        println!();
        println!("  \x1b[1;33mstatus\x1b[0m              - Show device status");
        println!("  \x1b[1;33mwatch\x1b[0m               - Live watch status updates");
        println!("  \x1b[1;33manc <on|off|transparency>\x1b[0m - Set ANC mode");
        println!("  \x1b[1;33mlevel <0-15>\x1b[0m        - Set ANC/transparency level");
        println!("  \x1b[1;33meq <0-6>\x1b[0m            - Set EQ preset");
        println!("  \x1b[1;33madaptive <on|off>\x1b[0m   - Set adaptive ANC");
        println!("  \x1b[1;33mdual <on|off>\x1b[0m       - Set dual device mode");
        println!("  \x1b[1;33mgamemode <on|off>\x1b[0m   - Enable/disable game (low latency) mode");
        println!("  \x1b[1;33mfind <on|off>\x1b[0m       - Make earbuds beep to find them");
        println!("  \x1b[1;33mfactoryreset\x1b[0m        - Factory reset (confirmation required)");
        println!("  \x1b[1;33mclear\x1b[0m               - Clear screen");
        println!("  \x1b[1;33mexit|quit\x1b[0m           - Exit CLI");
        println!();
        println!("  \x1b[1;90mTip: Use up/down arrows for history\x1b[0m");
        println!("  \x1b[1;90m     Tab for command completion\x1b[0m");
        println!();
    }
}