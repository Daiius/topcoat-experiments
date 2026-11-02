# topcoat-practice

技術ブログ（Zenn）用の検証リポジトリ。Rust の full-stack Web フレームワーク
[tokio-rs/topcoat](https://github.com/tokio-rs/topcoat) と Next.js で同じストアフロントを作り、
**「そのフレームワークが、効率良く動く設計を自然に書かせるか」**を比較する。

⚠ 目的は速度勝負ではない。「性能で勝ったから優れている」という結論にしない。
各機能について「どう書くのが流儀か」「その結果何が配信されるか（JS 量・TTFB・キャッシュの効き方）」
「書きやすさ・詰まった点」を並べる。ベンチマークは補助。

## 構成

| パス | 内容 |
|---|---|
| `data/products.json` | 商品データ 500 件。公式 `benchmarks/data/` から借用（MIT, `data/LICENSE.topcoat`） |
| `topcoat/` | topcoat 版（公式 `benchmarks/topcoat` @ v0.11.0 を土台に拡張） |
| `nextjs/` | Next.js 版（公式 `benchmarks/nextjs` を土台に、**推奨構成**で書き直す） |
| `docs/journal.md` | **実装中に詰まった点・気づきのログ（記事の素材）** |

## 決定事項

- topcoat / topcoat-cli は **0.11.0 に固定**（main には 0.11.0 以降の破壊的変更 #571 等がある）。
  土台にする公式コードも **v0.11.0 タグ時点**のもの。Rust は 1.99 以上（rust-version 要件）
- デプロイ先は **両方 Vercel**。Next.js はネイティブ、topcoat は **Container Runtime（Beta, Dockerfile）**。
  どちらも Fluid compute 上。ローカル docker での同一マシン比較も補助で取る
- **コンテナは stateless・スケールで複数インスタンス・アイドル 5 分で停止**する前提で設計する
  - 外部 DB は使わない。カタログは読み取り専用データをバイナリ/イメージに同梱
  - カートは cookie（topcoat は署名付き cookie）。インスタンスのメモリに状態を置かない
  - 在庫の live 値はインスタンス間で一致するよう、時刻から決定的に計算する等の擬似データにする
  - サーバーは `0.0.0.0:$PORT` で listen（127.0.0.1 だと Vercel で 502）
- Next.js 側は公式ベンチの `force-dynamic` を使わず、**推奨構成**（静的シェル + `use cache`/ISR、
  `<Suspense>`、必要な所だけ Client Component、`next/image`）で書く

- 商品画像は **Python PIL の生成スクリプト（`scripts/gen_images.py`）で合成した画像**を使う
  （外部素材はライセンス確認が要るので使わない）。最適化の差が出るよう **大きめの原寸**（例: 1600×1600 JPEG）で
  少数（例: カテゴリごとに数枚、計 20〜30 枚）を作り、商品 id から決定的に割り当てる。生成物は `data/images/` にコミットし、両版で共有する
- ローカル運用（GitHub リモートなし・PR/bot レビューなし）。受け入れ後に supervisor が main へ merge する

## 機能一覧（両版で揃える）

| # | 機能 | topcoat | Next.js |
|---|---|---|---|
| 1 | 商品一覧（ページング・ソート・カテゴリ）/ 詳細 | async SSR | 静的シェル + `use cache` / ISR |
| 2 | 詳細のレビュー・関連商品（擬似遅延: 例 200ms / 800ms の非同期取得） | `suspense` | `<Suspense>` ストリーミング |
| 3 | 一覧の検索 | `#[shard]` 部分再描画 | Client Component または searchParams |
| 4 | カート（追加・数量変更・表示） | handler/`#[procedure]` + cookie | Server Action + cookie |
| 5 | 在庫の live 更新 | `live!` + `connected()`（transport は **WebSocket**） | Route Handler SSE（Next は WebSocket を Route Handler で扱えない） |
| 6 | 商品画像 | `asset!` + 事前リサイズ WebP（`scripts/gen_variants.py`）の `srcset`/`sizes` を手書き（当初は原寸配信） | `next/image` |

## Next.js 版の実装方針（2026-11 時点の推奨構成）

- **Next.js 16.4 系の最新パッチに固定**（`npm view next version` で確認し exact 指定）。React 19.3 系、Turbopack 既定、pnpm
- `next.config.ts`: `cacheComponents: true`, `partialPrefetching: true`, `images.localPatterns` / `qualities`,
  `output: process.env.STANDALONE ? 'standalone' : undefined`（docker 計測用。Vercel では未使用）
- #1 詳細: `generateStaticParams` で 500 件 + カタログ読み込みは `'use cache'` + `cacheLife('max')`。
  一覧: `searchParams` は Promise のまま Suspense 内で await、絞り込みは引数付き `'use cache'` 関数
- #2 **レビューは鮮度が要るデータ扱い（キャッシュしない → `<Suspense>` でストリーム）、関連商品は `'use cache'` で静的シェルに焼く**。
  topcoat はリクエスト跨ぎのキャッシュ機構が無いので両方ストリームのまま（この差自体が比較材料）
- #3 検索: `next/form` の `<Form action="">`（GET）+ Server Component の `searchParams`
- #4 カート: Server Action + `cookies().set`、ヘッダー件数は `cookies()` を読む小コンポーネントを `<Suspense>` で包む（それ以外は静的シェル）。
  フォームは `<form action={serverAction}>` で JS なしでも動く形
- #4 補足: Add to cart 後は両版とも `/cart` へ redirect
- #5 在庫 live: Route Handler で SSE（`ReadableStream`）+ Client Component の `EventSource`、`maxDuration` 指定。
  （Vercel では polling の方が素直、という考察は journal に書く）
- #6 画像: `data/images/` をビルド前スクリプトで `public/images/` にコピー（**symlink は使わない**。コピー先は gitignore）、
  `next/image` + `sizes`、一覧先頭のみ `preload`

## 未決事項（既決として実装しないこと）

- Vercel コンテナ上で WebSocket の Upgrade と長時間保持が使えるか（topcoat `live!`。関数最大実行時間 Hobby 300 秒）。Next.js 側は SSE が同制約で使えるか。駄目なら polling 等に退避し、それ自体を知見として記録する
- 計測シナリオの詳細（oha / Lighthouse / playwright の配分）

## 既知の topcoat の地雷（v0.11.0 時点の open issue）

- #541 フォーム送信でドキュメント全体が再読込される
- #538 入れ子の shard が古い内容を表示する
- #543 参照をキャプチャした `$()` 式が実行時に JS エラー
- #484 `$()` 式は CSP `unsafe-eval` が必要
- #486 再ビルドが遅い・rust-analyzer がマクロ内で効きにくい

踏んだら回避して進め、**`docs/journal.md` に記録する**（踏んだこと自体が記事の素材）。

## 開発ルール

- 静的チェック（subagent が worktree で実行してよい）
  - topcoat: `cargo fmt --check`（`view!` 内は `topcoat fmt`）/ `cargo clippy --all-targets` / `cargo test`
  - nextjs: `pnpm install` / `pnpm typecheck`（`tsc --noEmit`）/ `pnpm lint`
- 成果物ビルド・開発サーバー起動・docker・デプロイ・計測は **supervisor（本体セッション）のみ**
  - `cargo build --release` / `topcoat asset bundle --release` / `topcoat dev` / `next build` / `next dev` / `docker build` / `vercel`
- git: パスを明示して stage（`git add -A` / `git add .` 禁止）。コミットメッセージは 3 行程度。Conventional Commits
- `docs/journal.md` への記録: 日付・どの機能か・何を書こうとして・何が起きて・どう回避したか・所要時間の体感
