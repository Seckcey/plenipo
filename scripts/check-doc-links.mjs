#!/usr/bin/env node
// Refuses a link in a Markdown file that points at a file in this repository that is not there
// (a renamed ADR, a moved screenshot). Web links and links within a page (#…) are not checked.
// It runs in `pnpm lint` and in CI's quick Docs check.
import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join, posix } from "node:path";
import { fileURLToPath } from "node:url";

/** The repository paths that each link in `text` (a Markdown file at `file`) points at. */
export function linkedPaths(file, text) {
  const prose = text.replace(/```[\s\S]*?```/g, "").replace(/`[^`\n]*`/g, "");
  const paths = [];
  for (const [, bracketed, plain] of prose.matchAll(
    /\]\((?:<([^>\n]+)>|([^)\s]+))(?:\s+"[^"]*")?\)/g,
  )) {
    const target = bracketed ?? plain;
    if (/^[a-z][a-z0-9+.-]*:/i.test(target) || target.startsWith("#")) continue;
    const raw = target.split("#")[0];
    let path = raw;
    try {
      path = decodeURIComponent(raw);
    } catch {
      // A stray "%": check the link as written.
    }
    if (!path) continue;
    // Repository paths use "/" on every system (git ls-files gives them so), Windows included.
    paths.push(
      path.startsWith("/")
        ? posix.normalize(path.slice(1))
        : posix.normalize(posix.join(posix.dirname(file), path)),
    );
  }
  return paths;
}

/** Every broken link in the repository's Markdown files, as "file: target". */
export function brokenLinks(root) {
  const files = execFileSync("git", ["ls-files", "*.md"], { cwd: root, encoding: "utf8" })
    .split("\n")
    .filter(Boolean);
  const broken = [];
  for (const file of files) {
    const text = readFileSync(join(root, file), "utf8");
    for (const path of linkedPaths(file, text)) {
      if (!existsSync(join(root, path))) broken.push(`${file}: ${path}`);
    }
  }
  return broken;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const root = join(dirname(fileURLToPath(import.meta.url)), "..");
  const broken = brokenLinks(root);
  if (broken.length > 0) {
    console.error(`Links to files that are not there:\n${broken.join("\n")}`);
    process.exit(1);
  }
}
