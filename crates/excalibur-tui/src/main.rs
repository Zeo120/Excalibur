use anyhow::Result;
use excalibur_tui::TuiApp;

#[tokio::main]
async fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let target_url = args.next();

    if let Some(ref arg) = target_url {
        if arg == "-h" || arg == "--help" {
            println!("Excalibur TUI - Security Workbench & Terminal Browser (PRoot Ready)");
            println!("\nUSAGE:");
            println!("    excalibur-tui [TARGET_URL]");
            println!("\nOPTIONS:");
            println!("    -h, --help       Print help information");
            println!("    -v, --version    Print version information");
            println!("\nKEYBINDINGS:");
            println!("    1-4              Switch tabs (1: Browser, 2: Inspector, 3: Repeater, 4: Scanner)");
            println!("    /                Focus & edit URL bar");
            println!("    Enter            Navigate to URL");
            println!("    j / k            Scroll content down / up");
            println!("    H / L            Navigate backward / forward in history");
            println!("    r                Reload current page");
            println!("    ?                Toggle full interactive help overlay");
            println!("    q                Quit");
            println!("\nPROOT / TERMUX TIPS:");
            println!("    Run without X11/Wayland dependencies. If terminal colors appear inverted,");
            println!("    ensure TERM is set (e.g. export TERM=xterm-256color).");
            return Ok(());
        }
        if arg == "-v" || arg == "--version" {
            println!("excalibur-tui {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
    }

    let mut app = TuiApp::new();
    if let Some(url) = target_url {
        app.navigate_to(&url);
    }

    app.run()
}
