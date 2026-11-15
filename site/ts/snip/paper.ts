// The mark as paper on a grid of small cells. Each cell belongs to one piece or to none; the
// scissors clear cells, and a flood fill then tells whether a piece came apart.

import type { Box } from "./mark.ts";

export type Pt = { x: number; y: number };
export type Piece = {
  id: number;
  x0: number; y0: number; x1: number; y1: number; // bounding box, in cells, inclusive
  count: number;
  c: Pt; // centroid in source px, the pivot for rotation
  t: Pt; v: Pt; // offset and velocity, px and px/s
  a: number; va: number; // angle and angular velocity
  glow: boolean; // holds part of the icon, which keeps its green glow
  anchored: boolean; // still joined to the bottom of its letter, so it never moves
  resting: boolean; // fell and settled, so others can land on it
  still: number; // frames in a row it has barely moved while touching something
  ghost: number; // a piece cut out of the middle of this one passes through it until clear
  edge: Pt[]; // centres of its border cells, source px, for collisions
  mask: HTMLCanvasElement; dirty: boolean;
};
export type Shard = { p: Pt; v: Pt; at: number };

export const CELL = 2; // px per cell
const KERF = 0.9; // cut radius, in cells

export const rot = (p: Pt, a: number): Pt => ({ x: p.x * Math.cos(a) - p.y * Math.sin(a), y: p.x * Math.sin(a) + p.y * Math.cos(a) });
// Source px of a piece to canvas px, and back.
export const toScreen = (pc: Piece, p: Pt): Pt => {
  const r = rot({ x: p.x - pc.c.x, y: p.y - pc.c.y }, pc.a);
  return { x: r.x + pc.c.x + pc.t.x, y: r.y + pc.c.y + pc.t.y };
};
export const toSource = (pc: Piece, q: Pt): Pt => {
  const r = rot({ x: q.x - pc.c.x - pc.t.x, y: q.y - pc.c.y - pc.t.y }, -pc.a);
  return { x: r.x + pc.c.x, y: r.y + pc.c.y };
};

export class Paper {
  gw = 0; gh = 0;
  paper = new Uint8Array(0); // cells that are paper when the mark is whole
  owner = new Int32Array(0);
  anchor = new Uint8Array(0); // the bottom rows of each letter and of the icon
  pieces = new Map<number, Piece>();
  shards: Shard[] = [];
  icon: Box = { x: 0, y: 0, w: 0, h: 0 };
  private nextId = 1;

  // Paper: every cell with ink in or next to it, so cell edges never clip a glyph.
  setup(alpha: Uint8ClampedArray, pw: number, ph: number, scale: number, w: number, h: number, icon: Box) {
    this.icon = icon;
    this.gw = Math.ceil(w / CELL); this.gh = Math.ceil(h / CELL);
    const { gw, gh } = this;
    const ink = new Uint8Array(gw * gh), step = CELL * scale;
    for (let y = 0; y < ph; y++) for (let x = 0; x < pw; x++) {
      if (alpha[(y * pw + x) * 4 + 3] > 6) ink[((y / step) | 0) * gw + ((x / step) | 0)] = 1;
    }
    this.paper = new Uint8Array(gw * gh);
    for (let y = 1; y < gh - 1; y++) for (let x = 1; x < gw - 1; x++) {
      const i = y * gw + x;
      if (ink[i] || ink[i - 1] || ink[i + 1] || ink[i - gw] || ink[i + gw]) this.paper[i] = 1;
    }
    this.reset();
  }

  // Whole again: one piece per letter and one for the icon, each anchored by its bottom rows.
  reset() {
    const { gw, gh } = this;
    this.owner = new Int32Array(gw * gh);
    this.anchor = new Uint8Array(gw * gh);
    this.paper.forEach((p, i) => { if (p) this.owner[i] = 1; });
    this.pieces = new Map();
    this.shards = [];
    this.nextId = 2;
    const all = this.make(1);
    Object.assign(all, { x0: 0, y0: 0, x1: gw - 1, y1: gh - 1 });
    this.pieces.set(1, all);
    this.regroup(all);
    for (const pc of this.pieces.values()) {
      Object.assign(pc, { v: { x: 0, y: 0 }, va: 0, t: { x: 0, y: 0 }, a: 0, anchored: true, ghost: 0 });
      for (let y = Math.max(pc.y0, pc.y1 - 3); y <= pc.y1; y++) for (let x = pc.x0; x <= pc.x1; x++) {
        if (this.owner[y * gw + x] === pc.id) this.anchor[y * gw + x] = 1;
      }
    }
  }

  private make(id: number, from?: Piece): Piece {
    return {
      id, x0: 0, y0: 0, x1: 0, y1: 0, count: 0, c: { x: 0, y: 0 },
      t: { x: 0, y: 0 }, v: { x: 0, y: 0 }, a: from?.a ?? 0, va: from?.va ?? 0, glow: false,
      anchored: from?.anchored ?? true, resting: false, still: 0, ghost: 0, edge: [],
      mask: document.createElement("canvas"), dirty: true,
    };
  }

  // Recount a piece's cells, box, centroid and border. A new centroid moves the pivot, so the
  // offset is adjusted to keep the piece exactly where it was on screen.
  private measure(pc: Piece, sx0: number, sy0: number, sx1: number, sy1: number, from?: Piece) {
    const { gw, owner, icon } = this;
    let n = 0, mx = 0, my = 0, x0 = this.gw, y0 = this.gh, x1 = -1, y1 = -1, glow = false;
    const edge: Pt[] = [];
    const ix0 = (icon.x / CELL) | 0, iy0 = (icon.y / CELL) | 0, ix1 = ((icon.x + icon.w) / CELL) | 0, iy1 = ((icon.y + icon.h) / CELL) | 0;
    for (let y = sy0; y <= sy1; y++) for (let x = sx0; x <= sx1; x++) {
      const i = y * gw + x;
      if (owner[i] !== pc.id) continue;
      n++; mx += x; my += y;
      if (x < x0) x0 = x; if (x > x1) x1 = x; if (y < y0) y0 = y; if (y > y1) y1 = y;
      glow ||= x >= ix0 && x <= ix1 && y >= iy0 && y <= iy1;
      if (owner[i - 1] !== pc.id || owner[i + 1] !== pc.id || owner[i - gw] !== pc.id || owner[i + gw] !== pc.id) {
        edge.push({ x: (x + 0.5) * CELL, y: (y + 0.5) * CELL });
      }
    }
    const c = n ? { x: (mx / n + 0.5) * CELL, y: (my / n + 0.5) * CELL } : pc.c;
    const ref = from ?? pc;
    const d = rot({ x: c.x - ref.c.x, y: c.y - ref.c.y }, ref.a);
    pc.t = { x: d.x + ref.c.x - c.x + ref.t.x, y: d.y + ref.c.y - c.y + ref.t.y };
    Object.assign(pc, { count: n, c, x0, y0, x1, y1, glow, edge, dirty: true });
  }

  // Flood-fill a piece after a cut. The part holding its anchor (or else the biggest) keeps it;
  // every other part becomes a new piece, and crumbs fall away as shards.
  regroup(pc: Piece): Piece[] {
    const { gw, owner } = this, { x0, y0, x1, y1 } = pc;
    const bw = x1 - x0 + 1, seen = new Uint8Array(bw * (y1 - y0 + 1));
    const parts: number[][] = [], stack: number[] = [];
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
    if (parts.length <= 1) { this.measure(pc, x0, y0, x1, y1); if (!pc.count) this.pieces.delete(pc.id); return []; }
    parts.sort((p, q) => q.length - p.length);
    const held = parts.map((cells) => pc.anchored && cells.some((j) => this.anchor[j]));
    const keep = Math.max(0, held.indexOf(true));
    if (keep) { [parts[0], parts[keep]] = [parts[keep], parts[0]]; [held[0], held[keep]] = [held[keep], held[0]]; }
    const parent = { ...pc, t: { ...pc.t }, c: { ...pc.c } };
    const born: Piece[] = [];
    const now = performance.now();
    parts.forEach((cells, i) => {
      if (i === 0) return;
      if (cells.length < 4) {
        for (const j of cells) {
          owner[j] = 0;
          const p = toScreen(parent, { x: ((j % gw) + 0.5) * CELL, y: (((j / gw) | 0) + 0.5) * CELL });
          this.shards.push({ p, v: { x: (Math.random() - 0.5) * 60, y: -20 - Math.random() * 40 }, at: now });
        }
        return;
      }
      const np = this.make(this.nextId++, parent);
      for (const j of cells) owner[j] = np.id;
      this.measure(np, x0, y0, x1, y1, parent);
      np.anchored = held[i];
      np.resting = false;
      // Cut out of the middle: it drops out through its parent instead of sitting in the hole.
      if (np.x0 > x0 && np.x1 < x1 && np.y0 > y0 && np.y1 < y1) np.ghost = pc.id;
      if (!np.anchored) {
        const away = rot({ x: np.c.x - parent.c.x, y: np.c.y - parent.c.y }, parent.a);
        const n = Math.hypot(away.x, away.y) || 1, speed = 14 + Math.random() * 12;
        np.v = { x: parent.v.x + (away.x / n) * speed, y: parent.v.y + (away.y / n) * speed };
        np.va = parent.va + (Math.random() - 0.5) * 0.5 * Math.min(1, 40 / Math.sqrt(np.count));
      }
      this.pieces.set(np.id, np);
      born.push(np);
    });
    this.measure(pc, x0, y0, x1, y1, parent);
    pc.anchored = held[0];
    pc.resting = false;
    pc.still = 0;
    return born;
  }

  // Clear the cells under the scissors at a canvas point, in every piece lying there.
  carve(q: Pt, touched: Set<Piece>) {
    const { gw, gh, owner } = this;
    for (const pc of this.pieces.values()) {
      const s = toSource(pc, q);
      const lx = s.x / CELL, ly = s.y / CELL;
      if (lx < pc.x0 - 2 || lx > pc.x1 + 2 || ly < pc.y0 - 2 || ly > pc.y1 + 2) continue;
      for (let y = Math.floor(ly - 1); y <= Math.ceil(ly + 1); y++) for (let x = Math.floor(lx - 1); x <= Math.ceil(lx + 1); x++) {
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
  }

  // A piece's mask: its cells, one pixel each, scaled up smoothly when drawn.
  maskOf(pc: Piece) {
    if (!pc.dirty) return pc.mask;
    const { gw, owner } = this;
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
  }

  // Every cut line and crumb: paper that no piece owns. Glows while the mark heals.
  seams(): HTMLCanvasElement {
    const { gw, gh, owner, paper } = this;
    const out = document.createElement("canvas");
    out.width = gw; out.height = gh;
    const m = out.getContext("2d")!;
    const data = m.createImageData(gw, gh);
    for (let i = 0; i < gw * gh; i++) {
      if (!paper[i] || owner[i]) continue;
      data.data.set([190, 255, 215, 255], i * 4);
    }
    m.putImageData(data, 0, 0);
    return out;
  }
}
