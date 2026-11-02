import { notFound } from "next/navigation";
import { Suspense } from "react";

import { Breadcrumbs } from "@/components/breadcrumbs";
import { ProductCard } from "@/components/product-card";
import { RatingStars } from "@/components/rating-stars";
import { ReviewList } from "@/components/review-list";
import { SpecTable } from "@/components/spec-table";
import {
  formatPrice,
  formatRating,
  getProduct,
  getProductIds,
  initials,
  lookupProducts,
} from "@/lib/catalog";

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
  const related = lookupProducts(product.related_ids);

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
        </div>
      </div>
      <section className="mt-16">
        <h2 className="text-2xl font-bold tracking-tight">Specifications</h2>
        <SpecTable specs={product.specs} />
      </section>
      <section className="mt-16">
        <h2 className="text-2xl font-bold tracking-tight">{`Reviews (${product.review_count})`}</h2>
        <ReviewList reviews={product.reviews} />
      </section>
      <section className="mt-16">
        <h2 className="text-2xl font-bold tracking-tight">Related products</h2>
        <div className="mt-6 grid grid-cols-2 gap-6 md:grid-cols-4">
          {related.map((entry) => (
            <ProductCard key={entry.id} product={entry} />
          ))}
        </div>
      </section>
    </>
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
