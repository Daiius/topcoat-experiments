# 意思決定ログ

後から確認するための記録。**[ユーザー]** はユーザーが選択したもの、**[自律]** は supervisor が推奨案を選んで進めたもの
（2026-11-03 夜〜翌朝はユーザー不在のため自律判断。朝に一括確認予定）。

| # | 日時 | 区分 | 決定 | 理由・代替案 |
|---|---|---|---|---|
| D1 | 11-03 | ユーザー | 題材は公式 `benchmarks/` storefront を土台に拡張 | 精査の結果、公式は SSR スループット比較専用（3 ルート、インメモリ、Next.js は force-dynamic）。**データと見た目だけ借りて機能を各 FW の流儀で足す**に修正 |
| D2 | 11-03 | ユーザー | 評価軸は「効率良く動く設計を自然に書かせるか」。性能勝負にしない | ベンチは補助。計測軸はサーバー応答・ブラウザ体感・資源・開発体験すべて |
| D3 | 11-03 | ユーザー | デプロイは両方 Vercel（Next.js ネイティブ / topcoat は Container Runtime Beta） | topcoat が長時間起動前提で serverless に不向きと分かったら再検討 |
| D4 | 11-03 | ユーザー | 機能 6 つ（一覧詳細・suspense・検索・カート・在庫 live・画像） | AGENTS.md 機能一覧 |
| D5 | 11-03 | 自律 | topcoat / topcoat-cli を 0.11.0 に固定、土台コードも v0.11.0 タグ | main に 0.11.0 後の破壊的変更（#571 マクロ構文統一） |
| D6 | 11-03 | 自律 | stateless 設計: DB なし・カタログ同梱・カート cookie・在庫は時刻から決定的計算 | Vercel コンテナは複数インスタンス・アイドル停止 |
| D7 | 11-03 | 自律 | listen アドレスはコード既定を変えず、コンテナで `ENV HOST=0.0.0.0` | `topcoat dev` が HOST を子に渡すため |
| D8 | 11-03 | ユーザー | 第 1 弾（一覧詳細 + suspense）受け入れ、main に merge | スクショ 5 枚で確認 |
| D9 | 11-03 | ユーザー | ローカル運用（GitHub リモート・PR・bot レビューなし） | 検証用・速度優先 |
| D10 | 11-03 | 自律 | 画像は PIL で合成生成（`scripts/gen_images.py`）、1600px 程度を少数、id で決定的割当 | 外部素材はライセンス確認が要る。最適化差を出すため原寸大きめ |
| D11 | 11-03 | 自律 | host toolchain に clippy を追加（`rustup component add clippy`） | 静的チェックに必要 |
| D12 | 11-03 | 自律 | Vercel への実デプロイは朝以降（アカウント操作が要る）。夜間は Vercel 互換 Dockerfile の準備とローカル docker 計測まで | goal の「ローカルで進められる範囲」。Vercel 調査: CDN に載るのは `s-maxage` 付き応答のみ、SSE は最大実行時間 300s(Hobby) で切れる前提 |
| D13 | 11-03 | 自律 | Docker は glibc + `gcr.io/distroless/cc-debian12:nonroot`（topcoat）/ `distroless/nodejs*-debian12:nonroot`（Next.js standalone）。依存キャッシュは BuildKit cache mount | cephylas は distroless/cc の素朴な 2 段のみ。Vercel 公式 Rust ガイドも cc-debian12:nonroot。musl は jemalloc と相性が悪い |
| D14 | 11-03 | 自律 | Next.js は 16.4 系 + Cache Components の推奨構成。#2 はレビュー=非キャッシュでストリーム、関連商品=`use cache` で静的シェルに焼く。topcoat は両方ストリーム | Next の流儀では共有データはキャッシュして遅延を消すのが正解。topcoat にリクエスト跨ぎキャッシュが無い差を比較材料にする |
| D15 | 11-03 | 自律 | Next.js の在庫 live は SSE（Route Handler + EventSource）。polling は考察で触れる | topcoat `live!` と同じ土俵で比べるため。Vercel では polling が素直という調査結果あり |
| D16 | 11-03 | 自律 | 並列度は 1 を維持（topcoat 第 2 弾の完了後に Next.js 版） | /supervise の規定（明示指示があるときだけ増やす） |
| D17 | 11-03 | 自律 | Next.js の画像は `data/images` をビルド前コピーで `public/images` へ（symlink 不使用） | symlink は .gitignore の効き方が罠になる |
| D18 | 11-03 | 自律 | topcoat の live transport は WebSocket（実装で判明）。Next.js は SSE のまま。AGENTS.md の論点を「Vercel コンテナで WS が使えるか」に修正 | Route Handler は WS 非対応。transport の違い自体を比較材料に |
| D19 | 11-03 | 自律 | `COOKIE_KEY` 未設定は release では起動失敗にする（Docker 化時に対応） | 複数インスタンスで鍵不一致だとカートが消える |
| D20 | 11-03 | 自律 | runtime JS（48KB, gzip 15KB）の全ページ読み込みは現状維持し知見として記録 | ページ単位の出し分け手段が docs に無い |
| D21 | 11-03 | 自律 | Add to cart 後は両版とも `/cart` へ redirect | 挙動を揃えて比較するため |
| D22 | 11-03 | 自律 | topcoat 第 2 弾（#3〜#6 + 修正 3 件）を supervisor 検証のうえ main へ仮 merge。ユーザー受け入れは朝に一括 | Next.js 版が `data/images` を使うため先に main へ。スクショは `scratchpad/shots/p2/` |
| D23 | 11-03 | 自律 | Vercel 案を継続（D3 の再検討条件に該当せず） | ソース読みで topcoat は長時間起動前提でないと判断（状態はクライアント再送・WS 自動再接続・SIGTERM 30s graceful）。live 通知のインスタンス間共有だけ要外部化 |
| D24 | 11-03 | 自律 | Next.js の未知 id は 200 + noindex（Cache Components の流儀）のまま。topcoat は 404 | 静的シェルを先に返すとステータスを後から変えられない、というトレードオフ自体を記事材料に |
| D25 | 11-03 | 自律 | Next.js は `products.json` もビルド前コピー。`turbopack.root` 拡大はやめる | docs が拡大を警告。dev が topcoat/target を watch する恐れ |
| D26 | 11-03 | 自律 | Next.js ヘッダーのカート件数は `cookies()` + Suspense のまま（全ページに動的な穴が開く） | HttpOnly を保てる。Client 読みの代案は記事で触れる |
| D27 | 11-03 | 自律 | Next.js 版を supervisor 検証のうえ main へ仮 merge | スクショは `scratchpad/shots/next/` |
| D28 | 11-03 | 自律 | 負荷計測ツールは oha（`cargo install oha`）。公式ベンチと同じツール | k6/oha 未導入。公式 bench.sh と揃える |
| D29 | 11-03 | 自律 | topcoat イメージの既定 `PORT=80`（Vercel 公式 Rust ガイドに合わせる。nonroot でも Docker 20.10+ は bind 可）。計測は compose で 8080 | Vercel は既定で 80 番へ転送 |
| D30 | 11-03 | 自律 | `bench/results/` は gitignore、記事で引用する回の summary だけ `git add -f` | 結果ディレクトリの肥大を避ける |
