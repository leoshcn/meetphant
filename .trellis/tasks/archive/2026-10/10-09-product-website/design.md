# Design — Meetphant 产品官网

## 目录与边界

```
website/                     # 发布根目录（GitHub Pages artifact）
  index.html                 # 中文（默认）
  en/index.html              # English
  assets/
    style.css                # 两种语言共用
    meetphant-logo.svg       # 复制自 branding/
    meetphant-icon.svg
    og.png                   # 由截图脚本顺带生成的分享图
    screenshots/*.png        # 截图脚本输出
scripts/screenshots/
  demo.html                  # 仅 Vite dev server 使用的演示入口
  demo-main.ts               # 安装 Tauri mocks → 动态 import 真实 src/main.tsx
  fixtures.ts                # 示例会议、转写、摘要、热词、设置
  capture.mjs                # Playwright：启动 vite、逐场景导航/操作/截图
.github/workflows/pages.yml
```

- 应用代码 `src/` **零改动**为目标；若某场景无法通过 mock 达到，只允许最小、无副作用的改动并在 PR 中说明。
- `vite build` 只以 `index.html` 为入口，`scripts/screenshots/` 不进产物；`tsconfig.json` include 仅 `src`，不影响 typecheck。

## 截图管线

1. `capture.mjs` 以编程方式启动 Vite dev server（独立端口，避免与 1420 冲突）。
2. 打开 `/scripts/screenshots/demo.html?scene=<name>&theme=<dark|light>`。
3. `demo-main.ts`：
   - `mockWindows("main")` 使 `getCurrentWindow().label === "main"`；
   - `mockIPC((cmd, args) => fixtures.handle(scene, cmd, args), { shouldMockEvents: true })`，覆盖全部业务命令（`meetings_*`、`jobs_*`、`summary_*`、`settings_*`、`record_*`、`ffmpeg_status`、`app_health` 等）及插件命令（updater 返回无更新、dialog 等返回空）；
   - 预写 `localStorage` 主题缓存，随后 `import("/src/main.tsx")`。
4. Playwright（`playwright` 包，优先用已缓存 Chromium，失败回退 `channel: "msedge"`）：viewport 1280×800，`deviceScaleFactor: 2`，等待 `document.fonts.ready` 与场景就绪选择器，必要时点击导航（如进入设置 → 热词）。
5. 输出 PNG 至 `website/assets/screenshots/`。

| 场景 | 内容 | 用于 |
|------|------|------|
| `workspace` | 侧栏多条会议 + 转写/摘要分栏（要点/待办/决策） | Hero |
| `recording` | 录音进行中（计时、波形、麦克风+系统声音） | 特性 1 |
| `hotwords` | 热词设置，若干产品名/人名 | 特性 2 |
| `llm` | 摘要模型设置，preset=custom，Base URL `http://localhost:11434/v1`，模型 `qwen3:14b` | 特性 3 |
| `home` | 首页/导入入口（可选） | 更多能力 |

## 网站设计

- 深色主视觉：背景 `#0B1216`→`#111A20` 渐变，前景 `#EDF4F0`，强调 `#69D8BD`（仅用于 CTA、小圆点、关键词）。
- 字体：标题 Syne 700（与应用一致，仅拉丁）、中文用系统栈（PingFang SC / Microsoft YaHei / Noto Sans SC）。
- 特性区左右交替图文；截图包裹在细描边、圆角、柔和阴影的「窗口框」中；移动端改为上下堆叠。
- CSS tokens 写在 `:root`；无 JS 或仅极少量内联 JS（无需框架）。
- 截图 `<img>` 带 `width/height`、`loading="lazy"`（Hero 除外）、中英文 alt。

## 部署

`pages.yml`：`on: push (main, paths: website/**)` + `workflow_dispatch`；权限 `pages: write`, `id-token: write`；`actions/configure-pages` → `actions/upload-pages-artifact`（path: `website`）→ `actions/deploy-pages`。风格对齐 `ci.yml`（注释头、concurrency）。

## Trade-offs

- 纯静态双 HTML 意味着结构重复；换来零构建、零依赖，单页规模下可接受。
- 截图入库而非 CI 生成：避免 CI 字体差异，代价是 UI 变更后需手动重跑 `npm run screenshots`。

## 回滚

所有改动为新增文件（+ `package.json` 一条脚本与一个 devDependency）；删除即回滚。关闭 Pages 即下线。
