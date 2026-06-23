#!/usr/bin/env bash
#
# 在离线 Linux 上把分卷合并回 headroom-offline.tar.gz 并校验。
# 分卷因 GitHub 单文件 100MB 上限而切分；本脚本负责还原。
#
# 用法：
#   ./reassemble.sh
#   之后：docker load -i headroom-offline.tar.gz
#         （或 ../load_run.sh headroom-offline.tar.gz）

set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")"

echo "==> 合并分卷 headroom-offline.tar.gz.part* ..."
cat headroom-offline.tar.gz.part* > headroom-offline.tar.gz

if command -v sha256sum >/dev/null 2>&1; then
  echo "==> 校验 sha256 ..."
  sha256sum -c headroom-offline.tar.gz.sha256
else
  echo "WARN: 未找到 sha256sum，跳过校验。"
fi

echo "[OK] headroom-offline.tar.gz 已就绪"
echo "下一步：docker load -i headroom-offline.tar.gz"
echo "    或：../load_run.sh headroom-offline.tar.gz"
