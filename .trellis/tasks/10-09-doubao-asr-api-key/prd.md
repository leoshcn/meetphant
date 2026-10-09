# 豆包 ASR 改用新版 API Key 鉴权 + 录音文件识别模型 2.0

## Goal

1. 鉴权方式从旧版控制台的 App Id + Access Token，**完全替换**为新版控制台的单个 API Key（请求头 `X-Api-Key`）。**已实现。**
2. 识别模型从 1.0 升到 **豆包录音文件识别模型 2.0**（`volc.seedasr.auc`）。2.0 只有标准版（submit/query），所以去掉极速版，**所有文件都上传 TOS 后走 2.0 标准版**。（2026-10-09 用户决定）

## Background

### Official API (verified 2026-10-09)
- 2.0 标准版文档：https://docs.volcengine.com/docs/DoubaoVoice/task-submission-http-1?lang=zh
  - `POST https://openspeech.bytedance.com/api/v3/auc/bigmodel/submit`，请求头 `X-Api-Key`、`X-Api-Resource-Id`、`X-Api-Request-Id`、`X-Api-Sequence: -1`。
  - `X-Api-Resource-Id`：`volc.seedasr.auc` 对应 2.0，`volc.bigasr.auc` 对应 1.0（历史版本）。
  - `audio.url` 必选，文档未列出 `audio.data`。`request.model_name` 仍为 `bigmodel`。音频 ≤512 MB、≤5 小时。
  - 响应头有 `X-Api-Status-Code`、`X-Api-Message`、`X-Tt-Logid`。结果通过配套的 query 接口（`.../bigmodel/query`）获取。
- 极速版 `.../recognize/flash` 只有 `volc.bigasr.auc_turbo`（1.0），没有对应的 2.0 资源。
- API Key 鉴权说明：https://www.volcengine.com/docs/6561/2119699

### Current implementation (after part 1)
- 鉴权已经统一走 `providers/doubao/auth.rs`，凭证为 `DoubaoCredentials { api_key }`，前后端接口字段为 `doubao_api_key`（见 design.md）。
- 双路径：`transcription_service.rs:147-150, 284-302, 351-406` 按 `FLASH_MAX_AUDIO_BYTES`（20 MiB，`meeting_service.rs:14`）分流，≤20 MiB 走 `flash_client.rs`，否则走 TOS + `async_client.rs`。
- 测试连接：`settings_test_service.rs:122-123` 调用 `HttpFlashClient::probe_connection`，即用极速版跑一段 WAV。
- 标准版 Resource Id：`async_client.rs:16`（`volc.bigasr.auc`）。
- 前端文案里写着“≤20 MiB 极速”：`SettingsCredentials.tsx:482`，`TranscriptionImport.tsx:355`，`MeetingRecording.tsx:401`。`formatError.ts:4` 中 `TOS_NOT_CONFIGURED` 的提示写的是“大文件转写需要 TOS”。

## Requirements

已完成（part 1）：
- R1 Provider 只发送 `X-Api-Key`。
- R2 keyring 账户为 `meetphant/doubao_api_key`，启动时清理旧版 App Id / Token。
- R3 前后端接口字段为 `doubao_api_key`。
- R4 设置页只保留一个 API Key 输入框。
- R5 文档已改为 API Key 方式。
- R6 测试已更新。

新增（part 2，2.0）：
- R7 标准版 `X-Api-Resource-Id` 改为 `volc.seedasr.auc`。
- R8 删除极速版路径：去掉 `flash_client.rs` 以及 `FlashRecognizer` / `HttpFlashClient` / `FLASH_MAX_AUDIO_BYTES`，所有 ≤512 MiB 的文件都走 TOS 上传、预签名 URL、2.0 submit/query。没配 TOS 时，任何大小的文件都返回 `TOS_NOT_CONFIGURED`。`audio_format_from_path` 与 `asr_parse` 保留（迁到 async 或共用模块）。
- R9 “测试连接”改为不依赖 TOS、不消耗识别额度的鉴权探测：向 2.0 query 接口发一个随机 Request-Id，用响应判断 Key 和 2.0 资源权限是否有效（判定规则见 design.md）。
- R10 文案：设置页、导入页、录音页、`TOS_NOT_CONFIGURED` 的错误提示、credentials guide（改为开通 2.0、TOS 变为必需、自检表）、README / README.zh-CN（需要 TOS 的场景和大小阈值表）、`api-shape.md` / `quality-guidelines.md` 中关于双路径的描述，全部改为“转写需要豆包 API Key + TOS”。
- R11 测试：删除 flash 相关测试，把“≤20 MiB 不需要 TOS”的用例改为“任意大小没配 TOS 都报 TOS_NOT_CONFIGURED”，新增 Resource Id 断言和探测判定的单测。

## Acceptance Criteria

- [ ] AC1（R1）：单测断言请求头只带 `X-Api-Key`。**已完成（`auth.rs`）。**
- [ ] AC2（R2）：只保存 API Key 后显示已配置，清除后显示未配置，空 Key 被拒绝。
- [ ] AC3（R2）：已有旧凭证的用户升级后显示未配置，转写报 `asr_not_configured`，旧条目被删除。
- [ ] AC5（R5/R10）：代码和文档里没有残留的豆包 App Id / Access Token、极速版、“≤20 MiB 不需要 TOS”等描述。
- [ ] AC6：`cargo test --no-run`、`tsc --noEmit`、vitest 通过。`cargo test` 和 clippy 的环境问题、既有告警都在报告中如实说明。
- [ ] AC7（R7）：单测断言标准版 Resource Id 为 `volc.seedasr.auc`，代码中不再出现 `volc.bigasr.*`。
- [ ] AC8（R8）：没配 TOS 时，导入 1 MiB 音频返回 `TOS_NOT_CONFIGURED`；配了 TOS 时，1 MiB 和 >20 MiB 音频都走 TOS + 2.0 并产出转写（stub 单测 + 手动验证）。
- [ ] AC9（R9）：用有效的 API Key 测试连接显示“连接正常”，用无效 Key 显示鉴权失败；测试连接不需要 TOS。（手动验证判定规则）

## Out of Scope

- 保留旧版 App Id + Token，或保留 1.0 / 极速版作为回退。
- 未在文档中说明的 `audio.data` base64 直传。
- 流式 ASR，以及 2.0 新增的情绪、语种等可选参数（请求体保持现有字段不变）。
- DashScope 鉴权。TOS 鉴权和上传逻辑本身不改。
