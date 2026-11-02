# 実装ジャーナル

実装中に詰まった点・気づき。記事の素材。新しいものを下に追記する。

## 2026-11-03 #1 公式ベンチを crates.io の topcoat 0.11.0 に載せ替える

- **書こうとしたこと**: 公式 `benchmarks/topcoat`（v0.11.0 タグ）を、workspace の外で単独のクレートとして動かす。
- **起きたこと / やったこと**
  - 公式は `version.workspace` / `topcoat.workspace` / `[lints] workspace = true` に依存している。
    `topcoat = "=0.11.0"`（default features）と、build-dependencies の
    `topcoat = { version = "=0.11.0", default-features = false, features = ["tailwind"] }` に置き換えた。
    lints は workspace の内容（`unsafe_code = "deny"`、clippy `pedantic` + `mod_module_files = "deny"`）をそのまま書き写した。
  - **runtime 側に `tailwind` feature は要らない。** `docs/tailwind.md` は「runtime と build の両方で `tailwind` を有効にし、
    `tailwind::stylesheet!()` を使う」と書いている。一方、公式ベンチは build.rs で `assets/tailwind.css` に書き出し、
    `asset!("assets/tailwind.css")` で参照する。このため runtime 側は default features だけで足りる。
    パスを固定しているのは、debug と release で asset ID を揃えるため（build.rs のコメントより）。
  - `include_str!("../../data/products.json")` は公式と**同じ相対パスのまま**動いた。
    `topcoat/src/` から 2 つ上がリポジトリ直下になり、公式の `benchmarks/topcoat/src/` → `benchmarks/data/` と同じ深さだからだ。
    データの中身も公式 `benchmarks/data/products.json` とバイト一致（`cmp` で確認）。
  - 本体を書き換える前の初回 `cargo check` は、**一発で通った**。載せ替えでコードの差分は出ていない。
  - ベンチ専用だったのは `.compression(Compression::off())` だけ（公平性のため全フレームワークで off）。これを外し、
    default（gzip / brotli 有効）に戻した。jemalloc は残した。
  - `rustfmt.toml` は公式の workspace 版が nightly 専用オプション（`imports_granularity` など）を含むため、
    `edition = "2024"` と `max_width = 100` だけにした。`topcoat fmt` は edition を `rustfmt.toml` から読む（`docs/cli/fmt.md`）。
- **所要時間の体感**: 載せ替え自体は 10 分程度。

## 2026-11-03 #1 `0.0.0.0:$PORT` での listen はコード変更なしで可能

- `topcoat::start` は `HOST` / `PORT` 環境変数を読み、既定は `127.0.0.1:3000`
  （`docs/router.md` の "Serving" 節、`crates/topcoat/src/serve.rs`）。
- `topcoat dev` は子プロセスのアプリに `HOST` / `PORT` を**自分で渡す**（`topcoat-cli/src/dev/app_server.rs`）。
  そのため、コード側で既定値を `0.0.0.0` に変える改造はしない方がよい（dev の動作とぶつかる恐れがある）。
- 結論として、`main.rs` は `topcoat::start` のまま。**コンテナ側で `ENV HOST=0.0.0.0` を設定する**。
  Vercel が渡すのは `PORT` だけと想定し、`HOST` は Dockerfile で固定する。main.rs にもその旨をコメントで残した。

## 2026-11-03 #2 `suspense` でレビュー・関連商品をストリーミング

- **書こうとしたこと**: 詳細ページのレビュー（擬似遅延 200ms）と関連商品（800ms）を非同期取得にし、
  シェルを先に返して後から差し込む。
- **書き味**
  - 流儀は `examples/suspense` そのまま。遅い処理を `#[component] async fn` に閉じ込め、
    `suspense(fallback: view! { ... }, child())` で包むだけ。差分はほぼ「`review_list(...)` を
    `suspense(fallback: ..., reviews(product_id: ...))` に置き換える」だけで済んだ。
    Next.js の「async Server Component を `<Suspense>` で包む」と同型で、書き味はかなり近い。
  - 子コンポーネントは `cx: &Cx` を受け取れる。`app_context::<Catalog>(cx)` で借用したまま `.await` をまたいでも、
    ライフタイムの問題は起きなかった（`&Catalog` を await の前後で保持している）。
  - 擬似 upstream は `src/remote.rs` に分けた（`tokio::time::sleep` → 同梱カタログを読むだけ）。
    キャッシュも可変状態も持たないので、どのインスタンスでも同じ応答になる。
  - 見出し「Reviews (n)」は `product.review_count` から即座に出せるので、suspense の**外**に置いた。
    スケルトンの枚数も、即座に分かる値（`review_count.min(3)`、`related_ids.len()`）から決めて、差し替え時のレイアウトずれを減らした。
- **詰まった点**
  - **`fallback:` にコンポーネント呼び出しを直接書けない。** `view!` の props の値は Rust の式として解釈される。
    `fallback: review_list_skeleton(count: 3)` は view 構文のコンポーネント呼び出しなので、そのままでは書けない。
    `fallback: view! { review_list_skeleton(count: 3) }` と包む必要がある（examples も `view! { <p>..</p> }` を渡している）。
  - **`#[component]` は関数名と同名の unit struct を生成する。** テストで `use super::*;` した状態で
    `let related = position(...)` と書いたところ、次のエラーになった。

    ```
    error[E0308]: mismatched types
    expected `usize`, found `related`
    `related` is interpreted as a unit struct, not a new binding
    ```

    パターン中の `related` が、コンポーネント `related` の unit struct として解釈された。
    rustc の help（"introduce a new binding instead"）は的確だった。コンポーネント名をローカル変数名と重ねない方がよい。
    なお、コンポーネント本体の中で `let related = ...` と書くのは通った。
  - **module 由来の `#[page]` は `Router::builder().page(...)` に渡せない。** テストで `product_detail` を単独の Router に
    載せようとしたところ、``the trait bound `id::product_detail: Page` is not satisfied`` になった。
    エラーの発生元は ``this error originates in the attribute macro `::topcoat::view::component` `` と表示される。
    `#[page]` の展開が component 経由であることが透けて見え、初見では原因を辿りにくい。
    `docs/router/page.md` によれば、`RouterBuilder::page` に渡せるのは明示パスの page だけ。
    また、本物の layout は `asset!` を描画するため、asset bundle の無いテストでは panic する
    （`no asset config registered in this router context`）。
    回避として、テスト内に `#[page("/boundaries")]` を作り、同じ 2 つの boundary を載せて `Router::handle` で検証した。
- **気づき（配信されるもの）**
  - 差し替えは**ランタイム JS 不要**。応答の中で、最初の swap の直前に一度だけ約 20 行のインライン `<script>`
    （`window.topcoat ??= { swap(id) {...} }`）が送られる。各 swap は
    `<template data-topcoat-swap="ID">…</template><script>topcoat.swap("ID")</script>` という形になる。
    fallback は `<!--::topcoat::region::start(ID)-->` と `end` のコメントで挟まれ、その間が置き換わる。
    CSP を厳しくする場合はインライン script の扱いが論点になる（記事で Next.js の RSC payload と比較する材料）。
  - テスト（`tokio::time::pause` 下）で、2 つの boundary の取得は**並行**に進むことを確かめた。
    応答完了は 200 + 800 ではなく 800ms。`suspense` の子は、ページ本体と独立に poll される。
  - **圧縮 on のままでもストリーミングは止まらない見込み。** topcoat の圧縮は tower-http 0.7.1 の `Compression` を使う。
    その下の async-compression 0.4.52 の bufread encoder は、入力が `Pending` になると encoder を flush してから返す
    （`generic/bufread/encoder.rs` の `State::Flushing`）。そのため fallback 部分は差し込み待ちの間にも送出されるはずだ。
    **実機（curl `--compressed -N`）での確認は supervisor 側で未実施。**
- **所要時間の体感**: 実装 15 分、テストの組み方で 10 分。

## 2026-11-03 #1 #2 ビルド時間・ツールの実測

- 環境: 16 コア、rustc 1.99.1、`target/` 空の状態。
  - 初回 `cargo check`（依存の取得と Tailwind CLI のダウンロードを含む）: **42.6 秒**
  - `cargo clean` 後の `cargo check`（ダウンロード済み）: **34.2 秒**。続けて `cargo test --no-run`: 11.4 秒
  - 1 ファイル変更後の `cargo check`: **0.3〜0.6 秒**。`cargo test --no-run`: 1.2〜1.7 秒
  - `build.rs` は `rerun-if-changed=src` なので、src を触るたびに Tailwind の再生成が走る。それでも check は 1 秒を切った。
  - **#486（再ビルドが遅い）は、`cargo check` の範囲では体感できなかった。** release ビルドと `topcoat dev` の再ビルドは未計測（supervisor 側）。
- `topcoat fmt --check` は約 25ms（9 ファイル）。`cargo fmt --check` が通っても、`topcoat fmt` は `view!` 内の長い属性行を
  `<div\n class="..."\n>` の形に折り返す。両方を回す必要がある。
- rust-analyzer は rustup の proxy だけが入っていて、コンポーネント本体は未インストールだった
  （`Unknown binary 'rust-analyzer' in official toolchain`）。そのため、マクロ内での効き方（#486）は今回検証できていない。
  clippy も未インストールだった（`'cargo-clippy' is not installed`）。

## 2026-11-03 #6 商品画像を `asset!` で配信する

- **書こうとしたこと**: `scripts/gen_images.py`（PIL）で合成した画像を商品に割り当てる。
  一覧カード、詳細、関連商品の頭文字プレースホルダを置き換える。
- **素材**: 8 カテゴリ × 3 枚 = 24 枚。1600×1600 の JPEG（quality 88）で、1 枚あたり約 280〜300KB、合計 6.7MB。
  割り当て規則は `data/images/{category_slug}-{(id % 3) + 1}.jpg` で、Next.js 版も同じ規則を使う。
  生成は決定的にした（同じスクリプトから同じバイト列が出る）。
  `Image.effect_noise` は C 側の乱数を使い、実行のたびに出力が変わる。そのため、シード付きの `random.Random.randbytes` でノイズを作った。
- **500 商品 → 静的アセットの対応づけ**
  - `asset!` の引数は**文字列リテラルだけ**。`format!` や `concat!` で組み立てたパスは渡せない。
    コンパイル時に ID を計算し、バイナリに宣言を埋め込んで bundler が拾う仕組みだからだ。
    そのため、動的な id から引くには「全ファイルを 1 回ずつ `asset!` で宣言した const 表」を書き、実行時に添字で引くしかない
    （`src/images.rs` の `IMAGES: [(&str, [Asset; 3]); 8]`）。
  - 24 枚なら手書きで済む。商品ごとに 1 枚（500 枚）だと 500 行の表になり、build.rs で生成するしかなくなる。
    ただし生成ファイルを `include!` すると、相対パスの基準がどこになるかという問題が出る
    （`asset!` の `./` `../` は呼び出し元ソースファイル基準）。今回は試していない。
  - 表と実ファイルのずれはコンパイルでは検出できない（`asset!` はパスの存在を確かめない。確かめるのは bundler）。
    そこで、`data/images/` の中身と表を突き合わせるテストを足した。
  - 流儀としては「少数の固定アセット（ロゴ、CSS、フォント）」向けの API で、**データ駆動の画像（CMS やユーザー投稿）は守備範囲外**という感触。
    その種の画像は外部ストレージの URL をそのまま `<img src>` に入れることになる。
- **最適化が無いことの実害**
  - `asset!` の options は `rename` / `extension` / `checksum` / `content_type` の 4 つだけで、リサイズや形式変換は無い。
    `srcset` に並べる縮小版も作れない。h-32（128px）のカードにも 1600px の原寸（約 290KB）が届く。
    一覧の 1 ページ（24 枚）で、画像だけで最大約 7MB になる。
  - `width` / `height` / `loading="lazy"` / `decoding="async"` は普通の HTML 属性としてそのまま書ける。
    レイアウトシフト対策と遅延読み込みはできる。詳細の主画像（LCP 候補）だけ `loading="eager"` にした。
    縮小版を自前で用意すれば（スクリプトで複数サイズを生成し、それぞれ `asset!` で宣言）`srcset` も手で書ける。ただし宣言の数が倍々に増える。
  - 配信は `Cache-Control: public, max-age=31536000, immutable`（`topcoat-asset/src/serve.rs`）。
    ファイル名に内容ハッシュが入るので、2 回目以降のアクセスはキャッシュが効く。
- **テストで踏んだこと**: 関連商品カードが画像を描画するようになったため、#2 の suspense テストが
  `no asset config registered in this router context` で panic した。
  bundle が無い環境でも、`Manifest` を手で組み立てて `AssetConfig::hosted_at("https://assets.test", manifest)` を登録すれば
  描画できる（`Asset::id()` が公開されている）。この test helper を `images.rs` に置いた。
- **所要時間の体感**: 画像スクリプト 15 分、topcoat 側 15 分、テストの asset 回避 10 分。

## 2026-11-03 #3 一覧の検索を `#[shard]` + `signal` で部分再描画する

- **書こうとしたこと**: 一覧ページに検索入力を足す。入力するたびに結果のグリッドだけをサーバーで描き直す。
- **書き味**
  - `examples/runtime/src/shard.rs` の形そのまま。ページで `signal(cx, || 初期値)` を作り、
    `<input :value=$(search.get()) @input=$(|e: Event| search.set(e.target.value))>` で結ぶ。
    結果側は `#[shard] async fn product_results(cx, search: String, page: usize, sort: Option<String>, category: Option<String>)`
    に切り出し、`product_results(search: $(search.get()), ...)` で呼ぶだけ。
    シグナルを読むのは `$()` の中だけなので、ページ全体は再実行されず、shard だけが POST で描き直される。
  - 初期値はクエリ文字列 `q` から取る。ページ番号は `page: $(if search.get() == initial_search { initial_page } else { 1 })` とした。
    「クエリが変わったら 1 ページ目に戻す」を、捕捉した初期値との比較で式として書ける。
  - ソート・カテゴリのチップ、件数、ページングも shard の中に入れた。外に置くと、入力後もリンクが古い `q` のままになるためだ。
    結果として、シグナルに依存しない見出しと入力欄だけが shard の外に残った。
  - shard の引数は**すべてユーザー入力**（docs が繰り返し強調している）。shard 内で `normalize_search`（trim、64 文字で切る、空なら None）と
    `normalize_sort` をかけ直した。
    パスは `#[shard("/_shards/product-results")]` で固定した。既定では名前とソース位置から生成され、移動すると変わる。
  - JS が無い場合のために、入力欄を `<form method="get" action="/products">` で包んだ。Enter で普通の GET 遷移になる。
- **URL との同期**
  - 式の語彙に URL / History の API は無い。`raw!` で JS を埋め込み、`@input` の中で `history.replaceState` を呼んだ
    （`q` を設定・削除し、`page` を消す）。push ではなく replace にしたのは、1 文字ごとに履歴が積まれるのを避けるため。
  - 流儀として「シグナル ↔ URL の同期」は用意されていない。runtime の `link` は文書ごと差し替えるナビゲーション用。
    サーバー側で URL を正とするなら、`<form method=get>` で遷移させるのが素直。今回は「入力のたびに部分再描画 + replaceState」と、
    「JS 無しなら GET 遷移」の二段構えにした。
- **詰まった点: `topcoat fmt` が `raw!` を壊した**
  - `raw!("…長い JS…")` を `topcoat fmt` にかけた。文字列が長いので複数行に折られ、末尾に `,` が付いた（`raw!(\n "…",\n)`）。
    その結果、**コンパイルが通らなくなった**。

    ```
    error: unexpected end of input, expected Rust expression after comma
      = note: this error originates in the macro `::topcoat::runtime::expr`
    error: cannot find macro `raw` in this scope
    error[E0599]: no method named `set` found for struct `topcoat::runtime::Signal<T>`
    ```

    `raw!` の 2 番目の引数（サーバー側の Rust 版）が省略可能なので、`,` の後に式が来ると解釈された。
    続く 2 つのエラーは連鎖で、`Signal::set` が無いという無関係な指摘まで出る。初見では原因を `topcoat fmt` に結びつけにくい。
  - 回避として、Rust 版に `()` を渡した（`raw!("…", ())`）。2 引数の後ろの末尾 `,` は受け付けられ、整形後もコンパイルが通る。
    イベントハンドラの中はブラウザでしか実行されないので、Rust 版は使われない。
- **#538（入れ子の shard）**: shard を入れ子にしていないので踏んでいない。
- **#484（`unsafe-eval`）**: shard の初期描画には、引数が
  `<!--::topcoat::shard::start("/_shards/product-results", "<id>", ["cx.hydrate(&quot;zzzz&quot;)", …])-->`
  という**JS 式の文字列**として埋め込まれる。ブラウザはこれを評価する。厳しい CSP を掛けると、この経路から壊れると予想する。
- **配信される JS**
  - `topcoat::runtime::script()` は `<script type="module" src="/_topcoat/assets/topcoat-<hash>.js">` を 1 本出す。
    実体は `topcoat-runtime-0.11.0/browser/dist/index.js` で、**48,582 B（gzip -9: 15,129 B、gzip 既定: 15,155 B、zstd -19: 14,513 B）**。
    brotli はローカルに無く未計測。実際の転送量は supervisor の計測待ち。
  - layout に入れたので**全ページ**で読み込まれる（ホームのように runtime を使わないページも含む）。
    docs の手順も「document head に入れる」だけで、ページ単位で出し分ける方法は書かれていない（内部に hoist はあるが、未文書化）。
  - 配信は `.runtime()` と `.discover()`（shard や procedure の登録）を router に足すだけで済む。
- **テスト**: shard も初期描画ではコンポーネントとして呼べる。テストでは明示パスの `#[page]` から固定引数で呼び、
  件数表示、0 件の文言、チップのリンクに `q` が載ることを確かめた。再描画の POST（`ShardRequest` の JSON と identity ヘッダ）は
  内部プロトコルなので、単体テストでは叩いていない。
- **所要時間の体感**: 実装 20 分、fmt 起因のエラーの切り分け 10 分。

## 2026-11-03 #4 カートを署名付き cookie + フォーム POST で実装する

- **書こうとしたこと**: 詳細の「Add to cart」、`/cart` での数量変更・削除、ヘッダーの件数表示（ハードコードの "Cart (3)" を置き換える）。
  状態は cookie に置き、サーバーのメモリには置かない。
- **`#[procedure]` ではなく、フォーム POST + 303 redirect を選んだ理由**
  - docs のミューテーションの例は、どれも `<form method="post" action=(href!(…))>` + `#[route(POST)]` + `see_other(…)` の形だった
    （`docs/router.md` の "Linking to pages and routes"、`docs/router/href.md`、`docs/router/error.md`）。
    `docs/runtime/link.md` も「ミューテーションはフォーム送信か procedure に置け」と書いており、両方とも正規の手段ではある。
  - procedure は「ブラウザの式から呼ぶ RPC」で、戻り値をシグナルに入れて**クライアント側で**画面を直す前提の作りになっている
    （`examples/runtime/src/procedure.rs`）。カートは、ヘッダーの件数とカートページの両方が同じ cookie から描かれるサーバー状態だ。
    procedure にすると、件数表示もシグナル化して二重管理になる。
  - フォーム POST なら、JS 無しで動き、cookie はハンドラ内で書けて、PRG によりリロードでの再送も防げる。
    件数はレイアウトが毎リクエスト cookie から描くので、同期の問題が起きない。
  - 結論として、**「サーバー状態の変更はフォーム、ブラウザ内で完結する操作は signal、ブラウザから値を取りに行く操作は procedure」**
    という使い分けが docs の流儀に沿うと判断した。
- **書き味**
  - cookie 側は docs の "A helper per store" をそのまま使った。`cookie_store(signed_cookies(cx)…, "cart").parse_or_default()` を
    `cart(cx)` に包み、各ハンドラは `cart(cx).update(|c| …).commit()?` と書くだけ。
    改ざん・欠落・旧形式は `parse_or_default` で空カートに倒れる。
  - 値は `[[id,qty],…]` の JSON（`#[serde(transparent)] struct Cart(Vec<(u32, u32)>)`）。上限は 50 行・各 99 個にした。
    cookie の 4KB 制限から十分離している。
  - 鍵は `COOKIE_KEY`（128 桁の hex = 64 バイト）から読む。未設定なら `Key::generate()` に倒して警告を出す。
    docs の例は `Key::generate()` を起動時に呼ぶだけで、「複数インスタンスでは同じ鍵を読み込め」と注記がある。
    stateless・マルチインスタンス前提では環境変数が必須になる。
  - `Form<T>` 抽出（`async fn add(cx: &Cx, Form(input): Form<AddForm>)`）と、`see_other(href!(super::cart_page).resolve(cx))` は素直に書けた。
    module ルーティングで `src/app/cart/add.rs` → `/cart/add` になる。
    詳細ページから `href!(crate::app::cart::add::add)` で参照するために、`mod add` を `pub(crate)` にする必要があった。
- **#541（フォーム送信で文書全体を再読込）**: **仕様上そのまま踏む**。runtime の `RuntimeLayer` はフォーム送信を素通しする
  （`topcoat-runtime/src/layer.rs` に "Other requests pass through unchanged, including form submissions"）。
  POST → 303 → GET で文書全体が描き直される。スクロール位置は失われ、ヘッダー・フッターも再取得される。
  Next.js の Server Action（RSC payload で差分更新）との比較点になる。
- **CSRF**: router 既定の `OriginPolicy` が、信頼しない Origin からの状態変更リクエストを拒否する（`topcoat-router/src/origin.rs`）。
  cookie は `SameSite=Lax`、`HttpOnly`、`Secure`、`Path=/`。
  ⚠ Vercel のプロキシ越しで Origin と Host の判定が一致するか（403 にならないか）は、デプロイ時に要確認。
  不一致なら `trusted_proxies` / `origin_policy` の設定が要る。
- **キャッシュへの影響**: ヘッダーの件数がレイアウトで cookie を読むので、**全ページがリクエストごとの描画**になる
  （topcoat はもともと全ページ動的なので、追加のコストではない）。Next.js 側では、これが静的シェルを崩す要因になりうる。
- **テスト**: `router()` を `build(assets, key)` に分けた。`test_asset_config` で Tailwind CSS と runtime の JS も解決できるようにし、
  **本物の `module_router!()` をテストで回せる**ようにした（#2 のときは layout の asset で詰まっていた）。
  追加 → 追加 → 数量変更 → 削除を cookie を受け渡しながら通すテスト、改ざんした cookie が空カートになるテスト、
  存在しない商品が 404 になるテストを足した。
- **所要時間の体感**: 実装 25 分、end-to-end テストの土台作り 15 分。

## 2026-11-03 #5 在庫数を `live!` + `emit!` + `connected()` で live 更新する

- **書こうとしたこと**: 詳細ページに在庫数を出し、5 秒ごとに更新する。値は時刻と商品 id から決定的に計算し、インスタンス間で揃える。
- **書き味**
  - 値は `src/stock.rs` の `stock_at(id, SystemTime)` で決める。5 秒スロットの番号と id を SplitMix64 で混ぜた純関数にした。
    broadcast も Mutex も持たない。時計が揃っていれば、どのインスタンスでも同じ瞬間に同じ値になる。
    待ち時間は「次のスロット境界まで」（`until_next_change`）なので、全クライアントが同じ境界で一斉に切り替わる。
  - `examples/live/src/chat.rs` の形をそのまま使った。`#[shard]` の中で `live! { loop { let token = emit!{…}?; if !connected(cx) { break Ok(token) } sleep… } }`。
    chat の `changed.recv().await` が `tokio::time::sleep(until_next_change(now))` に置き換わっただけ。
  - **page ではなく shard にした**。`connected()` は「囲んでいる page か shard」を接続付きで描き直させる。
    page の中で呼ぶと、再接続のたびに詳細ページ全体（800ms の関連商品を含む）が描き直される。
    shard（`#[shard("/_shards/stock-level")] async fn stock_level(cx, product_id: u32)`）なら、在庫の段落だけが接続付きで走る。
  - shard の引数 `product_id` はブラウザから戻ってくるので、存在確認してから使う（未知の id なら何も出さず、接続も要求しない）。
  - HTTP 描画では 1 回だけ emit して終わる。テスト（`tokio::time::pause`）で、応答が 5 秒の待ちを含まずに完了することを確かめた。
    HTML には `<!--::topcoat::shard::start("/_shards/stock-level", …)-->` の直後に `<!--::topcoat::connect-->` が入り、
    これがブラウザへの接続要求になる。
  - 一発で書けてコンパイルも通った。ただ、`emit!` の中に空の `if !known { }` 分岐を書けるかは試すまで分からなかった（書けた）。
- **transport（ソースで確認）: SSE ではなく WebSocket**
  - `docs/view/live.md` の記述は "Updates travel over the HTTP response, or over a WebSocket when the page or shard requests a connection"。
    `connected()` を使う長時間の更新は **WebSocket** で流れる。AGENTS.md の機能一覧にある「`live!` / SSE」は、topcoat については正確ではなかった。
  - 仕組み（`topcoat-runtime-0.11.0/src/layer.rs`、`src/layer/socket.rs`、`browser/src/render/connection.ts`）
    - ブラウザは**文書 1 つにつき WebSocket を 1 本**開く。開く先は**ページ自身の URL**で、`Sec-WebSocket-Protocol: topcoat-runtime` を付ける
      （`ws(s)://<host>/products/1`）。`.runtime()` が足す `RuntimeLayer` がこれを受ける。
    - 接続上では、`{run, method:"POST", path:"/_shards/stock-level", headers, body}` の JSON で「接続付き描画」を依頼する。
      サーバーは run ID 付きのフレームで、出力を流し続ける。cookie や Origin はハンドシェイクのものが使われ、run 側からは変えられない。
    - 1 接続あたりの同時 run は既定で 64（`.max_runs_per_connection(n)`）。超えると 429。
    - アプリ層の ping / heartbeat は無い（在庫は 5 秒ごとにフレームが流れるので、アイドル切断は起きにくいはず）。
  - **切断時の挙動**: サーバーは run ごと捨てる。ブラウザは `close` を受けると、**1 秒から倍々で最大 30 秒の指数バックオフ**で再接続する。
    つながると、各メンバー（shard）を**先頭から**走らせ直し、最新の値を 1 回 emit してから待ちに戻る。
    状態を持たない設計なので、再接続で値が飛んだりずれたりしない。ページから離れてメンバーがいなくなると、ソケットは閉じる。
  - **Vercel での判断材料**: 必要なのは「長時間の SSE」ではなく、**コンテナ上での WebSocket の Upgrade と、その長時間保持**。
    使えない場合の退避案は 2 つ考えられる。ただし、どちらを採るかは未決。
    1. `connected()` を使わず、`live!` の loop を HTTP 応答のまま流し続ける。チャンク化された HTML を延々返す形で、事実上の SSE 相当になる。
       docs は「無期限の待ちは接続チェックの後ろに置け」としており、流儀からは外れる。
    2. `sse` feature + 自前の route で、JSON を polling / SSE する。
- **所要時間の体感**: 実装 15 分、transport の読み込み 15 分。

## 2026-11-03 #3 検索の `@input` が、ブラウザで初めてコンパイルエラーになった

- **何が起きたか**: 第 2 弾の時点で、`cargo check`、clippy、unit test、shard の初期描画を含む end-to-end テストはすべて通っていた。
  ところが supervisor がブラウザで開くと、`Error: Failed to compile event @input`（runtime JS の `compile`）が出た。
  入力しても shard への POST が出ず、URL も変わらない。**Rust の型検査も unit test も通ったのに、生成された JS が不正で、実行時に初めて分かった。**
- **原因**
  - `$(|e: Event| { …; raw!("{ const url = …; history.replaceState(…); }", ()) })` のように、closure の**末尾式**に `raw!` を置いていた。
    中身は「文の並び」のつもりで `{ … }` と書いた JS だった。
  - runtime 式は JS に下ろされ、末尾式は `return <式>;` になる。ブラウザは各属性値を
    `new Function("cx", "return " + source + ";")`（`browser/src/expression/compile.ts`）でコンパイルする。
    その結果 `return { const url = … }` という形になり、`{` がオブジェクトリテラルとして解釈されて `SyntaxError: Unexpected identifier 'url'` になった。
  - `raw!` の JS 部分は、`${ident}` の置換以外は**文字列のまま連結されるだけ**（`topcoat-runtime-grammar/src/expr/builtin_macro.rs` の `interpolate_js`）。
    マクロも rustc も JS の構文を一切見ない。docs（`docs/runtime/expr.md` "Embedding JavaScript"）の例は `raw!("${n}.toUpperCase()", …)` という**式**だが、
    「JS は式でなければならない」とは明記されていない。
- **もう 1 つの潜在バグ**: `${value}` に入るのは JS の文字列ではなく、**runtime のサロゲート（`Str` クラス）**だった。
  - 旧コードの `${value}.trim()` はサロゲートの `trim()` を呼び、サロゲートを返す。そのため `if (q)` は空文字でも常に真になり、空入力で `?q=` が残る。
  - `Str` は `toString()` を持つので、`'' + ${value}` で生の文字列になる。
  - 一方、Rust 側の `value.to_owned()` は JS でも `__local1.to_owned()` のまま出力される。これはサロゲートのメソッド（`Str.to_owned()`）として正しく動く。
- **直し方**: JS を **1 つの式（即時実行のアロー関数）** にした。

  ```
  ((url, q) => { …; history.replaceState(history.state, '', url); })(new URL(location.href), ('' + ${value}).trim())
  ```

  `raw!("…", ())` の `()` は、前回の `topcoat fmt` の末尾 `,` 問題の回避として残した（イベントハンドラ内はサーバーでは実行されない）。
- **再発防止のテスト**（`app.rs` の `runtime_expressions_compile_as_javascript`）
  - 本物の router で `/products`、`/products?q=lamp`、`/products/1`、`/cart`、`/` を描画する。
  - `data-topcoat-on:*` / `data-topcoat-bind:*` の属性値を取り出して実体参照を戻し、`(function (cx) { return <source>; });` に包んで `node --check` にかける。
  - 修正前のコードで**このテストが同じ `SyntaxError` で落ちる**ことを確認してから直した。node が無い環境では skip する。ポートは使わない。
  - 構文だけでなく動作も、手元で一度確かめた: 実際に出力されたハンドラの JS を node で `new Function` し、サロゲートと `history` のスタブを渡す。
    `"  lamp "` → `?sort=price&q=lamp`（`page` は消える）、`""` → `q` が消える。こちらはテストには入れていない
    （runtime 本体のサロゲートを読み込めず、スタブの正しさに依存するため）。
- **記事向けの論点**: 「Rust で書けば型で守られる」という期待は、`$()` の中では**サーバー側の評価**にしか効かない。
  JS への下ろしは別経路で、特に `raw!` は無検査で素通しされる。Next.js の Client Component なら tsc とバンドラが同じ箇所を検査する。
  topcoat では、ブラウザ（または自前の node 検査）まで行かないと分からない。
- **所要時間の体感**: 原因の特定 15 分、テスト作り 15 分。

## 2026-11-03 #4 #6 カート 2 行目のサムネイルが、load 直後のスクショで空だった

- **報告**: `/cart` の 2 行目（Harbor Ultrawide Display Max、id 42）の画像が、load 直後のスクショでは空だった。
- **コードで切り分けた結果: 割り当ても bundle も問題なし。lazy 読み込みのタイミングと判断した。**
  - 割り当て: id 42 は `displays` カテゴリで、`42 % 3 + 1 = 1` から `data/images/displays-1.jpg`（292,223 B）になる。
    ファイルは存在し、`images.rs` の表にも載っている。
  - bundle: `Asset` が bundle に無ければ、描画時に `failed to resolve asset … in the asset catalog` で **panic する**
    （`topcoat-asset/src/config.rs` の `fmt_url`）。ページが 200 で描けているなら、URL は bundle から解決できている。
  - 原因の見立て: カートのサムネイルが `loading="lazy"` だった。lazy 画像は `load` イベントを待たせない。
    80px の枠に 1600px・約 290KB の原寸を 2 枚読むので、`load` 時点のスクショでは間に合わないことがある
    （「最適化が無いことの実害」の一例でもある）。
- **対応**: カートの行は短く、すべてファーストビューに入るので、`eager: true`（`loading="eager"`）にした。
  テストも足した。カートに id 1 と 42 を入れて `/cart` を描画し、両方の `<img>` が期待どおりのファイル名と `loading="eager"` で出ることを確かめる。
- **所要時間の体感**: 10 分。
