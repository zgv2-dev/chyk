// Point the main button at the visitor's system. Without JavaScript it scrolls to the
// download list, whose links (releases/latest/download/…) always serve the newest release.
const LATEST = "https://github.com/zgv2-dev/chyk/releases/latest/download/";
const MAIN = {
  mac: ["macOS", "Chyk-macos-arm64.dmg"],
  win: ["Windows", "Chyk-windows-x64.exe"],
  linux: ["Linux", "Chyk-linux-x86_64.AppImage"],
};

function detectOs() {
  const ua = navigator.userAgent.toLowerCase();
  if (/iphone|ipad|android/.test(ua)) return null;
  const p = (navigator.userAgentData?.platform || navigator.platform || ua).toLowerCase();
  if (p.includes("mac")) return "mac";
  if (p.includes("win")) return "win";
  if (p.includes("linux")) return "linux";
  return null;
}

const os = detectOs();
if (os) {
  const primary = document.getElementById("primary");
  primary.querySelector("span").textContent = `Download for ${MAIN[os][0]}`;
  primary.href = LATEST + MAIN[os][1];
  // Browsers don't reliably tell Apple silicon from Intel, so Intel gets its own link.
  document.getElementById("intel").hidden = os !== "mac";
  // The button already covers this system: list only the others.
  document.querySelector(`.chips a[href="#${os}"]`).remove();
  document.getElementById("also").textContent = "Free and open source. Also for";
}
