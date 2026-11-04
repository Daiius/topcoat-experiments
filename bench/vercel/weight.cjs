// 初回ロードの重さ（ページ × 版、ブラウザのキャッシュ空の新規コンテキスト）。
// usage: node bench/vercel/weight.cjs <path to playwright-core>   （docs/journal.md「ページごとの初回ロード」）
const pw = require(process.argv[2]);
const sites = {next: 'https://topcoat-experiments-nextjs.vercel.app', topcoat: 'https://topcoat-experiments.vercel.app'};
// VPS=https://<host> を付けると、VPS + Cloudflare に置いた topcoat 版も同じ手順で測る（ホスト名はリポジトリに書かない）。
if (process.env.VPS) sites['topcoat-vps'] = process.env.VPS;
const pages = ['/', '/products', '/products/1', '/cart'];
(async () => {
  const b = await pw.chromium.launch();
  const out = [];
  for (const [name, H] of Object.entries(sites)) for (const p of pages) {
    const ctx = await b.newContext({viewport: {width: 1280, height: 800}}); const page = await ctx.newPage();
    const reqs = [];
    page.on('requestfinished', async r => { try { const s = await r.sizes(); const h = r.headers();
      reqs.push({type: r.resourceType(), bytes: s.responseBodySize + s.responseHeadersSize, prefetch: !!(h['next-router-prefetch']), rsc: !!h['rsc']}); } catch {} });
    await page.goto(H + p, {waitUntil: 'networkidle'});
    await page.waitForTimeout(1500);
    const m = await page.evaluate(() => { const n = performance.getEntriesByType('navigation')[0];
      const lcp = performance.getEntriesByType('largest-contentful-paint'); const fcp = performance.getEntriesByName('first-contentful-paint')[0];
      return {ttfb: Math.round(n.responseStart), fcp: fcp ? Math.round(fcp.startTime) : null, dcl: Math.round(n.domContentLoadedEventEnd), load: Math.round(n.loadEventEnd)}; });
    const sum = f => reqs.filter(f).reduce((a, r) => a + r.bytes, 0);
    const kb = x => Math.round(x / 1024 * 10) / 10;
    out.push({site: name, page: p, ...m,
      reqs: reqs.length, kb: kb(sum(() => true)),
      js_n: reqs.filter(r => r.type === 'script').length, js_kb: kb(sum(r => r.type === 'script')),
      img_n: reqs.filter(r => r.type === 'image').length, img_kb: kb(sum(r => r.type === 'image')),
      prefetch_n: reqs.filter(r => r.prefetch).length, prefetch_kb: kb(sum(r => r.prefetch))});
    await ctx.close();
  }
  console.table(out);
  await b.close();
})();
