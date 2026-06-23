# Headroom Linux 一键部署（本地构建）设计

- 日期：2026-06-23
- 状态：已确认，进入实现
- 范围：为 headroom 增加一个面向 Linux 的「一键、本地源码构建、Docker 常驻」部署入口

## 背景与动机

headroom 现有的 Linux 部署路径丰富但分散：`pip install` 后还需 `headroom install apply --preset ...`；
或走 `scripts/install.sh` 的 Docker-native wrapper；预设多（`persistent-service`/`persistent-task`/`persistent-docker`），
新用户选择成本高。缺少「从已装 Docker 的机器 → proxy 作为常驻服务跑起来」的单条命令。

本设计提供一个最小、零侵入、可重入的一键脚本，从**本地源码构建镜像**（绝不 `docker pull`），
以 Docker Compose + `restart: unless-stopped` 常驻运行 proxy。

## 已确认的关键决策

| 决策点 | 选择 |
|---|---|
| 运行时形态 | Docker 容器 |
| 镜像来源 | 本地源码构建（`docker build`，不 pull） |
| 开机自启/进程管理 | `docker compose` + `restart: unless-stopped`（不引入 systemd unit） |
| 代码落点 | 仓库顶层隔离目录 `deploy/`，与 headroom 源码物理隔离 |
| 监听地址 | `0.0.0.0:8787`（局域网可访问） |

## 目录结构（全部新增，零侵入 headroom 源码）

```
deploy/
├── deploy-linux.sh          # 一键入口（幂等、可重入）
├── docker-compose.yml       # 本地构建 + 常驻 proxy
└── README.md                # 用法 + 安全提示
```

## 组件设计

### deploy/docker-compose.yml

- `build.context: ../headroom-main/headroom-main`，`dockerfile: Dockerfile`，走本地 `Dockerfile` 多阶段构建。
- `image: headroom:local`，构建产物打本地 tag，复用避免重复构建。
- `restart: unless-stopped`，Docker 守护进程开机自启后自动拉起 proxy。
- `ports: "${HEADROOM_PORT:-8787}:${HEADROOM_PORT:-8787}"`，容器内 `--host 0.0.0.0`。
- 挂载 `${HOME}/.headroom` 持久化状态/配置；设置 issue #175 文件系统契约环境变量
  （`HEADROOM_WORKSPACE_DIR` / `HEADROOM_CONFIG_DIR`）。

### deploy/deploy-linux.sh

`set -euo pipefail`，可重入幂等。流程：

1. **环境自检**：检测 `docker` 与 `docker compose`（兼容 `docker-compose` v1 回退）。缺失则清晰报错 +
   安装指引，非零退出；**不擅自安装 Docker**（避免破坏性操作）。
2. **本地构建**：`docker compose -f deploy/docker-compose.yml build`。
3. **常驻启动**：`docker compose -f deploy/docker-compose.yml up -d proxy`。
4. **健康验证**：轮询 `http://127.0.0.1:${PORT}/readyz`，默认超时 ~90s；成功打印下一步提示
   （让 claude/codex/cursor 指向 `http://<本机IP>:PORT`）。
5. **失败诊断**：超时或启动失败 → `docker compose logs` dump + 非零退出。

子命令：`up`（默认）、`down`、`status`、`logs`、`rebuild`、`--help`。
环境变量：`HEADROOM_PORT`（默认 8787）。

## 错误处理

- 顶层 `set -euo pipefail`；关键命令失败给可操作建议。
- 健康检查独立超时与重试，避免脚本无限阻塞。
- 端口/数值参数做基本校验。

## 安全考量

监听 `0.0.0.0` 意味着同局域网任意机器可访问该代理，且 proxy 默认无鉴权。
README 明确提示：仅建议可信内网使用，公网请加防火墙/反向代理鉴权。

## 测试策略

bash 脚本以函数化设计，便于单测：

- 参数解析（子命令、`HEADROOM_PORT`、`--help`）。
- 环境自检在 docker 缺失时的报错路径（PATH 打桩）。
- 健康检查轮询逻辑（对 curl/poll 打桩，验证成功/超时分支）。

不依赖真实 Docker 守护进程：以 `bash` 断言或 `bats`（若可用）对纯逻辑函数做桩测试。
`bash -n` 语法检查与 `shellcheck`（若可用）作为静态门禁。

## 与上游的兼容性

所有改动均为 `deploy/` 下的新增文件，不修改 headroom 任何现有源码或基础设施，
可随上游平滑同步、无飞线。
