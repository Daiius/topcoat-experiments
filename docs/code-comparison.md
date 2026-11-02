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
