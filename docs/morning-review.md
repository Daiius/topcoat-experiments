# 朝の確認ページ（2026-11-03 夜間の自律作業分）

夜間は「推奨案を選んで進め、決定はすべて記録する」方針で進めた。ここだけ読めば確認できるようにしてある。

## 1. 画面の受け入れ（まとめて確認）

`docs/screenshots/` に、現在の main の両版を同じ操作で撮影した（1280×900、モバイルは 390×844）。

| # | 状態 | topcoat | Next.js |
|---|---|---|---|
| 01 | ホーム | `topcoat-01-home.png` | `nextjs-01-home.png` |
| 02 | 一覧 | `topcoat-02-list.png` | `nextjs-02-list.png` |
| 03 | 詳細 120ms 時点（ストリーム途中） | `topcoat-03-detail-120ms.png`（レビュー・関連ともスケルトン） | `nextjs-03-detail-120ms.png`（関連は焼き込み済み） |
| 04 | 詳細 完了 | `topcoat-04-detail-done.png` | `nextjs-04-detail-done.png` |
| 05 | 検索ヒット（headphones, 14 件） | `topcoat-05-search-hit.png` | `nextjs-05-search-hit.png` |
| 06 | 検索 0 件 | `topcoat-06-search-zero.png` | `nextjs-06-search-zero.png` |
| 07 | カート空 | `topcoat-07-cart-empty.png` | `nextjs-07-cart-empty.png` |
| 08 | カート複数 | `topcoat-08-cart-multi.png` | `nextjs-08-cart-multi.png` |
| 09/10 | モバイル 一覧 / 詳細 | `topcoat-09…`, `topcoat-10…` | `nextjs-09…`, `nextjs-10…` |

- 撮れていない状態: 在庫 live の「数値が変わる瞬間」（静止画では表現できない。ブラウザで 5 秒ごとの更新を確認済み）、JS 無効時の挙動
- スクショ上でフォントの見た目が両版で異なる。どちらもフォントの明示指定は無く、原因（Tailwind の版差によるデフォルトフォントスタックの違い等）は**未確認**。比較記事として揃えるかは要判断

## 2. 夜間に自律で決めたこと（要確認）

詳細は `docs/decisions.md`（D10〜D37）。特に確認してほしいもの:

- **D22/D27/D34**: topcoat 第 2 弾・Next.js 版・画像最適化を**仮 merge** 済み（ユーザー受け入れ前）
- **D14**: Next.js の関連商品は `'use cache'` で焼き、レビューはストリーム（topcoat は両方ストリーム）
- **D24**: Next.js の未知 id は流儀どおり（初回 200 + noindex、2 回目以降 404）
- **D26/D35**: Next.js のカート件数は `cookies()` のまま（全ページに動的な穴）。外した版は `exp/nextjs-static-header` に保存（`/` が 95 → 699 RPS）
- **D34**: topcoat の画像は事前リサイズ WebP + srcset を手書きした版を main に（最適化前の数値も記録済み）
- **D36**: `next dev` が自動生成する `nextjs/AGENTS.md` をコミット
- **D37**: Vercel への**実デプロイはしていない**（公開を伴うため）。CLI は `daiius` でログイン済み

## 3. 次にやること（ユーザー操作が要る）

1. 画面の受け入れ（上記 1）
2. Vercel デプロイ: `docs/deploy-vercel.md` を読み、`! ./scripts/deploy-vercel.sh check` から。
   デプロイ後に `verify` で、ストリーミング・WebSocket・SSE・CDN キャッシュ・コールドスタートを測る
   - 未確定: Vercel のビルドで BuildKit の cache mount が通るか、コンテナで WebSocket が使えるか、`../data` が Next.js のビルドから見えるか
3. 記事執筆（別セッション）

## 4. 記事素材の置き場所

| ファイル | 中身 |
|---|---|
| `docs/article-notes.md` | 実測値と所見の集約（§1〜§11）。**記事を書くセッションはまずここ** |
| `docs/code-comparison.md` | 機能別の「同じ機能を各流儀で書くと」比較、抜粋、フロントエンド視点の所感 8 個 |
| `docs/journal.md` | 実装中に詰まった点の一次記録（topcoat / Next.js / Docker / 実験） |
| `docs/decisions.md` | 判断の経緯 |
| `bench/results/20261103-024909/` | ローカル docker 計測の生データと summary |
| `docs/screenshots/` | 画面 |

## 5. 現時点の要点（記事の骨子候補）

- **素の SSR の効率は topcoat が桁違い**（1 CPU で RPS 25〜60 倍、メモリ 1/10、イメージ 1/4、コールドスタート ≒10ms）
- **Next.js は「キャッシュして待たない」設計を API が後押しする**（`'use cache'` 2 行で 800ms が消える）。ただし学習コストが重く、
  **ヘッダーの件数のような小さな動的 UI 1 つで、自前ホストの効率が 1 桁変わる**
- **境界の引き方が根本的に違う**: topcoat は式単位（`$()`）で、サーバー描画の HTML を WebSocket で push する。Next.js はファイル単位（`'use client'`）で、データを送ってクライアントで描画する
- **型安全**: topcoat はサーバー側が強いが、ブラウザに落ちる JS は無検査だった（実際に本番相当でだけ壊れた）
- **周辺機能の厚み（画像など）は Next.js が圧倒的**。topcoat は 35 分の手書きで LCP を Next.js 以上にできたが、運用の手間が残る
- **serverless 適性**: topcoat は長時間起動を前提にしていない（状態はクライアントが再送し、WS は自動で再接続する）。ただし live 通知をインスタンス間で共有するなら外部 pub/sub が要る
