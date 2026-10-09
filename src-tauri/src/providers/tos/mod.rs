//! Volcengine TOS object storage — put, pre-signed GET, delete, head_bucket.

use std::path::Path;

use ve_tos_rust_sdk::auth::{PreSignedURLInput, SignerAPI};
use ve_tos_rust_sdk::bucket::{BucketAPI, HeadBucketInput};
use ve_tos_rust_sdk::enumeration::HttpMethodType::HttpMethodGet;
use ve_tos_rust_sdk::object::{DeleteObjectInput, ObjectAPI, PutObjectFromFileInput};
use ve_tos_rust_sdk::tos::{self, TosClient};

use crate::error::{AppErrorDto, CmdResult};
use crate::services::credentials::TosCredentials;

/// Pre-signed GET TTL — must outlive the 45 minute ASR poll window.
pub const PRESIGN_TTL_SECS: i64 = 2 * 60 * 60;

/// Short timeouts for credentials connectivity probes (not large uploads).
const TEST_CONNECTION_TIMEOUT_MS: isize = 20_000;
const TEST_REQUEST_TIMEOUT_MS: isize = 30_000;

/// Resolved TOS connection parameters (secrets + non-secrets).
#[derive(Debug, Clone)]
pub struct TosConfig {
    pub credentials: TosCredentials,
    pub region: String,
    pub bucket: String,
    /// Fully resolved endpoint including scheme.
    pub endpoint: String,
}

impl TosConfig {
    pub fn resolve_endpoint(region: &str, endpoint: &str) -> String {
        let trimmed = endpoint.trim();
        if !trimmed.is_empty() {
            if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
                return trimmed.to_string();
            }
            return format!("https://{trimmed}");
        }
        format!("https://tos-{}.volces.com", region.trim())
    }

    pub fn from_parts(
        credentials: TosCredentials,
        region: impl Into<String>,
        bucket: impl Into<String>,
        endpoint: impl Into<String>,
    ) -> Self {
        let region = region.into();
        let endpoint_raw = endpoint.into();
        let endpoint = Self::resolve_endpoint(&region, &endpoint_raw);
        Self {
            credentials,
            region,
            bucket: bucket.into(),
            endpoint,
        }
    }
}

/// Build object key: `meetphant/{meeting_id}/{uuid}{ext}`.
pub fn build_object_key(meeting_id: &str, source_path: &str) -> String {
    let ext = Path::new(source_path)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{}", e.to_ascii_lowercase()))
        .unwrap_or_default();
    let id = uuid::Uuid::new_v4();
    format!("meetphant/{meeting_id}/{id}{ext}")
}

/// Trait so transcription jobs can stub TOS in tests.
pub trait ObjectStorage: Send + Sync {
    fn put_file(&self, config: &TosConfig, local_path: &str, object_key: &str) -> CmdResult<()>;

    fn pre_sign_get(
        &self,
        config: &TosConfig,
        object_key: &str,
        expires_secs: i64,
    ) -> CmdResult<String>;

    fn delete_object(&self, config: &TosConfig, object_key: &str) -> CmdResult<()>;
}

fn map_tos_err(err: ve_tos_rust_sdk::error::TosError) -> AppErrorDto {
    // Never forward raw SDK Display (may include URLs/paths).
    let _ = err;
    AppErrorDto::tos_upload_error("TOS operation failed")
}

fn build_client(config: &TosConfig) -> CmdResult<impl TosClient> {
    // Large uploads need a long request timeout (up to 512 MiB).
    tos::builder()
        .ak(config.credentials.access_key_id.clone())
        .sk(config.credentials.secret_access_key.clone())
        .region(config.region.clone())
        .endpoint(config.endpoint.clone())
        .connection_timeout(30_000)
        .request_timeout(30 * 60 * 1000)
        .max_retry_count(2)
        .build()
        .map_err(map_tos_err)
}

fn build_test_client(config: &TosConfig) -> CmdResult<impl TosClient> {
    tos::builder()
        .ak(config.credentials.access_key_id.clone())
        .sk(config.credentials.secret_access_key.clone())
        .region(config.region.clone())
        .endpoint(config.endpoint.clone())
        .connection_timeout(TEST_CONNECTION_TIMEOUT_MS)
        .request_timeout(TEST_REQUEST_TIMEOUT_MS)
        .max_retry_count(1)
        .build()
        .map_err(map_tos_err)
}

pub struct HttpTosClient;

impl HttpTosClient {
    pub fn new() -> Self {
        Self
    }

    /// Connectivity probe: HeadBucket with short timeouts. Does not upload objects.
    pub fn head_bucket(&self, config: &TosConfig) -> CmdResult<()> {
        let client = build_test_client(config)?;
        let input = HeadBucketInput::new(&config.bucket);
        client.head_bucket(&input).map_err(|_| {
            // Never forward SDK Display (may include URLs/paths/secrets).
            AppErrorDto::tos_upload_error("TOS 连接失败，请检查密钥、Region 与 Bucket")
        })?;
        Ok(())
    }
}

impl Default for HttpTosClient {
    fn default() -> Self {
        Self::new()
    }
}

impl ObjectStorage for HttpTosClient {
    fn put_file(&self, config: &TosConfig, local_path: &str, object_key: &str) -> CmdResult<()> {
        let client = build_client(config)?;
        let input =
            PutObjectFromFileInput::new_with_file_path(&config.bucket, object_key, local_path);
        client.put_object_from_file(&input).map_err(map_tos_err)?;
        Ok(())
    }

    fn pre_sign_get(
        &self,
        config: &TosConfig,
        object_key: &str,
        expires_secs: i64,
    ) -> CmdResult<String> {
        let client = build_client(config)?;
        let mut input = PreSignedURLInput::new_with_key(&config.bucket, object_key);
        input.set_http_method(HttpMethodGet);
        input.set_expires(expires_secs);
        let output = client.pre_signed_url(&input).map_err(map_tos_err)?;
        let url = output.signed_url().to_string();
        if url.is_empty() {
            return Err(AppErrorDto::tos_upload_error(
                "TOS pre-signed URL was empty",
            ));
        }
        Ok(url)
    }

    fn delete_object(&self, config: &TosConfig, object_key: &str) -> CmdResult<()> {
        let client = build_client(config)?;
        let input = DeleteObjectInput::new(&config.bucket, object_key);
        client.delete_object(&input).map_err(map_tos_err)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct StubTos {
        put_ok: bool,
        deleted: Mutex<Vec<String>>,
    }

    impl ObjectStorage for StubTos {
        fn put_file(
            &self,
            _config: &TosConfig,
            _local_path: &str,
            _object_key: &str,
        ) -> CmdResult<()> {
            if self.put_ok {
                Ok(())
            } else {
                Err(AppErrorDto::tos_upload_error("stub put failed"))
            }
        }

        fn pre_sign_get(
            &self,
            _config: &TosConfig,
            object_key: &str,
            _expires_secs: i64,
        ) -> CmdResult<String> {
            Ok(format!("https://example.test/{object_key}?sig=1"))
        }

        fn delete_object(&self, _config: &TosConfig, object_key: &str) -> CmdResult<()> {
            self.deleted.lock().unwrap().push(object_key.to_string());
            Ok(())
        }
    }

    #[test]
    fn resolve_endpoint_defaults_and_custom() {
        assert_eq!(
            TosConfig::resolve_endpoint("cn-beijing", ""),
            "https://tos-cn-beijing.volces.com"
        );
        assert_eq!(
            TosConfig::resolve_endpoint("cn-beijing", "tos.example.com"),
            "https://tos.example.com"
        );
        assert_eq!(
            TosConfig::resolve_endpoint("cn-beijing", "http://local:8080"),
            "http://local:8080"
        );
    }

    #[test]
    fn object_key_includes_meeting_and_ext() {
        let key = build_object_key("meet-1", r"C:\audio\demo.WAV");
        assert!(key.starts_with("meetphant/meet-1/"));
        assert!(key.ends_with(".wav"));
    }

    #[test]
    fn stub_put_and_presign() {
        let stub = StubTos {
            put_ok: true,
            deleted: Mutex::new(vec![]),
        };
        let config = TosConfig::from_parts(
            TosCredentials {
                access_key_id: "ak".into(),
                secret_access_key: "sk".into(),
            },
            "cn-beijing",
            "bucket",
            "",
        );
        stub.put_file(&config, "/tmp/a.wav", "meetphant/m/x.wav")
            .unwrap();
        let url = stub
            .pre_sign_get(&config, "meetphant/m/x.wav", PRESIGN_TTL_SECS)
            .unwrap();
        assert!(url.contains("meetphant/m/x.wav"));
        stub.delete_object(&config, "meetphant/m/x.wav").unwrap();
        assert_eq!(stub.deleted.lock().unwrap().len(), 1);
    }
}
