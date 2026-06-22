import os
import json
import pytest
import subprocess
from unittest.mock import patch
from deploy import main

def test_e2e_deploy(tmp_path):
    # Setup mock kernel dir
    kernel_dir = tmp_path / "mock_kernel"
    kernel_dir.mkdir()
    (kernel_dir / "Makefile").write_text("VERSION = 5")
    
    # Setup mock local git repo
    repo_dir = tmp_path / "mock_repo"
    repo_dir.mkdir()
    
    subprocess.run(["git", "init"], cwd=str(repo_dir), check=True)
    subprocess.run(["git", "config", "user.name", "Test"], cwd=str(repo_dir), check=True)
    subprocess.run(["git", "config", "user.email", "test@test.com"], cwd=str(repo_dir), check=True)
    
    (repo_dir / "Settings.toml").write_text("[server]\nport = 3000\n")
    subprocess.run(["git", "add", "Settings.toml"], cwd=str(repo_dir), check=True)
    subprocess.run(["git", "commit", "-m", "init"], cwd=str(repo_dir), check=True)
    
    # We will use the mock repo as the target dir
    target_dir = repo_dir
    
    # Setup config
    config_file = tmp_path / "config.json"
    config_data = {
        "linux_kernel_dir": str(kernel_dir),
        "rust_install_cmds": ["echo 'mock rust install'"],
        "app_config": {
            "server": {
                "port": 9090
            },
            "ai": {
                "openai_key": "test_openai_key"
            }
        }
    }
    config_file.write_text(json.dumps(config_data))
    
    # Run main deployment script
    with patch('deploy.build_project') as mock_build, \
         patch('deploy.get_target_dir', return_value=str(target_dir)), \
         patch('deploy.get_monitor_dir', return_value=str(repo_dir / "my-src" / "tools" / "repo_monitor")):
        exit_code = main(str(config_file))
        mock_build.assert_called_once_with(str(target_dir))
    
    # Verify exit code
    assert exit_code == 0
    
    # Verify target dir
    assert target_dir.exists()
    assert (target_dir / "Settings.toml").exists()
    
    # Verify Settings.toml was updated
    settings_content = (target_dir / "Settings.toml").read_text()
    assert 'port = 9090' in settings_content
    
    # Verify kernel symlink
    symlink_path = target_dir / "third_party" / "linux"
    assert symlink_path.exists()
    assert symlink_path.is_symlink() or symlink_path.is_dir() # on windows symlinks to dirs might act differently or might not be possible without admin rights if not in developer mode, so os.symlink might fallback or we just check it exists
    
    # Verify .env file
    env_file = target_dir / ".env"
    assert env_file.exists()
    assert "LLM_API_KEY=test_openai_key" in env_file.read_text()

def test_e2e_missing_config_file():
    exit_code = main("nonexistent_config.json")
    assert exit_code == 1

def test_e2e_invalid_json(tmp_path):
    config_file = tmp_path / "invalid_config.json"
    config_file.write_text("{ invalid json")
    
    exit_code = main(str(config_file))
    assert exit_code == 1

def test_e2e_missing_keys(tmp_path):
    config_file = tmp_path / "missing_keys.json"
    config_file.write_text(json.dumps({}))
    
    exit_code = main(str(config_file))
    assert exit_code == 1

def test_e2e_rust_install_failure(tmp_path):
    config_file = tmp_path / "rust_fail.json"
    config_data = {
        "linux_kernel_dir": "",
        "rust_install_cmds": ["exit 1"],
        "app_config": {}
    }
    config_file.write_text(json.dumps(config_data))
    
    with patch('deploy.get_target_dir', return_value=str(tmp_path)), \
         patch('deploy.get_monitor_dir', return_value=str(tmp_path / "monitor")):
        exit_code = main(str(config_file))
    assert exit_code == 1

def test_e2e_missing_settings_toml(tmp_path):
    # Setup mock local git repo WITHOUT Settings.toml
    repo_dir = tmp_path / "mock_repo"
    repo_dir.mkdir()
    
    subprocess.run(["git", "init"], cwd=str(repo_dir), check=True)
    subprocess.run(["git", "config", "user.name", "Test"], cwd=str(repo_dir), check=True)
    subprocess.run(["git", "config", "user.email", "test@test.com"], cwd=str(repo_dir), check=True)
    
    (repo_dir / "README.md").write_text("hello")
    subprocess.run(["git", "add", "README.md"], cwd=str(repo_dir), check=True)
    subprocess.run(["git", "commit", "-m", "init"], cwd=str(repo_dir), check=True)
    
    target_dir = repo_dir
    
    config_file = tmp_path / "config.json"
    config_data = {
        "linux_kernel_dir": str(tmp_path),
        "rust_install_cmds": ["echo 'mock rust install'"],
        "app_config": {}
    }
    config_file.write_text(json.dumps(config_data))
    
    with patch('deploy.get_target_dir', return_value=str(target_dir)), \
         patch('deploy.get_monitor_dir', return_value=str(repo_dir / "my-src" / "tools" / "repo_monitor")):
        exit_code = main(str(config_file))
    assert exit_code == 1
