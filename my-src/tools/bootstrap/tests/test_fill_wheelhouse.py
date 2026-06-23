import fill_wheelhouse
import pytest


class RecordingCommandRunner:
    def __init__(self):
        self.commands = []

    def run(self, command, cwd=None):
        self.commands.append(command)
        return ""


def test_maturin_download_command_uses_index_url():
    cmd = fill_wheelhouse.build_maturin_download_command(
        "python", "/wh", index_url="https://mirror/simple"
    )

    assert cmd == [
        "python", "-m", "pip", "download", "maturin",
        "-d", "/wh", "--index-url", "https://mirror/simple",
    ]


def test_source_deps_download_command_targets_proxy_extra():
    cmd = fill_wheelhouse.build_source_deps_download_command(
        "python", "/src", "/wh", index_url="https://mirror/simple"
    )

    assert cmd == [
        "python", "-m", "pip", "download", "/src[proxy]",
        "-d", "/wh", "--index-url", "https://mirror/simple",
    ]


def test_trusted_host_is_appended_when_given():
    cmd = fill_wheelhouse.build_maturin_download_command(
        "python", "/wh", index_url="https://mirror/simple", trusted_host="mirror"
    )

    assert cmd[-2:] == ["--trusted-host", "mirror"]


def test_index_url_flag_omitted_when_not_provided():
    cmd = fill_wheelhouse.build_maturin_download_command("python", "/wh")

    assert "--index-url" not in cmd
    assert cmd == ["python", "-m", "pip", "download", "maturin", "-d", "/wh"]


def test_fill_creates_wheelhouse_and_runs_both_downloads_in_order(tmp_path):
    wheelhouse = tmp_path / "wh"
    runner = RecordingCommandRunner()

    fill_wheelhouse.fill_wheelhouse(
        source_dir="/src",
        wheelhouse_dir=str(wheelhouse),
        python_executable="python",
        index_url="https://mirror/simple",
        trusted_host=None,
        runner=runner,
    )

    assert wheelhouse.is_dir()
    assert len(runner.commands) == 2
    # maturin (build backend) must be downloaded before resolving the source deps.
    assert runner.commands[0][:5] == ["python", "-m", "pip", "download", "maturin"]
    assert runner.commands[1][4] == "/src[proxy]"


def test_fill_reports_failed_download(tmp_path):
    class FailingRunner:
        def run(self, command, cwd=None):
            raise fill_wheelhouse.subprocess.CalledProcessError(1, command)

    with pytest.raises(fill_wheelhouse.ExecutionError, match="pip download"):
        fill_wheelhouse.fill_wheelhouse(
            source_dir="/src",
            wheelhouse_dir=str(tmp_path / "wh"),
            python_executable="python",
            index_url="https://mirror/simple",
            runner=FailingRunner(),
        )
