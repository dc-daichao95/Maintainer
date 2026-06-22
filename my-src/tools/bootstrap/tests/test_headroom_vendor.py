import deploy
import json
import pytest
from pathlib import Path


def make_source_tree(path):
    path.mkdir(parents=True)
    (path / "pyproject.toml").write_text("[project]\nname = \"headroom-ai\"\n", encoding="utf-8")
    (path / "headroom").mkdir()
    (path / "crates" / "headroom-py").mkdir(parents=True)
    (path / "crates" / "headroom-core").mkdir(parents=True)


def test_source_vendor_config_defaults_when_disabled():
    config = deploy.parse_headroom_config({})

    assert config.install_mode == "source-vendor"
    assert config.source_dir.endswith("my-src/third_party/headroom/source".replace("/", deploy.os.sep))
    assert config.venv_dir.endswith("my-src/.venv-headroom".replace("/", deploy.os.sep))
    assert config.wheelhouse_dir.endswith("my-src/third_party/headroom/wheelhouse".replace("/", deploy.os.sep))
    assert config.python_executable == "python"


def test_source_vendor_config_accepts_valid_source_tree(tmp_path):
    source_dir = tmp_path / "headroom-source"
    make_source_tree(source_dir)

    config = deploy.parse_headroom_config({
        "headroom": {
            "enabled": True,
            "install_mode": "source-vendor",
            "source_dir": str(source_dir),
        }
    })

    assert config.source_dir == str(source_dir)
    assert config.install_mode == "source-vendor"


def test_source_vendor_config_resolves_relative_paths_from_repo_root(tmp_path, monkeypatch):
    repo_root = tmp_path / "repo"
    source_dir = repo_root / "my-src" / "third_party" / "headroom" / "source"
    make_source_tree(source_dir)
    monkeypatch.setattr(deploy, "get_target_dir", lambda: str(repo_root))

    config = deploy.parse_headroom_config({
        "headroom": {
            "enabled": True,
            "install_mode": "source-vendor",
            "source_dir": "my-src/third_party/headroom/source",
            "venv_dir": "my-src/.venv-headroom",
            "wheelhouse_dir": "my-src/third_party/headroom/wheelhouse",
        }
    })

    assert config.source_dir == str(source_dir)
    assert config.venv_dir == str(repo_root / "my-src" / ".venv-headroom")
    assert config.wheelhouse_dir == str(repo_root / "my-src" / "third_party" / "headroom" / "wheelhouse")


def test_source_vendor_config_rejects_missing_source_tree(tmp_path):
    missing_source = tmp_path / "missing-source"

    with pytest.raises(deploy.ValidationError, match="headroom.source_dir"):
        deploy.parse_headroom_config({
            "headroom": {
                "enabled": True,
                "install_mode": "source-vendor",
                "source_dir": str(missing_source),
            }
        })


def test_headroom_config_rejects_unknown_install_mode():
    with pytest.raises(deploy.ValidationError, match="headroom.install_mode"):
        deploy.parse_headroom_config({
            "headroom": {
                "enabled": True,
                "install_mode": "source_vendor",
            }
        })


class RecordingCommandRunner:
    def __init__(self, outputs=None):
        self.commands = []
        self.outputs = outputs or {}

    def run(self, command, cwd=None):
        self.commands.append((command, cwd))
        return self.outputs.get(tuple(command), "")


class FailingCommandRunner:
    def __init__(self, exception):
        self.exception = exception

    def run(self, command, cwd=None):
        raise self.exception


def test_vendor_manager_reports_missing_python_executable(tmp_path):
    source_dir = tmp_path / "headroom-source"
    make_source_tree(source_dir)
    config = deploy.HeadroomConfig(enabled=True, source_dir=str(source_dir))

    with pytest.raises(deploy.ExecutionError, match="python"):
        deploy.HeadroomVendorManager(
            runner=FailingCommandRunner(FileNotFoundError("python"))
        ).prepare(config)


def test_vendor_manager_reports_failed_wheel_command(tmp_path):
    source_dir = tmp_path / "headroom-source"
    make_source_tree(source_dir)
    config = deploy.HeadroomConfig(enabled=True, source_dir=str(source_dir))

    with pytest.raises(deploy.ExecutionError, match="Headroom source-vendor command failed"):
        deploy.HeadroomVendorManager(
            runner=FailingCommandRunner(deploy.subprocess.CalledProcessError(1, "pip wheel"))
        ).prepare(config)


def test_vendor_manager_rejects_python_below_310(tmp_path):
    source_dir = tmp_path / "headroom-source"
    make_source_tree(source_dir)
    config = deploy.HeadroomConfig(enabled=True, source_dir=str(source_dir))
    runner = RecordingCommandRunner({
        ("python", "--version"): "Python 3.9.18",
    })

    with pytest.raises(deploy.ExecutionError, match="Python 3.10"):
        deploy.HeadroomVendorManager(runner=runner).prepare(config)


def test_vendor_manager_requires_cargo(tmp_path):
    source_dir = tmp_path / "headroom-source"
    make_source_tree(source_dir)
    config = deploy.HeadroomConfig(enabled=True, source_dir=str(source_dir))
    runner = RecordingCommandRunner({
        ("python", "--version"): "Python 3.10.13",
        ("cargo", "--version"): "",
    })

    with pytest.raises(deploy.ExecutionError, match="cargo"):
        deploy.HeadroomVendorManager(runner=runner).prepare(config)


def test_vendor_manager_builds_and_installs_from_source(tmp_path):
    source_dir = tmp_path / "headroom-source"
    make_source_tree(source_dir)
    venv_dir = tmp_path / "venv"
    wheelhouse_dir = tmp_path / "wheelhouse"
    config = deploy.HeadroomConfig(
        enabled=True,
        source_dir=str(source_dir),
        venv_dir=str(venv_dir),
        wheelhouse_dir=str(wheelhouse_dir),
        python_executable="python",
    )
    runner = RecordingCommandRunner({
        ("python", "--version"): "Python 3.10.13",
        ("cargo", "--version"): "cargo 1.92.0",
    })

    status = deploy.HeadroomVendorManager(runner=runner).prepare(config)

    venv_python = deploy.get_venv_python(str(venv_dir))
    assert (["python", "-m", "venv", str(venv_dir)], None) in runner.commands
    assert ([venv_python, "-m", "pip", "install", "--no-index", "--find-links", str(wheelhouse_dir), "maturin"], None) in runner.commands
    assert ([venv_python, "-m", "pip", "wheel", ".", "--no-build-isolation", "--no-index", "--find-links", str(wheelhouse_dir), "--wheel-dir", str(wheelhouse_dir)], str(source_dir)) in runner.commands
    assert ([venv_python, "-m", "pip", "install", "--no-index", "--find-links", str(wheelhouse_dir), "headroom-ai[proxy]"], None) in runner.commands
    assert status.venv_python == venv_python
    assert status.headroom_command == deploy.get_venv_headroom_command(str(venv_dir))


def test_vendor_manager_uses_offline_wheelhouse_for_build_and_install(tmp_path):
    source_dir = tmp_path / "headroom-source"
    make_source_tree(source_dir)
    venv_dir = tmp_path / "venv"
    wheelhouse_dir = tmp_path / "wheelhouse"
    config = deploy.HeadroomConfig(
        enabled=True,
        source_dir=str(source_dir),
        venv_dir=str(venv_dir),
        wheelhouse_dir=str(wheelhouse_dir),
    )
    runner = RecordingCommandRunner({
        ("python", "--version"): "Python 3.10.13",
        ("cargo", "--version"): "cargo 1.92.0",
    })

    deploy.HeadroomVendorManager(runner=runner).prepare(config)

    assert ([deploy.get_venv_python(str(venv_dir)), "-m", "pip", "install", "--no-index", "--find-links", str(wheelhouse_dir), "maturin"], None) in runner.commands
    assert ([deploy.get_venv_python(str(venv_dir)), "-m", "pip", "wheel", ".", "--no-build-isolation", "--no-index", "--find-links", str(wheelhouse_dir), "--wheel-dir", str(wheelhouse_dir)], str(source_dir)) in runner.commands


def test_config_template_declares_source_vendor_defaults():
    template_path = Path(deploy.__file__).with_name("config_template.json")
    template = json.loads(template_path.read_text(encoding="utf-8"))
    headroom = template["headroom"]

    assert headroom["install_mode"] == "source-vendor"
    assert headroom["source_dir"] == "my-src/third_party/headroom/source"
    assert headroom["venv_dir"] == "my-src/.venv-headroom"
    assert headroom["wheelhouse_dir"] == "my-src/third_party/headroom/wheelhouse"
    assert headroom["python_executable"] == "python"
