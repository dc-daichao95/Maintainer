import deploy
import pytest


def make_source_tree(path):
    path.mkdir(parents=True)
    (path / "pyproject.toml").write_text("[project]\nname = \"headroom-ai\"\n", encoding="utf-8")
    (path / "headroom").mkdir()
    (path / "crates" / "headroom-py").mkdir(parents=True)
    (path / "crates" / "headroom-core").mkdir(parents=True)


class RecordingCommandRunner:
    def __init__(self, outputs=None):
        self.commands = []
        self.outputs = outputs or {}

    def run(self, command, cwd=None):
        self.commands.append((command, cwd))
        return self.outputs.get(tuple(command), "")


def test_source_local_config_accepts_valid_source_tree(tmp_path):
    source_dir = tmp_path / "headroom-source"
    make_source_tree(source_dir)

    config = deploy.parse_headroom_config({
        "headroom": {
            "enabled": True,
            "install_mode": "source-local",
            "source_dir": str(source_dir),
        }
    })

    assert config.install_mode == "source-local"
    assert config.source_dir == str(source_dir)


def test_source_local_config_rejects_missing_source_tree(tmp_path):
    missing_source = tmp_path / "missing-source"

    with pytest.raises(deploy.ValidationError, match="headroom.source_dir"):
        deploy.parse_headroom_config({
            "headroom": {
                "enabled": True,
                "install_mode": "source-local",
                "source_dir": str(missing_source),
            }
        })


def test_local_installer_installs_from_source_without_wheelhouse(tmp_path, monkeypatch):
    monkeypatch.setattr(deploy, "get_active_env_prefix", lambda: None)
    source_dir = tmp_path / "headroom-source"
    make_source_tree(source_dir)
    venv_dir = tmp_path / "venv"
    config = deploy.HeadroomConfig(
        enabled=True,
        install_mode="source-local",
        source_dir=str(source_dir),
        venv_dir=str(venv_dir),
        python_executable="python",
    )
    runner = RecordingCommandRunner({
        ("python", "--version"): "Python 3.10.13",
        ("cargo", "--version"): "cargo 1.92.0",
    })

    status = deploy.HeadroomLocalInstaller(runner=runner).prepare(config)

    venv_python = deploy.get_venv_python(str(venv_dir))
    assert (["python", "-m", "venv", str(venv_dir)], None) in runner.commands
    assert ([venv_python, "-m", "pip", "install", f"{source_dir}[proxy]"], None) in runner.commands
    # 免 wheelhouse：绝不能出现离线安装标志。
    for command, _ in runner.commands:
        assert "--no-index" not in command
    assert status.venv_python == venv_python
    assert status.headroom_command == deploy.get_venv_headroom_command(str(venv_dir))


def test_local_installer_installs_into_active_conda_env_without_venv(tmp_path, monkeypatch):
    env_prefix = str(tmp_path / "conda-env")
    monkeypatch.setattr(deploy, "get_active_env_prefix", lambda: env_prefix)
    source_dir = tmp_path / "headroom-source"
    make_source_tree(source_dir)
    config = deploy.HeadroomConfig(
        enabled=True,
        install_mode="source-local",
        source_dir=str(source_dir),
        venv_dir=str(tmp_path / "venv"),
        python_executable="python",
    )
    runner = RecordingCommandRunner({
        ("python", "--version"): "Python 3.10.13",
        ("cargo", "--version"): "cargo 1.92.0",
    })

    status = deploy.HeadroomLocalInstaller(runner=runner).prepare(config)

    venv_commands = [
        cmd for cmd, _ in runner.commands
        if len(cmd) >= 3 and cmd[1] == "-m" and cmd[2] == "venv"
    ]
    assert venv_commands == []
    assert (["python", "-m", "pip", "install", f"{source_dir}[proxy]"], None) in runner.commands
    assert status.venv_python == "python"
    assert status.headroom_command == deploy.get_env_headroom_command(env_prefix)


def test_local_installer_rejects_python_below_310(tmp_path):
    source_dir = tmp_path / "headroom-source"
    make_source_tree(source_dir)
    config = deploy.HeadroomConfig(enabled=True, install_mode="source-local", source_dir=str(source_dir))
    runner = RecordingCommandRunner({
        ("python", "--version"): "Python 3.9.18",
    })

    with pytest.raises(deploy.ExecutionError, match="Python 3.10"):
        deploy.HeadroomLocalInstaller(runner=runner).prepare(config)


def test_local_installer_requires_cargo(tmp_path):
    source_dir = tmp_path / "headroom-source"
    make_source_tree(source_dir)
    config = deploy.HeadroomConfig(enabled=True, install_mode="source-local", source_dir=str(source_dir))
    runner = RecordingCommandRunner({
        ("python", "--version"): "Python 3.10.13",
        ("cargo", "--version"): "",
    })

    with pytest.raises(deploy.ExecutionError, match="cargo"):
        deploy.HeadroomLocalInstaller(runner=runner).prepare(config)
