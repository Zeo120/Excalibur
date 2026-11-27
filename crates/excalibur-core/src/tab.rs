use std::time::Instant;

/// Minimal tab state tracked by the host shell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabInfo {
    pub id: u64,
    pub url: String,
    pub title: String,
    pub pinned: bool,
    pub is_loading: bool,
    pub can_go_back: bool,
    pub can_go_forward: bool,
    pub browser_created: bool,
    pub last_active_at: Instant,
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
        let now = Instant::now();
        self.tabs.push(TabInfo {
            id,
            title: url.clone(),
            url,
            pinned: false,
            is_loading: false,
            can_go_back: false,
            can_go_forward: false,
            browser_created: false,
            last_active_at: now,
        });
        self.active_index = Some(self.tabs.len() - 1);
        id
    }

    /// Returns a reference to the currently active tab.
    pub fn active_tab(&self) -> Option<&TabInfo> {
        self.active_index.and_then(|idx| self.tabs.get(idx))
    }

    /// Returns the active tab ID, if any.
    pub fn active_tab_id(&self) -> Option<u64> {
        self.active_tab().map(|tab| tab.id)
    }

    /// Selects a tab by ID and updates its `last_active_at` timestamp.
    pub fn select_tab(&mut self, id: u64) -> bool {
        let Some(pos) = self.tabs.iter().position(|tab| tab.id == id) else {
            return false;
        };
        let now = Instant::now();
        if let Some(prev) = self.active_index.and_then(|idx| self.tabs.get_mut(idx)) {
            prev.last_active_at = now;
        }
        self.active_index = Some(pos);
        self.tabs[pos].last_active_at = now;
        true
    }

    /// Cycles active selection forward (`delta > 0`) or backward (`delta < 0`).
    pub fn cycle_tab(&mut self, forward: bool) -> Option<u64> {
        if self.tabs.is_empty() {
            return None;
        }
        let len = self.tabs.len();
        let current = self.active_index.unwrap_or(0);
        let next = if forward {
            (current + 1) % len
        } else {
            (current + len - 1) % len
        };
        let id = self.tabs[next].id;
        self.select_tab(id);
        Some(id)
    }

    /// Toggles pinned state for `id` and keeps pinned tabs grouped at the front.
    pub fn toggle_pin(&mut self, id: u64) -> Option<bool> {
        let pos = self.tabs.iter().position(|tab| tab.id == id)?;
        let active_id = self.active_tab_id();
        let new_pinned = !self.tabs[pos].pinned;
        self.tabs[pos].pinned = new_pinned;

        let tab = self.tabs.remove(pos);
        let insert_at = if new_pinned {
            self.tabs.iter().filter(|t| t.pinned).count()
        } else {
            self.tabs.iter().filter(|t| t.pinned).count()
        };
        self.tabs.insert(insert_at, tab);

        if let Some(aid) = active_id {
            self.active_index = self.tabs.iter().position(|t| t.id == aid);
        }
        Some(new_pinned)
    }

    /// Returns a mutable reference to a tab by ID.
    pub fn tab_mut(&mut self, id: u64) -> Option<&mut TabInfo> {
        self.tabs.iter_mut().find(|tab| tab.id == id)
    }

    /// Returns a reference to a tab by ID.
    pub fn tab(&self, id: u64) -> Option<&TabInfo> {
        self.tabs.iter().find(|tab| tab.id == id)
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
            if let Some(tab) = self.active_index.and_then(|idx| self.tabs.get_mut(idx)) {
                tab.last_active_at = Instant::now();
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_select_cycle_and_close_tabs() {
        let mut mgr = TabManager::new();
        let t1 = mgr.open_tab("https://example.com");
        let t2 = mgr.open_tab("https://rust-lang.org");
        let t3 = mgr.open_tab("https://crates.io");
        assert_eq!(mgr.active_tab_id(), Some(t3));

        assert_eq!(mgr.cycle_tab(true), Some(t1));
        assert_eq!(mgr.cycle_tab(false), Some(t3));
        assert!(mgr.select_tab(t2));
        assert_eq!(mgr.active_tab_id(), Some(t2));

        assert!(mgr.close_tab(t2));
        assert_eq!(mgr.active_tab_id(), Some(t3));

        assert!(mgr.close_tab(t3));
        assert_eq!(mgr.active_tab_id(), Some(t1));

        assert!(mgr.close_tab(t1));
        assert!(mgr.active_tab().is_none());
    }

    #[test]
    fn toggle_pin_groups_pinned_tabs_first() {
        let mut mgr = TabManager::new();
        let t1 = mgr.open_tab("https://a.example");
        let t2 = mgr.open_tab("https://b.example");
        let _t3 = mgr.open_tab("https://c.example");

        assert_eq!(mgr.toggle_pin(t2), Some(true));
        assert_eq!(mgr.tabs()[0].id, t2);
        assert_eq!(mgr.tabs()[1].id, t1);

        assert_eq!(mgr.toggle_pin(t2), Some(false));
        assert_eq!(mgr.tabs()[0].id, t2);
    }
}
