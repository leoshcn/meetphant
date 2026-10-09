# 重构迭代1：响应性、可观测性、CI

## Goal

先解决三件事：界面不再被后端的耗时调用卡死；线上问题有日志可查；每次 push 或提 PR 都自动跑完整的质量检查。这是 CTO 评审路线中的第 1 个迭代，第 2、3 个迭代（作业恢复、录音崩溃恢复、版本化迁移、类型自动生成等）都依赖它打下的基础。

## Background

来源：2026-10-09 的 CTO 架构评审，对应评审中的第 1、2、7、11 条。

- 现在所有 `#[tauri::command]` 都是同步命令。在 Tauri 2 中，同步命令运行在主线程上，涉及网络或 FFmpeg 转码的命令执行期间，窗口会停止响应。
- `AppState.db` 是一个全局 `Mutex<Connection>`，`summary_generate` 在持有锁期间调用大模型。
- 代码里没有任何日志依赖，失败经常被 `let _ =` 直接吞掉。
- CI 只有 `release.yml`，打 tag 时才触发，并且只跑 `npm test`。

## Task Map

| 子任务 | 交付物 | 依赖 |
|---|---|---|
| `10-09-async-commands` | IO 类命令不再阻塞主线程；网络请求期间不持有 DB 锁 | 无 |
| `10-09-tracing-logging` | 后端接入结构化日志并写入本地文件；在命令失败处和被吞掉错误的地方补日志 | 建议排在 async-commands 之后，改动集中在同一批文件，避免冲突 |
| `10-09-ci-workflow` | 新增 PR/push 触发的 CI 工作流 | 无，可以最先做 |

建议顺序：ci-workflow → async-commands → tracing-logging。先上 CI，后面两个子任务的改动就有自动检查兜底。

## Cross-child Acceptance Criteria

- [ ] `cargo test`、`cargo clippy --all-targets -- -D warnings`、`npm test`、`npm run typecheck` 全部通过，并且由新的 CI 工作流自动执行
- [ ] 现有 IPC 命令的名称、参数和返回结构都不变；唯一新增的命令是 `logs_open_dir`（tracing-logging 的 R7）
- [ ] 被改造的命令和新增的 `logs_open_dir` 统一采用 `async fn` + `run_blocking`（spawn_blocking），不使用 `#[tauri::command(async)]` 修饰同步函数（原因见 async-commands 的 design 第 1 节）
- [ ] 不在日志或错误信息中泄露密钥、预签名 URL 签名或音频内容（`.trellis/spec/backend/error-handling.md` 第 2 条契约）

## Key Decisions（2026-10-09，用户确认）

- `record_stop` 只移出主线程，接口不变；后台转码和进度事件放到第 2 个迭代（async-commands 的 D1）
- 日志的用户侧入口只做"打开日志文件夹"按钮，不做导出诊断包（tracing-logging 的 D1）
- CI 只用 Windows runner（ci-workflow 的 D1）

## Out of Scope（留给第 2、3 个迭代）

- `record_stop` 后台转码和进度事件
- 让外键真正生效（`PRAGMA foreign_keys`），放进版本化迁移一起做

- 合并转写的两份流程、作业恢复、事件推送替代轮询
- 录音崩溃恢复、版本化迁移、事务化删除
- 错误码枚举化、TS 类型自动生成、前端状态管理重构
