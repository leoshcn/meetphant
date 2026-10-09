# Implement: 接入 tracing 日志

前置条件：`10-09-async-commands` 已完成。两个任务改的是同一批 `commands/*.rs` 文件，按顺序做可以避免冲突。

## Checklist

1. [ ] `Cargo.toml`：新增 `tracing`、`tracing-subscriber`（env-filter、fmt）、`tracing-appender`
2. [ ] 新增 `src-tauri/src/logging.rs`：`init(log_dir) -> Option<WorkerGuard>`，失败时降级为只输出到 stderr；在 `lib.rs` 的 setup 里，迁移之前调用
3. [ ] `error.rs`：新增 `CmdResultExt::log_cmd`，用户输入类错误码降为 debug；补单元测试
4. [ ] 给全部命令加上 `.log_cmd("…")`
5. [ ] `transcription_service.rs`：加 span 和阶段日志；补记被吞掉的错误；创建 HTTP 客户端失败时把作业标记为失败
6. [ ] `migration.rs`、`recording_service.rs`、`ffmpeg_service.rs`：在失败分支补日志
7. [ ] 新增 `logs_open_dir` 命令，并在 `lib.rs` 注册
8. [ ] 前端：新增 `ipc/commands/logs.ts` 和测试，在 `ipc/index.ts` 导出，`SettingsAbout.tsx` 加按钮
9. [ ] 改写 `.trellis/spec/backend/logging-guidelines.md`

## Validation

```bash
cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
npm run typecheck && npm test
```

手动验证（`npm run tauri dev`）：
- 跑一次转写和一次摘要，日志文件里能看到 span 和阶段日志
- 故意把 TOS bucket 填错，日志里出现 `TOS_UPLOAD_ERROR`
- 在日志目录里 grep 测试用的 API key、AK、SK 和 `X-Tos-Signature`，结果为零
- 在关于页点击"打开日志文件夹"，系统文件管理器会打开

## Risk / Rollback

- `WorkerGuard` 被提前 drop 会丢日志：必须由 AppState 或静态变量持有
- 日志改动都是增量的，回滚时删掉即可

## Result（2026-10-09）

- 第 1–9 步 DONE
  - `logging.rs`：按天滚动，保留 7 个文件；guard 由 `OnceLock` 持有；初始化失败时降级为只输出到 stderr；在 setup 最前面初始化
  - `CmdResultExt::log_cmd`：用户输入类错误码记为 debug，其余记为 warn；只记录 cmd 和 code
  - 所有命令都加上了 `.log_cmd`
  - 计划外改动：同步 DB 命令的 5 行加锁样板统一改成 `db::with`（14 处）。这样加锁失败也会经过 `log_cmd`；另外 `db::lock` 中毒时记 error
  - 转写：span（job_id、meeting_id）加阶段日志；新增 `fail_job`；创建 HTTP 客户端失败时作业不再永远停在 running
  - 迁移、FFmpeg 下载、M4A 编码、音频流错误都补上了日志
  - 新增 `logs_open_dir` 命令，前端加封装和测试，关于页加按钮
  - 规范：重写 `logging-guidelines.md`；`api-shape.md` 补充新命令和"命令执行模型"
- 自查：23 处日志语句都不含禁止字段（grep `audio_url`、`api_key`、`secret`、`.message`、`token`、`path` 均无匹配）；不再有 `let _ = mark_job_failed`
- 本地验证：fmt ✓、clippy ✓、cargo test 94/94 ✓、typecheck ✓、vitest 52/52 ✓、build ✓
- 手动验证 PENDING：跑一次转写和一次摘要后查看日志；填错 TOS bucket 后日志里出现错误码；在日志目录 grep 密钥结果为零；关于页按钮能打开日志目录
