//! OpenAI-compatible (Chat Completions) client for structured meeting summaries.
//!
//! The endpoint, model, and key come from user settings (`LlmConfig`); vendor
//! differences are captured by `presets::ProviderPreset`.

use std::fmt;

use serde::Deserialize;
use serde_json::{json, Value};

use super::presets::{resolve_preset, ProviderPreset};
use crate::error::{AppErrorDto, CmdResult};
use crate::models::SummaryContent;

/// Effective summary LLM configuration for one request.
///
/// `Debug` redacts `api_key`; never log this struct's key or format it into errors.
#[derive(Clone, PartialEq, Eq)]
pub struct LlmConfig {
    /// Preset id (`dashscope`, `deepseek`, …, `custom`).
    pub provider: String,
    /// OpenAI-compatible base URL, e.g. `https://api.deepseek.com/v1`.
    pub base_url: String,
    pub model: String,
    pub api_key: String,
}

impl fmt::Debug for LlmConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LlmConfig")
            .field("provider", &self.provider)
            .field("base_url", &self.base_url)
            .field("model", &self.model)
            .field("api_key", &"<redacted>")
            .finish()
    }
}

impl LlmConfig {
    pub fn preset(&self) -> &'static ProviderPreset {
        resolve_preset(&self.provider)
    }

    /// Provider label for user-facing messages.
    pub fn label(&self) -> &'static str {
        self.preset().label
    }

    pub fn chat_completions_url(&self) -> String {
        chat_completions_url(&self.base_url)
    }
}

/// `{base_url}/chat/completions`, tolerating surrounding whitespace and trailing `/`.
pub fn chat_completions_url(base_url: &str) -> String {
    format!("{}/chat/completions", base_url.trim().trim_end_matches('/'))
}

#[derive(Debug, Clone)]
pub struct SummaryGenerateInput {
    pub transcript: String,
    pub context_text: String,
    pub language: String,
}

fn system_prompt_for_language(language: &str) -> &'static str {
    match language {
        "en" => concat!(
            "You are a meeting-notes assistant. Based on the transcript and optional context, ",
            "output a structured summary in English only. ",
            "Even if the transcript or context is Chinese (or any other language), ",
            "every string value in key_points, action_items, and decisions must be English — ",
            "translate as needed; do not leave Chinese text in those arrays. ",
            "You must return a JSON object with fields key_points, action_items, decisions; ",
            "each field is an array of strings. Use an empty array when a section has no content."
        ),
        "zh-en" => concat!(
            "你是会议纪要助手。根据转写文本与可选上下文，输出中英文双语结构化摘要。",
            "必须返回一个 JSON 对象，字段为 key_points、action_items、decisions，",
            "每个字段是字符串数组。",
            "每个字符串条目必须同时包含简体中文与对应英文：先写完整中文段落，再换行空一行，",
            "然后写对应的英文段落（英文须独立成段，不要写在同一行，也不要使用「中文 / English」斜杠并列）。",
            "示例：\"讨论了发布节奏。\\n\\nDiscussed the release cadence.\"。",
            "某块没有内容时返回空数组。"
        ),
        _ => concat!(
            "你是会议纪要助手。根据转写文本与可选上下文，输出简体中文结构化摘要。",
            "必须返回一个 JSON 对象，字段为 key_points、action_items、decisions，",
            "每个字段是字符串数组。某块没有内容时返回空数组。"
        ),
    }
}

fn user_prompt_for_language(language: &str, context_section: &str, transcript: &str) -> String {
    match language {
        "en" => format!(
            "Generate the summary JSON from the materials below. \
             All key_points, action_items, and decisions strings must be English only \
             (translate from Chinese if needed).\n\n\
             [User context]\n{context_section}\n\n\
             [Meeting transcript]\n{transcript}"
        ),
        "zh-en" => format!(
            "请根据以下材料生成摘要 JSON。\
             每个条目先写完整中文段落，再空一行写对应英文段落；英文须独立成段，不要用「中文 / English」写在同一行。\n\n\
             【用户上下文】\n{context_section}\n\n【会议转写】\n{transcript}"
        ),
        _ => format!(
            "请根据以下材料生成摘要 JSON。\n\n【用户上下文】\n{context_section}\n\n【会议转写】\n{transcript}"
        ),
    }
}

fn empty_context_placeholder(language: &str) -> &'static str {
    match language {
        "en" => "(no extra context)",
        _ => "（无额外上下文）",
    }
}

/// Build system + user messages. Always mentions JSON (required by DashScope json_object mode).
pub fn build_summary_messages(input: &SummaryGenerateInput) -> Vec<Value> {
    let system = system_prompt_for_language(&input.language);

    let context_section = if input.context_text.trim().is_empty() {
        empty_context_placeholder(&input.language).to_string()
    } else {
        input.context_text.clone()
    };

    let user = user_prompt_for_language(&input.language, &context_section, &input.transcript);

    vec![
        json!({ "role": "system", "content": system }),
        json!({ "role": "user", "content": user }),
    ]
}

/// Summary request body. `response_format` only for presets known to support
/// JSON mode; vendor extensions (e.g. DashScope `enable_thinking`) per preset.
pub fn build_chat_body(cfg: &LlmConfig, input: &SummaryGenerateInput) -> Value {
    let preset = cfg.preset();
    let mut body = json!({
        "model": cfg.model.trim(),
        "messages": build_summary_messages(input),
    });
    if preset.json_mode {
        body["response_format"] = json!({ "type": "json_object" });
    }
    (preset.extra_body)(&mut body);
    body
}

/// Minimal chat completion used by「测试连接」: validates URL + key + model in one call.
pub fn build_test_body(cfg: &LlmConfig) -> Value {
    let preset = cfg.preset();
    let mut body = json!({
        "model": cfg.model.trim(),
        "messages": [{ "role": "user", "content": "ping" }],
    });
    body[preset.max_tokens_field] = json!(1);
    (preset.extra_body)(&mut body);
    body
}

/// Strip optional markdown fences and parse into SummaryContent.
pub fn parse_summary_json(content: &str) -> CmdResult<SummaryContent> {
    let trimmed = content.trim();
    let without_fence = strip_json_fence(trimmed);
    serde_json::from_str::<SummaryContent>(without_fence)
        .map_err(|_| AppErrorDto::summary_provider_error("Invalid summary JSON from provider"))
}

fn strip_json_fence(s: &str) -> &str {
    let s = s.trim();
    if let Some(rest) = s.strip_prefix("```json") {
        return rest.strip_suffix("```").unwrap_or(rest).trim();
    }
    if let Some(rest) = s.strip_prefix("```") {
        return rest.strip_suffix("```").unwrap_or(rest).trim();
    }
    s
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Option<Vec<ChatChoice>>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: Option<ChatMessage>,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    content: Option<String>,
}

/// Max characters of upstream error text surfaced to the UI.
const MAX_UPSTREAM_DETAIL_CHARS: usize = 200;

/// Extract a short upstream error message (`error.message` / `error` / `message`)
/// from a JSON error body. Non-JSON bodies (HTML pages etc.) yield `None`.
/// Any occurrence of the API key is redacted defensively.
fn upstream_error_detail(raw_body: &str, api_key: &str) -> Option<String> {
    let value: Value = serde_json::from_str(raw_body).ok()?;
    let text = value
        .get("error")
        .and_then(|e| {
            e.get("message")
                .and_then(Value::as_str)
                .or_else(|| e.as_str())
        })
        .or_else(|| value.get("message").and_then(Value::as_str))?
        .trim();
    if text.is_empty() {
        return None;
    }
    let mut text = text.to_string();
    let key = api_key.trim();
    if !key.is_empty() {
        text = text.replace(key, "***");
    }
    if text.chars().count() > MAX_UPSTREAM_DETAIL_CHARS {
        text = text
            .chars()
            .take(MAX_UPSTREAM_DETAIL_CHARS)
            .collect::<String>()
            + "…";
    }
    Some(text)
}

/// Map a non-success HTTP status to a user-safe `SUMMARY_PROVIDER_ERROR`.
///
/// 401/403 never include the upstream body (some vendors echo a masked key).
pub fn http_failure_error(label: &str, status: u16, raw_body: &str, api_key: &str) -> AppErrorDto {
    let details = json!({ "http_status": status });
    if status == 401 || status == 403 {
        return AppErrorDto::with_details(
            "SUMMARY_PROVIDER_ERROR",
            format!("摘要模型服务（{label}）API Key 无效或无权限 (HTTP {status})"),
            details,
        );
    }
    let hint = match status {
        404 => "接口地址或模型不存在，请检查 Base URL 与模型名",
        400 | 422 => "请求被拒绝，请检查模型名",
        429 => "请求过于频繁或额度不足",
        500..=599 => "服务端错误，请稍后重试",
        _ => "请求失败",
    };
    let mut message = format!("摘要模型服务（{label}）{hint} (HTTP {status})");
    if let Some(detail) = upstream_error_detail(raw_body, api_key) {
        message.push('：');
        message.push_str(&detail);
    }
    AppErrorDto::with_details("SUMMARY_PROVIDER_ERROR", message, details)
}

fn network_error(label: &str, err: &reqwest::Error) -> AppErrorDto {
    // Never forward reqwest Display (contains the full URL).
    let message = if err.is_timeout() {
        format!("摘要模型服务（{label}）请求超时，请检查 Base URL 与网络")
    } else {
        format!("无法连接摘要模型服务（{label}），请检查 Base URL 与网络")
    };
    AppErrorDto::summary_provider_error(message)
}

/// Trait so summary jobs can use a stub in tests.
pub trait SummaryGenerator: Send + Sync {
    fn generate(&self, cfg: &LlmConfig, input: &SummaryGenerateInput) -> CmdResult<SummaryContent>;
}

/// Blocking HTTP client for any OpenAI-compatible Chat Completions endpoint.
pub struct HttpChatClient {
    client: reqwest::blocking::Client,
}

impl HttpChatClient {
    /// Client for summary generation (long timeout: LLM output can be slow).
    pub fn new() -> CmdResult<Self> {
        Self::with_timeout(120)
    }

    /// Client for the connection test (short timeout).
    pub fn for_probe() -> CmdResult<Self> {
        Self::with_timeout(30)
    }

    fn with_timeout(secs: u64) -> CmdResult<Self> {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(secs))
            .build()
            .map_err(|_| AppErrorDto::internal("Failed to create HTTP client"))?;
        Ok(Self { client })
    }

    #[cfg(test)]
    fn from_client(client: reqwest::blocking::Client) -> Self {
        Self { client }
    }

    /// POST a body to `{base_url}/chat/completions`. Never logs key or body.
    fn post_chat(&self, cfg: &LlmConfig, body: &Value) -> CmdResult<(u16, String)> {
        let response = self
            .client
            .post(cfg.chat_completions_url())
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", cfg.api_key.trim()))
            .json(body)
            .send()
            .map_err(|err| network_error(cfg.label(), &err))?;
        let status = response.status().as_u16();
        let raw = response.text().map_err(|_| {
            AppErrorDto::summary_provider_error(format!(
                "摘要模型服务（{}）响应读取失败",
                cfg.label()
            ))
        })?;
        Ok((status, raw))
    }

    /// Connectivity test: one minimal chat completion with the configured model.
    /// Validates Base URL, API key, and model name together; persists nothing.
    pub fn test_connection(&self, cfg: &LlmConfig) -> CmdResult<()> {
        let (status, raw) = self.post_chat(cfg, &build_test_body(cfg))?;
        if !(200..300).contains(&status) {
            return Err(http_failure_error(cfg.label(), status, &raw, &cfg.api_key));
        }
        let looks_like_chat = serde_json::from_str::<Value>(&raw)
            .ok()
            .map(|v| v.get("choices").map(Value::is_array).unwrap_or(false))
            .unwrap_or(false);
        if !looks_like_chat {
            return Err(AppErrorDto::summary_provider_error(format!(
                "摘要模型服务（{}）返回的不是 OpenAI 兼容的 Chat Completions 响应，请检查 Base URL",
                cfg.label()
            )));
        }
        Ok(())
    }
}

impl SummaryGenerator for HttpChatClient {
    fn generate(&self, cfg: &LlmConfig, input: &SummaryGenerateInput) -> CmdResult<SummaryContent> {
        let body = build_chat_body(cfg, input);
        let (status, raw) = self.post_chat(cfg, &body)?;
        if !(200..300).contains(&status) {
            return Err(http_failure_error(cfg.label(), status, &raw, &cfg.api_key));
        }

        let parsed: ChatCompletionResponse = serde_json::from_str(&raw).map_err(|_| {
            AppErrorDto::summary_provider_error("Invalid summary response envelope")
        })?;

        let content = parsed
            .choices
            .and_then(|mut c| c.pop())
            .and_then(|c| c.message)
            .and_then(|m| m.content)
            .unwrap_or_default();

        parse_summary_json(&content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread::JoinHandle;

    fn cfg(provider: &str, base_url: &str, model: &str) -> LlmConfig {
        LlmConfig {
            provider: provider.into(),
            base_url: base_url.into(),
            model: model.into(),
            api_key: "sk-test-secret".into(),
        }
    }

    fn dashscope() -> LlmConfig {
        cfg(
            "dashscope",
            "https://dashscope.aliyuncs.com/compatible-mode/v1",
            "qwen3.7-plus",
        )
    }

    fn zh_input(transcript: &str, context: &str) -> SummaryGenerateInput {
        SummaryGenerateInput {
            transcript: transcript.into(),
            context_text: context.into(),
            language: "zh-CN".into(),
        }
    }

    #[test]
    fn chat_url_joins_with_and_without_trailing_slash() {
        assert_eq!(
            chat_completions_url("https://api.deepseek.com/v1"),
            "https://api.deepseek.com/v1/chat/completions"
        );
        assert_eq!(
            chat_completions_url("https://api.deepseek.com/v1/"),
            "https://api.deepseek.com/v1/chat/completions"
        );
        assert_eq!(
            chat_completions_url("  http://localhost:11434/v1//  "),
            "http://localhost:11434/v1/chat/completions"
        );
        assert_eq!(
            dashscope().chat_completions_url(),
            "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions"
        );
    }

    #[test]
    fn debug_redacts_api_key() {
        let printed = format!("{:?}", dashscope());
        assert!(!printed.contains("sk-test-secret"));
        assert!(printed.contains("<redacted>"));
    }

    #[test]
    fn prompt_includes_context_when_present() {
        let body = build_chat_body(&dashscope(), &zh_input("讨论了路线图", "产品周会"));
        let messages = body["messages"].as_array().unwrap();
        let user = messages[1]["content"].as_str().unwrap();
        assert!(user.contains("产品周会"));
        assert!(user.contains("讨论了路线图"));
        let system = messages[0]["content"].as_str().unwrap();
        assert!(system.to_ascii_lowercase().contains("json"));
        assert!(system.contains("简体中文"));
    }

    #[test]
    fn dashscope_body_has_json_mode_and_enable_thinking() {
        let body = build_chat_body(&dashscope(), &zh_input("t", ""));
        assert_eq!(body["model"], "qwen3.7-plus");
        assert_eq!(body["response_format"]["type"], "json_object");
        assert_eq!(body["enable_thinking"], false);
    }

    #[test]
    fn deepseek_body_uses_configured_model_without_enable_thinking() {
        let body = build_chat_body(
            &cfg("deepseek", "https://api.deepseek.com/v1", " my-model "),
            &zh_input("t", ""),
        );
        assert_eq!(body["model"], "my-model");
        assert_eq!(body["response_format"]["type"], "json_object");
        assert!(body.get("enable_thinking").is_none());
    }

    #[test]
    fn custom_body_has_no_response_format_or_enable_thinking() {
        let body = build_chat_body(
            &cfg("custom", "http://localhost:11434/v1", "llama3"),
            &zh_input("t", ""),
        );
        assert_eq!(body["model"], "llama3");
        assert!(body.get("response_format").is_none());
        assert!(body.get("enable_thinking").is_none());
    }

    #[test]
    fn ark_body_has_no_response_format() {
        let body = build_chat_body(
            &cfg("ark", "https://ark.cn-beijing.volces.com/api/v3", "ep-1"),
            &zh_input("t", ""),
        );
        assert!(body.get("response_format").is_none());
        assert!(body.get("enable_thinking").is_none());
    }

    #[test]
    fn test_body_is_minimal_chat_completion() {
        let body = build_test_body(&dashscope());
        assert_eq!(body["model"], "qwen3.7-plus");
        assert_eq!(body["max_tokens"], 1);
        assert_eq!(body["messages"].as_array().unwrap().len(), 1);
        assert_eq!(body["enable_thinking"], false);
        assert!(body.get("response_format").is_none());

        let custom = build_test_body(&cfg("custom", "http://x/v1", "m"));
        assert_eq!(custom["max_tokens"], 1);
        assert!(custom.get("enable_thinking").is_none());

        let openai = build_test_body(&cfg("openai", "https://api.openai.com/v1", "m"));
        assert_eq!(openai["max_completion_tokens"], 1);
        assert!(openai.get("max_tokens").is_none());
    }

    #[test]
    fn prompt_works_with_empty_context() {
        let body = build_chat_body(&dashscope(), &zh_input("hello", "  "));
        let user = body["messages"][1]["content"].as_str().unwrap();
        assert!(user.contains("（无额外上下文）"));
        assert!(user.contains("hello"));
    }

    #[test]
    fn prompt_en_and_bilingual() {
        let en = build_chat_body(
            &dashscope(),
            &SummaryGenerateInput {
                transcript: "讨论了发布计划".into(),
                context_text: "".into(),
                language: "en".into(),
            },
        );
        let en_system = en["messages"][0]["content"].as_str().unwrap();
        let en_user = en["messages"][1]["content"].as_str().unwrap();
        assert!(en_system.contains("English only"));
        assert!(en_system.contains("translate"));
        assert!(en_user.contains("English only"));
        assert!(en_user.contains("[Meeting transcript]"));
        assert!(en_user.contains("(no extra context)"));
        assert!(en_user.contains("讨论了发布计划"));
        assert!(!en_user.contains("请根据以下材料"));

        let bi = build_chat_body(
            &dashscope(),
            &SummaryGenerateInput {
                transcript: "t".into(),
                context_text: "".into(),
                language: "zh-en".into(),
            },
        );
        assert!(bi["messages"][0]["content"]
            .as_str()
            .unwrap()
            .contains("独立成段"));
        assert!(!bi["messages"][0]["content"]
            .as_str()
            .unwrap()
            .contains("格式为「中文 / English」"));
        assert!(bi["messages"][1]["content"]
            .as_str()
            .unwrap()
            .contains("请根据以下材料"));
        assert!(bi["messages"][1]["content"]
            .as_str()
            .unwrap()
            .contains("空一行"));
    }

    #[test]
    fn parse_valid_json() {
        let content = r#"{"key_points":["a"],"action_items":[],"decisions":["d"]}"#;
        let parsed = parse_summary_json(content).unwrap();
        assert_eq!(parsed.key_points, vec!["a"]);
        assert!(parsed.action_items.is_empty());
        assert_eq!(parsed.decisions, vec!["d"]);
    }

    #[test]
    fn parse_failure_maps_to_provider_error() {
        let err = parse_summary_json("not-json").expect_err("bad");
        assert_eq!(err.code, "SUMMARY_PROVIDER_ERROR");
    }

    #[test]
    fn parse_strips_markdown_fence() {
        let content = "```json\n{\"key_points\":[\"x\"],\"action_items\":[],\"decisions\":[]}\n```";
        let parsed = parse_summary_json(content).unwrap();
        assert_eq!(parsed.key_points, vec!["x"]);
    }

    #[test]
    fn failure_classification_is_distinguishable() {
        let auth = http_failure_error(
            "DeepSeek",
            401,
            r#"{"error":{"message":"bad key sk-test-secret"}}"#,
            "sk-test-secret",
        );
        assert_eq!(auth.code, "SUMMARY_PROVIDER_ERROR");
        assert!(auth.message.contains("Key 无效或无权限"));
        assert!(auth.message.contains("DeepSeek"));
        assert!(!auth.message.contains("sk-test-secret"));
        assert!(!auth.message.contains("bad key"));

        let not_found = http_failure_error("DeepSeek", 404, "<html>nope</html>", "k");
        assert!(not_found.message.contains("HTTP 404"));
        assert!(not_found.message.contains("Base URL"));
        assert!(!not_found.message.contains("Key 无效"));
        assert!(!not_found.message.contains("html"));

        let bad_model = http_failure_error(
            "DeepSeek",
            400,
            r#"{"error":{"message":"Model Not Exist; key=sk-test-secret"}}"#,
            "sk-test-secret",
        );
        assert!(bad_model.message.contains("HTTP 400"));
        assert!(bad_model.message.contains("Model Not Exist"));
        assert!(!bad_model.message.contains("sk-test-secret"));
        assert_eq!(bad_model.details.as_ref().unwrap()["http_status"], 400);

        let long = format!(r#"{{"message":"{}"}}"#, "x".repeat(500));
        let server = http_failure_error("X", 500, &long, "k");
        assert!(server.message.contains("HTTP 500"));
        assert!(server.message.chars().count() < 300);
    }

    // ---- Local one-shot HTTP server (no external deps) ----

    struct Captured {
        request_line: String,
        headers: String,
        body: Value,
    }

    fn find_header_end(buf: &[u8]) -> Option<usize> {
        buf.windows(4).position(|w| w == b"\r\n\r\n")
    }

    fn content_length(headers: &str) -> usize {
        headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                if name.trim().eq_ignore_ascii_case("content-length") {
                    value.trim().parse().ok()
                } else {
                    None
                }
            })
            .unwrap_or(0)
    }

    /// Serves exactly one request with `status` + `body`; returns base URL `http://addr/v1/`.
    fn one_shot_server(status: u16, body: &'static str) -> (String, JoinHandle<Captured>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut buf = Vec::new();
            let mut chunk = [0u8; 4096];
            let (head_end, len) = loop {
                let n = stream.read(&mut chunk).expect("read");
                assert!(n > 0, "client closed early");
                buf.extend_from_slice(&chunk[..n]);
                if let Some(end) = find_header_end(&buf) {
                    let head = String::from_utf8_lossy(&buf[..end]).to_string();
                    let len = content_length(&head);
                    if buf.len() >= end + 4 + len {
                        break (end, len);
                    }
                }
            };
            let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
            let req_body = &buf[head_end + 4..head_end + 4 + len];
            let response = format!(
                "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).expect("write");
            let (request_line, headers) = head.split_once("\r\n").unwrap_or((&head, ""));
            Captured {
                request_line: request_line.to_string(),
                headers: headers.to_string(),
                body: serde_json::from_slice(req_body).unwrap_or(Value::Null),
            }
        });
        (format!("http://{addr}/v1/"), handle)
    }

    fn local_client() -> HttpChatClient {
        HttpChatClient::from_client(
            reqwest::blocking::Client::builder()
                .no_proxy()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap(),
        )
    }

    const SUMMARY_OK: &str = r#"{"choices":[{"message":{"content":"{\"key_points\":[\"k\"],\"action_items\":[],\"decisions\":[]}"}}]}"#;

    #[test]
    fn http_generate_deepseek_posts_configured_url_and_model() {
        let (base_url, server) = one_shot_server(200, SUMMARY_OK);
        let config = cfg("deepseek", &base_url, "deepseek-chat");
        let out = local_client()
            .generate(&config, &zh_input("t", ""))
            .expect("generate");
        assert_eq!(out.key_points, vec!["k"]);

        let captured = server.join().unwrap();
        assert_eq!(captured.request_line, "POST /v1/chat/completions HTTP/1.1");
        assert!(captured
            .headers
            .to_ascii_lowercase()
            .contains("authorization: bearer sk-test-secret"));
        assert_eq!(captured.body["model"], "deepseek-chat");
        assert_eq!(captured.body["response_format"]["type"], "json_object");
        assert!(captured.body.get("enable_thinking").is_none());
    }

    #[test]
    fn http_generate_custom_local_endpoint_sends_plain_body() {
        let (base_url, server) = one_shot_server(
            200,
            r#"{"choices":[{"message":{"content":"```json\n{\"key_points\":[],\"action_items\":[\"a\"],\"decisions\":[]}\n```"}}]}"#,
        );
        let config = cfg("custom", base_url.trim_end_matches('/'), "llama3");
        let out = local_client()
            .generate(&config, &zh_input("t", ""))
            .expect("generate");
        assert_eq!(out.action_items, vec!["a"]);

        let captured = server.join().unwrap();
        assert_eq!(captured.request_line, "POST /v1/chat/completions HTTP/1.1");
        assert_eq!(captured.body["model"], "llama3");
        assert!(captured.body.get("response_format").is_none());
        assert!(captured.body.get("enable_thinking").is_none());
    }

    #[test]
    fn http_test_connection_ok_sends_minimal_request() {
        let (base_url, server) =
            one_shot_server(200, r#"{"choices":[{"message":{"content":"p"}}]}"#);
        local_client()
            .test_connection(&cfg("deepseek", &base_url, "deepseek-chat"))
            .expect("ok");
        let captured = server.join().unwrap();
        assert_eq!(captured.request_line, "POST /v1/chat/completions HTTP/1.1");
        assert_eq!(captured.body["model"], "deepseek-chat");
        assert_eq!(captured.body["max_tokens"], 1);
        assert!(captured.body.get("response_format").is_none());
    }

    #[test]
    fn http_test_connection_bad_key() {
        let (base_url, server) = one_shot_server(
            401,
            r#"{"error":{"message":"Incorrect API key provided: sk-tes***cret"}}"#,
        );
        let err = local_client()
            .test_connection(&cfg("openai", &base_url, "gpt"))
            .expect_err("401");
        server.join().unwrap();
        assert_eq!(err.code, "SUMMARY_PROVIDER_ERROR");
        assert!(err.message.contains("Key 无效或无权限"));
        assert!(!err.message.contains("sk-tes"));
    }

    #[test]
    fn http_test_connection_bad_model_reports_status() {
        let (base_url, server) = one_shot_server(
            404,
            r#"{"error":{"message":"The model `nope` does not exist"}}"#,
        );
        let err = local_client()
            .test_connection(&cfg("openai", &base_url, "nope"))
            .expect_err("404");
        server.join().unwrap();
        assert!(err.message.contains("HTTP 404"));
        assert!(err.message.contains("does not exist"));
        assert!(!err.message.contains("Key 无效"));
    }

    #[test]
    fn http_test_connection_rejects_non_chat_response() {
        let (base_url, server) = one_shot_server(200, r#"{"hello":"world"}"#);
        let err = local_client()
            .test_connection(&cfg("custom", &base_url, "m"))
            .expect_err("not chat");
        server.join().unwrap();
        assert!(err.message.contains("Base URL"));
    }

    #[test]
    fn http_unreachable_maps_to_provider_error_without_url() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        let err = local_client()
            .test_connection(&cfg("custom", &format!("http://{addr}/v1"), "m"))
            .expect_err("unreachable");
        assert_eq!(err.code, "SUMMARY_PROVIDER_ERROR");
        assert!(err.message.contains("无法连接"));
        assert!(!err.message.contains("127.0.0.1"));
    }
}
