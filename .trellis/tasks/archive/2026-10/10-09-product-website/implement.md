# Implement — Meetphant 产品官网

## Checklist

1. [ ] 新增 devDependency `playwright`；`package.json` 增加 `"screenshots": "node scripts/screenshots/capture.mjs"`。
2. [ ] `scripts/screenshots/fixtures.ts`：示例数据（3–5 场会议、带说话人的转写、要点/待办/决策摘要、热词、custom LLM 设置）；读取 `src/ipc/types.ts` 确保形状一致。
3. [ ] `scripts/screenshots/demo.html` + `demo-main.ts`：安装 `mockWindows` / `mockIPC`，按 `scene` / `theme` 参数渲染。
4. [ ] `scripts/screenshots/capture.mjs`：启动 vite → 逐场景截图 → 生成 `og.png`（1200×630，logo + slogan + 工作区截图裁切）→ 关闭服务。
5. [ ] 运行 `npm run screenshots`，人工检查每张图（无报错弹窗、无空状态、无乱码）。
6. [ ] `website/assets/style.css` + 复制 logo/icon。
7. [ ] `website/index.html`（中文）。
8. [ ] `website/en/index.html`（英文），hreflang 与语言切换互指。
9. [ ] `.github/workflows/pages.yml`。
10. [ ] README / README.zh-CN 增加「官网」一节：本地预览方式、`npm run screenshots`、需开启 Pages（source: GitHub Actions）。

## Validation

```bash
npm run typecheck
npm test
npm run build
npm run screenshots
npx serve website   # 或 py -3 -m http.server -d website
```

- 在 375 / 768 / 1440 宽度下检查 `index.html` 与 `en/index.html`（Playwright 截图即可）。
- `grep -riE "数据不出本机|全程本地|完全离线|never leaves|fully offline|100% local" website/` 应无结果（AC4）。
- 所有下载链接 `grep -o 'releases/latest' website -r` 覆盖（AC7）。

## Risky points

- `src/main.tsx` 顶层调用 `getCurrentWindow()`，必须在 import 前安装 `mockWindows`。
- 组件中 `listen(...)` 需 `shouldMockEvents: true`，否则未处理的 `plugin:event|listen` 会报错。
- 录音场景依赖 `record_status` 轮询/事件驱动的波形，fixture 需返回 running 状态与电平数据。
- 若 `src/` 需改动，保持最小并说明原因。
