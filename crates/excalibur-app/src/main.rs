#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;

use std::{process::ExitCode, rc::Rc};

use app::ExcaliburBrowserApp;
use excalibur_cef::{bootstrap_cef, StartupOutcome};
use excalibur_core::BrowserConfig;

fn append_diag_log(line: &str) {
    use std::io::Write;
    let path = std::env::temp_dir().join("excalibur_diag.log");
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(f, "[pid={}] {line}", std::process::id());
    }
}

fn main() -> ExitCode {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        append_diag_log(&format!("PANIC: {info}"));
        default_hook(info);
    }));

    let is_sub = std::env::args().any(|a| a.starts_with("--type="));
    if !is_sub {
        append_diag_log(&format!(
            "main enter cwd={:?} args={:?}",
            std::env::current_dir().ok(),
            std::env::args().collect::<Vec<_>>()
        ));
    }

    let mut config = BrowserConfig::default();
    if let Some(arg_url) = std::env::args().skip(1).find(|arg| !arg.starts_with('-')) {
        config.home_url =
            excalibur_core::normalize_url_or_search(&arg_url, &config.search_template);
    }

    let runtime = match bootstrap_cef(&config) {
        Ok(StartupOutcome::SubprocessExited(code)) => {
            if !is_sub {
                append_diag_log(&format!("bootstrap_cef SubprocessExited({code})"));
            }
            return ExitCode::from(u8::try_from(code.max(0)).unwrap_or(1));
        }
        Ok(StartupOutcome::BrowserInitialized(runtime)) => {
            append_diag_log(&format!("CEF initialized, url={}", config.home_url));
            Rc::new(runtime)
        }
        Err(err) => {
            append_diag_log(&format!("CEF bootstrap failed: {err}"));
            eprintln!("Excalibur CEF bootstrap failed: {err}");
            return ExitCode::FAILURE;
        }
    };

    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("excalibur=info")),
        )
        .try_init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Excalibur")
            .with_inner_size([1200.0, 780.0])
            .with_min_inner_size([640.0, 420.0]),
        ..Default::default()
    };

    tracing::info!(pid = std::process::id(), url = %config.home_url, "Starting Excalibur UI");

    let runtime_for_ui = runtime.clone();
    let ui_result = eframe::run_native(
        "Excalibur",
        options,
        Box::new(move |cc| {
            append_diag_log("eframe AppCreator invoked");
            Ok(Box::new(ExcaliburBrowserApp::new(
                cc,
                config,
                runtime_for_ui,
            )))
        }),
    );

    append_diag_log(&format!(
        "eframe::run_native returned: {:?}",
        ui_result.as_ref().err()
    ));
    runtime.shutdown_cleanly();

    if let Err(err) = ui_result {
        eprintln!("Excalibur UI error: {err}");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}
