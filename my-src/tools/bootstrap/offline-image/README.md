# Headroom 离线容器部署

把 Headroom proxy 以**容器镜像**形式带进离线 Linux：在联网机器上拉取官方镜像并打包成
tar，拷到离线机 `docker load` 后常驻运行。镜像已内置全部 Python 依赖与编译好的 Rust 核心，
**避开 wheel 跨平台与 cargo 离线编译两大难题**。

适用：离线 Linux **x86_64**（镜像架构需与目标机一致）。

## 步骤 1：联网机器上产出镜像 tar

在任意「有 Docker + 能联网」的 x86_64 机器上：

```bash
cd my-src/tools/bootstrap/offline-image
./pull_save.sh headroom-offline.tar
# 指定版本：HEADROOM_IMAGE=ghcr.io/chopratejas/headroom:v0.26.0 ./pull_save.sh headroom-offline.tar
```

产物 `headroom-offline.tar` 即可拷贝（U 盘/内网传输）到离线机。

## 步骤 2：离线 Linux 上加载并常驻运行

```bash
cd my-src/tools/bootstrap/offline-image
./load_run.sh headroom-offline.tar
# 自定义端口：HEADROOM_PORT=9000 ./load_run.sh headroom-offline.tar
```

脚本会 `docker load` 镜像、以 `--restart unless-stopped` 启动 proxy（监听 `0.0.0.0:8787`）、
并轮询 `/readyz` 确认就绪。开机随 Docker 守护进程自动拉起。

## 步骤 3：让 deploy.py 对接已运行的容器

容器已经在跑 proxy，所以 `deploy.py` **无需再本地构建/安装** Headroom——用 `external-cli`
模式让它探测到就绪的代理并把 Sashiko 的 provider 指过去。`config.json`：

```json
"headroom": {
  "enabled": true,
  "install_mode": "external-cli",
  "host": "127.0.0.1",
  "port": 8787,
  "mode": "token",
  "backend": "openrouter"
}
```

> **顺序很重要**：先 `load_run.sh` 让容器就绪，再跑 `python deploy.py --config config.json`。
> `external-cli` 模式下 deploy 只探测 `/readyz`；若代理已就绪，它不会尝试在宿主机启动 headroom
> （宿主机没有装 headroom），从而直接完成 Settings.toml 接线。

## 管理

```bash
docker ps                       # 查看容器
docker logs -f headroom         # 日志
docker restart headroom         # 重启
docker rm -f headroom           # 停止并移除
```

## 说明与边界

- **架构匹配**：`docker save` 出的镜像是 x86_64；离线机必须是 x86_64。aarch64 需在 arm64 机器上
  重新 pull/save（或用 buildx 跨架构）。
- **持久化（可选）**：如需保留 Headroom 状态/CCR 缓存，给 `docker run` 加
  `-v $HOME/.headroom:/home/nonroot/.headroom`（注意容器内以 nonroot/uid 1000 运行，宿主目录需可写）。
- **安全**：proxy 监听 `0.0.0.0` 且默认无鉴权，仅建议可信内网使用。
- 本目录所有文件均为新增，不修改 headroom 上游源码，亦不改 Sashiko 原生 `src/`。
