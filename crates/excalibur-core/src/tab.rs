/// Minimal tab state tracked by the host shell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabInfo {
    pub id: u64,
    pub url: String,
    pub title: String,
    pub is_loading: bool,
    pub can_go_back: bool,
    pub can_go_forward: bool,
}

/// Manages open tabs and active selection.
#[derive(Debug, Default)]
pub struct TabManager {
    next_id: u64,
    tabs: Vec<TabInfo>,
    active_index: Option<usize>,
}

impl TabManager {
    /// Creates an empty tab manager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Opens a new tab for `url`, selects it, and returns its assigned ID.
    pub fn open_tab(&mut self, url: impl Into<String>) -> u64 {
        self.next_id += 1;
        let id = self.next_id;
        let url = url.into();
        self.tabs.push(TabInfo {
            id,
            title: url.clone(),
            url,
            is_loading: false,
            can_go_back: false,
            can_go_forward: false,
        });
        self.active_index = Some(self.tabs.len() - 1);
        id
    }

    /// Returns a reference to the currently active tab.
    pub fn active_tab(&self) -> Option<&TabInfo> {
        self.active_index.and_then(|idx| self.tabs.get(idx))
    }

    /// Returns a mutable reference to a tab by ID.
    pub fn tab_mut(&mut self, id: u64) -> Option<&mut TabInfo> {
        self.tabs.iter_mut().find(|tab| tab.id == id)
    }

    /// Returns all tabs in order.
    pub fn tabs(&self) -> &[TabInfo] {
        &self.tabs
    }

    /// Closes a tab by ID and updates the active index.
    pub fn close_tab(&mut self, id: u64) -> bool {
        let Some(pos) = self.tabs.iter().position(|tab| tab.id == id) else {
            return false;
        };
        self.tabs.remove(pos);
        if self.tabs.is_empty() {
            self.active_index = None;
        } else if let Some(active) = self.active_index {
            if active >= self.tabs.len() {
                self.active_index = Some(self.tabs.len() - 1);
            } else if active > pos {
                self.active_index = Some(active - 1);
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_and_close_tabs_updates_active_selection() {
        let mut mgr = TabManager::new();
        let t1 = mgr.open_tab("https://example.com");
        let t2 = mgr.open_tab("https://rust-lang.org");
        assert_eq!(mgr.active_tab().map(|t| t.id), Some(t2));

        assert!(mgr.close_tab(t2));
        assert_eq!(mgr.active_tab().map(|t| t.id), Some(t1));

        assert!(mgr.close_tab(t1));
        assert!(mgr.active_tab().is_none());
    }
}
