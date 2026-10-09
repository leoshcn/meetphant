# PR/push CI 工作流

## Goal

每次 push 到 main 或提 PR，都自动跑完整的前后端质量检查，有问题在合并前暴露，不要等到发版才发现。

## Confirmed Facts（代码证据）

- `.github/workflows/` 下只有 `release.yml`，在 `v*` tag 或手动触发时运行，只跑了 `npm test`（第 65 行），没有 `cargo test`、clippy 和 typecheck
- 本地基线（2026-10-09）：`cargo clippy --all-targets -- -D warnings` 零告警，可以直接作为强制检查
- 录音只支持 Windows（`recording_service.rs:582` 用 `#[cfg(not(windows))]` 直接返回错误），WASAPI 相关代码只在 Windows 上编译
- `release.yml` 已经有可以复用的步骤：Node 20 + npm cache、`dtolnay/rust-toolchain`、`swatinem/rust-cache`
- `package.json` 里有 `test`（vitest）和 `typecheck`（tsc --noEmit）脚本

## Requirements

- R1：新增 `.github/workflows/ci.yml`，在 `push`（main）和 `pull_request` 时触发
- R2：前端检查 `npm ci` → `npm run typecheck` → `npm test` → `npm run build`（`dist` 在 `.gitignore` 里，Rust 编译需要这个目录存在，见 implement.md 第 3 步）
- R3：先执行一次 `cargo fmt`，修掉现有的 4 处格式差异（`commands/tray.rs:6/16/24`、`services/ffmpeg_service.rs:415`，2026-10-09 实测，都是纯格式问题）；然后后端检查依次跑 `cargo fmt --check` → `cargo clippy --all-targets -- -D warnings` → `cargo test`
- R4：使用 Rust 缓存和 npm 缓存；用 concurrency 配置取消同一分支上过时的运行
- R5：不改动 `release.yml` 的发版行为

## Acceptance Criteria

- [ ] 提一个 PR 时 CI 自动触发，所有步骤都是绿的
- [ ] 故意引入一条 clippy 告警或一个失败的测试，CI 会变红（本地 act 或分支上验证一次即可）
- [ ] 后端检查在 Windows runner 上通过（这样会编译到 WASAPI 相关代码）

## Key Decisions

- D1（2026-10-09，用户确认）：只用 `windows-latest` 一个 runner。等真的要发 macOS 或 Linux 版时，再加对应平台的 runner。

## Out of Scope

- Ubuntu/macOS runner（D1）

- 代码签名、自动发版改造
- 覆盖率统计
