# 接入 tracing 日志与诊断导出

## Goal

用户反馈"转写一直不动""摘要失败"时，开发者能从本地日志定位原因；日志里不出现任何密钥。

## Confirmed Facts（代码证据）

- `Cargo.toml` 没有 `log`、`tracing` 或 `tauri-plugin-log` 依赖
- `.trellis/spec/backend/logging-guidelines.md` 目前只有一个占位说明：建议用 `tracing`；不记录 Doubao token；命令失败时记录 `AppErrorDto.code`
- 被吞掉的错误举例：
  - `transcription_service.rs:283-289`：后台线程拿不到 `AppState` 或 HTTP 客户端时直接 `return`，作业永远停在 running
  - `transcription_service.rs:298/305/312/370/373/379`：`let _ = mark_job_failed(...)`
  - `transcription_service.rs:376`：TOS 删除失败被忽略
  - `lib.rs:39-41`：迁移函数的结果没有被检查
- 禁止记录：TOS AK/SK、预签名 URL 签名（`quality-guidelines.md`）、token、base64 音频（`error-handling.md`）

## Requirements

- R1：接入 `tracing`，日志写到 Tauri 标准日志目录 `app_log_dir()`（Windows 上是 `%LOCALAPPDATA%\com.meetphant.app\logs`），按天滚动并保留 7 个文件（design 第 1 节）
- R2：每条命令失败时记录 `warn` 级日志，内容包含命令名和 `AppErrorDto.code`，不记录参数值
- R3：转写作业的关键节点记录 `info` 级日志（上传开始/完成、提交、轮询结束、成功/失败），只记录 job_id、meeting_id、耗时、错误码
- R4：给上面列出的被吞错误补上 `warn` 或 `error` 级日志；其中后台转写线程创建 HTTP 客户端失败时（`transcription_service.rs:286`），除了记日志，还要把作业标记为失败，不能让它一直停在 running
- R5：发布版默认级别为 `info`，开发版为 `debug`
- R6：在 `logging-guidelines.md` 里补全正式规范（允许记录和禁止记录的字段、级别约定）

## Acceptance Criteria

- [ ] 跑一次转写和一次摘要后，日志文件里能看到完整的阶段日志和耗时
- [ ] 人为制造一次失败（例如填错 TOS bucket），日志里有对应的错误码
- [ ] 在日志目录里 grep 测试用的 API key、AK/SK 和 `X-Tos-Signature`，结果为零
- [ ] 日志总大小有上限（验证滚动配置）
- [ ] 在设置 → 关于里点击"打开日志文件夹"，系统文件管理器会打开日志目录

- R7：在设置 → 关于里新增"打开日志文件夹"按钮，通过新增的 `logs_open_dir` 命令打开日志目录（D1）

## Key Decisions

- D1（2026-10-09，用户确认）：用户侧入口只做"打开日志文件夹"按钮；"导出诊断包"（zip 打包、脱敏、保存对话框）推迟到真正需要远程排查时再做。

## Out of Scope

- 导出诊断包（D1）

- 远程上报、崩溃收集（Sentry 等）
- 前端日志汇入后端（可以以后再做）
