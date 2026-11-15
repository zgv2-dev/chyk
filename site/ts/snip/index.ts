// The footer wordmark can be cut with scissors. The cut follows the pointer exactly from the
// moment it touches the mark, so any shape can be cut out. Parts still joined to the bottom
// of a letter stay put; anything cut loose falls, lands on the footer line or on other pieces,
// and stacks. "Put it back together" flies every piece home and heals the cuts.
// Without JavaScript the CSS mark shows instead.

import { Mark } from "./mark.ts";
import { CELL, Paper, type Piece, type Pt } from "./paper.ts";
import { Physics } from "./physics.ts";
import { drawScissors } from "./scissors.ts";

type Trail = { a: Pt; b: Pt; at: number };
type Glue = { start: number; from: Map<number, { t: Pt; a: number; delay: number }>; seams: HTMLCanvasElement };

const PAD = 40; // room around the mark for glow, pieces and the scissors
const FLY_MS = 560, HEAL_MS = 380;
const still = matchMedia("(prefers-reduced-motion: reduce)");
const easeOut = (k: number) => 1 - (1 - k) ** 3;

const word = document.querySelector<HTMLElement>(".word");
if (word) {
  const canvas = document.createElement("canvas");
  canvas.className = "snip";
  canvas.setAttribute("aria-hidden", "true");
  const ctx = canvas.getContext("2d")!;
  const scratch = document.createElement("canvas"); // one piece at a time, masked
  const sctx = scratch.getContext("2d")!;
  const glueBtn = document.createElement("button");
  glueBtn.className = "glue";
  glueBtn.type = "button";
  glueBtn.textContent = "Put it back together";
  glueBtn.hidden = true;

  const mark = new Mark(word, PAD);
  const paper = new Paper();
  const physics = new Physics(paper);
  let pristine = true, trail: Trail[] = [], glue: Glue | null = null;
  let visible = false, running = false;
  // The scissors: where they are, which way they face, and when they last cut.
  let pointer: Pt | null = null, last: Pt | null = null, angle = -Math.PI / 4, aim = angle, lastCut = -1e9;

  const build = async () => {
    await mark.build();
    const { w, h, dpr } = mark;
    canvas.width = w * dpr; canvas.height = h * dpr;
    canvas.style.cssText = `width:${w}px;height:${h}px;inset:${-PAD}px`;
    mark.compose(0, false);
    const f = mark.frame;
    paper.setup(f.getContext("2d")!.getImageData(0, 0, f.width, f.height).data, f.width, f.height, dpr, w, h, mark.icon);
    physics.floor = mark.bottom;
    physics.dirty = true;
    pristine = true; trail = []; glue = null; glueBtn.hidden = true;
    word.classList.add("cut-ready");
    kick();
  };

  // ───── drawing ─────

  const drawPiece = (pc: Piece, alpha = 1) => {
    const { dpr } = mark;
    const sx = pc.x0 * CELL, sy = pc.y0 * CELL, bw = (pc.x1 - pc.x0 + 1) * CELL, bh = (pc.y1 - pc.y0 + 1) * CELL;
    const dw = Math.ceil(bw * dpr), dh = Math.ceil(bh * dpr);
    if (scratch.width < dw || scratch.height < dh) { scratch.width = Math.max(scratch.width, dw); scratch.height = Math.max(scratch.height, dh); }
    sctx.globalCompositeOperation = "copy";
    sctx.drawImage(mark.frame, sx * dpr, sy * dpr, dw, dh, 0, 0, dw, dh);
    sctx.globalCompositeOperation = "destination-in";
    sctx.drawImage(paper.maskOf(pc), 0, 0, dw, dh);
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.translate(pc.c.x + pc.t.x, pc.c.y + pc.t.y);
    ctx.rotate(pc.a);
    ctx.translate(-pc.c.x, -pc.c.y);
    ctx.globalAlpha = alpha;
    if (pc.glow) { ctx.shadowColor = "rgba(77,255,155,.3)"; ctx.shadowBlur = 24 * dpr; }
    ctx.drawImage(scratch, 0, 0, dw, dh, sx, sy, bw, bh);
    ctx.shadowBlur = 0;
    ctx.globalAlpha = 1;
  };

  const draw = (now: number) => {
    const { dpr, w, h } = mark;
    mark.compose(now, pristine);
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    if (pristine) ctx.drawImage(mark.frame, 0, 0);
    else {
      // Still pieces first, so falling ones pass in front.
      const order = [...paper.pieces.values()].sort((p, q) => +(!p.anchored && !p.resting) - +(!q.anchored && !q.resting));
      for (const pc of order) drawPiece(pc);
    }
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    if (glue) {
      // Healing: the whole mark fades in over the pieces while the seams glow and fade.
      const k = Math.min(1, Math.max(0, (now - glue.start - FLY_MS) / HEAL_MS));
      if (k > 0) {
        mark.compose(now, true);
        ctx.globalAlpha = easeOut(k);
        ctx.setTransform(1, 0, 0, 1, 0, 0);
        ctx.drawImage(mark.frame, 0, 0);
        ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      }
      const fly = Math.min(1, (now - glue.start) / FLY_MS);
      ctx.globalAlpha = Math.sin(Math.PI * Math.min(1, fly * 0.6 + k * 0.4)) * 0.9;
      ctx.imageSmoothingEnabled = true;
      ctx.shadowColor = "rgba(77,255,155,.9)";
      ctx.shadowBlur = 12;
      ctx.drawImage(glue.seams, 0, 0, glue.seams.width, glue.seams.height, 0, 0, glue.seams.width * CELL, glue.seams.height * CELL);
      ctx.shadowBlur = 0;
      ctx.globalAlpha = 1;
    }
    // The scissors' fresh path glows briefly.
    trail = trail.filter((s) => now - s.at < 260);
    ctx.lineCap = "round";
    for (const s of trail) {
      ctx.strokeStyle = `rgba(160,255,200,${0.7 * (1 - (now - s.at) / 260)})`;
      ctx.lineWidth = 1.2;
      ctx.beginPath(); ctx.moveTo(s.a.x, s.a.y); ctx.lineTo(s.b.x, s.b.y); ctx.stroke();
    }
    paper.shards = paper.shards.filter((s) => now - s.at < 900);
    for (const s of paper.shards) {
      ctx.fillStyle = `rgba(77,255,155,${0.6 * (1 - (now - s.at) / 900)})`;
      ctx.fillRect(s.p.x - 1, s.p.y - 1, CELL, CELL);
    }
    if (pointer && pointer.x > 0 && pointer.y > 0 && pointer.x < w && pointer.y < h) {
      // Open when idle, snapping shut and open while cutting.
      const cutting = now - lastCut < 140;
      const open = cutting && !still.matches ? 0.5 + 0.5 * Math.sin(now / 26) : 0.85;
      drawScissors(ctx, pointer.x, pointer.y, angle, open);
    }
  };

  // ───── frame loop ─────

  let prev = performance.now();
  const tick = (now: number) => {
    const dt = Math.min(0.033, (now - prev) / 1000);
    prev = now;
    let busy = false;
    // The scissors turn smoothly toward the way the pointer is going.
    const turn = Math.atan2(Math.sin(aim - angle), Math.cos(aim - angle));
    angle += turn * Math.min(1, dt * 16);
    if (Math.abs(turn) > 0.01) busy = true;
    if (glue) {
      busy = true;
      for (const pc of paper.pieces.values()) {
        const f = glue.from.get(pc.id);
        if (!f) continue;
        const k = easeOut(Math.min(1, Math.max(0, (now - glue.start - f.delay) / (FLY_MS - 160))));
        pc.t = { x: f.t.x * (1 - k), y: f.t.y * (1 - k) };
        pc.a = f.a * (1 - k);
      }
      if (now - glue.start > FLY_MS + HEAL_MS) {
        paper.reset();
        physics.dirty = true;
        pristine = true; glue = null; glueBtn.hidden = true;
      }
    } else if (!pristine) {
      if (physics.dirty) physics.rebuild();
      for (const pc of paper.pieces.values()) {
        if (pc.anchored || pc.resting) continue;
        busy = true;
        physics.step(pc, dt);
      }
      if (physics.dirty) physics.rebuild();
    }
    for (const s of paper.shards) { s.v.y += 500 * dt; s.p.x += s.v.x * dt; s.p.y += s.v.y * dt; }
    draw(now);
    running = visible && (busy || trail.length > 0 || paper.shards.length > 0 || now - lastCut < 200 || !still.matches);
    if (running) requestAnimationFrame(tick);
  };
  const kick = () => {
    if (running || !visible) return;
    running = true;
    prev = performance.now();
    requestAnimationFrame(tick);
  };

  // ───── input ─────

  // Walk the pointer's path half a cell at a time so fast strokes cut a continuous line.
  const cutAlong = (from: Pt, to: Pt) => {
    const touched = new Set<Piece>();
    const len = Math.hypot(to.x - from.x, to.y - from.y);
    const steps = Math.max(1, Math.ceil(len / (CELL * 0.5)));
    for (let i = 1; i <= steps; i++) paper.carve({ x: from.x + ((to.x - from.x) * i) / steps, y: from.y + ((to.y - from.y) * i) / steps }, touched);
    if (!touched.size) return;
    pristine = false;
    glueBtn.hidden = false;
    lastCut = performance.now();
    for (const pc of touched) {
      if (!paper.pieces.has(pc.id)) continue;
      paper.regroup(pc);
      if (pc.anchored || pc.resting) physics.dirty = true;
      pc.resting = false;
    }
    if (!still.matches) trail.push({ a: from, b: to, at: lastCut });
  };
  const local = (e: PointerEvent): Pt => { const r = canvas.getBoundingClientRect(); return { x: e.clientX - r.left, y: e.clientY - r.top }; };
  canvas.addEventListener("pointermove", (e) => {
    const p = local(e);
    pointer = e.pointerType === "mouse" ? p : null;
    if (glue) { last = p; kick(); return; }
    const events = e.getCoalescedEvents?.() ?? [];
    for (const ev of events.length ? events : [e]) {
      const q = ev === e ? p : local(ev);
      if (last) {
        const dx = q.x - last.x, dy = q.y - last.y;
        if (dx * dx + dy * dy > 2) aim = Math.atan2(dy, dx);
        cutAlong(last, q);
      }
      last = q;
    }
    kick();
  });
  canvas.addEventListener("pointerleave", () => { last = null; pointer = null; kick(); });
  glueBtn.addEventListener("click", () => {
    if (glue || pristine) return;
    const from = new Map<number, { t: Pt; a: number; delay: number }>();
    for (const pc of paper.pieces.values()) {
      pc.v = { x: 0, y: 0 }; pc.va = 0;
      // Far-flung pieces set off a little later, so they arrive together in a wave.
      from.set(pc.id, { t: { ...pc.t }, a: pc.a, delay: Math.min(160, Math.hypot(pc.t.x, pc.t.y) * 0.8) });
    }
    glue = { start: performance.now(), from, seams: paper.seams() };
    kick();
  });

  word.append(canvas);
  // Outside the aria-hidden mark, so it stays reachable by keyboard.
  word.parentElement!.querySelector(".base")?.prepend(glueBtn);
  new IntersectionObserver(([e]) => { visible = e.isIntersecting; kick(); }).observe(word);
  let lastWidth = innerWidth;
  addEventListener("resize", () => { if (innerWidth !== lastWidth) { lastWidth = innerWidth; void build(); } });
  still.addEventListener("change", kick);
  void build();
}

export {};
