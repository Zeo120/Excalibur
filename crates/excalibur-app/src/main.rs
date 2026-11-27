#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod drawer;
mod icons;
mod theme;

use std::{process::ExitCode, rc::Rc};

use app::ExcaliburBrowserApp;
use excalibur_cef::{bootstrap_cef, StartupOutcome};
use excalibur_core::{BrowserConfig, Storage};

fn main() -> ExitCode {
    let cli_arg = std::env::args().skip(1).find(|arg| !arg.starts_with('-'));

    // Avoid running Chromium from read-only OS directories such as System32.
    if !std::env::args().any(|a| a.starts_with("--type=")) {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(exe_dir) = exe.parent() {
                let _ = std::env::set_current_dir(exe_dir);
            }
        }
    }

    let storage = Storage::open_default().unwrap_or_else(|err| {
        eprintln!("Falling back to in-memory storage: {err}");
        Storage::open_in_memory().expect("in-memory SQLite initialization")
    });
    let config = storage
        .load_settings(&BrowserConfig::default())
        .unwrap_or_default();
    let initial_url = cli_arg
        .map(|arg| excalibur_core::normalize_url_or_search(&arg, &config.search_template))
        .unwrap_or_else(|| config.home_url.clone());

    let runtime = match bootstrap_cef(&config) {
        Ok(StartupOutcome::SubprocessExited(code)) => {
            return ExitCode::from(u8::try_from(code.max(0)).unwrap_or(1));
        }
        Ok(StartupOutcome::BrowserInitialized(runtime)) => Rc::new(runtime),
        Err(err) => {
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

    let runtime_for_ui = runtime.clone();
    let ui_result = eframe::run_native(
        "Excalibur",
        options,
        Box::new(move |cc| {
            Ok(Box::new(ExcaliburBrowserApp::new(
                cc,
                config,
                storage,
                runtime_for_ui,
                initial_url,
            )))
        }),
    );

    runtime.shutdown_cleanly();

    if let Err(err) = ui_result {
        eprintln!("Excalibur UI error: {err}");
        return ExitCode::FAILURE;
    }

    ExitCode::SUCCESS
}
