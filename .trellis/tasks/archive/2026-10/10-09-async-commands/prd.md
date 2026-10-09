# IO 命令异步化 + 网络请求期间不持锁

## Goal

用户在生成摘要、停止录音（FFmpeg 转码）、测试凭证时，界面保持可操作：窗口不出现"未响应"，侧栏和设置等其他操作不需要排队等待。

## Confirmed Facts（代码证据）

- `lib.rs:83-115` 注册的 29 条命令全部是同步 `fn`。在 Tauri 2 中，同步命令运行在主线程上。
- 耗时命令：
  - `summary_generate`（`commands/summary.rs:9`）：同步调用千问，HTTP 超时 120 秒（`providers/qwen/client.rs:160`），**整个过程持有 DB 锁**（`summary_service.rs:98` 的 `generate_summary` 一直拿着 `&Connection`）
  - `record_stop`（`commands/recording.rs:32`）：等录音 worker 回复，worker 内部同步执行 FFmpeg 转码（`recording_service.rs:497` 的 `try_encode_wav_to_m4a`），耗时随录音时长增长
  - `settings_test_doubao` / `settings_test_tos` / `settings_test_dashscope`（`settings_test_service.rs:119/127/148`）：发网络探测请求。`test_tos` 还需要读 DB 来合并配置
  - `record_list_input_devices`、`record_start`：cpal 枚举或打开设备，在 Windows 上可能需要几百毫秒
- `ffmpeg_download` 已经在后台线程下载（`ffmpeg_service.rs:224`），立即返回
- `jobs_start_transcription` 已经把上传和轮询放到后台线程，并且在网络请求期间释放了锁（`transcription_service.rs:279`）。可以作为"读上下文 → 释放锁 → 网络请求 → 重新拿锁写回"写法的参考
- 前端在 `MeetingSummary.tsx:200` await `summaryGenerate`，在 `AppShell.tsx:189` 和 `MeetingRecording.tsx:252` await `recordStop`，期间已经有忙碌状态

## Requirements

- R1：会发网络请求、调用子进程（FFmpeg）、操作音频设备或读取音频文件的命令都不能在主线程执行，具体清单见 design 第 2 节。只在毫秒级时间内访问 SQLite 的纯 DB 命令，以及 `record_status`，保持同步
- R2：`summary_generate` 调用大模型期间不持有 DB 锁，改成"短暂加锁读取 → 释放锁 → 调用大模型 → 短暂加锁写回"
- R3：`settings_test_tos` 只在合并配置时短暂持锁，网络探测期间不持锁
- R4：IPC 契约不变：命令名、参数、返回结构、错误码都保持现状
- R5：现有 `cargo test` 全部通过；`generate_summary` 的可测试性（注入 `SummaryGenerator`）保持不变，线上和测试走同一个函数
- R6：摘要生成期间如果会议被删除，写回时返回 `NOT_FOUND`，不留下孤立的摘要记录（外键没有生效，见 design 第 3 节）

## Acceptance Criteria

- [ ] 生成摘要期间，可以切换会议、打开设置页，侧栏列表能正常刷新，这些操作都不需要等摘要生成完
- [ ] 停止一段较长录音时，窗口可以拖动，也不出现"未响应"
- [ ] 凭证测试期间，窗口不出现"未响应"
- [ ] 代码中不存在"持有 `state.db` 锁时发起 HTTP 请求"的路径（代码审查确认）
- [ ] `cargo test` / `clippy -D warnings` 通过；新增单元测试，证明摘要生成调用 generator 时 DB 锁没有被持有
- [ ] 新增单元测试：生成期间会议被删除时返回 `NOT_FOUND`，并且 summaries 表为空

## Key Decisions

- D1（2026-10-09，用户确认）：`record_stop` 在本迭代**只移出主线程**，接口不变；停止按钮仍然要等转码完成。"立即返回 + 后台转码 + 进度事件"放到第 2 个迭代，和作业事件推送一起做。

## Out of Scope

- `record_stop` 后台转码和进度事件（D1，第 2 个迭代）
- 事件推送替代轮询（第 2 个迭代）
- 引入连接池或 Repository 层（中期）
