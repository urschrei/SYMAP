// Browser smoke test for the built site: every example deck renders without
// errors, and a broken deck reports its card. Serve target/site first, e.g.
//   python3 -m http.server 8765 --directory target/site
// then run: node scripts/smoke-test.mjs [http://127.0.0.1:8765/]
import { chromium } from 'playwright';

const base = process.argv[2] ?? 'http://127.0.0.1:8765/';
const failures = [];
const check = (ok, what) => {
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${what}`);
  if (!ok) failures.push(what);
};

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1280, height: 900 } });
const errors = [];
page.on('pageerror', (e) => errors.push(e.message));
page.on('console', (m) => { if (m.type() === 'error') errors.push(m.text()); });

await page.goto(base, { waitUntil: 'networkidle' });
await page.waitForSelector('#sheets canvas', { timeout: 20000 });
check(true, 'page loads and renders the default deck');

const examples = await page.$$eval('#examples option', (os) => os.map((o) => o.value).filter(Boolean));
check(examples.length >= 4, `example selector lists ${examples.length} decks`);
for (const file of examples) {
  await page.selectOption('#examples', file);
  await page.waitForFunction(
    () => document.querySelectorAll('#sheets canvas').length > 0 || document.querySelector('#messages li.error'),
    null,
    { timeout: 20000 },
  );
  await page.waitForTimeout(300);
  const sheets = await page.$$eval('#sheets canvas', (c) => c.length);
  const problems = await page.$$eval('#messages li.error', (l) => l.map((x) => x.innerText));
  check(sheets > 0 && problems.length === 0, `${file}: ${sheets} sheet(s)${problems.length ? `, errors: ${problems}` : ''}`);
}

const upright = await page.$eval('#sheets canvas', (c) => [c.width, c.height]);
await page.selectOption('#turn', '-1');
await page.waitForTimeout(300);
const sideways = await page.$eval('#sheets canvas', (c) => [c.width, c.height]);
check(sideways[0] === upright[1] && sideways[1] === upright[0], 'a sheet turns sideways');
await page.selectOption('#turn', '0');

await page.click('#export summary');
const [download] = await Promise.all([page.waitForEvent('download'), page.click('#download-png')]);
const png = (await import('node:fs')).readFileSync(await download.path());
const phys = png.indexOf('pHYs');
const ppm = phys > 0 ? png.readUInt32BE(phys + 4) : 0;
check(png.subarray(1, 4).toString() === 'PNG' && Math.round(ppm * 0.0254) === 300 && png.includes('SYMAP deck'),
  `a map exports as a 300 dpi PNG with its deck (${png.length} bytes)`);
const before = await page.$eval('#deck', (d) => d.value);
await page.fill('#deck', '');
await page.setInputFiles('#import-file', { name: 'map.png', mimeType: 'image/png', buffer: png });
await page.waitForFunction((t) => document.getElementById('deck').value === t, before, { timeout: 10000 });
check(true, 'a PNG dropped on the editor restores its deck');

const points = 'name,lat,lon,value\nA,51.52,-0.15,48\nB,51.52,-0.12,28\nC,51.45,0.01,24\nD,51.42,-0.34,13\n';
await page.setInputFiles('#import-file', { name: 'points.csv', mimeType: 'text/csv', buffer: Buffer.from(points) });
await page.waitForSelector('#import-dialog[open]', { timeout: 10000 });
const summary = await page.textContent('#import-summary');
const river = '{"type":"Feature","properties":{"name":"River"},"geometry":{"type":"LineString","coordinates":[[-0.4,51.45],[-0.12,51.5],[0.1,51.47]]}}';
await page.setInputFiles('#import-lines-file', { name: 'river.geojson', mimeType: 'application/geo+json', buffer: Buffer.from(river) });
await page.waitForFunction(() => document.getElementById('import-summary').textContent.includes('1 line'), null, { timeout: 10000 });
await page.click('#import-create');
await page.waitForFunction(() => document.getElementById('deck').value.startsWith('# Made by symap import'), null, { timeout: 10000 });
await page.waitForTimeout(500);
const imported = await page.$$eval('#messages li.error', (l) => l.length);
const withLines = await page.$eval('#deck', (d) => d.value.includes('C-OTOLEGENDS'));
check(summary.startsWith('4 points, mapping value') && imported === 0 && withLines, `points and a lines file import into a deck that runs (${summary.trim()})`);

const square = (x, v) => `{"type":"Feature","properties":{"name":"Z${v}","v":${v}},"geometry":{"type":"Polygon","coordinates":[[[${x},51.4],[${x + 0.1},51.4],[${x + 0.1},51.5],[${x},51.5],[${x},51.4]]]}}`;
const zones = `{"type":"FeatureCollection","features":[${square(-0.2, 1)},${square(-0.1, 2)}]}`;
await page.setInputFiles('#import-file', { name: 'zones.geojson', mimeType: 'application/geo+json', buffer: Buffer.from(zones) });
await page.waitForSelector('#import-dialog[open]', { timeout: 10000 });
const zoneSummary = await page.textContent('#import-summary');
const zoneMap = await page.$eval('#import-map', (s) => s.value);
await page.click('#import-create');
await page.waitForFunction(() => document.getElementById('deck').value.includes('A-CONFORMOLINES'), null, { timeout: 10000 });
await page.waitForTimeout(500);
const zoneErrors = await page.$$eval('#messages li.error', (l) => l.length);
check(zoneSummary.startsWith('2 zones') && zoneMap === 'conformant' && zoneErrors === 0, `polygons import as a conformant map (${zoneSummary.trim()})`);

await page.fill('#deck', 'B-DATA POINTS\n1 2\nnot a card\n99999\n');
await page.waitForSelector('#messages li.error button.line', { timeout: 10000 });
await page.click('#messages li.error button.line');
const selected = await page.$eval('#deck', (d) => d.value.slice(d.selectionStart, d.selectionEnd));
check(selected === 'not a card', 'an error links to its card');

check(errors.length === 0, `no console or page errors${errors.length ? `: ${errors.join(' | ')}` : ''}`);
await browser.close();
if (failures.length) {
  console.error(`${failures.length} check(s) failed`);
  process.exit(1);
}
