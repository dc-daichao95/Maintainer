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
#   HEADROOM_IMAGE      容器镜像引用（需与 pull_save.sh 一致，默认 ghcr.io/chopratejas/headroom:latest）
#   HEADROOM_PORT       代理端口（默认 8787）
#   HEADROOM_CONTAINER  容器名（默认 headroom）
#   HEALTH_TIMEOUT      健康检查最长等待秒数（默认 90）

set -euo pipefail

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
docker run -d --name "${NAME}" --restart unless-stopped \
  -p "${PORT}:${PORT}" \
  "${IMAGE}" --host 0.0.0.0 --port "${PORT}"

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
