# Headroom 部署简化：source-local 免 wheelhouse 模式 设计

- 日期：2026-06-23
- 状态：已实现
- 范围：简化 headroom 在 Linux 下的部署，集成进现有一键部署工具 `my-src/tools/bootstrap/deploy.py`

## 背景与动机

`deploy.py` 已是项目的一键部署工具，并已集成 headroom（`source-vendor` 模式：从
`my-src/third_party/headroom/source` 用 **离线 wheelhouse**（`pip --no-index --find-links`）构建
wheel → 装入 conda/venv → 启动 `headroom proxy` → 健康检查 → 写 Settings.toml 指向代理）。

`source-vendor` 适合离线/内网受控环境，但要求**预先在 `wheelhouse_dir` 备齐 maturin 及 proxy
extra 的全部依赖 wheel**，对「一键快速部署」是主要摩擦点。

> 注：探索过独立 `deploy.sh` / Docker compose / systemd 方案，均被否决——最终决定不新增独立
> 脚本，而是把简化能力**集成进既有 `deploy.py`**，保持单一部署入口。

## 决策

| 决策点 | 选择 |
|---|---|
| 集成位置 | 既有 `my-src/tools/bootstrap/deploy.py`（不新增独立脚本） |
| 简化目标 | 免 wheelhouse：新增直接 pip 安装本地源码的路径 |
| 实现方式 | 新增 `install_mode: "source-local"`，加性改动，不动 `source-vendor` |

## 实现

### 配置

`parse_headroom_config` 接受新的 `install_mode` 值 `"source-local"`，并对其校验 source 目录完整性。
非法值（如 `"source_vendor"`）仍被拒绝。`config_template.json` 默认仍为 `source-vendor`。

### HeadroomLocalInstaller

新类，`prepare(config)`：

1. 校验 source 目录、Python ≥ 3.10、`cargo` 存在（编译 `headroom._core` 必需）。
2. 复用 `resolve_build_python_and_command`：有激活 conda 则装入当前环境，否则建并用配置的 venv。
3. 执行 `pip install "<source_dir>[proxy]"`——**不带 `--no-index`**，由 pip 正常解析依赖。
4. 返回 `HeadroomBuildStatus`。

`main()` 按 `install_mode` 分派到 `HeadroomVendorManager` 或 `HeadroomLocalInstaller`，
后续 `ensure_headroom_running` / Settings.toml 写入逻辑不变。

### 重构（行为保持）

抽取模块级 `is_python_310_or_newer`、`read_headroom_source_version`、
`resolve_build_python_and_command`，供两种安装器共用，消除重复。

## 测试

新增 `tests/test_headroom_local.py`（6 个，TDD 先红后绿）：config 接受/拒绝、`pip install`
命令不含 `--no-index`、venv 路径、conda 路径、Python<3.10 与缺 cargo 报错。
全量 `pytest` 68 通过，既有 `source-vendor` 测试无回归。

## 与上游兼容性

仅改动 `my-src/` 下的 `deploy.py` / 测试 / README，未修改 headroom 上游源码快照，
未改 Sashiko 原生 `src/`，可随上游平滑同步。
