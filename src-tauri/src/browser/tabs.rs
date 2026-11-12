use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
