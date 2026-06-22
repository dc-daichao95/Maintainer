use anyhow::Result;
use crate::git_checker::GitChecker;
use crate::executor::SyncExecutor;
use crate::state::MonitorState;
use crate::config::MonitorConfig;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::time::sleep;

/// Polls a git repository for changes and triggers synchronization.
pub struct RepoPoller {
    config: MonitorConfig,
    state: MonitorState,
    state_path: String,
    git_checker: GitChecker,
    executor: SyncExecutor,
}

impl RepoPoller {
    /// Creates a new `RepoPoller`.
    pub fn new(config: MonitorConfig, state: MonitorState, state_path: &str) -> Result<Self> {
        let absolute_repo_path = config.get_absolute_repo_path()?.to_string_lossy().to_string();
        Ok(Self {
            executor: SyncExecutor::new(&config.server_ip, config.server_port, config.max_retries, &absolute_repo_path),
            config,
            state,
            state_path: state_path.to_string(),
            git_checker: GitChecker::new(&absolute_repo_path),
        })
    }

    /// Pre-flight check: verifies all configured branches have an upstream.
    pub fn pre_flight_check(&self) -> Result<()> {
        for branch in &self.config.branches {
            match self.git_checker.get_upstream_hash(branch) {
                Ok(_) => println!("Branch '{}' has a valid upstream.", branch),
                Err(e) => anyhow::bail!("Pre-flight check failed for branch '{}': {}", branch, e),
            }
        }
        Ok(())
    }

    /// Performs a single polling iteration.
    pub async fn poll_once(&mut self) -> Result<()> {
        // 1. Fetch latest changes
        if let Err(e) = self.git_checker.fetch_all() {
            eprintln!("Failed to fetch --all: {}", e);
            return Err(e);
        }

        // 2. Iterate through configured branches
        for branch in &self.config.branches {
            let upstream_hash = match self.git_checker.get_upstream_hash(branch) {
                Ok(hash) => hash,
                Err(e) => {
                    eprintln!("Failed to get upstream hash for branch '{}': {}", branch, e);
                    continue;
                }
            };

            let mut state_changed = false;

            {
                let branch_state = self.state.get_mut_branch_state(branch);
                
                // If there's a new commit (or no commit synced yet)
                if branch_state.last_synced_commit.as_deref() != Some(&upstream_hash) {
                    
                    if branch_state.retry_count >= self.config.max_retries {
                        eprintln!("Branch '{}' reached max retries ({}), skipping until new commit or manual fix.", branch, self.config.max_retries);
                        continue;
                    }

                    // Generate commit list
                    let new_commits = match self.git_checker.get_new_commits(
                        branch_state.last_synced_commit.as_deref(),
                        &upstream_hash,
                        self.config.max_history_days,
                    ) {
                        Ok(commits) => commits,
                        Err(e) => {
                            eprintln!("Failed to get new commits for branch '{}': {}", branch, e);
                            continue;
                        }
                    };

                    println!("Found {} new commits on branch '{}'", new_commits.len(), branch);

                    let mut all_success = true;
                    for commit in &new_commits {
                        println!("Syncing commit: {}", commit);
                        // Execute sync
                        if let Err(e) = self.executor.execute_sync(commit) {
                            eprintln!("Sync execution failed for branch '{}', commit '{}': {}", branch, commit, e);
                            branch_state.retry_count += 1;
                            state_changed = true;
                            all_success = false;
                            break;
                        } else {
                            // Success for this commit
                            branch_state.last_synced_commit = Some(commit.clone());
                            state_changed = true;
                        }
                    }

                    if all_success {
                        branch_state.last_synced_commit = Some(upstream_hash.clone());
                        branch_state.retry_count = 0;
                        state_changed = true;
                    }
                } else {
                    // Reset retry count if it's the same hash and already synced successfully
                    if branch_state.retry_count > 0 {
                        branch_state.retry_count = 0;
                        state_changed = true;
                    }
                }
            }

            // Save state if changed
            if state_changed {
                if let Err(e) = self.state.save(&self.state_path) {
                    eprintln!("Failed to save state: {}", e);
                }
            }
        }

        Ok(())
    }

    /// Runs the polling loop until the shutdown signal is received.
    pub async fn run(mut self, shutdown_signal: Arc<Mutex<bool>>) -> Result<()> {
        self.pre_flight_check()?;
        println!("Pre-flight check passed. Starting polling loop...");

        let interval = Duration::from_secs(self.config.pull_interval_sec);
        loop {
            {
                let shutdown = shutdown_signal.lock().expect("Mutex poisoned");
                if *shutdown {
                    println!("Shutdown signal received, exiting poller loop.");
                    break;
                }
            }

            if let Err(e) = self.poll_once().await {
                eprintln!("Poll error: {}", e);
            }

            sleep(interval).await;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_poller_e2e_mock() {
        let dir = tempdir().unwrap();
        let repo_path = dir.path().join("repo");
        fs::create_dir(&repo_path).unwrap();

        Command::new("git").current_dir(&repo_path).args(["init"]).output().unwrap();
        Command::new("git").current_dir(&repo_path).args(["config", "user.name", "Test"]).output().unwrap();
        Command::new("git").current_dir(&repo_path).args(["config", "user.email", "t@t.com"]).output().unwrap();

        fs::write(repo_path.join("file.txt"), "hello").unwrap();
        Command::new("git").current_dir(&repo_path).args(["add", "."]).output().unwrap();
        Command::new("git").current_dir(&repo_path).args(["commit", "-m", "init"]).output().unwrap();

        // Create remote
        let remote_path = dir.path().join("remote");
        fs::create_dir(&remote_path).unwrap();
        Command::new("git").current_dir(&remote_path).args(["init", "--bare"]).output().unwrap();

        let remote_url = remote_path.to_str().unwrap().replace("\\", "/");
        Command::new("git").current_dir(&repo_path).args(["remote", "add", "origin", &remote_url]).output().unwrap();
        Command::new("git").current_dir(&repo_path).args(["branch", "-M", "main"]).output().unwrap();
        Command::new("git").current_dir(&repo_path).args(["push", "-u", "origin", "main"]).output().unwrap();

        // Setup mock cli
        // Windows mock
        let mock_cli_path = dir.path().join("sashiko-cli.cmd");
        fs::write(&mock_cli_path, "@echo off\necho mock cli executed\nexit 0").unwrap();
        std::env::set_var("SASHIKO_CLI_PATH", mock_cli_path.to_str().unwrap());

        let config = MonitorConfig {
            repository_path: repo_path.to_str().unwrap().to_string(),
            max_retries: 3,
            pull_interval_sec: 1,
            server_ip: "127.0.0.1".to_string(),
            server_port: 8080,
            branches: vec!["main".to_string()],
            max_history_days: 180,
        };

        let state_path = dir.path().join(".state.json");
        let state = MonitorState::new();
        let mut poller = RepoPoller::new(config, state, state_path.to_str().unwrap()).unwrap();

        // 1. Pre-flight check should pass
        assert!(poller.pre_flight_check().is_ok());

        // 2. Poll once (should detect initial commit since state is empty)
        assert!(poller.poll_once().await.is_ok());

        // Check state updated
        let updated_state = MonitorState::load(state_path.to_str().unwrap()).unwrap();
        let branch_state = updated_state.get_branch_state("main").unwrap();
        assert!(branch_state.last_synced_commit.is_some());
        let synced_hash = branch_state.last_synced_commit.clone().unwrap();

        // 3. Add new commit to remote directly
        let cloned_path = dir.path().join("clone");
        let clone_out = Command::new("git").current_dir(dir.path()).args(["clone", "-b", "main", &remote_url, "clone"]).output().unwrap();
        if !clone_out.status.success() { panic!("Clone failed: {}", String::from_utf8_lossy(&clone_out.stderr)); }
        Command::new("git").current_dir(&cloned_path).args(["config", "user.name", "Test"]).output().unwrap();
        Command::new("git").current_dir(&cloned_path).args(["config", "user.email", "t@t.com"]).output().unwrap();
        fs::write(cloned_path.join("file2.txt"), "world").unwrap();
        Command::new("git").current_dir(&cloned_path).args(["add", "."]).output().unwrap();
        let commit_out = Command::new("git").current_dir(&cloned_path).args(["commit", "-m", "add file2"]).output().unwrap();
        if !commit_out.status.success() { panic!("Commit failed: {}", String::from_utf8_lossy(&commit_out.stderr)); }
        let push_out = Command::new("git").current_dir(&cloned_path).args(["push", "origin", "HEAD:main"]).output().unwrap();
        if !push_out.status.success() { panic!("Push failed: {}", String::from_utf8_lossy(&push_out.stderr)); }

        // 4. Poll again, should detect new commit
        assert!(poller.poll_once().await.is_ok());
        
        let updated_state2 = MonitorState::load(state_path.to_str().unwrap()).unwrap();
        let branch_state2 = updated_state2.get_branch_state("main").unwrap();
        let synced_hash2 = branch_state2.last_synced_commit.clone().unwrap();
        
        assert_ne!(synced_hash, synced_hash2);
    }
}
