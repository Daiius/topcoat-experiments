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
  - `asset!` の引数は**文字列リテラルだけ**。`format!` や `concat!` で組み立てたパスは渡せない。（→ 訂正: `concat!` は渡せる。後述「[topcoat][実験] #6」参照）
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

## 2026-11-03 #4 release ビルドでは `COOKIE_KEY` を必須にする

- release ビルド（`cfg!(debug_assertions)` が偽）で `COOKIE_KEY` が無い場合、`router()` を組み立てる時点で panic させる。
  サーバーは listen する前に終了する。メッセージは
  `COOKIE_KEY is not set. Release builds need the cookie signing key shared by every instance: 128 hex digits, e.g. from openssl rand -hex 64.`。
  debug ビルドは従来どおり、警告を出して `Key::generate()` を使う。
- 判定を `load_key(value, allow_random) -> Result<Key, String>` に切り出して、環境変数を触らずにテストできるようにした
  （Rust 2024 では `std::env::set_var` が `unsafe` で、このクレートは `unsafe_code = "deny"` にしている）。
- 経緯: マルチインスタンスで鍵がずれると、別のインスタンスが署名した cookie は「改ざん」扱いになり、**カートが黙って空になる**。
  エラーにならないので気づきにくい。そのため、起動時に落とす方が安全と判断した。

## 2026-11-03 [Next.js] #1 公式ベンチを Next.js 16.4 + Cache Components の推奨構成に書き直す

- **書こうとしたこと**: 公式 `benchmarks/nextjs`（v0.11.0 タグ、Next 15 + `force-dynamic`）の見た目とカタログのロジックを借り、
  `cacheComponents: true` / `partialPrefetching: true` の構成で一覧・詳細・ホームを書き直す。
- **土台の載せ替え**
  - `npm view next version` は 16.4.4（公開は 2026-11-02、前日）。exact で固定した。React は 19.3.1。
  - **pnpm 11 の `minimumReleaseAge` に引っかかった。** 公開から日が浅い `next@16.4.4` と `@next/swc-*` などが対象になり、
    pnpm が `pnpm-workspace.yaml` に `minimumReleaseAgeExclude` を 12 件**自動で追記**した。exact 固定と同じ意図なので、そのままコミットした。
  - TypeScript の `latest` は 7.0.2（ネイティブ版）だった。create-next-app 16.4.4 のテンプレートが `typescript: ^5` / `eslint: ^9` なので、
    5.9.3 / 9.39.5 に固定した。
  - **`../data/products.json` の import が、そのままでは解決できない。** Turbopack はロックファイルのある場所を root とみなし、
    root の外のファイルを解決しない（同梱 docs `turbopack.md` の "Root directory"）。公式ベンチは同じ相対 import で動いていたが、
    あちらは Next 15 で、build が webpack だった。最初は `turbopack.root` と `outputFileTracingRoot` をリポジトリ直下に広げて回避した。
    しかし docs は「root を広げすぎると watch が重くなる」と警告している。リポジトリ直下には `topcoat/target/` があり、`next dev` がそこまで watch しかねない。
    さらに standalone 出力が `.next/standalone/nextjs/server.js` に入れ子になる（tracing root からの相対パスになるため）。
  - **最終的な解決策: root は既定（`nextjs/`）のまま、データをパッケージ内へコピーする。** 画像と同じ `scripts/copy-data.mjs` で、
    `../data/products.json` を `src/generated/products.json`（gitignore）へコピーし、`@/generated/products.json` を import する。
    コピーは `predev` / `prebuild` に加え、`pretypecheck` / `prelint` でも走らせる（tsc が JSON の import を解決するため）。
    `../data` が唯一の正であることは変わらない。`next.config.ts` から root の設定が消えたので、standalone 出力は通常の `.next/standalone/server.js` に戻る見込み。
    「別ディレクトリの共有データを読む」という素朴な構成が、Turbopack の root の規則によってビルド前コピーという一手間に変わった。
  - 公式の `<a>` は `next/link` の `<Link>` にした。クライアント遷移と prefetch を使うのが Next の流儀なので。
- **書き味（流儀どおりに書いた箇所）**
  - **ホーム**: `getFeatured()` を `'use cache'` + `cacheLife('max')` にして await するだけ。runtime データを読まないので、ページ全体が静的になる。
  - **一覧**: ページ本体を `async` にせず、`searchParams` の Promise を子に渡して `<Suspense>` の中で await する
    （docs `caching.md` の "Maximizing the static shell"）。絞り込みは `getProductsPage(page, sort, category, search)` で、
    `'use cache'` の引数がそのままキャッシュキーになる。同じ URL の 2 回目以降は、インスタンスのメモリから返る。
  - **詳細**: `generateStaticParams` で 500 件を列挙する。ページ本体は `async` にせず、`params` も `<Suspense>` の中で await する。
    ISR の guide は「`generateStaticParams` で列挙済みの param でも、境界の内側で読め。外で await すると App Shell がその URL に縛られる」と書いている。
    そのため、列挙済みの id では中身が全部ビルド時に埋まり、それ以外の id では骨組み（App Shell）が先に出る、という 2 段構えになる。
  - **`'use cache'` をどこに付けるか**: docs の "Predictable values" によれば、module scope の import は**キャッシュ指定なしでも prerender 対象**になる。
    つまり JSON の import 自体には何も要らない。AGENTS.md の「カタログ読み込みは `'use cache'` + `cacheLife('max')`」は、
    読み出し関数（`getProduct` / `getFeatured` / `getProductsPage`）に付ける形で実装した。`'use cache'` は戻り値をシリアライズするので、
    `Map` を抱えたカタログ全体を返すより、関数ごとに結果を返す方が自然だった。`getProduct` は `undefined` ではなく `null` を返すようにした。
  - 型は `PageProps<'/products/[id]'>` を使った。これは `next typegen` が生成するグローバル型なので、`typecheck` スクリプトは `next typegen && tsc --noEmit` にした。
- **詰まった点 / topcoat との差**
  - **`dynamicParams` は Cache Components では使えない**（docs の `dynamicParams.md`）。未知の id（`/products/9999`）では App Shell を先に流してから、
    `<Suspense>` の中で `notFound()` に到達する。docs（`not-found.md`）によれば「ストリーム開始後はステータス 200 のまま、`noindex` を差し込む」。
    つまり **404 ではなく 200 + not-found UI** になる見込み。topcoat は 404 を返す。404 にしたければ、`params` を境界の外で await する
    （App Shell を諦める）か、proxy で弾くことになる。推奨構成どおりに書くと、ステータスコードの正しさと静的シェルが引き換えになる。
    **静的シェルを先に返す以上、ステータスは後から変えられない。** これは PPR の構造上のトレードオフだ。判断として、Next の流儀（200 + noindex）のまま残した。
  - 公式ベンチの `page` が数値でない場合（`?page=abc`）は 1 ページ目扱いのまま。topcoat は `query_params` の型エラーで 400 を返す。
- **気づき（配信されるもの・予想）**
  - `/`: 静的（`○`）。HTML は全部ビルド時に生成される。
  - `/products`: 部分 prerender（`◐`）。静的シェルは見出しとスケルトンで、URL に依存する部分（チップ、件数、グリッド）はリクエスト時にストリームされる。
    `searchParams` を読む限り、この部分は CDN キャッシュに載らない。`getProductsPage` の結果はインスタンス内のメモリにキャッシュされる。
  - `/products/[id]`: 500 件が SSG（`●`）。列挙済みの id では、HTML 全体がビルド時に完成している。
  - Client JS: Client Component はまだ自前で書いていない。それでも React と Next のルーター、`<Link>` の分が全ページに載る。
    topcoat の runtime（gzip 約 15KB）との比較は supervisor の計測待ち。
- **所要時間の体感**: 雛形と依存の固定で 15 分、docs（`node_modules/next/dist/docs` に同梱）の読み込みで 20 分、実装 20 分。

## 2026-11-03 [Next.js] #2 レビューはストリーム、関連商品は `'use cache'` で静的シェルに焼く

- **書こうとしたこと**: topcoat の #2 と同じ擬似 upstream（`src/lib/remote.ts`、遅延は `remote.rs` と同じ 200ms / 800ms）を使う。
  レビューは鮮度が要るデータとしてリクエストごとに取得し、関連商品は共有してよいデータとしてキャッシュする。
- **書き味**
  - 差分は小さい。レビューは「async Server Component を `<Suspense fallback={<ReviewListSkeleton …/>}>` で包む」だけで、
    topcoat の `suspense(fallback: …, reviews(…))` と同型。
  - 関連商品は、取得関数 `fetchRelated` の先頭に `'use cache'` と `cacheLife('max')` を書くだけ。
    **`<Suspense>` で包む必要すら無い。** ビルド時の prerender が 800ms を待って結果を HTML に焼くので、訪問者は待たない。
    「遅いが共有できるデータは、キャッシュして遅延を消す」が、関数 1 つへの 2 行で書ける。この点は Next の流儀がはっきり効いている。
  - 見出し「Reviews (n)」とスケルトンの枚数（`min(review_count, 3)`）は、topcoat と同じく即座に分かる値から決めた。
- **詰まった点: タイマーは「リクエスト時のデータ」と見なされるのか**
  - docs の例は、どれも「キャッシュしない `fetch`」で、それだけでリクエスト時扱いになる。一方、擬似遅延の `setTimeout` を
    prerender がどう扱うか（待つのか、穴にするのか、エラーにするのか）は docs から読み取れなかった。
  - そこで `fetchReviews` の先頭で `await connection()` を呼び、「これはビルド時には実行しない」と明示した
    （docs の "Random values and timestamps" で、リクエストごとの値に使う手段）。本物の upstream なら `fetch` なので、この 1 行は要らない。
  - `fetchRelated` 側は `'use cache'` の中で `setTimeout` を待つ。キャッシュの充填は prerender 中に待たれる（docs `use-cache.md` によれば、解決しない Promise を待ち続けた場合は 50 秒でビルドがタイムアウトする）。
- **気づき（配信されるもの・予想）**
  - 列挙済みの `/products/1` の HTML には、ビルド時の時点で**関連商品カードまで入っている**。レビュー部分はスケルトンで、
    その後、同じ応答の中で RSC の差し込み（`<template>` + インライン `<script>`）がストリームされる。
    レビューは `connection()` を呼んでいるため、リクエストごとに 200ms かかる。TTFB は静的シェルの配信の速さ、応答完了は約 200ms になる見込み。
  - topcoat は両方をストリームするので、応答完了は 800ms（テストで実測済み）。**同じ「遅い upstream が 2 つ」でも、Next は流儀どおりに書くだけで 800ms が消える。**
    ただし、その代わりに**ビルドが 500 件 × 800ms を払う**。並列に prerender されるので実時間はその何分の一かになるが、
    `next build` の所要時間は supervisor 側で要計測。
  - 関連商品の鮮度は `cacheLife('max')`（revalidate 30 日）で決まる。推薦を頻繁に変えたいなら `'hours'` などにする。
    そうすると ISR がバックグラウンドで再生成し、そのリクエストだけ 800ms を払う。topcoat には、リクエストを跨いでキャッシュする仕組みが無い。
  - レビューの差し込み自体には自前の Client JS は要らない（RSC のストリーム）。ただし React の runtime は全ページに載っている。
- **所要時間の体感**: 実装 10 分、`setTimeout` の扱いを docs で探して 10 分。

## 2026-11-03 [Next.js] #3 一覧の検索を `next/form` の GET + `searchParams` で書く

- **書こうとしたこと**: 一覧に検索欄を足す。AGENTS.md の方針どおり、`next/form` の `<Form action="/products">`（GET）と、
  Server Component が読む `searchParams` だけで作る。自前の Client Component は書かない。
- **書き味**
  - `<Form action="/products">` に hidden の `sort` / `category` と `q` の入力欄を入れるだけ。JS が無ければ普通の GET フォームで、
    JS があればクライアント遷移になる（docs `form.md`）。topcoat 版で `raw!` と `history.replaceState` を使って手で書いた「URL との同期」は、
    **そもそも URL が正なので要らない。** 検索結果のリンク（チップ、ページング）に `q` を載せる部分は、`productsUrl` に引数を 1 つ足しただけ。
  - 入力の正規化（trim、64 文字で切る、空なら無し）は `topcoat/src/urls.rs` の `normalize_search` を移植した。
    Rust の `chars().take(64)` に合わせ、`Array.from` でコードポイント単位に切っている。
    `q` の percent-encode も `encode_query_value` に合わせ、`encodeURIComponent` が残す `!'()*` を追加でエンコードした。topcoat 版と URL が一致する。
  - **検索欄の初期値も URL 依存なので、境界の中に置く必要があった。** 見出しの行に `<Suspense fallback={空の SearchForm}>` を置き、
    中で `searchParams` を await して値入りの `SearchForm` を返す。結果側の `<Suspense>` と合わせ、同じ Promise を 2 か所で await している。
    静的シェルには空の検索欄が入る。差し替えまでの間に入力した文字は、差し替えで消える（ストリームは速いので実害は小さいはず）。
  - `cacheComponents` 有効時は、遷移後も前のルートが `<Activity>` で保持される。`defaultValue` だけだと、`?q=a` から `?q=b` へ遷移しても
    入力欄が古い値のまま残りうる。そこで、input に `key={search}` を付けて作り直させた。docs によれば `<Form>` 自体に `key` は渡せない。
- **topcoat との差（挙動）**
  - topcoat は**1 文字ごとに** shard を POST で描き直す。Next は **Enter で確定**して遷移する。逐次検索にするなら、
    Client Component で `useRouter().replace` を debounce して呼ぶことになる。その分 JS が増え、流儀としては一段複雑になる。今回は採らなかった。
  - 結果は `getProductsPage(page, sort, category, search)`（`'use cache'`）が返す。同じ検索語の 2 回目は、インスタンスのメモリから返る。
    `partialPrefetching` 下では `<Form>` の prefetch は App Shell（見出しと空の検索欄）だけで、結果は遷移後に取りに行く見込み。
- **気づき（配信されるもの・予想）**
  - `next/form` は `'use client'` の部品（`next/dist/client/form.js`）なので、その分の Client JS が一覧ページに載る。
    自前の JS はゼロ。topcoat は runtime JS（gzip 約 15KB）と、shard の引数として埋め込まれる JS 式を配る。
  - 検索結果は毎回リクエスト時のストリームになる（`searchParams` 依存）。0 件時は「0 products matching “…”」と「No products found.」を出す。文言は topcoat と同じ。
- **所要時間の体感**: 実装 15 分。topcoat 版で詰まった URL 同期と JS の式検査が、丸ごと不要だった。

## 2026-11-03 [Next.js] #4 カートを Server Action + `cookies()` で実装する

- **書こうとしたこと**: 詳細の「Add to cart」、`/cart` での数量変更・削除、ヘッダーの件数表示。topcoat 版と同じく、状態は cookie に置く。
  cookie の値も topcoat と同じ形式 `[[id,qty],…]` の JSON にした（上限は 50 行・各 99 個）。
- **書き味**
  - `'use server'` のファイル（`src/app/cart/actions.ts`）に `addToCart(formData)` / `updateCart(formData)` を書き、
    `<form action={addToCart}>` に渡すだけ。JS が無い場合は、hidden の action ID 付きの普通の POST になる（docs `forms.md`）。
    topcoat の「フォーム POST + `#[route(POST)]` + 303」とほぼ同じ量で書け、ルートを別に切る必要もない。
  - Add to cart は `redirect('/cart')`（AGENTS.md の決定どおり）。
  - 数量変更は `redirect` を書かなかった。docs（`mutating-data.md`）によれば、**Server Action の中で cookie を set すると、
    Next が現在のページとレイアウトをサーバーで描き直す**。JS ありなら、ページ遷移なしでカートとヘッダーの件数が同時に更新される。
    topcoat は #541 のとおり、毎回文書全体を読み直す。
  - React 19 では、`<form action={fn}>` の送信後に、非制御の入力欄がリセットされる。数量欄は `defaultValue` なので、
    `key={quantity}` を付けて「サーバーから来た新しい数量で作り直す」ようにした。
- **署名: Next.js の標準機能には無い**
  - `cookies()` の API は `get` / `set` / `delete` だけで、署名や暗号化の手段は無い。docs で署名や暗号化が出てくるのは、
    認証ガイドの「セッションを自前で暗号化する」例（`jose` などの外部ライブラリを使う）だけだった。
  - そこで、**署名なし**の cookie にした。訪問者が書き換えられるのは自分のカートだけで、読むたびにカタログと照合する
    （未知の id は捨てる、数量は 1〜99 に丸める、重複を捨てる、50 行で切る）。価格はカタログから引くので、cookie に価格は載らない。
  - topcoat は署名付き cookie が標準で用意されているが、その代わり `COOKIE_KEY` を全インスタンスで揃える運用が要る（揃わないとカートが黙って消える）。
    Next は鍵を持たないので、その運用も要らない。「標準で付いてくる安全装置」と「運用の手間」が逆の向きに出ている。
  - CSRF: Server Action は Origin と Host（または `X-Forwarded-Host`）を比較し、不一致なら中断する（docs `data-security.md`）。
    cookie は `HttpOnly` / `SameSite=Lax` / `Secure` / `Path=/` で、topcoat と同じ属性にした。
- **詰まった点 / topcoat との差**
  - **ヘッダーの件数がレイアウトにあるので、全ページに動的な穴が開く。** 件数は `cookies()` を読む小さな `CartCount` に閉じ込め、
    `<Suspense fallback="Cart">` で包んだ。これで静的シェルは保たれる。ただし、#1 で完全に静的（`○`）だったホームと 500 件の詳細も、
    **「静的シェル + リクエスト時の差し込み」（`◐`）になる**見込み。CDN から返せるのはシェルまでで、件数のために毎回サーバーが動く。
    ページを完全に静的なまま保つなら、件数を Client Component にして、`HttpOnly` でない cookie を `document.cookie` から読む手もある。
    ただし、AGENTS.md の方針（`cookies()` + `<Suspense>`）を優先した。topcoat はもともと全ページが動的なので、この差は出ない。
  - `partialPrefetching` の docs には「`cookies()` を読むルートの App Shell はセッションのデータを含み、クライアントではセッションごとにキャッシュされる」とある。
    件数が prefetch にどう載るか（古い件数が見えないか）は、supervisor の動作確認で見てほしい。
  - **JS なしの数量変更は PRG にならない。** `redirect` しないので、POST の応答がそのまま `/cart` の HTML になる。リロードすると再送の確認が出るはずだ。
    topcoat は 303 で GET に戻す。JS ありの体験を優先した流儀どおりの書き方と、JS なしの作法がずれる例。
  - 未知の商品を Add to cart した場合、topcoat は 404 を返す。Next 版はカートを変えずに `/cart` へ redirect する。
    Server Action の中で `notFound()` を投げたときの挙動が docs から読み取れなかったので、無視する方を選んだ。
- **気づき（配信されるもの・予想）**
  - `/cart`: 見出しとスケルトンが静的シェルで、中身はリクエスト時にストリームされる。
  - フォームは Server Component のまま。自前の Client JS は増えない。ただし Server Action の呼び出しは React 側の仕組みに乗るので、
    React の runtime が要る（それはもともと全ページに載っている）。
- **所要時間の体感**: 実装 25 分。署名の有無と、件数の穴が全ページに広がる影響を docs で確かめて 10 分。

## 2026-11-03 [Next.js] #5 在庫数を Route Handler の SSE + `EventSource` で live 更新する

- **書こうとしたこと**: 詳細ページに在庫数を出し、5 秒ごとに更新する。値の計算式は `topcoat/src/stock.rs` を移植し、
  同じ時刻なら topcoat 版と同じ値になるようにする。
- **計算式の移植**
  - SplitMix64 は 64 bit の wrapping 演算なので、`number`（53 bit）では書けない。`BigInt` で書き、各演算の後に `& (2^64 - 1)` で丸めた。
    `saturating_sub(6)` は「負なら 0」に置き換えた。
  - 移植の確認: `stock.rs` の関数部分をそのまま切り出した scratch の Rust（`rustc --edition 2024`）と、`src/lib/stock.ts` を node で読み込んだ結果を比べた。
    id 5 種 × 時刻 5 点（スロット境界の前後を含む）の `stock_at` / `until_next_change` が、**25 行すべて一致**した。
    プロジェクトのテストには入れていない（Next 版にはテストランナーを入れていないため）。
  - **create-next-app の `tsconfig.json` は `target: ES2017` なので、`BigInt` リテラル（`1n`）が型エラーになる**（TS2737）。
    `ES2020` に上げた。noEmit なので出力には影響しない（変換は SWC が受け持つ）。
    ただし、上げた直後の `tsc --noEmit` は**同じエラーを出し続けた**。`incremental: true` の `tsconfig.tsbuildinfo` が古い診断を持っていたためで、
    ファイルを消すと通った。
- **書き味**
  - Route Handler（`src/app/api/stock/[id]/route.ts`）は `ReadableStream` を返すだけ。最初に `retry: 1000` と現在値を送り、
    以後は `setTimeout(send, untilNextChange(now))` で次のスロット境界ごとに 1 行ずつ送る。topcoat の `live!` の loop と同じ形になった。
    `request.signal` の abort でタイマーを止める。
  - `export const maxDuration = 300` を付けた。ストリームは、それより 10 秒早く自分から閉じる。`EventSource` は閉じられると
    `retry` の 1 秒後に再接続し、サーバーは接続時に現在値を送り直す。状態を持たない設計なので、再接続で値がずれない（topcoat と同じ性質）。
  - **`next start` の gzip がイベントを溜め込む恐れがある**。Next は内部で `compression` パッケージ（`next/dist/compiled/compression`）を使っている。
    `Cache-Control: no-cache, no-transform` を付けた。`compression` は `no-transform` を見ると圧縮を飛ばすので、溜め込みも起きないはず。
    実際にイベントが 5 秒ごとに届くかは supervisor の確認待ち。
  - 表示側は `'use client'` の `StockLevel` で、`useEffect` の中で `EventSource` を開き、`onmessage` で state を更新するだけ。
    `cacheComponents` では、遷移で隠れたルートの effect が片付けられる（docs `cacheComponents.md` の "Navigation with Activity"）。
    そのため、ページを離れると接続も閉じる。
  - **初期値は静的 HTML に入れられない**（時刻に依存するため）。`<Suspense>` の中の Server Component で `connection()` の後に時計を読み、
    現在値を `initialLevel` として Client Component に渡した。JS が無くても、その時点の在庫は見える。
- **詰まった点: `react-hooks/purity` が Server Component の `Date.now()` を弾いた**
  - `await connection()` の後で `stockAt(id, Date.now())` を JSX に書いたところ、`pnpm lint` が
    ``Error: Cannot call impure function during render`` を出した（eslint-config-next 16.4.4 が使う `eslint-plugin-react-hooks` 7.1.1）。
  - この lint は、Server Component がリクエストごとに 1 回しか描画されないことも、`connection()` で「リクエスト時に限る」と宣言したことも知らない。
    docs は「`connection()` の後なら `Date.now()` を使ってよい」と書いているので、**docs が勧める書き方を、同梱の lint が止める**形になった。
  - 時計を読む処理を `currentStock(id)`（`connection()` + `Date.now()`）として `src/lib/stock.ts` に移して回避した。
    `fetchReviews` と同じく「関数自身がリクエスト時専用だと宣言する」形になり、結果的に読みやすくはなった。
- **transport と Vercel での考察**
  - SSE は**ブラウザのタブ × 商品ごとに HTTP 接続を 1 本**持つ。HTTP/1.1 では、同じオリジンへの同時接続が 6 本までなので、
    詳細ページを 6 タブ開くと他のリクエストが詰まりうる（HTTP/2 なら多重化される）。topcoat は文書ごとに WebSocket を 1 本開き、その上で run を多重化する。
  - Vercel の Functions では、SSE の接続中、関数がずっと占有される（上限は `maxDuration`）。在庫の値は**時刻から決まり、5 秒間は誰が見ても同じ**だ。
    そのため、`GET /api/stock/1` を「次の境界まで `s-maxage`」で返す **polling** にすれば、ほぼすべてのリクエストが CDN で返り、関数は 5 秒に 1 回しか動かない。
    「サーバーから押す」より「CDN にキャッシュされた値を引く」方が、Vercel の課金と構造に素直に乗る。今回は topcoat の `live!` と同じ土俵で比べるため、SSE のまま実装した
    （AGENTS.md の方針どおり。SSE が Vercel で実際に何秒保つかは未決事項のまま）。
- **気づき（配信されるもの・予想）**
  - 詳細ページの動的な穴は 3 つになった（在庫の初期値、レビュー、ヘッダーのカート件数）。関連商品を含め、それ以外は静的 HTML のまま。
  - Client JS: `StockLevel`（`useState` + `useEffect` + `EventSource`、自前のコードは数十行）のチャンクが、詳細ページだけに載る。
    topcoat の runtime は全ページに載るので、ここは Next の「必要な所だけ Client Component」が効く。
  - SSE の 1 イベントは `data: 23\n\n` の数バイト。topcoat は WebSocket の JSON フレームで、描画済みの HTML 片を送る。
- **所要時間の体感**: 移植と突き合わせで 15 分、Route Handler と Client Component で 15 分、lint の回避で 5 分。

## 2026-11-03 [Next.js] #6 商品画像を `next/image` + `sizes` で配信する

- **書こうとしたこと**: topcoat 版と同じ 24 枚（1600×1600、約 290KB）を、同じ規則 `{category_slug}-{(id % 3) + 1}.jpg` で割り当てる。
  一覧カード・関連商品・詳細の主画像・カートのサムネイルに出す。
- **素材の持ち込み**
  - `public/` 配下しか静的配信されないので、`scripts/copy-data.mjs` で `../data/images/*.jpg` を `public/images/` へコピーし、
    `predev` / `prebuild` から呼ぶ。コピー先は gitignore した。symlink は使っていない（AGENTS.md の決定）。
  - `next.config.ts` の `images.localPatterns` を `{ pathname: '/images/**', search: '' }` に絞った。それ以外のパスや、クエリ付きの最適化要求は 400 になる。
    `qualities` は Next 16 から必須で、`[75]` だけにした。
- **書き味**
  - **500 商品 → 画像の対応は、文字列のパスを組み立てるだけ。** `src/lib/images.ts` は、カテゴリの集合と 1 行の template literal で済んだ。
    topcoat の `asset!` は文字列リテラルしか受け取れないので、24 枚の const 表を書き、表と実ファイルの一致をテストで守った。
    Next では、パスが実在しない場合の検出が「実行時の 404（最適化 API の 400 / 404）」まで遅れる。表と実ファイルの一致は、型でもビルドでも検査されない。
  - `<Image src width={1600} height={1600} sizes=… className="h-32 w-full object-cover">` と書くと、`srcset` は `deviceSizes` と `imageSizes` から自動で作られる。
    `sizes` はレイアウトから手計算した。グリッドは 72rem のコンテナに 2 / 3 / 4 列なので、
    `"(min-width: 1024px) 262px, (min-width: 768px) 33vw, 50vw"` とした。関連商品は 2 / 4 列、詳細の主画像は `lg` で 2 列（約 540px）。
    カートは `80px` 固定。Tailwind のクラスと `sizes` の文字列が別々に存在し、ずれても何も教えてくれない。そこは topcoat と同じく人間の責任になる。
- **`preload` / `loading` / `fetchPriority` の使い分け**
  - Next 16 で `priority` は非推奨になり、`preload` に置き換わった。一覧の先頭カードだけ `preload` にした（AGENTS.md の方針）。
  - ただし docs（`image.md`）は、「たいていの場合は `preload` ではなく `loading="eager"` か `fetchPriority="high"` を使え」
    「ビューポートによって LCP 候補が変わる場合は `preload` を使うな」とも書いている。
    また、**一覧のグリッドは `searchParams` 依存の `<Suspense>` の中にある**ので、`<head>` はすでに送り終わっている。
    App Router の Image は `ReactDOM.preload` を呼ぶ（`next/dist/client/image-component.js`）。ストリームの途中では `<link rel="preload">` を `<head>` に入れられず、
    `<img>` と同じチャンクで届くはずだ。その場合、preload の効果はほぼ無いと予想する。supervisor の Lighthouse で確認してほしい。
  - 詳細の主画像（LCP 候補）は、docs の勧めどおり `loading="eager"` + `fetchPriority="high"` にした。カートのサムネイルは、topcoat の修正と同じ理由で eager にした。
    `preload` と `loading` / `fetchPriority` は併用しないよう、`ProductPhoto` の中で排他にした。
- **気づき（配信されるもの・予想）**
  - 画像は `/_next/image?url=%2Fimages%2Faudio-1.jpg&w=640&q=75` のような URL で、要求された幅に縮小され、Accept に応じて WebP などに変換される。
    h-32 のカードには、2x DPR でも 640px 前後が選ばれるはずで、topcoat の原寸 290KB に対して 1 枚あたり数十 KB になる見込み。一覧 1 ページの画像総量は、桁で減るはずだ。
  - **最適化はサーバーの CPU で行われる**（初回のみ。結果は `.next/cache/images` に保存される）。standalone / docker では sharp が要る。
    pnpm 11 では、`pnpm-workspace.yaml` の `allowBuilds` で `sharp: true` を明示した（公式ベンチは画像を使わないので `false` だった）。
    Vercel では最適化は Vercel 側で行われ、関数の CPU は使わない。stateless・複数インスタンスのコンテナでは、インスタンスごとにキャッシュが温まるまで初回変換を払う。
  - `next/image` は `'use client'` の部品（`next/dist/client/image-component.js`）なので、画像を使うページには、その分の Client JS が載る。
    `onLoad` などを渡していないので、描画は Server Component から HTML として出る。JS は主にハイドレーションのためのもの。
- **所要時間の体感**: 実装 15 分、`sizes` の手計算と `preload` の docs 読みで 15 分。

## 2026-11-03 [Docker] topcoat は「バイナリ + `assets/`」の 2 点を配置する

- 書こうとしたもの: Vercel Container Runtime 用のルート `Dockerfile`（builder `rust:1.99-bookworm` → runtime `gcr.io/distroless/cc-debian12:nonroot`）。
- **成果物が 1 ファイルで終わらない。** `asset!` の実体は `topcoat asset bundle` が作る `assets/` で、
  実行時は `current_exe()` の隣を探す。そのため、`/app/storefront-topcoat` と `/app/assets/` を同じディレクトリに置く必要がある。
  「Rust ならバイナリ 1 本」と思って `COPY` を 1 行で済ませると、最初の描画で `failed to resolve asset … in the asset catalog` の panic になるはずだ（journal #4 #6 の節）。
- `topcoat asset bundle` は**内部で `cargo build` をもう一度呼び**、できたバイナリから asset の宣言を読む（`topcoat-cli/src/asset/bundle.rs`）。
  `--release` / `--profile` / `--bin` / `--package` しか受け付けず、`--locked` は渡せない。先に `cargo build --release --locked` を済ませておけば、2 回目は no-op になる。
- **BuildKit の cache mount に置いた `target/` は、その `RUN` が終わると見えなくなる。** そのため、bundle の出力先を `--out /out/assets` にし、
  バイナリも同じ `RUN` の中で `/out` にコピーした。既定の `target/release/assets` に出すと、キャッシュに古いハッシュのファイルが溜まっていく点も避けられる。
- cache mount の `target/` には mtime の罠がある。cargo は mtime で鮮度を判断する。そのため、古いコミットを checkout してビルドすると、ソースがキャッシュ済みの成果物より古くなり、
  再コンパイルされない恐れがある。対策として、ビルドの前に `src/` と `build.rs` を `touch` し、このクレートだけは毎回ビルドし直すようにした（依存はキャッシュのまま）。
- **ビルド中にネットワークが要る**。取得するのは crates.io（アプリと topcoat-cli の依存）と、`build.rs` 経由で GitHub releases から落とす Tailwind CLI の単体バイナリ。
  Tailwind CLI と bundler のダウンロードキャッシュは `target/` 配下（`topcoat_core::cache`）にあるので、cache mount に乗る。
- PORT の扱い（Vercel 公式の Rust ガイドに合わせた）:
  - Vercel のドキュメントでは、コンテナへの転送先は**既定で 80 番**。プロジェクト設定の環境変数 `PORT` で上書きできる
    （https://vercel.com/docs/functions/runtimes/container の "Port resolution"）。
  - 公式 Rust ガイドは `gcr.io/distroless/cc-debian12:nonroot` に `ENV PORT=80` を書き、`PORT` を読んで、無ければ 80 を使うよう説明している
    （https://vercel.com/kb/guide/deploy-rust-on-vercel-with-docker）。
  - nonroot でも 80 番を bind できる。Docker 20.10 以降は、コンテナの network namespace に `net.ipv4.ip_unprivileged_port_start=0` を設定するからだ。
    ホストのこの値は 1024 のままだが、コンテナの中では関係ない。
  - 結論として、Dockerfile は `ENV HOST=0.0.0.0 PORT=80`。ローカル計測（`bench/compose.yml`）では、Next.js と条件を揃えるため `PORT=8080` で上書きする。
- `COOKIE_KEY` はイメージに焼かず、実行時の環境変数で渡す。`bench/compose.yml` では `${COOKIE_KEY:?…}` にした。
  そのため、**Next.js だけを起動するときにも `COOKIE_KEY` の export が要る**（compose は補間をファイル全体に対して行う）。
- **所要時間の体感**: Dockerfile 自体は 20 分。topcoat-cli のソースで bundle の挙動（出力先・再ビルド）を確かめるのに 15 分。

## 2026-11-03 [Docker] Next.js の standalone は「3 か所を手で組み立てる」

- 書こうとしたもの: `nextjs/Dockerfile`（builder `node:24-bookworm-slim` + corepack の pnpm 11 → runtime `gcr.io/distroless/nodejs24-debian12:nonroot`）。
  ビルドコンテキストはリポジトリ直下（`copy-data` が `../data` を読むため）。
- `output: 'standalone'` の出力は、**そのままでは動く形になっていない**。`.next/standalone/` にはサーバーと、トレースされた `node_modules` だけが入る。
  `.next/static`（クライアント JS / CSS）と `public/`（ここでは `copy-data` が作る `public/images`）は、CDN に置く前提で**含まれない**ので、
  `standalone/.next/static` と `standalone/public` に手でコピーする。忘れると、HTML は返るのに JS・CSS・画像が 404 になる。
- `outputFileTracingRoot` を既定（`nextjs/`）のままにしているので、`server.js` は `standalone/` の直下に出る。
  `turbopack.root` を広げていたら（D25 でやめた）、`standalone/nextjs/server.js` のように一段深くなっていたはずだ。
- **`next/image` は実行時に `.next/cache/images` へ書き込む。** distroless の nonroot（uid 65532）で動かすので、`COPY --chown=65532:65532` にした。
  root 所有のままだと、最適化のたびにキャッシュの書き込みに失敗するはずだ（未確認。supervisor の実行で確認する）。
- sharp: Next.js 16.4.4 は `sharp@0.35.5` を optionalDependencies に持つ。pnpm は `@img/sharp-linux-x64` と `@img/sharp-libvips-linux-x64`
  （prebuilt、`libc: glibc`）を入れる。ビルド用イメージ（bookworm）と実行用イメージ（distroless debian12）はどちらも glibc 2.36 で揃う。
  `node` 本体のために distroless/nodejs には libstdc++ も入っている。standalone のトレースに sharp が含まれるかは、
  supervisor の実行で `/_next/image?...` が 200 + `image/webp` を返すことで確かめる（`bench/run.sh assets` の non-200 列でも分かる）。
- distroless/nodejs の `ENTRYPOINT` は `/nodejs/bin/node` なので、`CMD ["server.js"]` だけを書く。`HOSTNAME=0.0.0.0` と `PORT=8080` は `server.js` が読む。
- corepack は Node 24 には同梱されている（Node 25 で同梱をやめた）。`packageManager: pnpm@11.24.0` を読んで pnpm を取得する。
  pnpm の store は cache mount に置いた。node_modules とは別のファイルシステムになるので、ハードリンクではなくコピーになる。
- **所要時間の体感**: 15 分。

## 2026-11-03 [Docker] distroless ではヘルスチェックを書けない／各ビルドが落とすもの

- distroless にはシェルも curl も無いので、Dockerfile の `HEALTHCHECK` と compose の `healthcheck:` は書けない（`CMD-SHELL` も `curl` も無い）。
  アプリ側に「自分自身を叩くサブコマンド」を足せば書けるが、今回はアプリのコードを変えない方針にした。
  代わりに、`bench/run.sh` の `wait_ready` が**ホストから `/` を 10ms 間隔でポーリング**し、200 が返るまで待つ。コンテナが落ちたら、ログを出して即座に失敗する。
  `depends_on: condition: service_healthy` も使えないので、起動順の制御はスクリプト側で行う。
- 各ビルドが初回に取得するもの（いずれもキャッシュが効けば 2 回目以降は不要）:

  | ビルド | 取得するもの |
  |---|---|
  | topcoat | `rust:1.99-bookworm`（圧縮で約 0.5GB）、`distroless/cc-debian12`（約 10MB）、crates.io（アプリ + topcoat-cli の依存）、Tailwind CLI の単体バイナリ（GitHub releases） |
  | Next.js | `node:24-bookworm-slim`（約 80MB）、`distroless/nodejs24-debian12`（約 50MB）、corepack 経由の pnpm 本体、npm レジストリの依存（`next` の SWC バイナリと sharp / libvips の prebuilt を含む） |

  Next.js は Turbopack も SWC もネイティブバイナリ同梱の npm パッケージなので、ツールチェーンのダウンロードは依存の取得と一体になる。
  topcoat は Rust ツールチェーン（ベースイメージ）・CLI（`cargo install` でソースからビルド）・Tailwind CLI と、取得経路が 3 つに分かれる。
- `docker buildx build --check` は両 Dockerfile とも警告なしだった。実ビルドは supervisor が行う。サイズと所要時間はその結果で埋める。

## 2026-11-03 [topcoat][実験] #6 画像最適化（事前リサイズ + `srcset` / `sizes`）を手で書く

- **書こうとしたこと**: ブラウザ計測（article-notes 8 節）で、topcoat の LCP 差の主因は画像だと分かった
  （`/products` で画像 4.2MB、Slow 4G で LCP 8.6 秒）。Next.js の `next/image` と同等の最適化を topcoat で手書きし、
  どれだけの手間でどこまで追いつけるかを記録する（D32）。ブランチは `exp/topcoat-images`。
- **やったこと**
  1. **事前リサイズ**: `scripts/gen_variants.py`（55 行）を追加した。`data/images/*.jpg` の 24 枚から、
     幅 320 / 640 / 960 / 1600 の WebP（quality 75。Next 版の `qualities: [75]` に合わせた）を `data/images/variants/` に書き出す。
     96 ファイルで合計 780KB。1 枚あたりの平均は 320w が 1.8KB、640w が 4.1KB、960w が 7.2KB、1600w が 20KB。
     原寸 JPEG（約 290KB）に比べて、同じ 1600px でも 1/14 になる。合成画像は平坦な部分が多く、WebP が効きやすい。
     実写ではもっと重くなる。出力は決定的（2 回生成して sha256 が一致）で、所要 6 秒。
     既存の `data/images/*.jpg` は変えていない。Next.js 版はこれまでどおり原寸を `next/image` で縮小する。
  2. **const 表**: 24 行 × 3 の手書き表（72 行）を、`macro_rules!` 2 つと 8 行の呼び出しに置き換えた。
     ```rust
     macro_rules! photo {
         ($slug:literal, $n:literal) => { photo!($slug, $n, ["320", "640", "960", "1600"]) };
         ($slug:literal, $n:literal, [$($width:literal),*]) => {
             [$(asset!(concat!("../../data/images/variants/", $slug, "-", $n, "-", $width, ".webp"))),*]
         };
     }
     const IMAGES: [(&str, [Photo; VARIANTS]); 8] = [category!("audio"), /* … */];
     ```
     - **#6 の記述の訂正**: `asset!` は proc macro ではなく `macro_rules!` で、`const PATH: &str = $path;` として受け取る
       （`topcoat-asset/src/asset.rs`）。そのため **`concat!` で組み立てた const 文字列も渡せる**。
       ドキュメントの「文字列リテラル」は「コンパイル時定数」の意味だった。`format!` は const でないので渡せない。
     - `file!()`（AssetId と `../` の基準）は、マクロ展開の起点、つまり `images.rs` を指す。そのため相対パスは手書きのときと同じ基準で解決される。
       debug ビルドのテストバイナリには、96 個のパスが埋め込まれていることを `grep -a` で確認した。bundler が拾えるかは supervisor の `topcoat asset bundle` で確かめる。
     - **build.rs は使わなかった。** 生成ファイルを `include!` する場合の `file!()` の基準を調べる必要がなく、`macro_rules!` だけで済んだ。
       ただし `concat!` は `const` を読めないので、幅を `WIDTHS` 定数とマクロ内のリテラルの 2 か所に書く必要がある。
       スクリプトを入れると 3 か所になる。ずれはテストで検出する（表とディスクの突き合わせ、WebP ヘッダの幅と宣言幅の一致）。
  3. **`product_photo`**: `srcset` は URL の列なので、`view!` 内で `Asset` を直接並べられない
     （`src=(asset)` は描画時に解決されるが、`"url 320w, url 640w"` の文字列は自分で組み立てるしかない）。
     そこで、`asset_config(cx).resolve(asset)` で URL を得て `format!` + `join` した。
     `sizes` は必須の引数にした。`eager` と `priority`（`loading="eager"` + `fetchpriority="high"`）は `#[default]` 付きの省略可能な引数にした。
     `width` / `height`（1600）は残した。
  4. **表示箇所ごとの `sizes`**: Next 版と同じ文字列を使った。一覧とトップは `GRID_IMAGE_SIZES`（`product_card` の既定値）、
     関連商品は `RELATED_IMAGE_SIZES`、詳細の主画像は `(min-width: 1152px) 540px, (min-width: 1024px) 50vw, 100vw`、カートは `80px`。
  5. **LCP 候補**: Next 版の `preload` に合わせて、一覧の先頭カード（`index == 0`）と詳細の主画像だけを `priority` にした。
     トップは Next 版も何もしていないので、揃えて何もしていない。カートは従来どおり `eager` だけにした。
     - **`<link rel="preload">` は入れられなかった**: `<head>` は共有の `root_layout` が書いており、
       ページから `<head>` に要素を差し込む API は 0.11.0 に無い（router / view を grep しても head 関連の仕組みは見当たらない）。
       layout に「この要求の LCP 画像」を渡す経路を自作するか、レスポンスヘッダの `Link: <…>; rel=preload; as=image; imagesrcset=…` を
       ページで付けるかになる。topcoat の HTML は小さく（`/products` で 3.8KB）、先頭の `<img>` は最初のチャンクに入るので、
       プリロードスキャナがすぐ見つける。`fetchpriority="high"` で足りると判断し、どちらもやっていない。
       Next 版は、Suspense の中で `preload` しても `<head>` に入らない（[Next.js] #6）。結局、両版とも実質は「`<img>` に優先度を付ける」だけになる。
- **手間（定量）**: `git diff --stat`（コード）は 6 ファイルで +246 / −119。新規の `gen_variants.py` が 55 行、生成物が 96 ファイル・780KB。
  内訳は次のとおり。

  | 部分 | 行数（+/−） | 中身 |
  |---|---|---|
  | `scripts/gen_variants.py` | +55 | PIL でリサイズ + WebP 化 |
  | `images.rs` の本体 | +65 / −71 | マクロ 2 つ、`ProductImage`（`fallback()` / `widths()`）、手書き表（72 行）の削除 |
  | `images.rs` のテスト | +69 / −21 | 表とディスクの突き合わせ、WebP の幅検査、test helper の追随 |
  | `_components.rs` | +50 / −8 | `srcset` の組み立て、`sizes` 定数 2 つ、`product_card` / `product_photo` の引数 |
  | 呼び出し側（一覧・詳細・関連・カート） | +11 / −7 | `sizes` / `priority` を渡す |
  | `app.rs` のテスト | +51 / −12 | カートの `srcset` / `sizes`、LCP 候補だけが `fetchpriority` を持つこと |

  **Next.js の場合**: `<Image sizes="…">` に 1 属性を足すだけ（+ `sizes` 定数 2 つ、`preload` 1 か所）。
  リサイズ・形式変換・`srcset` の生成・URL の解決は、フレームワークが全部やる。
  topcoat では、その 4 つを**スクリプト（ビルド外）・マクロ・コンポーネント・テスト**に分けて自分で書いた。
  **手で書いた量は、プロダクションコードの追加が約 125 行（マクロ化で手書き表 71 行が減ったので、削除を差し引いた正味は約 +40 行）、
  スクリプトが 55 行、テストが約 120 行**。
  `sizes` を手計算する手間と、Tailwind と `sizes` がずれても誰も教えてくれない点は、どちらも同じ。
- **できなかったこと／Next との差（予想）**
  - **配る幅の粒度**: Next は `deviceSizes` + `imageSizes`（16〜3840 の 16 段階）から選ぶ。topcoat 版は 4 段階だけ。
    カートの 80px でも 320w（約 1.8KB）が届く。Next なら 96w か 128w になる。段階を増やすと、ファイル数もマクロのリテラルも増える。
  - **形式の交渉**: Next は `Accept` を見て AVIF / WebP を返す。topcoat 版は WebP 固定（JPEG のフォールバックは置いていない）。
    `<picture>` で AVIF を足すと、ファイル数とマークアップが倍になる。
  - **原画の差し替え**: Next は原画を置けば済む。topcoat 版は `gen_variants.py` の再実行とコミットが要る。生成忘れはテストが落ちて分かる。
- **ビルド時間・バイナリサイズの見込み（実測は supervisor）**
  - `asset!` 1 個につき 2048B の `static`（`ENCODED_ASSET_SIZE`）がバイナリに載る。宣言が 24 → 96 個になったので、**バイナリは約 +144KB**。
    const 評価は 96 回だが、proc macro ではないので、コンパイル時間への影響は小さいはずだ。
    clippy の増分（`storefront-topcoat` だけ）では、体感の差は無かった。
  - **asset bundle は小さくなる**: topcoat は原寸 JPEG（24 枚・6.7MB）を宣言しなくなった。bundle の画像は WebP 96 枚・約 0.8MB になる見込み。
    docker イメージもその分軽くなるはず。`data/` 全体は builder に COPY されるが、runtime ステージには bundle だけが入る。
  - **SSR の CPU**: カード 1 枚ごとに URL を 4 回解決し、`format!` + `join` する。`/products`（24 枚）なら、1 リクエストで約 100 回の小さな文字列確保が増える。
    RPS にわずかに効く可能性がある。HTML も `srcset` の分だけ増える（1 枚あたり約 250B。br 圧縮後はわずか）。
- **期待する配信（確認ポイント）**: 1280×900・DPR 1 なら、一覧カードは `sizes` が 262px なので 320w（約 1.8KB）、詳細の主画像は 540px なので 640w（約 4KB）になる。
  DPR 2 なら、それぞれ 640w と 1600w になる。`/products` 1 ページの画像は、計測時の 4.2MB に対して数十 KB になる見込みで、Next 版（33KB）と同じ桁になる。
- **詰まった点**: 大きな詰まりは無かった。一番の発見は「`asset!` はリテラル専用ではない」こと。#6 で書いた
  「500 枚なら build.rs しかない」も、規則性があるファイル名なら `macro_rules!` で済む。
  逆に、`srcset` のための URL 解決が `view!` の外に出ることは、`asset!` の設計（URL は描画時に `Cx` から解決）から来る小さな段差だった。
- **所要時間の体感**: 画像スクリプト 5 分、`asset!` の実装を読んでマクロ化 10 分、コンポーネントと呼び出し側 10 分、テスト 10 分、記録 15 分。
  記録を除けば約 35 分。Next.js 版の画像対応（30 分）とほぼ同じだが、その半分以上は「最適化の部品を自作する」作業だった。

## 2026-11-03 [Next.js][実験] #4 ヘッダーのカート件数を Client Component に移し、動的な穴を外す

- **書こうとしたこと**: ローカル docker 計測（article-notes 7 節）で、Next の `/` は 95 RPS（p99 789ms）、topcoat は 5,677 RPS だった。
  Next は、ヘッダーのカート件数（`cookies()` + `<Suspense>`）のために全ページが ◐ になり、自前ホストでは毎回 resume 描画が走っていると推定した。
  **「動的な穴 1 つ」の CPU コスト**を測るため、件数をサーバーで読まない形に変える（D33）。ブランチは `exp/nextjs-static-header`。
- **やったこと**
  - `writeCart`（Server Action からだけ呼ばれる）で、本体の `cart`（HttpOnly のまま）に加えて、
    **表示用の `cart_count`**（`HttpOnly` なし、`SameSite=Lax` / `Secure` / `Path=/`）にも件数を書くようにした。
  - ヘッダーの件数を Client Component `CartCount`（`src/components/cart-count.tsx`）にした。
    `useSyncExternalStore` で `document.cookie` から読み、Cookie Store API の `change` イベントで読み直す。
    Server Action の応答が cookie を書き換えると、その場で件数が変わる。
  - cookie 名とパーサは `src/lib/cart-count.ts` に分けた。`lib/cart.ts` は `next/headers` を import しているので、Client Component から読めない。
    また、`'use client'` のファイルから定数を export すると、サーバー側では client reference になってしまう。そのため、指示子の無い第 3 のファイルが要った。
  - `site-nav.tsx` から `<Suspense>` と `cookies()` が消え、レイアウトは request データに一切触らなくなった。
- **ちらつきの扱い**: `useSyncExternalStore` の `getServerSnapshot` で `null` を返し、そのときは件数なしの「Cart」を描く。
  これは旧版の Suspense fallback と同じ文字列なので、**静的 HTML は旧版のシェルと同じ見た目**になる。
  hydration の直後に「Cart (3)」に変わる。「Cart (0)」を先に出してから正しい数に変わる、という誤表示は起きない。
  hydration 時の不一致エラーも出ない（サーバー値で hydration してから、クライアント値で再描画するのが `useSyncExternalStore` の仕様）。
  旧版では件数が同じ HTML ストリームの中で届いていたので、**件数が見えるまでの時間は「サーバーの差し込み」から「JS の読み込み + hydration」に延びる**。
  JS なしでは、ずっと「Cart」のままになる（カートの操作自体は JS なしでも動く）。
- **手間（定量）**: `git diff --stat`（コード）は 4 ファイルで +80 / −30。新規は `cart-count.tsx`（46 行）と `cart-count.ts`（20 行）、
  `cart.ts` は +10 / −7、`site-nav.tsx` は +4 / −23。所要は実装 15 分ほど。設計の判断（どの cookie に何を置くか、ちらつき、client reference の罠）の方が時間を使った。
- **トレードオフ**
  - **HttpOnly でない cookie**: 件数（整数）だけなので、XSS で漏れても失うものは小さい。ただし「カートを HttpOnly で守る」という D26 の理由は、件数の分だけ弱まる。
    また、cookie は全リクエストに載るので、上りが十数バイト増える。
  - **件数の二重管理**: 本体と件数は `writeCart` の 1 か所でしか書かないが、それでも次の場合にずれる。
    (1) この変更より前から `cart` を持つ訪問者は、次にカートを更新するまで「Cart (0)」。
    (2) カタログから商品が消えると、本体の読み取り時に捨てられる行が、件数には残る。
    (3) 訪問者が `cart_count` を書き換えられる（表示が変わるだけで、カートの中身と価格には影響しない）。
    Server Component は cookie を書けないので、`/cart` の描画時に件数を直すことはできない。直すなら Server Action か Route Handler、または middleware（proxy）が要る。
  - **Cookie Store API の無いブラウザ**: `change` を購読できないので、Server Action の後に件数が変わるかは、ヘッダーが再描画されるかどうかに依存する（未検証）。
    Add to cart は `/cart` へ redirect するので、その経路の確認が要る。
- **各ページの予想（`next build` の表示）**
  - `/`: **○ Static**。レイアウトは request データに触らなくなり、ページは `'use cache'` の `getFeatured` だけを読む。`cookies()` / `headers()` / `connection()` / `searchParams` はどこにも無い。
  - `/products`: ◐。`searchParams` を待つ 2 つの穴（`SearchFormForQuery`、`ProductResults`）が残る。
  - `/products/[id]`（500 件）: ◐。`connection()` を待つ 2 つの穴（在庫 `LiveStock`、レビュー `Reviews`）が残る。prerender されていない id は、`ProductDetail` 全体が穴になる。
  - `/cart`: ◐。`CartContents` が `cookies()` を読む（意図どおり）。
  - `/api/stock/[id]`: ƒ（変化なし）。
- **計測で見たいこと（予想）**: `/` は prerender 済みの HTML をそのまま返すだけになるので、RPS は桁で上がるはず。
  `/products` と `/products/1` は穴が 1 つ減るだけなので、差が小さければ「resume 描画そのもの」が重いことになる（穴の数ではなく、穴が 1 つでもあるかどうか）。
  応答ヘッダでは、`/` の `x-nextjs-postponed` が消え、`Cache-Control` が `s-maxage` 付きになる見込み。
- **注記: Vercel では結論が変わりうる。** Vercel では、◐ のページでも静的シェルはエッジから返り、関数は穴の部分だけを描く。
  自前ホスト（`next start`）では、シェルの送出も resume も同じ Node プロセスの 1 CPU で行うので、穴 1 つのコストがそのまま RPS に出る。
  「動的な穴を外すと速くなる」は自前ホストの結論であり、Vercel ではまず TTFB（シェル）は変わらず、関数の起動回数と課金に効く、という形で現れるはず → デプロイ時に確認。

## 2026-11-03 [DX] Cloudflare Tunnel 越しに開発サーバーを開く（`scripts/dev-remote.sh`）

- **Next.js**: 公開ホスト名を `allowedDevOrigins` に入れないと、`/_next` の dev 資源と HMR が 403 で弾かれる（実測: 許可外ホストは 403、ログに設定例が出る）。env で渡す形にした。
- **topcoat**: `topcoat dev` は `PORT` が埋まっていると**黙って次の番号に移る**（`topcoat-cli` `src/dev/port.rs`）。tunnel の向き先とずれるので、スクリプト側で空きを確認してから起動する。
- **topcoat のライブ更新**: ページ更新用の `topcoat::dev::script` は `TOPCOAT_DEV_URL`（CLI が `http://127.0.0.1:59039` 付近に立てる broadcast サーバー）を `<script src>` に入れる。トンネル越しのブラウザからは届かない（https ページから http の loopback を参照する形）。そもそも本リポジトリの layout にはこの script を入れていないので、ローカルでも手動リロード。Next.js の HMR は同一オリジンの WebSocket なので、トンネル越しでもそのまま動く。

## 2026-11-03 [Next.js][DX] iOS の Chrome で開くと `<html>` のハイドレーション不一致

- トンネル越しに iPhone で開くと、dev overlay に `A tree hydrated but some attributes ... didn't match` が出た。差分は `<html>` に付いた `__gcrremoteframetoken="…"` だけ（サーバー側に無い属性）。
- コードの不具合ではない。iOS の Chrome（`__gCrWeb` 系の注入スクリプト）が、React の読み込み前に `<html>` へ属性を足していると見られる。ローカルの Chromium（iPhone 相当の表示）では再現しない。
- 続けて `/products/1` で、`<form>` と `<input>` に付いた `__gcruniqueid="1"`, `"2"` でも同じエラーが出た（フォームの自動入力用に、iOS の Chrome が番号を振っていると見られる）。
- 対処: **コードは変えない**。`<html>` に `suppressHydrationWarning` を付けても、フォームの要素ごとに付けて回ることになり、比較対象のコードを汚す。属性の食い違いは本番ビルドでは報告されず、表示も壊れないので、dev overlay にだけ出る雑音として扱う。iPhone で確認するときは Safari を使う。
- **比較の材料**: topcoat はクライアントで木を作り直さない（サーバーの HTML をそのまま使い、`$()` の部分だけ JS を結びつける）ので、ブラウザや拡張が属性を足してもこの種のエラーは起きない。ハイドレーションという仕組みがある側だけが「外部が HTML に触る」ことを気にする必要がある。

## 2026-11-03 [Vercel] topcoat の Container Runtime ビルドに 10 分かかった（初回）

- Vercel 上で `Dockerfile` をビルドする初回デプロイが 10 分。ローカルの初回（topcoat-cli のソースビルド + release ビルド）は 207 秒だった。Vercel 側でレイヤーキャッシュや `RUN --mount=type=cache` が効くかは docs に記載なし（2 回目以降で確認する）。
- 既製イメージを持ち込む方法は docs に無い。Vercel Container Registry（`vcr.vercel.com`）へ `vercel vcr build docker . --push` で push はできるが、それを関数として動かす手順は書かれていない。ghcr.io など外部レジストリの参照・pull secret も記載なし（https://vercel.com/docs/functions/runtimes/container, https://vercel.com/docs/container-registry）。
- 未検証の回避策: `FROM ghcr.io/…:<tag>` 1 行の Dockerfile を Vercel にビルドさせる（公開イメージなら認証不要のはず）。
- Next.js（ネイティブ）との対比は Next.js のデプロイ後に記録する（見込みは 2〜3 分）。Rust のコンテナは「コンパイル時間がそのままデプロイ時間」になる。
  - → 実測（11-03）: Next.js の初回ビルド（Vercel ネイティブ、キャッシュなし）は **109 秒**。topcoat は手元ビルド + prebuilt に切り替えた後、`vercel build`（キャッシュ済みイメージを VCR に push）9 秒 + デプロイ 4 秒。

## 2026-11-03 [Vercel][topcoat] COOKIE_KEY の形式違いで全リクエストが 500

- ダッシュボードで手入力した `COOKIE_KEY` が 128 桁の hex でなく、起動時に `COOKIE_KEY must be hex-encoded` で panic（`src/cart.rs` の `signing_key`）。Container Runtime はリクエストのたびにコンテナを起動し直し、毎回 `Application exited with code 101.` → 500。
- 設計どおりの fail-fast（不正な鍵で黙って動かない）だが、Vercel ではデプロイ自体は成功扱いで、最初のリクエストまで気づけない。鍵を `od -An -tx1 -N64 /dev/urandom | tr -d ' \n'` などで 128 桁の hex として作り直し、登録し直す。

## 2026-11-03 [Vercel][topcoat] Container のビルドは buildah で、前回のキャッシュが効かない

- ビルドログ（`vercel inspect <url> --logs`）より: ビルダーは **buildah 1.43.4**（BuildKit ではない）。2 回とも冒頭に `Previous build caches not available.`。Redeploy（2 回目）でも `cargo install topcoat-cli` から全部やり直した。
- 内訳（1 回目、計 10 分）: ベースイメージ取得 28 秒 → `topcoat-cli` のソースビルド **5 分 24 秒** → アプリの release ビルド **4 分 05 秒** → distroless へコピー数秒。
- `RUN --mount=type=cache` は構文エラーにはならないが、ビルドをまたいで中身が残らない。レイヤーキャッシュも引き継がれない。Rust は「依存込みのフルビルド時間 = 毎回のデプロイ時間」になる。
- Next.js（Vercel ネイティブ）はビルドキャッシュ（`.next/cache`）を Vercel が保存・復元する。この差はフレームワークではなく「プラットフォームのネイティブ対応か、汎用コンテナか」の差。

## 2026-11-03 [Vercel][topcoat] `vercel build` + `deploy --prebuilt` がコンテナでも使える

- docs の Container のページには書かれていないが、`vercel build --prod` を手元で実行すると、ローカルの docker で `Dockerfile` をビルドし、Vercel Container Registry に push して `.vercel/output/functions/container.func` を作った（キャッシュ済みで 19 秒、うち push 15 秒）。あとは `vercel deploy --prebuilt` で Vercel 側のビルドなしに出せる。
- 「キャッシュが効く場所でビルドして成果物だけ送る」は Vercel 公式の CI 手順（Using Vercel CLI for custom workflows）と同じ形。Next.js でも同じ手順が使えるので、Rust 専用の回避策ではなくプラットフォームの正規の道。
- これで ghcr を public にする必要も、`FROM` 1 行の Dockerfile を Vercel にビルドさせる回避策も要らなくなった（ghcr は VPS 用に残す）。

## 2026-11-03 [Vercel][topcoat] イメージは Vercel Container Registry に溜まり、Hobby は 50 件まで

- Vercel 側のビルドも `vercel build`（手元・GHA）も、イメージを VCR（`vcr.vercel.com/<team>/<project>/dockerfile`）に push する。`container.func` の `handler` はその digest を指すだけで、ghcr から直接 pull されることはない（ghcr は受け渡し用で private のままでよい）。
- 料金はストレージ $0.10/GB のみで 12MB 程度なら無視できる。効くのは件数の上限で、Hobby は 1 リポジトリ 50 イメージ。自動削除・保持期間の記載は無い（https://vercel.com/docs/container-registry/limits-and-pricing）。docker で push すると Index も 1 件として数えられ、1 デプロイで約 2 件増える。
- 古いイメージを消すと、それを使うデプロイへのロールバックが壊れるはず。当面は掃除せず、上限が近づいたら手で消す。Next.js 版にはこの管理対象自体が無い。

## 2026-11-03 [Vercel][topcoat] hnd1 に移したあとのレイテンシ（国内の回線から curl / Playwright）

- 新規接続ごとの TTFB は 100〜160ms。内訳は DNS 26 + TCP 17 + TLS 30 前後で、サーバー処理と往復は 25〜40ms。接続を再利用すると `/products` の TTFB は 25〜49ms（1 回目のみ 118ms）。
- HTML は `cache-control: public, max-age=0, must-revalidate` で毎回 `x-vercel-cache: MISS`（コンテナが毎回描画）。`/_topcoat/assets/*` は `immutable` なので 2 回目から CDN の HIT。topcoat 側で何もしなくても静的資源は Vercel の CDN に載る。
- `/products/1` は TTFB 110〜140ms、完了まで約 910ms。関連商品の擬似遅延（800ms）を streaming で待つ分。ブラウザの load も 978ms。
- 在庫の `live!` は `wss://…/products/1` で接続でき、12 秒で 3 フレーム受信して値も変わった。Container Runtime 上で WebSocket の Upgrade は通る（長時間保持の上限は未確認）。

## 2026-11-03 [Vercel][Next.js] corepack はリポジトリのルートの package.json しか見ない

- `nextjs/package.json` に `packageManager: pnpm@11.24.0`、環境変数に `ENABLE_EXPERIMENTAL_COREPACK=1` を入れても `Could not enable corepack because package.json is missing "packageManager"` と出て、古い pnpm で `Ignoring not compatible lockfile` → `ERR_INVALID_THIS` の連続で止まった。
- 当初 Node 26（corepack 非同梱）を疑って 24.x に下げたが、結果は同じだった。CLI のソース（`vercel/dist/commands/build/index.js` の `initCorepack`）を読むと、`join(repoRootPath, "package.json")` を読んでいて、Root Directory（`nextjs/`）の package.json は見ていない。
- 回避: リポジトリのルートに `{"private": true, "packageManager": "pnpm@11.24.0"}` だけの package.json を置く。モノレポでない「サブディレクトリに 1 アプリ」の構成で踏む。所要 30 分ほど。
- topcoat 版（Container）はビルドに Node を使わないので、この種の詰まりは無い。

## 2026-11-03 [Next.js][topcoat][計測] 遷移の仕組みとページごとの初回ロードの重さ（Vercel 本番、hnd1）

### Next.js の遷移で通信がほとんど出ない仕組み

1. **`<Link>` の先読み**: 本番ビルドでは、画面に入ったリンクの行き先を RSC で prefetch する。`cacheComponents` + `partialPrefetching` なので取るのはルートの構造（`_tree`）とキャッシュ済みの静的シェルだけ。詳細ページのシェルは URL に依存しない作り（`products/[id]/page.tsx` のコメント）なので、リンクが何十本あってもリクエストは数本で済む。
2. **Router Cache**: 先読みした結果はブラウザ内に保持され、クリック時はサーバーを待たずに骨格を描く。
3. **差分だけ取得**: クリック時に出たのは `/products/1` の RSC fetch 1 本（キャッシュしないレビュー・カート件数をストリームで受け取る）、在庫表示の Client Component の JS 1 本、画像 1 枚、在庫の SSE だけ。HTML の取り直しや全体の再読み込みは無い。
4. 戻る・進むは Router Cache をそのまま使うので、ほぼ通信なし。
5. DevTools では遷移時の RSC が Doc ではなく Fetch/XHR（`?_rsc=`）に出て小さいので、「通信が無い」ように見える。

topcoat の `<a>` は普通のリンクで、遷移のたびに HTML を丸ごと取り直す（TTFB 100ms 前後 + 描画）。

### ページごとの初回ロード（ヘッドレス Chromium、1280×800、キャッシュなしの新規コンテキスト、国内の回線、2 回目の値）

| ページ | 版 | TTFB | FCP | load | リクエスト | 転送量 | うち JS | うち prefetch |
|---|---|---|---|---|---|---|---|---|
| `/` | Next.js | 100 | 196 | 243 | 22 | 183KB | 153KB（8 本） | 6 本 / 2.5KB |
| | topcoat | 97 | 212 | 185 | 9 | 34KB | 15KB（1 本） | — |
| `/products` | Next.js | 96 | 164 | 259 | 31 | 208KB | 153KB | 6 本 / 2.7KB |
| | topcoat | 104 | 192 | 190 | 18 | 53KB | 15KB | — |
| `/products/1` | Next.js | 89 | 220 | 383 | 18 | 185KB | 159KB（9 本） | 4 本 / 1.4KB |
| | topcoat | 124 | 220 | 943 | 6 | 33KB | 15KB | — |
| `/cart` | Next.js | 118 | 176 | 225 | 14 | 164KB | 153KB | 4 本 / 1.5KB |
| | topcoat | 102 | 160 | 158 | 3 | 21KB | 15KB | — |

（時間は ms。1 回目はコールドスタートが混じり、topcoat `/` の FCP 2432ms、Next.js `/products/1` の FCP 516ms などが出た）

- **先読みの上乗せは小さい**: 1 ページあたり 4〜6 本・1.4〜2.7KB。初回ロードの重さの原因ではない。表示速度にも効かないので、注意点は「見えているリンクの分だけリクエストが出る（ただしシェル単位で共有される）」程度でよい。
- **初回ロードの差は JS の量**: Next.js は React と Next のランタイムで約 153KB を毎ページ読む。topcoat は 15KB 1 本。総転送量は topcoat が 1/4〜1/8。
- **それでも最初の描画（FCP）はほぼ同じ**: どちらも HTML を SSR で返すので、160〜220ms に収まる。速い回線と PC では JS の差は体感に出ない。遅い回線や低性能の端末では、Next.js の JS の解析とハイドレーションが効いてくるはず（未計測）。
- **`/products/1` は設計の差がそのまま出る**: topcoat は関連商品（擬似遅延 800ms）を毎回ストリームで待つので load が 943ms。Next.js は関連商品を `'use cache'` で静的シェルに焼いてあり、待つのはレビュー（200ms）だけ。topcoat にはリクエストをまたぐキャッシュ機構が無い。
  - ⚠ 遅延の定義は両版とも 800ms で同じ（`topcoat/src/remote.rs`, `nextjs/src/lib/remote.ts`）。違うのは払う回数で、Next.js は `cacheLife("max")` によりビルド時に商品ごと 1 回、topcoat は毎リクエスト（AGENTS.md #2 で意図した差）。Next.js 側の代償は鮮度（revalidate か再デプロイまで古いまま）。topcoat はプロセス内キャッシュで近づけられるが、stateless・複数インスタンス・アイドル停止の前提では温まったインスタンスでしか効かない。ページ全体の CDN キャッシュもカート件数（cookie）を含むので使えず、差の本体は「1 ページ内で静的部分と動的部分を分けられるか」。
- **ページ間の遷移は Next.js が圧倒的に有利**: 2 ページ目以降はシェルが手元にあり、差分しか取らない。topcoat は毎回フルロード。

### ブラウザのキャッシュあり・なし、遷移の比較（`/products/1`、3 回の中央値）

ヘッドレス Chromium、1280×800、国内の回線。CDN とサーバーは温まった状態。「h1」と「関連商品」は、ナビゲーション開始（A・B は `goto`、C はクリック）から、商品名と関連商品の 1 件目が DOM に出るまでの時間（ms）。転送量は CDP の `encodedDataLength` の合計。

| 状況 | 版 | h1 | 関連商品 | リクエスト（うちキャッシュ） | 転送量 |
|---|---|---|---|---|---|
| A 初回（ブラウザのキャッシュ空） | Next.js | 321 | 323 | 22（0） | 185KB |
| | topcoat | 164 | 946 | 6（0） | 33KB |
| B 再訪問（同じタブでもう一度 `goto`） | Next.js | 55 | 59 | 23（10） | 13KB |
| | topcoat | 62 | 847 | 6（5） | 4KB |
| C 一覧 → 詳細（未訪問の商品をクリック） | Next.js | 126 | 128 | 4（1） | 5KB |
| | topcoat | 84 | 889 | 5（3） | 9KB |

- **ブラウザのキャッシュが効くと、どちらも 50〜60ms で商品名が出る**。JS や CSS はハッシュ付きの `immutable` なので、2 回目は HTML（と Next.js の RSC）しか取らない。初回の JS 量の差（185KB 対 33KB）は 2 回目以降ほぼ消える。
- **初回の商品名は topcoat の方が早い**（164 対 321ms）。Next.js は HTML 自体は同じくらいに届くが、JS と CSS が多いぶん描画が遅れる、と見ている（内訳は未計測）。
- **関連商品は Next.js が常に早い**。静的シェルに焼いてあるので商品名と同時に出る。topcoat はどの状況でも 800ms の擬似遅延を払う（ブラウザのキャッシュでは解決しない）。
- **クリックでの遷移は、未訪問の商品なら Next.js でも通信が出る**。先読みしているのは URL に依存しないシェルだけで、商品ごとの中身はクリック時に RSC で取る（4 本・5KB、商品名まで 126ms）。topcoat の通常の遷移（84ms）より速いわけではない。「遷移で通信が無い」のは、一度見た商品に戻るときや、Router Cache に入っているページへの遷移。
- 速度差の本体は「遷移の仕組み」より「関連商品をキャッシュするかどうか」。遷移の利点は、通信量より画面の連続性（全体の再読み込みが無い・スクロールや状態が残る）の方が大きい。
- topcoat の関連商品で 1.3 秒台が時々出る（B の 1 回目、C の 3 回目）。インスタンスの切り替わりなどと見ているが、未確認。

### コールドスタートは計測に混ざっていたか（topcoat、11-03〜04）

- **混ざっていた回**: 初回ロード計測の 1 回目に topcoat `/` の FCP 2432ms、別の回で `/products/1` の商品名まで 2178ms。どちらも最初のバイトから遅れており、コールドスタートと判断して表からは外した。手で開いたときにも 3 秒ほどかかったことがある。
- **温まった状態は安定**: curl で 40 回連続（`/products/1`〜`/20`）、TTFB は中央値 116ms・p90 140ms・最大 179ms。TTFB 後に完了するまで（関連商品の 800ms を含む）は 806〜844ms。
- **6 分アクセスを止めても冷えなかった**: CDN のリクエスト記録で無通信（14:58:56〜15:05:04 UTC）を確かめたうえで測ると、1 回目から TTFB 119ms。「アイドル 5 分で停止」は少なくとも 6 分では起きていない。遅かった回は、デプロイ直後の初回か、同時リクエスト（ページ・WebSocket・画像）が新しいインスタンスに振られた場合と推定（未確認）。
- **関連商品だけ 1.3 秒台になる回**: ブラウザ計測で時々出る（商品名は 54〜62ms で出ているので、サーバーは応答中＝コールドスタートではない）。1 本の応答の後半だけが約 500ms 遅れている。curl の連続 40 回では一度も出ず、原因は未特定。
- **ログでは見分けられない**: CDN のリクエスト記録（`vercel logs requests`）にインスタンス ID も処理時間も無く、topcoat は起動時もリクエスト時もログを出さない。見分けるなら、topcoat の応答ヘッダーに起動時の ID と稼働時間を出させる必要がある（計測専用コードが入るので保留）。
- **ログの保持期間（Hobby）**: ランタイムログは約 1 時間（`--since 70m` で拒否、ダッシュボードの 1 時間と一致）。CDN のリクエスト記録は 3 時間前でも取れた。`vercel logs` の既定の検索範囲が 30 分なので、保持期間と取り違えやすい。
- **Next.js でもコールドスタートは起きる**が、`/`・`/products/1`・`/cart` はどれも `x-vercel-cache: HIT`（`age` 720〜1035 秒）で、静的シェルは CDN から返る。関数の起動を待つのはキャッシュしない部分（レビュー・カート件数・在庫 SSE）だけのはず（未計測）。topcoat は HTML を毎回コンテナで作るので、起動の遅れがページ全体に出る。

### ここまでの評価（11-04 時点）

- **ユーザー体験は Next.js が上**。ただし差の内訳は次のとおりで、「遷移の仕組み」より「何をキャッシュしているか」が大きい。
  - 初回の 1 ページ: 商品名が出るまでは topcoat が早い（164 対 321ms）。転送量も topcoat が 1/4〜1/8。
  - 2 回目以降: ブラウザのキャッシュでどちらも 50〜60ms。JS 量の差はほぼ消える。
  - 関連商品: Next.js は常に商品名と同時、topcoat は毎回 +800ms（定義は同じ 800ms。払う回数が違う）。
  - 遷移: 未訪問の商品は Next.js でも RSC を取る（126ms）。topcoat のフルロード（84ms）より速いわけではない。利点は全体の再読み込みが無いこと（スクロール・状態が残る）。
  - コールドスタート: Next.js は静的シェルが CDN から出るので、遅れが動的な部分に閉じる（推定）。topcoat はページ全体が遅れる。
- **記事の主題に照らすと**: Next.js は `<Link>` と `'use cache'` を素直に使うだけで、先読み・静的シェル・CDN キャッシュの構成に乗る。代わりに毎ページ約 150KB の実行環境を読む。topcoat は素直に書くと軽い HTML を毎回サーバーで組み立てる形になり、初回は軽いが、リクエストをまたぐキャッシュも遷移の体験も自分で作り込む必要がある。差の本体は「1 ページ内で静的部分と動的部分を分けられるか」。
- **未計測・未確認**: 遅い回線や低性能端末での初回（JS 量が効くはず）、Next.js のコールドスタート時のレビュー表示、topcoat の関連商品の 1.3 秒台の原因、topcoat に Speculation Rules で先読みを足した場合の遷移。

### 訂正: topcoat にもクライアント遷移と先読みがある（11-04）

- 0.11.0 の `topcoat::runtime::link` で、ページ全体を読み直さない遷移と先読み（`Never` / `Intent` 既定 / `Viewport`）が使える（`topcoat-runtime-0.11.0/src/link.rs`）。この実装は公式ベンチ由来の素の `<a>` のままで、使っていなかった。
- 上の計測・評価にある「topcoat は毎回フルロード」「遷移は Next.js が有利」は**この実装での話**。フレームワーク同士の比較にするには、topcoat 版を `link` に置き換えて測り直す。
- README の Roadmap には Static export、Pre-rendering for static pages、Image optimization / resizing がある。「topcoat に static な性質が無い」は現時点の事実だが、設計として拒んでいるわけではない（`docs/code-comparison.md` の見直し節）。
