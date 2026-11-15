// The changelog, from the repository's CHANGELOG.md (bundled as text at build time): a modal
// with every release, and the newest release's changes in the Download section.
import source from "../../CHANGELOG.md" with { type: "text" };

type Release = { version: string; date: string; groups: { title: string; items: string[] }[] };

// "## 1.3.2 — 2026-11-11", then "### Added" lists. Anything before the first release is a preface.
const parse = (md: string): Release[] => {
  const releases: Release[] = [];
  for (const line of md.split("\n")) {
    const release = /^## (\S+)(?:\s+—\s+(\S+))?/.exec(line);
    const group = /^### (.+)/.exec(line);
    const item = /^- (.+)/.exec(line);
    const current = releases.at(-1);
    if (release) releases.push({ version: release[1], date: release[2] ?? "", groups: [] });
    else if (group && current) current.groups.push({ title: group[1].trim(), items: [] });
    else if (item && current?.groups.length) current.groups.at(-1)!.items.push(item[1].trim());
  }
  return releases.filter((r) => r.version.toLowerCase() !== "unreleased" && r.groups.length);
};

const escape = (s: string) => s.replace(/[&<>"]/g, (c) => `&${{ "&": "amp", "<": "lt", ">": "gt", '"': "quot" }[c]};`);
// Plain text with `code`; nothing else from Markdown is needed (or allowed: no links out).
const inline = (s: string) => escape(s).replace(/`([^`]+)`/g, "<code>$1</code>");
const when = (iso: string) => {
  const d = new Date(`${iso}T12:00:00`);
  return Number.isNaN(+d) ? iso : d.toLocaleDateString("en", { year: "numeric", month: "short", day: "numeric" });
};
const groupsHtml = (r: Release) => r.groups.map((g) => `
  <section class="log-group" data-kind="${escape(g.title.toLowerCase())}">
    <h4>${escape(g.title)}</h4>
    <ul>${g.items.map((i) => `<li>${inline(i)}</li>`).join("")}</ul>
  </section>`).join("");

const releases = parse(source);
const latest = releases[0];

if (latest) {
  // The hero pill names the release and scrolls to it.
  const pill = document.querySelector<HTMLElement>("[data-release-text]");
  if (pill) pill.textContent = `Chyk ${latest.version} is out`;

  // What's new, above the downloads.
  const card = document.getElementById("whatsnew");
  if (card) {
    card.querySelector("[data-latest-version]")!.textContent = latest.version;
    const time = card.querySelector<HTMLTimeElement>("[data-latest-date]")!;
    time.dateTime = latest.date;
    time.textContent = when(latest.date);
    card.querySelector("[data-latest-notes]")!.innerHTML = groupsHtml(latest);
    card.hidden = false;
  }

  // The modal: a timeline of every release.
  const dialog = document.getElementById("changelog") as HTMLDialogElement;
  dialog.querySelector("[data-changelog-list]")!.innerHTML = releases.map((r, i) => `
    <article class="log-release">
      <div class="log-meta">
        <span class="log-version">${escape(r.version)}</span>
        ${i === 0 ? `<span class="log-latest">Latest</span>` : ""}
        <time datetime="${escape(r.date)}">${escape(when(r.date))}</time>
      </div>
      <div class="log-groups">${groupsHtml(r)}</div>
    </article>`).join("");

  const open = () => {
    if (dialog.open) return;
    dialog.showModal();
    dialog.querySelector(".modal-body")!.scrollTop = 0;
    if (location.hash !== "#changelog") history.pushState(null, "", "#changelog");
  };
  const close = () => dialog.open && dialog.close();
  dialog.addEventListener("close", () => {
    if (location.hash === "#changelog") history.pushState(null, "", location.pathname + location.search);
  });
  for (const el of document.querySelectorAll("[data-changelog]")) {
    el.addEventListener("click", (e) => { e.preventDefault(); open(); });
  }
  dialog.querySelector("[data-close]")!.addEventListener("click", close);
  // A click on the backdrop (the dialog itself, outside the card) closes it.
  dialog.addEventListener("click", (e) => { if (e.target === dialog) close(); });
  addEventListener("popstate", () => (location.hash === "#changelog" ? open() : close()));
  if (location.hash === "#changelog") open();
}

// Copy buttons (the Arch install command).
for (const btn of document.querySelectorAll<HTMLButtonElement>("[data-copy]")) {
  btn.addEventListener("click", async () => {
    try {
      await navigator.clipboard.writeText(btn.dataset.copy!);
      btn.classList.add("copied");
      setTimeout(() => btn.classList.remove("copied"), 1600);
    } catch {
      // No clipboard access: the command stays visible to copy by hand.
    }
  });
}

export {};
