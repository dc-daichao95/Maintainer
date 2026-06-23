# Headroom Linux 一键部署（原生直接集成）设计

- 日期：2026-06-23
- 状态：已确认，进入实现
- 范围：为 headroom 增加一个面向 Linux 的「一键、本地源码安装、systemd 常驻」原生部署入口（**不使用 Docker**）

## 背景与动机

headroom 现有 Linux 部署路径分散：`pip install` 后还需 `headroom install apply --preset ...`；
预设多、选择成本高。缺少「从本地源码 → proxy 作为 systemd 常驻服务跑起来」的单条命令。

本设计提供一个最小、零侵入、可重入的原生一键脚本：从**本地源码**安装 headroom，
注册 systemd 系统服务常驻运行 proxy，监听 `0.0.0.0:8787`，开机自启。

> 注：headroom 自带的 `headroom install apply --preset persistent-service` 把代理硬编码为
> `--host 127.0.0.1`（`headroom/install/planner.py:151,178`），无法绑定 `0.0.0.0`。
> 为满足「局域网可访问」又不修改原生代码，本方案使用自包含的 systemd unit。

## 已确认的关键决策

| 决策点 | 选择 |
|---|---|
| 运行时形态 | 原生（非 Docker） |
| 安装来源 | 本地源码（`pip install ".[proxy,code]"`，不从 PyPI 拉取） |
| 安装方式 | 系统 pip |
| 功能扩展 | `[proxy,code]`（与上游 Dockerfile 默认一致） |
| 进程管理/自启 | systemd 系统服务（需 root/sudo） |
| 服务运行身份 | root |
| 监听地址 | `0.0.0.0:8787` |
| Rust 工具链缺失 | 报错并给安装指引，不自动安装 |
| 代码落点 | 仓库顶层隔离目录 `deploy/` |

## 目录结构（全部 `deploy/` 下，零侵入 headroom 源码）

```
deploy/
├── deploy-linux.sh           # 一键入口（幂等、可重入）
├── headroom.service.template # systemd unit 模板（占位符运行时替换）
├── README.md                 # 原生用法 + 安全提示
└── .gitattributes            # *.sh eol=lf
```

## 组件设计

### deploy/headroom.service.template

systemd unit 模板，占位符 `__HEADROOM_BIN__` / `__PORT__` 在安装时替换：

- `ExecStart=__HEADROOM_BIN__ proxy --host 0.0.0.0 --port __PORT__`
- `Restart=on-failure`，`RestartSec=5`
- `After/Wants=network-online.target`
- `WantedBy=multi-user.target`（系统服务，默认 root 运行）

### deploy/deploy-linux.sh

`set -euo pipefail`，幂等。变更系统状态的命令（install/uninstall/restart）需 root。

子命令：`install`(默认) / `uninstall` / `status` / `logs` / `restart` / `--help`。
环境变量：`HEADROOM_PORT`(8787)、`HEALTH_TIMEOUT`(90)、`HEADROOM_PIP_BREAK_SYSTEM`(应对 PEP 668)。

`install` 流程：

1. **root 检查**：非 root 提示用 sudo，退出。
2. **环境自检**：`python3`(≥3.10)、`pip`、C 编译器(`cc`/`gcc`)、`cargo`。任一缺失 → 清晰报错 +
   安装指引，非零退出；**不自动安装任何依赖**。
3. **本地源码安装**：`python3 -m pip install "<repo>/headroom-main/headroom-main[proxy,code]"`。
   遇 PEP 668「externally-managed」失败时，提示用 `HEADROOM_PIP_BREAK_SYSTEM=1` 重试
   （追加 `--break-system-packages`）。
4. **解析 headroom 可执行路径**，渲染模板 → `/etc/systemd/system/headroom.service`。
5. `systemctl daemon-reload` → `enable --now headroom`。
6. **健康验证**：轮询 `http://127.0.0.1:${PORT}/readyz`，默认超时 90s；成功打印接入提示，
   失败 `journalctl -u headroom` dump + 非零退出。

`uninstall`：`disable --now` → 删除 unit → `daemon-reload`（不卸载 Python 包，避免误伤）。

## 错误处理

顶层 `set -euo pipefail`；每步失败给可操作建议；健康检查独立超时避免无限阻塞；端口做数值校验。

## 安全考量

监听 `0.0.0.0` 且 proxy 默认无鉴权、服务以 root 运行。README 明确提示：仅可信内网使用，
公网务必加防火墙/带鉴权反向代理。

## 测试策略

bash 函数化，便于桩测：参数解析、缺依赖报错路径（PATH 打桩）、健康轮询分支。
`bash -n` 语法检查与 shellcheck（若可用）作为静态门禁。真机 systemd 行为需在 Linux 上验证。

## 与上游的兼容性

所有改动均为 `deploy/` 下新增文件，不修改 headroom 任何现有源码或基础设施，可随上游平滑同步。
复用 headroom 自带的 `pyproject.toml` extras 与 `headroom proxy` CLI，无飞线。
