const fs = require('fs'), path = require('path');
const [html, outDir] = process.argv.slice(2);
const src = fs.readFileSync(html, 'utf8');
const script = src.slice(src.indexOf('<script>') + 8, src.indexOf('// ---------- runtime ----------'));

const EXCLUDE = new Set(['pacbar', 'backflip', 'candycane', 'gingerbread', 'sweater', 'mummy', 'witch', 'lunar',
  'thanksgiving', 'rocket', 'butterfly', 'turbo', 'hotdog', 'puppy', 'snake', 'knight', 'train', 'chef', 'trip']);
const ERROR = new Set(['error', 'thunder-error']);
const LOOP_FROM = {};

const window = {};
const ANIMS = eval(script + ';ANIMS');
const G = window.G;

const ESC = '\x1b';
const sgr = col => col ? `${ESC}[38;2;${parseInt(col.slice(1, 3), 16)};${parseInt(col.slice(3, 5), 16)};${parseInt(col.slice(5, 7), 16)}m` : `${ESC}[0m`;
function ansiRows(ch, co) {
  return ch.map((r, y) => {
    let end = r.length;
    while (end > 0 && r[end - 1] === ' ') end--;
    let s = '', cur = null;
    for (let x = 0; x < end; x++) {
      const col = co[y][x];
      if (col !== cur) { s += sgr(col); cur = col; }
      s += r[x];
    }
    if (cur) s += `${ESC}[0m`;
    return s;
  }).join('\n');
}

for (const dir of ['normal', 'error']) {
  fs.rmSync(path.join(outDir, dir), { recursive: true, force: true });
  fs.mkdirSync(path.join(outDir, dir), { recursive: true });
}
const written = { normal: [], error: [] };
for (const a of ANIMS) {
  if (a.meow || EXCLUDE.has(a.id)) continue;
  const kind = ERROR.has(a.id) ? 'error' : 'normal';
  const frames = [], bars = [];
  for (let t = 0; t < a.period; t++) {
    const g = new G(a.w, a.h);
    a.draw(g, t);
    if (a.cur) frames.push(g.plain()); else frames.push(ansiRows(g.ch, g.co));
    if (a.bar && !a.cur) {
      const cells = a.bar(t);
      bars.push(ansiRows([cells.map(c => c[0])], [cells.map(c => c[1])]));
    }
  }
  if (a.id === 'error') frames[0] = '\n' + frames[0].split('\n').slice(1).join('\n');
  const out = { name: a.name, fps: a.fps, frames };
  if (bars.length) out.bars = bars;
  if (LOOP_FROM[a.id] != null) out.loopFrom = LOOP_FROM[a.id];
  fs.writeFileSync(path.join(outDir, kind, a.id + '.json'), JSON.stringify(out, null, '\t') + '\n');
  written[kind].push(a.id);
}
console.log(JSON.stringify(written));
