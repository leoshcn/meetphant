# Implement: IO 命令异步化 + 网络请求期间不持锁

前置条件：`10-09-ci-workflow` 已合并，或者至少本地可以跑通 CI 的全部命令。

## Checklist

1. [ ] `commands/mod.rs`：新增 `run_blocking` 辅助函数（design 第 1 节）
2. [ ] `summary_service.rs`：`generate_summary` 改成接收 `&Mutex<Connection>`，按三段拆锁，写回前再检查一次会议是否存在（design 第 3 节）；`generate_summary_http` 同步调整
3. [ ] `summary_service.rs` 测试：现有 5 个用例改成用 Mutex 包装；新增 `LockProbeGenerator` 用例，以及"生成期间会议被删除时返回 `NOT_FOUND` 且不写入"用例
4. [ ] `commands/summary.rs`：`summary_generate` 改成 async + run_blocking
5. [ ] `settings_test_service.rs`：`test_tos` 改成接收 `&Mutex<Connection>`，只在合并配置时持锁；测试同步调整
6. [ ] `commands/settings.rs`：三条 `settings_test_*` 命令改成 async + run_blocking
7. [ ] `commands/recording.rs`：`record_start`、`record_stop`、`record_list_input_devices` 改成 async + run_blocking；`record_status` 保持同步
8. [ ] `commands/meetings.rs`：`meetings_create_from_file`、`meetings_attach_source` 改造
9. [ ] `commands/jobs.rs`：`jobs_start_transcription` 改造（`spawn_transcription_job` 不变）
10. [ ] 全仓搜索确认：任何持有 `state.db.lock()` 的作用域内都不调用 `Http*Client`、`HttpTosClient` 或 `generate_summary_http`

## Validation

```bash
cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
npm run typecheck && npm test
```

手动验证（`npm run tauri dev`，需要 Windows 和真实凭证）：
- 生成摘要时切换会议、打开设置页，界面没有卡顿
- 录 3 分钟以上再停止，转码期间可以拖动窗口
- 填一个错误的 TOS endpoint 后点"测试"，等待期间窗口不出现"未响应"

## Risk / Rollback

- 风险最高的是 `summary_service.rs` 的签名改动，回滚时还原这个文件和 `commands/summary.rs` 即可
- `record_stop` 改成 async 后，`lib.rs` 里的 CloseRequested 处理只调用 `status()`，不受影响；`AppShell` 的关闭流程也只 await 这个结果，同样不受影响

## Result（2026-10-09）

- 第 1–10 步 DONE
  - `commands/mod.rs::run_blocking`、`db::lock`
  - `generate_summary` 改为接收 `&Mutex<Connection>`，按三段拆锁，写回前再检查会议是否存在
  - `test_tos` 只在合并配置时持锁
  - 改成 async 的 9 条命令：`summary_generate`、`settings_test_doubao/tos/dashscope`、`record_start/stop/list_input_devices`、`meetings_create_from_file/attach_source`、`jobs_start_transcription`
- 第 10 步排查：所有网络调用点（`generate_summary_http`、`test_*`、转写线程）都不在持锁作用域内
- 新增测试 `db_lock_released_during_generation`、`meeting_deleted_during_generation_writes_nothing`
- 本地验证：fmt ✓、clippy ✓、cargo test 92/92 ✓、typecheck ✓、vitest 50/50 ✓
- 手动验证 DONE（用户确认，2026-10-09）：摘要生成期间切换会议、打开设置页；长录音停止时窗口能否拖动；TOS 探测失败等待期间窗口不出现"未响应"
