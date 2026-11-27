#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;

use std::{process::ExitCode, rc::Rc};

use app::ExcaliburBrowserApp;
use excalibur_cef::{bootstrap_cef, StartupOutcome};
use excalibur_core::BrowserConfig;

fn main() -> ExitCode {
    let mut config = BrowserConfig::default();
    if let Some(arg_url) = std::env::args().skip(1).find(|arg| !arg.starts_with('-')) {
        config.home_url =
            excalibur_core::normalize_url_or_search(&arg_url, &config.search_template);
    }

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

    tracing::info!(pid = std::process::id(), url = %config.home_url, "Starting Excalibur UI");

    let runtime_for_ui = runtime.clone();
    let ui_result = eframe::run_native(
        "Excalibur",
        options,
        Box::new(move |cc| {
            Ok(Box::new(ExcaliburBrowserApp::new(
                cc,
                config,
                runtime_for_ui,
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
