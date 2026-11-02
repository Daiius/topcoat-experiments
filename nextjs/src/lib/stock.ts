// Simulated live stock levels, ported from `topcoat/src/stock.rs`. A level is
// a pure function of the product ID and the current time slot, so every
// instance (and the topcoat version) reports the same number at the same
// moment without sharing state, given synchronized clocks.
import { connection } from "next/server";

/** How often the level changes, in milliseconds. Clients wake on the same boundaries. */
export const STOCK_INTERVAL_MS = 5000;

const MASK = (1n << 64n) - 1n;

/** `SplitMix64` finalizer on wrapping 64-bit integers, as in the Rust version. */
function mix(input: bigint): bigint {
  let x = (input + 0x9e3779b97f4a7c15n) & MASK;
  x = ((x ^ (x >> 30n)) * 0xbf58476d1ce4e5b9n) & MASK;
  x = ((x ^ (x >> 27n)) * 0x94d049bb133111ebn) & MASK;
  return x ^ (x >> 31n);
}

/** The stock level of `productId` during the slot containing `atMs` (Unix ms). */
export function stockAt(productId: number, atMs: number): number {
  const id = BigInt(productId);
  const slot = BigInt(Math.floor(Math.floor(atMs / 1000) / (STOCK_INTERVAL_MS / 1000)));
  // Each product has its own baseline (5..=44) and drifts around it by up
  // to ±6 from one slot to the next. Some slots dip to zero.
  const baseline = (mix(id) % 40n) + 5n;
  const drift = mix(((id << 32n) & MASK) ^ slot) % 13n;
  const level = baseline + drift - 6n;
  return Number(level < 0n ? 0n : level);
}

/**
 * The stock level of `productId` right now. Reads the clock, so it waits for
 * a request first: during a prerender it never resolves and stays a hole.
 */
export async function currentStock(productId: number): Promise<number> {
  await connection();
  return stockAt(productId, Date.now());
}

/** Milliseconds left until the next slot boundary after `atMs`. */
export function untilNextChange(atMs: number): number {
  return STOCK_INTERVAL_MS - (atMs % STOCK_INTERVAL_MS);
}
