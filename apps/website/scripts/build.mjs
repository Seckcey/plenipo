import { createHash } from "node:crypto";
import { cp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { build } from "esbuild";

export const websiteRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");

export async function buildWebsite(output = resolve(websiteRoot, "dist")) {
  // Only the known website output folder is replaceable. Tests use their own
  // temporary output directory and never delete application or public sources.
  if (output === resolve(websiteRoot, "dist")) await rm(output, { recursive: true, force: true });
  await mkdir(output, { recursive: true });
  await cp(resolve(websiteRoot, "public"), output, { recursive: true });
  let html = await readFile(resolve(websiteRoot, "index.html"), "utf8");
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
  for (const file of ["styles.css", "main.js"]) {
    const content = await readFile(resolve(websiteRoot, file));
    const hash = createHash("sha256").update(content).digest("hex").slice(0, 12);
    await writeFile(resolve(output, file), content);
    html = html.replaceAll(`"/${file}"`, `"/${file}?v=${hash}"`);
  }
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
