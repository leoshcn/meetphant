# 摘要 LLM 可配置服务商 / Base URL / API Key / 模型

## Goal

让用户在设置中自行指定会议摘要所用的大语言模型——服务商、Base URL、API Key、模型名——而不是写死 DashScope + `qwen3.7-plus`。用户可以换用更便宜/更快/公司指定的模型或中转服务，且已有 DashScope 用户升级后零配置继续可用。

## Background (current behavior, evidence)

- 摘要客户端写死 DashScope OpenAI 兼容端点与模型：`src-tauri/src/providers/qwen/client.rs:11`（`CHAT_COMPLETIONS_URL`）、`:15`（`MODELS_URL`）、`:18`（`MODEL_ID = "qwen3.7-plus"`）。
- 请求体为 OpenAI Chat Completions 格式，带 `response_format: json_object` 与 DashScope 专有 `enable_thinking: false`（`client.rs` `build_chat_body`）；解析已容忍 ```json 代码围栏（`parse_summary_json`）。
- 连通性测试 `GET /models`：`HttpQwenClient::list_models`；IPC `settings_test_dashscope`（`src-tauri/src/commands/settings.rs:75`），带临时覆盖、不落库，同 `settings_test_tos`（`:50`）。
- 密钥只存 OS keyring（service `meetphant`，account `dashscope_api_key`，含 legacy `meetly` 迁移）：`src-tauri/src/services/credentials.rs:160-162, 200`；IPC 仅回传 `dashscope_configured`：`src-tauri/src/models/settings.rs`。
- 摘要服务取密钥：`src-tauri/src/services/summary_service.rs:121` `require_dashscope_credentials()`；`SummaryGenerator` trait 以 `DashScopeCredentials` 为参数，测试用 Stub（同文件 `:173`）。
- settings 表加列已有幂等模式：`src-tauri/src/db/pool.rs:43` `ensure_tos_settings_columns`。
- UI：`src/features/settings-credentials/SettingsCredentials.tsx`（DashScope Key 保存/清除/测试）；`src/features/meeting-summary/MeetingSummary.tsx:300-303`「去设置配置 DashScope」错误提示。
- 契约文档：`.trellis/spec/backend/api-shape.md:24,28,79,99,124,174-175,185`；用户文档 `docs/credentials-guide.zh-CN.md`。
- 历史：`07-28-summary-system-prompt-settings` 已撤回——系统提示词保持内置。

## Decisions

| ID | Decision |
|----|----------|
| D1 | 仅支持 OpenAI 兼容协议（Chat Completions）。服务商 = 预设（DashScope、DeepSeek、OpenAI、Moonshot、智谱、豆包方舟）+「自定义（OpenAI 兼容）」；选预设自动填 Base URL，可改；厂商专有参数（如 `enable_thinking`）只对对应预设附加。（用户确认 2026-10-09） |
| D2 | 只存一套：服务商 + Base URL + 模型（SQLite）+ 一把 Key（keyring）。切换服务商时 Base URL 换为该预设默认值，Key/模型需重新填写。（用户确认 2026-10-09） |
| D3 | 模型名手填；选预设时预填推荐模型，可改；不做「拉取模型列表」。（用户确认 2026-10-09） |
| D4 | 「测试连接」= 用当前配置的模型发一次最小 chat completion（`max_tokens` 极小），一次性校验 URL + Key + 模型名；不再依赖 `/models`（部分服务商/中转没有）。（规划建议，随最终摘要确认） |
| D5 | 切换服务商或修改 Base URL 后，旧 Key 不会被发往新地址：UI 清空 Key 输入并要求重新填写后才能保存/测试。（规划建议，安全默认，随最终摘要确认） |

## Requirements

- R1 设置「转写与摘要」凭据区提供：服务商下拉、Base URL、API Key（write-only，已配置时掩码）、模型名；保存 / 清除 / 测试连接。
- R2 API Key 只存 OS keyring，永不经 IPC、日志、错误信息回传；服务商/Base URL/模型存 SQLite settings 单行。
- R3 升级兼容：已存在的 DashScope Key 自动沿用；未配置过的新字段默认 服务商=DashScope、Base URL=`https://dashscope.aliyuncs.com/compatible-mode/v1`、模型=`qwen3.7-plus`，行为与升级前一致。
- R4 摘要生成按配置拼接 `{base_url}/chat/completions`（容忍末尾 `/`），`model` 取配置值；`enable_thinking` 仅 DashScope 附加；`response_format: json_object` 仅对已知支持的预设发送，「自定义」不发送（依赖提示词 + 围栏剥离）。
- R5 校验：Base URL 必须 `http(s)://` 开头且非空；模型名非空；不合法时 `settings_update` 拒绝且不写入。
- R6 错误文案不再写死「DashScope」，改为通用「摘要模型服务」并带服务商名；摘要页未配置提示跳转设置。
- R7 更新 `api-shape.md` 与 `docs/credentials-guide.zh-CN.md`。

## Acceptance Criteria

- AC1（R3）带旧 DashScope Key 的数据库/keyring 升级后，不改任何设置即可生成摘要，请求发往 DashScope、模型 `qwen3.7-plus`。
- AC2（R1,R2,R4）将服务商设为 DeepSeek、填 Key 与模型后生成摘要，请求 URL = `<base_url>/chat/completions`、`model` = 所填值、无 `enable_thinking`。
- AC3（R4）「自定义」+ 任意 Base URL（如本地 `http://localhost:11434/v1`）可生成摘要，请求体无 `response_format`、无 `enable_thinking`。
- AC4（R2）`settings_get` 返回值与日志中不出现 Key；只出现 `summary_llm_configured` 布尔。
- AC5（D4）测试连接：Key 错 → 「Key 无效或无权限」；模型名错/URL 错 → 可区分的失败提示（含 HTTP 状态码）；成功 → 通过。测试不落库。
- AC6（D5）切换服务商后 Key 输入被清空，未重新填写前保存/测试按钮禁用或提示。
- AC7（R5）Base URL 为 `ftp://x` 或空、模型名为空时保存被拒，数据库不变。
- AC8 `cargo test`、前端单测、类型检查、lint 全部通过；新增后端单测覆盖 URL 拼接、预设参数差异、迁移默认值。

## Out of Scope

- 原生 Anthropic Messages / Gemini 等非 OpenAI 协议（D1）。
- 多套配置 / 每服务商分别保存（D2）。
- 拉取模型列表下拉（D3）。
- 自定义系统提示词（07-28 已撤回）。
- ASR（豆包）与 TOS 配置。
- 温度、max_tokens 等高级参数。

## Deferred / Risks

- 预设 Base URL 与推荐模型名需在实现时对照各厂商官方文档核实；无法确认推荐模型的预设留空模型名（用户必填）。
- 部分 OpenAI 兼容服务对 `response_format` 支持不一；若某预设实测不支持，从该预设能力表中移除即可。
