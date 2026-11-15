// The footer wordmark redrawn on a canvas, matching the CSS one: dark green gradient text with
// scanlines, a light sweeping across it every 7 s, and the glowing icon.

export type Box = { x: number; y: number; w: number; h: number };

const SWEEP_MS = 7000;
const still = matchMedia("(prefers-reduced-motion: reduce)");

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

export class Mark {
  readonly base = document.createElement("canvas"); // the text, gradient and scanlines
  readonly frame = document.createElement("canvas"); // base plus this frame's sweep and the icon
  w = 0; h = 0; dpr = 1; bottom = 0;
  text: Box = { x: 0, y: 0, w: 0, h: 0 };
  icon: Box = { x: 0, y: 0, w: 0, h: 0 };

  constructor(private word: HTMLElement, private pad: number) {}

  async build() {
    const span = this.word.querySelector("span")!, img = this.word.querySelector("img")!;
    const cs = getComputedStyle(span);
    const size = parseFloat(cs.fontSize);
    await document.fonts.load(`700 ${size}px SpaceMono`);
    await img.decode().catch(() => {});
    const wr = this.word.getBoundingClientRect(), sr = span.getBoundingClientRect(), ir = img.getBoundingClientRect();
    const pad = this.pad;
    this.dpr = Math.min(devicePixelRatio || 1, 2);
    this.w = Math.ceil(wr.width + pad * 2);
    this.h = Math.ceil(wr.height + pad * 2);
    this.bottom = pad + wr.height;
    for (const c of [this.base, this.frame]) { c.width = this.w * this.dpr; c.height = this.h * this.dpr; }
    this.text = { x: sr.left - wr.left + pad, y: sr.top - wr.top + pad, w: sr.width, h: sr.height };
    this.icon = { x: ir.left - wr.left + pad, y: ir.top - wr.top + pad, w: ir.width, h: ir.height };

    const b = this.base.getContext("2d")!, t = this.text;
    b.setTransform(this.dpr, 0, 0, this.dpr, 0, 0);
    b.font = `700 ${size}px SpaceMono`;
    const m = b.measureText("CHYK");
    const lh = parseFloat(cs.lineHeight) || size * 0.8;
    const baseline = t.y + (lh - m.fontBoundingBoxAscent - m.fontBoundingBoxDescent) / 2 + m.fontBoundingBoxAscent;
    const spacing = parseFloat(cs.letterSpacing) || 0;
    let x = t.x;
    for (const ch of span.textContent ?? "") { b.fillText(ch, x, baseline); x += b.measureText(ch).width + spacing; }
    b.globalCompositeOperation = "source-in";
    const g = b.createLinearGradient(0, t.y, 0, t.y + t.h);
    g.addColorStop(0, "#16502f"); g.addColorStop(1, "#072011");
    b.fillStyle = g;
    b.fillRect(0, 0, this.w, this.h);
    b.globalCompositeOperation = "source-atop";
    b.fillStyle = "rgba(0,0,0,.35)";
    for (let y = t.y + t.h - 2; y > t.y - 5; y -= 5) b.fillRect(t.x, y, t.w, 2);
    b.globalCompositeOperation = "source-over";
  }

  // Paint this frame's mark into `frame`. `glow` adds the icon's halo (left out for pieces).
  compose(now: number, glow = true) {
    const f = this.frame.getContext("2d")!, t = this.text;
    f.setTransform(1, 0, 0, 1, 0, 0);
    f.clearRect(0, 0, this.frame.width, this.frame.height);
    f.drawImage(this.base, 0, 0);
    f.setTransform(this.dpr, 0, 0, this.dpr, 0, 0);
    // 105deg, like the CSS: a band 32% of the gradient line wide, centred on the sweep.
    const ang = (105 * Math.PI) / 180, dir = { x: Math.sin(ang), y: -Math.cos(ang) };
    const len = Math.abs(t.w * dir.x) + Math.abs(t.h * dir.y);
    const at = (k: number) => ({ x: t.x + t.w / 2 + dir.x * len * (k - 0.5), y: t.y + t.h / 2 + dir.y * len * (k - 0.5) });
    const s = sweepAt(now), p0 = at(s - 0.16), p1 = at(s + 0.16);
    const g = f.createLinearGradient(p0.x, p0.y, p1.x, p1.y);
    g.addColorStop(0, "rgba(217,255,233,0)"); g.addColorStop(0.5, "rgba(217,255,233,.9)"); g.addColorStop(1, "rgba(217,255,233,0)");
    f.globalCompositeOperation = "source-atop";
    f.fillStyle = g;
    f.fillRect(t.x, t.y, t.w, t.h);
    f.globalCompositeOperation = "source-over";
    f.globalAlpha = 0.9;
    if (glow) { f.shadowColor = "rgba(77,255,155,.35)"; f.shadowBlur = 30; }
    const img = this.word.querySelector("img")!, i = this.icon;
    f.drawImage(img, i.x, i.y, i.w, i.h);
    f.globalAlpha = 1;
    f.shadowBlur = 0;
  }
}
