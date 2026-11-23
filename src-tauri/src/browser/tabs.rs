use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TabInfo {
    pub id: String,
    pub label: String,
    pub title: String,
    pub url: String,
    pub favicon: Option<String>,
    pub is_loading: bool,
    pub can_go_back: bool,
    pub can_go_forward: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TabManager {
    pub tabs: Vec<TabInfo>,
    pub active_tab_id: Option<String>,
}

impl TabManager {
    pub fn new() -> Self {
        Self {
            tabs: Vec::new(),
            active_tab_id: None,
        }
    }

    pub fn add_tab(&mut self, tab: TabInfo) {
        let id = tab.id.clone();
        self.tabs.push(tab);
        self.active_tab_id = Some(id);
    }

    pub fn get_active_tab(&self) -> Option<&TabInfo> {
        if let Some(active_id) = &self.active_tab_id {
            self.tabs.iter().find(|t| &t.id == active_id)
        } else {
            self.tabs.first()
        }
    }

    pub fn get_active_tab_mut(&mut self) -> Option<&mut TabInfo> {
        if let Some(active_id) = self.active_tab_id.clone() {
            self.tabs.iter_mut().find(|t| t.id == active_id)
        } else {
            self.tabs.first_mut()
        }
    }

    pub fn find_tab(&self, id: &str) -> Option<&TabInfo> {
        self.tabs.iter().find(|t| t.id == id)
    }

    pub fn find_tab_mut(&mut self, id: &str) -> Option<&mut TabInfo> {
        self.tabs.iter_mut().find(|t| t.id == id)
    }

    pub fn find_tab_by_label_mut(&mut self, label: &str) -> Option<&mut TabInfo> {
        self.tabs.iter_mut().find(|t| t.label == label)
    }

    pub fn remove_tab(&mut self, id: &str) -> Option<TabInfo> {
        let index = self.tabs.iter().position(|t| t.id == id)?;
        let removed = self.tabs.remove(index);

        if self.active_tab_id.as_deref() == Some(id) {
            if self.tabs.is_empty() {
                self.active_tab_id = None;
            } else if index < self.tabs.len() {
                self.active_tab_id = Some(self.tabs[index].id.clone());
            } else {
                self.active_tab_id = Some(self.tabs.last().unwrap().id.clone());
            }
        }

        Some(removed)
    }

    pub fn set_active(&mut self, id: &str) -> bool {
        if self.tabs.iter().any(|t| t.id == id) {
            self.active_tab_id = Some(id.to_string());
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_tab(id: &str) -> TabInfo {
        TabInfo {
            id: id.to_string(),
            label: format!("tab_{}", id),
            title: format!("Tab {}", id),
            url: format!("https://example.com/{}", id),
            favicon: None,
            is_loading: false,
            can_go_back: false,
            can_go_forward: false,
        }
    }

    #[test]
    fn test_tab_manager_add_and_active() {
        let mut mgr = TabManager::new();
        assert!(mgr.tabs.is_empty());
        assert_eq!(mgr.get_active_tab(), None);

        mgr.add_tab(dummy_tab("1"));
        assert_eq!(mgr.tabs.len(), 1);
        assert_eq!(mgr.active_tab_id, Some("1".to_string()));
        assert_eq!(mgr.get_active_tab().unwrap().id, "1");

        mgr.add_tab(dummy_tab("2"));
        assert_eq!(mgr.tabs.len(), 2);
        assert_eq!(mgr.active_tab_id, Some("2".to_string()));
        assert_eq!(mgr.get_active_tab().unwrap().id, "2");
    }

    #[test]
    fn test_tab_manager_set_active() {
        let mut mgr = TabManager::new();
        mgr.add_tab(dummy_tab("1"));
        mgr.add_tab(dummy_tab("2"));

        assert!(mgr.set_active("1"));
        assert_eq!(mgr.active_tab_id, Some("1".to_string()));

        // Invalid tab id returns false
        assert!(!mgr.set_active("999"));
        assert_eq!(mgr.active_tab_id, Some("1".to_string()));
    }

    #[test]
    fn test_tab_manager_remove_non_active() {
        let mut mgr = TabManager::new();
        mgr.add_tab(dummy_tab("1"));
        mgr.add_tab(dummy_tab("2"));
        mgr.add_tab(dummy_tab("3"));
        mgr.set_active("1");

        // Remove tab "2" while "1" is active
        let removed = mgr.remove_tab("2");
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().id, "2");
        assert_eq!(mgr.tabs.len(), 2);
        assert_eq!(mgr.active_tab_id, Some("1".to_string()));
    }

    #[test]
    fn test_tab_manager_remove_active_fallback() {
        let mut mgr = TabManager::new();
        mgr.add_tab(dummy_tab("1"));
        mgr.add_tab(dummy_tab("2"));
        mgr.add_tab(dummy_tab("3"));
        mgr.set_active("2");

        // Removing active tab at index 1 -> next tab ("3") becomes active
        mgr.remove_tab("2");
        assert_eq!(mgr.active_tab_id, Some("3".to_string()));

        // Removing active tab at end ("3") -> previous tab ("1") becomes active
        mgr.remove_tab("3");
        assert_eq!(mgr.active_tab_id, Some("1".to_string()));

        // Removing last remaining tab -> active_tab_id becomes None
        mgr.remove_tab("1");
        assert_eq!(mgr.active_tab_id, None);
        assert!(mgr.tabs.is_empty());
    }
}
