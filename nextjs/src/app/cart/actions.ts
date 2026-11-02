"use server";

import { redirect } from "next/navigation";

import { MAX_QUANTITY, addLine, readCart, setLine, writeCart } from "@/lib/cart";
import { isProductId } from "@/lib/catalog";

/** Reads a whole number in `0..=MAX_QUANTITY` from form data, or `null`. */
function quantityField(formData: FormData, name: string): number | null {
  const value = Number(formData.get(name));
  return Number.isInteger(value) && value >= 0 ? Math.min(value, MAX_QUANTITY) : null;
}

/**
 * Adds a product to the cart, then goes to the cart page. The form data is
 * user input: an unknown product leaves the cart as it is.
 */
export async function addToCart(formData: FormData): Promise<void> {
  const productId = Number(formData.get("product_id"));
  const quantity = Math.max(quantityField(formData, "quantity") ?? 1, 1);
  if (isProductId(productId)) {
    await writeCart(addLine(await readCart(), productId, quantity));
  }
  redirect("/cart");
}

/**
 * Sets a cart line's quantity; zero removes it. No redirect: setting a
 * cookie in a Server Action makes Next.js re-render the current page.
 */
export async function updateCart(formData: FormData): Promise<void> {
  const productId = Number(formData.get("product_id"));
  const quantity = quantityField(formData, "quantity");
  if (isProductId(productId) && quantity !== null) {
    await writeCart(setLine(await readCart(), productId, quantity));
  }
}
