# 機能別コード比較（同じ機能を各 FW の流儀で書くと）

2026-11-03 時点の main。行数はコメント・空行を除く概算。根拠は file:line と `docs/journal.md` の見出し。
**推定**と書いたもの以外は読み取りで確認済み。

## 全体

- 規模: topcoat `topcoat/src` 約 2,500 行（テスト数百行を含む）／Next.js `nextjs/src` 約 1,400 行（app 510・components 450・lib 330）
- 依存: topcoat `Cargo.lock` 247 パッケージ（直接は serde, tokio, jemalloc, topcoat）／Next.js `pnpm-lock.yaml` 427 エントリ（直接は next, react, react-dom + dev）。性質が違うので優劣にしない
- テスト: topcoat は `cargo test` 30 本超（本物の router を通す e2e、`tokio::time::pause` による遅延検証、生成 JS の `node --check`、画像表とディスクの突き合わせ）／Next.js 版はテストランナーなし

| 状態 | topcoat | Next.js |
|---|---|---|
| 検索語 | `signal`（+ `raw!` で URL へ replaceState） | URL（`searchParams`）が正 |
| カート | 署名付き cookie + サーバー `cart(cx)` | 署名なし cookie + `cookies()`（読むたびにカタログと照合） |
| 在庫 | WebSocket 上の shard（HTML を push） | React state（SSE で数値を受信） |
| ヘッダー件数 | layout が毎回 cookie から描画 | `cookies()` + `<Suspense>`（全ページ ◐） |

- ルーティング: topcoat はモジュール = ルート（`module_router!()`、`_xxx.rs` はレイアウト、`module_param!`、shard/procedure は `.discover()`）／Next.js は App Router 標準
- エラー: topcoat は型の失敗が 404/400 に直結（`module_param!(…, error = not_found)`、`#[query_params(error = bad_request)]`）／Next.js は `notFound()` が Suspense 内だと 200 + noindex、`?page=abc` は 1 ページ目扱い

## #1 一覧・詳細

- topcoat: `#[page]`, `module_param!(product_id: u32, error = not_found)`（`topcoat/src/app/products/id.rs:20`）, `#[query_params]`（`products.rs:31`）, `view!`
- Next.js: `PageProps<'/products/[id]'>`, `generateStaticParams`, `'use cache'` + `cacheLife('max')`, `params` を `<Suspense>` 内で await（`nextjs/src/app/products/[id]/page.tsx:23-34`）
- **型**: topcoat は URL の型が handler 引数まで届く。Next.js は文字列で、`/^\d+$/` 等を手書き（`[id]/page.tsx:40`）
- **素直さ**: topcoat は素直。Next.js は「params を境界内で await」「ページを async にしない」作法が要り、その結果 404 が 200 に

```rust
#[query_params(error = bad_request)]
struct ProductsQuery { page: Option<usize>, sort: Option<String>,
    category: Option<String>, q: Option<String> }
```
```tsx
export function generateStaticParams() { return getProductIds().map((id) => ({ id: String(id) })); }
export default function ProductDetailPage({ params }: PageProps<"/products/[id]">) {
  return (<Suspense fallback={<DetailSkeleton />}><ProductDetail params={params} /></Suspense>);
}
```

## #2 遅いデータ

- 書き味は**同型**（async コンポーネントを suspense で包む）。topcoat は `fallback:` を `view!` で包む・`#[component]` が同名 unit struct を作る癖
- **流儀の効き方は Next.js 有利**: 関連商品は `'use cache'` 2 行で 800ms が消える（代償は `next build` が 500×800ms を吸収）。topcoat はリクエスト跨ぎキャッシュ機構なし

```rust
suspense(
    fallback: view! { review_list_skeleton(count: product.review_count.min(3)) },
    reviews(product_id: product.id)
)
```
```ts
export async function fetchRelated(productId: number): Promise<Product[]> {
  "use cache";
  cacheLife("max");
```

## #3 検索

- topcoat: `signal`, `$()`, `@input`, `:value`, `#[shard]`, `raw!`, `history.replaceState`（`products.rs:53, 74-77`）
- Next.js: `next/form` の `<Form action="/products">`（`search-form.tsx:21`）+ `searchParams`。**自前 JS ゼロ**
- **型**: topcoat の `$()` 内 Rust はサーバー評価時しか検査されず、`raw!` の JS は無検査 → ブラウザで初めて `Failed to compile event @input`
- **素直さ**: 対照的。topcoat は URL 同期が流儀に無く手書き＋`topcoat fmt` が `raw!` を壊す。Next.js は丸ごと不要。
  ただし UX が違う（topcoat は 1 文字ごと部分更新、Next.js は Enter 確定。逐次にするなら Client Component + `router.replace` + debounce）

## #4 カート

- topcoat: `#[route(POST)]`, `Form<T>`, `see_other`, `href!`, 署名付き cookie（`cart/add.rs:22-33`）→ POST/303/GET で文書全体再読込（#541）、JS なしでも PRG
- Next.js: `'use server'`, `<form action={fn}>`, `cookies()`, `redirect`（`cart/actions.ts:17-25`）→ cookie 更新だけで現在ページが差分再描画。JS なし数量変更は PRG にならない
- 署名: topcoat は標準（代わりに全インスタンスで鍵共有が必要）、Next.js は無し

```rust
#[route(POST)]
pub async fn add(cx: &Cx, Form(input): Form<AddForm>) -> Result<SeeOther> {
    let product = catalog.get(input.product_id).ok_or_not_found()?;
    cart(cx).update(|cart| { cart.sanitize(catalog); cart.add(product.id, quantity); }).commit()?;
    Ok(see_other(href!(super::cart_page).resolve(cx)))
}
```
```ts
export async function addToCart(formData: FormData): Promise<void> {
  const productId = Number(formData.get("product_id"));
  if (isProductId(productId)) { await writeCart(addLine(await readCart(), productId, quantity)); }
  redirect("/cart");
}
```

## #5 在庫 live

- topcoat: `#[shard]`, `live!`, `emit!`, `connected(cx)` → **サーバーが描画した HTML 片を WebSocket で push**。描画ロジックはサーバーの Rust 1 箇所（journal「一発で書けた」）
- Next.js: Route Handler + `ReadableStream`（SSE）+ `'use client'` + `useState`/`useEffect` + `EventSource` → **数値を送ってクライアントで描画**。
  文字列を `Number()` でパースする手動境界、UI 分岐の再実装、BigInt/tsconfig、`no-transform`、lint 回避、`maxDuration` の工夫が要った

```rust
#[shard("/_shards/stock-level")]
async fn stock_level(cx: &Cx, product_id: u32) -> Result<impl View> {
    Ok(live! { loop { let token = emit! { /* <p>…</p> */ }?;
        if !known || !connected(cx) { break Ok(token); }
        tokio::time::sleep(until_next_change(now)).await; } })
}
```
```tsx
const [level, setLevel] = useState(initialLevel);
useEffect(() => {
  const source = new EventSource(`/api/stock/${productId}`);
  source.onmessage = (event: MessageEvent<string>) => setLevel(Number(event.data));
  return () => source.close();
}, [productId]);
```

## #6 画像

- Next.js: `<Image sizes=…>` 1 属性（+ `localPatterns`/`qualities` 設定、`preload` と `loading` の排他）
- topcoat: 変換スクリプト・`macro_rules!` の表・`srcset` 組み立て・テストの 4 点を自作（本番 +125 行、スクリプト 55 行、テスト 120 行、約 35 分）

```rust
let srcset = image.widths()
    .map(|(asset, width)| format!("{} {width}w", config.resolve(asset)))
    .collect::<Vec<_>>().join(", ");
```

## フロントエンドエンジニアから見た所感（候補）

1. **データ取得と表示のコンポーネント境界は同型**。async コンポーネント + Suspense の知識がそのまま使える
2. **「どこがブラウザで動くか」の境界の引き方が根本的に違う**。topcoat は式（`$()`/`@input`）単位、Next.js はファイル（`'use client'`）単位
3. **型安全は topcoat がサーバー側で強いが、ブラウザに落ちる所で切れる**（`raw!`・`$()` の JS は無検査）。Next.js は tsc 一発で境界も通る
4. **状態を URL に置くなら Next.js は URL 同期が不要**。topcoat は signal を持ち込むと同期を手書き
5. **フォームは Next.js が滑らか（差分更新）、topcoat は PE（JS なし PRG）が自然**
6. **周辺機能（画像など）の厚みは Next.js が圧倒的**。ただし実行時変換の CPU/課金/キャッシュ運用が付いてくる
7. **Next.js はキャッシュモデルの学習コストが重い**（`'use cache'`/`cacheLife`/`connection()`/境界内 await/404 が 200）。
   一方 topcoat は**どこを静的にするかを考えなくてよい**（全部動的で、それでも速い）
8. **テストとコンパイル時検査は topcoat が強い**（router を通す e2e が `cargo test` に載る）。ただし rust-analyzer のマクロ対応（#486）、`topcoat fmt` の不具合あり

## 対比の表現（2026-11-04 案。実装と Vercel 本番の計測を踏まえて）

**軸**: 何を「最初に書く形」として用意しているか。Next.js は素直に書くと**速い構成**（静的シェル・キャッシュ・先読み）になり、代わりにキャッシュの仕組みを学ぶ。topcoat は素直に書くと**単純な構成**（毎回サーバーで描画し、送るのはほぼ HTML）になり、単純なまま十分速いが、キャッシュや遷移の滑らかさは自分で作る。

| 観点 | topcoat が自然に書かせるもの | Next.js が自然に書かせるもの | フロントエンドから見た帰結 |
|---|---|---|---|
| 静的と動的の分け方 | 全部動的。どこを静的にするか考えなくてよい | `'use cache'` 2 行で遅いデータを静的シェルに焼く | **Next.js 有利**。関連商品の 800ms が消え、コールドスタートの影響も動的な部分に閉じる（推定）。代償は学習（境界内 await、404 が 200、鮮度管理） |
| ブラウザに送る JS | 15KB を 1 本、HTML だけで描画が完結 | 実行環境で毎ページ約 150KB | **topcoat 有利（初回のみ）**。商品名表示 164 対 321ms。ブラウザのキャッシュが効けば差はほぼ消える |
| どこをブラウザで動かすか | 式の単位（`$()`、`@input`） | ファイルの単位（`'use client'`） | 好みが分かれる。topcoat は細かいが境界が見えにくい |
| live 更新 | サーバーが描いた HTML 片を WebSocket で送る。描画ロジックは 1 か所 | 値を SSE で送りクライアントで描き直す。表示の分岐を両側に書く | **topcoat 有利**（一発で書けた／Next.js はパース・`useEffect`・`maxDuration` で手間） |
| 遷移 | 普通のリンク、毎回フルロード | 先読み + Router Cache + 差分取得 | **Next.js 有利**。ただし速さより**画面の連続性**。未訪問の商品への遷移は 126 対 84ms |
| フォーム | PRG が自然、JS なしで動く | Server Action で差分再描画 | 体験は Next.js、PE は topcoat（#541 で毎回フルロード） |
| 状態と URL | `signal` を使うと URL 同期を `raw!` で手書き | URL がそのまま状態（自前 JS ゼロ） | **Next.js 有利** |
| 型と検査 | URL の型がハンドラまで届き、不一致は 404/400。router を通す e2e が `cargo test` に載る | `params` は文字列。tsc はブラウザ側との境界まで通る | サーバー側は topcoat が強いが、**ブラウザで動く部分（`$()`・`raw!`）は無検査**で、ブラウザで初めてエラーになった |
| 画像 | 変換・`srcset`・テストを自作（約 35 分） | `<Image sizes>` 1 属性 | **Next.js 有利**（最大の差）。LCP は画像最適化でほぼ決まる |
| デプロイ・運用 | コンテナ。Vercel ではビルドキャッシュなしで 10 分、コールドスタートでページ全体が遅れる、署名鍵の共有 | ネイティブ対応、ビルドキャッシュ、CDN | **Next.js 有利**。FW というよりプラットフォームのネイティブ対応の差 |

文章案:

> どちらも「async コンポーネントを Suspense で包み、遅いデータをストリームで流す」書き方は同じで、React の知識がそのまま通じる。違いはその先の最適化を誰が持つか。Next.js は 1 ページを静的な部分と動的な部分に分ける API を持ち、`'use cache'` や `<Link>` を使うだけで CDN キャッシュ・先読み・差分の遷移に乗れる。代わりに毎ページ約 150KB の実行環境を送り、キャッシュの仕組みを学ぶ。topcoat は何でもサーバーで描画して HTML として送る（live 更新すら HTML 片）。ブラウザ側のコードがほぼ要らず、描画ロジックが 1 か所にまとまり、初回は軽く単純。一方でリクエストをまたぐキャッシュ、遷移の連続性、画像最適化は用意されておらず、ブラウザで動く部分には Rust の型の保証が届かない。

公平のための注記: topcoat は 0.11 で、キャッシュ・先読み・画像は「未整備」であって設計思想の欠点とは限らない（Speculation Rules を足せば遷移は近づけられそう、未検証）。Next.js の強みの一部は Vercel のネイティブ対応に由来する。

## 描画とパフォーマンスを制御する仕組みの比較（2026-11-04）

**一言で**: Next.js は描画の**タイミング**（ビルド時・キャッシュ・リクエスト時・ブラウザ）を制御する仕組みが厚い。topcoat は描画の**場所**（サーバーか、ブラウザの式か）を細かく選べるが、タイミングは常に「リクエスト時」。

| 仕組み | topcoat 0.11 | Next.js 16 | 同じ／違う |
|---|---|---|---|
| 描画の基本 | サーバーで HTML。ハイドレーションなし | RSC をサーバーで描画 + Client Component をハイドレーション | **同じ**: サーバーが先。**違う**: Next.js はブラウザに React のツリーを持つ |
| 遅いデータ | `suspense(fallback:, …)` でストリーム | `<Suspense>` でストリーム | **同じ**（書き方もほぼ同型） |
| ストリームの「先に出る部分」 | 毎リクエスト計算する | ビルド時に作って CDN に置く（静的シェル） | **違う**。形は同じ「シェル + 後から穴埋め」だが、Next.js はシェルを事前計算する |
| ページのキャッシュ | 無し。`response_headers(cx)` で `Cache-Control` は付けられるので、CDN に載せるなら手書き | ビルド時の事前生成、ISR、`'use cache'` + `cacheLife` | **違う**（topcoat に静的・キャッシュの概念が無い） |
| データのキャッシュ | 無し。アプリのコードでメモリに持つなら手書き（stateless・複数インスタンスでは温まったインスタンスだけ） | `'use cache'`（関数・コンポーネント単位）+ `cacheLife` + タグで無効化 | **違う** |
| 静的か動的かの判定 | 概念なし（全部動的） | API の使用から推論（`cookies()` や `searchParams` を使うと動的。Suspense の外で使うとビルドエラー） | **違う**。Next.js はビルドが設計を矯正する |
| 部分更新 | `#[shard]` をサーバーで描き直し、HTML 片を差し替え | Server Action / `router.refresh` で RSC を取り直し、React が差分を当てる | **同じ**: サーバーで描き直す。**違う**: 送るのが HTML 片か RSC か |
| サーバーからの push | `live!` + `emit!`（WebSocket で HTML 片） | 仕組みなし（Route Handler で SSE を自作し、クライアントで描画） | **違う**（topcoat に専用の仕組みがある） |
| ブラウザの状態 | `signal` + `$()` 式（細かい単位の反応性、必要な所だけ JS） | `useState` など React 全般 | **違う**。topcoat は JS が使った分だけ、Next.js は実行環境を常に送る |
| 遷移・先読み | 無し（MPA、毎回フルロード） | `<Link>` 先読み + Router Cache | **違う** |
| 静的資源 | ハッシュ付きで `immutable`（`topcoat-asset`、フォントも同様） | ハッシュ付きで `immutable` | **同じ** |
| 画像最適化 | 無し（自作した） | `next/image`（実行時変換） | **違う** |

- **「topcoat に static な性質が無い」はそのとおり**。ページ・データ・ブラウザの 3 層のどこにも、リクエストをまたいで結果を再利用する仕組みが無い（`Cache-Control` を自動で付けるのはハッシュ付きの静的資源とフォントだけ、`topcoat-asset/src/serve.rs`）。
- 自分で書くなら 2 通り。(1) `response_headers(cx)` で `Cache-Control: s-maxage=…` を付けて CDN に任せる。ただしこのストアはレイアウトのヘッダーにカート件数（cookie）を描くので、ページ全体がユーザーごとに変わり CDN に載せられない。件数をブラウザ側（`signal` + cookie を読む JS）に移せば載る（Next.js で試した「動的な穴を外す」実験の逆）。(2) アプリ内のメモリキャッシュ。インスタンスごとなので効きは限られる。
- 逆に topcoat の方が厚いのは**サーバーからの push**（`live!`）と**ブラウザ側 JS の量を式単位で絞れること**。Next.js はここを自作するか、実行環境ごと送る。
- まとめると、Next.js は「いつ描くか」を API で宣言させ、ビルドがそれを検査する。topcoat は「どこで描くか」を細かく選ばせるが、「いつ」は常にリクエスト時で、再利用は利用者の責任。

### 「いつ描くか／どこで描くか」への反論と妥当性（2026-11-04）

| 反論 | 根拠 | 妥当性 |
|---|---|---|
| Next.js も「どこで描くか」を制御している | Server Component と `'use client'` は、まさにサーバーとブラウザへの振り分け | **高い**。Next.js にも「どこ」の軸はある。違いは粒度（Next.js はモジュール単位で、`'use client'` は import 先へ伝播する／topcoat は属性の式単位）と、境界を越える物（Next.js はシリアライズした props／topcoat は JS に下ろした式と runtime のサロゲート値。journal「`@input` がブラウザで初めてコンパイルエラー」） |
| topcoat も「いつ描くか」を制御している | `suspense`（何を後回しにするか）、`live!` + `emit!`（時間をおいて描き直して push）、`#[shard]`（必要な時にだけ描き直す）、`signal`（ブラウザで後から） | **中〜高**。ただしどれも **1 つのリクエスト・接続の中の時間**。Next.js の厚みは**リクエストをまたぐ時間**（ビルド時、キャッシュ期間、再検証） |
| topcoat もビルド時に決めている物がある | カタログは `include_str!` でバイナリに埋め込み（`topcoat/src/catalog.rs:88`）、`asset!` と Tailwind はビルド時に束ねる | **中**。データと資源はビルド時だが、**HTML の描画は常にリクエスト時**。「描画結果の再利用」が無いという主張は崩れない |
| Next.js の「いつ」は Vercel の機能では | ISR や CDN キャッシュはデプロイ先に依存する | **中**。ビルド時の事前生成（静的シェル）と `'use cache'` はフレームワークの機能でどこでも動く。CDN に置く・複数インスタンスで共有するのはプラットフォーム（自前ホストではキャッシュハンドラが要る） |
| 描画が安ければ「いつ」は要らない | ローカル docker で topcoat の素の SSR は RPS 25〜60 倍、メモリ 1/10（article-notes §7） | **高い（CPU に関しては）**。描画コストのためのキャッシュは topcoat ではほぼ不要。ただし**遅い外部データ**（関連商品の 800ms）には効かず、そこがキャッシュの本来の役割 |
| 0.11 時点の未整備では | キャッシュ・先読み・画像は今後入るかもしれない | **中**。記事では「設計思想」と「現時点の機能」を分けて書く |

**結論（言い換え）**: 「Next.js は『いつ』、topcoat は『どこ』」は**重心の違い**としては妥当だが、排他的な対比ではない。より正確には:

> Next.js は「どこ」と「いつ」の両方を API で宣言させ、特に**リクエストをまたぐ「いつ」**（事前生成・キャッシュ・再検証）が厚い。topcoat は「どこ」を**式の単位**で細かく選ばせ、「いつ」は **1 つのリクエスト・接続の中**（ストリームの順序、`live!` の push）に限られる。描画が十分安いので、リクエストをまたぐ再利用を前提にしていない。

### topcoat 公式の目指す姿と照らした見直し（2026-11-04、topcoat 0.11.0 の README・ソースで確認）

公式の自己紹介（crate の `README.md`）: "modular, batteries-included … prioritizes simplicity and productivity"。売りは (1) 全マークアップをサーバー描画しつつ、`$(...)` を「型検査された Rust」としてサーバーで評価し JS にも変換してブラウザで即時に再実行、(2) `#[shard]` でサーバー再描画した HTML を差し替え、(3) モジュールでのルーティング、(4) 編集できる UI 部品、(5) 強いブラウザキャッシュ付きの資源バンドル、(6) Node 不要の Tailwind。

**見落としていた既存機能**

- **クライアント遷移と先読み（`topcoat::runtime::link`）が 0.11.0 にある**。ページ全体を読み直さずに遷移し、先読みは `Never` / `Intent`（ホバーかフォーカスで先読み、既定）/ `Viewport`（画面に入ったら）から選ぶ（`topcoat-runtime-0.11.0/src/link.rs`、`browser/src/render/navigation.ts`, `prefetch.ts`）。JS が無ければ普通のリンクとして動く。
  → **この実装は公式ベンチ由来の素の `<a>` のままで、使っていなかった**。journal と上の表の「topcoat は毎回フルロード」「遷移は Next.js 有利」は**この実装の話**で、フレームワークの比較としては未検証。
- `#[memoize]`（1 リクエスト内のキャッシュと重複呼び出しの集約）と App context（リクエストをまたいで値を共有する置き場）がある。リクエストをまたぐキャッシュは、App context の上に自分で書く形になる。

**ロードマップ（README の Roadmap）にあるもの**: Static export、Pre-rendering for static pages、Image optimization / resizing、(More) reactivity、Docs for how to deploy。
→ 「いつ描くか」（ビルド時の事前描画）と画像最適化は**作る予定の物**で、設計として拒んでいるわけではない。前節の「描画が十分安いので、リクエストをまたぐ再利用を前提にしていない」は**撤回**（推測で書いていた）。

**結論の修正**

- 「どこで描くか」を式の単位で選べることは、公式の看板（`$()` の二重評価）そのもので、結論と合っている。ただし公式は「型検査された Rust」と言っており、穴は `raw!`（JS を文字列で埋める逃げ道）にある、と書き分ける。
- 「いつ描くか」の差の多くは**成熟度の差**（事前描画・静的書き出し・画像はロードマップ上）。設計の差として残りそうなのは、**1 ページの中で静的な部分と動的な部分を分けてキャッシュする仕組み**（Next.js の静的シェル + `'use cache'` + `cacheLife` / タグ）。ロードマップの "Pre-rendering for static pages" はページ単位に読め、ページ内の分割やデータ単位のキャッシュは挙がっていない（推定、今後変わりうる）。
- 遷移の比較は、topcoat 版を `link` に置き換えてから測り直す必要がある。
