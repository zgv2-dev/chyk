// Live mockup preview, and startup. Loaded after table.js, whose state it shares.
// ---- preview
const iframe = $("#iframe");
function fitFrame() {
  const box = $("#frame");
  const scale = Math.min((box.clientWidth - 32) / 1280, (box.clientHeight - 32) / 800);
  iframe.style.transform = `scale(${scale})`;
}
function value(key) { return state.cur[state.lang][key] || state.cur.en[key] || key; }
function applyPreview() {
  const doc = iframe.contentDocument;
  if (!doc) return;
  doc.documentElement.lang = state.lang;
  doc.querySelectorAll("[data-i18n]").forEach(el => { el.textContent = fill(value(el.dataset.i18n)); });
  doc.querySelectorAll("[data-i18n-title]").forEach(el => { el.title = fill(value(el.dataset.i18nTitle)); });
}
function highlight(key) {
  const doc = iframe.contentDocument;
  if (!doc) return;
  doc.querySelectorAll("[data-i18n],[data-i18n-title]").forEach(el => {
    const hit = key && (el.dataset.i18n === key || el.dataset.i18nTitle === key);
    el.style.outline = hit ? "2px solid #ffc857" : "";
    el.style.outlineOffset = hit ? "2px" : "";
  });
}
function showScreen(s) {
  if (state.screen === s && iframe.src) return;
  state.screen = s;
  document.querySelectorAll("#screens button").forEach(b => b.classList.toggle("on", b.dataset.s === s));
  iframe.src = `/phosphor-${s}.html`;
}
function setLang(l) {
  state.lang = l;
  document.querySelectorAll("#langs button").forEach(b => b.classList.toggle("on", b.dataset.l === l));
  applyPreview();
}
iframe.onload = () => {
  applyPreview();
  const doc = iframe.contentDocument;
  doc.addEventListener("click", e => {
    const el = e.target.closest("[data-i18n],[data-i18n-title]");
    if (!el) return;
    e.preventDefault();
    const key = el.dataset.i18n || el.dataset.i18nTitle;
    let row = document.querySelector(`.row[data-key="${CSS.escape(key)}"]`);
    if (!row) { state.query = ""; $("#q").value = ""; state.filter = "all"; syncFilter(); render(); row = document.querySelector(`.row[data-key="${CSS.escape(key)}"]`); }
    row.scrollIntoView({ block: "center", behavior: "smooth" });
    row.classList.remove("flash"); void row.offsetWidth; row.classList.add("flash");
    row.querySelector(`textarea[data-lang="${state.lang}"]`)?.focus();
  });
  doc.querySelectorAll("[data-i18n],[data-i18n-title]").forEach(el => el.style.cursor = "pointer");
};

function syncFilter() { document.querySelectorAll("#filter button").forEach(b => b.classList.toggle("on", b.dataset.f === state.filter)); }

async function init() {
  const files = await (await fetch("/api/strings")).json();
  const en = parse(files.en), uk = parse(files.uk);
  state.saved = { en: { ...en.values }, uk: { ...uk.values } };
  state.cur = { en: { ...en.values }, uk: { ...uk.values } };
  state.rows = buildRows(en, uk);
  // Which keys each mockup screen shows, for the row tags.
  await Promise.all(SCREENS.map(async s => {
    const html = await (await fetch(`/phosphor-${s}.html`)).text();
    state.onScreen[s] = new Set([...html.matchAll(/data-i18n(?:-title)?="([^"]+)"/g)].map(m => m[1]));
  }));
  render();
  showScreen("start");
  fitFrame();
}

$("#q").oninput = e => { state.query = e.target.value; render(); };
document.querySelectorAll("#filter button").forEach(b => b.onclick = () => { state.filter = b.dataset.f; syncFilter(); render(); });
document.querySelectorAll("#screens button").forEach(b => b.onclick = () => showScreen(b.dataset.s));
document.querySelectorAll("#langs button").forEach(b => b.onclick = () => setLang(b.dataset.l));
$("#save").onclick = save;
addEventListener("keydown", e => { if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "s") { e.preventDefault(); save(); } });
addEventListener("beforeunload", e => { if (dirty()) { e.preventDefault(); e.returnValue = ""; } });
addEventListener("resize", fitFrame);
init();
