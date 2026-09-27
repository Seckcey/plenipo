// The release's `latest.json` (Phase 13, ADR-037): the newest version, its notes, and where its
// installer and updater signature are, in the layout of Tauri's updater. The Release workflow
// writes it next to the installer; Plenipo reads it from
// https://github.com/Seckcey/plenipo/releases/latest/download/latest.json.
//
// Usage: node scripts/update-manifest.mjs <version> <installer> <signature file> <notes file> <out>
import { readFileSync, writeFileSync } from "node:fs";
import { basename } from "node:path";

export const REPOSITORY = "Seckcey/plenipo";
export const PLATFORM = "windows-x86_64";

/** The release notes without their first line (the title, which GitHub shows already). */
export function notesBody(markdown) {
  return markdown.split(/\r?\n/).slice(1).join("\n").trim();
}

/**
 * The manifest for `version`, whose installer is `installerName` with updater signature
 * `signature` (the `.sig` file's text).
 */
export function manifest({ version, installerName, signature, notes, pubDate }) {
  if (!/^\d+\.\d+\.\d+$/.test(version)) {
    throw new Error(`not a release version: ${version}`);
  }
  if (!installerName.endsWith("-setup.exe") || installerName.includes("/")) {
    throw new Error(`not an installer name: ${installerName}`);
  }
  const sig = signature.trim();
  if (!sig || /\s/.test(sig)) throw new Error("the updater signature is empty or broken");
  return {
    version,
    notes,
    pub_date: pubDate,
    platforms: {
      [PLATFORM]: {
        signature: sig,
        url: `https://github.com/${REPOSITORY}/releases/download/v${version}/${installerName}`,
      },
    },
  };
}

if (basename(process.argv[1] ?? "") === "update-manifest.mjs") {
  const [version, installer, sigFile, notesFile, out] = process.argv.slice(2);
  if (!out) {
    console.error(
      "usage: node scripts/update-manifest.mjs <version> <installer> <signature file> <notes file> <out>",
    );
    process.exit(2);
  }
  const doc = manifest({
    version,
    installerName: basename(installer),
    signature: readFileSync(sigFile, "utf8"),
    notes: notesBody(readFileSync(notesFile, "utf8")),
    pubDate: new Date().toISOString(),
  });
  writeFileSync(out, JSON.stringify(doc, null, 2) + "\n");
  console.log(`Wrote ${out} for ${version}`);
}
