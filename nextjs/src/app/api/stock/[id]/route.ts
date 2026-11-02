import { isProductId } from "@/lib/catalog";
import { stockAt, untilNextChange } from "@/lib/stock";

// An SSE stream may hold the function open for this long (seconds). Vercel
// caps it by plan (300 on Hobby); the stream ends a little earlier on its own
// and the browser's EventSource reconnects.
export const maxDuration = 300;

const STREAM_FOR_MS = (maxDuration - 10) * 1000;

/**
 * Live stock level of one product as Server-Sent Events: the current level
 * right away, then a new one at every slot boundary. The level is computed
 * from the clock, so every instance sends the same values and a reconnect
 * simply resumes. Nothing is shared between connections.
 */
export async function GET(request: Request, { params }: RouteContext<"/api/stock/[id]">) {
  const { id } = await params;
  const productId = /^\d+$/.test(id) ? Number(id) : NaN;
  if (!isProductId(productId)) {
    return new Response("Unknown product", { status: 404 });
  }

  const encoder = new TextEncoder();
  let tick: ReturnType<typeof setTimeout> | undefined;
  let end: ReturnType<typeof setTimeout> | undefined;
  const stop = () => {
    clearTimeout(tick);
    clearTimeout(end);
  };

  const stream = new ReadableStream<Uint8Array>({
    start(controller) {
      const close = () => {
        stop();
        try {
          controller.close();
        } catch {
          // Already closed by the other path.
        }
      };
      const send = () => {
        const now = Date.now();
        controller.enqueue(encoder.encode(`data: ${stockAt(productId, now)}\n\n`));
        tick = setTimeout(send, untilNextChange(now));
      };

      // Reconnect after 1s if the stream drops (or ends below).
      controller.enqueue(encoder.encode("retry: 1000\n\n"));
      send();
      end = setTimeout(close, STREAM_FOR_MS);
      request.signal.addEventListener("abort", close);
    },
    cancel: stop,
  });

  return new Response(stream, {
    headers: {
      "Content-Type": "text/event-stream; charset=utf-8",
      // `no-transform` also keeps `next start`'s gzip from buffering events.
      "Cache-Control": "no-cache, no-transform",
      "X-Accel-Buffering": "no",
    },
  });
}
