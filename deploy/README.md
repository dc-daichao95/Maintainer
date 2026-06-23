# Headroom Linux 一键部署（原生，无 Docker）

从**本地源码**安装 headroom（绝不从 PyPI 拉取），注册 **systemd 系统服务**常驻运行 proxy，
监听 `0.0.0.0:8787`，开机自启。

## 前置条件

目标机需预装以下依赖（脚本**不会**替你安装，缺失时会给出指引并退出）：

- Linux + systemd
- `python3` ≥ 3.10 与 `pip`
- C 编译器（`build-essential` / `gcc`）
- Rust 工具链（`cargo`）—— 编译 headroom 的 Rust 核心所必需

安装 Rust（若缺）：

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh && rustup default stable
```

## 快速开始

```bash
cd deploy
sudo ./deploy-linux.sh
```

这一条命令会：

1. 检查 root 权限与构建依赖（python3/pip/cc/cargo）
2. 从 `../headroom-main/headroom-main` 本地源码 `pip install ".[proxy,code]"`
   （首次需编译 Rust 核心，约数分钟）
3. 渲染并安装 `/etc/systemd/system/headroom.service`，`enable --now`
4. 轮询 `http://127.0.0.1:8787/readyz`，就绪后打印接入提示

## 管理命令

```bash
sudo ./deploy-linux.sh install     # 默认：安装 + 注册服务 + 健康检查
./deploy-linux.sh status           # 查看服务状态
./deploy-linux.sh logs             # 跟随日志（journalctl）
sudo ./deploy-linux.sh restart     # 重启服务
sudo ./deploy-linux.sh uninstall   # 停止禁用并删除服务（保留 Python 包）
./deploy-linux.sh --help           # 帮助
```

## 配置

| 环境变量 | 默认值 | 说明 |
|---|---|---|
| `HEADROOM_PORT` | `8787` | 代理监听端口 |
| `HEALTH_TIMEOUT` | `90` | 健康检查最长等待秒数 |
| `HEADROOM_PIP_BREAK_SYSTEM` | 未设 | 设为 `1` 时给 pip 追加 `--break-system-packages`（应对 PEP 668 externally-managed） |

示例：

```bash
sudo HEADROOM_PORT=9000 ./deploy-linux.sh install
```

### PEP 668（Debian/Ubuntu 较新版本）

若 `pip install` 报 `externally-managed-environment`，按提示重试：

```bash
sudo HEADROOM_PIP_BREAK_SYSTEM=1 ./deploy-linux.sh install
```

## 接入 agent

部署成功后，让你的 agent 指向该代理（`<本机IP>` 脚本会自动猜测）：

```bash
export ANTHROPIC_BASE_URL="http://<本机IP>:8787"        # Claude
export OPENAI_BASE_URL="http://<本机IP>:8787/v1"         # Codex
# Cursor: OpenAI Base URL 填 http://<本机IP>:8787/v1
```

## ⚠️ 安全提示

- 代理监听 `0.0.0.0`，**同局域网内任意机器都能访问**，且 proxy 默认**无鉴权**。
- systemd 服务以 **root** 身份运行。
- 仅建议在可信内网使用；公网务必加防火墙规则或带鉴权的反向代理。

## 与上游的关系

本目录所有文件均为**新增**，不修改 headroom 任何现有源码或基础设施，可随上游平滑同步。
复用 headroom 自带的 `pyproject.toml` extras 与 `headroom proxy` CLI。
