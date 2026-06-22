use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Represents the state of a single branch.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct BranchState {
    pub branch_name: String,
    pub last_synced_commit: Option<String>,
    pub retry_count: u32,
}

/// Represents the persistent state of the monitor daemon.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct MonitorState(pub Vec<BranchState>);

impl Default for MonitorState {
    fn default() -> Self {
        Self::new()
    }
}

impl MonitorState {
    /// Creates a new, empty monitor state.
    pub fn new() -> Self {
        MonitorState(Vec::new())
    }

    /// Loads the monitor state from the specified file path.
    pub fn load(path: &str) -> Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let state: Vec<BranchState> = serde_json::from_str(&content)?;
        Ok(MonitorState(state))
    }

    /// Saves the monitor state to the specified file path safely using a temporary file.
    pub fn save(&self, path: &str) -> Result<()> {
        let content = serde_json::to_string_pretty(&self.0)?;
        let tmp_path = format!("{}.tmp", path);
        std::fs::write(&tmp_path, content)?;
        std::fs::rename(&tmp_path, path)?;
        Ok(())
    }

    /// Gets the state for a specific branch.
    pub fn get_branch_state(&self, branch_name: &str) -> Option<&BranchState> {
        self.0.iter().find(|b| b.branch_name == branch_name)
    }

    /// Gets or creates the state for a specific branch (mutable).
    pub fn get_mut_branch_state(&mut self, branch_name: &str) -> &mut BranchState {
        if let Some(pos) = self.0.iter().position(|b| b.branch_name == branch_name) {
            &mut self.0[pos]
        } else {
            self.0.push(BranchState {
                branch_name: branch_name.to_string(),
                last_synced_commit: None,
                retry_count: 0,
            });
            self.0.last_mut().unwrap()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;

    #[test]
    fn test_save_and_load_state() {
        let dir = tempdir().unwrap();
        let state_file = dir.path().join(".monitor_state.json");
        let path_str = state_file.to_str().unwrap();

        let mut state = MonitorState::new();
        let branch_state = state.get_mut_branch_state("main");
        branch_state.last_synced_commit = Some("12345abcde".to_string());
        branch_state.retry_count = 1;

        state.save(path_str).unwrap();

        let loaded = MonitorState::load(path_str).unwrap();
        assert_eq!(state, loaded);
        
        let loaded_branch = loaded.get_branch_state("main").unwrap();
        assert_eq!(loaded_branch.last_synced_commit.as_deref(), Some("12345abcde"));
        assert_eq!(loaded_branch.retry_count, 1);
        
        // Also check if `.tmp` file is not left behind
        let tmp_file = format!("{}.tmp", path_str);
        assert!(fs::metadata(&tmp_file).is_err());
    }

    #[test]
    fn test_load_non_existent_state() {
        let dir = tempdir().unwrap();
        let state_file = dir.path().join("does_not_exist.json");
        let path_str = state_file.to_str().unwrap();

        let loaded = MonitorState::load(path_str);
        assert!(loaded.is_err());
    }
}
