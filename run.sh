#!/usr/bin/env bash
# 连锁药店拆零药品系统 —— 一条本地命令启动前后端
#
#   ./run.sh          交付模式: 构建 Svelte, 由 Axum 单进程托管 (http://localhost:8080)
#   ./run.sh dev      开发模式: 后端 8080 + Vite 热更新 5173 (/api 自动代理)
#
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT"

# 保证非交互 shell 也能找到 cargo
if ! command -v cargo >/dev/null 2>&1 && [ -f "$HOME/.cargo/env" ]; then
  # shellcheck disable=SC1091
  source "$HOME/.cargo/env"
fi

export DATABASE_URL="${DATABASE_URL:-sqlite://data/pharmacy.db?mode=rwc}"
export PORT="${PORT:-8080}"

mode="${1:-serve}"

if [ "$mode" = "dev" ]; then
  echo "▶ 开发模式: Axum :8080 + Vite :5173"
  [ -d frontend/node_modules ] || (cd frontend && npm install)
  (cd frontend && npm run dev) &
  VITE_PID=$!
  trap 'kill $VITE_PID 2>/dev/null || true' EXIT
  exec cargo run --manifest-path backend/Cargo.toml
fi

echo "▶ 交付模式: 构建前端并由 Axum 单进程托管 → http://localhost:${PORT}"
command -v cargo >/dev/null 2>&1 || { echo "缺少 Rust 工具链, 请先安装: https://rustup.rs"; exit 1; }
command -v npm  >/dev/null 2>&1 || { echo "缺少 Node.js/npm"; exit 1; }

[ -d frontend/node_modules ] || (cd frontend && npm install)
(cd frontend && npm run build)

# cwd 保持在仓库根: backend/src/main.rs 会托管 frontend/dist, 数据库落在 ./data/
exec cargo run --manifest-path backend/Cargo.toml
