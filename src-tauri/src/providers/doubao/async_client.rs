//! Doubao Seed-ASR 2.0 recording-file recognition (submit + query) using `audio.url`.
//! This is the only ASR path: every file is uploaded to TOS first.

use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::error::{AppErrorDto, CmdResult};
use crate::providers::doubao::auth::with_auth;
use crate::providers::doubao::hotwords::{body_excludes_context_text, build_corpus_context};
use crate::services::credentials::DoubaoCredentials;

pub const ASYNC_SUBMIT_URL: &str = "https://openspeech.bytedance.com/api/v3/auc/bigmodel/submit";
pub const ASYNC_QUERY_URL: &str = "https://openspeech.bytedance.com/api/v3/auc/bigmodel/query";
/// 豆包录音文件识别模型 2.0 (`volc.bigasr.auc` is the legacy 1.0 model).
pub const ASYNC_RESOURCE_ID: &str = "volc.seedasr.auc";

pub const SUCCESS_STATUS: &str = "20000000";
pub const QUEUED_STATUS: &str = "20000001";
pub const PROCESSING_STATUS: &str = "20000002";

/// Client-side poll budget for async query loop.
pub const ASYNC_POLL_TIMEOUT: Duration = Duration::from_secs(45 * 60);
/// Default interval between query calls.
pub const ASYNC_POLL_INTERVAL: Duration = Duration::from_secs(3);

#[derive(Debug, Clone)]
pub struct AsyncSubmitInput {
    pub audio_url: String,
    pub format: String,
    pub hotwords: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct AsyncSubmitOutput {
    pub request_id: String,
    pub log_id: Option<String>,
}

#[derive(Debug, Clone)]
pub enum AsyncQueryStatus {
    Succeeded { text: String, raw_json: String },
    InProgress,
    Failed { message: String },
}

#[derive(Debug, Clone)]
pub struct AsyncRecognizeOutput {
    pub text: String,
    pub raw_json: String,
    #[allow(dead_code)]
    pub request_id: String,
}

#[derive(Debug, Deserialize)]
struct AsyncResponseBody {
    result: Option<AsyncResult>,
}

#[derive(Debug, Deserialize)]
struct AsyncResult {
    text: Option<String>,
}

/// Build submit JSON body. Uses `audio.url`; never includes Meetphant `context_text`.
pub fn build_async_submit_body(input: &AsyncSubmitInput) -> Value {
    let mut request = json!({
        "model_name": "bigmodel",
        "enable_speaker_info": true,
        "show_utterances": true,
        "ssd_version": "200",
    });

    if let Some(context) = build_corpus_context(&input.hotwords) {
        request["corpus"] = json!({ "context": context });
    }

    let body = json!({
        "user": { "uid": "meetphant" },
        "audio": {
            "url": input.audio_url,
            "format": input.format,
        },
        "request": request,
    });

    debug_assert!(body_excludes_context_text(&body));
    body
}

pub fn audio_format_from_path(path: &str) -> String {
    std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .filter(|e| {
            matches!(
                e.as_str(),
                "wav" | "mp3" | "ogg" | "m4a" | "flac" | "aac" | "wma" | "mp4"
            )
        })
        .unwrap_or_else(|| "mp3".to_string())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProbeVerdict {
    /// Key authenticated and the 2.0 resource answered (even "task not found").
    Ok,
    /// Key rejected or 2.0 resource not granted.
    AuthFailed,
    /// Provider-side failure or a response without status headers.
    Unexpected,
}

/// Classify a query response for an unknown random request id.
/// Not in the official docs: HTTP 401/403 are treated as auth failure, `55xxxxxx`
/// as server error, any other provider status as proof the key works.
pub fn classify_probe(http_status: u16, status_code: &str) -> ProbeVerdict {
    if http_status == 401 || http_status == 403 {
        return ProbeVerdict::AuthFailed;
    }
    if status_code.is_empty() || status_code.starts_with("55") || http_status >= 500 {
        return ProbeVerdict::Unexpected;
    }
    ProbeVerdict::Ok
}

/// Trait so transcription jobs can stub async ASR in tests.
pub trait AsyncRecognizer: Send + Sync {
    fn submit(
        &self,
        credentials: &DoubaoCredentials,
        input: &AsyncSubmitInput,
    ) -> CmdResult<AsyncSubmitOutput>;

    fn query(
        &self,
        credentials: &DoubaoCredentials,
        request_id: &str,
        log_id: Option<&str>,
    ) -> CmdResult<AsyncQueryStatus>;
}

/// Poll until success/failure or timeout.
pub fn poll_until_done(
    recognizer: &dyn AsyncRecognizer,
    credentials: &DoubaoCredentials,
    request_id: &str,
    log_id: Option<&str>,
    timeout: Duration,
    interval: Duration,
) -> CmdResult<AsyncRecognizeOutput> {
    let started = Instant::now();
    loop {
        match recognizer.query(credentials, request_id, log_id)? {
            AsyncQueryStatus::Succeeded { text, raw_json } => {
                return Ok(AsyncRecognizeOutput {
                    text,
                    raw_json,
                    request_id: request_id.to_string(),
                });
            }
            AsyncQueryStatus::Failed { message } => {
                return Err(AppErrorDto::asr_provider_error(message));
            }
            AsyncQueryStatus::InProgress => {
                if started.elapsed() >= timeout {
                    return Err(AppErrorDto::asr_timeout());
                }
                std::thread::sleep(interval);
            }
        }
    }
}

pub struct HttpAsyncClient {
    client: reqwest::blocking::Client,
}

impl HttpAsyncClient {
    pub fn new() -> CmdResult<Self> {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|_| AppErrorDto::internal("Failed to create HTTP client"))?;
        Ok(Self { client })
    }

    /// Credential probe for「测试连接」: query a random request id. Needs no TOS
    /// and no recognition quota; proves the API Key and 2.0 resource access.
    pub fn probe_auth(&self, credentials: &DoubaoCredentials) -> CmdResult<()> {
        let response = with_auth(self.client.post(ASYNC_QUERY_URL), credentials)
            .header("Content-Type", "application/json")
            .header("X-Api-Resource-Id", ASYNC_RESOURCE_ID)
            .header("X-Api-Request-Id", Uuid::new_v4().to_string())
            .json(&json!({}))
            .send()
            .map_err(|_| AppErrorDto::asr_provider_error("Failed to reach Doubao ASR"))?;

        let http_status = response.status().as_u16();
        let status_code = header_str(&response, "X-Api-Status-Code");
        let api_message = header_str(&response, "X-Api-Message");
        let _ = response.text();

        match classify_probe(http_status, &status_code) {
            ProbeVerdict::Ok => Ok(()),
            ProbeVerdict::AuthFailed => Err(AppErrorDto::asr_provider_error(format!(
                "豆包 API Key 无效或未开通录音文件识别模型 2.0 ({http_status} {status_code}): {api_message}"
            ))),
            ProbeVerdict::Unexpected => Err(AppErrorDto::asr_provider_error(format!(
                "Doubao ASR probe failed ({http_status} {status_code}): {api_message}"
            ))),
        }
    }
}

fn header_str(response: &reqwest::blocking::Response, name: &str) -> String {
    response
        .headers()
        .get(name)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string()
}

impl AsyncRecognizer for HttpAsyncClient {
    fn submit(
        &self,
        credentials: &DoubaoCredentials,
        input: &AsyncSubmitInput,
    ) -> CmdResult<AsyncSubmitOutput> {
        let request_id = Uuid::new_v4().to_string();
        let body = build_async_submit_body(input);

        let response = with_auth(self.client.post(ASYNC_SUBMIT_URL), credentials)
            .header("Content-Type", "application/json")
            .header("X-Api-Resource-Id", ASYNC_RESOURCE_ID)
            .header("X-Api-Request-Id", &request_id)
            .header("X-Api-Sequence", "-1")
            .json(&body)
            .send()
            .map_err(|_| AppErrorDto::asr_provider_error("Failed to reach Doubao ASR submit"))?;

        let status_code = header_str(&response, "X-Api-Status-Code");
        let api_message = header_str(&response, "X-Api-Message");
        let log_id = {
            let v = header_str(&response, "X-Tt-Logid");
            if v.is_empty() {
                None
            } else {
                Some(v)
            }
        };

        // Consume body without logging (may echo URL).
        let _ = response.text();

        if status_code != SUCCESS_STATUS {
            return Err(AppErrorDto::asr_provider_error(format!(
                "Doubao ASR submit failed ({status_code}): {api_message}"
            )));
        }

        Ok(AsyncSubmitOutput { request_id, log_id })
    }

    fn query(
        &self,
        credentials: &DoubaoCredentials,
        request_id: &str,
        log_id: Option<&str>,
    ) -> CmdResult<AsyncQueryStatus> {
        let mut req = with_auth(self.client.post(ASYNC_QUERY_URL), credentials)
            .header("Content-Type", "application/json")
            .header("X-Api-Resource-Id", ASYNC_RESOURCE_ID)
            .header("X-Api-Request-Id", request_id);

        if let Some(log_id) = log_id {
            req = req.header("X-Tt-Logid", log_id);
        }

        let response = req
            .json(&json!({}))
            .send()
            .map_err(|_| AppErrorDto::asr_provider_error("Failed to reach Doubao ASR query"))?;

        let status_code = header_str(&response, "X-Api-Status-Code");
        let api_message = header_str(&response, "X-Api-Message");
        let raw = response
            .text()
            .map_err(|_| AppErrorDto::asr_provider_error("Failed to read ASR query response"))?;

        if status_code == SUCCESS_STATUS {
            let parsed: AsyncResponseBody = serde_json::from_str(&raw)
                .map_err(|_| AppErrorDto::asr_provider_error("Invalid ASR query response JSON"))?;
            let text = parsed.result.and_then(|r| r.text).unwrap_or_default();
            return Ok(AsyncQueryStatus::Succeeded {
                text,
                raw_json: raw,
            });
        }

        if status_code == QUEUED_STATUS || status_code == PROCESSING_STATUS {
            return Ok(AsyncQueryStatus::InProgress);
        }

        Ok(AsyncQueryStatus::Failed {
            message: format!("Doubao ASR query failed ({status_code}): {api_message}"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn async_body_uses_url_and_hotwords_not_context_text() {
        let body = build_async_submit_body(&AsyncSubmitInput {
            audio_url: "https://example.test/a.wav".into(),
            format: "wav".into(),
            hotwords: vec!["Meetphant".into()],
        });
        assert_eq!(body["audio"]["url"], "https://example.test/a.wav");
        assert!(body["audio"].get("data").is_none());
        let ctx = body["request"]["corpus"]["context"].as_str().unwrap();
        assert!(ctx.contains("Meetphant"));
        assert!(!body.to_string().contains("context_text"));
        assert_eq!(body["request"]["enable_speaker_info"], true);
        assert_eq!(body["request"]["show_utterances"], true);
    }

    struct StubAsync {
        queries_before_success: usize,
        calls: AtomicUsize,
        fail: bool,
    }

    impl AsyncRecognizer for StubAsync {
        fn submit(
            &self,
            _credentials: &DoubaoCredentials,
            input: &AsyncSubmitInput,
        ) -> CmdResult<AsyncSubmitOutput> {
            assert!(!input.audio_url.is_empty());
            let body = build_async_submit_body(input);
            assert!(!body.to_string().contains("context_text"));
            Ok(AsyncSubmitOutput {
                request_id: "req-1".into(),
                log_id: Some("log-1".into()),
            })
        }

        fn query(
            &self,
            _credentials: &DoubaoCredentials,
            _request_id: &str,
            _log_id: Option<&str>,
        ) -> CmdResult<AsyncQueryStatus> {
            if self.fail {
                return Ok(AsyncQueryStatus::Failed {
                    message: "provider boom".into(),
                });
            }
            let n = self.calls.fetch_add(1, Ordering::SeqCst);
            if n + 1 >= self.queries_before_success {
                Ok(AsyncQueryStatus::Succeeded {
                    text: "async hello".into(),
                    raw_json: r#"{"result":{"text":"async hello"}}"#.into(),
                })
            } else {
                Ok(AsyncQueryStatus::InProgress)
            }
        }
    }

    #[test]
    fn poll_succeeds_after_in_progress() {
        let stub = StubAsync {
            queries_before_success: 2,
            calls: AtomicUsize::new(0),
            fail: false,
        };
        let creds = DoubaoCredentials {
            api_key: "k".into(),
        };
        let out = poll_until_done(
            &stub,
            &creds,
            "req-1",
            None,
            Duration::from_secs(5),
            Duration::from_millis(1),
        )
        .unwrap();
        assert_eq!(out.text, "async hello");
    }

    #[test]
    fn poll_times_out() {
        let stub = StubAsync {
            queries_before_success: 1000,
            calls: AtomicUsize::new(0),
            fail: false,
        };
        let creds = DoubaoCredentials {
            api_key: "k".into(),
        };
        let err = poll_until_done(
            &stub,
            &creds,
            "req-1",
            None,
            Duration::from_millis(20),
            Duration::from_millis(5),
        )
        .expect_err("timeout");
        assert_eq!(err.code, "ASR_TIMEOUT");
    }

    #[test]
    fn resource_id_is_seed_asr_2() {
        assert_eq!(ASYNC_RESOURCE_ID, "volc.seedasr.auc");
    }

    #[test]
    fn format_from_path() {
        assert_eq!(audio_format_from_path(r"C:\a\b.WAV"), "wav");
        assert_eq!(audio_format_from_path("/tmp/x.mp3"), "mp3");
        assert_eq!(audio_format_from_path("/tmp/x.unknown"), "mp3");
    }

    #[test]
    fn probe_classification() {
        assert_eq!(classify_probe(401, ""), ProbeVerdict::AuthFailed);
        assert_eq!(classify_probe(403, "45000030"), ProbeVerdict::AuthFailed);
        assert_eq!(classify_probe(200, "45000001"), ProbeVerdict::Ok);
        assert_eq!(classify_probe(200, SUCCESS_STATUS), ProbeVerdict::Ok);
        assert_eq!(classify_probe(200, ""), ProbeVerdict::Unexpected);
        assert_eq!(classify_probe(200, "55000031"), ProbeVerdict::Unexpected);
        assert_eq!(classify_probe(502, "20000000"), ProbeVerdict::Unexpected);
    }
}
