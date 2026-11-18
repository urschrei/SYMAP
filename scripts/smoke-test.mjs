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
