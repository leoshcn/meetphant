# Design: IO 命令异步化 + 网络请求期间不持锁

## 1. 执行模型

### 为什么不直接加 `#[tauri::command(async)]`

在 tauri-macros 2.6.3 中，`async` 修饰同步函数时，生成的代码通过 `respond_async` → `async_runtime::spawn` 执行（见 `tauri-2.11.5/src/ipc/mod.rs:324`）。也就是说，同步函数会**直接跑在 tokio worker 线程上**。这样有两个问题：

1. 120 秒的阻塞调用会占住 worker 线程，几个并发请求就可能把运行时饿死
2. 项目用的是 `reqwest::blocking`。在 tokio 的 async 上下文里创建或销毁 blocking client 会 panic（"Cannot drop a runtime in a context where blocking is not allowed"）

### 采用的写法

```rust
#[tauri::command(rename_all = "snake_case")]
pub async fn summary_generate(app: AppHandle, meeting_id: String, language: String) -> CmdResult<Summary> {
    run_blocking(move || {
        let state = app.state::<AppState>();
        services::generate_summary_http(&state.db, &meeting_id, &language)
    }).await
}
```

新增 `src-tauri/src/commands/mod.rs` 里的公共辅助函数：

```rust
pub(crate) async fn run_blocking<T, F>(f: F) -> CmdResult<T>
where F: FnOnce() -> CmdResult<T> + Send + 'static, T: Send + 'static
{
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|_| AppErrorDto::internal("Background task failed"))?
}
```

- `State<'_>` 带生命周期，不能移进 `'static` 闭包，所以改为传入 `AppHandle`，在闭包里调用 `app.state::<AppState>()`
- `spawn_blocking` 的线程不在 async 上下文中，`reqwest::blocking` 可以安全使用
- JoinError（闭包 panic）映射为 `INTERNAL`，错误信息固定，不透传 panic 内容

## 2. 改造范围

| 命令 | 改造 | 原因 |
|---|---|---|
| `summary_generate` | async + run_blocking + 拆锁（见第 3 节） | 网络请求最长 120 秒，且持有 DB 锁 |
| `settings_test_doubao` / `settings_test_dashscope` | async + run_blocking | 网络请求 |
| `settings_test_tos` | async + run_blocking；只在 `merge_tos_config` 期间短暂持锁 | 网络请求 + 读 DB |
| `record_stop` | async + run_blocking（D1：接口不变） | 要等 FFmpeg 转码 |
| `record_start` / `record_list_input_devices` | async + run_blocking | cpal 设备操作 |
| `meetings_create_from_file` / `meetings_attach_source` / `jobs_start_transcription` | async + run_blocking | 文件 IO（metadata），并且要拿 DB 锁 |
| 其余纯 DB 命令（list/get/rename/delete/settings_get/update 等） | **不改** | 毫秒级；锁竞争在第 3 节解决后可以忽略；改了只会增加改动面 |
| `record_status` | **不改** | 只是和 worker 线程一来一回的 channel 通信，耗时在微秒级，而且每 250ms 轮询一次，放进线程池反而增加开销 |
| `ffmpeg_download` / `ffmpeg_status` / tray / health | **不改** | 本来就立即返回 |

纯 DB 命令为什么可以留在主线程：第 3 节解决之后，DB 锁的最长持有时间降到毫秒级，主线程等锁的时间可以忽略。

## 3. `summary_generate` 拆锁

把 `generate_summary(conn: &Connection, …, generator)` 改成接收 `db: &Mutex<Connection>`，内部分三段：

```
{ lock → 校验语言 → get_meeting → require_transcript → get_settings → unlock }
credentials::require_dashscope_credentials()   // 钥匙串，不需要锁
generator.generate(...)                        // 不持锁
{ lock → upsert_summary → unlock }
```

- 只保留**一个**函数，线上和测试都走这条路径。不要再出现转写那样"测试一份、线上一份"的情况
- 现有 5 个单元测试改成用 `Mutex::new(open_memory()?)` 构造参数，断言内容不变
- 新增测试：`LockProbeGenerator` 在 `generate()` 里对同一个 Mutex 调用 `try_lock()`，并断言成功。这能证明调用大模型期间没有持锁。测试模式下凭证存在 thread-local 里，`try_lock` 也在同一线程上执行，所以这个测试可以成立
- 并发语义：释放锁之后，会议可能被删除。`summaries.meeting_id` 虽然声明了 `REFERENCES meetings(id)`（`003_summaries.sql`），但代码里从来没有执行 `PRAGMA foreign_keys = ON`，**外键实际不生效**，所以直接写入会留下一条孤立摘要。因此写回段要在同一次加锁内先调用 `get_meeting`，会议不存在就返回 `NOT_FOUND`，不写入。新增一个测试覆盖这种情况：stub generator 在 `generate()` 里删除会议，然后断言返回 `NOT_FOUND`，且 summaries 表为空
- 转写流程里也有同样的竞态（转写期间会议被删除）；外键不生效这个问题本身留给第 2 个迭代的迁移改造处理，本任务不处理

## 4. `settings_test_tos`

`test_tos(conn, …)` 改成 `test_tos(db: &Mutex<Connection>, …)`：先在锁内执行 `merge_tos_config`，释放锁后再调用 `head_bucket`。现有测试同样改成用 Mutex 包装。

## 5. 兼容性

- 命令名、参数名（仍然是 `rename_all = "snake_case"`）、返回类型、错误码都不变，前端零改动
- `async fn` 命令如果有引用参数，必须返回 `Result`。改造后的命令都只接收 `AppHandle` 和拥有所有权的参数，也都返回 `CmdResult`，满足这个要求
- `lib.rs` 的 `invoke_handler` 列表不变

## 6. 回滚

每条命令的改造彼此独立，可以逐条回退。`summary_service` 的签名改动只影响 `commands/summary.rs` 和它自己的测试。
