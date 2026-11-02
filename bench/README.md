# bench: ローカル docker での並べ比べ

topcoat 版と Next.js 版を**同じマシン・同じリソース制限**の docker コンテナで動かし、応答・配信量・負荷時の挙動を測る。
記事の主軸は「流儀に沿って書いた結果、何が配信されるか」であり、ここでの数値は**補助**として扱う（`AGENTS.md`）。

## 使い方

```bash
export COOKIE_KEY="$(openssl rand -hex 64)"     # topcoat の release ビルドは必須。compose は全サービスで要求する
export DOCKER_BUILDKIT=1
docker build -t storefront-topcoat:bench .                       # リポジトリ直下で実行
docker build -f nextjs/Dockerfile -t storefront-nextjs:bench .
bench/run.sh all                                                 # 結果は bench/results/<timestamp>/
```

- `docker compose -f bench/compose.yml build` でも同じイメージ名でビルドできる。
- サブコマンドは `bench/run.sh --help` を参照。単体でも実行でき、`-o <dir>` で既存の結果ディレクトリに追記できる。
  `bench/run.sh summary -o <dir>` は JSON から `summary.md` を作り直す。
- 依存: bash・curl・jq・oha・docker（compose プラグイン込み）。計算は awk / jq で行う。

## 計測条件

| 項目 | 値 |
|---|---|
| リソース制限 | 両サービスとも `cpus: 1`、`mem_limit: 2g`（swap なし）。Vercel Hobby の 1 vCPU / 2 GB に合わせた |
| 待受 | 両方ともコンテナ内 8080、ホスト側は `127.0.0.1:8081`（topcoat）/ `127.0.0.1:8082`（Next.js） |
| 同時稼働 | **測る側だけを起動し、もう片方は停止する**（`only_app`）。CPU の取り合いを避けるため |
| ウォームアップ | `routes` / `stream` / `load` の前に、全ルートへ 1 回ずつリクエストする |
| 負荷生成 | **ホスト自身が load generator を兼ねる**（oha）。コンテナは 1 CPU に制限するが、oha は制限しない |
| 圧縮 | `routes` は identity・br・gzip を別々に要求。oha は既定（圧縮を受け入れる） |

rootless docker では、cgroup v2 の `cpu` / `memory` コントローラがユーザーに委譲されていないと制限が効かない。
`cat /sys/fs/cgroup/user.slice/user-$(id -u).slice/user@$(id -u).service/cgroup.controllers` に `cpu memory` が出ることを確かめる。

## 何を測るか

| サブコマンド | 内容 | 出力 |
|---|---|---|
| `images` | `docker image inspect` の `.Size`（非圧縮の合計） | `images.json` |
| `coldstart` | `docker stop` → `docker start` → `/` が最初に 200 を返すまで（ポーリング間隔 10ms）。続けて、まだ触っていない `/products/1` の TTFB。既定 5 回 | `coldstart.json(l)` |
| `routes` | 6 ルート（`/`、`/products`、`/products?q=headphones`、`/products/1`、`/cart`、存在しない `/products/9999`）の TTFB・完了時間・サイズ。identity / br / gzip ごとに 5 回の中央値。`Cache-Control` も記録 | `routes.json(l)` |
| `stream` | `/products/1` を `curl --trace-time` で取り、受信データの到着時刻を並べる。50ms 以上の間隔で区切って「バースト」にまとめる | `stream.json(l)` |
| `assets` | 4 ページの HTML から `<script src>`・`<link rel=stylesheet / preload / modulepreload>`・`<img>` を抜き出し、各リソースの非圧縮・圧縮（`br, gzip`）バイト数を合計する | `assets.json(l)` |
| `load` | 各ルートを oha で 2 通り（`-z 20s -c 32` の無制限、`-q 200 --latency-correction` の固定レート）。RPS・p50・p99・ステータス分布。各アプリの開始時（再起動 + ウォームアップ直後）と全負荷の後に `docker stats` のメモリ | `load.json(l)`、`memory.json`、`oha/*.json` |

補足:

- `/products/1` は擬似遅延（レビュー 200ms・関連商品 800ms）を含む。topcoat は両方をストリームで待つ。Next.js は関連商品をビルド時に焼くので、レビューだけを待つ。
  **並列 32 の無制限でも、topcoat の `/products/1` は 1 リクエストあたり 800ms かかるので、RPS の上限は約 40 になる。**これは CPU 性能ではなく設計の差である。
- 存在しない id は、topcoat が 404、Next.js が 200 + `noindex` を返す（D24）。表の status 列で確認できる。
- `assets` の画像は、`srcset` + `sizes` があればブラウザと同じ規則で候補を 1 つ選ぶ（既定はビューポート 1280px・DPR 1。`VIEWPORT` / `DPR` で変更可）。
  `sizes` は `(min-width: Npx)` / `(max-width: Npx)` と `px` / `vw` だけを解釈する。`loading="lazy"` の画像は「eager」列から外す（スクロールしないと読まれないため）。
- `docker stats` のメモリは cgroup の使用量からファイルキャッシュ（inactive_file）を引いた値で、RSS と厳密には一致しない。

## 何を測らないか

- **ブラウザでの体感**（LCP・ハイドレーション・JS の実行時間）。Lighthouse / Playwright は別に回す
- **動的に読まれる JS**（クライアント遷移で取得するチャンク、`next/image` 以外の遅延読み込み）。`assets` は HTML に書かれた参照だけを数える
- **Vercel 上の挙動**（CDN・PPR の静的シェル配信・Fluid compute のスケール）。ここの `coldstart` はイメージ取得を含まず、OS のページキャッシュも温まっている。
  Vercel のコールドスタートとは別物で、「プロセスが起動して応答できるまで」の目安にすぎない
- live 更新（WebSocket / SSE）の接続維持コスト、カートの POST
- 複数インスタンス・ネットワーク越しの遅延

## 公式 `benchmarks/` との違い

topcoat の公式 `benchmarks/`（v0.11.0）は、HTML を返す速さだけを比べるスループット比較である。

| | 公式 `benchmarks/` | この `bench/` |
|---|---|---|
| ルート | 3 ルート、インメモリ | 6 ルート（検索・カート・存在しない id を含む） |
| Next.js | 全ページ `force-dynamic`（静的化・キャッシュを封じる） | 推奨構成（Cache Components・`'use cache'`・`<Suspense>`・`next/image`） |
| 実行形態 | ホスト上でプロセスを直接起動 | docker、1 CPU / 2 GB に制限、配布形態（distroless）のまま |
| 測るもの | RPS・レイテンシ | 加えてイメージサイズ・起動時間・ストリームの到着時刻・配信バイト数・メモリ |
| ツール | oha | oha（同じ）+ curl |

Next.js の推奨構成では、多くのページが静的シェル + ストリームになる。そのため「同じ HTML を何回返せるか」だけでなく、
「最初の 1 バイトがいつ届き、残りがいつ届くか」「何バイト配るか」を並べることを重視した。
