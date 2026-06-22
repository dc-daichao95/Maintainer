pub mod config;
pub mod executor;
pub mod git_checker;
pub mod poller;
pub mod state;

use crate::config::MonitorConfig;
use crate::poller::RepoPoller;
use crate::state::MonitorState;
use clap::Parser;
use std::sync::{Arc, Mutex};

/// Command line arguments for the monitor daemon.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the monitor config JSON file
    #[arg(short, long, default_value = "my-src/tools/repo_monitor/monitor_config.json")]
    config: String,

    /// Path to the Settings TOML file
    #[arg(short, long, default_value = "Settings.toml")]
    settings: String,

    /// Path to the state JSON file
    #[arg(long, default_value = ".monitor_state.json")]
    state: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let shutdown_signal = Arc::new(Mutex::new(false));
    let shutdown_signal_clone = shutdown_signal.clone();

    ctrlc::set_handler(move || {
        println!("Received shutdown signal...");
        let mut shutdown = shutdown_signal_clone.lock().expect("Mutex poisoned");
        *shutdown = true;
    })
    .expect("Error setting Ctrl-C handler");

    println!("Starting repo_monitor daemon...");

    // Create default config if doesn't exist
    if !std::path::Path::new(&args.config).exists() {
        println!("Config not found, creating a default one...");
        std::fs::write(
            &args.config,
            r#"{
    "pull_interval_sec": 10,
    "max_retries": 3,
    "max_history_days": 180,
    "server_ip": "127.0.0.1",
    "server_port": 8080,
    "branches": ["main"]
}"#,
        )?;
    }

    let config = MonitorConfig::load(&args.config, &args.settings)?;

    // Load state or create new
    let state = MonitorState::load(&args.state).unwrap_or_else(|_| MonitorState::new());

    let poller = RepoPoller::new(config, state, &args.state)?;
    poller.run(shutdown_signal).await?;

    println!("Daemon stopped cleanly.");
    Ok(())
}
