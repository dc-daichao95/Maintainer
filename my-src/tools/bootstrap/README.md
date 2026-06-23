# Kernel Maintainer 一键部署工具

本工具提供了一种在 Linux 环境下自动化部署 `kernel-maintainer` 项目的便捷方式。

## 前置条件
- Python 3.8+
- Git
- `pip3`

## 自动化特性
该部署脚本在执行时会自动完成以下操作：
1. **自动安装依赖**：脚本启动时会自动执行 `pip3 install -r requirements.txt` 安装所需的 Python 依赖（如 `tomlkit`）。
2. **智能 Rust 环境管理**：
   - 自动检测系统中是否已安装 `cargo` 且版本 `>= 1.92.0`。如果是，则跳过重复安装。
   - 如果需要安装，会自动清理旧的 `~/.cargo` 目录以确保环境纯净。
   - 安装完成后，自动加载 `~/.profile` 环境变量。
3. **自动编译**：部署配置完成后，会自动在目标目录下执行 `cargo build --release` 和 `cargo build`。

> **注意**：脚本会自动根据其所在位置推导出 `kernel-maintainer` 仓库的根目录，并在该目录下执行配置、编译和运行。

## 配置说明
将 `config_template.json` 复制为 `config.json`，并根据您的实际环境调整参数。

配置字段说明：
- `linux_kernel_dir`: 本地 Linux 内核源码目录的绝对路径（用于建立软链接）。
- `rust_install_cmds`: 用于在您的环境中安装 Rust 的 Shell 命令数组。
- `app_config.server.port`: 注入到 `Settings.toml` 中的 Web 服务端口。
- `app_config.ai.openai_key`: 您的 LLM API 密钥，该值将被保存到目标目录的 `.env` 文件中。

## 使用方法
运行部署脚本，并传入您的配置文件路径：
```bash
python deploy.py --config config.json
```

如果您希望在部署和编译完成后**自动运行项目**（执行 `cargo run -- --debug`），可以加上 `--run` 参数：
```bash
python deploy.py --config config.json --run
```

## 运行测试
使用 `pytest` 运行单元测试和端到端测试：
```bash
cd my-src/tools/bootstrap
python -m pytest tests/
```

## Headroom 上下文压缩代理集成

部署工具支持可选的 Headroom 本地代理托管。默认集成方式为 `source-vendor`：从 `my-src/third_party/headroom/source/` 中的 Headroom 源码构建 wheel，安装到目标 Python 环境，再启动该环境中的 `headroom proxy`。脚本会先检查 `http://<host>:<port>/readyz`，如果代理未就绪则尝试启动代理；只有代理 ready 后，才会把 Sashiko 的 OpenAI-compatible provider 指向 Headroom。

如果希望**一键快速部署、免去预备 wheelhouse**，可改用 `source-local` 模式：直接 `pip install "<source_dir>[proxy]"`，由 pip 正常（联网）解析依赖并经 maturin 构建，无需提前在 `wheelhouse_dir` 备齐离线 wheel。前置仍需 Python 3.10+ 与 `cargo`。`source-vendor` 适合离线/内网受控环境，`source-local` 适合可联网的快速部署。

> **conda 环境说明**：如果部署本身就运行在一个已激活的 conda 环境中（即检测到 `CONDA_PREFIX`），脚本不会再创建独立的 `venv`，而是直接把 Headroom 构建并安装进当前 conda 环境，`headroom` 命令解析为 `$CONDA_PREFIX/bin/headroom`。这样可以避免在 conda 中嵌套 venv 导致的 `ensurepip` 等启动错误。此时 `venv_dir` 配置会被忽略。

### 前置条件

- Headroom 源码已固定在 `my-src/third_party/headroom/source/`。
- 用于构建 Headroom 的 Python 必须为 3.10+。
- 系统中必须有 `cargo`，因为 Headroom wheel 通过 `maturin` 构建 Rust 扩展 `headroom._core`。
- 离线或内网环境下，`wheelhouse_dir` 必须提供 `maturin` 和 Headroom `proxy` extra 所需依赖 wheel；bootstrap 使用 `--no-index --find-links`，不会隐式访问公网 PyPI。
- Headroom 的上游 provider 凭据需要按现场环境配置。部署脚本不会在日志中打印 API Key。

### 配置字段

在 `config.json` 中设置：

```json
"headroom": {
  "enabled": true,
  "install_mode": "source-vendor",
  "source_dir": "my-src/third_party/headroom/source",
  "venv_dir": "my-src/.venv-headroom",
  "wheelhouse_dir": "my-src/third_party/headroom/wheelhouse",
  "python_executable": "python",
  "host": "127.0.0.1",
  "port": 8787,
  "mode": "token",
  "backend": "openrouter",
  "telemetry": false,
  "startup_timeout_secs": 20
}
```

字段说明：
- `enabled`: 是否启用 Headroom 集成。默认 `false`，不影响现有部署。
- `install_mode`: Headroom 安装模式。`source-vendor`（默认，离线 wheelhouse 构建安装）、`source-local`（免 wheelhouse，直接 pip 联网安装本地源码）或 `external-cli`。
- `source_dir`: Headroom vendor 源码目录。
- `venv_dir`: Headroom 专用 Python 虚拟环境目录。**若部署运行在已激活的 conda 环境中，此项被忽略，Headroom 直接装入当前 conda 环境。**
- `wheelhouse_dir`: Headroom wheel 构建与离线依赖目录。
- `python_executable`: 用于创建 venv 和构建 wheel 的 Python，启用 source vendor 时必须为 3.10+。
- `host` / `port`: Headroom 监听地址，默认 `127.0.0.1:8787`。
- `mode`: Headroom 优化模式，默认 `token`。
- `backend`: Headroom 上游后端，由现场实际 provider 决定。
- `telemetry`: 是否开启 Headroom 遥测，默认关闭。
- `startup_timeout_secs`: 启动后等待 `/readyz` 就绪的秒数。

### 写入的 Sashiko 配置

Headroom ready 后，部署工具会写入：
- `[ai].provider = "openai-compatible"`
- `[ai.openai_compat].base_url = "http://<host>:<port>/v1"`

Sashiko 的 OpenAI-compatible client 会将该 base URL 规范化为 `/v1/chat/completions` 请求路径。

### 验证与排查

- 健康检查：`http://<host>:<port>/readyz`
- 压缩统计：`http://<host>:<port>/stats`
- Prometheus 指标：`http://<host>:<port>/metrics`

常见失败：
- `source_dir` 不完整：确认目录包含 `pyproject.toml`、`headroom/`、`crates/headroom-py/`、`crates/headroom-core/`。
- Python 版本低于 3.10：调整 `python_executable` 指向 Python 3.10+。
- `cargo` 命令不存在：安装 Rust/cargo 或修正 `PATH`。
- wheel 构建或安装失败：检查 `wheelhouse_dir` 是否包含 `maturin`、`fastapi`、`uvicorn`、`httpx`、`openai` 等 Headroom `proxy` extra 依赖 wheel。
- 端口被占用：修改 `headroom.port` 或停止占用端口的进程。
- `/readyz` 超时：检查 Headroom 日志、上游 provider 凭据和网络连通性。

### 边界说明

本集成不修改 Sashiko 原生 `src/` 代码，不把 Headroom Rust crates 加入根 Cargo workspace。`my-src/third_party/headroom/source/` 是上游源码快照，默认不在本项目内直接修改；版本更新应通过替换快照并更新 `my-src/third_party/headroom/VENDOR.md` 完成。Headroom 作为代理运行，关闭集成只需将 `headroom.enabled` 改回 `false` 或恢复原有 LLM provider 配置。

## 常见问题排查
- 如果脚本在安装 Rust 时失败，请确保您有网络访问权限（如使用 `curl`），或者在 `rust_install_cmds` 中提供正确的内网/离线安装命令。
- 请确保您对 `kernel-maintainer` 仓库目录具有写入权限。
