#!/usr/bin/env bash
#
# 在「有 Docker + 能联网」的 x86_64 机器上运行：拉取官方 Headroom 镜像并打包成 tar，
# 供拷贝到离线 Linux 使用。
#
# 用法：
#   ./pull_save.sh [输出tar路径]
#   HEADROOM_IMAGE=ghcr.io/chopratejas/headroom:v0.26.0 ./pull_save.sh headroom-offline.tar
#
# 环境变量：
#   HEADROOM_IMAGE  要拉取的镜像（默认 ghcr.io/chopratejas/headroom:latest）

set -euo pipefail

IMAGE="${HEADROOM_IMAGE:-ghcr.io/chopratejas/headroom:latest}"
OUT="${1:-headroom-offline.tar}"

command -v docker >/dev/null 2>&1 || {
  echo "ERROR: 未找到 docker。请在装有 Docker 的联网机器上运行本脚本。" >&2
  exit 1
}

echo "==> 拉取镜像：${IMAGE}"
docker pull "${IMAGE}"

echo "==> 打包到：${OUT}"
docker save "${IMAGE}" -o "${OUT}"

echo "✓ 已保存 ${IMAGE} -> ${OUT}"
echo "  下一步：把 ${OUT} 拷到离线 Linux，运行 load_run.sh"
