#!/usr/bin/env bash
#
# 在离线 Linux（x86_64）上运行：从 tar 加载 Headroom 镜像并以常驻容器启动 proxy，
# 监听 0.0.0.0:${HEADROOM_PORT}，开机随 Docker 守护进程自启。幂等、可重入。
#
# 用法：
#   ./load_run.sh [镜像tar路径]
#   HEADROOM_PORT=9000 ./load_run.sh headroom-offline.tar
#
# 环境变量：
#   HEADROOM_IMAGE         容器镜像引用（需与 pull_save.sh 一致，默认 ghcr.io/chopratejas/headroom:latest）
#   HEADROOM_PORT          代理端口（默认 8787）
#   HEADROOM_CONTAINER     容器名（默认 headroom）
#   HEALTH_TIMEOUT         健康检查最长等待秒数（默认 90）
#   OPENAI_TARGET_API_URL  上游 OpenAI 兼容 LLM 的 base 地址（不带 /v1，Headroom 自动拼 /v1/chat/completions）
#   HEADROOM_NETWORK       可选 docker 网络模式（如 host），用于容器需共享宿主机网络才能访问上游
#
# 以上参数可集中写入同目录的 headroom.env（key=value），本脚本会自动读取；见 headroom.env.example。

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# 读取可选配置文件，集中管理上游地址、网络模式等参数。
if [ -f "${SCRIPT_DIR}/headroom.env" ]; then
  set -a
  # shellcheck disable=SC1091
  . "${SCRIPT_DIR}/headroom.env"
  set +a
fi

TAR="${1:-headroom-offline.tar}"
IMAGE="${HEADROOM_IMAGE:-ghcr.io/chopratejas/headroom:latest}"
PORT="${HEADROOM_PORT:-8787}"
NAME="${HEADROOM_CONTAINER:-headroom}"
HEALTH_TIMEOUT="${HEALTH_TIMEOUT:-90}"

command -v docker >/dev/null 2>&1 || {
  echo "ERROR: 未找到 docker。请先在该离线机器上安装 Docker 引擎。" >&2
  exit 1
}
[ -f "${TAR}" ] || {
  echo "ERROR: 找不到镜像 tar：${TAR}" >&2
  exit 1
}

echo "==> 加载镜像：${TAR}"
docker load -i "${TAR}"

echo "==> (重新)启动常驻容器 ${NAME}，端口 ${PORT}"
docker rm -f "${NAME}" >/dev/null 2>&1 || true
# 离线必需环境变量：
#   HEADROOM_SKIP_UPSTREAM_CHECK=1 —— /readyz 默认会探测上游 LLM，气隙环境探测失败会
#     导致容器一直 unhealthy；跳过它才能正常就绪。
#   HF_HUB_OFFLINE / TRANSFORMERS_OFFLINE / HEADROOM_UPDATE_CHECK —— 阻止后台联网拉
#     模型/查更新，避免离线下的噪声与超时（不影响就绪，仅图干净）。
run_args=(run -d --name "${NAME}" --restart unless-stopped)
if [ -n "${HEADROOM_NETWORK:-}" ]; then
  run_args+=(--network "${HEADROOM_NETWORK}")   # host 模式下 -p 端口映射无意义，故省略
else
  run_args+=(-p "${PORT}:${PORT}")
fi
run_args+=(-e HEADROOM_SKIP_UPSTREAM_CHECK=1 -e HF_HUB_OFFLINE=1 \
  -e TRANSFORMERS_OFFLINE=1 -e HEADROOM_UPDATE_CHECK=off)
# 上游 OpenAI 兼容 LLM 地址：仅在配置了才注入，避免覆盖镜像默认值。
if [ -n "${OPENAI_TARGET_API_URL:-}" ]; then
  run_args+=(-e "OPENAI_TARGET_API_URL=${OPENAI_TARGET_API_URL}")
  echo "    上游(OPENAI_TARGET_API_URL): ${OPENAI_TARGET_API_URL}"
else
  echo "    警告：未配置 OPENAI_TARGET_API_URL，Headroom 将打默认 api.openai.com（离线会 502）。" >&2
fi
run_args+=("${IMAGE}" --host 0.0.0.0 --port "${PORT}")
docker "${run_args[@]}"

echo "==> 等待 /readyz 就绪（最多 ${HEALTH_TIMEOUT}s）"
elapsed=0
while [ "${elapsed}" -lt "${HEALTH_TIMEOUT}" ]; do
  if command -v curl >/dev/null 2>&1; then
    curl --fail --silent "http://127.0.0.1:${PORT}/readyz" >/dev/null 2>&1 && {
      echo "✓ Headroom proxy 已就绪：http://0.0.0.0:${PORT}"
      exit 0
    }
  elif (echo >"/dev/tcp/127.0.0.1/${PORT}") >/dev/null 2>&1; then
    echo "✓ 端口 ${PORT} 已监听：http://0.0.0.0:${PORT}"
    exit 0
  fi
  sleep 2
  elapsed=$((elapsed + 2))
done

echo "ERROR: proxy 在 ${HEALTH_TIMEOUT}s 内未就绪，容器日志：" >&2
docker logs --tail 50 "${NAME}" >&2 || true
exit 1
