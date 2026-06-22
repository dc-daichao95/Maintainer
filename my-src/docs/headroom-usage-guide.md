# Kernel-Maintainer Headroom 使用指南

## 1. 目标

本文说明如何在当前 Kernel-Maintainer 框架中使用 Headroom。当前推荐方式是将 Headroom 作为本地上下文压缩代理运行，再通过 Sashiko 已有的 OpenAI-compatible provider 接入。这样可以复用现有 LLM 调用链路，不修改 Sashiko 原生 `src/` 代码，也不把 `headroom-main/` 加入 Cargo workspace。

## 2. 当前集成方式

Kernel-Maintainer 的 LLM 请求链路如下：

1. Sashiko review worker 生成 OpenAI-compatible chat completions 请求。
2. `[ai.openai_compat].base_url` 指向本地 Headroom 代理。
3. Headroom 对上下文进行压缩、缓存与统计。
4. Headroom 将压缩后的请求转发给真实上游 LLM provider。
5. Sashiko 接收 provider 响应并继续原有审查流程。

关键点：
- Sashiko 配置写入 `base_url = "http://<headroom-host>:<headroom-port>/v1"`。
- Sashiko 的 OpenAI-compatible client 会自动规范化到 `/v1/chat/completions`。
- Headroom 默认建议运行在 `127.0.0.1:8787`。
- Headroom 的 `/readyz` 用于部署前健康检查，`/stats` 和 `/metrics` 用于观察压缩收益。

## 3. 推荐路径：通过 bootstrap 托管

### 3.1 前置条件

- Python 3.8+。
- `headroom` CLI 已安装并在 `PATH` 中。
- 目标上游 LLM provider 的 API Key 已按现场环境准备。
- `my-src/tools/bootstrap/config_template.json` 已复制为 `config.json`。

如果本机尚未安装 Headroom，可先安装代理能力：

```bash
pip install "headroom-ai[proxy]"
```

### 3.2 配置 `config.json`

在 `config.json` 中启用 `headroom` 段：

```json
{
  "headroom": {
    "enabled": true,
    "host": "127.0.0.1",
    "port": 8787,
    "mode": "token",
    "backend": "openrouter",
    "telemetry": false,
    "startup_timeout_secs": 20
  }
}
```

字段说明：
- `enabled`: 是否启用 Headroom。默认 `false`，不会影响旧部署。
- `host`: Headroom 监听地址，默认 `127.0.0.1`。
- `port`: Headroom 监听端口，默认 `8787`。
- `mode`: Headroom 优化模式，当前默认 `token`。
- `backend`: Headroom 上游后端，需按现场实际 provider 调整。
- `telemetry`: 是否开启 Headroom 遥测，默认 `false`。
- `startup_timeout_secs`: bootstrap 等待 `/readyz` 就绪的最长秒数。

### 3.3 运行部署

在 bootstrap 目录运行：

```bash
cd my-src/tools/bootstrap
python deploy.py --config config.json
```

启用 Headroom 后，部署脚本会：

1. 解析并校验 `headroom` 配置。
2. 先访问 `http://<host>:<port>/readyz`，复用已就绪的代理。
3. 如代理未就绪，启动 `headroom proxy`。
4. 等待 `/readyz` 返回成功。
5. 只有 Headroom ready 后，才写入 Sashiko 的 OpenAI-compatible 配置。

如果 Headroom 未能就绪，部署会停止，不会静默切换到未压缩链路。

### 3.4 自动写入的 Sashiko 配置

Headroom ready 后，bootstrap 会写入或更新 `Settings.toml`：

```toml
[ai]
provider = "openai-compatible"
model = "gpt-4o"
api_timeout_secs = 300

[ai.openai_compat]
base_url = "http://127.0.0.1:8787/v1"
streaming = true
stream_idle_timeout_secs = 240
```

`model`、`api_timeout_secs`、`streaming` 和 `stream_idle_timeout_secs` 来自 `config.json` 的 `app_config.ai`。如果现场 provider 或网关不支持 streaming，应将 `streaming` 设置为 `false`。

## 4. 备选路径：手工配置

如果不使用 bootstrap，也可以手工运行 Headroom 并配置 Sashiko。

### 4.1 启动 Headroom

```bash
headroom proxy --host 127.0.0.1 --port 8787 --mode token --backend openrouter --no-telemetry
```

确认代理就绪：

```bash
curl http://127.0.0.1:8787/readyz
```

### 4.2 修改 `Settings.toml`

将 Sashiko provider 指向 Headroom：

```toml
[ai]
provider = "openai-compatible"
model = "gpt-4o"
api_timeout_secs = 300

[ai.openai_compat]
base_url = "http://127.0.0.1:8787/v1"
streaming = true
stream_idle_timeout_secs = 240
```

手工方式不会自动检查 Headroom 是否 ready，也不会自动保护配置写入顺序，因此生产部署优先使用 bootstrap。

## 5. 验证方式

### 5.1 健康检查

```bash
curl http://127.0.0.1:8787/readyz
```

返回成功表示 Headroom 可接收请求。

### 5.2 压缩收益

运行一次 Sashiko review 后查看：

```bash
curl http://127.0.0.1:8787/stats
```

也可以接入 Prometheus：

```bash
curl http://127.0.0.1:8787/metrics
```

### 5.3 Sashiko 路径确认

检查 `Settings.toml`：

```toml
[ai.openai_compat]
base_url = "http://127.0.0.1:8787/v1"
```

如果 `base_url` 指向真实 provider 而不是 Headroom，本次审查不会经过 Headroom 压缩。

## 6. 故障排查

### 6.1 `headroom` 命令不存在

现象：bootstrap 输出 Headroom command not found。

处理：
- 安装 Headroom CLI。
- 确认 `headroom` 在当前 shell 的 `PATH` 中。
- 重新运行 `python deploy.py --config config.json`。

### 6.2 `/readyz` 超时

现象：bootstrap 等待 Headroom ready 超时。

处理：
- 检查端口是否被占用。
- 检查 Headroom 上游 provider 凭据。
- 检查 `headroom.backend` 是否与现场 provider 匹配。
- 提高 `startup_timeout_secs` 后重试。

### 6.3 请求没有经过 Headroom

现象：`/stats` 没有新增记录。

处理：
- 确认 `Settings.toml` 中 provider 是 `openai-compatible`。
- 确认 `[ai.openai_compat].base_url` 指向 `http://<host>:<port>/v1`。
- 确认运行中的 Sashiko 使用的是同一份 `Settings.toml`。

### 6.4 需要回滚

处理方式：
- 将 `config.json` 中 `headroom.enabled` 改为 `false`。
- 恢复原有 LLM provider 配置。
- 重启 Sashiko。

## 7. 安全与边界

- 不提交 API Key，bootstrap 日志不得输出 API Key。
- Headroom telemetry 默认关闭。
- `headroom-main/` 是已下载的上游参考源码，不作为本项目 workspace 成员。
- 本集成不修改 Sashiko 原生 `src/` 目录。
- 如未来需要把 Headroom 统计接入 Web UI，应单独创建 spec。

## 8. 相关文档

- `my-src/docs/spec/spec-00009-headroom-context-compression.md`
- `my-src/tools/bootstrap/README.md`
- `my-src/tools/bootstrap/config_template.json`
- `docs/llm-providers.md`
- `docs/examples/Settings.openai-compat.toml`
