// Translation table: parsing .lang files, rows, issue badges, saving.
const LANGS = ["en", "uk"];
const FORMS = ["one", "few", "many", "other"];
const SCREENS = ["start", "editor", "strip", "metadata", "export"];
const SCREEN_NAMES = { start: "Start", editor: "Crop", strip: "Strip", metadata: "Info", export: "Export" };
// Sample values for placeholders in the preview.
const SAMPLE = { key: "⌘W", n: 2, time: "0:05.720", pct: "62", size: "48.2 MB", speed: "2x", view: "1.0x · 0:00.000 – 0:42.000", path: "rooftop_take3.mov", error: "…" };

const state = { saved: { en: {}, uk: {} }, cur: { en: {}, uk: {} }, rows: [], screen: "start", lang: "uk", filter: "all", query: "", onScreen: {} };
const $ = (s, el = document) => el.querySelector(s);

function parse(text) {
  const values = {}, order = [], sections = {};
  let lastComment = null, blankBefore = true;
  for (const raw of text.split("\n")) {
    const line = raw.trim();
    if (!line) { blankBefore = true; lastComment = null; continue; }
    if (line.startsWith("#")) { lastComment = blankBefore ? line.replace(/^#\s*/, "") : null; blankBefore = false; continue; }
    const i = line.indexOf("=");
    if (i < 0) continue;
    const key = line.slice(0, i).trim();
    if (lastComment) { sections[key] = lastComment; lastComment = null; }
    values[key] = line.slice(i + 1).trim();
    order.push(key);
    blankBefore = false;
  }
  return { values, order, sections };
}

const baseOf = k => { const m = k.match(/^(.*)\.(one|few|many|other)$/); return m ? m[1] : k; };
const isPlural = k => baseOf(k) !== k;
const holders = v => new Set([...(v || "").matchAll(/\{(\w+)\}/g)].map(m => m[1]));
const fill = v => v.replace(/\{(\w+)\}/g, (m, n) => n in SAMPLE ? SAMPLE[n] : m);

function buildRows(en, uk) {
  const rows = [], done = new Set();
  const add = (key, section) => { if (!done.has(key)) { done.add(key); rows.push({ key, section }); } };
  for (const key of en.order) {
    const section = en.sections[key];
    if (isPlural(key)) {
      const base = baseOf(key);
      FORMS.map(f => `${base}.${f}`).filter(k => k in en.values || k in uk.values).forEach((k, i) => add(k, i === 0 ? section : undefined));
    } else add(key, section);
  }
  uk.order.filter(k => !done.has(k)).forEach((k, i) => add(k, i === 0 ? "Only in Ukrainian" : undefined));
  return rows;
}

function issues(key) {
  const en = state.cur.en[key], uk = state.cur.uk[key];
  const out = [];
  if (!uk) out.push(["missing", "Missing"]);
  else if (en && uk === en && /[a-z]{3}/i.test(en) && !/^ffmpeg/.test(en)) out.push(["same", "Same as English"]);
  // Placeholders must match per base key, across plural forms.
  const base = baseOf(key);
  const union = lang => { const s = new Set(); for (const k in state.cur[lang]) if (baseOf(k) === base) holders(state.cur[lang][k]).forEach(h => h !== "n" && s.add(h)); return [...s].sort().join(); };
  if (uk && union("en") !== union("uk")) out.push(["ph", "Placeholders differ"]);
  return out;
}

function autosize(t) { t.style.height = "auto"; t.style.height = t.scrollHeight + 2 + "px"; }

function render() {
  const box = $("#rows");
  box.textContent = "";
  const q = state.query.toLowerCase();
  let shown = 0, flagged = 0;
  for (const { key, section } of state.rows) {
    const probs = issues(key);
    if (probs.length) flagged++;
    const text = `${key} ${state.cur.en[key] || ""} ${state.cur.uk[key] || ""}`.toLowerCase();
    if (q && !text.includes(q)) continue;
    if (state.filter === "issues" && !probs.length) continue;
    if (section && !q && state.filter === "all") {
      const h = document.createElement("div");
      h.className = "section"; h.textContent = section;
      box.append(h);
    }
    const row = document.createElement("div");
    row.className = "row"; row.dataset.key = key;
    const k = document.createElement("div");
    k.className = "key"; k.textContent = key;
    const tags = document.createElement("div");
    tags.className = "tags";
    for (const [cls, label] of probs) {
      const b = document.createElement("span");
      b.className = `badge ${cls}`; b.textContent = label;
      tags.append(b);
    }
    for (const s of SCREENS) if (state.onScreen[s]?.has(key)) {
      const b = document.createElement("button");
      b.innerHTML = `<i class="ph ph-eye"></i>${SCREEN_NAMES[s]}`;
      b.onclick = () => { showScreen(s); highlight(key); };
      tags.append(b);
    }
    k.append(tags);
    row.append(k);
    for (const lang of LANGS) {
      const t = document.createElement("textarea");
      t.rows = 1; t.dataset.lang = lang; t.spellcheck = true; t.lang = lang;
      const has = key in state.cur[lang] || lang === "uk" || !isPlural(key);
      t.disabled = !has;
      t.value = state.cur[lang][key] ?? "";
      if (!has) t.placeholder = "— (Ukrainian-only form)";
      t.classList.toggle("changed", (state.cur[lang][key] ?? "") !== (state.saved[lang][key] ?? ""));
      t.oninput = () => {
        state.cur[lang][key] = t.value.replace(/\s*\n\s*/g, " ");
        t.classList.toggle("changed", state.cur[lang][key] !== (state.saved[lang][key] ?? ""));
        autosize(t); markDirty(); applyPreview(); refreshTags(row, key);
      };
      t.onfocus = () => { if (lang !== state.lang) setLang(lang); highlight(key); };
      t.onblur = () => highlight(null);
      row.append(t);
    }
    row.onmouseenter = () => highlight(key);
    row.onmouseleave = () => highlight(null);
    box.append(row);
    shown++;
  }
  if (!shown) { const e = document.createElement("p"); e.className = "empty"; e.textContent = "Nothing matches."; box.append(e); }
  box.querySelectorAll("textarea").forEach(autosize);
  $("#count").innerHTML = flagged ? `<b>${flagged}</b> need attention` : "All translated";
}

function refreshTags(row, key) {
  row.querySelectorAll(".badge").forEach(b => b.remove());
  const tags = row.querySelector(".tags");
  for (const [cls, label] of issues(key).reverse()) {
    const b = document.createElement("span");
    b.className = `badge ${cls}`; b.textContent = label;
    tags.prepend(b);
  }
  let flagged = state.rows.filter(r => issues(r.key).length).length;
  $("#count").innerHTML = flagged ? `<b>${flagged}</b> need attention` : "All translated";
}

function dirty() {
  return LANGS.some(l => { const a = state.cur[l], b = state.saved[l]; return Object.keys({ ...a, ...b }).some(k => (a[k] ?? "") !== (b[k] ?? "")); });
}
function markDirty() {
  const d = dirty();
  $("#save").disabled = !d;
  $("#state").textContent = d ? "Unsaved changes" : "Saved";
  $("#state").classList.toggle("dirty", d);
}

async function save() {
  if (!dirty()) return;
  const body = {};
  for (const l of LANGS) body[l] = state.cur[l];
  const res = await fetch("/api/save", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(body) });
  if (!res.ok) { $("#state").textContent = "Save failed: " + ((await res.json()).error || res.status); return; }
  state.saved = JSON.parse(JSON.stringify(state.cur));
  markDirty();
  document.querySelectorAll("textarea.changed").forEach(t => t.classList.remove("changed"));
}
