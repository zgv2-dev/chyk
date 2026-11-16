// Count download clicks in GoatCounter (no cookies). Links still go straight to GitHub, so a
// blocked or slow counter never gets in the way. Open the site with #toggle-goatcounter to
// stop counting your own browser.
type GoatCounter = { count?: (vars: { path: string; title?: string; event?: boolean }) => void };
declare global {
  interface Window { goatcounter?: GoatCounter }
}

const ASSET = /\/releases\/(?:latest\/download|download\/[^/]+)\/([^/?#]+)$/;

// Middle-clicks open in a new tab and fire auxclick instead of click.
const track = (e: MouseEvent) => {
  if (e.type === "auxclick" && e.button !== 1) return;
  const a = (e.target as Element | null)?.closest?.<HTMLAnchorElement>("a[href]");
  const file = a?.href.match(ASSET)?.[1];
  if (!file) return;
  window.goatcounter?.count?.({ path: `download/${file}`, title: a!.textContent?.trim(), event: true });
};
document.addEventListener("click", track);
document.addEventListener("auxclick", track);

export {};
