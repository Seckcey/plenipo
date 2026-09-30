import { mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { escapeHtml, renderDocument } from "./release-notes.mjs";

const policies = [
  {
    slug: "terms",
    title: "Terms of service",
    description: "Terms for using Plenipo's website, software, AI tools, and support.",
  },
  {
    slug: "privacy",
    title: "Privacy statement",
    description:
      "How Plenipo handles local records, AI requests, website visits, and support information.",
  },
];

// The same Markdown is readable on GitHub and rendered here. Keeping it inside
// apps/website also makes it available to the standalone Docker build context.
export async function buildLegalPages(websiteRoot, output, stylesheet) {
  for (const policy of policies) {
    const markdown = await readFile(resolve(websiteRoot, "legal", `${policy.slug}.md`), "utf8");
    const source = `https://github.com/Seckcey/plenipo/blob/main/apps/website/legal/${policy.slug}.md`;
    const body = renderDocument(markdown, source);
    const navigation = policies
      .map(
        ({ slug, title }) =>
          `<a href="/${slug}/"${slug === policy.slug ? ' aria-current="page"' : ""}>${title}</a>`,
      )
      .join("\n");
    const html = `<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>${escapeHtml(policy.title)} | Plenipo</title>
    <meta name="description" content="${escapeHtml(policy.description)}" />
    <link rel="canonical" href="https://plenipo.8westit.com/${policy.slug}/" />
    <link rel="icon" href="/brand/pip-favicon-on-light.svg" type="image/svg+xml" />
    <link rel="apple-touch-icon" href="/brand/pip-icon-180.png" />
    <link rel="stylesheet" href="${escapeHtml(stylesheet)}" />
  </head>
  <body>
    <a class="skip-link" href="#main">Skip to content</a>
    <header class="site-header legal-header">
      <div class="header-inner">
        <a class="brand" href="/" aria-label="Plenipo home">
          <img src="/brand/plenipo-horizontal-on-light.webp" width="600" height="239" alt="Plenipo" />
        </a>
        <nav class="legal-navigation" aria-label="Main navigation">
          <a href="/">Home</a>
          ${navigation}
        </nav>
      </div>
    </header>
    <main id="main" class="legal-document">
      <p class="legal-kicker">Plenipo · 8 West Ventures, LLC</p>
      <article aria-label="${escapeHtml(policy.title)}">
        ${body}
      </article>
      <p class="legal-source"><a href="${source}">Read this statement on GitHub</a></p>
    </main>
    <footer class="site-footer container legal-footer">
      <div class="footer-brand"><p>© 2026 8 West Ventures, LLC.</p></div>
      <nav aria-label="Footer navigation">
        ${navigation}
        <a href="https://github.com/Seckcey/plenipo">GitHub</a>
        <a href="mailto:admin@8westventures.com">Contact</a>
      </nav>
    </footer>
  </body>
</html>
`;
    await mkdir(resolve(output, policy.slug), { recursive: true });
    await writeFile(resolve(output, policy.slug, "index.html"), html);
  }
}
