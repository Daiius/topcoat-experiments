/** Placeholder for `ProductCard` while its data loads. */
export function ProductCardSkeleton() {
  return (
    <div
      className="flex animate-pulse flex-col rounded-xl border border-slate-200 bg-white p-4 shadow-sm"
      aria-hidden="true"
    >
      <div className="mb-4 h-32 rounded-lg bg-slate-100"></div>
      <div className="h-3 w-1/3 rounded bg-slate-100"></div>
      <div className="mt-2 h-4 w-3/4 rounded bg-slate-200"></div>
      <div className="mt-3 h-4 w-1/2 rounded bg-slate-100"></div>
      <div className="mt-3 h-5 w-1/4 rounded bg-slate-200"></div>
    </div>
  );
}

/**
 * Placeholder for `ReviewList` while reviews load. Matches the card layout so
 * the page does not jump when the content replaces it.
 */
export function ReviewListSkeleton({ count }: { count: number }) {
  return (
    <div className="mt-6 space-y-6" aria-busy="true" aria-label="Loading reviews">
      {Array.from({ length: count }, (_, index) => (
        <div key={index} className="animate-pulse rounded-xl border border-slate-200 bg-white p-6">
          <div className="h-4 w-1/4 rounded bg-slate-200"></div>
          <div className="mt-3 h-4 w-1/2 rounded bg-slate-200"></div>
          <div className="mt-4 h-3 w-full rounded bg-slate-100"></div>
          <div className="mt-2 h-3 w-5/6 rounded bg-slate-100"></div>
        </div>
      ))}
    </div>
  );
}

/** A grid of card placeholders, laid out like the product grids. */
export function ProductGridSkeleton({
  count,
  className,
}: {
  count: number;
  className: string;
}) {
  return (
    <div className={className}>
      {Array.from({ length: count }, (_, index) => (
        <ProductCardSkeleton key={index} />
      ))}
    </div>
  );
}
