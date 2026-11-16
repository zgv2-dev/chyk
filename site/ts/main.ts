// Page behaviour: nav, spotlight borders, reveals and the editor tilt.
import "./download.ts";
import "./track.ts";
import "./rain.ts";
import "./snip/index.ts";
import "./changelog.ts";

const still = matchMedia("(prefers-reduced-motion: reduce)");
const $$ = (sel: string, root: ParentNode = document) => [...root.querySelectorAll<HTMLElement>(sel)];

// The nav turns glassy once the page scrolls under it.
const nav = document.querySelector<HTMLElement>(".nav")!;
const onScroll = () => nav.classList.toggle("scrolled", scrollY > 8);
addEventListener("scroll", onScroll, { passive: true });
onScroll();

// Spotlight: every card in a group gets the pointer position relative to itself, so the
// border light around the cursor also catches the edges of neighbouring cards.
const track = (area: HTMLElement, cards: HTMLElement[]) => {
  let frame = 0;
  let point: { x: number; y: number } | null = null;
  const paint = () => {
    frame = 0;
    for (const card of cards) {
      const r = card.getBoundingClientRect();
      card.style.setProperty("--mx", point ? `${point.x - r.left}px` : "-999px");
      card.style.setProperty("--my", point ? `${point.y - r.top}px` : "-999px");
    }
  };
  const schedule = () => { frame ||= requestAnimationFrame(paint); };
  area.addEventListener("pointermove", (e) => {
    if (e.pointerType !== "mouse") return;
    point = { x: e.clientX, y: e.clientY };
    schedule();
  });
  area.addEventListener("pointerleave", () => { point = null; schedule(); });
};
for (const group of $$("[data-spot-group]")) track(group, $$(".spot", group));
for (const lone of $$(".spot")) if (!lone.closest("[data-spot-group]")) track(lone, [lone]);

// Key caps press in a wave, left to right.
$$(".viz-keys kbd").forEach((k, i) => k.style.setProperty("--i", String(i)));

// Sections rise in as they enter the view; the size bars grow when the terminal does.
const reveal = new IntersectionObserver((entries) => {
  for (const { target, isIntersecting } of entries) {
    if (!isIntersecting) continue;
    target.classList.add("in");
    reveal.unobserve(target);
  }
}, { rootMargin: "0px 0px -12% 0px" });
for (const el of $$(".sect-head, .bento, .details, .downloads, .term")) {
  if (!still.matches && !el.matches(".term")) el.classList.add("reveal");
  reveal.observe(el);
}

// The editor shot starts tilted back (class set in <head> to avoid a flash) and settles flat.
const tilt = document.querySelector<HTMLElement>("[data-tilt]");
if (tilt) setTimeout(() => tilt.classList.remove("pre"), 250);
