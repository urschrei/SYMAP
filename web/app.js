import { createRunner } from './symap.js';

const EXAMPLES = [
  { file: 'example-1-14.sym', name: 'Manual Example 1-14: proximal maps' },
  { file: 'example-1-3.sym', name: 'Manual Example 1-3: contour map' },
  { file: 'example-1-20.sym', name: 'Manual Example 1-20: conformant map' },
  { file: 'hills.sym', name: 'Two hills: contour and proximal' },
  { file: 'districts.sym', name: 'Six districts: conformant map' },
  { file: 'barrier.sym', name: 'Two banks: a river as a barrier' },
];

const STORE_KEY = 'symap.deck';
const $ = (id) => document.getElementById(id);
const deck = $('deck');
const sheets = $('sheets');
const messages = $('messages');

const state = { symap: null, result: null, overprint: true, zoom: 'fit', paper: 'greenbar' };

// ---------------------------------------------------------------- storage

const store = {
  get(k) { try { return localStorage.getItem(k); } catch { return null; } },
  set(k, v) { try { localStorage.setItem(k, v); } catch { /* private mode */ } },
};

// ------------------------------------------------------------ share links

async function compress(text) {
  const stream = new Blob([text]).stream().pipeThrough(new CompressionStream('deflate-raw'));
  const bytes = new Uint8Array(await new Response(stream).arrayBuffer());
  let s = '';
  for (const b of bytes) s += String.fromCharCode(b);
  return btoa(s).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
}

async function decompress(code) {
  const b64 = code.replace(/-/g, '+').replace(/_/g, '/');
  const bin = atob(b64 + '='.repeat((4 - (b64.length % 4)) % 4));
  const bytes = Uint8Array.from(bin, (c) => c.charCodeAt(0));
  const stream = new Blob([bytes]).stream().pipeThrough(new DecompressionStream('deflate-raw'));
  return new Response(stream).text();
}

// ------------------------------------------------------------- the editor

function buildRuler() {
  let tens = '', ones = '';
  for (let c = 1; c <= 90; c++) {
    tens += c % 10 === 0 ? String((c / 10) % 10) : ' ';
    ones += String(c % 10);
  }
  $('ruler').textContent = `${tens}\n${ones}`;
}

const FIELDS = [
  [1, 5, 'columns 1–5: elective number, zone or legend reference'],
  [6, 9, 'columns 6–9: legend symbol strikes'],
  [10, 10, 'column 10: zone or legend type (A, L, P)'],
  [11, 20, 'columns 11–20: down / value'],
  [21, 30, 'columns 21–30: across'],
  [31, 40, 'columns 31–40: field 3'],
  [41, 50, 'columns 41–50: field 4'],
  [51, 60, 'columns 51–60: field 5'],
  [61, 70, 'columns 61–70: field 6'],
  [71, 80, 'columns 71–80: field 7'],
];

// Columns where card fields start (1-based): elective number (4-5), legend
// symbol (6-9), shape or legend type (10), then 10-column numeric fields.
const STOPS = [4, 6, 10, 11, 21, 31, 41, 51, 61, 71, 81];

/** Replace deck text in [start, end) keeping the editor's undo history. */
function replaceText(start, end, text) {
  deck.setSelectionRange(start, end);
  if (!document.execCommand || !document.execCommand('insertText', false, text)) {
    deck.setRangeText(text, start, end, 'end');
    deck.dispatchEvent(new Event('input'));
  }
}

/**
 * Tab: right-justify a number just typed into a numeric field, then move to
 * the next field, padding with spaces. Shift+Tab: back to the previous field.
 * Returns false when the key should keep its default meaning.
 */
function tabField(back) {
  if (deck.selectionStart !== deck.selectionEnd) return false;
  const pos = deck.selectionStart;
  const lineStart = deck.value.lastIndexOf('\n', pos - 1) + 1;
  const lineEnd = (() => { const i = deck.value.indexOf('\n', pos); return i < 0 ? deck.value.length : i; })();
  const line = deck.value.slice(lineStart, lineEnd);
  const col = pos - lineStart + 1;
  if (back) {
    const prev = [...STOPS].reverse().find((s) => s < col) ?? 1;
    deck.setSelectionRange(lineStart + prev - 1, lineStart + prev - 1);
    updateStatus();
    return true;
  }
  const next = STOPS.find((s) => s > col);
  if (!next) return false;
  const fieldStart = [...STOPS].reverse().find((s) => s <= col) ?? 1;
  const typed = line.slice(fieldStart - 1, col - 1);
  const rest = line.slice(col - 1);
  if (fieldStart >= 11 && next - fieldStart === 10 && /^\s*-?(\d+\.?\d*|\.\d+)\s*$/.test(typed) && rest.trim() === '') {
    // A number in a 10-column field: right-justify it to the field's end.
    replaceText(lineStart + fieldStart - 1, lineStart + col - 1, typed.trim().padStart(10));
  } else if (rest.trim() === '' || col > line.length) {
    replaceText(pos, pos, ' '.repeat(next - col));
  } else {
    deck.setSelectionRange(lineStart + next - 1, lineStart + next - 1);
  }
  updateStatus();
  return true;
}

function updateStatus() {
  const pos = deck.selectionStart;
  const before = deck.value.slice(0, pos);
  const line = before.split('\n').length;
  const col = pos - before.lastIndexOf('\n');
  const text = deck.value.split('\n')[line - 1] ?? '';
  const field = col > 80 ? 'beyond column 80: not on the card' : (FIELDS.find(([a, b]) => col >= a && col <= b)?.[2] ?? '');
  const long = text.length > 80 ? ' · card is longer than 80 columns' : '';
  $('status').textContent = `Card ${line} · column ${col} · ${field}${long}`;
}

function syncRuler() {
  $('ruler').style.transform = `translateX(${-deck.scrollLeft}px)`;
}

function selectCard(n) {
  const lines = deck.value.split('\n');
  const start = lines.slice(0, n - 1).reduce((a, l) => a + l.length + 1, 0);
  deck.focus();
  deck.setSelectionRange(start, start + (lines[n - 1] ?? '').length);
  const lh = parseFloat(getComputedStyle(deck).lineHeight) || 19;
  deck.scrollTop = Math.max(0, (n - 4) * lh);
  updateStatus();
}

// ----------------------------------------------------------- the printout

const CELL_W = 9;          // 1/10 inch
const CELL_H = 11.25;      // 1/8 inch at 8 lines per inch
const MARGIN_COLS = 6;     // tractor-feed strip on each side
const TOP_LINES = 2;
const INK = 'rgba(30, 28, 40, 0.88)';
const CAP_HEIGHT = 0.7;    // of the line pitch
const GLYPH_WIDTH = 0.8;   // of the character pitch
const SPREAD = 0.7;        // ink spread around each stroke, in pixels

function hash(a, b, c) {
  let h = (a * 73856093) ^ (b * 19349663) ^ (c * 83492791);
  h = (h ^ (h >>> 13)) * 1274126177;
  return ((h ^ (h >>> 16)) >>> 0) / 4294967296;
}

// Font size and horizontal scale that give capitals the printed proportions.
let typeCache = null;
function typeMetrics(ctx) {
  const family = getComputedStyle(document.body).getPropertyValue('--mono');
  if (typeCache && typeCache.family === family) return typeCache;
  const probe = 100;
  ctx.font = `bold ${probe}px ${family}`;
  const m = ctx.measureText('H');
  const cap = m.actualBoundingBoxAscent + m.actualBoundingBoxDescent || 0.72 * probe;
  const ink = m.actualBoundingBoxLeft + m.actualBoundingBoxRight || 0.5 * probe;
  const size = (CAP_HEIGHT * CELL_H) / (cap / probe);
  const widen = Math.max(1, (GLYPH_WIDTH * CELL_W) / ((ink / probe) * size));
  typeCache = { family, font: `bold ${size.toFixed(2)}px ${family}`, widen };
  return typeCache;
}

// A map's printout split at its page ejects (the tables, then the map).
function pages(map) {
  const out = [];
  map.lines.forEach((l, i) => {
    if (i === 0 || l.eject) out.push([]);
    out[out.length - 1].push(l);
  });
  return out;
}

function drawSheet(map, lines) {
  // Every sheet of a map is as wide as its widest line, like the paper.
  const width = Math.max(...map.lines.map((l) => Math.max(l.plain.length, ...l.strikes.map((s) => s.length))), 80);
  const cols = width + 2 * MARGIN_COLS;
  const rows = lines.length + 2 * TOP_LINES;
  const scale = state.zoom === 'fit' ? 1.5 : Number(state.zoom);
  const dpr = window.devicePixelRatio || 1;
  const px = scale * dpr;
  const canvas = document.createElement('canvas');
  canvas.width = Math.ceil(cols * CELL_W * px);
  canvas.height = Math.ceil(rows * CELL_H * px);
  canvas.style.width = `${(cols * CELL_W * scale).toFixed(1)}px`;
  canvas.setAttribute('role', 'img');
  canvas.setAttribute('aria-label', `${map.kind} map: ${map.titles.filter(Boolean).join(' / ') || 'untitled'}`);
  const ctx = canvas.getContext('2d');
  ctx.scale(px, px);

  // Continuous-feed paper: green bars every half inch (4 lines at 8 lpi),
  // or plain white stock as the reports reproduced their maps.
  const W = cols * CELL_W, H = rows * CELL_H;
  const greenbar = state.paper === 'greenbar';
  ctx.fillStyle = greenbar ? '#fbfbf4' : '#ffffff';
  ctx.fillRect(0, 0, W, H);
  if (greenbar) {
    ctx.fillStyle = '#dfeedd';
    for (let y = 0; y < rows; y += 8) {
      ctx.fillRect(MARGIN_COLS * CELL_W - 4, (y + TOP_LINES) * CELL_H, (width) * CELL_W + 8, 4 * CELL_H);
    }
    // Tractor strips: perforation line and sprocket holes every half inch.
    ctx.strokeStyle = 'rgba(0,0,0,0.12)';
    ctx.setLineDash([2, 3]);
    for (const x of [MARGIN_COLS * CELL_W - 10, (MARGIN_COLS + width) * CELL_W + 10]) {
      ctx.beginPath(); ctx.moveTo(x, 0); ctx.lineTo(x, H); ctx.stroke();
    }
    ctx.setLineDash([]);
    ctx.fillStyle = '#e9e6dc';
    ctx.strokeStyle = 'rgba(0,0,0,0.15)';
    for (let y = CELL_H * 2; y < H; y += CELL_H * 4) {
      for (const x of [MARGIN_COLS * CELL_W / 2 - 2, W - MARGIN_COLS * CELL_W / 2 + 2]) {
        ctx.beginPath(); ctx.arc(x, y, 6, 0, Math.PI * 2); ctx.fill(); ctx.stroke();
      }
    }
  }

  // Type: every strike is printed into the same cell; the translucent ink
  // darkens where strikes overlap. (fillText with ordinary blending measured
  // fastest: about 1.7x quicker than multiply blending or glyph stamps.)
  // Chain-printer type is heavy and nearly fills its cell: on the manual's
  // printouts capitals are about 0.7 of the line pitch tall and 0.8 of the
  // character pitch wide, so the font is sized and widened to match.
  const type = typeMetrics(ctx);
  ctx.font = type.font;
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  ctx.fillStyle = INK;
  ctx.strokeStyle = INK;
  ctx.lineWidth = SPREAD;
  ctx.lineJoin = 'round';
  ctx.scale(type.widen, 1);
  lines.forEach((line, i) => {
    const y = (i + TOP_LINES + 0.55) * CELL_H;
    const layers = state.overprint ? line.strikes : [line.plain];
    layers.forEach((s, k) => {
      for (let c = 0; c < s.length; c++) {
        const ch = s[c];
        if (ch === ' ') continue;
        // A little ribbon wobble between strikes.
        const dx = state.overprint ? (hash(i, c, k) - 0.5) * 0.6 : 0;
        const dy = state.overprint ? (hash(c, i, k + 7) - 0.5) * 0.6 : 0;
        const x = ((MARGIN_COLS + c + 0.5) * CELL_W + dx) / type.widen;
        ctx.fillText(ch, x, y + dy);
        // Ribbon ink spreads beyond the type face.
        ctx.strokeText(ch, x, y + dy);
      }
    });
  });
  return canvas;
}

function render() {
  const r = state.result;
  sheets.replaceChildren();
  sheets.classList.toggle('fit', state.zoom === 'fit');
  if (!r || !r.ok) {
    const p = document.createElement('p');
    p.className = 'empty';
    p.textContent = r ? 'No printout: the deck has an error.' : 'Loading…';
    sheets.append(p);
    return;
  }
  // One sheet per frame, so long runs never block the page for long.
  const token = (render.token = (render.token || 0) + 1);
  const queue = r.maps.flatMap((m) => pages(m).map((lines) => [m, lines]));
  const next = () => {
    if (token !== render.token || !queue.length) return;
    sheets.append(drawSheet(...queue.shift()));
    requestAnimationFrame(next);
  };
  next();
}

function showMessages() {
  const r = state.result;
  messages.replaceChildren();
  const add = (cls, line, text) => {
    const li = document.createElement('li');
    li.className = cls;
    if (line != null) {
      const b = document.createElement('button');
      b.type = 'button';
      b.className = 'line';
      b.textContent = `card ${line}`;
      b.addEventListener('click', () => selectCard(line));
      li.append(b);
    }
    li.append(text);
    messages.append(li);
  };
  if (!r) return;
  if (!r.ok) {
    add('error', r.error.line, r.error.message);
    return;
  }
  for (const d of r.diagnostics) add('warning', d.line, d.message);
  r.maps.forEach((m, i) => {
    for (const w of m.warnings) {
      const match = /^line (\d+): (.*)$/.exec(w);
      if (match) add('warning', Number(match[1]), `map ${i + 1}: ${match[2]}`);
      else add('warning', null, `map ${i + 1}: ${w}`);
    }
  });
}

async function run() {
  if (!state.symap) return;
  store.set(STORE_KEY, deck.value);
  const result = await state.symap.run(deck.value);
  if (!result) return; // superseded by a newer edit
  state.result = result;
  showMessages();
  render();
  $('download-values').disabled = !(result.ok && result.maps.some((m) => m.valuesCsv));
}

// --------------------------------------------------------------- actions

function download(name, text) {
  const a = document.createElement('a');
  a.href = URL.createObjectURL(new Blob([text], { type: 'text/plain' }));
  a.download = name;
  a.click();
  setTimeout(() => URL.revokeObjectURL(a.href), 1000);
}

function outputs(kind) {
  const r = state.result;
  if (!r?.ok) return null;
  return r.maps.map((m) => (kind === 'asa' ? m.asa : m.lines.map((l) => (l.eject ? '\f' : '') + l.plain).join('\n') + '\n')).join(kind === 'asa' ? '' : '\f\n');
}

async function loadExample(file) {
  const res = await fetch(`examples/${file}`);
  deck.value = await res.text();
  deck.scrollTop = 0;
  deck.scrollLeft = 0;
  deck.setSelectionRange(0, 0);
  syncRuler();
  await run();
}

function flash(button, text) {
  const old = button.textContent;
  button.textContent = text;
  setTimeout(() => { button.textContent = old; }, 1500);
}

async function init() {
  buildRuler();
  const sel = $('examples');
  sel.append(new Option('Examples…', ''));
  for (const e of EXAMPLES) sel.append(new Option(e.name, e.file));
  sel.addEventListener('change', () => { if (sel.value) loadExample(sel.value); sel.value = ''; });

  let timer;
  deck.addEventListener('input', () => { clearTimeout(timer); timer = setTimeout(run, 300); updateStatus(); });
  for (const ev of ['keyup', 'click', 'select']) deck.addEventListener(ev, updateStatus);
  deck.addEventListener('scroll', syncRuler);
  deck.addEventListener('keydown', (e) => {
    if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') { e.preventDefault(); run(); }
    else if (e.key === 'Tab' && !e.altKey && !e.ctrlKey && !e.metaKey) {
      // Escape moves focus out of the editor; Tab and Shift+Tab move between fields.
      if (tabField(e.shiftKey)) e.preventDefault();
    } else if (e.key === 'Escape') {
      deck.blur();
    }
  });
  $('run').addEventListener('click', run);

  const setMode = (overprint) => {
    state.overprint = overprint;
    $('mode-overprint').setAttribute('aria-pressed', String(overprint));
    $('mode-plain').setAttribute('aria-pressed', String(!overprint));
    render();
  };
  $('mode-overprint').addEventListener('click', () => setMode(true));
  $('mode-plain').addEventListener('click', () => setMode(false));
  $('zoom').addEventListener('change', (e) => { state.zoom = e.target.value; render(); });
  $('paper').addEventListener('change', (e) => { state.paper = e.target.value; render(); });
  $('download-asa').addEventListener('click', () => { const t = outputs('asa'); if (t) download('symap-asa.txt', t); });
  $('download-text').addEventListener('click', () => { const t = outputs('text'); if (t) download('symap.txt', t); });
  $('download-values').addEventListener('click', () => {
    const maps = state.result?.ok ? state.result.maps.filter((m) => m.valuesCsv) : [];
    if (maps.length) download('symap-values.csv', maps.map((m) => m.valuesCsv).join('\n'));
  });
  $('share').addEventListener('click', async (e) => {
    const url = `${location.origin}${location.pathname}#deck=${await compress(deck.value)}`;
    history.replaceState(null, '', url);
    try { await navigator.clipboard.writeText(url); flash(e.target, 'Copied'); } catch { flash(e.target, 'Link in address bar'); }
  });
  let resizeTimer;
  window.addEventListener('resize', () => { clearTimeout(resizeTimer); resizeTimer = setTimeout(render, 150); });

  render();
  state.symap = await createRunner('symap.wasm');

  const fromHash = /^#deck=(.+)$/.exec(location.hash);
  if (fromHash) {
    try { deck.value = await decompress(fromHash[1]); await run(); return; } catch { /* fall through */ }
  }
  const saved = store.get(STORE_KEY);
  if (saved) { deck.value = saved; await run(); } else { await loadExample(EXAMPLES[0].file); }
  updateStatus();
}

init().catch((err) => {
  messages.replaceChildren();
  const li = document.createElement('li');
  li.className = 'error';
  li.textContent = `Could not start SYMAP: ${err.message}`;
  messages.append(li);
});
