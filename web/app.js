import { createRunner, loadSymap } from './symap.js';

const EXAMPLES = [
  { group: 'From the 1971 manual', file: 'example-1-14.sym', name: 'Example 1-14: proximal maps' },
  { group: 'From the 1971 manual', file: 'example-1-3.sym', name: 'Example 1-3: contour map' },
  { group: 'From the 1971 manual', file: 'example-1-20.sym', name: 'Example 1-20: conformant map' },
  { group: 'Demonstrations', file: 'hills.sym', name: 'Two hills: contour and proximal' },
  { group: 'Demonstrations', file: 'furniture.sym', name: 'Map furniture: lettering, north arrow, scale' },
  { group: 'Demonstrations', file: 'districts.sym', name: 'Six districts: conformant map' },
  { group: 'Demonstrations', file: 'barrier.sym', name: 'Two banks: a river as a barrier' },
  { group: 'Open data: London and Dublin', file: 'london-boroughs.sym', name: 'London boroughs: population density, Census 2021' },
  { group: 'Open data: London and Dublin', file: 'london-no2.sym', name: 'London: nitrogen dioxide, 2025 (contour)' },
  { group: 'Open data: London and Dublin', file: 'london-house-prices.sym', name: 'Central London house prices: the Thames as a barrier' },
  { group: 'Open data: London and Dublin', file: 'london-zones.sym', name: 'London fare zones: nearest station (proximal)' },
  { group: 'Open data: London and Dublin', file: 'dublin-density.sym', name: 'Dublin City: population density, Census 2022' },
  { group: 'Open data: London and Dublin', file: 'london-suitability.sym', name: 'London: where to build, two weighted indexes (after Delmarva)' },
];

const STORE_KEY = 'symap.deck';
const $ = (id) => document.getElementById(id);
const deck = $('deck');
const sheets = $('sheets');
const messages = $('messages');

const state = { symap: null, result: null, overprint: true, zoom: 'fit', paper: 'white', turn: 0 };

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
const CAP_HEIGHT = 0.78;   // of the line pitch
const GLYPH_WIDTH = 0.68;  // of the character pitch
const SPREAD = 0.7;        // ink spread around each stroke, in pixels
// Each further strike on a cell soaks more ink into the paper: four-strike
// overprints (OXAV) print near-solid black, not just a darker O.
const SPREAD_PER_STRIKE = 1.0;
// Glyphs narrowed to the printer's type. A modern '=' spans its whole glyph
// width and prints darker than '+', inverting the standard levels' order
// (Table 3-2); the manual's printouts show a narrower '=', a little lighter
// than '+'.
const NARROW = { '=': 0.75 };

function hash(a, b, c) {
  let h = (a * 73856093) ^ (b * 19349663) ^ (c * 83492791);
  h = (h ^ (h >>> 13)) * 1274126177;
  return ((h ^ (h >>> 16)) >>> 0) / 4294967296;
}

// Font size and horizontal scale that give capitals the printed proportions.
let typeCache = null;
function typeMetrics(ctx) {
  const family = getComputedStyle(document.body).getPropertyValue('--type');
  if (typeCache && typeCache.family === family) return typeCache;
  const probe = 100;
  ctx.font = `bold ${probe}px ${family}`;
  const m = ctx.measureText('H');
  const cap = m.actualBoundingBoxAscent + m.actualBoundingBoxDescent || 0.72 * probe;
  const ink = m.actualBoundingBoxLeft + m.actualBoundingBoxRight || 0.5 * probe;
  const size = (CAP_HEIGHT * CELL_H) / (cap / probe);
  const widen = (GLYPH_WIDTH * CELL_W) / ((ink / probe) * size);
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

// `density` is canvas pixels per layout unit; by default, the zoom times
// the screen's pixel ratio.
function drawSheet(map, lines, density) {
  // Every sheet of a map is as wide as its widest line, like the paper.
  const width = Math.max(...map.lines.map((l) => Math.max(l.plain.length, ...l.strikes.map((s) => s.length))), 80);
  const cols = width + 2 * MARGIN_COLS;
  const rows = lines.length + 2 * TOP_LINES;
  const scale = state.zoom === 'fit' ? 1.5 : Number(state.zoom);
  const px = density ?? scale * (window.devicePixelRatio || 1);
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
    const struck = new Uint8Array(Math.max(...layers.map((s) => s.length), 0));
    layers.forEach((s, k) => {
      for (let c = 0; c < s.length; c++) {
        const ch = s[c];
        if (ch === ' ') continue;
        ctx.lineWidth = SPREAD + SPREAD_PER_STRIKE * struck[c]++;
        // A little ribbon wobble between strikes.
        const dx = state.overprint ? (hash(i, c, k) - 0.5) * 0.6 : 0;
        const dy = state.overprint ? (hash(c, i, k + 7) - 0.5) * 0.6 : 0;
        const x = ((MARGIN_COLS + c + 0.5) * CELL_W + dx) / type.widen;
        const narrow = NARROW[ch];
        if (narrow) {
          ctx.save();
          ctx.translate(x, y + dy);
          ctx.scale(narrow, 1);
          ctx.fillText(ch, 0, 0);
          ctx.strokeText(ch, 0, 0);
          ctx.restore();
          continue;
        }
        ctx.fillText(ch, x, y + dy);
        // Ribbon ink spreads beyond the type face.
        ctx.strokeText(ch, x, y + dy);
      }
    });
  });
  return turned(canvas, state.turn, rows * CELL_H * scale);
}

// A sheet turned a quarter-turn for sideways printing: -1 to the left (the
// foot of the page runs up the right edge, as the Laboratory's report maps
// were reproduced), 1 to the right.
function turned(canvas, turn, cssHeight) {
  if (!turn) return canvas;
  const out = document.createElement('canvas');
  out.width = canvas.height;
  out.height = canvas.width;
  out.style.width = `${cssHeight.toFixed(1)}px`;
  out.setAttribute('role', 'img');
  out.setAttribute('aria-label', `${canvas.getAttribute('aria-label')}, turned ${turn < 0 ? 'left' : 'right'}`);
  const ctx = out.getContext('2d');
  ctx.translate(out.width / 2, out.height / 2);
  ctx.rotate((turn * Math.PI) / 2);
  ctx.drawImage(canvas, -canvas.width / 2, -canvas.height / 2);
  return out;
}

// The sheets of every map: [map, lines, label, isMap].
function sheetList(r) {
  const many = r.maps.length > 1;
  return r.maps.flatMap((m, i) => {
    const ps = pages(m);
    return ps.map((lines, k) => {
      const isMap = k === ps.length - 1;
      const what = isMap ? 'Map' : 'Tables';
      return [m, lines, many ? `${what} ${i + 1}` : what, isMap];
    });
  });
}

function render(opts = {}) {
  const r = state.result;
  sheets.replaceChildren();
  $('sheet-nav').replaceChildren();
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
  const list = sheetList(r);
  const nav = $('sheet-nav');
  if (list.length > 1) {
    list.forEach(([, , label, isMap], k) => {
      const b = document.createElement('button');
      b.type = 'button';
      b.textContent = label;
      b.className = isMap ? 'map' : '';
      b.addEventListener('click', () => showSheet(k));
      nav.append(b);
    });
  }
  const firstMap = list.findIndex((x) => x[3]);
  const queue = list.map(([m, lines, label], k) => [m, lines, label, k]);
  const next = () => {
    if (token !== render.token || !queue.length) return;
    const [m, lines, label, k] = queue.shift();
    const canvas = drawSheet(m, lines);
    canvas.dataset.sheet = k;
    canvas.addEventListener('click', () => { if (state.zoom === 'fit') openSheet(m, lines, label); });
    sheets.append(canvas);
    if (opts.toMap && k === firstMap) showSheet(k, 'auto');
    requestAnimationFrame(next);
  };
  next();
}

// Scroll the printout to a sheet (within its pane on wide screens).
function showSheet(k, behavior = 'smooth') {
  const c = sheets.querySelector(`canvas[data-sheet="${k}"]`);
  if (!c) return;
  if (sheets.scrollHeight > sheets.clientHeight) {
    sheets.scrollTo({ top: c.offsetTop - sheets.offsetTop - 12, behavior });
  } else {
    c.scrollIntoView({ block: 'start', behavior });
  }
}

// A sheet at full size (100%), for reading on small screens.
function openSheet(m, lines, label) {
  const saved = state.zoom;
  state.zoom = '1';
  const canvas = drawSheet(m, lines);
  state.zoom = saved;
  $('sheet-view-title').textContent = `${label}: ${m.titles.filter(Boolean).join(' / ')}`;
  $('sheet-view-body').replaceChildren(canvas);
  $('sheet-view').showModal();
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

// The bundled type face, loaded before the first sheet is measured and drawn.
const typeLoaded = document.fonts.load('bold 16px "IBM Plex Mono"').catch(() => []);

async function run(opts = {}) {
  if (!state.symap) return;
  store.set(STORE_KEY, deck.value);
  const [result] = await Promise.all([state.symap.run(deck.value), typeLoaded]);
  if (!result) return; // superseded by a newer edit
  state.result = result;
  showMessages();
  render(opts);
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

// ------------------------------------------------------------- PNG export

const DECK_KEYWORD = 'SYMAP deck';
const PNG_DECK_KEY = 'symap.pngDeck';

// The deck embedded in a SYMAP PNG (an iTXt chunk), or null.
function deckFromPng(bytes) {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const sig = [137, 80, 78, 71, 13, 10, 26, 10];
  if (bytes.length < 8 || sig.some((b, i) => bytes[i] !== b)) return null;
  const dec = new TextDecoder();
  for (let i = 8; i + 12 <= bytes.length;) {
    const len = view.getUint32(i);
    const type = dec.decode(bytes.subarray(i + 4, i + 8));
    if (type === 'iTXt') {
      const data = bytes.subarray(i + 8, i + 8 + len);
      const nul = data.indexOf(0);
      if (dec.decode(data.subarray(0, nul)) === DECK_KEYWORD && data[nul + 1] === 0) {
        // keyword, NUL, compression flag and method, language tag, NUL,
        // translated keyword, NUL, text.
        let j = nul + 3;
        j = data.indexOf(0, j) + 1;
        j = data.indexOf(0, j) + 1;
        return dec.decode(data.subarray(j));
      }
    }
    if (type === 'IEND') break;
    i += 12 + len;
  }
  return null;
}

// One character is 0.1 inch, CELL_W layout units, so 300 dpi is exact: 30
// pixels per character and 37.5 per line.
const PRINT_DPI = 300;

const CRC_TABLE = Array.from({ length: 256 }, (_, n) => {
  let c = n;
  for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  return c >>> 0;
});

function crc32(bytes) {
  let c = 0xffffffff;
  for (const b of bytes) c = CRC_TABLE[(c ^ b) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
  const out = new Uint8Array(12 + data.length);
  const view = new DataView(out.buffer);
  view.setUint32(0, data.length);
  out.set(new TextEncoder().encode(type), 4);
  out.set(data, 8);
  view.setUint32(8 + data.length, crc32(out.subarray(4, 8 + data.length)));
  return out;
}

// An uncompressed international text chunk (UTF-8).
function itxt(keyword, text) {
  const enc = new TextEncoder();
  return chunk('iTXt', new Uint8Array([...enc.encode(keyword), 0, 0, 0, 0, 0, ...enc.encode(text)]));
}

// The PNG with its physical size (pHYs) and text chunks after the header.
function annotate(png, dpi, texts) {
  const ppm = Math.round(dpi / 0.0254);
  const phys = new Uint8Array(9);
  new DataView(phys.buffer).setUint32(0, ppm);
  new DataView(phys.buffer).setUint32(4, ppm);
  phys[8] = 1; // metres
  const parts = [png.subarray(0, 8)];
  let i = 8;
  while (i < png.length) {
    const len = new DataView(png.buffer, png.byteOffset + i).getUint32(0);
    const type = String.fromCharCode(...png.subarray(i + 4, i + 8));
    const end = i + 12 + len;
    if (type !== 'pHYs') parts.push(png.subarray(i, end));
    if (type === 'IHDR') parts.push(chunk('pHYs', phys), ...texts.map(([k, v]) => itxt(k, v)));
    i = end;
  }
  return new Blob(parts, { type: 'image/png' });
}

// Each map's sheet (the page that holds the map) at print resolution.
async function exportPng() {
  const r = state.result;
  if (!r?.ok) return;
  const density = PRINT_DPI / (10 * CELL_W);
  const maps = r.maps;
  for (const [i, m] of maps.entries()) {
    const canvas = drawSheet(m, pages(m).at(-1), density);
    const blob = await new Promise((resolve) => canvas.toBlob(resolve, 'image/png'));
    const png = new Uint8Array(await blob.arrayBuffer());
    const texts = [
      ['Title', m.titles.filter(Boolean).join(' / ')],
      ['Software', 'SYMAP, https://urschrei.github.io/SYMAP/'],
    ];
    if ($('png-deck').checked) texts.push([DECK_KEYWORD, deck.value]);
    const titled = annotate(png, PRINT_DPI, texts);
    const a = document.createElement('a');
    a.href = URL.createObjectURL(titled);
    a.download = maps.length > 1 ? `symap-${i + 1}.png` : 'symap.png';
    a.click();
    setTimeout(() => URL.revokeObjectURL(a.href), 1000);
  }
}

function outputs(kind) {
  const r = state.result;
  if (!r?.ok) return null;
  return r.maps.map((m) => (kind === 'asa' ? m.asa : m.lines.map((l) => (l.eject ? '\f' : '') + l.plain).join('\n') + '\n')).join(kind === 'asa' ? '' : '\f\n');
}

async function loadExample(file, toMap = true) {
  const res = await fetch(`examples/${file}`);
  deck.value = await res.text();
  deck.scrollTop = 0;
  deck.scrollLeft = 0;
  deck.setSelectionRange(0, 0);
  syncRuler();
  await run({ toMap });
}

function flash(button, text) {
  const old = button.innerHTML;
  button.textContent = text;
  setTimeout(() => { button.innerHTML = old; }, 1500);
}

// ------------------------------------------------------------------ layout

// The deck pane's width for 80 columns: the editor's character width, its
// padding and borders, and room for a scroll bar.
function cardWidth() {
  const probe = document.createElement('span');
  probe.style.cssText = 'position:absolute;visibility:hidden;white-space:pre;font:' + getComputedStyle(deck).font;
  probe.textContent = '0'.repeat(80);
  document.body.append(probe);
  const w = probe.getBoundingClientRect().width;
  probe.remove();
  return Math.ceil(w + 24 + 2 + 16);
}

const WIDTH_KEY = 'symap.deckWidth';

function setDeckWidth(px) {
  const max = Math.max(360, document.querySelector('.workbench').clientWidth * 0.7);
  const w = Math.round(Math.min(Math.max(px, 320), max));
  document.documentElement.style.setProperty('--deck-width', `${w}px`);
  $('divider').setAttribute('aria-valuenow', String(w));
  return w;
}

function wireLayout() {
  const head = () => document.documentElement.style.setProperty(
    '--masthead-height', `${document.querySelector('.masthead').offsetHeight}px`);
  head();
  window.addEventListener('resize', head);
  const natural = cardWidth();
  setDeckWidth(Number(store.get(WIDTH_KEY)) || natural);
  const divider = $('divider');
  divider.addEventListener('pointerdown', (e) => {
    divider.setPointerCapture(e.pointerId);
    divider.classList.add('dragging');
    const start = e.clientX;
    const from = document.querySelector('.deck').getBoundingClientRect().width;
    const move = (ev) => setDeckWidth(from + ev.clientX - start);
    const up = () => {
      divider.classList.remove('dragging');
      divider.removeEventListener('pointermove', move);
      store.set(WIDTH_KEY, String(document.querySelector('.deck').getBoundingClientRect().width));
      render();
    };
    divider.addEventListener('pointermove', move);
    divider.addEventListener('pointerup', up, { once: true });
  });
  divider.addEventListener('dblclick', () => { store.set(WIDTH_KEY, ''); setDeckWidth(natural); render(); });
  divider.addEventListener('keydown', (e) => {
    const step = e.shiftKey ? 80 : 20;
    const w = document.querySelector('.deck').getBoundingClientRect().width;
    if (e.key === 'ArrowLeft' || e.key === 'ArrowRight') {
      e.preventDefault();
      store.set(WIDTH_KEY, String(setDeckWidth(w + (e.key === 'ArrowLeft' ? -step : step))));
      render();
    }
  });

  // The Export menu closes after a choice, or a click elsewhere.
  const menu = $('export');
  menu.querySelectorAll('button').forEach((b) => b.addEventListener('click', () => {
    setTimeout(() => { menu.open = false; }, b.id === 'share' ? 1500 : 0);
  }));
  document.addEventListener('click', (e) => { if (!menu.contains(e.target)) menu.open = false; });
  menu.addEventListener('keydown', (e) => { if (e.key === 'Escape') { menu.open = false; menu.querySelector('summary').focus(); } });

  $('sheet-view-close').addEventListener('click', () => $('sheet-view').close());

  const embed = $('png-deck');
  embed.checked = store.get(PNG_DECK_KEY) !== 'no';
  embed.addEventListener('change', () => store.set(PNG_DECK_KEY, embed.checked ? 'yes' : 'no'));
}

// ---------------------------------------------------------------- import

// Imports are occasional, so they use their own instance on the main thread.
let importer = null;
const pending = { text: '', name: '', linesText: '', linesName: '', result: null };

// `auto` lets the importer choose the map: conformant for polygons,
// contour for points.
function importRequest(auto) {
  return {
    text: pending.text,
    name: pending.name,
    value: $('import-value').value,
    map: auto ? '' : $('import-map').value,
    width: Number($('import-width').value) || 8,
    title: $('import-title').value.trim(),
    outline: $('import-outline').checked,
    lines: $('import-lines').value,
    lineSymbol: $('import-line-symbol').value,
    linesText: pending.linesText,
    linesName: pending.linesName,
  };
}

function refreshImport(auto = false) {
  const r = importer.importData(importRequest(auto));
  pending.result = r;
  const summary = $('import-summary');
  summary.classList.toggle('error', !r.ok);
  $('import-create').disabled = !r.ok;
  if (!r.ok) {
    summary.textContent = (r.error.line ? `Line ${r.error.line}: ` : '') + r.error.message;
    return;
  }
  const sel = $('import-value');
  if ([...sel.options].map((o) => o.value).join() !== r.fields.join()) {
    sel.replaceChildren(...r.fields.map((f) => new Option(f, f)));
  }
  sel.value = r.value;
  $('import-map').value = r.map;
  $('import-lines-options').hidden = !(r.lines > 0 || pending.linesText);
  const notes = r.warnings.length ? ` ${r.warnings.map((w) => w[0].toUpperCase() + w.slice(1)).join('. ')}.` : '';
  const lines = r.lines ? ` ${r.lines} ${r.lines === 1 ? 'line' : 'lines'}.` : '';
  summary.textContent = `${r.count} ${r.unit}, mapping ${r.value}.${lines}${notes}`;
}

async function openImport(file) {
  if (file.type === 'image/png' || /\.png$/i.test(file.name)) {
    const text = deckFromPng(new Uint8Array(await file.arrayBuffer()));
    if (text != null) {
      deck.value = text;
      deck.scrollTop = 0;
      deck.setSelectionRange(0, 0);
      syncRuler();
      await run({ toMap: true });
      return;
    }
    pending.result = null;
    $('import-source').textContent = file.name;
    $('import-value').replaceChildren();
    $('import-summary').textContent = 'This PNG has no SYMAP deck embedded in it.';
    $('import-summary').classList.add('error');
    $('import-create').disabled = true;
    $('import-dialog').showModal();
    return;
  }
  importer ??= await loadSymap();
  pending.text = await file.text();
  pending.name = file.name;
  $('import-source').textContent = file.name;
  $('import-value').replaceChildren();
  $('import-title').value = '';
  pending.linesText = '';
  pending.linesName = '';
  $('import-lines-file').value = '';
  refreshImport(true);
  $('import-dialog').showModal();
}

function wireImport() {
  const input = $('import-file');
  $('import').addEventListener('click', () => input.click());
  input.addEventListener('change', () => { if (input.files[0]) openImport(input.files[0]); input.value = ''; });
  $('import-lines-file').addEventListener('change', async (e) => {
    const f = e.target.files[0];
    pending.linesText = f ? await f.text() : '';
    pending.linesName = f ? f.name : '';
    refreshImport();
  });
  for (const id of ['import-value', 'import-map', 'import-width', 'import-title', 'import-outline', 'import-lines', 'import-line-symbol']) {
    $(id).addEventListener('change', () => refreshImport());
  }
  $('import-cancel').addEventListener('click', () => $('import-dialog').close('cancel'));
  $('import-dialog').addEventListener('close', async () => {
    if ($('import-dialog').returnValue !== 'create') return;
    refreshImport();
    const r = pending.result;
    if (!r?.ok) return;
    deck.value = r.deck;
    deck.scrollTop = 0;
    deck.setSelectionRange(0, 0);
    syncRuler();
    await run({ toMap: true });
  });
  const editor = deck.closest('.editor');
  editor.addEventListener('dragover', (e) => {
    if ([...e.dataTransfer.types].includes('Files')) { e.preventDefault(); editor.classList.add('dropping'); }
  });
  editor.addEventListener('dragleave', () => editor.classList.remove('dropping'));
  editor.addEventListener('drop', (e) => {
    editor.classList.remove('dropping');
    const file = e.dataTransfer.files[0];
    if (file) { e.preventDefault(); openImport(file); }
  });
}

async function init() {
  buildRuler();
  wireLayout();
  const sel = $('examples');
  sel.append(new Option('Examples…', ''));
  for (const e of EXAMPLES) {
    let group = [...sel.children].find((g) => g.label === e.group);
    if (!group) {
      group = document.createElement('optgroup');
      group.label = e.group;
      sel.append(group);
    }
    group.append(new Option(e.name, e.file));
  }
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
  wireImport();

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
  $('turn').addEventListener('change', (e) => { state.turn = Number(e.target.value); render(); });
  $('download-asa').addEventListener('click', () => { const t = outputs('asa'); if (t) download('symap-asa.txt', t); });
  $('download-text').addEventListener('click', () => { const t = outputs('text'); if (t) download('symap.txt', t); });
  $('download-png').addEventListener('click', exportPng);
  $('download-values').addEventListener('click', () => {
    const maps = state.result?.ok ? state.result.maps.filter((m) => m.valuesCsv) : [];
    if (maps.length) download('symap-values.csv', maps.map((m) => m.valuesCsv).join('\n'));
  });
  $('share').addEventListener('click', async (e) => {
    const button = e.currentTarget; // gone after the first await
    const url = `${location.origin}${location.pathname}#deck=${await compress(deck.value)}`;
    history.replaceState(null, '', url);
    try { await navigator.clipboard.writeText(url); flash(button, 'Copied'); } catch { flash(button, 'Link in address bar'); }
  });
  let resizeTimer;
  window.addEventListener('resize', () => { clearTimeout(resizeTimer); resizeTimer = setTimeout(render, 150); });

  render();
  state.symap = await createRunner();

  const fromHash = /^#deck=(.+)$/.exec(location.hash);
  if (fromHash) {
    try { deck.value = await decompress(fromHash[1]); await run(); return; } catch { /* fall through */ }
  }
  const saved = store.get(STORE_KEY);
  if (saved) { deck.value = saved; await run(); } else { await loadExample(EXAMPLES[0].file, false); }
  updateStatus();
}

init().catch((err) => {
  messages.replaceChildren();
  const li = document.createElement('li');
  li.className = 'error';
  li.textContent = `Could not start SYMAP: ${err.message}`;
  messages.append(li);
});
