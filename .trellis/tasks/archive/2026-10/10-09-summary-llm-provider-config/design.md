# Design — 摘要 LLM 可配置

## Boundaries

```
SettingsCredentials.tsx ──settings_update / settings_test_summary_llm──▶ commands/settings.rs
                                                                         │
                     settings_service (SQLite: provider, base_url, model)│ credentials (keyring: key)
                                                                         ▼
summary_service::generate_summary ──▶ LlmConfig ──▶ providers/openai_compat (原 qwen) HttpChatClient
```

## Data

- SQLite `settings` 新增列（幂等 `ensure_summary_llm_columns`，仿 `pool.rs:43`）：
  - `summary_llm_provider TEXT NOT NULL DEFAULT 'dashscope'`
  - `summary_llm_base_url TEXT NOT NULL DEFAULT ''`（空 = 用预设默认）
  - `summary_llm_model TEXT NOT NULL DEFAULT ''`（空 = 用预设推荐）
- Keyring：新 account `summary_llm_api_key`；启动迁移：若新 account 空且 `dashscope_api_key`（含 legacy `meetly`）存在 → 复制后删除旧 account（沿用 `migrate_legacy` 模式）。

## Provider presets（后端单一真源，前端经 IPC 或常量镜像）

```rust
pub struct ProviderPreset { id, label, default_base_url, default_model: Option<&str>,
                            json_mode: bool, extra_body: fn(&mut Value) }
```

`dashscope`（json_mode, `enable_thinking:false`）、`deepseek`、`openai`、`moonshot`、`zhipu`、`ark`、`custom`（json_mode=false, 无默认 URL/模型）。预设 URL/模型实现时核实官方文档。

## Contracts (IPC, snake_case)

- `Settings`：`dashscope_configured` → `summary_llm_configured`；新增 `summary_llm_provider`、`summary_llm_base_url`（已解析的有效值）、`summary_llm_model`（有效值）。
- `SettingsUpdate`：`dashscope_api_key` → `summary_llm_api_key`；新增 `summary_llm_provider/base_url/model`。
- `settings_clear_dashscope_credentials` → `settings_clear_summary_llm_credentials`（只清 Key）。
- `settings_test_dashscope` → `settings_test_summary_llm { api_key?, provider?, base_url?, model? }`，合并已存配置，POST 最小 chat completion（`max_tokens: 1`，`messages:[{user:"ping"}]`），不落库。
- 错误码沿用 `SUMMARY_PROVIDER_ERROR`；文案通用化并带服务商 label。

## Client

`SummaryGenerator::generate(&self, cfg: &LlmConfig, input)`，`LlmConfig { provider, base_url, model, api_key }`。URL = `base_url.trim_end_matches('/') + "/chat/completions"`。`build_chat_body(cfg, input)` 依预设决定 `response_format` 与 extra body。提示词构建保持不变。

## Compatibility

- 旧安装：列默认值 + keyring 迁移 ⇒ 行为与升级前一致（AC1）。
- 单进程桌面应用，IPC 改名前后端同次发布，无需兼容旧名。

## Rollback

纯增量列 + keyring 复制；回滚版本仍读 `dashscope_api_key` —— 因此迁移**先复制、验证读回成功后再删除旧 account**；若担心回滚，可保留旧 account 不删（实现时选保留，代价仅一条冗余 keyring 项）。
