#!/usr/bin/env bash
# 清理项目缓存与编译产物。
#
# 用法：
#   bash scripts/clean-cache.sh              # 默认：安全清理（仅清增量缓存与超过 7 天未访问的产物）
#   bash scripts/clean-cache.sh --deep       # 深度清理（cargo clean + 删 dist/.next/node_modules 缓存）
#   bash scripts/clean-cache.sh --nuke       # 核弹级（含 node_modules、pnpm store prune）
#   bash scripts/clean-cache.sh --dry-run    # 只打印将被清理的体积，不真正删除
#
# 任意模式下都会先打印 before/after 体积对比。

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
TAURI_DIR="${ROOT_DIR}/apps/desktop/src-tauri"

MODE="safe"
DRY_RUN=0

for arg in "$@"; do
  case "$arg" in
    --deep) MODE="deep" ;;
    --nuke) MODE="nuke" ;;
    --dry-run) DRY_RUN=1 ;;
    -h|--help)
      sed -n '2,12p' "$0"; exit 0 ;;
    *) echo "未知参��: $arg" >&2; exit 1 ;;
  esac
done

human_size() { du -sh "$1" 2>/dev/null | awk '{print $1}'; }

print_sizes() {
  local label="$1"
  echo "── ${label} ──"
  for p in \
    "${TAURI_DIR}/target" \
    "${ROOT_DIR}/node_modules" \
    "${ROOT_DIR}/apps/desktop/node_modules" \
    "${ROOT_DIR}/apps/desktop/dist" \
    "${ROOT_DIR}/apps/website/.next" \
    "${ROOT_DIR}/apps/website/node_modules"; do
    [ -e "$p" ] && printf "  %6s  %s\n" "$(human_size "$p")" "${p#${ROOT_DIR}/}"
  done
  echo
}

run() {
  if [ "$DRY_RUN" -eq 1 ]; then
    echo "  [dry-run] $*"
  else
    echo "  > $*"
    eval "$@"
  fi
}

echo "模式: ${MODE}$([ $DRY_RUN -eq 1 ] && echo ' (dry-run)')"
echo
print_sizes "清理前"

case "$MODE" in
  safe)
    echo "→ 清 Rust 增量编译缓存"
    run "rm -rf '${TAURI_DIR}/target/debug/incremental' '${TAURI_DIR}/target/release/incremental'"

    echo "→ 清 7 天未访问的 cargo 产物（需 cargo-sweep）"
    if command -v cargo-sweep >/dev/null 2>&1; then
      run "cargo sweep --time 7 '${TAURI_DIR}'"
    else
      echo "  [skip] cargo-sweep 未安装；可执行 'cargo install cargo-sweep' 后再跑"
    fi

    echo "→ 清前端构建产物"
    run "rm -rf '${ROOT_DIR}/apps/desktop/dist' '${ROOT_DIR}/apps/website/.next'"

    echo "→ 清 tsbuildinfo"
    run "find '${ROOT_DIR}/apps' -name '*.tsbuildinfo' -type f -delete"
    ;;

  deep)
    echo "→ cargo clean"
    if [ -f "${TAURI_DIR}/Cargo.toml" ]; then
      run "cargo clean --manifest-path '${TAURI_DIR}/Cargo.toml'"
    fi

    echo "→ 清前端构建产物 + 测试缓存"
    run "rm -rf '${ROOT_DIR}/apps/desktop/dist' '${ROOT_DIR}/apps/website/.next'"
    run "find '${ROOT_DIR}/apps' -name '*.tsbuildinfo' -type f -delete"
    run "find '${ROOT_DIR}' -type d -name '.vitest-cache' -prune -exec rm -rf {} +"
    run "find '${ROOT_DIR}' -type d -name 'coverage' -not -path '*/node_modules/*' -prune -exec rm -rf {} +"

    echo "→ 清 playwright 缓存（如有）"
    run "rm -rf '${ROOT_DIR}/apps/desktop/test-results' '${ROOT_DIR}/apps/desktop/playwright-report'"
    ;;

  nuke)
    echo "⚠️  核弹模式：将删除所有 node_modules 与 cargo target"
    if [ "$DRY_RUN" -eq 0 ]; then
      read -r -p "确认继续？输入 yes 回车: " confirm
      [ "$confirm" = "yes" ] || { echo "已取消"; exit 0; }
    fi

    run "cargo clean --manifest-path '${TAURI_DIR}/Cargo.toml'"
    run "find '${ROOT_DIR}' -type d -name 'node_modules' -prune -exec rm -rf {} +"
    run "rm -rf '${ROOT_DIR}/apps/desktop/dist' '${ROOT_DIR}/apps/website/.next'"
    run "find '${ROOT_DIR}/apps' -name '*.tsbuildinfo' -type f -delete"

    if command -v pnpm >/dev/null 2>&1; then
      echo "→ pnpm store prune"
      run "pnpm store prune"
    fi
    ;;
esac

echo
print_sizes "清理后"
echo "完成。"
