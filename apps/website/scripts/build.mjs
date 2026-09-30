import { createHash } from "node:crypto";
import { cp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { build } from "esbuild";
import { noReleaseNotes, renderReleaseNotes } from "./release-notes.mjs";
import { buildLegalPages } from "./legal.mjs";

export const websiteRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
export const rootPackage = resolve(websiteRoot, "..", "..", "package.json");
// The version the website shows is never typed into the page: its build fills the marks in from
// the repository's root package.json (docs/development/versioning.md), or from PLENIPO_VERSION
// when it is given (a container build has no repository around it; a deploy passes the release
// it checked on GitHub Releases, docs/development/website.md).
const VERSION_MARK = "__PLENIPO_VERSION__";
// Where the "What's new" section goes: the notes of the version the page shows.
const NOTES_MARK = "<!-- __RELEASE_NOTES__ -->";
// A release version as the Release workflow tags it: 1.2.3, or a pre-release such as 1.2.3-beta.1.
// Nothing else goes into the page's links and structured data.
const RELEASE_VERSION = /^\d+\.\d+\.\d+(?:-[0-9A-Za-z]+(?:\.[0-9A-Za-z]+)*)?$/;

export async function releaseVersion({ env = process.env, packageFile = rootPackage } = {}) {
  const given = (env.PLENIPO_VERSION || "").trim();
  if (given) {
    if (!RELEASE_VERSION.test(given)) {
      throw new Error(`PLENIPO_VERSION "${given}" is not a release version`);
    }
    return given;
  }
  let version;
  try {
    version = JSON.parse(await readFile(packageFile, "utf8")).version;
  } catch {
    throw new Error(
      `No version for the website: pass PLENIPO_VERSION (the image's build argument), or build inside the repository (${packageFile} was not found)`,
    );
  }
  if (!RELEASE_VERSION.test(version || "")) {
    throw new Error(`The version in ${packageFile} ("${version}") is not a release version`);
  }
  return version;
}

// The notes for a version, looked for in order: PLENIPO_RELEASE_NOTES (a file a deploy names),
// release-notes/vX.Y.Z.md beside the website (the automatic update puts the release's own notes
// there, since a container build sees only this folder), then the repository's
// docs/releases/vX.Y.Z.md. A named file must exist; otherwise no notes is allowed.
export async function releaseNotes(version, { env = process.env } = {}) {
  const named = (env.PLENIPO_RELEASE_NOTES || "").trim();
  const candidates = named
    ? [resolve(named)]
    : [
        resolve(websiteRoot, "release-notes", `v${version}.md`),
        resolve(websiteRoot, "..", "..", "docs", "releases", `v${version}.md`),
      ];
  for (const file of candidates) {
    try {
      return await readFile(file, "utf8");
    } catch (error) {
      if (named)
        throw new Error(`PLENIPO_RELEASE_NOTES names ${file}, which cannot be read`, {
          cause: error,
        });
    }
  }
  return null;
}

export async function buildWebsite(output = resolve(websiteRoot, "dist"), options = {}) {
  const version = await releaseVersion(options);
  // Only the known website output folder is replaceable. Tests use their own
  // temporary output directory and never delete application or public sources.
  if (output === resolve(websiteRoot, "dist")) await rm(output, { recursive: true, force: true });
  await mkdir(output, { recursive: true });
  await cp(resolve(websiteRoot, "public"), output, { recursive: true });
  let html = await readFile(resolve(websiteRoot, "index.html"), "utf8");
  if (!html.includes(VERSION_MARK)) {
    throw new Error(`index.html has no ${VERSION_MARK} to fill in`);
  }
  html = html.replaceAll(VERSION_MARK, version);
  if (!html.includes(NOTES_MARK)) throw new Error(`index.html has no ${NOTES_MARK} to fill in`);
  const notes = await releaseNotes(version, options);
  html = html.replace(
    NOTES_MARK,
    notes ? renderReleaseNotes(notes, version) : noReleaseNotes(version),
  );
  const bundle = await build({
    entryPoints: [resolve(websiteRoot, "src/mount.tsx")],
    outdir: resolve(output, "demo"),
    entryNames: "demo-[hash]",
    bundle: true,
    minify: true,
    format: "esm",
    target: ["es2022"],
    jsx: "automatic",
    define: { "process.env.NODE_ENV": '"production"' },
    metafile: true,
    logLevel: "silent",
  });
  const bundleFiles = Object.keys(bundle.metafile.outputs);
  const script = bundleFiles.find((file) => file.endsWith(".js"));
  const style = bundleFiles.find((file) => file.endsWith(".css"));
  if (!script || !style) throw new Error("Interactive demo bundle is incomplete");
  html = html
    .replace("__DEMO_SCRIPT__", `/demo/${script.split(/[\\/]/).pop()}`)
    .replace("__DEMO_STYLE__", `/demo/${style.split(/[\\/]/).pop()}`);
  let stylesheet;
  for (const file of ["styles.css", "main.js"]) {
    const content = await readFile(resolve(websiteRoot, file));
    const hash = createHash("sha256").update(content).digest("hex").slice(0, 12);
    await writeFile(resolve(output, file), content);
    html = html.replaceAll(`"/${file}"`, `"/${file}?v=${hash}"`);
    if (file === "styles.css") stylesheet = `/${file}?v=${hash}`;
  }
  await buildLegalPages(websiteRoot, output, stylesheet);
  await writeFile(resolve(output, "index.html"), html);
  const structuredData = JSON.parse(
    html.match(/<script type="application\/ld\+json">([\s\S]*?)<\/script>/)[1],
  );
  await writeFile(
    resolve(output, "release.json"),
    `${JSON.stringify(
      {
        sourceRevision: process.env.SOURCE_REVISION || "development",
        version: structuredData.softwareVersion,
        downloadUrl: structuredData.downloadUrl,
        operatingSystem: structuredData.operatingSystem,
        architecture: structuredData.processorRequirements,
        releaseNotes: Boolean(notes),
      },
      null,
      2,
    )}\n`,
  );
  return output;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const output = await buildWebsite();
  console.log(`Built Plenipo website: ${output}`);
}
