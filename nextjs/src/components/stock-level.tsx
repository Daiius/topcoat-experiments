"use client";

import { useEffect, useState } from "react";

/** At or below this level the badge warns that stock is low. */
const LOW_STOCK = 5;

/**
 * The live stock level. Starts from the level the server rendered, then
 * follows the SSE stream from `/api/stock/[id]`. The browser's EventSource
 * reconnects on its own; the server resends the current level on connect.
 */
export function StockLevel({
  productId,
  initialLevel,
}: {
  productId: number;
  initialLevel: number;
}) {
  const [level, setLevel] = useState(initialLevel);

  useEffect(() => {
    // Also closed when the route is hidden by a navigation (Activity).
    const source = new EventSource(`/api/stock/${productId}`);
    source.onmessage = (event: MessageEvent<string>) => setLevel(Number(event.data));
    return () => source.close();
  }, [productId]);

  return <StockBadge level={level} />;
}

function StockBadge({ level }: { level: number }) {
  if (level === 0) {
    return <p className="mt-3 text-sm font-semibold text-rose-600">Out of stock</p>;
  }
  if (level <= LOW_STOCK) {
    return (
      <p className="mt-3 text-sm font-semibold text-amber-600">{`Only ${level} left in stock`}</p>
    );
  }
  return <p className="mt-3 text-sm font-medium text-emerald-700">{`${level} in stock`}</p>;
}
