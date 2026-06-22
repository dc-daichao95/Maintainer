use std::process::Command;
use anyhow::{Context, Result};

/// Checks a git repository for changes.
pub struct GitChecker {
    repo_path: String,
}

impl GitChecker {
    /// Creates a new `GitChecker` for the specified repository path.
    pub fn new(repo_path: &str) -> Self {
        Self {
            repo_path: repo_path.to_string(),
        }
    }

    /// Fetches all changes from all remotes.
    pub fn fetch_all(&self) -> Result<()> {
        let output = Command::new("git")
            .current_dir(&self.repo_path)
            .args(["fetch", "--all"])
            .output()
            .context("Failed to execute git fetch --all")?;
        
        if !output.status.success() {
            anyhow::bail!("git fetch --all failed: {}", String::from_utf8_lossy(&output.stderr));
        }
        Ok(())
    }

    /// Gets the hash of the upstream commit for a given branch.
    pub fn get_upstream_hash(&self, branch: &str) -> Result<String> {
        let upstream = format!("{}@{{u}}", branch);
        let output = Command::new("git")
            .current_dir(&self.repo_path)
            .args(["rev-parse", &upstream])
            .output()
            .context("Failed to execute git rev-parse")?;
            
        if !output.status.success() {
            anyhow::bail!("git rev-parse {} failed: {}", upstream, String::from_utf8_lossy(&output.stderr));
        }
        
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Gets the commit range, respecting max_history_days.
    /// If start is missing or too old, it uses the oldest commit within the days limit.
    pub fn get_new_commits(&self, start: Option<&str>, end: &str, days: u32) -> Result<Vec<String>> {
        let since_arg = format!("--since={} days ago", days);
        let mut args = vec!["rev-list", &since_arg, "--reverse"];
        
        let range;
        if let Some(start_hash) = start {
            range = format!("{}..{}", start_hash, end);
            args.push(&range);
        } else {
            args.push(end);
        }

        let output = Command::new("git")
            .current_dir(&self.repo_path)
            .args(&args)
            .output()
            .context("Failed to execute git rev-list")?;

        if !output.status.success() {
            anyhow::bail!("git rev-list failed: {}", String::from_utf8_lossy(&output.stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let commits: Vec<String> = stdout
            .lines()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        Ok(commits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_git_checker_fetch_and_hash() {
        let dir = tempdir().unwrap();
        let repo_path = dir.path().join("repo");
        fs::create_dir(&repo_path).unwrap();

        Command::new("git").current_dir(&repo_path).args(["init"]).output().unwrap();
        Command::new("git").current_dir(&repo_path).args(["config", "user.name", "Test User"]).output().unwrap();
        Command::new("git").current_dir(&repo_path).args(["config", "user.email", "test@example.com"]).output().unwrap();

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

        let checker = GitChecker::new(repo_path.to_str().unwrap());
        
        // Test fetch_all
        assert!(checker.fetch_all().is_ok());

        // Test get_upstream_hash
        let upstream_hash = checker.get_upstream_hash("main").unwrap();
        assert!(!upstream_hash.is_empty());
    }

    #[test]
    fn test_get_new_commits() {
        let dir = tempdir().unwrap();
        let repo_path = dir.path().join("repo");
        fs::create_dir(&repo_path).unwrap();

        Command::new("git").current_dir(&repo_path).args(["init"]).output().unwrap();
        Command::new("git").current_dir(&repo_path).args(["config", "user.name", "Test User"]).output().unwrap();
        Command::new("git").current_dir(&repo_path).args(["config", "user.email", "test@example.com"]).output().unwrap();

        fs::write(repo_path.join("file.txt"), "hello").unwrap();
        Command::new("git").current_dir(&repo_path).args(["add", "."]).output().unwrap();
        Command::new("git").current_dir(&repo_path).args(["commit", "-m", "init"]).output().unwrap();

        let checker = GitChecker::new(repo_path.to_str().unwrap());
        let commits = checker.get_new_commits(None, "HEAD", 180).unwrap();
        assert!(!commits.is_empty());
    }
}
