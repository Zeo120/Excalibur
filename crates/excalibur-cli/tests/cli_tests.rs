use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "excalibur")]
struct Cli {
    #[arg(value_name = "URL")]
    url: Option<String>,

    #[arg(short, long)]
    tui: bool,

    #[arg(short, long)]
    gui: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(clap::Subcommand, Debug, PartialEq, Eq)]
enum Commands {
    Scan {
        #[arg(value_name = "URL")]
        url: String,

        #[arg(short, long)]
        json: bool,

        #[arg(short, long)]
        script: Option<String>,
    },
    Proxy {
        #[arg(short, long, default_value = "127.0.0.1:8080")]
        bind: String,

        #[arg(short, long)]
        storage: Option<String>,
    },
    Repeat {
        #[arg(value_name = "TARGET")]
        target: String,

        #[arg(short, long, default_value = "GET")]
        method: String,
    },
}

#[test]
fn test_cli_parse_tui_and_url() {
    let args = vec!["excalibur", "--tui", "https://target.local"];
    let cli = Cli::try_parse_from(args).expect("parse cli");
    assert!(cli.tui);
    assert!(!cli.gui);
    assert_eq!(cli.url, Some("https://target.local".to_string()));
    assert!(cli.command.is_none());
}

#[test]
fn test_cli_parse_scan_subcommand() {
    let args = vec![
        "excalibur",
        "scan",
        "https://api.target.local",
        "--json",
        "--script",
        "fuzzer.lua",
    ];
    let cli = Cli::try_parse_from(args).expect("parse scan command");
    match cli.command {
        Some(Commands::Scan { url, json, script }) => {
            assert_eq!(url, "https://api.target.local");
            assert!(json);
            assert_eq!(script, Some("fuzzer.lua".to_string()));
        }
        _ => panic!("Expected Scan subcommand"),
    }
}

#[test]
fn test_cli_parse_proxy_subcommand() {
    let args = vec![
        "excalibur",
        "proxy",
        "--bind",
        "0.0.0.0:8888",
        "--storage",
        "test.db",
    ];
    let cli = Cli::try_parse_from(args).expect("parse proxy command");
    match cli.command {
        Some(Commands::Proxy { bind, storage }) => {
            assert_eq!(bind, "0.0.0.0:8888");
            assert_eq!(storage, Some("test.db".to_string()));
        }
        _ => panic!("Expected Proxy subcommand"),
    }
}

#[test]
fn test_cli_parse_repeat_subcommand() {
    let args = vec![
        "excalibur",
        "repeat",
        "https://target.local/api",
        "--method",
        "POST",
    ];
    let cli = Cli::try_parse_from(args).expect("parse repeat command");
    match cli.command {
        Some(Commands::Repeat { target, method }) => {
            assert_eq!(target, "https://target.local/api");
            assert_eq!(method, "POST");
        }
        _ => panic!("Expected Repeat subcommand"),
    }
}
