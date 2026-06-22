import os
import json
import pytest
from unittest.mock import patch, MagicMock
from deploy import load_config, ValidationError, run_rust_install, setup_kernel_symlink, ExecutionError, update_settings_toml, generate_env_file


def normalized_kernel_path():
    return os.path.abspath(os.path.expanduser("/kernel/path"))

def test_load_config_valid(tmp_path):
    config_data = {
        "linux_kernel_dir": "/path",
        "rust_install_cmds": ["cmd"],
        "app_config": {"server": {"port": 8080}, "ai": {"openai_key": "key"}}
    }
    config_file = tmp_path / "config.json"
    config_file.write_text(json.dumps(config_data))
    
    config = load_config(str(config_file))
    assert config["linux_kernel_dir"] == "/path"

def test_load_config_missing_file():
    with pytest.raises(ValidationError, match="Configuration file not found"):
        load_config("nonexistent.json")

def test_load_config_invalid_json(tmp_path):
    config_file = tmp_path / "config.json"
    config_file.write_text("invalid json")
    
    with pytest.raises(ValidationError, match="Invalid JSON"):
        load_config(str(config_file))

def test_load_config_missing_keys(tmp_path):
    config_file = tmp_path / "config.json"
    config_file.write_text(json.dumps({}))
    
    with pytest.raises(ValidationError, match="Missing required configuration keys"):
        load_config(str(config_file))

@patch("deploy.load_profile_env")
@patch("deploy.check_rust_version_meets_requirement")
@patch("subprocess.run")
def test_run_rust_install_success(mock_run, mock_check, mock_load_profile):
    mock_check.return_value = False
    mock_run.return_value = MagicMock(returncode=0)
    cmds = ["echo 'installing'", "rustup-init -y"]
    run_rust_install(cmds)
    assert mock_run.call_count == 2

@patch("deploy.check_rust_version_meets_requirement")
@patch("subprocess.run")
def test_run_rust_install_failure(mock_run, mock_check):
    mock_check.return_value = False
    from subprocess import CalledProcessError
    mock_run.side_effect = CalledProcessError(1, "bad_cmd", stderr="error")
    cmds = ["bad_cmd"]
    with pytest.raises(ExecutionError, match="Rust installation failed"):
        run_rust_install(cmds)

@patch("os.symlink")
@patch("os.remove")
@patch("os.path.isdir")
@patch("os.path.exists")
@patch("os.path.islink")
@patch("os.unlink")
@patch("shutil.rmtree")
@patch("os.makedirs")
def test_setup_kernel_symlink(mock_makedirs, mock_rmtree, mock_unlink, mock_islink, mock_exists, mock_isdir, mock_remove, mock_symlink):
    mock_exists.return_value = True
    mock_islink.return_value = False
    mock_isdir.return_value = True
    
    setup_kernel_symlink("target", "/kernel/path")
    
    mock_rmtree.assert_called_once_with(os.path.join("target", "third_party", "linux"))
    mock_symlink.assert_called_once_with(normalized_kernel_path(), os.path.join("target", "third_party", "linux"))

@patch("os.symlink")
@patch("os.remove")
@patch("os.path.isdir")
@patch("os.path.exists")
@patch("os.path.islink")
@patch("os.unlink")
@patch("shutil.rmtree")
@patch("os.makedirs")
def test_setup_kernel_symlink_file(mock_makedirs, mock_rmtree, mock_unlink, mock_islink, mock_exists, mock_isdir, mock_remove, mock_symlink):
    mock_exists.return_value = True
    mock_islink.return_value = False
    mock_isdir.return_value = False
    
    setup_kernel_symlink("target", "/kernel/path")
    
    mock_remove.assert_called_once_with(os.path.join("target", "third_party", "linux"))
    mock_symlink.assert_called_once_with(normalized_kernel_path(), os.path.join("target", "third_party", "linux"))

@patch("os.symlink")
@patch("os.remove")
@patch("os.path.isdir")
@patch("os.path.exists")
@patch("os.path.islink")
@patch("os.unlink")
@patch("shutil.rmtree")
@patch("os.makedirs")
def test_setup_kernel_symlink_islink(mock_makedirs, mock_rmtree, mock_unlink, mock_islink, mock_exists, mock_isdir, mock_remove, mock_symlink):
    mock_exists.return_value = True
    mock_islink.return_value = True
    
    setup_kernel_symlink("target", "/kernel/path")
    
    mock_unlink.assert_called_once_with(os.path.join("target", "third_party", "linux"))
    mock_symlink.assert_called_once_with(normalized_kernel_path(), os.path.join("target", "third_party", "linux"))

def test_update_settings_toml(tmp_path):
    toml_content = '''# Some comment
[server]
port = 3000
'''
    settings_file = tmp_path / "Settings.toml"
    settings_file.write_text(toml_content)
    
    app_config = {"server": {"port": 8080}}
    update_settings_toml(str(tmp_path), app_config)
    
    updated_content = settings_file.read_text()
    assert 'port = 8080' in updated_content
    assert '# Some comment' in updated_content

def test_update_settings_toml_missing_file(tmp_path):
    with pytest.raises(ExecutionError, match="Settings.toml not found"):
        update_settings_toml(str(tmp_path), {})

def test_generate_env_file(tmp_path):
    app_config = {"ai": {"openai_key": "test_key_123"}}
    generate_env_file(str(tmp_path), app_config)
    
    env_file = tmp_path / ".env"
    assert env_file.exists()
    assert "LLM_API_KEY=test_key_123" in env_file.read_text()

def test_update_settings_toml_no_tomlkit(tmp_path, monkeypatch):
    import sys
    monkeypatch.setitem(sys.modules, 'tomlkit', None)
    settings_file = tmp_path / "Settings.toml"
    settings_file.write_text("")
    with pytest.raises(ExecutionError, match="tomlkit is not installed"):
        update_settings_toml(str(tmp_path), {})

def test_update_settings_toml_no_server(tmp_path):
    toml_content = '''# Some comment
'''
    settings_file = tmp_path / "Settings.toml"
    settings_file.write_text(toml_content)
    
    app_config = {"server": {"port": 8080}}
    update_settings_toml(str(tmp_path), app_config)
    
    updated_content = settings_file.read_text()
    assert 'port = 8080' in updated_content
    assert '[server]' in updated_content

def test_generate_env_file_missing_key(tmp_path):
    app_config = {"ai": {}}
    generate_env_file(str(tmp_path), app_config)
    env_file = tmp_path / ".env"
    assert not env_file.exists()

from deploy import main

@patch("deploy.load_config")
def test_main_validation_error(mock_load_config):
    mock_load_config.side_effect = ValidationError("test error")
    exit_code = main("dummy_path")
    assert exit_code == 1

@patch("deploy.load_config")
def test_main_unexpected_error(mock_load_config):
    mock_load_config.side_effect = Exception("unexpected")
    exit_code = main("dummy_path")
    assert exit_code == 1

