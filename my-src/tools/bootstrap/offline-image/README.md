# Headroom 离线容器部署

把 Headroom proxy 以**容器镜像**形式带进离线 Linux：在联网机器上拉取官方镜像并打包成
tar，拷到离线机 `docker load` 后常驻运行。镜像已内置全部 Python 依赖与编译好的 Rust 核心，
**避开 wheel 跨平台与 cargo 离线编译两大难题**。

适用：离线 Linux **x86_64**（镜像架构需与目标机一致）。

## 已内置镜像分卷（可直接离线使用）

仓库已内置 `ghcr.io/chopratejas/headroom:latest` 的镜像归档（gzip），因 GitHub 单文件
100MB 上限切分为分卷置于 `image/`。离线机上先合并再加载，**无需联网、无需重新拉取**：

```bash
cd my-src/tools/bootstrap/offline-image/image
./reassemble.sh                              # 合并 + sha256 校验 → headroom-offline.tar.gz
docker load -i headroom-offline.tar.gz       # 或：../load_run.sh headroom-offline.tar.gz
```

> 若镜像需更新或换架构，用下面的 `pull_image_http.py` / `pull_save.sh` 重新产出并替换 `image/` 下分卷。
> 重新切分可参考：`split -b 90m headroom-offline.tar.gz headroom-offline.tar.gz.part`（注意命名排序）。

## 步骤 1：联网机器上产出镜像 tar

在任意「有 Docker + 能联网」的 x86_64 机器上：

```bash
cd my-src/tools/bootstrap/offline-image
./pull_save.sh headroom-offline.tar
# 指定版本：HEADROOM_IMAGE=ghcr.io/chopratejas/headroom:v0.26.0 ./pull_save.sh headroom-offline.tar
```

产物 `headroom-offline.tar` 即可拷贝（U 盘/内网传输）到离线机。

## 步骤 2：配置上游 LLM（必做）

Headroom 是**透明代理**：Maintainer → Headroom →（压缩后）转发到**真实 LLM** → 原路返回。
所以容器必须知道上游 LLM 地址，否则会默认打 `api.openai.com`（离线 → 502）。

把参数写进配置文件 `headroom.env`（被 `load_run.sh` 自动读取）：

```bash
cd my-src/tools/bootstrap/offline-image
cp headroom.env.example headroom.env
# 编辑 headroom.env，至少设置 OPENAI_TARGET_API_URL（不带 /v1）：
#   OPENAI_TARGET_API_URL=http://10.0.0.5:8000
#   # 若容器默认网络访问不到上游，再设 HEADROOM_NETWORK=host
```

- `OPENAI_TARGET_API_URL` 填 **Maintainer 接入 Headroom 之前原本直连的那台 LLM**（base，不带 `/v1`）。
- `headroom.env` 已被 `.gitignore`，含内网地址不会入库。

## 步骤 3：离线 Linux 上加载并常驻运行

```bash
./load_run.sh headroom-offline.tar
# 或先合并分卷：image/reassemble.sh 后 ./load_run.sh image/headroom-offline.tar.gz
```

脚本会读取 `headroom.env`、`docker load` 镜像、以 `--restart unless-stopped` 启动 proxy
（注入 `OPENAI_TARGET_API_URL` 及离线必需变量）、并轮询 `/readyz` 确认就绪。开机自启。

## 步骤 4：让 deploy.py 对接已运行的容器

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

## 故障排查：容器 unhealthy

`/readyz` 默认会探测**上游 LLM provider 是否可达**；在离线/气隙环境上游不可达 → `/readyz` 返回
503 → 容器 `unhealthy`。`load_run.sh` 已默认注入 `HEADROOM_SKIP_UPSTREAM_CHECK=1` 跳过该探测，
容器即可正常 healthy。若手动 `docker run`，请记得加上：

```bash
-e HEADROOM_SKIP_UPSTREAM_CHECK=1 \
-e HF_HUB_OFFLINE=1 -e TRANSFORMERS_OFFLINE=1 -e HEADROOM_UPDATE_CHECK=off
```

> healthy ≠ 能用：Headroom 是转发代理，真正发请求时仍需上游 provider 可达 + 有效密钥。

## 说明与边界

- **架构匹配**：`docker save` 出的镜像是 x86_64；离线机必须是 x86_64。aarch64 需在 arm64 机器上
  重新 pull/save（或用 buildx 跨架构）。
- **持久化（可选）**：如需保留 Headroom 状态/CCR 缓存，给 `docker run` 加
  `-v $HOME/.headroom:/home/nonroot/.headroom`（注意容器内以 nonroot/uid 1000 运行，宿主目录需可写）。
- **安全**：proxy 监听 `0.0.0.0` 且默认无鉴权，仅建议可信内网使用。
- 本目录所有文件均为新增，不修改 headroom 上游源码，亦不改 Sashiko 原生 `src/`。
