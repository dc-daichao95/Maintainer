use anyhow::Result;
use std::process::Command;

/// Executes the synchronization command.
pub struct SyncExecutor {
    server_ip: String,
    server_port: u16,
    max_retries: u32,
    repo_path: String,
}

impl SyncExecutor {
    /// Creates a new `SyncExecutor`.
    pub fn new(server_ip: &str, server_port: u16, max_retries: u32, repo_path: &str) -> Self {
        Self {
            server_ip: server_ip.to_string(),
            server_port,
            max_retries,
            repo_path: repo_path.to_string(),
        }
    }

    /// Executes the sync command with a specific commit hash.
    pub fn execute_sync(&self, commit_hash: &str) -> Result<()> {
        let mut retries = 0;
        let exe_suffix = std::env::consts::EXE_SUFFIX;
        let mut cli_path = format!("target/release/sashiko-cli{}", exe_suffix);
        
        // Fallback to debug build for development
        if !std::path::Path::new(&cli_path).exists() {
            cli_path = format!("target/debug/sashiko-cli{}", exe_suffix);
        }
        
        let cli_path = std::env::var("SASHIKO_CLI_PATH").unwrap_or(cli_path);
        let server_arg = format!("http://{}:{}", self.server_ip, self.server_port);
        let args = vec![
            "--server", &server_arg, 
            "submit", 
            "--repo", &self.repo_path,
            commit_hash
        ];

        loop {
            println!("Running command: {} {}", cli_path, args.join(" "));
            match Command::new(&cli_path).args(&args).output() {
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    if output.status.success() {
                        println!("Sync successful:\n{}", stdout);
                        println!("Submitted commit: {}", commit_hash);
                        return Ok(());
                    } else {
                        eprintln!(
                            "Execution failed (attempt {}/{}): \nStdout: {}\nStderr: {}",
                            retries + 1,
                            self.max_retries,
                            stdout,
                            stderr
                        );
                    }
                }
                Err(e) => {
                    eprintln!(
                        "Failed to spawn process (attempt {}/{}): {}",
                        retries + 1,
                        self.max_retries,
                        e
                    );
                }
            }
            
            retries += 1;
            if retries >= self.max_retries {
                anyhow::bail!("Max retries reached. Execution failed.");
            }
            // Simple backoff for testing
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execute_sync_args() {
        // Just instantiate to ensure it compiles
        let executor = SyncExecutor::new("127.0.0.1", 8080, 3, "/test/repo");
        assert_eq!(executor.server_ip, "127.0.0.1");
        assert_eq!(executor.server_port, 8080);
        assert_eq!(executor.max_retries, 3);
        assert_eq!(executor.repo_path, "/test/repo");
    }
}
