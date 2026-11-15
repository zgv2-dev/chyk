// The footer wordmark can be cut with scissors. The cut follows the pointer exactly from the
// moment it touches the mark, so any shape can be cut out: whenever a cut closes a loop or
// runs edge to edge, the part it separated comes loose and drifts a little. Every piece can
// be cut again, down to shards a few pixels big.
//
// The mark is redrawn on a canvas (same gradient, scanlines and light sweep as the CSS one,
// which stays as the no-JavaScript fallback). Paper is tracked on a grid of small cells: each
// cell belongs to one piece or to none, a cut clears the cells under the scissors, and a
// flood fill finds out whether a piece fell apart.

type Pt = { x: number; y: number };
type Piece = {
  id: number;
  x0: number; y0: number; x1: number; y1: number; // bounding box, in cells, inclusive
  count: number;
  c: Pt; // centroid in source px, the pivot for rotation
  t: Pt; v: Pt; // offset and velocity, px and px/s
  a: number; va: number; // angle and angular velocity
  glow: boolean; // holds part of the icon, which keeps its green glow
  mask: HTMLCanvasElement; dirty: boolean;
};
type Shard = { p: Pt; v: Pt; at: number };
type Trail = { a: Pt; b: Pt; at: number };

const CELL = 2; // px per grid cell
const KERF = 0.9; // cut radius, in cells
const PAD = 40; // room around the mark for glow and drifting pieces
const SWEEP_MS = 7000;

const word = document.querySelector<HTMLElement>(".word");
const still = matchMedia("(prefers-reduced-motion: reduce)");

const rot = (p: Pt, a: number): Pt => ({ x: p.x * Math.cos(a) - p.y * Math.sin(a), y: p.x * Math.sin(a) + p.y * Math.cos(a) });

// CSS cubic-bezier(.4, 0, .2, 1), the timing of the sweep.
const ease = (x: number) => {
  const bx = (t: number) => 3 * (1 - t) ** 2 * t * 0.4 + 3 * (1 - t) * t * t * 0.2 + t ** 3;
  const by = (t: number) => 3 * (1 - t) * t * t + t ** 3;
  let lo = 0, hi = 1;
  for (let i = 0; i < 24; i++) { const m = (lo + hi) / 2; bx(m) < x ? (lo = m) : (hi = m); }
  return by((lo + hi) / 2);
};
const sweepAt = (ms: number) => {
  if (still.matches) return 0.5;
  const k = ((((ms - 1000) % SWEEP_MS) + SWEEP_MS) % SWEEP_MS) / SWEEP_MS;
  return k >= 0.45 ? 1.3 : -0.3 + 1.6 * ease(k / 0.45);
};

if (word) {
  const img = word.querySelector("img")!;
  const span = word.querySelector("span")!;
  const canvas = document.createElement("canvas");
  canvas.className = "snip";
  canvas.setAttribute("aria-hidden", "true");
  const ctx = canvas.getContext("2d")!;
  const base = document.createElement("canvas"); // the text with its gradient and scanlines
  const frame = document.createElement("canvas"); // base plus this frame's sweep and the icon
  const scratch = document.createElement("canvas"); // one piece at a time, masked
  const sctx = scratch.getContext("2d")!;
  const glue = document.createElement("button");
  glue.className = "glue";
  glue.type = "button";
  glue.textContent = "Put it back together";
  glue.hidden = true;

  let w = 0, h = 0, dpr = 1, gw = 0, gh = 0;
  let textBox = { x: 0, y: 0, w: 0, h: 0 }, imgBox = { x: 0, y: 0, w: 0, h: 0 };
  let owner = new Int32Array(0);
  let pieces = new Map<number, Piece>(), nextId = 1, pristine = true;
  let shards: Shard[] = [], trail: Trail[] = [];
  let last: Pt | null = null, visible = false, running = false, gluing = 0, chomp = 0;

  // ───── drawing the mark ─────

  const drawBase = (size: number, cs: CSSStyleDeclaration) => {
    const b = base.getContext("2d")!;
    b.setTransform(dpr, 0, 0, dpr, 0, 0);
    b.clearRect(0, 0, w, h);
    b.font = `700 ${size}px SpaceMono`;
    const m = b.measureText("CHYK");
    const lh = parseFloat(cs.lineHeight) || size * 0.8;
    const baseline = textBox.y + (lh - m.fontBoundingBoxAscent - m.fontBoundingBoxDescent) / 2 + m.fontBoundingBoxAscent;
    const spacing = parseFloat(cs.letterSpacing) || 0;
    let x = textBox.x;
    for (const ch of span.textContent ?? "") { b.fillText(ch, x, baseline); x += b.measureText(ch).width + spacing; }
    b.globalCompositeOperation = "source-in";
    const g = b.createLinearGradient(0, textBox.y, 0, textBox.y + textBox.h);
    g.addColorStop(0, "#16502f"); g.addColorStop(1, "#072011");
    b.fillStyle = g;
    b.fillRect(0, 0, w, h);
    b.globalCompositeOperation = "source-atop";
    b.fillStyle = "rgba(0,0,0,.35)";
    for (let y = textBox.y + textBox.h - 2; y > textBox.y - 5; y -= 5) b.fillRect(textBox.x, y, textBox.w, 2);
    b.globalCompositeOperation = "source-over";
  };

  // This frame's source image: the text with the light sweep across it, then the icon.
  const compose = (now: number, glow = true) => {
    const f = frame.getContext("2d")!;
    f.setTransform(1, 0, 0, 1, 0, 0);
    f.clearRect(0, 0, frame.width, frame.height);
    f.drawImage(base, 0, 0);
    f.setTransform(dpr, 0, 0, dpr, 0, 0);
    // 105deg, like the CSS: a band 32% of the gradient line wide, centred on the sweep.
    const ang = (105 * Math.PI) / 180, dir = { x: Math.sin(ang), y: -Math.cos(ang) };
    const len = Math.abs(textBox.w * dir.x) + Math.abs(textBox.h * dir.y);
    const mid = { x: textBox.x + textBox.w / 2, y: textBox.y + textBox.h / 2 };
    const at = (k: number) => ({ x: mid.x + dir.x * len * (k - 0.5), y: mid.y + dir.y * len * (k - 0.5) });
    const s = sweepAt(now), p0 = at(s - 0.16), p1 = at(s + 0.16);
    const g = f.createLinearGradient(p0.x, p0.y, p1.x, p1.y);
    g.addColorStop(0, "rgba(217,255,233,0)"); g.addColorStop(0.5, "rgba(217,255,233,.9)"); g.addColorStop(1, "rgba(217,255,233,0)");
    f.globalCompositeOperation = "source-atop";
    f.fillStyle = g;
    f.fillRect(textBox.x, textBox.y, textBox.w, textBox.h);
    f.globalCompositeOperation = "source-over";
    f.globalAlpha = 0.9;
    if (glow) { f.shadowColor = "rgba(77,255,155,.35)"; f.shadowBlur = 30; }
    f.drawImage(img, imgBox.x, imgBox.y, imgBox.w, imgBox.h);
    f.globalAlpha = 1;
    f.shadowBlur = 0;
  };

  // ───── pieces ─────

  const newPiece = (id: number, from?: Piece): Piece => ({
    id, x0: 0, y0: 0, x1: 0, y1: 0, count: 0, c: { x: 0, y: 0 },
    t: { x: 0, y: 0 }, v: { x: 0, y: 0 }, a: from?.a ?? 0, va: from?.va ?? 0, glow: false,
    mask: document.createElement("canvas"), dirty: true,
  });

  // Recount a piece's cells, bounding box and centroid. A new centroid moves the pivot,
  // so the offset is adjusted to keep the piece exactly where it was on screen.
  const measure = (pc: Piece, sx0: number, sy0: number, sx1: number, sy1: number, from?: Piece) => {
    let n = 0, mx = 0, my = 0, x0 = gw, y0 = gh, x1 = -1, y1 = -1, glow = false;
    const ix0 = (imgBox.x / CELL) | 0, iy0 = (imgBox.y / CELL) | 0, ix1 = ((imgBox.x + imgBox.w) / CELL) | 0, iy1 = ((imgBox.y + imgBox.h) / CELL) | 0;
    for (let y = sy0; y <= sy1; y++) for (let x = sx0; x <= sx1; x++) {
      if (owner[y * gw + x] !== pc.id) continue;
      n++; mx += x; my += y;
      if (x < x0) x0 = x; if (x > x1) x1 = x; if (y < y0) y0 = y; if (y > y1) y1 = y;
      glow ||= x >= ix0 && x <= ix1 && y >= iy0 && y <= iy1;
    }
    const c = n ? { x: (mx / n + 0.5) * CELL, y: (my / n + 0.5) * CELL } : pc.c;
    const ref = from ?? pc;
    const d = rot({ x: c.x - ref.c.x, y: c.y - ref.c.y }, ref.a);
    pc.t = { x: d.x + ref.c.x - c.x + ref.t.x, y: d.y + ref.c.y - c.y + ref.t.y };
    Object.assign(pc, { count: n, c, x0, y0, x1, y1, glow, dirty: true });
  };

  // After a cut, flood-fill the piece. The biggest part keeps it; every other part becomes a
  // new piece that drifts away from the cut a little; crumbs fall off as shards.
  const regroup = (pc: Piece) => {
    const { x0, y0, x1, y1 } = pc;
    const bw = x1 - x0 + 1, seen = new Uint8Array(bw * (y1 - y0 + 1));
    const parts: number[][] = [];
    const stack: number[] = [];
    for (let y = y0; y <= y1; y++) for (let x = x0; x <= x1; x++) {
      const k = (y - y0) * bw + (x - x0);
      if (seen[k] || owner[y * gw + x] !== pc.id) continue;
      const cells: number[] = [];
      seen[k] = 1; stack.push(y * gw + x);
      while (stack.length) {
        const i = stack.pop()!;
        cells.push(i);
        const cx = i % gw, cy = (i / gw) | 0;
        for (const [nx, ny] of [[cx + 1, cy], [cx - 1, cy], [cx, cy + 1], [cx, cy - 1]]) {
          if (nx < x0 || nx > x1 || ny < y0 || ny > y1) continue;
          const nk = (ny - y0) * bw + (nx - x0);
          if (seen[nk] || owner[ny * gw + nx] !== pc.id) continue;
          seen[nk] = 1; stack.push(ny * gw + nx);
        }
      }
      parts.push(cells);
    }
    if (parts.length <= 1) { measure(pc, x0, y0, x1, y1); if (!pc.count) pieces.delete(pc.id); return; }
    parts.sort((p, q) => q.length - p.length);
    const parent = { ...pc, t: { ...pc.t }, c: { ...pc.c } };
    const now = performance.now();
    parts.forEach((cells, i) => {
      if (i === 0) return;
      if (cells.length < 4) {
        // Crumbs: clear them and let them fall as shards.
        for (const j of cells) {
          owner[j] = 0;
          const local = { x: ((j % gw) + 0.5) * CELL, y: (((j / gw) | 0) + 0.5) * CELL };
          const r = rot({ x: local.x - parent.c.x, y: local.y - parent.c.y }, parent.a);
          shards.push({ p: { x: r.x + parent.c.x + parent.t.x, y: r.y + parent.c.y + parent.t.y }, v: { x: (Math.random() - 0.5) * 60, y: -20 - Math.random() * 40 }, at: now });
        }
        return;
      }
      const np = newPiece(nextId++, parent);
      for (const j of cells) owner[j] = np.id;
      measure(np, x0, y0, x1, y1, parent);
      // Away from the parent's centre, a few pixels' worth, with a slight turn.
      const away = rot({ x: np.c.x - parent.c.x, y: np.c.y - parent.c.y }, parent.a);
      const n = Math.hypot(away.x, away.y) || 1, speed = 20 + Math.random() * 18;
      np.v = { x: parent.v.x + (away.x / n) * speed, y: parent.v.y + (away.y / n) * speed };
      np.va = parent.va + (Math.random() - 0.5) * 0.3 * Math.min(1, 40 / Math.sqrt(np.count));
      pieces.set(np.id, np);
    });
    measure(pc, x0, y0, x1, y1, parent);
  };

  // Clear the cells under the scissors at a screen point, in every piece lying there.
  const carve = (q: Pt, touched: Set<Piece>) => {
    for (const pc of pieces.values()) {
      const r = rot({ x: q.x - pc.c.x - pc.t.x, y: q.y - pc.c.y - pc.t.y }, -pc.a);
      const lx = (r.x + pc.c.x) / CELL, ly = (r.y + pc.c.y) / CELL;
      if (lx < pc.x0 - 2 || lx > pc.x1 + 2 || ly < pc.y0 - 2 || ly > pc.y1 + 2) continue;
      const k = Math.ceil(KERF);
      for (let y = Math.floor(ly - k); y <= Math.ceil(ly + k); y++) for (let x = Math.floor(lx - k); x <= Math.ceil(lx + k); x++) {
        if (x < 0 || y < 0 || x >= gw || y >= gh) continue;
        if ((x + 0.5 - lx) ** 2 + (y + 0.5 - ly) ** 2 > KERF * KERF) continue;
        const i = y * gw + x;
        if (owner[i] !== pc.id) continue;
        owner[i] = 0;
        pc.count--;
        pc.dirty = true;
        touched.add(pc);
      }
    }
  };

  // A piece's mask: its cells, one pixel each, scaled up smoothly when drawn.
  const maskOf = (pc: Piece) => {
    if (!pc.dirty) return pc.mask;
    const bw = pc.x1 - pc.x0 + 1, bh = pc.y1 - pc.y0 + 1;
    pc.mask.width = bw; pc.mask.height = bh;
    const m = pc.mask.getContext("2d")!;
    const data = m.createImageData(bw, bh);
    for (let y = 0; y < bh; y++) for (let x = 0; x < bw; x++) {
      if (owner[(y + pc.y0) * gw + x + pc.x0] === pc.id) data.data[(y * bw + x) * 4 + 3] = 255;
    }
    m.putImageData(data, 0, 0);
    pc.dirty = false;
    return pc.mask;
  };

  const build = async () => {
    const cs = getComputedStyle(span);
    const size = parseFloat(cs.fontSize);
    await document.fonts.load(`700 ${size}px SpaceMono`);
    await img.decode().catch(() => {});
    const wr = word.getBoundingClientRect(), sr = span.getBoundingClientRect(), ir = img.getBoundingClientRect();
    dpr = Math.min(devicePixelRatio || 1, 2);
    w = Math.ceil(wr.width + PAD * 2); h = Math.ceil(wr.height + PAD * 2);
    for (const c of [canvas, base, frame]) { c.width = w * dpr; c.height = h * dpr; }
    canvas.style.cssText = `width:${w}px;height:${h}px;inset:${-PAD}px`;
    textBox = { x: sr.left - wr.left + PAD, y: sr.top - wr.top + PAD, w: sr.width, h: sr.height };
    imgBox = { x: ir.left - wr.left + PAD, y: ir.top - wr.top + PAD, w: ir.width, h: ir.height };
    drawBase(size, cs);

    // Paper: every cell with any ink in or next to it, so cell edges never clip a glyph.
    compose(0, false);
    gw = Math.ceil(w / CELL); gh = Math.ceil(h / CELL);
    const px = frame.getContext("2d")!.getImageData(0, 0, frame.width, frame.height).data;
    const ink = new Uint8Array(gw * gh);
    const step = CELL * dpr;
    for (let y = 0; y < frame.height; y++) for (let x = 0; x < frame.width; x++) {
      if (px[(y * frame.width + x) * 4 + 3] > 6) ink[((y / step) | 0) * gw + ((x / step) | 0)] = 1;
    }
    owner = new Int32Array(gw * gh);
    for (let y = 1; y < gh - 1; y++) for (let x = 1; x < gw - 1; x++) {
      const i = y * gw + x;
      if (ink[i] || ink[i - 1] || ink[i + 1] || ink[i - gw] || ink[i + gw]) owner[i] = 1;
    }
    // Start from one piece and let the flood fill split it into letters and the icon.
    pieces = new Map();
    nextId = 2;
    const all = newPiece(1);
    Object.assign(all, { x0: 0, y0: 0, x1: gw - 1, y1: gh - 1 });
    pieces.set(1, all);
    regroup(all);
    for (const pc of pieces.values()) { pc.v = { x: 0, y: 0 }; pc.va = 0; pc.t = { x: 0, y: 0 }; pc.a = 0; }
    pristine = true; shards = []; trail = []; glue.hidden = true;
    word.classList.add("cut-ready");
    kick();
  };

  // ───── frame loop ─────

  const draw = (now: number) => {
    compose(now, pristine);
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    if (pristine) ctx.drawImage(frame, 0, 0);
    else for (const pc of pieces.values()) {
      const sx = pc.x0 * CELL, sy = pc.y0 * CELL, bw = (pc.x1 - pc.x0 + 1) * CELL, bh = (pc.y1 - pc.y0 + 1) * CELL;
      const dw = Math.ceil(bw * dpr), dh = Math.ceil(bh * dpr);
      if (scratch.width < dw || scratch.height < dh) { scratch.width = Math.max(scratch.width, dw); scratch.height = Math.max(scratch.height, dh); }
      sctx.globalCompositeOperation = "copy";
      sctx.drawImage(frame, sx * dpr, sy * dpr, dw, dh, 0, 0, dw, dh);
      sctx.globalCompositeOperation = "destination-in";
      sctx.imageSmoothingEnabled = true;
      sctx.drawImage(maskOf(pc), 0, 0, dw, dh);
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.translate(pc.c.x + pc.t.x, pc.c.y + pc.t.y);
      ctx.rotate(pc.a);
      ctx.translate(-pc.c.x, -pc.c.y);
      if (pc.glow) { ctx.shadowColor = "rgba(77,255,155,.3)"; ctx.shadowBlur = 24 * dpr; }
      ctx.drawImage(scratch, 0, 0, dw, dh, sx, sy, bw, bh);
      ctx.shadowBlur = 0;
    }
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    // The scissors' fresh path glows briefly.
    trail = trail.filter((s) => now - s.at < 260);
    ctx.lineCap = "round";
    for (const s of trail) {
      const k = 1 - (now - s.at) / 260;
      ctx.strokeStyle = `rgba(160,255,200,${0.7 * k})`;
      ctx.lineWidth = 1.2;
      ctx.beginPath(); ctx.moveTo(s.a.x, s.a.y); ctx.lineTo(s.b.x, s.b.y); ctx.stroke();
    }
    shards = shards.filter((s) => now - s.at < 900);
    for (const s of shards) {
      ctx.fillStyle = `rgba(77,255,155,${0.6 * (1 - (now - s.at) / 900)})`;
      ctx.fillRect(s.p.x - 1, s.p.y - 1, CELL, CELL);
    }
  };

  let prev = performance.now();
  const tick = (now: number) => {
    const dt = Math.min(0.05, (now - prev) / 1000);
    prev = now;
    let moving = false;
    if (gluing) {
      // Everything eases back to where it was cut from, then the mark is whole again.
      const k = Math.min(1, (now - gluing) / 600);
      for (const pc of pieces.values()) { pc.t = { x: pc.t.x * (1 - k), y: pc.t.y * (1 - k) }; pc.a *= 1 - k; }
      if (k === 1) { gluing = 0; void build(); }
      moving = true;
    }
    for (const pc of pieces.values()) {
      if (!pc.v.x && !pc.v.y && !pc.va) continue;
      pc.t.x += pc.v.x * dt; pc.t.y += pc.v.y * dt; pc.a += pc.va * dt;
      const damp = Math.exp(-dt * 8);
      pc.v.x *= damp; pc.v.y *= damp; pc.va *= damp;
      if (Math.hypot(pc.v.x, pc.v.y) < 0.5 && Math.abs(pc.va) < 0.002) { pc.v = { x: 0, y: 0 }; pc.va = 0; }
      else moving = true;
    }
    for (const s of shards) { s.v.y += 500 * dt; s.p.x += s.v.x * dt; s.p.y += s.v.y * dt; }
    draw(now);
    running = visible && (moving || trail.length > 0 || shards.length > 0 || !still.matches);
    if (running) requestAnimationFrame(tick);
  };
  const kick = () => {
    if (running || !visible) return;
    running = true;
    prev = performance.now();
    requestAnimationFrame(tick);
  };

  // ───── input ─────

  // Walk the pointer's path a cell at a time so fast strokes cut a continuous line.
  const cutAlong = (from: Pt, to: Pt) => {
    const touched = new Set<Piece>();
    const steps = Math.max(1, Math.ceil(Math.hypot(to.x - from.x, to.y - from.y) / (CELL * 0.75)));
    for (let i = 1; i <= steps; i++) carve({ x: from.x + ((to.x - from.x) * i) / steps, y: from.y + ((to.y - from.y) * i) / steps }, touched);
    if (!touched.size) return;
    pristine = false;
    glue.hidden = false;
    for (const pc of touched) if (pieces.has(pc.id)) regroup(pc);
    if (!still.matches) trail.push({ a: from, b: to, at: performance.now() });
    // The cursor snaps open and shut while it cuts.
    const now = performance.now();
    if (now - chomp > 90) { chomp = now; canvas.classList.toggle("snap"); }
  };
  const local = (e: PointerEvent): Pt => { const r = canvas.getBoundingClientRect(); return { x: e.clientX - r.left, y: e.clientY - r.top }; };
  canvas.addEventListener("pointermove", (e) => {
    if (gluing) return;
    const p = local(e);
    // Coalesced events keep a quick flick's curve instead of a few straight chords.
    const events = e.getCoalescedEvents?.() ?? [];
    for (const ev of events.length ? events : [e]) {
      const q = ev === e ? p : local(ev);
      if (last) cutAlong(last, q);
      last = q;
    }
    kick();
  });
  canvas.addEventListener("pointerleave", () => { last = null; canvas.classList.remove("snap"); });
  glue.addEventListener("click", () => {
    for (const pc of pieces.values()) { pc.v = { x: 0, y: 0 }; pc.va = 0; }
    gluing = performance.now();
    kick();
  });

  word.append(canvas);
  // Outside the aria-hidden mark, so it stays reachable by keyboard.
  word.parentElement!.querySelector(".base")?.prepend(glue);
  new IntersectionObserver(([e]) => { visible = e.isIntersecting; kick(); }).observe(word);
  let lastWidth = innerWidth;
  addEventListener("resize", () => { if (innerWidth !== lastWidth) { lastWidth = innerWidth; void build(); } });
  still.addEventListener("change", kick);
  void build();
}

export {};
