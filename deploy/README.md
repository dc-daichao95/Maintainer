# Headroom Linux 一键部署

从**本地源码构建**镜像（绝不 `docker pull`），用 Docker Compose 把 headroom proxy
以常驻服务跑起来，监听 `0.0.0.0:8787`，开机随 Docker 守护进程自动拉起。

## 前置条件

- Linux 主机
- 已安装 Docker（含 Compose v2：`docker compose`；或旧版 `docker-compose`）
- 当前用户可访问 Docker 守护进程（在 `docker` 组中，或使用 `sudo`）

脚本**不会**替你安装 Docker。未安装时会给出官方安装链接并退出。

## 快速开始

```bash
cd deploy
./deploy-linux.sh
```

这一条命令会：

1. 自检 `docker` 与 `docker compose`
2. 从 `../headroom-main/headroom-main` 的 `Dockerfile` 本地构建镜像 `headroom:local`
   （首次需编译 Rust 核心，约数分钟）
3. 以 `restart: unless-stopped` 常驻启动 proxy
4. 轮询 `http://127.0.0.1:8787/readyz`，就绪后打印接入提示

## 管理命令

```bash
./deploy-linux.sh up        # 默认：构建 + 常驻启动 + 健康检查
./deploy-linux.sh status    # 查看容器状态
./deploy-linux.sh logs      # 跟随日志
./deploy-linux.sh down      # 停止并移除
./deploy-linux.sh rebuild   # 无缓存重建镜像并重启
./deploy-linux.sh --help    # 帮助
```

## 配置

| 环境变量 | 默认值 | 说明 |
|---|---|---|
| `HEADROOM_PORT` | `8787` | 代理监听端口 |
| `HEADROOM_HOST_HOME` | `$HOME` | 状态持久化的宿主机 HOME（挂载其 `.headroom`） |
| `HEALTH_TIMEOUT` | `90` | 健康检查最长等待秒数 |

示例：

```bash
HEADROOM_PORT=9000 ./deploy-linux.sh up
```

## 接入 agent

部署成功后，让你的 agent 指向该代理（把 `<本机IP>` 换成实际 IP，脚本会自动猜测）：

```bash
export ANTHROPIC_BASE_URL="http://<本机IP>:8787"        # Claude
export OPENAI_BASE_URL="http://<本机IP>:8787/v1"         # Codex
# Cursor: OpenAI Base URL 填 http://<本机IP>:8787/v1
```

## ⚠️ 安全提示

代理监听 `0.0.0.0`，意味着**同局域网内任意机器都能访问**，且 proxy 默认**无鉴权**。

- 仅建议在可信内网使用。
- 公网环境务必加防火墙规则，或在前面放置带鉴权的反向代理。
- 如只需本机使用，可改为 `HEADROOM_PORT` 不变但将 compose 端口映射改为 `127.0.0.1:8787:8787`。

## 与上游的关系

本目录所有文件均为**新增**，不修改 headroom 任何现有源码或基础设施，
可随上游平滑同步。底层复用 headroom 自带的多阶段 `Dockerfile`。
