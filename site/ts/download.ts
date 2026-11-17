// Point the download buttons at the visitor's system and mark its card. Without JavaScript
// they scroll to the download list, whose links (releases/latest/download/…) always serve
// the newest release.
const LATEST = "https://github.com/zgv2-dev/chyk/releases/latest/download/";
type Os = "mac" | "win" | "linux";
const MAIN: Record<Os, [label: string, file: string]> = {
  mac: ["macOS", "Chyk-macos-arm64.dmg"],
  win: ["Windows", "Chyk-windows-x64-setup.exe"],
  linux: ["Linux", "Chyk-linux-x86_64.AppImage"],
};

const detectOs = (): Os | null => {
  const ua = navigator.userAgent.toLowerCase();
  if (/iphone|ipad|android/.test(ua)) return null;
  const nav = navigator as Navigator & { userAgentData?: { platform: string } };
  const p = (nav.userAgentData?.platform || navigator.platform || ua).toLowerCase();
  if (p.includes("mac")) return "mac";
  if (p.includes("win")) return "win";
  if (p.includes("linux")) return "linux";
  return null;
};

const os = detectOs();
if (os) {
  const [label, file] = MAIN[os];
  for (const a of document.querySelectorAll<HTMLAnchorElement>("[data-dl]")) a.href = LATEST + file;
  document.querySelector("#primary span")!.textContent = `Download for ${label}`;
  document.getElementById(os)?.classList.add("mine");
}

export {};
