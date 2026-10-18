use anyhow::{Context as AnyhowContext, Result};
use clap::{Parser, Subcommand};
use eframe::NativeOptions;
use excalibur_audit::ResponseAuditor;
use excalibur_core::{
    client::{ClientOptions, HttpClient},
    network::{HttpHeaders, HttpRequest},
    storage::WorkspaceStorage,
};
use excalibur_dom::{HtmlParser, ResourceExtractor};
use excalibur_gui::GuiApp;
use excalibur_proxy::{
    cert::CertificateAuthority, interceptor::TrafficInterceptor, server::ProxyServer,
};
use excalibur_script::{JsRuntime, LuaRuntime};
use excalibur_tui::TuiApp;
use serde::Serialize;
use std::net::SocketAddr;
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(
    name = "excalibur",
    author,
    version,
    about = "Security-focused non-Chromium browser workbench",
    long_about = None
)]
struct Cli {
    /// Target URL to open in browser viewport
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
    /// Perform a headless security audit against a target URL
    Scan {
        #[arg(value_name = "URL")]
        url: String,

        /// Output report in JSON format
        #[arg(short, long)]
        json: bool,

        /// Path to custom Lua fuzzer/auditor script
        #[arg(short, long)]
        script: Option<String>,
    },
    /// Launch the inbound MITM proxy server
    Proxy {
        /// Address to bind the proxy server to
        #[arg(short, long, default_value = "127.0.0.1:8080")]
        bind: String,

        /// Path to SQLite workspace storage database
        #[arg(short, long)]
        storage: Option<String>,
    },
    /// Replay / repeat an HTTP request
    Repeat {
        /// Target URL to replay
        #[arg(value_name = "TARGET")]
        target: String,

        /// Override HTTP method (default: GET)
        #[arg(short, long, default_value = "GET")]
        method: String,
    },
}

#[derive(Serialize)]
struct CliScanReport {
    target: String,
    status: u16,
    total_findings: usize,
    header_flaws: Vec<excalibur_audit::HeaderAuditFinding>,
    secret_leaks: Vec<excalibur_audit::SecretFinding>,
    dom_xss_sinks: Vec<excalibur_script::js::SinkFinding>,
    form_warnings: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    if let Some(cmd) = cli.command {
        match cmd {
            Commands::Scan { url, json, script } => {
                run_scan(&url, json, script.as_deref()).await?;
            }
            Commands::Proxy { bind, storage } => {
                run_proxy(&bind, storage.as_deref()).await?;
            }
            Commands::Repeat { target, method } => {
                run_repeat(&target, &method).await?;
            }
        }
        return Ok(());
    }

    // Determine whether to run GUI or TUI
    let run_tui_mode = if cli.tui {
        true
    } else if cli.gui {
        false
    } else {
        // Auto-detect headless / SSH environment
        std::env::var("SSH_CLIENT").is_ok()
            || std::env::var("SSH_TTY").is_ok()
            || (cfg!(target_os = "linux")
                && std::env::var("DISPLAY").is_err()
                && std::env::var("WAYLAND_DISPLAY").is_err())
    };

    if run_tui_mode {
        let mut app = TuiApp::new();
        if let Some(target) = cli.url {
            app.navigate_to(&target);
        }
        app.run()?;
    } else {
        let native_options = NativeOptions::default();
        let target_url = cli.url.clone();
        let _ = eframe::run_native(
            "Excalibur Security Workbench",
            native_options,
            Box::new(move |cc| {
                let mut app = GuiApp::new(cc);
                if let Some(url) = target_url {
                    app.url_input = url;
                }
                Ok(Box::new(app))
            }),
        );
    }

    Ok(())
}

async fn run_scan(url: &str, json_output: bool, script_path: Option<&str>) -> Result<()> {
    if !json_output {
        println!("[*] Excalibur Security Audit Scanner");
        println!("[*] Dispatching request to target: {}", url);
    }

    let client = HttpClient::new(ClientOptions::default())?;
    let mut req = HttpRequest::new("GET", url);
    req.headers.append(
        "User-Agent",
        "Excalibur-Auditor/0.1.0 (Security Inspection)",
    );

    // If custom Lua script is provided, apply request hooks
    if let Some(script_file) = script_path {
        let lua_code = std::fs::read_to_string(script_file)
            .with_context(|| format!("Failed to read Lua script from: {}", script_file))?;
        let lua = LuaRuntime::new()?;
        lua.execute(&lua_code)?;
        req = lua.apply_request_hook(req)?;
    }

    let is_https = url.starts_with("https://");
    let res = client.dispatch(&req).await?;

    if !json_output {
        println!(
            "[*] Received response: HTTP {} ({} bytes, {} ms)",
            res.status,
            res.body.len(),
            res.elapsed_ms
        );
    }

    // 1. Audit Headers and Secrets
    let auditor = ResponseAuditor::new();
    let audit_report = auditor.audit_response(&res, is_https);

    // 2. Parse HTML and inspect forms
    let body_str = String::from_utf8_lossy(&res.body);
    let dom = HtmlParser::parse(&body_str);
    let forms = ResourceExtractor::extract_forms(&dom);

    let mut form_warnings = Vec::new();
    for form in forms {
        if form.has_password_field && !is_https {
            form_warnings.push(format!(
                "Form action '{}' contains password field submitted over cleartext HTTP",
                form.action
            ));
        }
        if !form.has_csrf_token && form.method.eq_ignore_ascii_case("POST") {
            form_warnings.push(format!(
                "POST Form action '{}' lacks apparent anti-CSRF token",
                form.action
            ));
        }
    }

    // 3. QuickJS DOM Sink Analysis on scripts
    let js = JsRuntime::new()?;
    js.set_url(url)?;

    for script_node in dom.find_by_tag("script") {
        let code = script_node.text_content();
        if !code.trim().is_empty() {
            let _ = js.eval(code.trim());
        }
    }
    let sink_findings = js.get_sink_findings();

    let total_issues = audit_report.total_issues() + form_warnings.len() + sink_findings.len();

    let has_critical_or_high = audit_report.header_findings.iter().any(|f| {
        f.severity == excalibur_audit::AuditSeverity::High
            || f.severity == excalibur_audit::AuditSeverity::Critical
    }) || audit_report.secret_findings.iter().any(|s| {
        s.severity == excalibur_audit::AuditSeverity::High
            || s.severity == excalibur_audit::AuditSeverity::Critical
    });

    if json_output {
        let report = CliScanReport {
            target: url.to_string(),
            status: res.status,
            total_findings: total_issues,
            header_flaws: audit_report.header_findings,
            secret_leaks: audit_report.secret_findings,
            dom_xss_sinks: sink_findings,
            form_warnings,
        };
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        println!("\n=======================================================");
        println!("                EXCALIBUR AUDIT REPORT                 ");
        println!("=======================================================");
        println!(" Target: {}", url);
        println!(" Status: HTTP {}", res.status);
        println!(" Total Issues Identified: {}", total_issues);
        println!("-------------------------------------------------------");

        if audit_report.header_findings.is_empty() {
            println!("  [+] Response Security Headers: PASS");
        } else {
            println!(
                "  [-] Header Security Flaws ({}):",
                audit_report.header_findings.len()
            );
            for f in &audit_report.header_findings {
                println!("      - [{:?}] {}: {}", f.severity, f.rule_id, f.title);
                println!("        Recommendation: {}", f.recommendation);
            }
        }

        if audit_report.secret_findings.is_empty() {
            println!("  [+] Sensitive Token Leak Hunter: CLEAN");
        } else {
            println!(
                "  [!] Detected Secret Disclosures ({}):",
                audit_report.secret_findings.len()
            );
            for s in &audit_report.secret_findings {
                println!(
                    "      - [{:?}] {}: {} (Offset: {})",
                    s.severity, s.rule_name, s.redacted_text, s.offset
                );
            }
        }

        if !form_warnings.is_empty() {
            println!(
                "  [!] Form Vulnerability Warnings ({}):",
                form_warnings.len()
            );
            for w in &form_warnings {
                println!("      - {}", w);
            }
        }

        if !sink_findings.is_empty() {
            println!("  [!] DOM XSS Sinks Intercepted ({}):", sink_findings.len());
            for sink in &sink_findings {
                println!("      - [XSS SINK] {}: {}", sink.sink_name, sink.argument);
            }
        }
        println!("=======================================================\n");
    }

    if has_critical_or_high {
        std::process::exit(1);
    }

    Ok(())
}

async fn run_proxy(bind_addr: &str, storage_path: Option<&str>) -> Result<()> {
    println!("=======================================================");
    println!("        EXCALIBUR INBOUND MITM PROXY DAEMON            ");
    println!("=======================================================");
    println!(" [*] Binding to: http://{}", bind_addr);

    let ca = Arc::new(CertificateAuthority::new_ephemeral().context("Failed to generate root CA")?);
    println!(" [*] Root CA dynamically forged for TLS interception.");

    let storage = Arc::new(if let Some(path) = storage_path {
        println!(" [*] Connecting to persistent workspace: {}", path);
        WorkspaceStorage::open(path)?
    } else {
        println!(" [*] Operating in ephemeral in-memory storage mode.");
        WorkspaceStorage::in_memory()?
    });

    let bind_socket: SocketAddr = bind_addr
        .parse()
        .with_context(|| format!("Invalid bind address: {}", bind_addr))?;

    let (event_tx, _event_rx) = tokio::sync::mpsc::channel(100);
    let interceptor = Arc::new(TrafficInterceptor::new(event_tx));
    let proxy = ProxyServer::new(bind_socket, ca, interceptor, Some(storage), None)?;

    tokio::spawn(async move {
        if let Err(e) = proxy.run().await {
            eprintln!("Proxy server error: {}", e);
        }
    });

    println!(" [*] Proxy is actively listening and intercepting traffic.");
    println!(" [*] Configure browser HTTP/HTTPS proxy to: {}", bind_addr);
    println!(" [*] Press Ctrl+C to terminate proxy.");
    println!("=======================================================");

    tokio::signal::ctrl_c().await?;
    println!("\n[*] Shutting down Excalibur MITM Proxy.");
    Ok(())
}

async fn run_repeat(target: &str, method: &str) -> Result<()> {
    println!("[*] Replaying request: {} {}", method, target);
    let client = HttpClient::new(ClientOptions::default())?;

    let mut headers = HttpHeaders::new();
    headers.append("User-Agent", "Excalibur-Repeater/0.1.0");
    headers.append("Accept", "*/*");

    let mut req = HttpRequest::new(method, target);
    req.headers = headers;

    let res = client.dispatch(&req).await?;
    println!("\n--- Response Summary ---");
    println!("HTTP/1.1 {} {}", res.status, res.reason);
    println!(
        "Duration: {} ms | Size: {} bytes\n",
        res.elapsed_ms,
        res.body.len()
    );

    println!("--- Headers ---");
    println!("{}", res.headers.to_raw_string());

    println!("--- Body Preview (up to 512 bytes) ---");
    let preview_len = res.body.len().min(512);
    let body_str = String::from_utf8_lossy(&res.body[..preview_len]);
    println!("{}", body_str);
    if res.body.len() > 512 {
        println!(
            "\n[... truncated {} remaining bytes ...]",
            res.body.len() - 512
        );
    }

    Ok(())
}
