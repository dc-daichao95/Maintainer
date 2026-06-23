import json
import os
import sys
import subprocess
import shutil
import re
import time
from dataclasses import dataclass

class ValidationError(Exception):
    pass

class ExecutionError(Exception):
    pass

@dataclass
class HeadroomConfig:
    enabled: bool = False
    install_mode: str = "source-vendor"
    source_dir: str = "my-src/third_party/headroom/source"
    venv_dir: str = "my-src/.venv-headroom"
    wheelhouse_dir: str = "my-src/third_party/headroom/wheelhouse"
    python_executable: str = "python"
    host: str = "127.0.0.1"
    port: int = 8787
    mode: str = "token"
    backend: str = "openrouter"
    telemetry: bool = False
    startup_timeout_secs: int = 20

@dataclass
class HeadroomStatus:
    running: bool
    ready: bool
    base_url: str
    stats_url: str

@dataclass
class HeadroomBuildStatus:
    source_version: str
    wheel_path: str
    venv_python: str
    headroom_command: str

def get_script_dir():
    return os.path.dirname(os.path.abspath(__file__))

def get_target_dir():
    return os.path.abspath(os.path.join(get_script_dir(), "..", "..", ".."))

def get_monitor_dir():
    return os.path.abspath(os.path.join(get_script_dir(), "..", "repo_monitor"))

def load_config(file_path):
    if not os.path.exists(file_path):
        raise ValidationError(f"Configuration file not found: {file_path}")
    
    try:
        with open(file_path, "r", encoding="utf-8") as f:
            config = json.load(f)
    except json.JSONDecodeError as e:
        raise ValidationError(f"Invalid JSON format in {file_path}: {e}")
    
    required_keys = ["linux_kernel_dir", "rust_install_cmds", "app_config"]
    missing_keys = [k for k in required_keys if k not in config]
    if missing_keys:
        raise ValidationError(f"Missing required configuration keys: {', '.join(missing_keys)}")
        
    # Optional validation for monitor
    if "monitor" in config:
        if "branches" not in config["monitor"]:
            raise ValidationError("Monitor configuration must contain 'branches' array")
        
    return config

def parse_headroom_config(config):
    headroom = config.get("headroom", {})
    if not isinstance(headroom, dict):
        raise ValidationError("headroom must be an object")
    enabled = headroom.get("enabled", False)
    telemetry = headroom.get("telemetry", False)
    install_mode = headroom.get("install_mode", "source-vendor")
    source_dir = headroom.get("source_dir", "my-src/third_party/headroom/source")
    venv_dir = headroom.get("venv_dir", "my-src/.venv-headroom")
    wheelhouse_dir = headroom.get("wheelhouse_dir", "my-src/third_party/headroom/wheelhouse")
    python_executable = headroom.get("python_executable", "python")
    host = headroom.get("host", "127.0.0.1")
    backend = headroom.get("backend", "openrouter")
    mode = headroom.get("mode", "token")
    port = headroom.get("port", 8787)
    startup_timeout_secs = headroom.get("startup_timeout_secs", 20)

    if not isinstance(enabled, bool):
        raise ValidationError("headroom.enabled must be a boolean")
    if not isinstance(telemetry, bool):
        raise ValidationError("headroom.telemetry must be a boolean")
    if not isinstance(port, int) or isinstance(port, bool) or port < 1 or port > 65535:
        raise ValidationError("headroom.port must be an integer between 1 and 65535")
    if not isinstance(startup_timeout_secs, int) or isinstance(startup_timeout_secs, bool):
        raise ValidationError("headroom.startup_timeout_secs must be an integer")
    if install_mode not in ("source-vendor", "source-local", "external-cli"):
        raise ValidationError(
            "headroom.install_mode must be one of 'source-vendor', 'source-local', or 'external-cli'"
        )
    for field_name, value in [
        ("install_mode", install_mode),
        ("source_dir", source_dir),
        ("venv_dir", venv_dir),
        ("wheelhouse_dir", wheelhouse_dir),
        ("python_executable", python_executable),
    ]:
        if not isinstance(value, str) or not value.strip():
            raise ValidationError(f"headroom.{field_name} must be a non-empty string")
    source_dir = resolve_repo_path(source_dir)
    venv_dir = resolve_repo_path(venv_dir)
    wheelhouse_dir = resolve_repo_path(wheelhouse_dir)
    if enabled and (not isinstance(host, str) or not host.strip()):
        raise ValidationError("headroom.host must be set when headroom is enabled")
    if enabled and (not isinstance(backend, str) or not backend.strip()):
        raise ValidationError("headroom.backend must be set when headroom is enabled")
    if enabled and (not isinstance(mode, str) or not mode.strip()):
        raise ValidationError("headroom.mode must be set when headroom is enabled")
    if enabled and startup_timeout_secs <= 0:
        raise ValidationError("headroom.startup_timeout_secs must be positive when headroom is enabled")
    if enabled and install_mode in ("source-vendor", "source-local"):
        validate_headroom_source_dir(source_dir)

    return HeadroomConfig(
        enabled=enabled,
        install_mode=install_mode.strip(),
        source_dir=source_dir.strip(),
        venv_dir=venv_dir.strip(),
        wheelhouse_dir=wheelhouse_dir.strip(),
        python_executable=python_executable.strip(),
        host=host.strip() if isinstance(host, str) else host,
        port=port,
        mode=mode.strip() if isinstance(mode, str) else mode,
        backend=backend.strip() if isinstance(backend, str) else backend,
        telemetry=telemetry,
        startup_timeout_secs=startup_timeout_secs,
    )

def resolve_repo_path(path):
    if os.path.isabs(path):
        return os.path.abspath(path)
    return os.path.abspath(os.path.join(get_target_dir(), path))

def validate_headroom_source_dir(source_dir):
    required_paths = [
        "pyproject.toml",
        "headroom",
        os.path.join("crates", "headroom-py"),
        os.path.join("crates", "headroom-core"),
    ]
    missing = [
        path for path in required_paths
        if not os.path.exists(os.path.join(source_dir, path))
    ]
    if missing:
        raise ValidationError(
            "headroom.source_dir must point to a complete Headroom source tree; "
            f"missing: {', '.join(missing)}"
        )

def get_venv_python(venv_dir):
    if os.name == "nt":
        return os.path.join(venv_dir, "Scripts", "python.exe")
    return os.path.join(venv_dir, "bin", "python")

def get_venv_headroom_command(venv_dir):
    if os.name == "nt":
        return os.path.join(venv_dir, "Scripts", "headroom.exe")
    return os.path.join(venv_dir, "bin", "headroom")

def get_active_env_prefix():
    """Return the active conda environment prefix, or None when not inside one.

    Deployments on Linux often already run inside a conda environment. In that
    case creating a separate venv is redundant and frequently breaks (e.g.
    conda `ensurepip` failures), so we install Headroom into the active env.
    """
    prefix = os.environ.get("CONDA_PREFIX")
    if prefix and prefix.strip():
        return prefix
    return None

def get_env_headroom_command(env_prefix):
    if os.name == "nt":
        return os.path.join(env_prefix, "Scripts", "headroom.exe")
    return os.path.join(env_prefix, "bin", "headroom")

def is_python_310_or_newer(version_output):
    match = re.search(r"Python\s+(\d+)\.(\d+)", version_output)
    if not match:
        return False
    return (int(match.group(1)), int(match.group(2))) >= (3, 10)

def read_headroom_source_version(source_dir):
    version_file = os.path.join(source_dir, "headroom", "_version.py")
    if not os.path.exists(version_file):
        return "unknown"
    with open(version_file, "r", encoding="utf-8") as f:
        content = f.read()
    match = re.search(r"__version__\s*=\s*['\"]([^'\"]+)['\"]", content)
    return match.group(1) if match else "unknown"

def resolve_build_python_and_command(config):
    """Resolve the Python interpreter to build with and the resulting headroom command.

    Reuses an active conda environment when present (avoids nesting a redundant
    venv); otherwise creates and targets the configured venv. Returns a tuple of
    (build_python, headroom_command, needs_venv_creation).
    """
    env_prefix = get_active_env_prefix()
    if env_prefix:
        return config.python_executable, get_env_headroom_command(env_prefix), False
    return (
        get_venv_python(config.venv_dir),
        get_venv_headroom_command(config.venv_dir),
        True,
    )

class SubprocessCommandRunner:
    def run(self, command, cwd=None):
        result = subprocess.run(
            command,
            cwd=cwd,
            capture_output=True,
            text=True,
            check=True,
        )
        return result.stdout or result.stderr

class HeadroomVendorManager:
    def __init__(self, runner=None):
        self.runner = runner or SubprocessCommandRunner()

    def prepare(self, config):
        validate_headroom_source_dir(config.source_dir)
        python_version = self._run([config.python_executable, "--version"]).strip()
        if not self._is_python_310_or_newer(python_version):
            raise ExecutionError(
                f"Headroom source-vendor mode requires Python 3.10 or newer; got {python_version}"
            )
        cargo_cmd = get_cargo_path()
        cargo_version = self._run([cargo_cmd, "--version"]).strip()
        if not cargo_version:
            raise ExecutionError("Headroom source-vendor mode requires cargo on PATH")

        os.makedirs(config.wheelhouse_dir, exist_ok=True)

        # Reuse the active conda environment instead of nesting a redundant venv.
        env_prefix = get_active_env_prefix()
        if env_prefix:
            build_python = config.python_executable
            headroom_command = get_env_headroom_command(env_prefix)
        else:
            build_python = get_venv_python(config.venv_dir)
            self._run([config.python_executable, "-m", "venv", config.venv_dir])
            headroom_command = get_venv_headroom_command(config.venv_dir)

        self._run([
            build_python,
            "-m",
            "pip",
            "install",
            "--no-index",
            "--find-links",
            config.wheelhouse_dir,
            "maturin",
        ])
        self._run(
            [
                build_python,
                "-m",
                "pip",
                "wheel",
                ".",
                "--no-build-isolation",
                "--no-index",
                "--find-links",
                config.wheelhouse_dir,
                "--wheel-dir",
                config.wheelhouse_dir,
            ],
            cwd=config.source_dir,
        )
        self._run(
            [
                build_python,
                "-m",
                "pip",
                "install",
                "--no-index",
                "--find-links",
                config.wheelhouse_dir,
                "headroom-ai[proxy]",
            ]
        )
        return HeadroomBuildStatus(
            source_version=self._read_source_version(config.source_dir),
            wheel_path=config.wheelhouse_dir,
            venv_python=build_python,
            headroom_command=headroom_command,
        )

    def _run(self, command, cwd=None):
        try:
            return self.runner.run(command, cwd=cwd)
        except FileNotFoundError as e:
            raise ExecutionError(
                f"Headroom source-vendor command not found: {command[0]}"
            ) from e
        except subprocess.CalledProcessError as e:
            raise ExecutionError(
                f"Headroom source-vendor command failed: {' '.join(map(str, command))}"
            ) from e

    def _is_python_310_or_newer(self, version_output):
        return is_python_310_or_newer(version_output)

    def _read_source_version(self, source_dir):
        return read_headroom_source_version(source_dir)


class HeadroomLocalInstaller:
    """直接从本地源码 pip 安装 Headroom（免 wheelhouse）。

    与 source-vendor 的区别：不使用 ``--no-index --find-links`` 离线 wheelhouse，
    而是让 pip 正常解析依赖并经 maturin 构建，省去预先备齐 wheel 的步骤。前置仍需
    Python 3.10+ 与 cargo（编译 Rust 扩展 ``headroom._core``）。
    """

    def __init__(self, runner=None):
        self.runner = runner or SubprocessCommandRunner()

    def prepare(self, config):
        validate_headroom_source_dir(config.source_dir)
        python_version = self._run([config.python_executable, "--version"]).strip()
        if not is_python_310_or_newer(python_version):
            raise ExecutionError(
                f"Headroom source-local mode requires Python 3.10 or newer; got {python_version}"
            )
        cargo_cmd = get_cargo_path()
        cargo_version = self._run([cargo_cmd, "--version"]).strip()
        if not cargo_version:
            raise ExecutionError("Headroom source-local mode requires cargo on PATH")

        build_python, headroom_command, needs_venv = resolve_build_python_and_command(config)
        if needs_venv:
            self._run([config.python_executable, "-m", "venv", config.venv_dir])

        # 免 wheelhouse：直接安装本地源码 + proxy extra，由 pip 正常解析依赖。
        self._run([build_python, "-m", "pip", "install", f"{config.source_dir}[proxy]"])

        return HeadroomBuildStatus(
            source_version=read_headroom_source_version(config.source_dir),
            wheel_path=config.source_dir,
            venv_python=build_python,
            headroom_command=headroom_command,
        )

    def _run(self, command, cwd=None):
        try:
            return self.runner.run(command, cwd=cwd)
        except FileNotFoundError as e:
            raise ExecutionError(
                f"Headroom source-local command not found: {command[0]}"
            ) from e
        except subprocess.CalledProcessError as e:
            raise ExecutionError(
                f"Headroom source-local command failed: {' '.join(map(str, command))}"
            ) from e

def get_headroom_base_url(config):
    return f"http://{config.host}:{config.port}/v1"

def get_headroom_origin(config):
    return f"http://{config.host}:{config.port}"

def build_headroom_proxy_command(config):
    command = [
        getattr(config, "headroom_command", "headroom"),
        "proxy",
        "--host",
        config.host,
        "--port",
        str(config.port),
        "--mode",
        config.mode,
    ]
    if config.backend:
        command.extend(["--backend", config.backend])
    command.append("--telemetry" if config.telemetry else "--no-telemetry")
    return command

class HeadroomHealthChecker:
    def check_ready(self, base_url):
        import urllib.request
        origin = base_url[:-3] if base_url.endswith("/v1") else base_url.rstrip("/")
        ready_url = f"{origin}/readyz"
        try:
            with urllib.request.urlopen(ready_url, timeout=2) as response:
                return 200 <= response.status < 300
        except Exception:
            return False

class HeadroomProcessRunner:
    def __init__(self, headroom_command=None):
        self.headroom_command = headroom_command

    def start(self, command):
        subprocess.Popen(
            command,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )

def ensure_headroom_running(config, health_checker=None, runner=None, sleeper=time.sleep):
    health_checker = health_checker or HeadroomHealthChecker()
    runner = runner or HeadroomProcessRunner()
    base_url = get_headroom_base_url(config)
    origin = get_headroom_origin(config)
    stats_url = f"{origin}/stats"
    if health_checker.check_ready(base_url):
        return HeadroomStatus(
            running=True,
            ready=True,
            base_url=base_url,
            stats_url=stats_url,
        )

    try:
        runner.start(build_headroom_proxy_command(config))
    except FileNotFoundError as e:
        raise ExecutionError(
            "Headroom command not found. Install Headroom with "
            "`pip install \"headroom-ai[proxy]\"` or provide it on PATH."
        ) from e

    attempts = max(1, config.startup_timeout_secs)
    for _ in range(attempts):
        sleeper(1)
        if health_checker.check_ready(base_url):
            return HeadroomStatus(
                running=True,
                ready=True,
                base_url=base_url,
                stats_url=stats_url,
            )

    raise ExecutionError(
        f"Headroom did not become ready at {origin}/readyz. "
        "Check Headroom logs, port availability, and backend credentials."
    )

def check_rust_version_meets_requirement(min_version=(1, 90, 0)):
    cargo_cmd = get_cargo_path()
    if cargo_cmd == "cargo" and not shutil.which("cargo"):
        return False
    try:
        kwargs = {"capture_output": True, "text": True, "check": True}
        if os.name == 'nt':
            result = subprocess.run(f'"{cargo_cmd}" --version', shell=True, **kwargs)
        else:
            result = subprocess.run([cargo_cmd, "--version"], **kwargs)
            
        match = re.search(r"cargo\s+(\d+)\.(\d+)\.(\d+)", result.stdout)
        if match:
            version = (int(match.group(1)), int(match.group(2)), int(match.group(3)))
            if version >= min_version:
                return True
    except Exception as e:
        print(f"Warning: Failed to check cargo version: {e}")
    return False

def load_profile_env():
    profile_path = os.path.expanduser("~/.profile")
    if not os.path.exists(profile_path):
        print(f"Warning: {profile_path} does not exist, skipping environment load.")
        return
    print(f"Loading environment variables from {profile_path}...")
    try:
        cmd = f"source {profile_path} && env"
        result = subprocess.run(["bash", "-c", cmd], capture_output=True, text=True, check=True)
        for line in result.stdout.splitlines():
            if "=" in line:
                key, value = line.split("=", 1)
                os.environ[key] = value
        print("Environment variables updated.")
    except subprocess.CalledProcessError as e:
        print(f"Warning: Failed to load {profile_path}: {e}")
    except Exception as e:
        print(f"Warning: Unexpected error loading {profile_path}: {e}")

def run_rust_install(cmds):
    if check_rust_version_meets_requirement():
        print("Rust (cargo) version >= 1.92.0 detected. Skipping Rust installation.")
        return

    cargo_dir = os.path.expanduser("~/.cargo")
    if os.path.exists(cargo_dir):
        print(f"Cleaning old Rust environment: removing {cargo_dir}")
        shutil.rmtree(cargo_dir, ignore_errors=True)
        
    for cmd in cmds:
        # Ignore comment lines
        if cmd.strip().startswith("#"):
            continue
        try:
            print(f"Executing: {cmd}")
            result = subprocess.run(cmd, shell=True, check=True, capture_output=True, text=True)
            if result.stdout:
                print(result.stdout)
        except subprocess.CalledProcessError as e:
            raise ExecutionError(f"Rust installation failed on command '{cmd}': {e.stderr}")
            
    load_profile_env()

def setup_kernel_symlink(target_dir, linux_kernel_dir):
    # Strip any accidental single or double quotes from the path
    linux_kernel_dir = linux_kernel_dir.strip("'\"")
    # Expand ~ to home directory and resolve to absolute path
    linux_kernel_dir = os.path.abspath(os.path.expanduser(linux_kernel_dir))
    
    third_party_dir = os.path.join(target_dir, "third_party")
    if not os.path.exists(third_party_dir):
        os.makedirs(third_party_dir)
        
    linux_symlink_path = os.path.join(third_party_dir, "linux")
    
    print(f"Setting up kernel symlink at {linux_symlink_path} pointing to {linux_kernel_dir}")
    if os.path.islink(linux_symlink_path):
        os.unlink(linux_symlink_path)
    elif os.path.exists(linux_symlink_path):
        if os.path.isdir(linux_symlink_path):
            shutil.rmtree(linux_symlink_path)
        else:
            os.remove(linux_symlink_path)
            
    os.symlink(linux_kernel_dir, linux_symlink_path)
    print("Kernel symlink created successfully.")

def update_settings_toml(target_dir, app_config, headroom_status=None):
    settings_path = os.path.join(target_dir, "Settings.toml")
    if not os.path.exists(settings_path):
        raise ExecutionError(f"Settings.toml not found at {settings_path}")
        
    try:
        import tomlkit
    except ImportError:
        raise ExecutionError("tomlkit is not installed. Please install it via requirements.txt")
        
    print(f"Updating {settings_path}...")
    with open(settings_path, "r", encoding="utf-8") as f:
        toml_data = tomlkit.load(f)
        
    # Mapping app_config.server.port to [server] port
    if "server" in app_config and "port" in app_config["server"]:
        if "server" not in toml_data:
            toml_data["server"] = tomlkit.table()
        toml_data["server"]["port"] = app_config["server"]["port"]

    if headroom_status and headroom_status.ready:
        ai_config = app_config.get("ai", {})
        if "ai" not in toml_data:
            toml_data["ai"] = tomlkit.table()
        toml_data["ai"]["provider"] = "openai-compatible"
        if "model" in ai_config:
            toml_data["ai"]["model"] = ai_config["model"]
        if "api_timeout_secs" in ai_config:
            toml_data["ai"]["api_timeout_secs"] = ai_config["api_timeout_secs"]

        if "openai_compat" not in toml_data["ai"]:
            toml_data["ai"]["openai_compat"] = tomlkit.table()
        openai_compat = toml_data["ai"]["openai_compat"]
        openai_compat["base_url"] = headroom_status.base_url
        if "streaming" in ai_config:
            openai_compat["streaming"] = ai_config["streaming"]
        if "stream_idle_timeout_secs" in ai_config:
            openai_compat["stream_idle_timeout_secs"] = ai_config["stream_idle_timeout_secs"]
        
    with open(settings_path, "w", encoding="utf-8") as f:
        tomlkit.dump(toml_data, f)
    print("Settings.toml updated successfully.")

def generate_env_file(target_dir, app_config):
    env_path = os.path.join(target_dir, ".env")
    openai_key = app_config.get("ai", {}).get("openai_key")
    if not openai_key:
        print("Warning: openai_key not found in configuration. .env file will not contain LLM_API_KEY.")
        return
        
    print(f"Generating {env_path}...")
    with open(env_path, "w", encoding="utf-8") as f:
        f.write(f"LLM_API_KEY={openai_key}\n")
    print(".env file generated successfully.")

def generate_monitor_config(target_dir, config):
    if "monitor" not in config:
        return
        
    monitor_dir = get_monitor_dir()
    if not os.path.exists(monitor_dir):
        os.makedirs(monitor_dir)
        
    config_path = os.path.join(monitor_dir, "monitor_config.json")
    
    monitor_data = {
        "pull_interval_sec": 3600,
        "max_retries": 3,
        "max_history_days": 180,
        "server_ip": "127.0.0.1",
        "server_port": 18888,
        "branches": []
    }
    
    if os.path.exists(config_path):
        try:
            with open(config_path, "r", encoding="utf-8") as f:
                existing_data = json.load(f)
                monitor_data.update(existing_data)
        except Exception as e:
            print(f"Warning: failed to read existing monitor_config.json: {e}")
            
    # Override with values from bootstrap config
    if "app_config" in config and "server" in config["app_config"]:
        monitor_data["server_port"] = config["app_config"]["server"].get("port", monitor_data["server_port"])
        
    if "monitor" in config and "branches" in config["monitor"]:
        monitor_data["branches"] = config["monitor"]["branches"]
        
    # Ensure server_ip is set
    if "server_ip" not in monitor_data or not monitor_data["server_ip"]:
        monitor_data["server_ip"] = "127.0.0.1"
        
    with open(config_path, "w", encoding="utf-8") as f:
        json.dump(monitor_data, f, indent=2)
    print(f"Generated monitor_config.json at {config_path}")

def get_cargo_path():
    cargo_path = shutil.which("cargo")
    if cargo_path:
        return cargo_path
        
    cargo_path = os.path.expanduser("~/.cargo/bin/cargo")
    if os.path.exists(cargo_path):
        return cargo_path
    elif os.name == 'nt':
        cargo_path_nt = os.path.expanduser("~\\.cargo\\bin\\cargo.exe")
        if os.path.exists(cargo_path_nt):
            return cargo_path_nt
    return "cargo"

def build_project(target_dir):
    print("Building project...")
    cargo_cmd = get_cargo_path()
    try:
        subprocess.run([cargo_cmd, "build", "--release"], cwd=target_dir, check=True)
        subprocess.run([cargo_cmd, "build"], cwd=target_dir, check=True)
        
        monitor_dir = get_monitor_dir()
        if os.path.exists(monitor_dir):
            print("Building repo_monitor...")
            manifest_path = os.path.join(monitor_dir, "Cargo.toml")
            subprocess.run([cargo_cmd, "build", "--release", "--manifest-path", manifest_path], cwd=target_dir, check=True)
            subprocess.run([cargo_cmd, "build", "--manifest-path", manifest_path], cwd=target_dir, check=True)
            
        print("Build successful.")
    except subprocess.CalledProcessError as e:
        raise ExecutionError(f"Failed to build project: {e}")

def check_ai_provider_network(target_dir):
    settings_path = os.path.join(target_dir, "Settings.toml")
    if not os.path.exists(settings_path):
        return
        
    try:
        import tomlkit
        with open(settings_path, "r", encoding="utf-8") as f:
            toml_data = tomlkit.load(f)
    except ImportError:
        print("Warning: tomlkit not installed, skipping AI provider network check.")
        return
    except Exception as e:
        print(f"Warning: Failed to read Settings.toml for network check: {e}")
        return
        
    ai_config = toml_data.get("ai", {})
    provider = ai_config.get("provider", "").lower()
    
    if provider == "claude-cli":
        print("Using claude-cli provider, skipping network check.")
        return
        
    import urllib.parse
    import socket
    
    domain = None
    port = 443
    
    base_url = ai_config.get("base_url")
    if base_url:
        parsed = urllib.parse.urlparse(base_url)
        domain = parsed.hostname
        if parsed.port:
            port = parsed.port
        elif parsed.scheme == 'http':
            port = 80
    else:
        domains = {
            "gemini": "generativelanguage.googleapis.com",
            "openai": "api.openai.com",
            "claude": "api.anthropic.com",
            "deepseek": "api.deepseek.com",
        }
        domain = domains.get(provider)
        
    if not domain:
        print(f"Skipping network check for unknown or unconfigured AI provider: {provider}")
        return
        
    print(f"Checking network connectivity to AI provider '{provider}' ({domain}:{port})...")
    try:
        socket.create_connection((domain, port), timeout=5)
        print("Network check passed.")
    except OSError as e:
        raise ExecutionError(f"Network error: Unable to connect to AI provider '{provider}' at {domain}:{port}. Please check your network or proxy settings. Detail: {e}")

def run_project(target_dir, config=None):
    import signal
    def handle_sig(signum, frame):
        raise KeyboardInterrupt()
    if hasattr(signal, "SIGBREAK"):
        signal.signal(signal.SIGBREAK, handle_sig)
    signal.signal(signal.SIGTERM, handle_sig)

    print("Running project in debug mode...")
    cargo_cmd = get_cargo_path()
    
    env = os.environ.copy()
    if config and "app_config" in config and "ai" in config["app_config"]:
        openai_key = config["app_config"]["ai"].get("openai_key")
        if openai_key:
            env["LLM_API_KEY"] = openai_key
            
    processes = []
    
    try:
        # Main server process
        p_main = subprocess.Popen([cargo_cmd, "run", "--", "--debug"], cwd=target_dir, env=env)
        processes.append(p_main)
        
        # Placeholder for repo_monitor
        if config and config.get("monitor"):
            monitor_dir = get_monitor_dir()
            monitor_config_path = os.path.join(monitor_dir, "monitor_config.json")
            manifest_path = os.path.join(monitor_dir, "Cargo.toml")
            p_monitor = subprocess.Popen(
                [cargo_cmd, "run", "--manifest-path", manifest_path, "--", "--config", monitor_config_path],
                cwd=target_dir,
                env=env
            )
            processes.append(p_monitor)
            
        # Wait for processes
        while True:
            for p in processes:
                ret = p.poll()
                if ret is not None:
                    raise ExecutionError(f"A child process exited prematurely with code {ret}")
            time.sleep(1)
            
    except KeyboardInterrupt:
        print("\nReceived SIGINT, waiting for processes to terminate gracefully...")
        for p in processes:
            if p.poll() is None:
                try:
                    p.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    print(f"Process {p.pid} did not terminate gracefully, forcing...")
                    p.terminate()
                    p.wait()
        print("All processes terminated.")
    except Exception as e:
        for p in processes:
            if p.poll() is None:
                p.terminate()
                p.wait()
        if not isinstance(e, ExecutionError):
            raise ExecutionError(f"Failed to run project: {e}")
        raise

def install_dependencies():
    req_path = os.path.join(get_script_dir(), "requirements.txt")
    if os.path.exists(req_path):
        print("Installing Python dependencies from requirements.txt...")
        try:
            subprocess.run(["pip3", "install", "-r", req_path], check=True)
            print("Dependencies installed successfully.")
        except subprocess.CalledProcessError as e:
            print(f"Warning: Failed to install dependencies: {e}")
    else:
        print(f"Warning: {req_path} not found, skipping dependency installation.")

def main(config_path, should_run=False):
    try:
        print("Starting deployment process...")
        install_dependencies()
        
        config = load_config(config_path)
        
        run_rust_install(config.get("rust_install_cmds", []))
        
        target_dir = get_target_dir()
        setup_kernel_symlink(target_dir, config["linux_kernel_dir"])

        headroom_config = parse_headroom_config(config)
        headroom_status = None
        if headroom_config.enabled:
            if headroom_config.install_mode == "source-vendor":
                build_status = HeadroomVendorManager().prepare(headroom_config)
                headroom_config.headroom_command = build_status.headroom_command
            elif headroom_config.install_mode == "source-local":
                build_status = HeadroomLocalInstaller().prepare(headroom_config)
                headroom_config.headroom_command = build_status.headroom_command
            headroom_status = ensure_headroom_running(headroom_config)

        update_settings_toml(target_dir, config["app_config"], headroom_status=headroom_status)
        generate_env_file(target_dir, config["app_config"])
        generate_monitor_config(target_dir, config)
        
        build_project(target_dir)
        
        if should_run:
            check_ai_provider_network(target_dir)
            run_project(target_dir, config)
            
        print("Deployment completed successfully!")
        return 0
    except (ValidationError, ExecutionError) as e:
        print(f"Error during deployment: {e}", file=sys.stderr)
        return 1
    except Exception as e:
        print(f"Unexpected error: {e}", file=sys.stderr)
        return 1

if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser(description="One-Click Deployment Tool")
    parser.add_argument("--config", required=True, help="Path to the JSON configuration file")
    parser.add_argument("--run", action="store_true", help="Run the project after deployment")
    args = parser.parse_args()
    sys.exit(main(args.config, args.run))

