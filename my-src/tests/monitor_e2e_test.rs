use std::fs;
use std::path::PathBuf;
use std::process::{Command, Child};
use std::time::Duration;
use tempfile::TempDir;

struct TestEnv {
    pub dir: TempDir,
    pub repo_path: PathBuf,
    pub daemon_process: Option<Child>,
    pub sync_cli_path: PathBuf,
    pub config_path: PathBuf,
    pub settings_path: PathBuf,
    pub state_path: PathBuf,
}

impl TestEnv {
    fn new() -> Self {
        let dir = TempDir::new().unwrap();
        let repo_path = dir.path().join("repo");
        fs::create_dir(&repo_path).unwrap();

        // 1. Init git repo
        Command::new("git").current_dir(&repo_path).args(["init"]).output().unwrap();
        Command::new("git").current_dir(&repo_path).args(["config", "user.name", "Test User"]).output().unwrap();
        Command::new("git").current_dir(&repo_path).args(["config", "user.email", "test@example.com"]).output().unwrap();

        // Initial commit
        fs::write(repo_path.join("init.txt"), "init").unwrap();
        Command::new("git").current_dir(&repo_path).args(["add", "."]).output().unwrap();
        Command::new("git").current_dir(&repo_path).args(["commit", "-m", "init"]).output().unwrap();
            
        // Create a remote repo and set upstream
        let remote_path = dir.path().join("remote_repo");
        fs::create_dir(&remote_path).unwrap();
        Command::new("git").args(["init", "--bare"]).current_dir(&remote_path).output().unwrap();
        let remote_url = remote_path.to_str().unwrap().replace("\\", "/");
        Command::new("git").args(["remote", "add", "origin", &remote_url]).current_dir(&repo_path).output().unwrap();
        Command::new("git").args(["branch", "-M", "main"]).current_dir(&repo_path).output().unwrap();
        Command::new("git").args(["push", "-u", "origin", "main"]).current_dir(&repo_path).output().unwrap();

        // Create a fake sync_cli that logs arguments and can be instructed to fail
        let sync_cli_path = if cfg!(windows) {
            let path = dir.path().join("sync_cli.bat");
            let script = format!(
                "@echo off\r\n\
                echo %* >> {}\r\n\
                if exist {} (\r\n\
                    del {}\r\n\
                    exit /b 1\r\n\
                )\r\n\
                exit /b 0",
                dir.path().join("sync_cli_args.txt").display(),
                dir.path().join("fail_next.txt").display(),
                dir.path().join("fail_next.txt").display()
            );
            fs::write(&path, script).unwrap();
            path
        } else {
            let path = dir.path().join("sync_cli.sh");
            let script = format!(
                "#!/bin/sh\n\
                echo \"$@\" >> {}\n\
                if [ -f {} ]; then\n\
                    rm {}\n\
                    exit 1\n\
                fi\n\
                exit 0",
                dir.path().join("sync_cli_args.txt").display(),
                dir.path().join("fail_next.txt").display(),
                dir.path().join("fail_next.txt").display()
            );
            fs::write(&path, script).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&path).unwrap().permissions();
                perms.set_mode(0o755);
                fs::set_permissions(&path, perms).unwrap();
            }
            path
        };

        // 2. Setup config
        let config_path = dir.path().join("monitor_config.json");
        fs::write(&config_path, format!(r#"{{
            "pull_interval_sec": 1,
            "max_retries": 3,
            "max_history_days": 180,
            "server_ip": "127.0.0.1",
            "server_port": 8080,
            "branches": ["main"]
        }}"#)).unwrap();
        
        let settings_path = dir.path().join("Settings.toml");
        fs::write(&settings_path, format!(r#"
        [git]
        repository_path = "{}"
        "#, repo_path.to_str().unwrap().replace("\\", "/"))).unwrap();

        let state_path = dir.path().join(".monitor_state.json");

        Self {
            dir,
            repo_path,
            daemon_process: None,
            sync_cli_path,
            config_path,
            settings_path,
            state_path,
        }
    }

    fn start_daemon(&mut self) {
        let cargo_toml_dir = std::env::current_dir().unwrap().parent().unwrap().to_path_buf();
        let daemon_process = Command::new("cargo")
            .current_dir(&cargo_toml_dir)
            .args(["run", "-p", "repo_monitor", "--"])
            .env("SASHIKO_CLI_PATH", &self.sync_cli_path)
            .arg("--config")
            .arg(&self.config_path)
            .arg("--settings")
            .arg(&self.settings_path)
            .arg("--state")
            .arg(&self.state_path)
            .spawn()
            .unwrap();
        self.daemon_process = Some(daemon_process);
    }

    fn stop_daemon(&mut self) {
        if let Some(mut child) = self.daemon_process.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    fn commit_file(&self, filename: &str, content: &str) -> String {
        fs::write(self.repo_path.join(filename), content).unwrap();
        Command::new("git")
            .current_dir(&self.repo_path)
            .args(["add", filename])
            .output()
            .unwrap();
        Command::new("git")
            .current_dir(&self.repo_path)
            .args(["commit", "-m", "add file"])
            .output()
            .unwrap();
        Command::new("git")
            .current_dir(&self.repo_path)
            .args(["push"])
            .output()
            .unwrap();

        let output = Command::new("git")
            .current_dir(&self.repo_path)
            .args(["rev-parse", "HEAD"])
            .output()
            .unwrap();
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    fn get_args_log(&self) -> String {
        let path = self.dir.path().join("sync_cli_args.txt");
        if path.exists() {
            fs::read_to_string(path).unwrap()
        } else {
            String::new()
        }
    }

    fn clear_args_log(&self) {
        let path = self.dir.path().join("sync_cli_args.txt");
        if path.exists() {
            fs::remove_file(path).unwrap();
        }
    }

    fn set_fail_next(&self) {
        fs::write(self.dir.path().join("fail_next.txt"), "fail").unwrap();
    }
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        self.stop_daemon();
    }
}

#[test]
fn test_pre_flight_check_failure() {
    let mut env = TestEnv::new();
    
    // Create a branch without upstream
    Command::new("git").current_dir(&env.repo_path).args(["branch", "no_upstream"]).output().unwrap();
    
    // Update config to include this branch
    fs::write(&env.config_path, format!(r#"{{
        "pull_interval_sec": 1,
        "max_retries": 3,
        "max_history_days": 180,
        "server_ip": "127.0.0.1",
        "server_port": 8080,
        "branches": ["main", "no_upstream"]
    }}"#)).unwrap();

    let cargo_toml_dir = std::env::current_dir().unwrap().parent().unwrap().to_path_buf();
        let output = Command::new("cargo")
            .current_dir(&cargo_toml_dir)
            .args(["run", "-p", "repo_monitor", "--"])
            .env("SASHIKO_CLI_PATH", &env.sync_cli_path)
        .arg("--config")
        .arg(&env.config_path)
        .arg("--settings")
        .arg(&env.settings_path)
        .arg("--state")
        .arg(&env.state_path)
        .output()
        .unwrap();

    // Should fail fast
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Pre-flight check failed for branch 'no_upstream'"));
}

#[test]
fn test_successful_detection_and_sync() {
    let mut env = TestEnv::new();
    env.start_daemon();
    
    // Wait for initial sync to complete
    for _ in 0..10 {
        let args_log = env.get_args_log();
        if args_log.contains("submit --type range") {
            break;
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    
    env.clear_args_log();
    
    let hash1 = env.commit_file("test1.txt", "content1");
    
    // Wait for second sync to complete
    for _ in 0..10 {
        let args_log = env.get_args_log();
        if args_log.contains(&hash1) {
            break;
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    
    let args_log = env.get_args_log();
    println!("args_log: {}", args_log);
    assert!(args_log.contains("--server http://127.0.0.1:8080 submit --repo"));
    assert!(args_log.contains(&hash1));
    
    let state_content = fs::read_to_string(&env.state_path).unwrap();
    assert!(state_content.contains(&hash1));
}

#[test]
fn test_retry_logic() {
    let mut env = TestEnv::new();
    env.start_daemon();
    
    // Wait for initial sync to complete
    for _ in 0..10 {
        let args_log = env.get_args_log();
        if args_log.contains("submit --type range") {
            break;
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    
    env.clear_args_log();
    
    // Make the next sync fail
    env.set_fail_next();
    
    let hash1 = env.commit_file("test_retry.txt", "retry content");
    
    // Wait enough time for it to fail once, and then retry and succeed
    for _ in 0..10 {
        let args_log = env.get_args_log();
        let count = args_log.lines().filter(|l| l.contains(&hash1)).count();
        if count >= 2 {
            break;
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    
    let args_log = env.get_args_log();
    // It should have been called twice
    let count = args_log.lines().filter(|l| l.contains(&hash1)).count();
    assert_eq!(count, 2, "Should have retried once");
    
    let state_content = fs::read_to_string(&env.state_path).unwrap();
    assert!(state_content.contains(&hash1));
}

#[test]
fn test_state_persistence_across_restarts() {
    let mut env = TestEnv::new();
    env.start_daemon();
    
    // Wait for initial sync to complete
    for _ in 0..10 {
        let args_log = env.get_args_log();
        if args_log.contains("submit --type range") {
            break;
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    
    let hash1 = env.commit_file("test_persist.txt", "persist");
    
    // Wait for second sync to complete
    for _ in 0..10 {
        let state_content = fs::read_to_string(&env.state_path).unwrap_or_default();
        if state_content.contains(&hash1) {
            break;
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    
    let state_content = fs::read_to_string(&env.state_path).unwrap();
    assert!(state_content.contains(&hash1));
    
    // Stop daemon
    env.stop_daemon();
    env.clear_args_log();
    
    // Start daemon again
    env.start_daemon();
    std::thread::sleep(Duration::from_secs(3));
    
    // Should not have synced again
    let args_log = env.get_args_log();
    assert!(!args_log.contains(&hash1), "Should not re-sync after restart");
}
