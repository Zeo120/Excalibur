use clap::{Parser, Subcommand};
use eframe::NativeOptions;
use excalibur_gui::GuiApp;
use excalibur_tui::TuiApp;

#[derive(Parser, Debug)]
#[command(name = "excalibur", author, version, about = "Security-focused non-Chromium browser workbench", long_about = None)]
struct Cli {
    /// URL to open
    #[arg(value_name = "URL")]
    url: Option<String>,

    /// Launch in full-screen Terminal TUI mode
    #[arg(short, long)]
    tui: bool,

    /// Force Native GUI mode
    #[arg(short, long)]
    gui: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Perform a headless security scan against a target URL
    Scan {
        #[arg(value_name = "URL")]
        url: String,
    },
    /// Launch the inbound MITM proxy server
    Proxy {
        #[arg(short, long, default_value = "127.0.0.1:8080")]
        bind: String,
    },
    /// Replay / repeat a saved HTTP request
    Repeat {
        #[arg(value_name = "REQUEST_ID")]
        request_id: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    if let Some(cmd) = cli.command {
        match cmd {
            Commands::Scan { url } => {
                println!("⚔️ Scanning target: {}", url);
            }
            Commands::Proxy { bind } => {
                println!("⚔️ Starting MITM Proxy on: {}", bind);
            }
            Commands::Repeat { request_id } => {
                println!("⚔️ Replaying request: {}", request_id);
            }
        }
        return Ok(());
    }

    if cli.tui {
        let mut app = TuiApp::new();
        if let Some(target) = cli.url {
            app.current_url = target;
        }
        app.run().await?;
    } else {
        let native_options = NativeOptions::default();
        let _ = eframe::run_native(
            "⚔️ Excalibur",
            native_options,
            Box::new(|cc| Ok(Box::new(GuiApp::new(cc)))),
        );
    }

    Ok(())
}
