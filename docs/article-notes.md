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
