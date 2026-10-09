#!/usr/bin/env node
/**
 * Render the real Meetphant React UI with mocked Tauri IPC and capture
 * website screenshots (dev-only; see scripts/screenshots/demo-main.ts).
 *
 *   npm run screenshots                 # all scenes, dark theme
 *   npm run screenshots -- --theme=light
 *   npm run screenshots -- --only=workspace,llm
 *
 * Output: website/assets/screenshots/<scene>.png, <scene>-mobile.png (focused
 * crops for phones, for scenes with a `mobile` entry) and website/assets/og.png
 */
import { existsSync, mkdirSync, readFileSync, readdirSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { createServer } from "vite";

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "..", "..");
const outDir = join(root, "website", "assets", "screenshots");
const ogPath = join(root, "website", "assets", "og.png");

const argv = process.argv.slice(2);
const argValue = (name) =>
  argv.find((a) => a.startsWith(`--${name}=`))?.split("=")[1] ?? null;
const theme = argValue("theme") === "light" ? "light" : "dark";
const only = argValue("only")?.split(",").filter(Boolean) ?? null;

const VIEWPORT = { width: 1280, height: 800 };
/** Phone-ish app viewport for the focused mobile crops (rendered at 3x). */
const MOBILE_VIEWPORT = { width: 430, height: 1000 };
const MOBILE_SCALE = 3;

/**
 * Union of the locators' boxes, horizontally spanning `spanX` when given,
 * padded and clamped to the viewport. Element-based so crops survive UI tweaks.
 * @param {import("playwright").Page} page
 * @param {{ parts: import("playwright").Locator[]; spanX?: import("playwright").Locator; padX?: number; padY?: number; padBottom?: number }} opts
 */
async function clipAround(page, { parts, spanX, padX = 16, padY = 16, padBottom = padY }) {
  const boxes = [];
  for (const loc of parts) {
    const box = await loc.first().boundingBox();
    if (!box) throw new Error(`clip target not visible: ${loc}`);
    boxes.push(box);
  }
  let left = Math.min(...boxes.map((b) => b.x));
  let right = Math.max(...boxes.map((b) => b.x + b.width));
  const top = Math.min(...boxes.map((b) => b.y));
  const bottom = Math.max(...boxes.map((b) => b.y + b.height));
  if (spanX) {
    const span = await spanX.first().boundingBox();
    if (span) {
      left = span.x;
      right = span.x + span.width;
    }
  }
  const vp = page.viewportSize() ?? MOBILE_VIEWPORT;
  const x = Math.max(0, left - padX);
  const y = Math.max(0, top - padY);
  const width = Math.min(vp.width, right + padX) - x;
  const height = Math.min(vp.height, bottom + padBottom) - y;
  return {
    x: Math.round(x),
    y: Math.round(y),
    width: Math.round(width),
    height: Math.round(height),
  };
}

async function collapseSidebar(page) {
  await page.getByRole("button", { name: "收起侧边栏" }).click();
  await page.getByRole("button", { name: "展开侧边栏" }).waitFor();
}

/**
 * @typedef {import("playwright").Page} Page
 * @type {{
 *   name: string;
 *   ready: (page: Page) => Promise<void>;
 *   mobile?: {
 *     ready: (page: Page) => Promise<void>;
 *     clip: (page: Page) => Promise<{ x: number; y: number; width: number; height: number }>;
 *   };
 * }[]}
 */
const SCENES = [
  {
    name: "workspace",
    async ready(page) {
      await page.getByRole("button", { name: /Q4 产品路线评审/ }).click();
      await page.getByRole("heading", { name: "决策" }).waitFor();
      await page.getByText("我们先过一下 Q4 的路线").waitFor();
    },
    mobile: {
      // Narrow window -> tabbed workspace; the summary tab is the default.
      async ready(page) {
        await page.getByRole("button", { name: /Q4 产品路线评审/ }).click();
        await collapseSidebar(page);
        await page.getByRole("heading", { name: "决策" }).waitFor();
      },
      clip: (page) =>
        clipAround(page, {
          parts: [
            page.getByRole("tablist", { name: "工作区" }),
            page.getByRole("heading", { name: "要点" }).locator("xpath=.."),
          ],
          spanX: page.locator("section[aria-label='摘要']"),
          padX: 0,
          padY: 0,
          padBottom: 12,
        }),
    },
  },
  {
    name: "recording",
    async ready(page) {
      await page.getByRole("heading", { name: "正在录音" }).waitFor();
      await page.getByText("系统声音：扬声器").waitFor();
      // Let the waveform history (56 samples x 40 ms) fill up.
      await page.waitForTimeout(3200);
    },
    mobile: {
      async ready(page) {
        await collapseSidebar(page);
        await page.getByRole("heading", { name: "正在录音" }).waitFor();
        await page.getByText("系统声音：扬声器").waitFor();
        await page.waitForTimeout(3200);
      },
      clip: (page) =>
        clipAround(page, {
          parts: [
            page.getByRole("heading", { name: "正在录音" }),
            page.getByRole("button", { name: "停止并转写" }),
          ],
          spanX: page
            .getByRole("heading", { name: "正在录音" })
            .locator("xpath=ancestor::section[1]"),
          padX: 24,
          padY: 12,
          padBottom: 20,
        }),
    },
  },
  {
    name: "hotwords",
    async ready(page) {
      await openSettings(page);
      await page.getByRole("tab", { name: "转写与摘要" }).click();
      await page.getByRole("heading", { name: "热词（转写）" }).waitFor();
      await page.getByText("火山引擎 TOS", { exact: true }).waitFor();
    },
    mobile: {
      async ready(page) {
        await openSettings(page);
        await page.getByRole("tab", { name: "转写与摘要" }).click();
        const heading = page.getByRole("heading", { name: "热词（转写）" });
        await heading.waitFor();
        await page.getByText("火山引擎 TOS", { exact: true }).waitFor();
        await heading.evaluate((el) => el.scrollIntoView({ block: "start" }));
      },
      clip: (page) => {
        const block = page
          .getByRole("heading", { name: "热词（转写）" })
          .locator("xpath=..");
        return clipAround(page, { parts: [block], padX: 16, padY: 16 });
      },
    },
  },
  {
    name: "llm",
    async ready(page) {
      await openSettings(page);
      const heading = page.getByRole("heading", { name: "摘要模型" });
      await heading.waitFor();
      await page.locator('input[value="http://localhost:11434/v1"]').waitFor();
      // Make the 摘要模型 card the focus: hide the cards above it (TOS etc.)
      // and centre it in the scroll area. The card is last on the page, so
      // pad the scroller to make that scroll position reachable.
      await heading.evaluate((el) => {
        const card = el.closest("section") ?? el.parentElement;
        if (!card) return;
        let scroller = card.parentElement;
        while (scroller && !/(auto|scroll)/.test(getComputedStyle(scroller).overflowY)) {
          scroller = scroller.parentElement;
        }
        if (!scroller) return;
        for (let node = card; node && node !== scroller; node = node.parentElement) {
          let sib = node.previousElementSibling;
          while (sib) {
            sib.style.visibility = "hidden";
            sib = sib.previousElementSibling;
          }
        }
        scroller.style.paddingBottom = `${scroller.clientHeight}px`;
        const view = scroller.getBoundingClientRect();
        const box = card.getBoundingClientRect();
        const gap = Math.max(24, (view.height - box.height) / 2);
        scroller.scrollTop += box.top - view.top - gap;
      });
    },
    mobile: {
      async ready(page) {
        await openSettings(page);
        const heading = page.getByRole("heading", { name: "摘要模型" });
        await heading.waitFor();
        await page.locator('input[value="http://localhost:11434/v1"]').waitFor();
        await heading.evaluate((el) =>
          (el.closest("section") ?? el).scrollIntoView({ block: "start" }),
        );
      },
      clip: (page) => {
        const heading = page.getByRole("heading", { name: "摘要模型" });
        return clipAround(page, {
          parts: [heading, page.locator('input[value="qwen3:14b"]')],
          spanX: page.locator('input[value="qwen3:14b"]').locator("xpath=ancestor::label[1]"),
          padX: 16,
          padY: 16,
          padBottom: 8,
        });
      },
    },
  },
  {
    name: "home",
    async ready(page) {
      await page.getByRole("heading", { name: "把会议录音变成可带走的纪要" }).waitFor();
      await page.getByRole("button", { name: /Q4 产品路线评审/ }).waitFor();
    },
  },
];

async function openSettings(page) {
  await page.getByRole("button", { name: "设置", exact: true }).click();
  await page.getByRole("heading", { name: "设置", exact: true }).waitFor();
}

/** Newest Chromium already in the local Playwright cache (any revision). */
function cachedChromium() {
  const cacheDir =
    process.env.PLAYWRIGHT_BROWSERS_PATH ||
    (process.platform === "win32"
      ? join(homedir(), "AppData", "Local", "ms-playwright")
      : process.platform === "darwin"
        ? join(homedir(), "Library", "Caches", "ms-playwright")
        : join(homedir(), ".cache", "ms-playwright"));
  if (!existsSync(cacheDir)) return null;
  const candidates = readdirSync(cacheDir)
    .filter((d) => /^chromium-\d+$/.test(d))
    .sort((a, b) => Number(b.split("-")[1]) - Number(a.split("-")[1]));
  for (const dir of candidates) {
    for (const rel of [
      ["chrome-win64", "chrome.exe"],
      ["chrome-win", "chrome.exe"],
      ["chrome-linux", "chrome"],
      ["chrome-linux64", "chrome"],
      ["chrome-mac", "Chromium.app", "Contents", "MacOS", "Chromium"],
    ]) {
      const exe = join(cacheDir, dir, ...rel);
      if (existsSync(exe)) return exe;
    }
  }
  return null;
}

async function launchBrowser() {
  const attempts = [
    ["bundled Chromium", {}],
    ["cached Chromium", { executablePath: cachedChromium() }],
    ["Microsoft Edge", { channel: "msedge" }],
  ];
  const errors = [];
  for (const [label, opts] of attempts) {
    if ("executablePath" in opts && !opts.executablePath) continue;
    try {
      const browser = await chromium.launch({ headless: true, ...opts });
      console.log(`[screenshots] browser: ${label}`);
      return browser;
    } catch (err) {
      errors.push(`${label}: ${String(err).split("\n")[0]}`);
    }
  }
  throw new Error(`No usable Chromium found:\n  ${errors.join("\n  ")}`);
}

async function captureScene(browser, baseUrl, scene, variant = "desktop") {
  const mobile = variant === "mobile";
  const context = await browser.newContext({
    viewport: mobile ? MOBILE_VIEWPORT : VIEWPORT,
    deviceScaleFactor: mobile ? MOBILE_SCALE : 2,
    locale: "zh-CN",
    timezoneId: "Asia/Shanghai",
    colorScheme: theme,
    reducedMotion: "reduce",
  });
  const page = await context.newPage();
  const problems = [];
  page.on("pageerror", (err) => problems.push(`pageerror: ${err.message}`));
  page.on("console", (msg) => {
    if (msg.type() === "error" || msg.type() === "warning") {
      const text = msg.text();
      // Font CDN hiccups and React dev hints are not UI problems.
      if (/fonts\.g|Download the React DevTools/.test(text)) return;
      problems.push(`console.${msg.type()}: ${text}`);
    }
  });

  const url = `${baseUrl}/scripts/screenshots/demo.html?scene=${scene.name}&theme=${theme}`;
  await page.goto(url, { waitUntil: "networkidle" });
  await page.locator(".app-shell").waitFor();
  await (mobile ? scene.mobile.ready(page) : scene.ready(page));
  await page.evaluate(() => document.fonts.ready);
  await page.waitForTimeout(400);

  const alerts = await page.locator('[role="alert"]').allInnerTexts();
  for (const text of alerts) problems.push(`visible alert: ${text}`);

  const file = join(outDir, `${scene.name}${mobile ? "-mobile" : ""}.png`);
  if (mobile) {
    await page.screenshot({ path: file, clip: await scene.mobile.clip(page) });
  } else {
    await page.screenshot({ path: file });
  }
  await context.close();
  return { file, problems };
}

async function captureOg(browser, workspacePng) {
  const logo = readFileSync(join(root, "branding", "meetphant-logo.svg"), "utf8");
  const shot = readFileSync(workspacePng).toString("base64");
  const html = `<!doctype html><html lang="zh-CN"><head><meta charset="utf-8">
<link href="https://fonts.googleapis.com/css2?family=Syne:wght@700&display=swap" rel="stylesheet">
<style>
  html,body{margin:0;width:1200px;height:630px;overflow:hidden}
  body{background:radial-gradient(900px 500px at 85% 0%,rgba(105,216,189,.14),transparent 60%),linear-gradient(160deg,#111A20,#0B1216);
    color:#EDF4F0;font-family:"PingFang SC","Microsoft YaHei","Noto Sans SC",system-ui,sans-serif;position:relative}
  .copy{position:absolute;left:72px;top:96px;width:470px}
  .logo svg{width:260px;height:auto;display:block}
  h1{font-size:42px;line-height:1.2;margin:44px 0 18px;font-weight:700;letter-spacing:-.01em;white-space:nowrap}
  h1 .latin{font-family:Syne,sans-serif}
  p{font-size:24px;line-height:1.55;margin:0;color:rgba(237,244,240,.72)}
  .dot{display:inline-block;width:10px;height:10px;border-radius:50%;background:#69D8BD;margin-right:12px;vertical-align:middle}
  .frame{position:absolute;left:600px;top:92px;width:860px;border-radius:14px;overflow:hidden;
    border:1px solid rgba(237,244,240,.14);box-shadow:0 30px 80px rgba(0,0,0,.55)}
  .bar{height:30px;background:#18232A;display:flex;align-items:center;gap:8px;padding:0 14px}
  .bar i{width:10px;height:10px;border-radius:50%;background:rgba(237,244,240,.18)}
  .frame img{display:block;width:100%}
</style></head><body>
  <div class="copy">
    <div class="logo">${logo}</div>
    <h1><span class="latin">Meetphant</span> 会议助手</h1>
    <p><span class="dot"></span>大象从不遗忘。<br>把会议录音变成可带走的纪要</p>
  </div>
  <div class="frame"><div class="bar"><i></i><i></i><i></i></div><img src="data:image/png;base64,${shot}"></div>
</body></html>`;
  const context = await browser.newContext({
    viewport: { width: 1200, height: 630 },
    deviceScaleFactor: 1,
  });
  const page = await context.newPage();
  await page.setContent(html, { waitUntil: "networkidle" });
  await page.evaluate(() => document.fonts.ready);
  await page.screenshot({ path: ogPath });
  await context.close();
}

async function main() {
  mkdirSync(outDir, { recursive: true });
  const server = await createServer({
    root,
    configFile: join(root, "vite.config.ts"),
    logLevel: "warn",
    server: { port: 1430, strictPort: false, host: "127.0.0.1", open: false },
  });
  await server.listen();
  const address = server.httpServer?.address();
  const port = typeof address === "object" && address ? address.port : 1430;
  const baseUrl = `http://127.0.0.1:${port}`;
  console.log(`[screenshots] vite dev server: ${baseUrl}`);

  let browser;
  let failed = false;
  try {
    browser = await launchBrowser();
    const scenes = only ? SCENES.filter((s) => only.includes(s.name)) : SCENES;
    const jobs = scenes.flatMap((scene) =>
      scene.mobile ? [[scene, "desktop"], [scene, "mobile"]] : [[scene, "desktop"]],
    );
    for (const [scene, variant] of jobs) {
      const label = variant === "mobile" ? `${scene.name}-mobile` : scene.name;
      try {
        const { file, problems } = await captureScene(browser, baseUrl, scene, variant);
        console.log(`[screenshots] ${label} -> ${file}`);
        for (const p of problems) console.warn(`  ! ${p}`);
        if (problems.some((p) => p.startsWith("visible alert") || p.startsWith("pageerror"))) {
          failed = true;
        }
      } catch (err) {
        failed = true;
        console.error(`[screenshots] ${label} FAILED: ${err}`);
      }
    }
    const workspacePng = join(outDir, "workspace.png");
    if (existsSync(workspacePng)) {
      await captureOg(browser, workspacePng);
      console.log(`[screenshots] og -> ${ogPath}`);
    }
  } finally {
    await browser?.close();
    await server.close();
  }
  if (failed) {
    console.error("[screenshots] finished with problems (see above)");
    process.exitCode = 1;
  }
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
