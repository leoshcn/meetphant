use serde::Serialize;
use serde_json::Value;

/// IPC error envelope shared with the frontend `AppError` shape.
#[derive(Debug, Clone, Serialize)]
pub struct AppErrorDto {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

pub type CmdResult<T> = Result<T, AppErrorDto>;

impl AppErrorDto {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(
        code: impl Into<String>,
        message: impl Into<String>,
        details: Value,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: Some(details),
        }
    }

    pub fn settings_invalid(message: impl Into<String>) -> Self {
        Self::new("SETTINGS_INVALID", message)
    }

    pub fn db_error(message: impl Into<String>) -> Self {
        Self::new("DB_ERROR", message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new("INTERNAL", message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new("NOT_FOUND", message)
    }

    pub fn asr_not_configured() -> Self {
        Self::new(
            "ASR_NOT_CONFIGURED",
            "Doubao credentials are not configured",
        )
    }

    pub fn asr_payload_too_large(max_bytes: u64) -> Self {
        Self::new(
            "ASR_PAYLOAD_TOO_LARGE",
            format!("Audio file exceeds the {max_bytes} byte limit"),
        )
    }

    pub fn asr_provider_error(message: impl Into<String>) -> Self {
        Self::new("ASR_PROVIDER_ERROR", message)
    }

    pub fn asr_timeout() -> Self {
        Self::new(
            "ASR_TIMEOUT",
            "Doubao async transcription exceeded the 45 minute poll window",
        )
    }

    pub fn tos_not_configured() -> Self {
        Self::new(
            "TOS_NOT_CONFIGURED",
            "TOS credentials and bucket/region are required for files larger than 20 MiB",
        )
    }

    pub fn tos_upload_error(message: impl Into<String>) -> Self {
        Self::new("TOS_UPLOAD_ERROR", message)
    }

    pub fn io_error(message: impl Into<String>) -> Self {
        Self::new("IO_ERROR", message)
    }

    pub fn summary_not_ready() -> Self {
        Self::new(
            "SUMMARY_NOT_READY",
            "Transcript is not ready for summary generation",
        )
    }

    pub fn summary_not_configured() -> Self {
        Self::new(
            "SUMMARY_NOT_CONFIGURED",
            "摘要模型服务未配置 API Key，请先在设置中配置",
        )
    }

    pub fn summary_provider_error(message: impl Into<String>) -> Self {
        Self::new("SUMMARY_PROVIDER_ERROR", message)
    }

    pub fn invalid_argument(message: impl Into<String>) -> Self {
        Self::new("INVALID_ARGUMENT", message)
    }

    pub fn transcript_no_speakers() -> Self {
        Self::new(
            "TRANSCRIPT_NO_SPEAKERS",
            "Transcript has no speaker segments to rename",
        )
    }

    pub fn record_no_device() -> Self {
        Self::new("RECORD_NO_DEVICE", "No audio input device is available")
    }

    pub fn record_busy() -> Self {
        Self::new("RECORD_BUSY", "A recording is already in progress")
    }

    pub fn record_not_active() -> Self {
        Self::new("RECORD_NOT_ACTIVE", "No recording is in progress")
    }

    pub fn record_device_error(message: impl Into<String>) -> Self {
        Self::new("RECORD_DEVICE_ERROR", message)
    }
}

impl From<rusqlite::Error> for AppErrorDto {
    fn from(err: rusqlite::Error) -> Self {
        // Never forward rusqlite Display — it can include absolute filesystem paths.
        let _ = err;
        AppErrorDto::db_error("Database operation failed")
    }
}

impl From<serde_json::Error> for AppErrorDto {
    fn from(err: serde_json::Error) -> Self {
        let _ = err;
        AppErrorDto::db_error("Failed to encode or decode settings JSON")
    }
}

/// Error codes caused by user input; logged at debug so they do not drown real failures.
const USER_INPUT_CODES: &[&str] = &["NOT_FOUND", "SETTINGS_INVALID", "INVALID_ARGUMENT"];

fn is_user_input_error(code: &str) -> bool {
    USER_INPUT_CODES.contains(&code)
}

/// Log command failures by command name and error code only. `message` is never logged:
/// some messages carry upstream HTTP response text that may contain sensitive data.
pub trait CmdResultExt<T> {
    fn log_cmd(self, cmd: &'static str) -> CmdResult<T>;
}

impl<T> CmdResultExt<T> for CmdResult<T> {
    fn log_cmd(self, cmd: &'static str) -> CmdResult<T> {
        if let Err(err) = &self {
            if is_user_input_error(&err.code) {
                tracing::debug!(cmd, code = %err.code, "command failed");
            } else {
                tracing::warn!(cmd, code = %err.code, "command failed");
            }
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_input_codes_are_classified() {
        assert!(is_user_input_error("NOT_FOUND"));
        assert!(is_user_input_error("SETTINGS_INVALID"));
        assert!(!is_user_input_error("ASR_PROVIDER_ERROR"));
        assert!(!is_user_input_error("INTERNAL"));
    }

    #[test]
    fn log_cmd_passes_result_through() {
        let ok: CmdResult<u8> = Ok(7);
        assert_eq!(ok.log_cmd("test_cmd").unwrap(), 7);
        let err: CmdResult<u8> = Err(AppErrorDto::asr_timeout());
        assert_eq!(err.log_cmd("test_cmd").unwrap_err().code, "ASR_TIMEOUT");
    }

    #[test]
    fn rusqlite_error_maps_to_db_error() {
        let err = AppErrorDto::from(rusqlite::Error::QueryReturnedNoRows);
        assert_eq!(err.code, "DB_ERROR");
        assert_eq!(err.message, "Database operation failed");
        assert!(!err.message.contains('/') && !err.message.contains('\\'));
    }
}
