import Link from "next/link";
import { Suspense } from "react";

import { MAX_QUANTITY, cartItems, readCart, totalCents } from "@/lib/cart";
import { formatPrice, initials } from "@/lib/catalog";

import { updateCart } from "./actions";

// The heading is static; everything that depends on the cookie streams in.
export default function CartPage() {
  return (
    <>
      <h1 className="text-3xl font-bold tracking-tight">Your cart</h1>
      <Suspense
        fallback={
          <div
            className="mt-8 h-24 animate-pulse rounded-xl border border-slate-200 bg-white"
            aria-busy="true"
            aria-label="Loading cart"
          />
        }
      >
        <CartContents />
      </Suspense>
    </>
  );
}

async function CartContents() {
  const items = cartItems(await readCart());

  if (items.length === 0) {
    return (
      <div className="mt-8 rounded-xl border border-slate-200 bg-white p-8 text-center">
        <p className="text-slate-500">Your cart is empty.</p>
        <Link
          href="/products"
          className="mt-4 inline-block text-sm font-medium text-indigo-600 hover:text-indigo-500"
        >
          Browse products
        </Link>
      </div>
    );
  }

  const count = items.reduce((sum, [, quantity]) => sum + quantity, 0);

  return (
    <>
      <ul className="mt-8 divide-y divide-slate-100 rounded-xl border border-slate-200 bg-white">
        {items.map(([product, quantity]) => (
          <li key={product.id} className="flex flex-wrap items-center gap-4 p-4">
            <div className="w-20 shrink-0">
              <div className="flex h-20 items-center justify-center rounded-lg bg-slate-100 text-3xl font-bold text-slate-300">
                {initials(product.name)}
              </div>
            </div>
            <div className="min-w-0 flex-1">
              <Link
                href={`/products/${product.id}`}
                className="font-semibold text-slate-900 hover:text-indigo-600"
              >
                {product.name}
              </Link>
              <p className="text-sm text-slate-500">{`${formatPrice(product.price_cents)} each`}</p>
            </div>
            {/* Server Action forms: they post without JavaScript too. */}
            <form action={updateCart} className="flex items-center gap-2">
              <input type="hidden" name="product_id" value={product.id} />
              <label className="sr-only" htmlFor={`quantity-${product.id}`}>
                Quantity
              </label>
              <input
                // A changed quantity from the server replaces the field.
                key={quantity}
                id={`quantity-${product.id}`}
                type="number"
                name="quantity"
                min={0}
                max={MAX_QUANTITY}
                defaultValue={quantity}
                className="w-20 rounded-lg border border-slate-300 px-2 py-1 text-sm"
              />
              <button
                type="submit"
                className="rounded-lg border border-slate-300 px-3 py-1 text-sm font-medium hover:bg-slate-50"
              >
                Update
              </button>
            </form>
            <form action={updateCart}>
              <input type="hidden" name="product_id" value={product.id} />
              <input type="hidden" name="quantity" value="0" />
              <button
                type="submit"
                className="text-sm font-medium text-rose-600 hover:text-rose-500"
              >
                Remove
              </button>
            </form>
          </li>
        ))}
      </ul>
      <div className="mt-6 flex items-baseline justify-end gap-4">
        <p className="text-sm text-slate-500">{`${count} items`}</p>
        <p className="text-2xl font-bold">{formatPrice(totalCents(items))}</p>
      </div>
    </>
  );
}
