#!/usr/bin/env node
/**
 * Build Windows NSIS installers: lean (no FFmpeg) and/or offline (bundled FFmpeg).
 * Copies artifacts into dist-installers/ with distinguishable names.
 */
import { existsSync, mkdirSync, readdirSync, copyFileSync, statSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { execFileSync, spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = resolve(__dirname, "..");
const nsisDir = join(root, "src-tauri", "target", "release", "bundle", "nsis");
const outDir = join(root, "dist-installers");
const pkg = JSON.parse(readFileSync(join(root, "package.json"), "utf8"));
const version = pkg.version || "0.0.0";

const mode = (process.argv[2] || "all").toLowerCase();

function run(cmd, args, opts = {}) {
  console.log(`> ${cmd} ${args.join(" ")}`);
  const r = spawnSync(cmd, args, {
    cwd: root,
    stdio: "inherit",
    shell: false,
    ...opts,
  });
  if (r.status !== 0) {
    process.exit(r.status ?? 1);
  }
}

function tauriCliJs() {
  return join(root, "node_modules", "@tauri-apps", "cli", "tauri.js");
}

function runTauri(args) {
  run(process.execPath, [tauriCliJs(), ...args]);
}

function ensureFfmpegStaged() {
  let triple = "x86_64-pc-windows-msvc";
  try {
    triple = execFileSync("rustc", ["--print", "host-tuple"], {
      encoding: "utf8",
    }).trim();
  } catch {
    /* default */
  }
  const staged = join(root, "src-tauri", "binaries", `ffmpeg-${triple}.exe`);
  if (!existsSync(staged)) {
    console.log("FFmpeg binary not staged; running prepare-ffmpeg…");
    run("node", [join("scripts", "prepare-ffmpeg.mjs")]);
  }
  if (!existsSync(staged)) {
    throw new Error(`Expected staged binary missing: ${staged}`);
  }
}

function latestNsisExe() {
  if (!existsSync(nsisDir)) {
    throw new Error(`NSIS output dir missing: ${nsisDir}`);
  }
  const files = readdirSync(nsisDir)
    .filter((f) => f.toLowerCase().endsWith(".exe"))
    .map((f) => {
      const full = join(nsisDir, f);
      return { full, name: f, mtime: statSync(full).mtimeMs };
    })
    .sort((a, b) => b.mtime - a.mtime);
  if (!files.length) {
    throw new Error(`No .exe found in ${nsisDir}`);
  }
  return files[0];
}

function copyArtifact(suffix) {
  mkdirSync(outDir, { recursive: true });
  const src = latestNsisExe();
  const destName =
    suffix === "offline"
      ? `Meetphant_${version}_x64-offline-setup.exe`
      : `Meetphant_${version}_x64-setup.exe`;
  const dest = join(outDir, destName);
  copyFileSync(src.full, dest);
  console.log(`Copied:\n  ${src.full}\n→ ${dest}`);

  const sigSrc = `${src.full}.sig`;
  if (existsSync(sigSrc)) {
    const sigDest = `${dest}.sig`;
    copyFileSync(sigSrc, sigDest);
    console.log(`Copied:\n  ${sigSrc}\n→ ${sigDest}`);
  } else if (suffix === "lean") {
    console.warn(
      `Warning: no signature next to ${src.full}. Set TAURI_SIGNING_PRIVATE_KEY before packing for updater.`,
    );
  }
  return dest;
}

function writeLatestJson() {
  run(process.execPath, [join("scripts", "write-latest-json.mjs")]);
}

function buildLean() {
  console.log("\n=== Building lean (no bundled FFmpeg) ===\n");
  runTauri(["build"]);
  copyArtifact("lean");
  writeLatestJson();
}

function buildOffline() {
  console.log("\n=== Building offline (bundled FFmpeg) ===\n");
  ensureFfmpegStaged();
  runTauri(["build", "--config", "src-tauri/tauri.offline.conf.json"]);
  copyArtifact("offline");
}

function main() {
  if (process.platform !== "win32") {
    console.error("pack-installers currently supports Windows NSIS builds only.");
    process.exit(1);
  }

  if (existsSync(outDir) && mode === "all") {
    /* keep prior artifacts unless rebuilding all */
  }
  mkdirSync(outDir, { recursive: true });

  if (mode === "lean") {
    buildLean();
  } else if (mode === "offline") {
    buildOffline();
  } else if (mode === "all") {
    buildLean();
    buildOffline();
  } else {
    console.error(`Unknown mode: ${mode} (use lean | offline | all)`);
    process.exit(1);
  }

  console.log("\nInstaller(s) ready under dist-installers/");
}

try {
  main();
} catch (err) {
  console.error(err.message || err);
  process.exit(1);
}
