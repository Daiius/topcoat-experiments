# 記事素材メモ（supervisor が集約）

Zenn 記事を別セッションで書くための素材。**実測値・確認済み事実**と**推定**を分けて書く。
実装中の詰まりどころの一次記録は `docs/journal.md`、判断の経緯は `docs/decisions.md`。

## 0. 記事の軸（ユーザー指定）

- topcoat（tokio-rs/topcoat v0.11.0, 2026-10-24, "Early-stage and experimental"）と Next.js を、
  **「効率良く動く設計を素直に書かせるか」「アプリ全体の設計・構成を素直にやりやすいか」**で比べる
- フロントエンドエンジニアの視点が主。バックエンド視点も可。**性能で勝ったから優れている、にしない**

## 1. topcoat の正体（調査・確認済み）

- Tokio チーム公式。2026-04 公開、2026-07-22 発表（https://tokio.rs/blog/2026-07-22-announcing-topcoat）。MIT
- 全マークアップをサーバー描画。wasm を使わない。クライアント側の振る舞いは `$(...)` 内の **Rust 式を JS に変換**して実行
- `#[shard]` = サーバー再描画される部分（htmx 的）。`suspense` / `live!` / `emit!` で段階的差し替え
- Tailwind は Node 不要で同梱。成果物は単一バイナリ + `assets/`（`current_exe` 基準で隣に置く）
- 公式 `benchmarks/` に Next.js 比較があるが、**HTML 取得の速度比較専用**（3 ルート・インメモリ・Next.js は全ページ `force-dynamic`、
  Suspense/Client Component/next/image/静的化を使わない）。→「公式ベンチは Next.js の得意技を封じた設定」と言える

## 2. 機能ごとの「流儀」と配信物

| 機能 | topcoat の書き方 | 実測・確認 | Next.js（推奨構成） |
|---|---|---|---|
| 一覧・詳細 | async `#[page]` で毎リクエスト SSR | TTFB 1〜3ms（debug, local）。`/` 40KB, `/products` 83KB | 詳細 500 件 `generateStaticParams` で静的 HTML、一覧は静的シェル + searchParams 部分をストリーム |
| 遅いデータ | `suspense(fallback: view!{..}, ..)` | `/products/1` TTFB 1.3ms / 完了 803ms。差し替えは末尾の `<template>` + 小さなインライン script（runtime JS 不要）。br 圧縮下でもストリーム維持 | レビュー=`<Suspense>` ストリーム、関連=`'use cache'` で**遅延ごと消える**。topcoat には**リクエスト跨ぎキャッシュ機構が無い**（`#[memoize]` はリクエスト内） |
| 検索 | `#[shard]` + `signal`、URL 同期は `raw!` で `history.replaceState` | POST `/_shards/product-results`、入力フォーカス維持。**生成 JS の構文エラーをブラウザで初めて検出**（下記 4） | `next/form` GET + Server Component の searchParams |
| カート | 署名付き cookie + フォーム POST + 303 | 動作。送信ごとに文書全体再読込（#541） | Server Action + `cookies()`、件数だけ `<Suspense>` |
| 在庫 live | `#[shard]` + `live!` + `emit!` + `connected()` | **transport は WebSocket**（`ws://<page URL>`, subprotocol `topcoat-runtime`）。5 秒ごとに 150〜270B の HTML 断片フレーム | Route Handler SSE + `EventSource`（Route Handler は WS 非対応）。データを流してクライアントで描画 |
| 画像 | `asset!`（文字列リテラルのみ → 動的 id は const 表を手書き） | 1600px 原寸 297KB がそのまま、カード 128px でも同じ。`immutable` は付く。リサイズ/srcset 機能なし | `next/image` + `sizes`（Vercel 最適化は変換数で課金） |
| 共通 JS | runtime script を layout で読込 | **48.6KB（gzip 15.4KB）を全ページ**で読込。ページ単位の出し分け手段が docs に無い | （Next.js 側は build 後に計測予定） |

## 3. serverless（Vercel Container Runtime）適性（ソース読み・v0.11.0）

- 起動は HOST/PORT を読んで bind するだけ（重い初期化なし）。既定 127.0.0.1:3000 → コンテナでは `HOST=0.0.0.0`
- SIGTERM/Ctrl+C で graceful shutdown、猶予既定 30 秒（Vercel の SIGTERM 後 30 秒と一致）
- shard / live の状態（args, signals）は**毎回ブラウザが送る** → サーバーにセッション状態を持たない。別インスタンスでも成立
- WS 切断時は 1〜30 秒の指数バックオフで自動再接続し、shard を fresh に再実行 → 最大実行時間 300 秒での切断も自己回復
- 注意点:
  - 署名付き cookie の鍵は `Key::generate()` だと起動ごとに変わる → 全インスタンスで共有する鍵を環境変数で（実装済み: release は `COOKIE_KEY` 必須）
  - HTML に既定の `Cache-Control` は付かない。`view!` 内でヘッダーを宣言すれば `s-maxage` で CDN に載せられる（layout で既定化も可）
  - Origin 検査は `Sec-Fetch-Site` 優先 → 現行ブラウザは proxy 越しでも通る見込み。保険で `trust_origins` に公開ドメイン
  - `live!` の**通知元をプロセス内 broadcast にする公式例（chat）は複数インスタンスで破綻**。外部 pub/sub か「毎回現在値を読む」設計に。
    本検証は「時刻から決定的に計算する在庫値」で回避
  - `#[shard]` のパスは既定で再ビルドごとに変わり得る → ローリングデプロイ用に `#[shard("/path")]` で固定（実装済み）
- **判定: topcoat 自体は長時間起動を前提にしていない。** 再検討が要るのはインスタンス間で live 通知を共有したい場合のみ

## 4. 開発体験（実測・記録）

- 初回 `cargo check` 42.6s、1 ファイル変更後 0.3〜0.6s
- **Rust の型検査も unit test も通ったのに、`$(...)` から生成された JS がブラウザで構文エラー**（`raw!` を closure 末尾式に置くと
  `return { 文; }` に変換される）。`raw!` 内の値は JS 文字列でなく runtime の `Str` サロゲート、という落とし穴も。
  → 対策としてページを描画し `data-topcoat-on:*` を `node --check` にかけるテストを追加
- `topcoat fmt` が `raw!(…)` 末尾に `,` を足してコンパイル不能にする
- `#[component]` が同名 unit struct を作り、`let related = …` が型エラー（E0308）
- エラー発生元が `::topcoat::view::component` と出て原因を追いにくい

## 5. Vercel 側の事実（調査）

- Container Runtime は Beta・全プラン（docs 2026-07）。ルート `Dockerfile` を検出、Fluid compute 上
- CDN に載るのは `s-maxage` 系付きの関数応答のみ（Set-Cookie 付きは不可）。Next.js の静的出力はビルド時点で CDN
- Hobby: 2GB/1vCPU 固定、最大実行 300s。Active CPU 課金（I/O 待ちは非課金、メモリは接続中も課金）
- ローカル検証は `vercel dev -L`（docker 必要）

## 6. Next.js 版の実測（`next build` + `next start`、ローカル、2026-11-03）

- Next.js 16.4.4 / React 19.3.1 / Turbopack。`next build`: コンパイル 1.5s + TS 1.3s + 静的生成 506 ページ 8.5s（8 workers）
  - 関連商品の 800ms 遅延 × 500 ページは**ビルド時に吸収**される（`'use cache'`）
  - 全ルートが ◐ Partial Prerender（静的シェル + 動的部分ストリーム）。`/api/stock/[id]` のみ ƒ
- 応答（非圧縮 / br）:

| URL | TTFB | 完了 | 非圧縮 | br |
|---|---|---|---|---|
| `/` | 3.9ms | 16ms | 165KB | 12.6KB |
| `/products` | 5.2ms | 29ms | 337KB | 18.9KB |
| `/products?q=headphones` | 2.7ms | 15ms | 214KB | — |
| `/products/1` | 7.3ms | **220ms**（レビュー 200ms のみ待つ） | 154KB | 16.7KB |
| `/products/9999` | — | — | 27KB, **200**（noindex） | — |

- HTML が topcoat の 3〜4 倍（RSC ペイロードを HTML 内に同梱するため）。圧縮後は 12〜19KB
- `/products/1` の JS: 8 チャンク、**595KB raw / 183KB gzip**（topcoat は runtime 1 本 48.6KB / 15.4KB）
- 詳細ページの `Cache-Control: private, no-cache, no-store`（`x-nextjs-postponed: 1`）→ 自前サーバーでは HTML 全体を CDN キャッシュしない。
  Vercel 上では静的シェル部分を CDN から返す仕組み（PPR）が働く想定 → デプロイ時に確認
- 画像: `/_next/image?url=…&w=384|640&q=75` で縮小配信（topcoat は 1600px 原寸 297KB）
- 100ms 時点のスクショ: **Next は関連商品が表示済み・レビューのみスケルトン**／topcoat は両方スケルトン
- 一覧のストリーム途中、Suspense フォールバックとストリーム後の両方に検索 input が一瞬 2 つ存在（フォールバックに input を描いているため）。
  差し替え前に入力した文字は失われうる → 「静的シェルに何を入れるか」の設計判断の例
- 検索は Enter 確定（`next/form` GET）。topcoat の shard は入力ごとの部分再描画。**同じ「検索」でも流儀に沿うと UX が変わる**

## 7. ローカル docker 計測（`bench/results/20261103-024909/summary.md`）

条件: Ryzen 7 8745H、rootless docker、**各コンテナ cpus=1 / mem 2g**（Vercel Hobby 相当）、oha 20s×32 並列 / 200rps、
load generator は同一ホスト。Next.js は `next start` 相当の standalone 自前ホスト（**Vercel の CDN/PPR エッジ配信は含まない**）。

| 指標 | topcoat | Next.js | 読み方 |
|---|---|---|---|
| イメージ（圧縮） | 18.5 MiB（展開 64.7MB） | 77.4 MiB（展開 350MB） | distroless/cc + 単一バイナリ vs distroless/nodejs + standalone |
| 初回ビルド（DL 込み） | 207s | 76s | topcoat は topcoat-cli をソースからビルドする分が大きい |
| コールドスタート（docker start→初 200） | 184ms（うち docker start 174ms） | 696ms | アプリ起動は topcoat ≒10ms / Node ≒500ms |
| メモリ idle → 負荷後 | 12.3 → 26.9 MiB | 92.6 → 279.5 MiB | |
| `/` 最大 RPS（1 CPU） | 5,677（p99 7ms） | 95（p99 789ms） | Next は全ページに「カート件数」の動的な穴 → **自前ホストでは毎リクエスト resume 描画** |
| `/products` 最大 RPS | 2,437 | 65 | 同上 + RSC ペイロード込みの 329KB HTML |
| `/products/1` 最大 RPS | **40**（800ms 待ち × 32 並列で上限） | 90 | topcoat は関連商品を毎回待つ。Next は `'use cache'` で焼いた分だけ待たない＝**設計差がそのままスループット差** |
| `/products/1` ストリーム | 3 回: 1.1ms（シェル）/ 202ms（レビュー）/ 802ms（関連） | 2 回: 4ms（シェル+関連）/ 210ms（レビュー） | |
| HTML（br/gzip） | `/` 2.2KB, `/products` 3.8KB | `/` 12.3KB, `/products` 18.5KB（gzip。standalone は br 非対応） | |
| JS | 1 本 47KB / 15KB | 8 本 580KB / 179KB | |
| 画像（`/products` 1 ページ分） | **4.2MB**（1600px 原寸 ×15） | **33KB**（`/_next/image` 縮小 + lazy） | topcoat に画像最適化が無いことの実害が最も大きい項目 |
| 未知 id | 404（**本文 0 バイト**） | 初回 200（noindex、シェル先行）、2 回目以降 404 | |

所見（記事の論点候補）:
- **素の SSR の効率は topcoat が桁違い**（RPS 25〜60 倍、メモリ 1/10）。ただし Next は「動的な穴 1 つ（ヘッダーのカート件数）」で
  全ページが resume 描画になっており、自前ホストでは PPR の静的シェルの恩恵が CPU 面で出ない。Vercel ではシェルがエッジから返るはず → 要デプロイ検証
- **Next の強みは「キャッシュして待たない」設計を API が後押しすること**（`'use cache'`）。topcoat は待つしかなく、遅い依存がそのまま並列度を食う
- **フロントエンド配信物は Next が重い（JS 12 倍）が、画像は Next が圧倒的に軽い**。topcoat で同等にするには事前リサイズ + `srcset` を自作
- `/products?q=` の topcoat p99 42ms は他より揺れが大きい（max モード）。要再計測

## 8. ブラウザ体感（Playwright + CDP、docker コンテナ、キャッシュ無効、1280×900）

**回線制限なし**（中央値 3 回, ms）

| app | route | TTFB | FCP | LCP | 転送合計 | JS | 画像 |
|---|---|---|---|---|---|---|---|
| topcoat | `/` | 2 | 36 | 96 | 1724KB | 16KB | 1701KB |
| topcoat | `/products` | 3 | 40 | 60 | 4258KB | 16KB | 4234KB |
| topcoat | `/products/1` | 2 | 24 | 24 | 315KB | 16KB | 291KB |
| nextjs | `/` | 5 | 44 | 60 | 191KB | 150KB | 16KB |
| nextjs | `/products` | 3 | 48 | 360 | 228KB | 150KB | 38KB |
| nextjs | `/products/1` | 3 | 36 | 340 | 200KB | 156KB | 6KB |

**Slow 4G 相当（150ms RTT, 1.6Mbps）+ CPU 4x スロットル**（networkidle まで待機, ms）

| app | route | FCP | LCP（要素） | networkidle | 転送合計 | JS | 画像 |
|---|---|---|---|---|---|---|---|
| topcoat | `/` | **380** | 8680 (IMG) | 9343 | 1724KB | 16KB | 1701KB |
| topcoat | `/products` | **384** | **8648** (IMG) | 21751 | 4258KB | 16KB | 4234KB |
| topcoat | `/products/1` | 388 | 1844 (IMG) | 2338 | 315KB | 16KB | 291KB |
| nextjs | `/` | 460 | 720 (IMG) | 2265 | 191KB | 150KB | 16KB |
| nextjs | `/products` | 460 | **796** (IMG) | 2400 | 228KB | 150KB | 38KB |
| nextjs | `/products/1` | 464 | 464 (IMG) | 2209 | 199KB | 156KB | 6KB |

所見:
- **FCP は topcoat が速い**（JS 16KB vs 150KB、低速 CPU ほど効く）。HTML だけで描画が完結する設計の強み
- **LCP は画像最適化の有無で決まる**。topcoat は原寸配信のため Slow 4G で 8.6 秒。フレームワークの差というより
  「最適化を標準で持つか」の差 → topcoat で手書きするとどれだけの手間で追いつけるかを実験（下記 9）
- Next の LCP 340〜360ms（回線制限なし）は画像最適化 API の初回変換待ちを含む可能性（キャッシュ無効で毎回 `/_next/image`）

## 9. 実験: topcoat で画像最適化を手書きする（`exp/topcoat-images` → main に merge）

- 手間: 変換スクリプト 55 行（PIL、320/640/960/1600w の WebP、q75）+ 本番コード約 125 行（純増約 40 行、const 表を `macro_rules!` 化）+ テスト約 120 行。
  所要約 35 分（Next.js の画像実装とほぼ同じ時間）。Next.js は画像ごとに `sizes` 1 行
- 気づき: `asset!` は `macro_rules!` なので `concat!` でパスを組める（当初の「リテラルのみ」は誤り、journal を訂正済み）。`build.rs` 不要
- `<head>` にタグを足す API が 0.11.0 に無い → `<link rel=preload>` は出せず `fetchpriority=high` のみ
- 限界: 幅 4 種固定・WebP のみ（AVIF 交渉なし）・原画変更時は手で再生成。**ビルド時に全部決める**ので実行時の変換コストと課金はゼロ

| 指標 | 原寸 JPEG（前） | 手書き最適化（後） | Next.js |
|---|---|---|---|
| イメージ（展開） | 64.7MB | **52.8MB** | 350MB |
| `/products` 画像転送 | 4,234KB | **31KB** | 38KB |
| `/products` LCP（制限なし） | 60ms | 32ms | 360ms |
| `/products` LCP（Slow 4G + CPU 4x） | 8,648ms | **400ms** | 796ms |
| `/` LCP（Slow 4G + CPU 4x） | 8,680ms | 568ms | 720ms |
| `/products/1` LCP（Slow 4G + CPU 4x） | 1,844ms | 436ms | 464ms |
| `/products` 最大 RPS（1 CPU） | 2,437 | 2,014（URL 解決が 1 カード 4 回増えた分） | 65 |
| メモリ（負荷後） | 26.9MiB | 22.8MiB | 279.5MiB |

所見: **画像の差はフレームワークの能力差ではなく「標準で持っているか」の差**。topcoat でも 35 分の手書きで Next.js 以上の LCP に届く。
ただし Next.js は「画像を足すたびに何も考えなくてよい」のに対し、topcoat は「画像の追加・変更のたびにスクリプトを回す」運用が残る。
（Next の LCP には `/_next/image` の実行時変換が含まれる。Vercel ではエッジキャッシュに載るので、デプロイ後に再比較）
