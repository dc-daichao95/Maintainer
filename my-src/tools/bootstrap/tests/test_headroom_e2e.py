import json
import threading
from http.server import BaseHTTPRequestHandler, HTTPServer
from unittest.mock import patch

import deploy


class ReadyHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path in ("/readyz", "/health", "/stats"):
            self.send_response(200)
            self.end_headers()
            self.wfile.write(b"ok")
            return
        self.send_response(404)
        self.end_headers()

    def log_message(self, format, *args):
        return


class NotReadyHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(503)
        self.end_headers()
        self.wfile.write(b"not ready")

    def log_message(self, format, *args):
        return


class NoopRunner:
    def __init__(self):
        self.commands = []

    def start(self, command):
        self.commands.append(command)
        return None


class FakeVendorManager:
    def __init__(self):
        self.prepared_configs = []

    def prepare(self, config):
        self.prepared_configs.append(config)
        return deploy.HeadroomBuildStatus(
            source_version="0.26.0",
            wheel_path=config.wheelhouse_dir,
            venv_python=deploy.get_venv_python(config.venv_dir),
            headroom_command="vendor-headroom",
        )


def start_ready_server():
    server = HTTPServer(("127.0.0.1", 0), ReadyHandler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    return server


def start_not_ready_server():
    server = HTTPServer(("127.0.0.1", 0), NotReadyHandler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    return server


def test_bootstrap_headroom_flow_with_fake_ready_server(tmp_path):
    server = start_ready_server()
    try:
        target_dir = tmp_path / "target"
        target_dir.mkdir()
        settings_file = target_dir / "Settings.toml"
        settings_file.write_text("[server]\nport = 3000\n", encoding="utf-8")

        config_file = tmp_path / "config.json"
        config_file.write_text(
            json.dumps({
                "linux_kernel_dir": str(tmp_path / "kernel"),
                "rust_install_cmds": [],
                "app_config": {
                    "server": {"port": 9090},
                    "ai": {
                        "model": "gpt-4o",
                        "api_timeout_secs": 300,
                        "streaming": True,
                        "stream_idle_timeout_secs": 240,
                    },
                },
                "headroom": {
                    "enabled": True,
                    "install_mode": "external-cli",
                    "host": "127.0.0.1",
                    "port": server.server_port,
                    "mode": "token",
                    "backend": "openrouter",
                    "telemetry": False,
                    "startup_timeout_secs": 1,
                },
            }),
            encoding="utf-8",
        )

        with patch("deploy.install_dependencies"), \
             patch("deploy.run_rust_install"), \
             patch("deploy.setup_kernel_symlink"), \
             patch("deploy.build_project"), \
             patch("deploy.get_target_dir", return_value=str(target_dir)):
            exit_code = deploy.main(str(config_file))

        updated_content = settings_file.read_text(encoding="utf-8")
        assert exit_code == 0
        assert f'base_url = "http://127.0.0.1:{server.server_port}/v1"' in updated_content
        assert 'provider = "openai-compatible"' in updated_content
        assert "streaming = true" in updated_content
    finally:
        server.shutdown()
        server.server_close()


def test_bootstrap_headroom_flow_stops_when_not_ready(tmp_path, capsys):
    server = start_not_ready_server()
    try:
        target_dir = tmp_path / "target"
        target_dir.mkdir()
        settings_file = target_dir / "Settings.toml"
        settings_file.write_text("[server]\nport = 3000\n", encoding="utf-8")

        config_file = tmp_path / "config.json"
        config_file.write_text(
            json.dumps({
                "linux_kernel_dir": str(tmp_path / "kernel"),
                "rust_install_cmds": [],
                "app_config": {"server": {"port": 9090}, "ai": {"model": "gpt-4o"}},
                "headroom": {
                    "enabled": True,
                    "install_mode": "external-cli",
                    "host": "127.0.0.1",
                    "port": server.server_port,
                    "mode": "token",
                    "backend": "openrouter",
                    "telemetry": False,
                    "startup_timeout_secs": 1,
                },
            }),
            encoding="utf-8",
        )

        with patch("deploy.install_dependencies"), \
             patch("deploy.run_rust_install"), \
             patch("deploy.setup_kernel_symlink"), \
             patch("deploy.build_project"), \
             patch("deploy.HeadroomProcessRunner", return_value=NoopRunner()), \
             patch("deploy.get_target_dir", return_value=str(target_dir)):
            exit_code = deploy.main(str(config_file))

        captured = capsys.readouterr()
        updated_content = settings_file.read_text(encoding="utf-8")
        assert exit_code == 1
        assert "base_url" not in updated_content
        assert f"http://127.0.0.1:{server.server_port}/readyz" in captured.err
        assert "Check Headroom logs" in captured.err
    finally:
        server.shutdown()
        server.server_close()


def test_bootstrap_source_vendor_prepares_before_starting_proxy(tmp_path):
    server = start_not_ready_server()
    try:
        target_dir = tmp_path / "target"
        target_dir.mkdir()
        settings_file = target_dir / "Settings.toml"
        settings_file.write_text("[server]\nport = 3000\n", encoding="utf-8")
        source_dir = tmp_path / "headroom-source"
        source_dir.mkdir()
        (source_dir / "pyproject.toml").write_text("[project]\nname = \"headroom-ai\"\n", encoding="utf-8")
        (source_dir / "headroom").mkdir()
        (source_dir / "crates" / "headroom-py").mkdir(parents=True)
        (source_dir / "crates" / "headroom-core").mkdir(parents=True)
        runner = NoopRunner()
        vendor_manager = FakeVendorManager()

        config_file = tmp_path / "config.json"
        config_file.write_text(
            json.dumps({
                "linux_kernel_dir": str(tmp_path / "kernel"),
                "rust_install_cmds": [],
                "app_config": {"server": {"port": 9090}, "ai": {"model": "gpt-4o"}},
                "headroom": {
                    "enabled": True,
                    "install_mode": "source-vendor",
                    "source_dir": str(source_dir),
                    "venv_dir": str(tmp_path / "venv"),
                    "wheelhouse_dir": str(tmp_path / "wheelhouse"),
                    "host": "127.0.0.1",
                    "port": server.server_port,
                    "mode": "token",
                    "backend": "openrouter",
                    "telemetry": False,
                    "startup_timeout_secs": 1,
                },
            }),
            encoding="utf-8",
        )

        with patch("deploy.install_dependencies"), \
             patch("deploy.run_rust_install"), \
             patch("deploy.setup_kernel_symlink"), \
             patch("deploy.build_project"), \
             patch("deploy.HeadroomVendorManager", return_value=vendor_manager), \
             patch("deploy.HeadroomProcessRunner", return_value=runner), \
             patch("deploy.get_target_dir", return_value=str(target_dir)):
            exit_code = deploy.main(str(config_file))

        assert exit_code == 1
        assert len(vendor_manager.prepared_configs) == 1
        assert runner.commands[0][0] == "vendor-headroom"
        assert "base_url" not in settings_file.read_text(encoding="utf-8")
    finally:
        server.shutdown()
        server.server_close()
