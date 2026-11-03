#!/usr/bin/env bash
# topcoat 本番の TTFB と完了までの時間を連続で測る（温まった状態の安定性・コールドスタートの確認用）。
#   ./bench/vercel/ttfb-series.sh [回数=40]          # 連続
#   IDLE=360 ./bench/vercel/ttfb-series.sh 5         # IDLE 秒アクセスを止めてから測る
set -euo pipefail
N="${1:-40}"; BASE="${BASE:-https://topcoat-experiments.vercel.app}"
[ -n "${IDLE:-}" ] && { echo "idle ${IDLE}s"; sleep "$IDLE"; }
for i in $(seq 1 "$N"); do
  curl -s -o /dev/null -w "%{time_starttransfer} %{time_total}\n" "$BASE/products/$((i % 20 + 1))"
done | awk '{printf "%d ttfb=%.0f done=%.0f\n", NR, $1*1000, $2*1000}'
