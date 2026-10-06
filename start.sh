#!/usr/bin/env bash
# 一条命令启动：构建前端 + 启动后端（后端托管前端静态文件）
set -euo pipefail
cd "$(dirname "$0")"

export PATH="$HOME/.cargo/bin:$PATH"

if [ ! -d web/node_modules ]; then
  echo "==> 安装前端依赖"
  (cd web && npm install)
fi

if [ ! -d web/dist ] || [ "${REBUILD_WEB:-0}" = "1" ]; then
  echo "==> 构建前端"
  (cd web && npm run build)
fi

echo "==> 启动后端 http://127.0.0.1:3000"
cd server
exec cargo run --release
