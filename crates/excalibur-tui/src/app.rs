use anyhow::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveTab {
    Browser,
    Inspector,
    Repeater,
    Scanner,
}

pub struct TuiApp {
    pub active_tab: ActiveTab,
    pub should_quit: bool,
    pub current_url: String,
}

impl TuiApp {
    pub fn new() -> Self {
        Self {
            active_tab: ActiveTab::Browser,
            should_quit: false,
            current_url: "https://example.com".to_string(),
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        tracing::info!("Initializing Excalibur TUI");
        Ok(())
    }
}

impl Default for TuiApp {
    fn default() -> Self {
        Self::new()
    }
}
