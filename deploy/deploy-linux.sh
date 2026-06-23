#!/usr/bin/env bash
#
# Headroom Linux 一键部署脚本（原生直接集成，无 Docker）。
#
# 从本地源码安装 headroom（绝不从 PyPI 拉取），注册 systemd 系统服务常驻运行
# proxy，监听 0.0.0.0:${HEADROOM_PORT}，开机自启。幂等、可重入。
#
# 用法：
#   sudo ./deploy-linux.sh [install|uninstall|status|logs|restart]
#   sudo HEADROOM_PORT=9000 ./deploy-linux.sh install
#
# 环境变量：
#   HEADROOM_PORT              代理监听端口（默认 8787）
#   HEALTH_TIMEOUT            健康检查最长等待秒数（默认 90）
#   HEADROOM_PIP_BREAK_SYSTEM 设为 1 时给 pip 追加 --break-system-packages（应对 PEP 668）

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SOURCE_DIR="$(cd "${SCRIPT_DIR}/../headroom-main/headroom-main" 2>/dev/null && pwd || true)"
TEMPLATE_FILE="${SCRIPT_DIR}/headroom.service.template"
SERVICE_NAME="headroom"
UNIT_PATH="/etc/systemd/system/${SERVICE_NAME}.service"

HEADROOM_PORT="${HEADROOM_PORT:-8787}"
HEALTH_TIMEOUT="${HEALTH_TIMEOUT:-90}"

# ---- 输出辅助 ----
info() { printf '==> %s\n' "$*"; }
warn() { printf 'WARN: %s\n' "$*" >&2; }
die() {
  printf 'ERROR: %s\n' "$*" >&2
  exit 1
}

require_root() {
  [[ "$(id -u)" -eq 0 ]] ||
    die "该操作需要 root 权限，请用 sudo 重新运行：sudo ${0} ${1:-install}"
}

validate_port() {
  [[ "${HEADROOM_PORT}" =~ ^[0-9]+$ ]] || die "HEADROOM_PORT 非法（应为整数）：${HEADROOM_PORT}"
  ((10#${HEADROOM_PORT} >= 1 && 10#${HEADROOM_PORT} <= 65535)) ||
    die "HEADROOM_PORT 超出范围 1-65535：${HEADROOM_PORT}"
}

# 逐项检查构建/安装前置依赖，缺失则给出针对性安装指引后退出。
check_prerequisites() {
  [[ -n "${SOURCE_DIR}" && -f "${SOURCE_DIR}/pyproject.toml" ]] ||
    die "未找到本地 headroom 源码（期望在 ${SCRIPT_DIR}/../headroom-main/headroom-main）。"

  command -v python3 >/dev/null 2>&1 || die "缺少 python3。请安装 Python 3.10+。"
  python3 -c 'import sys; raise SystemExit(0 if sys.version_info >= (3, 10) else 1)' ||
    die "Python 版本过低，需 3.10+（当前 $(python3 -V 2>&1))。"
  python3 -m pip --version >/dev/null 2>&1 ||
    die "缺少 pip。请安装：apt-get install -y python3-pip （或对应发行版包）。"

  command -v cc >/dev/null 2>&1 || command -v gcc >/dev/null 2>&1 ||
    die "缺少 C 编译器。请安装：apt-get install -y build-essential（或 gcc）。"
  command -v cargo >/dev/null 2>&1 ||
    die "缺少 Rust 工具链（cargo），编译 headroom 核心所必需。请安装：
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh && rustup default stable
然后重新运行本脚本。"
}

# 从本地源码安装 headroom[proxy,code]；按需追加 --break-system-packages。
install_headroom_package() {
  local pip_args=(install "${SOURCE_DIR}[proxy,code]")
  if [[ "${HEADROOM_PIP_BREAK_SYSTEM:-0}" == "1" ]]; then
    pip_args+=(--break-system-packages)
  fi

  info "从本地源码安装 headroom[proxy,code]（首次需编译 Rust 核心，约数分钟）..."
  if ! python3 -m pip "${pip_args[@]}"; then
    die "pip 安装失败。若报 'externally-managed-environment'（PEP 668），请用：
  sudo HEADROOM_PIP_BREAK_SYSTEM=1 ${0} install"
  fi
}

# 解析已安装的 headroom 可执行文件绝对路径（覆盖常见安装位置）。
resolve_headroom_bin() {
  local bin
  bin="$(command -v headroom 2>/dev/null || true)"
  if [[ -z "${bin}" ]]; then
    for cand in /usr/local/bin/headroom /usr/bin/headroom /root/.local/bin/headroom; do
      [[ -x "${cand}" ]] && bin="${cand}" && break
    done
  fi
  [[ -n "${bin}" ]] || die "安装后仍未找到 headroom 可执行文件，请检查 pip 安装输出。"
  printf '%s\n' "${bin}"
}

# 用模板渲染 systemd unit 并启用启动服务。
install_service() {
  local headroom_bin="$1"
  [[ -f "${TEMPLATE_FILE}" ]] || die "缺少 systemd 模板：${TEMPLATE_FILE}"

  info "写入 systemd unit：${UNIT_PATH}"
  sed -e "s#__HEADROOM_BIN__#${headroom_bin}#g" \
    -e "s#__PORT__#${HEADROOM_PORT}#g" \
    "${TEMPLATE_FILE}" >"${UNIT_PATH}"

  systemctl daemon-reload
  systemctl enable --now "${SERVICE_NAME}"
}

# 轮询 /readyz，直到就绪或超时。优先 curl，回退 /dev/tcp。
wait_for_ready() {
  local url="http://127.0.0.1:${HEADROOM_PORT}/readyz"
  local elapsed=0
  info "等待 proxy 就绪（最多 ${HEALTH_TIMEOUT}s）：${url}"
  while ((elapsed < HEALTH_TIMEOUT)); do
    if command -v curl >/dev/null 2>&1; then
      curl --fail --silent "${url}" >/dev/null 2>&1 && return 0
    elif (echo >"/dev/tcp/127.0.0.1/${HEADROOM_PORT}") >/dev/null 2>&1; then
      return 0
    fi
    sleep 2
    ((elapsed += 2))
  done
  return 1
}

print_next_steps() {
  local lan_ip="<本机IP>"
  if command -v hostname >/dev/null 2>&1; then
    lan_ip="$(hostname -I 2>/dev/null | awk '{print $1}')"
    [[ -n "${lan_ip}" ]] || lan_ip="<本机IP>"
  fi
  cat <<EOF

✓ Headroom proxy 已作为 systemd 系统服务常驻：http://0.0.0.0:${HEADROOM_PORT}

下一步（让 agent 走该代理）：
  Claude:  export ANTHROPIC_BASE_URL="http://${lan_ip}:${HEADROOM_PORT}"
  Codex:   export OPENAI_BASE_URL="http://${lan_ip}:${HEADROOM_PORT}/v1"
  Cursor:  OpenAI Base URL 填 http://${lan_ip}:${HEADROOM_PORT}/v1

管理命令：
  ${0} status      查看状态
  ${0} logs        查看日志
  sudo ${0} restart   重启服务
  sudo ${0} uninstall 卸载服务（保留已安装的 Python 包）

安全提示：代理监听 0.0.0.0 且默认无鉴权、服务以 root 运行，请仅在可信内网使用。
EOF
}

cmd_install() {
  require_root install
  validate_port
  check_prerequisites
  install_headroom_package
  local headroom_bin
  headroom_bin="$(resolve_headroom_bin)"
  install_service "${headroom_bin}"

  if wait_for_ready; then
    print_next_steps
  else
    warn "proxy 在 ${HEALTH_TIMEOUT}s 内未就绪，输出日志用于诊断："
    journalctl -u "${SERVICE_NAME}" --no-pager -n 50 >&2 || true
    die "部署失败：健康检查超时。"
  fi
}

cmd_uninstall() {
  require_root uninstall
  info "停止并禁用 ${SERVICE_NAME}..."
  systemctl disable --now "${SERVICE_NAME}" 2>/dev/null || true
  if [[ -f "${UNIT_PATH}" ]]; then
    rm -f "${UNIT_PATH}"
    systemctl daemon-reload
  fi
  info "已移除服务（Python 包未卸载，如需清理请手动 pip uninstall headroom-ai）。"
}

cmd_restart() {
  require_root restart
  systemctl restart "${SERVICE_NAME}"
  if wait_for_ready; then
    info "已重启并就绪。"
  else
    journalctl -u "${SERVICE_NAME}" --no-pager -n 50 >&2 || true
    die "重启后健康检查超时。"
  fi
}

cmd_status() {
  systemctl status "${SERVICE_NAME}" --no-pager || true
}

cmd_logs() {
  journalctl -u "${SERVICE_NAME}" -f --no-pager
}

print_help() {
  cat <<EOF
Headroom Linux 一键部署（原生，无 Docker）

用法: ${0} [COMMAND]

Commands:
  install    (默认) 自检 + 本地源码安装 + 注册 systemd 服务 + 健康检查（需 root）
  uninstall  停止禁用并删除 systemd 服务（需 root）
  restart    重启服务（需 root）
  status     查看服务状态
  logs       跟随查看服务日志
  --help     显示本帮助

环境变量:
  HEADROOM_PORT              代理端口（默认 8787）
  HEALTH_TIMEOUT             健康检查超时秒数（默认 90）
  HEADROOM_PIP_BREAK_SYSTEM  =1 时给 pip 追加 --break-system-packages（PEP 668）
EOF
}

main() {
  local cmd="${1:-install}"
  case "${cmd}" in
    --help | -h | help) print_help ;;
    install) cmd_install ;;
    uninstall) cmd_uninstall ;;
    restart) cmd_restart ;;
    status) cmd_status ;;
    logs) cmd_logs ;;
    *) die "未知命令：${cmd}（用 --help 查看用法）" ;;
  esac
}

main "$@"
