// Faint digital rain behind the hero, like the app's start screen (src/app/backdrop.rs).
// Fades out toward the bottom; draws nothing when the visitor prefers reduced motion.
const canvas = document.getElementById("rain");
const ctx = canvas.getContext("2d");
const GLYPHS = "0123456789ABCDEF:.=*+<>/";
const COL = 22, LINE = 19, ALPHA = 0.22;
const still = matchMedia("(prefers-reduced-motion: reduce)");
let streams = [], width = 0, height = 0, running = false;

function stream(top) {
  return {
    x: COL / 2 + Math.floor(Math.random() * Math.max(1, width / COL)) * COL,
    y: top ? Math.random() * height : -Math.random() * height * 0.5,
    speed: 60 + Math.random() * 70,
    len: 8 + Math.floor(Math.random() * 12),
  };
}

function resize() {
  const dpr = devicePixelRatio || 1;
  width = canvas.clientWidth;
  height = canvas.clientHeight;
  canvas.width = width * dpr;
  canvas.height = height * dpr;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.font = "15px SpaceMono, Menlo, monospace";
  ctx.textAlign = "center";
  streams = Array.from({ length: Math.ceil(width / 160) }, () => stream(true));
}

let last = performance.now();
function frame(now) {
  const dt = Math.min(0.1, (now - last) / 1000);
  last = now;
  ctx.clearRect(0, 0, width, height);
  const flicker = Math.floor(now / 160);
  for (const s of streams) {
    s.y += s.speed * dt;
    if (s.y - s.len * LINE > height) Object.assign(s, stream(false));
    for (let k = 0; k < s.len; k++) {
      const y = Math.round((s.y - k * LINE) / LINE) * LINE;
      if (y < 0 || y > height) continue;
      // Full strength at the top, gone by the bottom of the canvas.
      const a = ALPHA * (1 - k / s.len) * (1 - y / height);
      const g = GLYPHS[(s.x * 131 + y * 7 + Math.floor(flicker / (1 + (k % 3)))) % GLYPHS.length];
      ctx.fillStyle = k === 0 ? `rgba(217,255,233,${a})` : `rgba(77,255,155,${a})`;
      ctx.fillText(g, s.x, y);
    }
  }
  running = !still.matches && !document.hidden;
  if (running) requestAnimationFrame(frame);
}

function start() {
  if (still.matches) return ctx.clearRect(0, 0, width, height);
  if (running) return;
  running = true;
  last = performance.now();
  requestAnimationFrame(frame);
}

resize();
addEventListener("resize", resize);
document.addEventListener("visibilitychange", () => document.hidden || start());
still.addEventListener("change", start);
start();
