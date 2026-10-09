# Design: 接入 tracing 日志

## 1. 依赖与初始化

新增依赖：`tracing`、`tracing-subscriber`（启用 `env-filter`、`fmt` 特性）、`tracing-appender`。

不选 `tauri-plugin-log`，原因有两个：一是规范（`logging-guidelines.md`）明确建议用 `tracing`；二是 `tracing` 的 span 可以把同一个 job_id 的各阶段日志串起来。

初始化放在 `lib.rs` 的 `setup` 里，**在迁移和开数据库之前**完成，这样迁移失败也能记录下来：

- 日志目录用 `app.path().app_log_dir()`。在 Windows 上是 `%LOCALAPPDATA%\com.meetphant.app\logs`，这是 Tauri 的标准位置。PRD R1 写的"应用数据目录下的 logs/"以这里为准
- 滚动策略：`tracing_appender::rolling::Builder`，按天滚动（`Rotation::DAILY`），`max_log_files(7)`，文件名前缀 `meetphant`，后缀 `log`
- 用 `tracing_appender::non_blocking` 写文件，`WorkerGuard` 存进 `AppState`（或者用一个 `OnceLock` 静态变量持有），保证进程退出前把缓冲写完
- 级别：默认 `info`；`cfg!(debug_assertions)` 时用 `debug`；允许用 `MEETPHANT_LOG` 环境变量覆盖（EnvFilter）
- 开发版额外输出到 stderr
- 初始化失败（比如目录不可写）不能阻塞应用启动：降级为只输出到 stderr，并在 stderr 打一行警告

日志总量上限：按天滚动并保留 7 个文件。在 `info` 级别下，一天的日志预计不超过几 MB（只有阶段日志和错误）。tracing-appender 不支持按大小滚动；如果以后发现某一天的日志异常膨胀，再考虑换方案。这一点记为已知限制。

## 2. 命令失败日志（R2）

在 `error.rs` 里新增：

```rust
pub trait CmdResultExt<T> { fn log_cmd(self, cmd: &'static str) -> CmdResult<T>; }
impl<T> CmdResultExt<T> for CmdResult<T> {
    fn log_cmd(self, cmd: &'static str) -> CmdResult<T> {
        if let Err(e) = &self { tracing::warn!(cmd, code = %e.code, "command failed"); }
        self
    }
}
```

- 每个命令函数的返回值末尾加 `.log_cmd("<cmd_name>")`
- 只记录错误码，不记录 `message`。现有 message 大多可以安全展示给用户，但一部分来自上游 HTTP 响应体（例如 `asr_provider_error(message)`），无法保证不含敏感内容
- `NOT_FOUND`、`SETTINGS_INVALID`、`INVALID_ARGUMENT` 这类用户输入错误降为 `debug` 级，避免刷屏

## 3. 转写作业日志（R3）

在 `spawn_transcription_job` 的线程里创建 `info_span!("transcription", job_id, meeting_id)`，在下面这些节点记录 `info`：

- `upload.start`，字段 `file_size`；`upload.done`，字段 `elapsed_ms`
- `submit.done`，字段 `provider_task_id`。这是服务商返回的任务 ID，不属于密钥，排查时需要它
- `poll.done`，字段 `elapsed_ms`、`outcome`
- `job.succeeded` 或 `job.failed`（`code`）

**禁止**记录：`audio_url`（带预签名）、`object_key` 以外的 TOS 信息、凭证、请求体和响应体。

## 4. 补记被吞掉的错误（R4）

| 位置 | 级别 | 内容 |
|---|---|---|
| `transcription_service.rs:283` 拿不到 AppState | error | `"app state unavailable"`，并带上 job_id。作业会一直停在 running，必须能在日志里查到 |
| `transcription_service.rs:286` 创建 HTTP 客户端失败 | error | 记录错误码，**并且**把作业标记为失败（现在是直接 return）。这是顺带修的一个行为缺陷，仍然限定在 R4 范围内 |
| `let _ = mark_job_failed(...)` 共 5 处 | error | 原始错误码，以及标记失败本身的错误码 |
| `transcription_service.rs:376` TOS 删除失败 | warn | 只记录 `object_key` 和错误码 |
| `lib.rs:39-41` 三个迁移函数 | 由迁移函数内部记录 | 迁移函数现在返回 `()`，需要在函数内部失败的分支补 `warn` |
| `recording_service` 转码失败、录音 worker 的回复通道断开 | warn | 错误码 |
| `ffmpeg_service` 下载失败 | warn | 阶段名和错误码 |

## 5. 打开日志文件夹（R7）

- 新增命令 `logs_open_dir(app: AppHandle) -> CmdResult<()>`：先 `create_dir_all`，再调用 `tauri_plugin_opener::OpenerExt::opener().open_path(dir, None::<&str>)`。从 Rust 端调用 opener 不受前端 capability 限制，所以不需要新增权限
- 前端在 `src/ipc/commands/logs.ts` 新增 `logsOpenDir()`，从 `ipc/index.ts` 导出，并补一个测试（照 `health.test.ts` 的写法）
- 在 `SettingsAbout.tsx` 的"关于"区块里加一个次要按钮"打开日志文件夹"，失败时用 `friendlyErrorMessage` 提示
- 这条是**新增**命令，不影响父任务"不改动现有 IPC 契约"的约束

## 6. 规范更新（R6）

把 `logging-guidelines.md` 从占位说明改写成正式规范：初始化位置、日志目录、级别约定、允许和禁止记录的字段清单、`log_cmd` 的用法、span 命名约定。

## 7. 回滚

日志属于纯增量改动。初始化失败会自动降级，不影响功能。需要回滚时，删掉依赖、初始化代码和 `.log_cmd` 调用即可。
