mod config;
mod daemon;
mod hook;
mod install;
mod setup;
mod types;

use clap::{Parser, Subcommand};
use types::HookType;

#[derive(Parser)]
#[command(name = "claude-telegram-bridge")]
#[command(about = "Bridge between Claude Code hooks and Telegram")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the background daemon (HTTP server + Telegram polling + tray icon)
    Daemon,
    /// Hook thin client — called by Claude Code
    Hook {
        /// Hook type: permission, notification, or stop
        #[arg(value_enum)]
        hook_type: HookTypeArg,
    },
    /// Interactive setup wizard
    Setup,
    /// Install as system service and configure Claude Code settings
    Install,
}

#[derive(Clone, clap::ValueEnum)]
enum HookTypeArg {
    Permission,
    Notification,
    Stop,
}

impl From<HookTypeArg> for HookType {
    fn from(arg: HookTypeArg) -> Self {
        match arg {
            HookTypeArg::Permission => HookType::Permission,
            HookTypeArg::Notification => HookType::Notification,
            HookTypeArg::Stop => HookType::Stop,
        }
    }
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Daemon => run_daemon(),
        Commands::Hook { hook_type } => {
            hook::run(hook_type.into());
        }
        Commands::Setup => {
            if let Err(e) = setup::run() {
                eprintln!("Setup error: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Install => {
            if let Err(e) = install::run() {
                eprintln!("Install error: {}", e);
                std::process::exit(1);
            }
        }
    }
}

fn run_daemon() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let config = match config::Config::load() {
        Some(c) => c,
        None => {
            eprintln!("Error: Config not found or missing bot_token/chat_id.");
            eprintln!("Run first: claude-telegram-bridge setup");
            std::process::exit(1);
        }
    };

    if config.disabled {
        eprintln!("Warning: Telegram bridge is disabled (disabled=true)");
        std::process::exit(0);
    }

    // Create channels for tray <-> async runtime communication
    let (tray_tx, tray_rx) = std::sync::mpsc::channel();
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    // Spawn the async runtime on a separate thread
    let config_clone = config.clone();
    let tray_tx_clone = tray_tx.clone();
    let runtime_thread = std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
        rt.block_on(async {
            if let Err(e) = daemon::run(config_clone, tray_tx_clone, shutdown_rx).await {
                tracing::error!("Daemon error: {}", e);
            }
        });
    });

    // Run tray event loop on the main thread (required by macOS)
    daemon::tray::run_tray_loop(tray_rx, shutdown_tx);

    // Wait for runtime thread to finish
    let _ = runtime_thread.join();
}
