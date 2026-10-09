#!/usr/bin/env node
/**
 * Build `latest.json` for Tauri updater from the lean NSIS installer + `.sig`.
 * Updater channel is lean-only (offline installers stay manual).
 *
 * URL / payload rules must stay in sync with `src/shared/lib/buildLatestJson.ts`.
 */
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = resolve(__dirname, "..");

function buildLatestJson(opts) {
  const version = opts.version;
  const repo = opts.repo || "leoshcn/meetphant";
  const assetName = opts.assetName || `Meetphant_${version}_x64-setup.exe`;
  const tag = version.startsWith("v") ? version : `v${version}`;
  const url = `https://github.com/${repo}/releases/download/${tag}/${assetName}`;
  return {
    version,
    notes: opts.notes ?? "",
    pub_date: opts.pubDate || new Date().toISOString(),
    platforms: {
      "windows-x86_64": {
        signature: opts.signature.trim(),
        url,
      },
    },
  };
}

function main() {
  const pkg = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
  const version = pkg.version || "0.0.0";
  const outDir = join(root, "dist-installers");
  const exeName = `Meetphant_${version}_x64-setup.exe`;
  const sigPath = join(outDir, `${exeName}.sig`);
  const exePath = join(outDir, exeName);

  if (!existsSync(exePath)) {
    throw new Error(`Lean installer missing: ${exePath}`);
  }
  if (!existsSync(sigPath)) {
    throw new Error(
      `Lean signature missing: ${sigPath}\n` +
        "Set TAURI_SIGNING_PRIVATE_KEY (or _PATH) before `tauri build` / pack.",
    );
  }

  const signature = readFileSync(sigPath, "utf8");
  const latest = buildLatestJson({ version, signature });
  const outPath = join(outDir, "latest.json");
  writeFileSync(outPath, `${JSON.stringify(latest, null, 2)}\n`, "utf8");
  console.log(`Wrote ${outPath}`);
  console.log(`  version=${latest.version}`);
  console.log(`  url=${latest.platforms["windows-x86_64"].url}`);
}

try {
  main();
} catch (err) {
  console.error(err.message || err);
  process.exit(1);
}
