# Implement plan

1. Backend model & DB
   - `models/settings.rs`：字段改名/新增（见 design Contracts）。
   - `db/pool.rs`：`ensure_summary_llm_columns`；`settings_service` 读写新列 + 校验（R5）+ 有效值解析。
2. Credentials
   - `services/credentials.rs`：`SummaryLlmCredentials`，account `summary_llm_api_key`，从 `dashscope_api_key` 复制迁移（保留旧项）。
3. Provider
   - `providers/qwen` → `providers/openai_compat`（或保留目录、重命名类型）；`presets.rs`；`LlmConfig`；URL 拼接；按预设构造 body；`test_connection` 最小 chat 请求。
4. Service & commands
   - `summary_service` 组装 `LlmConfig`；`commands/settings.rs` 改名 test/clear 命令；`lib.rs` 注册。
5. Frontend
   - `src/ipc/types.ts`、`commands/settings.ts`、`index.ts` 同步；`SettingsCredentials.tsx` 新表单（下拉、URL、Key、模型、D5 清空逻辑）；`MeetingSummary.tsx:300` 文案。
6. Docs/spec：`api-shape.md`、`docs/credentials-guide.zh-CN.md`。
7. Tests：URL 拼接（有/无末尾 `/`）、dashscope vs custom body 差异、迁移默认值、校验拒绝不写库、Key 不出现在 `Settings`；前端 ipc 测试更新。

## Validation

```
cd src-tauri && cargo test && cargo clippy -- -D warnings
npm run lint && npm run typecheck && npm test
```
（以 package.json 实际脚本为准。）

## Risky points

- keyring 迁移（勿丢 Key；保留旧项）。
- IPC 改名需前后端同步，grep `dashscope` 确认无遗留。
