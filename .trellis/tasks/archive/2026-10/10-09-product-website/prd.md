# Meetphant 产品官网

## Goal

为 Meetphant 打造一个简约、有质感的中英双语产品官网（单页落地页），使用现有产品 logo，产品名「Meetphant 会议助手」，突出三大特性，并配以**由真实应用界面自动生成**的示例截图；通过 GitHub Pages 发布。

## Background（仓库证据）

- 品牌资产：`branding/meetphant-logo.svg`（横向标识，适用深色背景）、`branding/meetphant-icon.svg`；品牌色 `#111A20`（底）/ `#EDF4F0`（前景）/ `#69D8BD`（强调）。已有 slogan：「大象从不遗忘。把会议录音变成可带走的纪要」（`README.zh-CN.md`）。
- 特性 1 依据：WASAPI loopback 同时采集麦克风 + 系统声音，与任何会议软件无关；另支持导入本地音频（`src/features/meeting-recording`, `src/features/transcription-import`）。
- 特性 2 依据：设置页热词表，热词随 ASR 请求发送（`src/features/settings-hotwords`）。
- 特性 3 依据：摘要走任意 OpenAI 兼容端点，`custom` 预设占位 `http://localhost:11434/v1`（`src/features/settings-credentials/SettingsCredentials.tsx:803`），后端有本地端点测试（`src-tauri/src/providers/openai_compat/client.rs:413,708`）；密钥只进系统钥匙串。**限制**：转写仍为云端（音频经火山 TOS 上传至豆包录音文件识别 2.0）。
- 现有 `docs/screenshots/*.png` 拍摄于 2026-07-28，早于 UI 视觉重构（commit f969d83），已过时。
- 前端所有 IPC 经 `window.__TAURI_INTERNALS__`（入口 `src/ipc/client.ts`）；`@tauri-apps/api/mocks`（`mockIPC` / `mockWindows`）已随依赖安装，可在普通浏览器中渲染真实 React 界面。
- 环境：Windows 11，Node、Edge 可用，`~/AppData/Local/ms-playwright` 已有 Chromium 缓存；项目尚无 playwright 依赖。CI 风格见 `.github/workflows/ci.yml`。

## Decisions

- D1（用户选 A）：特性 3 保留但措辞准确——强调「摘要可接本地大模型，会议内容不交给第三方 AI；密钥只存系统钥匙串」；**禁止**「数据不出本机 / 全程本地 / 完全离线」等表述。
- D2（用户选 B）：`website/` 纯 HTML/CSS 静态站（无构建步骤）+ 新增 GitHub Actions 部署到 GitHub Pages（合并到 `main` 即公开）。截图脚本放 `scripts/screenshots/`，输出到 `website/assets/screenshots/`。下载 CTA → `https://github.com/leoshcn/meetphant/releases/latest`。
- D3（用户选 B）：中英双语。中文为默认（`website/index.html`），英文 `website/en/index.html`，页头语言切换；两种语言共用同一套截图（应用界面仅中文）。
- D4（规划默认，可在审阅时调整）：官网以深色为主视觉（匹配深色背景 logo），不做深浅色切换；截图默认用应用深色主题。

## Requirements

- R1 页面结构（中/英一致）：页头（logo + 语言切换 + 下载）→ Hero（产品名「Meetphant 会议助手」/ "Meetphant Meeting Assistant"、slogan、下载 CTA、工作区主截图）→ 三大特性（每项：标题、副标题、2–3 条要点、对应截图）→ 工作流程三步（录制/导入 → 转写 → 摘要）→ 更多能力（导入音频、要点/待办/决策、多家模型预设、自动更新）→ 下载区 → 页脚。
- R2 特性文案：
  1. 独立录制，不被会议平台绑定 —— 同时录麦克风与系统声音，腾讯会议/飞书/Zoom/Teams 或线下会议都能录，也可导入已有音频。
  2. 自定义热词，越用越懂你 —— 专有名词、人名、产品名加入热词表，转写更准。
  3. 可连接本地大模型，纪要由你做主 —— 按 D1 措辞。
- R3 视觉：简约有质感（大留白、细描边、克制的强调色、截图带窗口框/柔和阴影）；响应式；无 JS 框架依赖；字体仅 Google Fonts 或系统字体。
- R4 截图自动化：一条 npm 命令以 mock 数据渲染真实应用界面，用无头 Chromium 导出截图；mock 数据为可信的中文示例会议，不含真实个人信息或真实密钥。
- R5 部署：新增 `.github/workflows/pages.yml`，`website/**` 变更推送到 `main` 时发布；截图入库（CI 不重新生成）。
- R7 手机版截图（用户 2026-10-09 追加）：为 Hero 与三大特性各生成一张聚焦关键区域的移动端裁剪图（`*-mobile.png`），页面用 `<picture>` + media query 在窄屏（≤ 640px）切换；桌面端不变。
- R6 SEO/分享基础：`<title>`、description、Open Graph 图、favicon（`meetphant-icon.svg`）、`hreflang` 互指。

## Acceptance Criteria

- [x] AC1 `npm run screenshots` 无需 Tauri 即可生成至少 4 张截图：工作区（转写+摘要）、录音中、热词设置、摘要模型设置（自定义端点显示 `http://localhost:11434/v1`），均来自真实 React 组件。（R4）
- [x] AC2 截图脚本只在开发期使用；`npm run build`、`npm run typecheck`、`npm test` 均通过且产物不含演示入口。（R4）
- [x] AC3 `website/index.html` 与 `website/en/index.html` 经静态服务器打开即完整呈现，三大特性均有标题、说明与对应截图，语言切换互通。（R1, R2, D3）
- [x] AC4 页面全文不含「数据不出本机」「全程本地」「完全离线」及英文等价表述。（D1）
- [x] AC5 375px 宽度下无横向滚动；1440px 下布局居中美观。（R3）
- [x] AC6 `pages.yml` 仅发布 `website/` 目录；README 写明需在仓库设置中将 Pages source 设为 GitHub Actions。（R5）
- [x] AC7 所有下载按钮指向 Releases latest 链接。（D2）
- [x] AC8 两个页面具备 title/description/OG 图/favicon/hreflang。（R6）

- [x] AC9 `npm run screenshots` 同时产出 4 张 `*-mobile.png`；在 390px 视口下页面加载的是移动版截图，关键文字（计时、热词、Base URL/模型、摘要要点）肉眼可读；1440px 下仍加载桌面版；仍无横向滚动。（R7）

## Out of Scope

- 改动应用功能（如本地 ASR）。
- 自定义域名、统计埋点、博客/文档多页。
- 更新 README 中的 `docs/screenshots`（脚本可复用，留作后续）。
- 英文版应用界面截图。

## Risks

- 合并到 `main` 后官网即公开（D2，用户已接受）；首次需手动开启 Pages。
- 截图依赖本机中文字体，故本地生成后入库，而非 CI 生成；UI 变更后需手动重跑。
