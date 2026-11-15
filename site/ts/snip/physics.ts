// Gravity for cut pieces. Pieces that are anchored or have come to rest are solid: they are
// stamped onto a grid of canvas cells, and a falling piece collides with that grid (and with
// the floor) through the centres of its border cells. It bounces a little, slides to a stop,
// tips over when its weight hangs past what it stands on, and then rests, so the next piece
// can land on top of it.

import { CELL, type Paper, type Piece, type Pt, toScreen } from "./paper.ts";

const GRAVITY = 1400; // px/s²
const MAX_SPIN = 5; // rad/s
const SETTLE_FRAMES = 150; // touching this long without calming down: rest anyway

export class Physics {
  solid = new Int32Array(0); // canvas cell → id of the solid piece there, or 0
  dirty = true;
  floor = 0;

  constructor(private paper: Paper) {}

  // Stamp every anchored or resting piece, then let anything that lost its support fall.
  rebuild() {
    const { gw, gh, owner } = this.paper;
    if (this.solid.length !== gw * gh) this.solid = new Int32Array(gw * gh);
    const solid = this.solid;
    for (let round = 0; round < 6 && this.dirty; round++) {
      this.dirty = false;
      solid.fill(0);
      // Anchored pieces first, so resting ones never cover them.
      const still = [...this.paper.pieces.values()].filter((p) => p.anchored || p.resting).sort((p, q) => +p.resting - +q.resting);
      for (const pc of still) {
        const flat = !pc.a && !pc.t.x && !pc.t.y;
        for (let y = pc.y0; y <= pc.y1; y++) for (let x = pc.x0; x <= pc.x1; x++) {
          const i = y * gw + x;
          if (owner[i] !== pc.id) continue;
          if (flat) { solid[i] = pc.id; continue; }
          const s = toScreen(pc, { x: (x + 0.5) * CELL, y: (y + 0.5) * CELL });
          const sx = Math.round(s.x / CELL - 0.5), sy = Math.round(s.y / CELL - 0.5);
          if (sx < 0 || sy < 0 || sx >= gw - 1 || sy >= gh - 1) continue;
          // Stamp a 2×2 block so a turned piece leaves no gaps to fall through, but only into
          // empty cells: overwriting a neighbour would hide the very piece holding this one up.
          for (const j of [sy * gw + sx, sy * gw + sx + 1, (sy + 1) * gw + sx, (sy + 1) * gw + sx + 1]) solid[j] ||= pc.id;
        }
      }
      // Bottom up: a resting piece with nothing under it starts falling again.
      const resting = [...this.paper.pieces.values()].filter((p) => p.resting).sort((p, q) => q.c.y + q.t.y - (p.c.y + p.t.y));
      for (const pc of resting) {
        if (pc.wedged) continue;
        pc.t.y += 1.5;
        const held = this.contacts(pc).length > 0;
        pc.t.y -= 1.5;
        if (!held) { pc.resting = false; pc.still = 0; this.dirty = true; }
      }
    }
  }

  // Border points of a piece that sit inside something solid or below the floor.
  private contacts(pc: Piece): Pt[] {
    const { gw, gh } = this.paper, solid = this.solid;
    const hits: Pt[] = [];
    let ghostSeen = false;
    for (const e of pc.edge) {
      const s = toScreen(pc, e);
      if (s.y >= this.floor) { hits.push(s); continue; }
      const cx = (s.x / CELL) | 0, cy = (s.y / CELL) | 0;
      if (cx < 0 || cy < 0 || cx >= gw || cy >= gh) continue;
      const id = solid[cy * gw + cx];
      if (!id || id === pc.id) continue;
      if (id === pc.ghost) { ghostSeen = true; continue; }
      hits.push(s);
    }
    // Clear of the piece it was cut from: from now on it lands on that one too.
    if (pc.ghost && !ghostSeen) pc.ghost = 0;
    return hits;
  }

  // Advance a loose piece by dt. Returns true once it has come to rest.
  step(pc: Piece, dt: number): boolean {
    const r = Math.sqrt(pc.count) * CELL;
    const travel = Math.hypot(pc.v.x, pc.v.y + GRAVITY * dt) * dt + Math.abs(pc.va) * r * dt;
    const n = Math.min(14, Math.max(1, Math.ceil(travel / (CELL * 0.8))));
    const sdt = dt / n;
    let touching = false;
    for (let k = 0; k < n; k++) {
      pc.v.y += GRAVITY * sdt;
      const before = { t: { ...pc.t }, a: pc.a };
      pc.t.x += pc.v.x * sdt; pc.t.y += pc.v.y * sdt; pc.a += pc.va * sdt;
      let hits = this.contacts(pc);
      if (!hits.length) { pc.va *= Math.exp(-sdt * 1.5); continue; }
      touching = true;
      // Only contacts below the centre of mass hold it up; side and top contacts don't.
      const comY = pc.c.y + pc.t.y;
      const support = hits.filter((s) => s.y > comY);
      // Lift it out of whatever it sank into; if that fails, undo the step and stop turning
      // (it's wedged against something).
      for (let lift = 0; lift < 10 && hits.length; lift++) { pc.t.y -= 0.5; hits = this.contacts(pc); }
      if (hits.length) { pc.t = before.t; pc.a = before.a; pc.v.x *= -0.3; pc.va = 0; }
      if (pc.v.y > 0) pc.v.y = pc.v.y > 160 ? -pc.v.y * 0.25 : 0;
      pc.v.x *= Math.exp(-sdt * 9);
      // Tip over when the centre of mass hangs outside what it stands on, settle when it doesn't.
      const com = pc.c.x + pc.t.x;
      let lo = Infinity, hi = -Infinity;
      for (const s of support) { if (s.x < lo) lo = s.x; if (s.x > hi) hi = s.x; }
      const over = !support.length ? 0 : com < lo ? com - lo : com > hi ? com - hi : 0;
      if (over) pc.va += (Math.max(-12, Math.min(12, over)) * 14 * sdt) / Math.max(16, r);
      pc.va *= Math.exp(-sdt * (over ? 3 : 14));
      pc.va = Math.max(-MAX_SPIN, Math.min(MAX_SPIN, pc.va));
    }
    // Standing on something: touching now, or within a pixel of it.
    if (!touching) { pc.t.y += 1; touching = this.contacts(pc).length > 0; pc.t.y -= 1; }
    const calm = touching && Math.abs(pc.v.y) < 40 && Math.abs(pc.v.x) < 5 && Math.abs(pc.va) < 0.04;
    pc.still = calm ? pc.still + 1 : 0;
    pc.touch = touching ? pc.touch + 1 : 0;
    if (pc.still < 8 && pc.touch < SETTLE_FRAMES) return false;
    // Jammed against things for seconds: it stays where it is until it's cut again.
    pc.wedged = pc.still < 8;
    pc.v = { x: 0, y: 0 }; pc.va = 0; pc.resting = true;
    this.dirty = true;
    return true;
  }
}
