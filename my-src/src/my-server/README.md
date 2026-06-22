# my-server（多服务器管理与远端 Sashiko 聚合代理）

`my-server` 是 Kernel-Maintainer 的本地后端服务，提供一个统一的 Web 看板来纳管多个远端 Sashiko 实例：

- 在本地 SQLite 中持久化「服务器配置」（名称 / IP / Web 端口 / 描述）。
- 作为聚合代理，并发拉取各远端 Sashiko 的真实 JSON API（`/api/stats`、`/api/stats/findings`、`/api/stats/timeline`），汇算出大盘统计。
- 提供静态 Web UI（概览看板 + 服务器配置页），支持整卡点击在新标签页直达远端 Sashiko。

对应特性设计见 `my-src/docs/spec/spec-00007-multi-server-real-data.md`。

## 目录结构

```
src/my-server/
├── main.rs        # 二进制入口（薄壳，调用 lib 的 run_server）
├── lib.rs         # 应用装配：AppState、Axum 路由与 HTTP 处理函数
├── models.rs      # 数据模型 ServerConfig
├── dal.rs         # 数据访问层：ServerRepository 契约 + SqliteRepository
├── aggregator.rs  # 聚合层：RemoteStatsFetcher 契约 + 并发 Fan-out + 缓存
└── webui/         # 前端静态资源（原生 JS + TailwindCSS + ECharts）
```

## 运行前置条件

- 已安装 Rust 工具链（`cargo`）。
- 监听端口可用（默认 `127.0.0.1:3000`，可通过环境变量 `MY_SERVER_PORT` 修改，见下文「自定义端口」）。
- 无需任何 SSH 凭据；服务器配置仅保存 名称/IP/Web端口/描述。

> **重要：必须在 `my-src` 目录下启动。**
> 服务使用相对路径加载静态资源（`src/my-server/webui`）并在当前工作目录创建 SQLite 文件（`servers.db`）。请始终以 `my-src` 作为工作目录运行，否则会出现页面 404 或数据库文件位置异常。

## 启动服务

在 `my-src` 目录下执行：

```bash
cargo run --bin my-server
```

启动后访问：

- Web UI：<http://127.0.0.1:3000/>
  - 概览看板：`#/dashboard`
  - 服务器配置：`#/server-config`
- 首次运行会自动在工作目录创建 `servers.db` 并建表（`servers`）。

### 自定义端口

监听端口通过环境变量 `MY_SERVER_PORT` 配置，未设置时默认 `3000`。非法或越界的值（如非数字、`0`、大于 65535）会被忽略并回退到默认端口，保证服务始终可启动。监听地址固定为 `127.0.0.1`（仅本机回环）。

```bash
# Linux / macOS
MY_SERVER_PORT=8088 cargo run --bin my-server
```

```powershell
# Windows / PowerShell
$env:MY_SERVER_PORT = "8088"
cargo run --bin my-server
```

> 前端 `webui` 通过相对路径请求 `/api/v1/...`，不依赖具体端口，因此修改端口后无需改动前端。

### Windows / PowerShell 提示

PowerShell 使用 `;` 串联命令（不支持 `&&`）。若 `cargo` 不在 PATH 中，可使用完整路径并临时加入 PATH：

```powershell
$env:PATH = "$env:USERPROFILE\.rustup\toolchains\stable-x86_64-pc-windows-msvc\bin;$env:USERPROFILE\.cargo\bin;$env:PATH"
cargo run --bin my-server
```

### 日志级别

通过 `RUST_LOG` 控制日志（默认 `my_server=debug,tower_http=debug`）：

```bash
RUST_LOG=my_server=info cargo run --bin my-server
```

## 使用流程

1. 打开 <http://127.0.0.1:3000/#/server-config>，点击「添加服务器」，填写 名称 / 服务器地址(IP) / Web 端口 / 描述 并保存（数据持久化到本地 SQLite）。
2. 切换到「概览」(`#/dashboard`)：
   - 每台服务器以卡片展示名称、在线/离线状态、`IP:Web端口` 与描述。
   - 在线判定：本地后端探测远端 `GET /api/stats` 且返回 `status == "ok"`。
   - 单台远端不可达只会让该卡片标记为「离线」，不会导致整页崩溃。
   - 汇总指标与图表（趋势图、各服务器告警分布、平均准确率）由远端真实数据汇算。
3. 点击任意服务器卡片，在新标签页打开远端 Sashiko：`http://<IP>:<Web端口>`。

## HTTP API

本地命名空间为 `/api/v1/...`（远端 Sashiko 原生端点为 `/api/...`，由聚合层消费）。

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `GET` | `/api/v1/servers` | 获取全部服务器配置 |
| `POST` | `/api/v1/servers` | 新增服务器配置 |
| `PUT` | `/api/v1/servers/:id` | 更新指定服务器配置 |
| `DELETE` | `/api/v1/servers/:id` | 删除指定服务器配置 |
| `GET` | `/api/v1/dashboard/stats` | 返回汇算后的大盘统计与服务器卡片数据 |

### `ServerConfig`

```json
{
  "id": 1,
  "name": "Node-A",
  "ip": "192.168.1.100",
  "web_port": 8080,
  "description": "生产环境核心节点"
}
```

### 示例：新增服务器

```bash
curl -X POST http://127.0.0.1:3000/api/v1/servers \
  -H "Content-Type: application/json" \
  -d '{"name":"Node-A","ip":"192.168.1.100","web_port":8080,"description":"核心节点"}'
```

### 示例：获取大盘统计

```bash
curl http://127.0.0.1:3000/api/v1/dashboard/stats
```

返回 `DashboardStats`：

```json
{
  "total_issues": 0,
  "avg_accuracy": 0.0,
  "online_servers": 0,
  "offline_servers": 1,
  "pie_chart_data": [{ "name": "Node-A", "value": 0 }],
  "trend_data": [{ "day": "2026-06-12", "count": 0 }],
  "servers": [
    {
      "id": 1,
      "name": "Node-A",
      "ip": "192.168.1.100",
      "web_port": 8080,
      "description": "核心节点",
      "online": false
    }
  ]
}
```

> 平均准确率公式：`(to_fix + fixed) / (not_issue + to_fix + fixed)`，分母为 0 时返回 `0.0`（已规避零除）。

## 聚合与容错机制

- **并发 Fan-out**：对每台服务器并发请求 `/api/stats`、`/api/stats/findings`、`/api/stats/timeline`（`futures::join_all` + `tokio::join!`）。
- **超时**：单次远端请求超时阈值 **2 秒**，超时/报错即将该服务器标记为离线。
- **短 TTL 缓存**：聚合结果缓存 **30 秒**，避免请求风暴与单点阻塞。
- **可演进**：数据来源经 `RemoteStatsFetcher` 契约抽象，未来可平滑切换为「后台定时拉取并落库」的本地检索模式（见 `my-src/docs/adr/adr-00002-multi-server-local-cache.md`），对前端 API 契约无感。

## 测试

后端单元 / 集成测试（在 `my-src` 目录执行）：

```bash
cargo test
```

前端端到端测试（Playwright，需先安装浏览器）：

```bash
npx playwright install
npx playwright test tests/e2e/multi_server.spec.js
```

## 配置约定

| 项 | 取值 | 配置方式 / 位置 |
| --- | --- | --- |
| 监听地址 | `127.0.0.1` | 硬编码于 `lib.rs` `run_server` |
| 监听端口 | 环境变量 `MY_SERVER_PORT`，默认 `3000` | `lib.rs` `resolve_port`（常量 `PORT_ENV` / `DEFAULT_PORT`） |
| 日志级别 | 环境变量 `RUST_LOG`，默认 `my_server=debug,tower_http=debug` | `lib.rs` `run_server` |
| SQLite 数据库 | `servers.db`（工作目录） | 硬编码于 `lib.rs` `run_server` |
| 静态资源目录 | `src/my-server/webui`（相对工作目录） | 硬编码于 `lib.rs` `run_server` |
| 远端请求超时 | 2 秒 | `aggregator.rs` `HttpStatsFetcher::new` |
| 聚合缓存 TTL | 30 秒 | `aggregator.rs` `AggregationService::aggregate` |

端口与日志级别可通过上述环境变量在运行时配置；数据库路径、静态目录与缓存阈值目前仍需改动 `lib.rs` / `aggregator.rs` 中的对应代码。
