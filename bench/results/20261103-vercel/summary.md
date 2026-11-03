# Vercel 本番（hnd1）でのブラウザ計測（2026-11-03〜04）

- 対象: Next.js `topcoat-experiments-nextjs.vercel.app`（Git 連携、Node 24）、topcoat `topcoat-experiments.vercel.app`（Container、prebuilt、`ee53515` 時点のイメージ）
- 環境: 国内の回線、ヘッドレス Chromium（playwright-core 1.62 alpha）、1280×800
- スクリプト: `bench/vercel/weight.cjs`、`bench/vercel/warm.cjs`、`bench/vercel/ttfb-series.sh`
- 解釈は `docs/journal.md` の「遷移の仕組みとページごとの初回ロードの重さ」節
- ⚠ topcoat 版はこの時点で素の `<a>`（`topcoat::runtime::link` 未使用）。遷移の数字はフレームワークの比較ではない

## weight.cjs（2 回目。1 回目はコールドスタート混入）

| site | page | ttfb | fcp | dcl | load | reqs | kb | js_n | js_kb | img_n | img_kb | prefetch_n | prefetch_kb |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| next | / | 100 | 196 | 177 | 243 | 22 | 182.6 | 8 | 152.5 | 6 | 14.8 | 6 | 2.5 |
| next | /products | 96 | 164 | 216 | 259 | 31 | 208.1 | 8 | 152.5 | 15 | 35.8 | 6 | 2.7 |
| next | /products/1 | 89 | 220 | 336 | 383 | 18 | 184.9 | 9 | 158.7 | 3 | 9.7 | 4 | 1.4 |
| next | /cart | 118 | 176 | 160 | 225 | 14 | 163.7 | 8 | 152.5 | 0 | 0 | 4 | 1.5 |
| topcoat | / | 97 | 212 | 183 | 185 | 9 | 34.1 | 1 | 15.4 | 6 | 11.4 | 0 | 0 |
| topcoat | /products | 104 | 192 | 190 | 190 | 18 | 52.5 | 1 | 15.4 | 15 | 27.8 | 0 | 0 |
| topcoat | /products/1 | 124 | 220 | 942 | 943 | 6 | 32.5 | 1 | 15.4 | 3 | 8.6 | 0 | 0 |
| topcoat | /cart | 102 | 160 | 158 | 158 | 3 | 20.8 | 1 | 15.4 | 0 | 0 | 0 | 0 |

1 回目の外れ値: topcoat `/` FCP 2432ms、Next.js `/products/1` FCP 516ms。

## warm.cjs（`/products/1`、ms。h1 = 商品名、related = 関連商品 1 件目）

| run | site | phase | h1 | related | reqs | cached | kb |
|---|---|---|---|---|---|---|---|
| 1 | next | A 初回 | 321 | 323 | 22 | 0 | 184.9 |
| 1 | next | B 再訪問 | 54 | 59 | 23 | 10 | 13.4 |
| 1 | next | C クリック | 221 | 222 | 4 | 1 | 5.3 |
| 1 | topcoat | A 初回 | 164 | 945 | 6 | 0 | 32.5 |
| 1 | topcoat | B 再訪問 | 62 | 1344 | 6 | 5 | 4 |
| 1 | topcoat | C クリック | 105 | 889 | 5 | 3 | 9.3 |
| 2 | next | A 初回 | 376 | 382 | 22 | 0 | 185 |
| 2 | next | B 再訪問 | 55 | 57 | 23 | 10 | 13.4 |
| 2 | next | C クリック | 126 | 128 | 4 | 1 | 5.3 |
| 2 | topcoat | A 初回 | 164 | 946 | 6 | 0 | 32.5 |
| 2 | topcoat | B 再訪問 | 53 | 837 | 6 | 5 | 4 |
| 2 | topcoat | C クリック | 84 | 872 | 5 | 3 | 9.3 |
| 3 | next | A 初回 | 215 | 216 | 23 | 0 | 187.9 |
| 3 | next | B 再訪問 | 58 | 59 | 23 | 10 | 10.4 |
| 3 | next | C クリック | 125 | 127 | 4 | 1 | 5.3 |
| 3 | topcoat | A 初回 | 213 | 998 | 6 | 0 | 32.5 |
| 3 | topcoat | B 再訪問 | 63 | 847 | 6 | 5 | 4 |
| 3 | topcoat | C クリック | 62 | 1343 | 5 | 3 | 9.3 |

（C の待ち条件を `waitUntil: 'commit'` に直した後の回。直す前は topcoat の C が load まで待って 870〜900ms になっていた）

## ttfb-series.sh

- `ttfb-series-40.txt`: 温まった状態で 40 回連続（各行 `ttfb total` 秒）。TTFB 中央値 116ms・p90 140ms・最大 179ms、TTFB 後〜完了 806〜844ms
- IDLE=360（無通信 6 分 8 秒をリクエスト記録で確認）の後の 5 回: TTFB 119 / 159 / 127 / 124 / 117ms、完了 923〜964ms。コールドスタートなし
