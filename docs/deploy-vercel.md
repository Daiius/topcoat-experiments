# Vercel デプロイ手順

両版を Vercel に載せ、CDN・ストリーミング・live 接続・画像の挙動を確かめる。
コマンドはすべて `scripts/deploy-vercel.sh` にまとめてある（`--help` で一覧が出る）。
この手順書は Claude Code のセッション内で `!` を付けて実行する前提で書いている。

| プロジェクト | 中身 | ビルド |
|---|---|---|
| `topcoat-practice-topcoat` | topcoat 版 | Container Runtime（Beta）。リポジトリ直下の `Dockerfile` |
| `topcoat-practice-nextjs` | Next.js 版 | ネイティブ。Root Directory は `nextjs/` |

## 所要時間と費用の目安

- **時間**: 全体で 40〜60 分。
  - setup 1 分。
  - topcoat のビルドは 5〜15 分。ローカルの初回ビルドは 207 秒だった。内訳は topcoat-cli のソースビルドと Rust の release ビルドで、Vercel 側のマシン性能とキャッシュの効き方は未確認。
  - Next.js のビルドは 2〜3 分（静的生成 506 ページ）。
  - verify は約 6 分。live 接続を 330 秒保持するため。`--quick` を付ければ 1 分程度で終わる。
  - 残りの時間はブラウザでの確認と、コールドスタートの手測に使う。
- **費用**: Hobby の範囲内に収まり、0 円の見込み。Hobby は上限を超えても課金されず、機能が止まるだけ。

| 資源 | Hobby の枠（月） | この検証での見込み |
|---|---|---|
| 画像最適化の変換 | 5,000 | 最大 **400**。原画 25 枚 × 幅 16 種 × 品質 1 種（75）× WebP。verify 1 回で使うのは 3 変換。上限を超えると新しい変換は 402 を返す |
| Active CPU | 4 時間 | 数分。I/O 待ちとアイドル中の接続には課金されない |
| Provisioned Memory | 360 GB 時 | 1 インスタンス 2 GB。最後のリクエストから 5 分で停止する（preview は 30 秒）。1 回の検証で 1 GB 時未満 |
| Function 呼び出し | 100 万回 | 数百回 |
| CLI アップロード | 1 回 100 MB まで | 約 10 MB。`.vercelignore` で `target/` などを除いている |
| デプロイ | 1 日 100 回 | 数回 |

- ⚠ Hobby の runtime log は **1 時間しか残らない**。ログを見るなら、デプロイ直後にまとめて見る。
- Container Registry のストレージ料金（$0.10/GB）は、Hobby でも課金対象になるのか書かれていない。ただし topcoat のイメージは 18.5 MiB なので、課金されても月 1 円未満。

## 手順

### 1. 現状を見る（何も変えない）

```
! ./scripts/deploy-vercel.sh check
```

CLI の版、ログイン状態（`vercel whoami`）、プロジェクトの有無と設定、環境変数の名前を表示する。
`NOT logged in` と出たら、ユーザー自身のターミナルで `vercel login` を実行する（ブラウザ認証が必要）。
2026-11-03 時点で CLI 59.17.0、ログイン済み（`daiius`）を確認した。

### 2. プロジェクトを作る

```
! ./scripts/deploy-vercel.sh setup
```

最初の実行では、何をするかの計画を表示して止まる。
`!` で実行すると stdin が端末にならず、確認プロンプトに答えられないため。
計画を読んで問題がなければ、`--yes` を付けて同じコマンドを再実行する。

```
! ./scripts/deploy-vercel.sh setup --yes
```

setup は次のことを行う。どの処理も、設定済みの部分は飛ばす。

- 2 つのプロジェクトを作る（`vercel project add`）。
- topcoat の framework を `container` にする。
- `COOKIE_KEY`（128 桁の hex）を生成し、**Sensitive** の環境変数として Production と Preview に登録する。
  Sensitive の値は後から読み出せない。値は stdin で渡すので、ファイルにもシェル履歴にも残らない。
- Next.js の framework を `nextjs`、Root Directory を `nextjs`、ビルドコマンドを `pnpm run copy-data && pnpm run build` にする。
- Next.js の「Root Directory の外のソースを含める」設定を true にする（API の `sourceFilesOutsideRootDirectory`）。
  ビルド前のコピーで `../data` を読むため。
- Next.js に `ENABLE_EXPERIMENTAL_COREPACK=1` を設定する。`package.json` の `packageManager`（pnpm 11.24.0）を Vercel に使わせるため。
- 組織 ID とプロジェクト ID を `deploy/projects.env` に書く（gitignore 済み）。

### 3. デプロイする

```
! ./scripts/deploy-vercel.sh deploy all --prod --yes
```

- アップロード元は、両プロジェクトともリポジトリ直下である（`.vercelignore` で除外したものは送らない）。ビルドは Vercel 側で行う。
- 本番 URL（`<プロジェクト名>.vercel.app`）は `deploy/urls.env` に書き出す。
- `--prod` を付ける理由: preview URL は Deployment Protection（Standard Protection）で Vercel ログインを要求される。curl から叩けるのは本番ドメインだけ。
  preview を検証する場合は、プロジェクト設定で Protection Bypass for Automation のシークレットを作る。
  それを `VERCEL_AUTOMATION_BYPASS_SECRET` に入れておくと、verify がヘッダーで送る。
- ビルドログを後から見るときは `vercel inspect <deployment URL> --logs` を使う。
- 片方だけデプロイし直すときは、`deploy topcoat --prod --yes` のように対象を指定する。

### 4. 自動検証

```
! ./scripts/deploy-vercel.sh verify
```

結果は `bench/results/vercel-<時刻>/` に保存される（`summary.md` と JSON。`bench/results/` は gitignore 済み。記事で引用する回だけ `git add -f` する（D30））。

| 検証 | 中身 |
|---|---|
| ルート | 6 ルートを 3 回ずつ取得する。status・TTFB・`Cache-Control`・`x-vercel-cache`・`age`・`x-vercel-id`（どのリージョンを通ったか）を記録する。1 回目と 2 回目以降の TTFB の差が、CDN キャッシュの効きを表す |
| ストリーム | `/products/1` の受信チャンクの到着時刻を、identity と br で 2 回ずつ記録する。50ms 以上の間隔で「バースト」にまとめる |
| WebSocket（topcoat） | `/products/1` への Upgrade（subprotocol `topcoat-runtime`）が 101 になるかを確かめる。Origin の送り方を 3 通り試す（`Sec-Fetch-Site` あり / Origin のみ / なし） |
| SSE（Next.js） | `/api/stock/1` を 12 秒受信し、イベントの間隔を記録する（5 秒ごとの想定） |
| 長時間保持 | WS と SSE を 1 本ずつ 330 秒保持し、誰がいつ切ったかを記録する（`--quick` で省略） |
| 画像 | `/products` の HTML から 3 件取り出し、2 回ずつ取得して `x-vercel-cache` と型・サイズを記録する（topcoat は WebP、Next は `/_next/image`） |

### 5. ブラウザで確かめる（手動）

`deploy/urls.env` の `TOPCOAT_URL` / `NEXTJS_URL` を開き、DevTools の Network タブを見る。

- 商品詳細を開いたまま 5 分以上待つ。topcoat は WS が切れて再接続し、在庫表示が更新され続けるか。Next は EventSource が再接続するか。
- カートに入れてから数回リロードし、カートが消えないことを確かめる。インスタンスが複数あっても `COOKIE_KEY` を共有できているかの確認になる。
- 存在しない id（`/products/9999`）の status（topcoat 404 / Next 200 + noindex、D24）。

### 6. コールドスタート（手動）

両方とも **6 分以上アクセスしない**（本番は 5 分のアイドルでスケールダウンする）。
その後、CDN にキャッシュされていないルート（例: `/products/2` や `/cart`）の初回 TTFB を測る。
verify の最後に、測定用のコマンド例を表示する。
結果は同じ `bench/results/vercel-*/` に追記する。
Container（topcoat、イメージ 18.5 MiB）と Node 関数（Next）で差が出るかを見る。

### 7. 戻す・片付ける

| したいこと | コマンド |
|---|---|
| 本番を 1 つ前のデプロイに戻す | `! ./scripts/deploy-vercel.sh rollback topcoat`（計画を表示）→ `--yes` を付けて再実行 |
| ロールバックを取り消す | `! ./scripts/deploy-vercel.sh rollback topcoat --undo --yes`（最新を `vercel promote` する） |
| ログを見る | `! ./scripts/deploy-vercel.sh logs nextjs --since 1h`。live のログは `--follow` を付ける |
| 公開を止める（データは残す） | `vercel project pause <名前>` / `vercel project resume <名前>` |
| 全部消す | `! ./scripts/deploy-vercel.sh teardown`（計画を表示）→ `--yes`。プロジェクト・デプロイ・環境変数・`deploy/` 内の記録を消す |

- Hobby でロールバックできるのは「直前の本番デプロイ」だけ。
- ロールバックすると、それ以降の本番デプロイが自動では本番ドメインに割り当てられなくなる。`--undo` で元に戻すまで、その状態が続く。
- setup で変えたものは、すべて Vercel 側のプロジェクト設定である。リポジトリ側で増えたのは `.vercelignore`、`.gitignore` の `/deploy/`、このスクリプトだけ。

## 確認すべき観点（記事の論点との対応）

| 観点 | 何を見るか | 関連 |
|---|---|---|
| PPR の静的シェルがエッジから返るか | Next `/`・`/products/1` の `x-vercel-cache`（`PRERENDER` / `HIT`）と、TTFB・第 1 バーストの時刻。第 1 バーストにシェルと関連商品が入り、約 200ms 後にレビューが届くか | article-notes §6, §7, §10 |
| 動的な穴（カート件数）が CDN でどう扱われるか | 自前ホストでは毎リクエスト resume 描画で 95 RPS だった。Vercel ではシェルが CDN から返り、穴だけが関数で描かれるか。`Cache-Control` と 2 回目の TTFB | §7, §10, D26 |
| topcoat の HTML はキャッシュされない | `Cache-Control` なし → `x-vercel-cache: MISS` が続き、2 回目の TTFB も縮まらない。すべての HTML が関数（コンテナ）経由 | §3 |
| ストリーミングがプロキシでバッファされないか | topcoat `/products/1` の 3 バースト（ローカル: 1ms / 202ms / 802ms）が保たれるか。br でまとめられないか | §7 |
| Container Runtime で WS が使えるか | 101 になるか。300 秒前後で切られるか、誰が切るか（close フレームの有無）。ブラウザで再接続（1〜30 秒の指数バックオフ）後も在庫表示が続くか | §3, AGENTS.md 未決事項, D18 |
| Origin 検査がプロキシ越しに通るか | WS の `origin=origin-only` が 403 なら、プロキシが Host を書き換えている。その場合は公開ドメインを `trust_origins` に足す必要がある（現行ブラウザは `Sec-Fetch-Site` を送るので、`origin=same` が通れば実害はない） | §3 |
| SSE が最大実行時間でどう切れるか | Next の SSE はアプリ自身が 290 秒で閉じる（`maxDuration - 10`）。290 秒前後に `server (stream ended)` で終われば想定どおり。それより早く切れたら、プラットフォーム側の制約 | §5, D12, D15 |
| 画像はどこでキャッシュされるか | Next `/_next/image`: 1 回目 MISS → 2 回目 HIT。topcoat の WebP は `immutable` だが `s-maxage` が無い。関数の応答として毎回コンテナへ行くのか、CDN に載るのか | §9 |
| コールドスタート | 6 分アイドル後の初回 TTFB。topcoat 18.5 MiB のコンテナと Node 関数 | §7（ローカルの起動は topcoat 約 10ms / Node 約 500ms） |
| 複数インスタンスでも成立するか | カート cookie（共有 `COOKIE_KEY`）と、在庫値（時刻からの決定的な計算）がリロードしても矛盾しないか | D6, D19, §3 |

## 仕組みのメモ

- **link しない理由**: 同じディレクトリ（リポジトリ直下）を 2 プロジェクトから deploy する。`.vercel/project.json` は 1 プロジェクト分しか持てない。
  複数プロジェクトを link する `vercel link --repo` は Git 連携が前提で、この環境では使えない（リモートなし、D9）。
  そこで、コマンドごとに `VERCEL_ORG_ID` / `VERCEL_PROJECT_ID` でプロジェクトを指定する（CI 向けの公式の方法）。
- **vercel.json は不要**: Container Runtime は直下の `Dockerfile` を検出し、すべてのトラフィックをイメージへ送る。待受ポートは既定の 80（D29）。Next はネイティブのまま。
- **Root Directory**: CLI から deploy するときも、Root Directory を設定すれば `vercel` をリポジトリ直下から実行できる（docs）。
  ただし docs には「Root Directory の外のファイルにはアクセスできない」ともある。それでもビルドで `../data` を読めるように、「外のソースを含める」設定を true にしている。
- **`.vercelignore`**: 必要。無いと `topcoat/target`（数 GB）までアップロードしようとして、Hobby の 100 MB 制限で失敗する。
  `.dockerignore` は Vercel の既定でアップロードされないので、Docker ビルドのコンテキストを絞る役は `.vercelignore` が担う。

## 未確定（デプロイして初めて分かること）

- Vercel のビルドで、`Dockerfile` の BuildKit 記法（`# syntax=docker/dockerfile:1`、`RUN --mount=type=cache`）が通るか。
  通らなければ、cache mount を外した版に直す必要がある。これはアプリのコード変更ではない。
- Container Runtime で WebSocket が使えるか。WS の docs は Vercel Functions 全般の説明で、コンテナについては明記がない。
- `sourceFilesOutsideRootDirectory` を true にした状態で、CLI 経由のビルドから `../data` が見えるか。
  見えなければ、`nextjs/` 内にデータを置く方式（コピーのコミット等）を相談する。
- pnpm 11 が Corepack 経由で使われるか。ビルドログの先頭で、pnpm のバージョンを確認する。
- `vercel env add` に stdin と `--sensitive` を同時に渡せるか。setup が失敗したら、`--value` 方式に切り替える。

## 根拠（Vercel 公式 docs、2026-11-03 に確認）

- Container Runtime: https://vercel.com/docs/functions/runtimes/container
- Rust + Docker ガイド: https://vercel.com/kb/guide/deploy-rust-on-vercel-with-docker
- CLI からのデプロイ一式: https://vercel.com/docs/projects/deploy-from-cli
- `vercel deploy`（stdout は deployment URL）: https://vercel.com/docs/cli/deploy
- `vercel link` / `--repo` は Git 連携が必要: https://vercel.com/docs/cli/link
- プロジェクト指定の優先順位、`VERCEL_ORG_ID` / `VERCEL_PROJECT_ID`: https://vercel.com/docs/cli/global-options
- Root Directory: https://vercel.com/docs/builds/configure-a-build#root-directory
- モノレポ: https://vercel.com/docs/monorepos
- `.vercelignore`: https://vercel.com/docs/deployments/vercel-ignore
- 既定で除外されるファイル: https://vercel.com/docs/builds/build-features
- Deployment Protection: https://vercel.com/docs/deployment-protection
- Protection Bypass for Automation: https://vercel.com/docs/deployment-protection/methods-to-bypass-deployment-protection/protection-bypass-automation
- `vercel curl`: https://vercel.com/docs/cli/curl
- WebSockets: https://vercel.com/docs/functions/websockets
- Functions の上限: https://vercel.com/docs/functions/limitations
- CDN キャッシュ: https://vercel.com/docs/caching/cdn-cache
- `x-vercel-cache`: https://vercel.com/docs/headers/response-headers
- 画像最適化の上限: https://vercel.com/docs/image-optimization/limits-and-pricing
- Hobby の利用枠: https://vercel.com/docs/limits/fair-use-guidelines
- 制限一覧: https://vercel.com/docs/limits
- Instant Rollback: https://vercel.com/docs/instant-rollback
- Container Registry の上限: https://vercel.com/docs/container-registry/limits-and-pricing

## 追記（2026-11-03）: topcoat は GitHub Actions でビルドしてから Vercel に出す（D40）

Vercel の Container ビルドはキャッシュが効かず毎回 10 分かかったため（journal）、topcoat の流れを変えた。Next.js は Git 連携のまま。

| 段 | どこで | 中身 |
|---|---|---|
| 1. イメージ | GitHub Actions（`.github/workflows/topcoat.yml`） | `topcoat.Dockerfile` を buildx でビルド。レイヤーを GitHub Actions のキャッシュに保存（`type=gha`, `mode=max`）。`ghcr.io/daiius/topcoat-experiments-topcoat:{sha,latest}` に push |
| 2. 起動確認 | 同上 | push したイメージを起動し、5 ルートの status を確かめる |
| 3. デプロイ | 同上 | ルートの `Dockerfile` を `FROM <image>@<digest>` に書き換え（コミットはしない）、`vercel build --prod` → `vercel deploy --prebuilt --prod` |

- **`vercel build` はコンテナにも対応している**（2026-11-03 にローカルで確認）。ランナー（手元）の docker で `Dockerfile` をビルドし、Vercel Container Registry（`vcr.vercel.com/<team>/<project>/dockerfile@sha256:…`）へ push して、`.vercel/output/functions/container.func` を作る。`deploy --prebuilt` は Vercel 側でビルドしない。手元で試した所要は 19 秒（ビルドはキャッシュ済み、push 15 秒）。
  公式の「CI でビルドして成果物だけ送る」手順（https://vercel.com/kb/guide/using-vercel-cli-for-custom-workflows）がそのまま使える。
- ghcr からの pull はランナーが行うので、**ghcr のパッケージは private のままでよい**。
- 手元から出すときも同じ: `vercel pull --yes --environment=production && vercel build --prod && vercel deploy --prebuilt --prod`（`VERCEL_ORG_ID` / `VERCEL_PROJECT_ID` を指定）。

- **Vercel の topcoat プロジェクトは Git 連携を切る**（Settings > Git > Disconnect）。つないだままだと、push のたびに Vercel 側でも `FROM …:latest` のデプロイが走り、Actions の push 前の古いイメージを出してしまう（最後は Actions のデプロイで上書きされるが無駄）。
- 必要な設定は `scripts/setup-gha-vercel.sh`（Variables）と、`gh secret set VERCEL_TOKEN`（Secret、手で入れる）。
- ローカルや VPS 向けのビルドは `scripts/image-topcoat.sh`（`topcoat.Dockerfile` を使う）。
