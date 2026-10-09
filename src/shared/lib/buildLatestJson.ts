/**
 * Pure helper for Tauri updater `latest.json` (lean channel only).
 * Kept in `src/` so Vitest/tsc can cover it; `scripts/write-latest-json.mjs` mirrors the URL rules.
 */
export function buildLatestJson(opts: {
  version: string;
  signature: string;
  repo?: string;
  notes?: string;
  pubDate?: string;
  assetName?: string;
}): {
  version: string;
  notes: string;
  pub_date: string;
  platforms: {
    "windows-x86_64": { signature: string; url: string };
  };
} {
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
