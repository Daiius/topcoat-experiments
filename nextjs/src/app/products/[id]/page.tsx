import { notFound } from "next/navigation";
import { Suspense } from "react";

import { Breadcrumbs } from "@/components/breadcrumbs";
import { ProductCard } from "@/components/product-card";
import { RatingStars } from "@/components/rating-stars";
import { ReviewList } from "@/components/review-list";
import { SpecTable } from "@/components/spec-table";
import { ReviewListSkeleton } from "@/components/skeletons";
import { addToCart } from "@/app/cart/actions";
import { MAX_QUANTITY } from "@/lib/cart";
import { formatPrice, formatRating, getProduct, getProductIds, initials } from "@/lib/catalog";
import { fetchRelated, fetchReviews } from "@/lib/remote";

// All 500 products are prerendered at build time. Any other ID gets the
// App Shell (the fallback below) and resolves at request time.
export function generateStaticParams() {
  return getProductIds().map((id) => ({ id: String(id) }));
}

// Not async: `params` is awaited inside the boundary, as the docs advise,
// so the route also has a URL-independent App Shell for prefetching.
export default function ProductDetailPage({ params }: PageProps<"/products/[id]">) {
  return (
    <Suspense fallback={<DetailSkeleton />}>
      <ProductDetail params={params} />
    </Suspense>
  );
}

async function ProductDetail({ params }: Pick<PageProps<"/products/[id]">, "params">) {
  const { id } = await params;
  const product = /^\d+$/.test(id) ? await getProduct(Number(id)) : null;
  if (!product) {
    notFound();
  }
  return (
    <>
      <Breadcrumbs
        category={product.category}
        categorySlug={product.category_slug}
        name={product.name}
      />
      <div className="mt-6 grid gap-10 lg:grid-cols-2">
        <div className="flex aspect-square items-center justify-center rounded-2xl bg-slate-100 text-7xl font-bold text-slate-300">
          {initials(product.name)}
        </div>
        <div>
          <p className="text-sm font-medium uppercase tracking-wide text-slate-400">
            {product.category}
          </p>
          <h1 className="mt-1 text-3xl font-bold tracking-tight">{product.name}</h1>
          <div className="mt-3 flex items-center gap-2">
            <RatingStars tenths={product.rating_tenths} size="h-5 w-5" />
            <span className="text-sm text-slate-500">
              {`${formatRating(product.rating_tenths)} (${product.review_count} reviews)`}
            </span>
          </div>
          <p className="mt-6 text-4xl font-bold">{formatPrice(product.price_cents)}</p>
          <div className="mt-6 space-y-4 text-slate-600">
            {product.description.map((paragraph, index) => (
              <p key={index}>{paragraph}</p>
            ))}
          </div>
          {/* A Server Action form inside the static HTML: it posts without
              JavaScript too. The action updates the cart cookie and
              redirects to the cart page. */}
          <form action={addToCart} className="mt-8 flex items-center gap-3">
            <input type="hidden" name="product_id" value={product.id} />
            <label className="sr-only" htmlFor="quantity">
              Quantity
            </label>
            <input
              id="quantity"
              type="number"
              name="quantity"
              min={1}
              max={MAX_QUANTITY}
              defaultValue={1}
              className="w-20 rounded-lg border border-slate-300 px-3 py-3 text-sm"
            />
            <button
              type="submit"
              className="inline-block rounded-lg bg-indigo-600 px-8 py-3 text-sm font-semibold text-white hover:bg-indigo-500"
            >
              Add to cart
            </button>
          </form>
        </div>
      </div>
      <section className="mt-16">
        <h2 className="text-2xl font-bold tracking-tight">Specifications</h2>
        <SpecTable specs={product.specs} />
      </section>
      <section className="mt-16">
        <h2 className="text-2xl font-bold tracking-tight">{`Reviews (${product.review_count})`}</h2>
        {/* Fresh data: the skeleton ships in the static HTML, the reviews
            stream in at request time. */}
        <Suspense fallback={<ReviewListSkeleton count={Math.min(product.review_count, 3)} />}>
          <Reviews productId={product.id} />
        </Suspense>
      </section>
      <section className="mt-16">
        <h2 className="text-2xl font-bold tracking-tight">Related products</h2>
        {/* Cached data, no boundary: resolved at build time and part of the
            static HTML, so the 800ms never reaches a visitor. */}
        <Related productId={product.id} />
      </section>
    </>
  );
}

/** Reviews come from a slow, uncached service, so they stream in. */
async function Reviews({ productId }: { productId: number }) {
  const reviews = await fetchReviews(productId);
  return <ReviewList reviews={reviews} />;
}

/** Recommendations come from an even slower service, but are cached. */
async function Related({ productId }: { productId: number }) {
  const related = await fetchRelated(productId);
  return (
    <div className="mt-6 grid grid-cols-2 gap-6 md:grid-cols-4">
      {related.map((entry) => (
        <ProductCard key={entry.id} product={entry} />
      ))}
    </div>
  );
}

/** The App Shell of the detail route: shown only for IDs not prerendered. */
function DetailSkeleton() {
  return (
    <div className="animate-pulse" aria-busy="true" aria-label="Loading product">
      <div className="h-4 w-1/3 rounded bg-slate-200"></div>
      <div className="mt-6 grid gap-10 lg:grid-cols-2">
        <div className="aspect-square rounded-2xl bg-slate-100"></div>
        <div>
          <div className="h-3 w-1/4 rounded bg-slate-100"></div>
          <div className="mt-3 h-8 w-3/4 rounded bg-slate-200"></div>
          <div className="mt-6 h-10 w-1/3 rounded bg-slate-200"></div>
        </div>
      </div>
    </div>
  );
}
