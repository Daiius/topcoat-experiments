#!/usr/bin/env node
// Network probes for scripts/deploy-vercel.sh verify. Each command prints one
// JSON object on stdout. Only Node.js built-ins: no install step.
//
//   node scripts/vercel-probe.mjs stream <url> [--encoding identity|br] [--gap-ms 50]
//       Arrival time of every body chunk, grouped into bursts. Tells whether
//       the streamed HTML reaches the client in pieces or is buffered.
//   node scripts/vercel-probe.mjs ws <url> [--protocol P] [--origin same|origin-only|none] [--hold S]
//       WebSocket handshake (expects 101). With --hold, keeps the socket open
//       (answering pings) and reports who closed it and when.
//   node scripts/vercel-probe.mjs sse <url> [--hold S]
//       Server-Sent Events: time of every event, and when/how the stream ended.
//
// Every command accepts --header "name: value" (repeatable), used for the
// deployment protection bypass header.

import { randomBytes } from "node:crypto";
import http from "node:http";
import https from "node:https";

const [command, target, ...rest] = process.argv.slice(2);

function parseFlags(args) {
  const flags = { header: [] };
  for (let i = 0; i < args.length; i++) {
    const arg = args[i];
    if (!arg.startsWith("--")) throw new Error(`unexpected argument: ${arg}`);
    const name = arg.slice(2);
    const value = args[++i];
    if (value === undefined) throw new Error(`missing value for ${arg}`);
    if (name === "header") flags.header.push(value);
    else flags[name] = value;
  }
  return flags;
}

function extraHeaders(flags) {
  const headers = {};
  for (const line of flags.header) {
    const colon = line.indexOf(":");
    if (colon > 0) headers[line.slice(0, colon).trim().toLowerCase()] = line.slice(colon + 1).trim();
  }
  return headers;
}

const now = () => performance.now();
const round = (ms) => Math.round(ms * 10) / 10;
const clientFor = (url) => (url.protocol === "https:" ? https : http);

// Response headers worth keeping in the results (the rest is noise).
const KEEP = [
  "cache-control",
  "content-type",
  "content-encoding",
  "transfer-encoding",
  "age",
  "x-vercel-cache",
  "x-vercel-id",
  "x-nextjs-postponed",
  "x-nextjs-prerender",
  "x-nextjs-cache",
  "x-matched-path",
  "sec-websocket-protocol",
  "upgrade",
  "connection",
  "server",
];
function pick(headers) {
  const out = {};
  for (const name of KEEP) if (headers[name] !== undefined) out[name] = headers[name];
  return out;
}

function print(result) {
  process.stdout.write(`${JSON.stringify(result)}\n`);
}

// --- stream -----------------------------------------------------------------

function stream(url, flags) {
  const encoding = flags.encoding ?? "identity";
  const gap = Number(flags["gap-ms"] ?? 50);
  const timeout = Number(flags.timeout ?? 30) * 1000;
  return new Promise((resolve) => {
    const started = now();
    const result = { kind: "stream", url: url.href, encoding, gap_ms: gap };
    const request = clientFor(url).get(
      url,
      {
        headers: { "accept-encoding": encoding, accept: "text/html", ...extraHeaders(flags) },
        timeout,
      },
      (response) => {
        result.status = response.statusCode;
        result.ttfb_ms = round(now() - started);
        result.headers = pick(response.headers);
        const chunks = [];
        response.on("data", (data) => chunks.push({ t: round(now() - started), bytes: data.length }));
        response.on("end", () => {
          result.total_ms = round(now() - started);
          result.bytes = chunks.reduce((sum, c) => sum + c.bytes, 0);
          result.chunks = chunks;
          const bursts = [];
          for (const chunk of chunks) {
            const last = bursts.at(-1);
            if (last && chunk.t - last.end_ms < gap) {
              last.end_ms = chunk.t;
              last.bytes += chunk.bytes;
              last.chunks += 1;
            } else {
              bursts.push({ start_ms: chunk.t, end_ms: chunk.t, bytes: chunk.bytes, chunks: 1 });
            }
          }
          result.bursts = bursts;
          resolve(result);
        });
      },
    );
    request.on("timeout", () => request.destroy(new Error("timeout")));
    request.on("error", (error) => resolve({ ...result, error: String(error.message ?? error) }));
  });
}

// --- ws ---------------------------------------------------------------------

/** Encodes a masked client frame (clients must mask). */
function clientFrame(opcode, payload = Buffer.alloc(0)) {
  const mask = randomBytes(4);
  const length = payload.length;
  const head = length < 126 ? Buffer.from([0x80 | opcode, 0x80 | length]) : Buffer.from([0x80 | opcode, 0x80 | 126, length >> 8, length & 0xff]);
  const body = Buffer.from(payload);
  for (let i = 0; i < body.length; i++) body[i] ^= mask[i % 4];
  return Buffer.concat([head, mask, body]);
}

/** Splits unmasked server frames off the front of `buffer`. */
function* serverFrames(state) {
  for (;;) {
    const buf = state.buffer;
    if (buf.length < 2) return;
    const opcode = buf[0] & 0x0f;
    let length = buf[1] & 0x7f;
    let offset = 2;
    if (length === 126) {
      if (buf.length < 4) return;
      length = buf.readUInt16BE(2);
      offset = 4;
    } else if (length === 127) {
      if (buf.length < 10) return;
      length = Number(buf.readBigUInt64BE(2));
      offset = 10;
    }
    if (buf.length < offset + length) return;
    state.buffer = buf.subarray(offset + length);
    yield { opcode, payload: buf.subarray(offset, offset + length) };
  }
}

function ws(url, flags) {
  const protocol = flags.protocol ?? "topcoat-runtime";
  const originMode = flags.origin ?? "same";
  const hold = Number(flags.hold ?? 0) * 1000;
  const wsUrl = new URL(url);
  const headers = {
    connection: "Upgrade",
    upgrade: "websocket",
    "sec-websocket-version": "13",
    "sec-websocket-key": randomBytes(16).toString("base64"),
    "sec-websocket-protocol": protocol,
    "user-agent": "vercel-probe",
    ...extraHeaders(flags),
  };
  // What a browser sends on a same-origin page vs. an older client that only
  // sends Origin (topcoat then compares Origin with Host).
  if (originMode !== "none") headers.origin = wsUrl.origin;
  if (originMode === "same") headers["sec-fetch-site"] = "same-origin";

  return new Promise((resolve) => {
    const started = now();
    const result = { kind: "ws", url: wsUrl.href, protocol, origin_mode: originMode, hold_s: hold / 1000 };
    let settled = false;
    const finish = (extra) => {
      if (settled) return;
      settled = true;
      resolve({ ...result, ...extra });
    };
    const request = clientFor(wsUrl).request(wsUrl, { method: "GET", headers, timeout: 30_000 });
    request.on("response", (response) => {
      // Not upgraded: keep the status and the start of the body.
      let body = "";
      response.setEncoding("utf8");
      response.on("data", (data) => {
        if (body.length < 500) body += data;
      });
      response.on("end", () =>
        finish({ status: response.statusCode, upgraded: false, headers: pick(response.headers), body_head: body.slice(0, 500), connect_ms: round(now() - started) }),
      );
    });
    request.on("upgrade", (response, socket, head) => {
      // The 30 s handshake timeout must not apply to the held connection.
      socket.setTimeout(0);
      result.status = response.statusCode;
      result.upgraded = response.statusCode === 101;
      result.headers = pick(response.headers);
      result.connect_ms = round(now() - started);
      if (!hold) {
        socket.end(clientFrame(0x8, Buffer.from([0x03, 0xe8])));
        socket.destroy();
        finish({});
        return;
      }
      const state = { buffer: Buffer.from(head) };
      const counts = { text: 0, binary: 0, ping: 0, pong: 0 };
      let closeCode = null;
      let closedBy = null;
      const timer = setTimeout(() => {
        closedBy = "client (hold elapsed)";
        socket.end(clientFrame(0x8, Buffer.from([0x03, 0xe8])));
        setTimeout(() => socket.destroy(), 2000);
      }, hold);
      socket.on("data", (data) => {
        state.buffer = Buffer.concat([state.buffer, data]);
        for (const frame of serverFrames(state)) {
          if (frame.opcode === 0x1) counts.text++;
          else if (frame.opcode === 0x2) counts.binary++;
          else if (frame.opcode === 0x9) {
            counts.ping++;
            socket.write(clientFrame(0xa, frame.payload));
          } else if (frame.opcode === 0xa) counts.pong++;
          else if (frame.opcode === 0x8) {
            closeCode = frame.payload.length >= 2 ? frame.payload.readUInt16BE(0) : null;
            closedBy ??= "server (close frame)";
            socket.end(clientFrame(0x8, frame.payload.subarray(0, 2)));
          }
        }
      });
      const done = (reason) => {
        clearTimeout(timer);
        finish({ closed_by: closedBy ?? reason, close_code: closeCode, open_ms: round(now() - started - result.connect_ms), frames: counts });
      };
      socket.on("close", () => done("server (TCP close without close frame)"));
      socket.on("error", (error) => done(`error: ${error.message}`));
    });
    request.on("timeout", () => request.destroy(new Error("timeout")));
    request.on("error", (error) => finish({ error: String(error.message ?? error) }));
    request.end();
  });
}

// --- sse --------------------------------------------------------------------

function sse(url, flags) {
  const hold = Number(flags.hold ?? 15) * 1000;
  return new Promise((resolve) => {
    const started = now();
    const result = { kind: "sse", url: url.href, hold_s: hold / 1000 };
    let settled = false;
    const finish = (extra) => {
      if (settled) return;
      settled = true;
      resolve({ ...result, ...extra });
    };
    const request = clientFor(url).get(
      url,
      { headers: { accept: "text/event-stream", "accept-encoding": "identity", ...extraHeaders(flags) }, timeout: 30_000 },
      (response) => {
        request.setTimeout(0);
        result.status = response.statusCode;
        result.ttfb_ms = round(now() - started);
        result.headers = pick(response.headers);
        const events = [];
        let pending = "";
        let current = [];
        response.setEncoding("utf8");
        response.on("data", (text) => {
          pending += text;
          let newline;
          while ((newline = pending.indexOf("\n")) >= 0) {
            const line = pending.slice(0, newline).replace(/\r$/, "");
            pending = pending.slice(newline + 1);
            if (line === "") {
              if (current.length) events.push({ t_ms: round(now() - started), lines: current });
              current = [];
            } else current.push(line);
          }
        });
        const timer = setTimeout(() => {
          request.destroy();
          end("client (hold elapsed)");
        }, hold);
        const end = (reason) => {
          clearTimeout(timer);
          const data = events.filter((e) => e.lines.some((l) => l.startsWith("data:")));
          const gaps = data.slice(1).map((e, i) => round(e.t_ms - data[i].t_ms));
          finish({ ended_by: reason, ended_after_ms: round(now() - started), events: events.length, data_events: data.length, gaps_ms: gaps, first_events: events.slice(0, 5) });
        };
        response.on("end", () => end("server (stream ended)"));
        response.on("error", (error) => end(`error: ${error.message}`));
      },
    );
    request.on("timeout", () => request.destroy(new Error("timeout")));
    request.on("error", (error) => finish({ error: String(error.message ?? error) }));
  });
}

// --- main -------------------------------------------------------------------

const commands = { stream, ws, sse };
if (!commands[command] || !target) {
  process.stderr.write("usage: vercel-probe.mjs stream|ws|sse <url> [--flag value ...]\n");
  process.exit(2);
}
try {
  print(await commands[command](new URL(target), parseFlags(rest)));
} catch (error) {
  print({ kind: command, url: target, error: String(error.message ?? error) });
}
