// The footer wordmark can be cut. Moving the pointer through a piece slices it along the
// path the pointer took, and every piece can be cut again, down to shards a few pixels big.
// The mark is redrawn on a canvas (with the same gradient, scanlines and light sweep as the
// CSS version) so thousands of pieces stay cheap. Without JavaScript the CSS mark shows.

type Pt = { x: number; y: number };
type Piece = {
  poly: Pt[]; // convex polygon, in source (uncut mark) coordinates
  c: Pt; // its centroid, the pivot for rotation
  t: Pt; v: Pt; // offset and velocity
  a: number; va: number; // angle and angular velocity
  inside: boolean; entry: Pt | null; // where the pointer came in, while it is inside
};
type Flash = { a: Pt; b: Pt; at: number };

const PAD = 40; // room around the mark for glow and pieces drifting apart
const MIN_AREA = 6; // px², the smallest shard that can still be cut
const MAX_PIECES = 6000;
const SWEEP_MS = 7000;

const word = document.querySelector<HTMLElement>(".word");
const still = matchMedia("(prefers-reduced-motion: reduce)");

const sub = (p: Pt, q: Pt): Pt => ({ x: p.x - q.x, y: p.y - q.y });
const cross = (o: Pt, a: Pt, b: Pt) => (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x);
const rot = (p: Pt, a: number): Pt => ({ x: p.x * Math.cos(a) - p.y * Math.sin(a), y: p.x * Math.sin(a) + p.y * Math.cos(a) });

const area = (poly: Pt[]) => Math.abs(poly.reduce((s, p, i) => s + cross({ x: 0, y: 0 }, p, poly[(i + 1) % poly.length]), 0)) / 2;
const centroid = (poly: Pt[]): Pt => {
  const n = poly.length;
  return poly.reduce((s, p) => ({ x: s.x + p.x / n, y: s.y + p.y / n }), { x: 0, y: 0 });
};
const contains = (poly: Pt[], p: Pt) => {
  let sign = 0;
  for (let i = 0; i < poly.length; i++) {
    const s = Math.sign(cross(poly[i], poly[(i + 1) % poly.length], p));
    if (s && sign && s !== sign) return false;
    sign ||= s;
  }
  return true;
};
// Both halves of a convex polygon on either side of the line through a and b.
const split = (poly: Pt[], a: Pt, b: Pt): [Pt[], Pt[]] => {
  const left: Pt[] = [], right: Pt[] = [];
  poly.forEach((p, i) => {
    const q = poly[(i + 1) % poly.length];
    const sp = cross(a, b, p), sq = cross(a, b, q);
    (sp >= 0 ? left : right).push(p);
    if (sp * sq < 0) {
      const k = sp / (sp - sq);
      const x = { x: p.x + (q.x - p.x) * k, y: p.y + (q.y - p.y) * k };
      left.push(x); right.push(x);
    }
  });
  return [left, right];
};

// Screen-to-piece and back: p_screen = rot(p - c, a) + c + t.
const toLocal = (pc: Piece, q: Pt): Pt => {
  const r = rot(sub(q, { x: pc.c.x + pc.t.x, y: pc.c.y + pc.t.y }), -pc.a);
  return { x: r.x + pc.c.x, y: r.y + pc.c.y };
};
const make = (poly: Pt[], from?: Piece): Piece => {
  const c = centroid(poly);
  // Keep the parent's placement: solve for the offset that maps the new pivot the same way.
  const t = from ? (() => { const d = rot(sub(c, from.c), from.a); return { x: d.x + from.c.x - c.x + from.t.x, y: d.y + from.c.y - c.y + from.t.y }; })() : { x: 0, y: 0 };
  return { poly, c, t, v: { x: 0, y: 0 }, a: from?.a ?? 0, va: 0, inside: false, entry: null };
};

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
  const k = (((ms - 1000) % SWEEP_MS) + SWEEP_MS) % SWEEP_MS / SWEEP_MS;
  return k >= 0.45 ? 1.3 : -0.3 + 1.6 * ease(k / 0.45);
};

if (word) {
  const img = word.querySelector("img")!;
  const span = word.querySelector("span")!;
  const canvas = document.createElement("canvas");
  canvas.className = "snip";
  canvas.setAttribute("aria-hidden", "true");
  const ctx = canvas.getContext("2d")!;
  const base = document.createElement("canvas"); // text with its gradient and scanlines
  const frame = document.createElement("canvas"); // base plus this frame's sweep and the icon
  const glue = document.createElement("button");
  glue.className = "glue";
  glue.type = "button";
  glue.textContent = "Put it back together";
  glue.hidden = true;

  let pieces: Piece[] = [], flashes: Flash[] = [];
  let w = 0, h = 0, dpr = 1, textBox = { x: 0, y: 0, w: 0, h: 0 }, imgBox = { x: 0, y: 0, w: 0, h: 0 };
  let last: Pt | null = null, visible = false, running = false, gluing = 0;

  const build = async () => {
    const cs = getComputedStyle(span);
    const size = parseFloat(cs.fontSize);
    await document.fonts.load(`700 ${size}px SpaceMono`);
    await img.decode().catch(() => {});
    const wr = word.getBoundingClientRect(), sr = span.getBoundingClientRect(), ir = img.getBoundingClientRect();
    dpr = Math.min(devicePixelRatio || 1, 2);
    w = wr.width + PAD * 2; h = wr.height + PAD * 2;
    for (const c of [canvas, base, frame]) { c.width = w * dpr; c.height = h * dpr; }
    canvas.style.cssText = `width:${w}px;height:${h}px;inset:${-PAD}px`;
    textBox = { x: sr.left - wr.left + PAD, y: sr.top - wr.top + PAD, w: sr.width, h: sr.height };
    imgBox = { x: ir.left - wr.left + PAD, y: ir.top - wr.top + PAD, w: ir.width, h: ir.height };

    // The text: dark green gradient, then scanlines, clipped to the glyphs.
    const b = base.getContext("2d")!;
    b.setTransform(dpr, 0, 0, dpr, 0, 0);
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

    const rect = { x: Math.min(textBox.x, imgBox.x) - 24, y: Math.min(textBox.y, imgBox.y) - 24 };
    const right = Math.max(textBox.x + textBox.w, imgBox.x + imgBox.w) + 24, bottom = Math.max(textBox.y + textBox.h, imgBox.y + imgBox.h) + 24;
    pieces = [make([{ x: rect.x, y: rect.y }, { x: right, y: rect.y }, { x: right, y: bottom }, { x: rect.x, y: bottom }])];
    flashes = [];
    glue.hidden = true;
    word.classList.add("cut-ready");
    kick();
  };

  // This frame's source image: the text with the light sweep across it, then the icon.
  const compose = (now: number) => {
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
    f.shadowColor = "rgba(77,255,155,.35)";
    f.shadowBlur = 30;
    f.drawImage(img, imgBox.x, imgBox.y, imgBox.w, imgBox.h);
    f.globalAlpha = 1;
    f.shadowBlur = 0;
  };

  const draw = (now: number) => {
    compose(now);
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    const whole = pieces.length === 1 && !pieces[0].a && !pieces[0].t.x && !pieces[0].t.y;
    if (whole) ctx.drawImage(frame, 0, 0);
    else for (const pc of pieces) {
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.translate(pc.c.x + pc.t.x, pc.c.y + pc.t.y);
      ctx.rotate(pc.a);
      ctx.translate(-pc.c.x, -pc.c.y);
      ctx.beginPath();
      pc.poly.forEach((p, i) => (i ? ctx.lineTo(p.x, p.y) : ctx.moveTo(p.x, p.y)));
      ctx.closePath();
      ctx.save();
      ctx.clip();
      ctx.drawImage(frame, 0, 0, w, h);
      ctx.restore();
    }
    // A bright line along each fresh cut, fading out.
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    flashes = flashes.filter((fl) => now - fl.at < 350);
    for (const fl of flashes) {
      const k = 1 - (now - fl.at) / 350;
      ctx.strokeStyle = `rgba(217,255,233,${0.85 * k})`;
      ctx.lineWidth = 1.5;
      ctx.shadowColor = "rgba(77,255,155,.9)";
      ctx.shadowBlur = 10 * k;
      ctx.beginPath(); ctx.moveTo(fl.a.x, fl.a.y); ctx.lineTo(fl.b.x, fl.b.y); ctx.stroke();
    }
    ctx.shadowBlur = 0;
  };

  let prev = performance.now();
  const tick = (now: number) => {
    const dt = Math.min(0.05, (now - prev) / 1000);
    prev = now;
    let moving = false;
    if (gluing) {
      // Everything eases back to where it was cut from, then becomes one piece again.
      const k = Math.min(1, (now - gluing) / 600), e = 1 - (1 - k) ** 3;
      for (const pc of pieces) { pc.t = { x: pc.t.x * (1 - e), y: pc.t.y * (1 - e) }; pc.a *= 1 - e; }
      if (k === 1) { gluing = 0; pieces = [make(pieces.length ? hull() : [])]; glue.hidden = true; }
      moving = true;
    }
    for (const pc of pieces) {
      if (!pc.v.x && !pc.v.y && !pc.va) continue;
      pc.t.x += pc.v.x * dt; pc.t.y += pc.v.y * dt; pc.a += pc.va * dt;
      const damp = Math.exp(-dt * 9);
      pc.v.x *= damp; pc.v.y *= damp; pc.va *= damp;
      if (Math.hypot(pc.v.x, pc.v.y) < 0.5 && Math.abs(pc.va) < 0.002) { pc.v = { x: 0, y: 0 }; pc.va = 0; }
      else moving = true;
    }
    draw(now);
    running = visible && (moving || flashes.length > 0 || !still.matches);
    if (running) requestAnimationFrame(tick);
  };
  const kick = () => {
    if (running || !visible) return;
    running = true;
    prev = performance.now();
    requestAnimationFrame(tick);
  };
  // The original outline, for gluing: the bounding box of every piece in source coordinates.
  const hull = (): Pt[] => {
    const xs = pieces.flatMap((p) => p.poly.map((q) => q.x)), ys = pieces.flatMap((p) => p.poly.map((q) => q.y));
    const [x0, x1, y0, y1] = [Math.min(...xs), Math.max(...xs), Math.min(...ys), Math.max(...ys)];
    return [{ x: x0, y: y0 }, { x: x1, y: y0 }, { x: x1, y: y1 }, { x: x0, y: y1 }];
  };

  const cut = (pc: Piece, a: Pt, b: Pt) => {
    if (pieces.length >= MAX_PIECES) return;
    const [l, r] = split(pc.poly, a, b);
    if (l.length < 3 || r.length < 3 || area(l) < MIN_AREA || area(r) < MIN_AREA) return;
    const halves = [make(l, pc), make(r, pc)];
    // Push the halves apart along the cut's normal, with a little spin.
    const d = sub(b, a), n0 = Math.hypot(d.x, d.y), n = rot({ x: -d.y / n0, y: d.x / n0 }, pc.a);
    halves.forEach((hv, i) => {
      const s = i ? -1 : 1, kick = 26 + Math.random() * 20;
      hv.v = { x: pc.v.x + n.x * s * kick, y: pc.v.y + n.y * s * kick };
      // Big pieces barely turn; small shards spin more freely.
      hv.va = pc.va + (Math.random() - 0.5) * 0.3 * Math.min(1, 60 / Math.sqrt(area(hv.poly)));
    });
    pieces.splice(pieces.indexOf(pc), 1, ...halves);
    const toScreen = (p: Pt) => { const q = rot(sub(p, pc.c), pc.a); return { x: q.x + pc.c.x + pc.t.x, y: q.y + pc.c.y + pc.t.y }; };
    if (!still.matches) flashes.push({ a: toScreen(a), b: toScreen(b), at: performance.now() });
    glue.hidden = false;
    canvas.classList.add("snap");
    setTimeout(() => canvas.classList.remove("snap"), 110);
  };

  // Walk the pointer's path in small steps so fast strokes still cut every piece they cross.
  const trace = (from: Pt, to: Pt) => {
    const steps = Math.max(1, Math.ceil(Math.hypot(to.x - from.x, to.y - from.y) / 3));
    for (let i = 1; i <= steps; i++) {
      const q = { x: from.x + ((to.x - from.x) * i) / steps, y: from.y + ((to.y - from.y) * i) / steps };
      for (const pc of [...pieces]) {
        const p = toLocal(pc, q);
        const inside = contains(pc.poly, p);
        if (inside && !pc.inside) pc.entry = p;
        if (!inside && pc.inside && pc.entry && Math.hypot(p.x - pc.entry.x, p.y - pc.entry.y) > 4) cut(pc, pc.entry, p);
        pc.inside = inside;
        if (!inside) pc.entry = null;
      }
    }
  };
  const local = (e: PointerEvent): Pt => { const r = canvas.getBoundingClientRect(); return { x: e.clientX - r.left, y: e.clientY - r.top }; };
  canvas.addEventListener("pointermove", (e) => {
    if (gluing) return;
    const p = local(e);
    if (last) trace(last, p);
    last = p;
    kick();
  });
  canvas.addEventListener("pointerleave", () => {
    last = null;
    for (const pc of pieces) { pc.inside = false; pc.entry = null; }
  });
  glue.addEventListener("click", () => {
    for (const pc of pieces) { pc.v = { x: 0, y: 0 }; pc.va = 0; }
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
