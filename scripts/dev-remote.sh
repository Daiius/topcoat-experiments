#!/usr/bin/env bash
# Next.js と topcoat の開発サーバーを、Cloudflare Tunnel 向けの固定ポートで同時に起動する。
#   ./scripts/dev-remote.sh          # 両方起動（foreground。Ctrl-C で両方止まる）
#   ./scripts/dev-remote.sh check    # 設定とポートの空きを表示するだけ
# 設定は .env.remote（.env.remote.example を参照）。
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
env_file="$root/.env.remote"
[ -f "$env_file" ] || { echo "error: $env_file がない（.env.remote.example をコピーして埋める）" >&2; exit 1; }
set -a; . "$env_file"; set +a
: "${NEXTJS_PORT:?}" "${TOPCOAT_PORT:?}" "${NEXTJS_PUBLIC_HOST:?}" "${TOPCOAT_PUBLIC_HOST:?}"

port_busy() { ss -Hltn "sport = :$1" | grep -q .; }

echo "Next.js : https://$NEXTJS_PUBLIC_HOST  -> 127.0.0.1:$NEXTJS_PORT"
echo "topcoat : https://$TOPCOAT_PUBLIC_HOST -> 127.0.0.1:$TOPCOAT_PORT"

busy=0
for p in "$NEXTJS_PORT" "$TOPCOAT_PORT"; do
  if port_busy "$p"; then echo "  port $p: 使用中"; busy=1; else echo "  port $p: 空き"; fi
done
[ "${1:-}" = check ] && exit "$busy"
# topcoat dev はポートが埋まっていると黙って次の番号へずれ、tunnel の向き先と食い違う。先に止める。
[ "$busy" -eq 0 ] || { echo "error: 使用中のポートがある" >&2; exit 1; }

pids=()
cleanup() { trap - INT TERM EXIT; kill "${pids[@]}" 2>/dev/null || true; wait 2>/dev/null || true; }
trap cleanup INT TERM EXIT

prefix() { sed -u "s/^/[$1] /"; }

(
  cd "$root/nextjs"
  # next.config.ts が NEXT_ALLOWED_DEV_ORIGIN を allowedDevOrigins に入れる
  NEXT_ALLOWED_DEV_ORIGIN="$NEXTJS_PUBLIC_HOST" exec pnpm dev --hostname 127.0.0.1 --port "$NEXTJS_PORT"
) > >(prefix next) 2>&1 &
pids+=($!)

(
  cd "$root/topcoat"
  HOST=127.0.0.1 PORT="$TOPCOAT_PORT" exec topcoat dev
) > >(prefix topcoat) 2>&1 &
pids+=($!)

# どちらかが落ちたら、もう片方も止めて終わる
wait -n "${pids[@]}"
echo "一方のサーバーが終了したので停止する" >&2
