# Implement: PR/push CI 工作流

轻量任务，不单独写 design.md。

## Checklist

1. [ ] 在 `src-tauri` 下执行 `cargo fmt`，修掉 4 处格式差异，确认只有纯格式变化
2. [ ] 新增 `.github/workflows/ci.yml`：
   - 触发条件：`push`（branches: main）、`pull_request`
   - 设置 `concurrency: ci-${{ github.ref }}`，`cancel-in-progress: true`
   - 设置 `permissions: contents: read`
   - 只有一个 job，`runs-on: windows-latest`（D1），`timeout-minutes: 45`
   - 步骤：checkout → setup-node 20（npm cache）→ `npm ci` → `npm run typecheck` → `npm test` → `dtolnay/rust-toolchain@stable`（带 `rustfmt`、`clippy` 组件）→ `swatinem/rust-cache`（workspaces: `src-tauri -> target`）→ `cargo fmt --check` → `cargo clippy --all-targets -- -D warnings` → `cargo test`
   - Rust 相关步骤设置 `working-directory: src-tauri`
3. [ ] 在 Rust 步骤之前执行 `npm run build`。原因：`dist` 在 `.gitignore` 里，全新 checkout 时不存在；而 tauri-codegen 发现 `frontendDist` 指向的目录不存在时会直接 panic（`tauri-codegen/src/context.rs:189`，"…but this path doesn't exist"）。用 `npm run build` 而不是创建空目录，这样 `tsc` 和 vite 构建本身也成为一项检查
4. [ ] 不改动 `release.yml`
5. [ ] 推送到一个分支并提 PR，确认 CI 全绿；再推一个故意让测试失败的提交，确认 CI 变红后撤销这个提交

## Validation

本地按顺序执行同样的命令：

```bash
npm run typecheck && npm test
cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

## Result（2026-10-09）

- 第 1 步 DONE：`cargo fmt` 只改了 `commands/tray.rs` 和 `services/ffmpeg_service.rs`，都是纯格式变化
- 第 2–4 步 DONE：新增 `.github/workflows/ci.yml`，`release.yml` 没有改动
- **计划外修复**：`cargo test` 一直以 0xc0000139 崩溃，之前的任务记录把它当成"环境问题"，实际上 Rust 测试已经很久没真正跑过，放到 CI 上也会失败。根因是 Tauri 只把 Common-Controls v6 manifest 嵌进 app exe。改法：`src-tauri/build.rs` 改用 `new_without_app_manifest()`，再通过链接参数给所有 MSVC 产物声明这个依赖。验证：90/90 测试通过；`target/debug/meetphant.exe` 里仍然包含 Common-Controls 依赖。已写入 `spec/backend/quality-guidelines.md`
- 本地验证：typecheck ✓、vitest 50/50 ✓、build ✓、fmt ✓、clippy ✓、cargo test 90/90 ✓
- 第 5 步 DONE：PR #1。`e959c42` 绿（8m02s，CI 上 Rust 90/90）；探测提交 `bc7856d` 被 `cargo fmt --check` 拦下变红，`68ccadf` 被 `cargo test` 拦下变红（90 passed, 1 failed）；revert 提交 `2599c93` 恢复绿色，代码树与 `e959c42` 一致
- 手动检查 DONE（用户确认）：`tauri dev` 里的文件对话框与 manifest 改动前一致
