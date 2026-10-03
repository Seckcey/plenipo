// The release's `latest.json` (Phase 13, ADR-038): the newest version, its notes, and, for each
// system, where its download and updater signature are, in the layout of Tauri's updater. The
// Release workflow writes it next to the installer; Plenipo reads it from
// https://github.com/Seckcey/plenipo/releases/latest/download/latest.json.
//
// Phase 23 (ADR-152): Windows' installer, and Linux's AppImage, which replaces itself. A Linux
// `.deb` copy reads the same entry and offers the download on GitHub instead.
//
// Usage: node scripts/update-manifest.mjs <version> <notes file> <out>
//          <system> <download> <signature file> [<system> <download> <signature file> ...]
// where <system> is one of the names in DOWNLOADS (e.g. windows-x86_64, linux-x86_64).
import { readFileSync, writeFileSync } from "node:fs";
import { basename } from "node:path";

export const REPOSITORY = "Seckcey/plenipo";

/** Each system a release updates, and what its download is called. */
export const DOWNLOADS = {
  "windows-x86_64": { what: "installer", ending: "-setup.exe" },
  "linux-x86_64": { what: "AppImage", ending: ".AppImage" },
};

/** The release notes without their first line (the title, which GitHub shows already). */
export function notesBody(markdown) {
  return markdown.split(/\r?\n/).slice(1).join("\n").trim();
}

/**
 * The manifest for `version`: for each of `downloads` (`{ system, fileName, signature }`, the
 * signature being the `.sig` file's text), where it is on GitHub Releases and its signature.
 */
export function manifest({ version, notes, pubDate, downloads }) {
  if (!/^\d+\.\d+\.\d+$/.test(version)) {
    throw new Error(`not a release version: ${version}`);
  }
  if (!Array.isArray(downloads) || downloads.length === 0) {
    throw new Error("a release has at least one download");
  }
  const platforms = {};
  for (const { system, fileName, signature } of downloads) {
    const kind = Object.hasOwn(DOWNLOADS, system) ? DOWNLOADS[system] : undefined;
    if (!kind) throw new Error(`not a system Plenipo updates: ${system}`);
    if (Object.hasOwn(platforms, system)) throw new Error(`${system} is named twice`);
    if (
      !fileName.endsWith(kind.ending) ||
      fileName.includes("/") ||
      fileName.includes("\\") ||
      !fileName.includes(version)
    ) {
      throw new Error(`not ${system}'s ${kind.what} for ${version}: ${fileName}`);
    }
    const sig = signature.trim();
    if (!sig || /\s/.test(sig)) {
      throw new Error(`${system}'s updater signature is empty or broken`);
    }
    platforms[system] = {
      signature: sig,
      url: `https://github.com/${REPOSITORY}/releases/download/v${version}/${encodeURIComponent(fileName)}`,
    };
  }
  return { version, notes, pub_date: pubDate, platforms };
}

if (basename(process.argv[1] ?? "") === "update-manifest.mjs") {
  const [version, notesFile, out, ...rest] = process.argv.slice(2);
  if (!out || rest.length === 0 || rest.length % 3 !== 0) {
    console.error(
      "usage: node scripts/update-manifest.mjs <version> <notes file> <out> <system> <download> <signature file> [...]",
    );
    process.exit(2);
  }
  const downloads = [];
  for (let i = 0; i < rest.length; i += 3) {
    downloads.push({
      system: rest[i],
      fileName: basename(rest[i + 1]),
      signature: readFileSync(rest[i + 2], "utf8"),
    });
  }
  const doc = manifest({
    version,
    notes: notesBody(readFileSync(notesFile, "utf8")),
    pubDate: new Date().toISOString(),
    downloads,
  });
  writeFileSync(out, JSON.stringify(doc, null, 2) + "\n");
  console.log(`Wrote ${out} for ${version}: ${Object.keys(doc.platforms).join(", ")}`);
}
