// Turns a release's notes (docs/releases/vX.Y.Z.md) into the website's "What's new" section.
// The notes are our own Markdown, but the page never trusts them as HTML: every character is
// escaped, and only the few shapes release notes use are rendered — headings, paragraphs, lists
// (one level deep), code blocks, **bold**, `code`, and links. A link that is not http(s) after
// resolving against the release on GitHub is shown as plain text. The shared
// document renderer also supports mailto links for legal contact addresses.

const REPOSITORY = "https://github.com/Seckcey/plenipo";

export function escapeHtml(text) {
  return text
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;");
}

function safeUrl(target, base, allowMailto = false) {
  try {
    const url = new URL(target, base);
    return url.protocol === "https:" ||
      url.protocol === "http:" ||
      (allowMailto && url.protocol === "mailto:")
      ? url.href
      : null;
  } catch {
    return null;
  }
}

// `code`, **bold**, and [text](url); everything else is escaped text.
function inline(text, base, allowMailto = false) {
  const pattern = /`([^`]+)`|\*\*(.+?)\*\*|\[([^\]]+)\]\(([^)\s]+)\)/g;
  let html = "";
  let last = 0;
  for (const match of text.matchAll(pattern)) {
    html += escapeHtml(text.slice(last, match.index));
    const [, code, bold, label, target] = match;
    if (code !== undefined) html += `<code>${escapeHtml(code)}</code>`;
    else if (bold !== undefined) html += `<strong>${inline(bold, base, allowMailto)}</strong>`;
    else {
      const url = safeUrl(target, base, allowMailto);
      html += url
        ? `<a href="${escapeHtml(url)}">${inline(label, base, allowMailto)}</a>`
        : inline(label, base, allowMailto);
    }
    last = match.index + match[0].length;
  }
  return html + escapeHtml(text.slice(last));
}

// Splits the notes into blocks: { type: "heading", level, text } | { type: "paragraph", text } |
// { type: "list", ordered, items: [{ text, children: [text] }] } | { type: "code", text }.
function blocks(markdown) {
  const lines = markdown.replaceAll("\r\n", "\n").split("\n");
  const result = [];
  let i = 0;
  const item = /^( *)([-*]|\d+\.) +(.*)$/;
  while (i < lines.length) {
    const line = lines[i];
    if (!line.trim()) {
      i += 1;
    } else if (line.startsWith("```")) {
      const code = [];
      i += 1;
      while (i < lines.length && !lines[i].startsWith("```")) code.push(lines[i++]);
      i += 1;
      result.push({ type: "code", text: code.join("\n") });
    } else if (/^#{1,6} /.test(line)) {
      const [, hashes, text] = line.match(/^(#+) +(.*)$/);
      result.push({ type: "heading", level: hashes.length, text: text.trim() });
      i += 1;
    } else if (item.test(line) && line.match(item)[1].length === 0) {
      const ordered = /\d/.test(line.match(item)[2]);
      const items = [];
      while (i < lines.length && lines[i].trim()) {
        const match = lines[i].match(item);
        if (match && match[1].length === 0) {
          items.push({ text: match[3], children: [] });
        } else if (match && items.length) {
          items.at(-1).children.push(match[3]);
        } else if (items.length) {
          // A wrapped line continues the latest item (or its latest sub-item).
          const current = items.at(-1);
          if (current.children.length)
            current.children[current.children.length - 1] += ` ${lines[i].trim()}`;
          else current.text += ` ${lines[i].trim()}`;
        }
        i += 1;
      }
      result.push({ type: "list", ordered, items });
    } else {
      const text = [];
      while (
        i < lines.length &&
        lines[i].trim() &&
        !/^#{1,6} /.test(lines[i]) &&
        !lines[i].startsWith("```") &&
        !(item.test(lines[i]) && lines[i].match(item)[1].length === 0)
      ) {
        text.push(lines[i++].trim());
      }
      result.push({ type: "paragraph", text: text.join(" ") });
    }
  }
  return result;
}

function render(parts, base, { headingOffset = 2, maxHeading = 5, allowMailto = false } = {}) {
  return parts
    .map((part) => {
      if (part.type === "heading") {
        // The section's own heading is an h2 and the release title an h3, so the notes' "##"
        // sections are h4 and anything deeper h5.
        const level = Math.min(part.level + headingOffset, maxHeading);
        return `<h${level}>${inline(part.text, base, allowMailto)}</h${level}>`;
      }
      if (part.type === "code") return `<pre><code>${escapeHtml(part.text)}</code></pre>`;
      if (part.type === "list") {
        const tag = part.ordered ? "ol" : "ul";
        const items = part.items
          .map((entry) => {
            const children = entry.children.length
              ? `<ul>${entry.children.map((child) => `<li>${inline(child, base, allowMailto)}</li>`).join("")}</ul>`
              : "";
            return `<li>${inline(entry.text, base, allowMailto)}${children}</li>`;
          })
          .join("");
        return `<${tag}>${items}</${tag}>`;
      }
      return `<p>${inline(part.text, base, allowMailto)}</p>`;
    })
    .join("\n");
}

// Complete documents retain their h1/h2 structure. Email links are allowed only
// here; release notes keep their original http(s)-only link policy.
export function renderDocument(markdown, base) {
  return render(blocks(markdown), base, {
    headingOffset: 0,
    maxHeading: 6,
    allowMailto: true,
  });
}

// The notes' first line is "# vX.Y.Z — Title". The intro (everything before the first "##")
// is shown; the rest opens under "Read the full release notes".
export function renderReleaseNotes(markdown, version) {
  const base = `${REPOSITORY}/blob/v${version}/docs/releases/`;
  const parts = blocks(markdown);
  let title = `v${version}`;
  if (parts[0]?.type === "heading" && parts[0].level === 1) title = parts.shift().text;
  // The section's heading already names the version: "v1.2.3 — Title" shows as "Title".
  const prefix = `v${version} — `;
  if (title.startsWith(prefix) && title.length > prefix.length) title = title.slice(prefix.length);
  const split = parts.findIndex((part) => part.type === "heading" && part.level <= 2);
  const intro = split === -1 ? parts : parts.slice(0, split);
  const rest = split === -1 ? [] : parts.slice(split);
  const releaseUrl = `${REPOSITORY}/releases/tag/v${version}`;
  return [
    `<h3 class="release-title">${inline(title, base)}</h3>`,
    `<div class="release-intro">\n${render(intro, base)}\n</div>`,
    rest.length
      ? `<details class="release-more">\n<summary>Read the full release notes</summary>\n<div class="release-body">\n${render(rest, base)}\n</div>\n</details>`
      : "",
    `<a class="release-link" href="${escapeHtml(releaseUrl)}">This release on GitHub <span aria-hidden="true">↗</span></a>`,
  ]
    .filter(Boolean)
    .join("\n");
}

// When a build has no notes for its version (a pre-release build, or a version not written up
// yet), the section says so and points to GitHub instead of showing nothing.
export function noReleaseNotes(version) {
  const releaseUrl = `${REPOSITORY}/releases/tag/v${version}`;
  return [
    `<p class="release-intro">The notes for v${escapeHtml(version)} are on GitHub.</p>`,
    `<a class="release-link" href="${escapeHtml(releaseUrl)}">This release on GitHub <span aria-hidden="true">↗</span></a>`,
  ].join("\n");
}
