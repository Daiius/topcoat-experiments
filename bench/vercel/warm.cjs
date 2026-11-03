// /products/1 の初回・再訪問・一覧からのクリック遷移・1 秒ホバー後のクリック遷移（h1 と関連商品が出るまで、転送量）。3 回。
// usage: node bench/vercel/warm.cjs <path to playwright-core>   （docs/journal.md「ブラウザのキャッシュあり・なし」）
const pw = require(process.argv[2]);
const sites = {next: 'https://topcoat-experiments-nextjs.vercel.app', topcoat: 'https://topcoat-experiments.vercel.app'};
const relatedSel = 'xpath=//h2[contains(., "Related products")]/following::a[starts-with(@href, "/products/")][1]';
async function net(page) {
  const cdp = await page.context().newCDPSession(page); await cdp.send('Network.enable');
  const st = {reqs: 0, cached: 0, bytes: 0};
  cdp.on('Network.requestWillBeSent', () => st.reqs++);
  cdp.on('Network.requestServedFromCache', () => st.cached++);
  cdp.on('Network.responseReceived', e => { if (e.response.fromDiskCache || e.response.fromMemoryCache) st.cached++; });
  cdp.on('Network.loadingFinished', e => st.bytes += e.encodedDataLength);
  return () => { const r = {...st, kb: Math.round(st.bytes / 102.4) / 10}; st.reqs = st.cached = st.bytes = 0; delete r.bytes; return r; };
}
async function timedLoad(page, url) {
  const t0 = Date.now(); await page.goto(url, {waitUntil: 'commit'});
  await page.locator('h1', {hasNotText: 'All products'}).first().waitFor(); const h1 = Date.now() - t0;
  await page.locator(relatedSel).waitFor({state: 'attached'}); const rel = Date.now() - t0;
  await page.waitForLoadState('networkidle'); return {h1, related: rel};
}
(async () => {
  const b = await pw.chromium.launch(); const rows = [];
  for (let run = 1; run <= 3; run++) for (const [name, H] of Object.entries(sites)) {
    const ctx = await b.newContext({viewport: {width: 1280, height: 800}}); const page = await ctx.newPage(); const take = await net(page);
    const a = await timedLoad(page, H + '/products/1'); rows.push({run, site: name, phase: 'A 初回(キャッシュ空)', ...a, ...take()});
    const c = await timedLoad(page, H + '/products/1'); rows.push({run, site: name, phase: 'B 再訪問(goto)', ...c, ...take()});
    await page.goto(H + '/products', {waitUntil: 'networkidle'}); await page.waitForTimeout(2000); take();
    const link = page.locator('main a[href^="/products/"]:not([href="/products/1"])').nth(2); const href = await link.getAttribute('href');
    const t0 = Date.now(); await link.click();
    await page.waitForURL(H + href, {waitUntil: 'commit'}); await page.locator('h1', {hasNotText: 'All products'}).first().waitFor(); const h1 = Date.now() - t0;
    await page.locator(relatedSel).waitFor({state: 'attached'}); const rel = Date.now() - t0;
    await page.waitForLoadState('networkidle'); await page.waitForTimeout(500);
    rows.push({run, site: name, phase: 'C 一覧→詳細(クリック)', h1, related: rel, ...take()});
    // D: 一覧に戻り、未訪問の商品に 1 秒ホバーしてからクリック（先読みが済んだ状態の遷移）
    await page.goto(H + '/products', {waitUntil: 'networkidle'}); await page.waitForTimeout(1000); take();
    const link2 = page.locator('main a[href^="/products/"]:not([href="/products/1"])').nth(5); const href2 = await link2.getAttribute('href');
    await link2.hover(); await page.waitForTimeout(1000);
    const t1 = Date.now(); await link2.click();
    await page.waitForURL(H + href2, {waitUntil: 'commit'}); await page.locator('h1', {hasNotText: 'All products'}).first().waitFor(); const h1d = Date.now() - t1;
    await page.locator(relatedSel).waitFor({state: 'attached'}); const reld = Date.now() - t1;
    await page.waitForLoadState('networkidle'); await page.waitForTimeout(500);
    rows.push({run, site: name, phase: 'D 1 秒ホバー後にクリック', h1: h1d, related: reld, ...take()});
    await ctx.close();
  }
  console.table(rows); await b.close();
})();
