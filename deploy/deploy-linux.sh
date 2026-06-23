#!/usr/bin/env bash
#
# Headroom Linux 一键部署脚本。
#
# 从本地源码构建 Docker 镜像（绝不 pull），以 docker compose +
# restart: unless-stopped 常驻运行 proxy，监听 0.0.0.0:${HEADROOM_PORT}。
# 幂等、可重入：重复执行只会重建/重启，不会产生副本。
#
# 用法：
#   ./deploy-linux.sh [up|down|status|logs|rebuild]
#   HEADROOM_PORT=9000 ./deploy-linux.sh up
#
# 环境变量：
#   HEADROOM_PORT        代理监听端口（默认 8787）
#   HEADROOM_HOST_HOME   状态持久化的宿主机 HOME（默认 $HOME）
#   HEALTH_TIMEOUT       健康检查最长等待秒数（默认 90）

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
COMPOSE_FILE="${SCRIPT_DIR}/docker-compose.yml"
SERVICE="proxy"

HEADROOM_PORT="${HEADROOM_PORT:-8787}"
HEADROOM_HOST_HOME="${HEADROOM_HOST_HOME:-${HOME:?HOME is not set}}"
HEALTH_TIMEOUT="${HEALTH_TIMEOUT:-90}"
export HEADROOM_PORT HEADROOM_HOST_HOME

# ---- 输出辅助 ----
info() { printf '==> %s\n' "$*"; }
warn() { printf 'WARN: %s\n' "$*" >&2; }
die() {
  printf 'ERROR: %s\n' "$*" >&2
  exit 1
}

# 校验端口为合法的 1-65535 整数，避免把脏值传给 Docker。
validate_port() {
  [[ "${HEADROOM_PORT}" =~ ^[0-9]+$ ]] || die "HEADROOM_PORT 非法（应为整数）：${HEADROOM_PORT}"
  ((10#${HEADROOM_PORT} >= 1 && 10#${HEADROOM_PORT} <= 65535)) ||
    die "HEADROOM_PORT 超出范围 1-65535：${HEADROOM_PORT}"
}

# 探测可用的 compose 实现：优先 docker compose (v2)，回退 docker-compose (v1)。
# 结果写入全局数组 COMPOSE_CMD，供后续命令复用。
detect_compose() {
  command -v docker >/dev/null 2>&1 ||
    die "未找到 docker。请先安装 Docker：https://docs.docker.com/engine/install/"
  docker version >/dev/null 2>&1 ||
    die "Docker 已安装但当前用户无法访问守护进程（尝试把用户加入 docker 组或用 sudo）。"

  if docker compose version >/dev/null 2>&1; then
    COMPOSE_CMD=(docker compose)
  elif command -v docker-compose >/dev/null 2>&1; then
    COMPOSE_CMD=(docker-compose)
  else
    die "未找到 docker compose 插件或 docker-compose。请安装 Compose v2：https://docs.docker.com/compose/install/"
  fi
}

# 统一的 compose 调用入口，自动带上 -f 指定的部署 compose 文件。
compose() {
  "${COMPOSE_CMD[@]}" -f "${COMPOSE_FILE}" "$@"
}

# 轮询 /readyz，直到就绪或超时。优先用 curl，缺失时回退到 /dev/tcp。
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

# 打印部署成功后的下一步使用提示（含本机 LAN IP 猜测）。
print_next_steps() {
  local lan_ip="<本机IP>"
  if command -v hostname >/dev/null 2>&1; then
    lan_ip="$(hostname -I 2>/dev/null | awk '{print $1}')"
    [[ -n "${lan_ip}" ]] || lan_ip="<本机IP>"
  fi
  cat <<EOF

✓ Headroom proxy 已常驻运行：http://0.0.0.0:${HEADROOM_PORT}

下一步（让 agent 走该代理）：
  Claude:  export ANTHROPIC_BASE_URL="http://${lan_ip}:${HEADROOM_PORT}"
  Codex:   export OPENAI_BASE_URL="http://${lan_ip}:${HEADROOM_PORT}/v1"
  Cursor:  OpenAI Base URL 填 http://${lan_ip}:${HEADROOM_PORT}/v1

管理命令：
  ${0} status     查看状态
  ${0} logs       查看日志
  ${0} down       停止并移除
  ${0} rebuild    重新构建镜像并重启

安全提示：代理监听 0.0.0.0 且默认无鉴权，请仅在可信内网使用。
EOF
}

# 构建本地镜像并常驻启动，最后做健康检查。
cmd_up() {
  validate_port
  info "本地构建镜像（headroom:local，首次较慢，需编译 Rust）..."
  compose build
  info "启动常驻 proxy 容器..."
  compose up -d "${SERVICE}"

  if wait_for_ready; then
    print_next_steps
  else
    warn "proxy 在 ${HEALTH_TIMEOUT}s 内未就绪，输出日志用于诊断："
    compose logs --tail 50 "${SERVICE}" >&2 || true
    die "部署失败：健康检查超时。"
  fi
}

cmd_down() {
  info "停止并移除 proxy..."
  compose down
}

cmd_status() {
  compose ps
}

cmd_logs() {
  compose logs -f --tail 100 "${SERVICE}"
}

# 强制重建镜像（忽略缓存）并重启。
cmd_rebuild() {
  validate_port
  info "无缓存重建镜像..."
  compose build --no-cache
  compose up -d "${SERVICE}"
  if wait_for_ready; then
    print_next_steps
  else
    compose logs --tail 50 "${SERVICE}" >&2 || true
    die "重建后健康检查超时。"
  fi
}

print_help() {
  cat <<EOF
Headroom Linux 一键部署

用法: ${0} [COMMAND]

Commands:
  up        (默认) 本地构建镜像并常驻启动 proxy，做健康检查
  down      停止并移除 proxy 容器
  status    查看容器状态
  logs      跟随查看 proxy 日志
  rebuild   无缓存重建镜像并重启
  --help    显示本帮助

环境变量:
  HEADROOM_PORT       代理端口（默认 8787）
  HEADROOM_HOST_HOME  状态持久化宿主机 HOME（默认 \$HOME）
  HEALTH_TIMEOUT      健康检查超时秒数（默认 90）
EOF
}

main() {
  local cmd="${1:-up}"
  case "${cmd}" in
    --help | -h | help)
      print_help
      return 0
      ;;
  esac

  detect_compose
  case "${cmd}" in
    up) cmd_up ;;
    down) cmd_down ;;
    status) cmd_status ;;
    logs) cmd_logs ;;
    rebuild) cmd_rebuild ;;
    *) die "未知命令：${cmd}（用 --help 查看用法）" ;;
  esac
}

main "$@"
